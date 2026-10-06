//! Type representation and the inference (unification) table.

use tarn_resolve::SymbolId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
}

impl IntTy {
    pub fn name(self) -> &'static str {
        use IntTy::*;
        match self {
            I8 => "i8",
            I16 => "i16",
            I32 => "i32",
            I64 => "i64",
            Isize => "isize",
            U8 => "u8",
            U16 => "u16",
            U32 => "u32",
            U64 => "u64",
            Usize => "usize",
        }
    }

    pub fn signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize)
    }

    /// Inclusive range of values, as i128.
    pub fn range(self) -> (i128, i128) {
        use IntTy::*;
        match self {
            I8 => (i8::MIN as i128, i8::MAX as i128),
            I16 => (i16::MIN as i128, i16::MAX as i128),
            I32 => (i32::MIN as i128, i32::MAX as i128),
            I64 | Isize => (i64::MIN as i128, i64::MAX as i128),
            U8 => (0, u8::MAX as i128),
            U16 => (0, u16::MAX as i128),
            U32 => (0, u32::MAX as i128),
            U64 | Usize => (0, u64::MAX as i128),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FloatTy {
    F32,
    F64,
}

/// Identity of a generic type parameter: the `SymbolId` of its declaration,
/// or a synthetic id (above every symbol) for prelude types' parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParamId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Bool,
    Int(IntTy),
    Float(FloatTy),
    Str,
    Void,
    /// Type of expressions that never produce a value (`panic(..)`).
    Never,
    /// Struct, enum or prelude type with its type arguments.
    Adt(SymbolId, Vec<Ty>),
    Ref(bool, Box<Ty>),
    Array(Box<Ty>, u64),
    /// Unsized; only valid behind a reference.
    Slice(Box<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    /// A generic parameter, rigid inside its declaration.
    Param(ParamId),
    /// `any I` — dynamic dispatch through interface `I`.
    Any(SymbolId),
    /// Inference variable.
    Var(u32),
    /// Member of a standard-library module that does not exist yet: accepted
    /// everywhere, never reported (see `docs/types.md`, "opaque std").
    Opaque,
    /// An error was already reported for this expression.
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    General,
    /// Integer literal: unifies only with integer types; defaults to `i64`.
    Int,
    /// Float literal: unifies only with float types; defaults to `f64`.
    Float,
}

#[derive(Default)]
pub struct Infer {
    vars: Vec<(VarKind, Option<Ty>)>,
}

impl Infer {
    pub fn fresh(&mut self, kind: VarKind) -> Ty {
        self.vars.push((kind, None));
        Ty::Var(self.vars.len() as u32 - 1)
    }

    pub fn kind(&self, v: u32) -> VarKind {
        self.vars[v as usize].0
    }

    /// Follow bound variables at the top level only.
    pub fn shallow(&self, t: &Ty) -> Ty {
        let mut t = t.clone();
        while let Ty::Var(v) = t {
            match &self.vars[v as usize].1 {
                Some(x) => t = x.clone(),
                None => return t,
            }
        }
        t
    }

    /// Fully substitute bound variables.
    pub fn zonk(&self, t: &Ty) -> Ty {
        match self.shallow(t) {
            Ty::Adt(s, args) => Ty::Adt(s, args.iter().map(|a| self.zonk(a)).collect()),
            Ty::Ref(m, x) => Ty::Ref(m, Box::new(self.zonk(&x))),
            Ty::Array(x, n) => Ty::Array(Box::new(self.zonk(&x)), n),
            Ty::Slice(x) => Ty::Slice(Box::new(self.zonk(&x))),
            Ty::Fn(ps, r) => Ty::Fn(ps.iter().map(|p| self.zonk(p)).collect(), Box::new(self.zonk(&r))),
            t => t,
        }
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.shallow(t) {
            Ty::Var(w) => v == w,
            Ty::Adt(_, args) => args.iter().any(|a| self.occurs(v, a)),
            Ty::Ref(_, x) | Ty::Array(x, _) | Ty::Slice(x) => self.occurs(v, &x),
            Ty::Fn(ps, r) => ps.iter().any(|p| self.occurs(v, p)) || self.occurs(v, &r),
            _ => false,
        }
    }

    fn bind(&mut self, v: u32, t: Ty) -> bool {
        let ok = match (self.kind(v), &t) {
            (VarKind::General, _) => true,
            (VarKind::Int, Ty::Int(_)) | (VarKind::Float, Ty::Float(_)) => true,
            (_, Ty::Opaque | Ty::Error | Ty::Never) => true,
            _ => false,
        };
        if ok && !self.occurs(v, &t) {
            self.vars[v as usize].1 = Some(t);
            true
        } else {
            false
        }
    }

    pub fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let (a, b) = (self.shallow(a), self.shallow(b));
        match (&a, &b) {
            (Ty::Opaque | Ty::Error, _) | (_, Ty::Opaque | Ty::Error) => true,
            (Ty::Var(x), Ty::Var(y)) if x == y => true,
            (Ty::Var(x), Ty::Var(y)) => {
                // Keep the more specific kind.
                let (kx, ky) = (self.kind(*x), self.kind(*y));
                match (kx, ky) {
                    (VarKind::General, _) => self.bind(*x, b.clone()),
                    (_, VarKind::General) => self.bind(*y, a.clone()),
                    _ if kx == ky => self.bind(*x, b.clone()),
                    _ => false,
                }
            }
            (Ty::Var(x), _) => self.bind(*x, b.clone()),
            (_, Ty::Var(y)) => self.bind(*y, a.clone()),
            (Ty::Never, _) | (_, Ty::Never) => true,
            (Ty::Adt(s1, a1), Ty::Adt(s2, a2)) => {
                s1 == s2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| self.unify(x, y))
            }
            (Ty::Ref(m1, x), Ty::Ref(m2, y)) => m1 == m2 && self.unify(x, y),
            (Ty::Array(x, n1), Ty::Array(y, n2)) => n1 == n2 && self.unify(x, y),
            (Ty::Slice(x), Ty::Slice(y)) => self.unify(x, y),
            (Ty::Fn(p1, r1), Ty::Fn(p2, r2)) => {
                p1.len() == p2.len() && p1.iter().zip(p2).all(|(x, y)| self.unify(x, y)) && self.unify(r1, r2)
            }
            _ => a == b,
        }
    }

    /// Unresolved literal variables become `i64` / `f64`. Returns the general
    /// variables that are still unresolved.
    pub fn default_literals(&mut self) {
        for i in 0..self.vars.len() {
            if self.vars[i].1.is_none() {
                match self.vars[i].0 {
                    VarKind::Int => self.vars[i].1 = Some(Ty::Int(IntTy::I64)),
                    VarKind::Float => self.vars[i].1 = Some(Ty::Float(FloatTy::F64)),
                    VarKind::General => {}
                }
            }
        }
    }

    pub fn has_unresolved(&self, t: &Ty) -> bool {
        match self.zonk(t) {
            Ty::Var(_) => true,
            Ty::Adt(_, args) => args.iter().any(|a| self.has_unresolved(a)),
            Ty::Ref(_, x) | Ty::Array(x, _) | Ty::Slice(x) => self.has_unresolved(&x),
            Ty::Fn(ps, r) => ps.iter().any(|p| self.has_unresolved(p)) || self.has_unresolved(&r),
            _ => false,
        }
    }
}
