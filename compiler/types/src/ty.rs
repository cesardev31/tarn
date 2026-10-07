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
    /// Raw pointer `*T` / `*mut T` (ADR 0047): Copy, no loan, no capabilities.
    Ptr(bool, Box<Ty>),
    Array(Box<Ty>, u64),
    /// Unsized; only valid behind a reference.
    Slice(Box<Ty>),
    Fn(tarn_ast::CallMode, Vec<Ty>, Box<Ty>),
    /// Source-level owned computation; the declared function output is the inner type.
    Async(Box<Ty>),
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

/// Unification table. Every binding is recorded in an undo log so that a
/// failed `unify` restores exactly the previous state: a failure never leaves
/// variables partially bound.
#[derive(Default)]
pub struct Infer {
    vars: Vec<(VarKind, Option<Ty>)>,
    /// Variables bound since the oldest open checkpoint, in binding order.
    undo: Vec<u32>,
    open_checkpoints: u32,
}

/// Position in the undo log.
pub struct Checkpoint(usize);

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
            Ty::Ptr(m, x) => Ty::Ptr(m, Box::new(self.zonk(&x))),
            Ty::Array(x, n) => Ty::Array(Box::new(self.zonk(&x)), n),
            Ty::Slice(x) => Ty::Slice(Box::new(self.zonk(&x))),
            Ty::Fn(mode, ps, r) => Ty::Fn(mode, ps.iter().map(|p| self.zonk(p)).collect(), Box::new(self.zonk(&r))),
            Ty::Async(r) => Ty::Async(Box::new(self.zonk(&r))),
            t => t,
        }
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.shallow(t) {
            Ty::Var(w) => v == w,
            Ty::Adt(_, args) => args.iter().any(|a| self.occurs(v, a)),
            Ty::Ref(_, x) | Ty::Ptr(_, x) | Ty::Array(x, _) | Ty::Slice(x) => self.occurs(v, &x),
            Ty::Fn(_, ps, r) => ps.iter().any(|p| self.occurs(v, p)) || self.occurs(v, &r),
            Ty::Async(r) => self.occurs(v, &r),
            _ => false,
        }
    }

    fn bind(&mut self, v: u32, t: Ty) -> bool {
        let ok = match (self.kind(v), &t) {
            (VarKind::General, _) => true,
            (VarKind::Int, Ty::Int(_)) | (VarKind::Float, Ty::Float(_)) => true,
            // Two literal variables of the same kind.
            (k, Ty::Var(w)) => self.kind(*w) == k,
            (_, Ty::Opaque | Ty::Error | Ty::Never) => true,
            _ => false,
        };
        if ok && !self.occurs(v, &t) {
            self.vars[v as usize].1 = Some(t);
            if self.open_checkpoints > 0 {
                self.undo.push(v);
            }
            true
        } else {
            false
        }
    }

    pub fn checkpoint(&mut self) -> Checkpoint {
        self.open_checkpoints += 1;
        Checkpoint(self.undo.len())
    }

    /// Undo every binding made since `cp`.
    pub fn rollback(&mut self, cp: Checkpoint) {
        while self.undo.len() > cp.0 {
            let v = self.undo.pop().unwrap();
            self.vars[v as usize].1 = None;
        }
        self.close(cp.0);
    }

    /// Keep the bindings made since `cp`.
    pub fn commit(&mut self, cp: Checkpoint) {
        self.close(cp.0);
    }

    fn close(&mut self, _at: usize) {
        self.open_checkpoints -= 1;
        if self.open_checkpoints == 0 {
            self.undo.clear();
        }
    }

    /// Atomic unification: on failure the table is exactly as before.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let cp = self.checkpoint();
        let ok = self.unify_inner(a, b);
        if ok {
            self.commit(cp);
        } else {
            self.rollback(cp);
        }
        ok
    }

    fn unify_inner(&mut self, a: &Ty, b: &Ty) -> bool {
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
            (Ty::Adt(s1, a1), Ty::Adt(s2, a2)) => s1 == s2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| self.unify_inner(x, y)),
            (Ty::Ref(m1, x), Ty::Ref(m2, y)) => m1 == m2 && self.unify_inner(x, y),
            (Ty::Ptr(m1, x), Ty::Ptr(m2, y)) => m1 == m2 && self.unify_inner(x, y),
            (Ty::Array(x, n1), Ty::Array(y, n2)) => n1 == n2 && self.unify_inner(x, y),
            (Ty::Slice(x), Ty::Slice(y)) => self.unify_inner(x, y),
            (Ty::Fn(m1, p1, r1), Ty::Fn(m2, p2, r2)) => m1 == m2 && p1.len() == p2.len() && p1.iter().zip(p2).all(|(x, y)| self.unify_inner(x, y)) && self.unify_inner(r1, r2),
            (Ty::Async(a), Ty::Async(b)) => self.unify_inner(a, b),
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
            Ty::Ref(_, x) | Ty::Ptr(_, x) | Ty::Array(x, _) | Ty::Slice(x) => self.has_unresolved(&x),
            Ty::Fn(_, ps, r) => ps.iter().any(|p| self.has_unresolved(p)) || self.has_unresolved(&r),
            Ty::Async(r) => self.has_unresolved(&r),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tarn_resolve::SymbolId;

    fn pair(a: Ty, b: Ty) -> Ty {
        Ty::Adt(SymbolId(1), vec![a, b])
    }

    /// `Pair<?a, ?b>` vs `Pair<i64, bool>` where `?b` is an integer literal:
    /// `?a := i64` happens before `?b` fails; the failure must undo it.
    #[test]
    fn failed_unify_restores_partial_bindings() {
        let mut inf = Infer::default();
        let a = inf.fresh(VarKind::General);
        let b = inf.fresh(VarKind::Int);
        assert!(!inf.unify(&pair(a.clone(), b.clone()), &pair(Ty::Int(IntTy::I64), Ty::Bool)));
        assert_eq!(inf.shallow(&a), a, "?a must be unbound again");
        assert_eq!(inf.shallow(&b), b);
        // A later, valid unification is unaffected by the failed one.
        assert!(inf.unify(&a, &Ty::Str));
        assert_eq!(inf.zonk(&a), Ty::Str);
    }

    #[test]
    fn nested_failure_deep_in_the_structure() {
        let mut inf = Infer::default();
        let (x, y, z) = (inf.fresh(VarKind::General), inf.fresh(VarKind::General), inf.fresh(VarKind::Float));
        let lhs = Ty::Fn(tarn_ast::CallMode::Shared, vec![x.clone(), pair(y.clone(), z.clone())], Box::new(Ty::Void));
        let rhs = Ty::Fn(tarn_ast::CallMode::Shared, vec![Ty::Bool, pair(Ty::Str, Ty::Int(IntTy::U8))], Box::new(Ty::Void));
        assert!(!inf.unify(&lhs, &rhs));
        for v in [&x, &y, &z] {
            assert!(matches!(inf.shallow(v), Ty::Var(_)));
        }
    }

    #[test]
    fn success_keeps_bindings_and_outer_checkpoint_rolls_back() {
        let mut inf = Infer::default();
        let a = inf.fresh(VarKind::General);
        assert!(inf.unify(&pair(a.clone(), Ty::Bool), &pair(Ty::Str, Ty::Bool)));
        assert_eq!(inf.zonk(&a), Ty::Str);
        // An outer checkpoint spanning several unifications rolls back all of
        // them, including the ones that succeeded.
        let b = inf.fresh(VarKind::General);
        let cp = inf.checkpoint();
        assert!(inf.unify(&b, &Ty::Bool));
        assert!(!inf.unify(&Ty::Str, &Ty::Bool));
        inf.rollback(cp);
        assert!(matches!(inf.shallow(&b), Ty::Var(_)), "the successful step is undone too");
    }
}
