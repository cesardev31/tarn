//! Declarations as types: struct/enum layouts and function signatures,
//! lowered from the AST using the resolver's tables.

use crate::ty::*;
use std::collections::HashMap;
use tarn_ast::{FnDecl, GenericParam, ItemKind, ReceiverKind, Type, TypeKind};
use tarn_diagnostics::{Diagnostic, Span};
use tarn_resolve::{ModuleId, ModuleInput, Res, Resolved, SymbolId, SymbolKind};

pub struct FieldDef {
    pub name: String,
    pub ty: Ty,
    pub is_pub: bool,
}

pub struct StructDef {
    pub module: ModuleId,
    pub generics: Vec<ParamId>,
    pub fields: Vec<FieldDef>,
    pub is_copy: bool,
}

pub struct VariantDef {
    pub sym: SymbolId,
    pub name: String,
    pub fields: Vec<Ty>,
}

pub struct EnumDef {
    pub generics: Vec<ParamId>,
    pub variants: Vec<VariantDef>,
    pub is_copy: bool,
}

#[derive(Clone)]
pub struct FnSig {
    /// Owner binders, then the function's own generics.
    pub generics: Vec<ParamId>,
    pub receiver: Option<ReceiverKind>,
    /// Type of `self` for methods of structs/enums and impl methods.
    pub self_ty: Option<Ty>,
    pub params: Vec<Ty>,
    pub ret: Ty,
    /// `extern "C"` / `extern "intrinsic"`.
    pub abi: Option<String>,
    pub module: ModuleId,
    pub span: Span,
}

/// Prelude symbols the checker needs by identity.
/// The "lang items": the only names the type checker knows by identity.
/// `Option`, `Result` and `Copy` are declared in `core` (ADR 0020).
pub struct Prelude {
    pub option: SymbolId,
    pub result: SymbolId,
    /// `Copy` capability; `None` only when compiling without `core`.
    pub copy: Option<SymbolId>,
    pub channel: SymbolId,
    pub print: SymbolId,
    pub panic: SymbolId,
    pub channel_fn: SymbolId,
}

/// Declarations as types. Produced by the type checker and handed to later
/// phases (IR lowering) so they never re-derive layouts or signatures.
#[derive(Default)]
pub struct Decls {
    pub structs: HashMap<SymbolId, StructDef>,
    pub enums: HashMap<SymbolId, EnumDef>,
    pub fns: HashMap<SymbolId, FnSig>,
    pub bounds: HashMap<ParamId, Vec<SymbolId>>,
    pub param_names: HashMap<ParamId, String>,
    /// The `Copy` capability from `core`, if loaded.
    pub copy: Option<SymbolId>,
}

impl Decls {
    /// Is `t` a copy type? The single definition used by the type checker and
    /// by IR lowering (ADR 0021). Inference variables are not handled here.
    pub fn is_copy(&self, t: &Ty) -> bool {
        match t {
            Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::Never | Ty::Void | Ty::Opaque | Ty::Error => true,
            Ty::Ref(m, _) => !m,
            Ty::Array(e, _) => self.is_copy(e),
            Ty::Adt(s, _) => self.structs.get(s).is_some_and(|d| d.is_copy) || self.enums.get(s).is_some_and(|d| d.is_copy),
            Ty::Param(p) => self.copy.is_some_and(|c| self.bounds.get(p).is_some_and(|b| b.contains(&c))),
            _ => false,
        }
    }
}

pub struct Env<'a> {
    pub inputs: &'a [ModuleInput<'a>],
    pub r: &'a Resolved,
    pub decls: Decls,
    pub prelude: Prelude,
    pub diags: Vec<Diagnostic>,
}

impl<'a> Env<'a> {
    pub fn new(inputs: &'a [ModuleInput<'a>], r: &'a Resolved) -> Env<'a> {
        let ps = r.scope(tarn_resolve::ScopeId(0));
        let get = |n: &str| ps.get(n).expect("core must declare the lang items");
        let prelude = Prelude {
            option: get("Option"),
            result: get("Result"),
            copy: ps.get("Copy"),
            channel: get("Channel"),
            print: get("print"),
            panic: get("panic"),
            channel_fn: get("channel"),
        };
        let decls = Decls { copy: prelude.copy, ..Decls::default() };
        let mut env = Env { inputs, r, decls, prelude, diags: Vec::new() };
        env.collect();
        env
    }

    fn def(&self, m: ModuleId, node: tarn_ast::NodeId) -> Option<SymbolId> {
        self.r.tables[m.0 as usize].defs.get(&node).copied()
    }

    fn params_of(&mut self, m: ModuleId, gs: &[GenericParam]) -> Vec<ParamId> {
        let mut out = Vec::new();
        for g in gs {
            let Some(s) = self.def(m, g.id) else { continue };
            let p = ParamId(s.0);
            self.decls.param_names.insert(p, g.name.name.clone());
            let bounds: Vec<SymbolId> = g
                .bounds
                .iter()
                .filter_map(|b| match self.r.tables[m.0 as usize].uses.get(&b.id).map(|u| &u.res) {
                    Some(Res::Symbol(i)) => Some(*i),
                    _ => None,
                })
                .collect();
            self.decls.bounds.insert(p, bounds);
            out.push(p);
        }
        out
    }

    fn collect(&mut self) {
        self.collect_pass(true);
        self.collect_pass(false);
    }

    /// Pass 1 (`types`): structs and enums. Pass 2: functions, interfaces and
    /// impls, which may need the types' declared bounds.
    fn collect_pass(&mut self, types: bool) {
        for (mi, input) in self.inputs.iter().enumerate() {
            let m = ModuleId(mi as u32);
            for item in &input.ast.items {
                let is_type = matches!(item.kind, ItemKind::Struct(_) | ItemKind::Enum(_));
                if is_type != types {
                    continue;
                }
                match &item.kind {
                    ItemKind::Struct(s) => {
                        let Some(sym) = self.def(m, item.id) else { continue };
                        let generics = self.params_of(m, &s.generics);
                        let fields = s
                            .fields
                            .iter()
                            .map(|f| FieldDef { name: f.name.name.clone(), ty: self.lower(m, &f.ty), is_pub: f.is_pub })
                            .collect();
                        self.decls.structs.insert(sym, StructDef { module: m, generics, fields, is_copy: s.is_copy });
                    }
                    ItemKind::Enum(e) => {
                        let Some(sym) = self.def(m, item.id) else { continue };
                        let generics = self.params_of(m, &e.generics);
                        let variants = e
                            .variants
                            .iter()
                            .filter_map(|v| {
                                let vs = self.def(m, v.id)?;
                                let fields = v.fields.iter().map(|t| self.lower(m, t)).collect();
                                Some(VariantDef { sym: vs, name: v.name.name.clone(), fields })
                            })
                            .collect();
                        self.decls.enums.insert(sym, EnumDef { generics, variants, is_copy: e.is_copy });
                    }
                    ItemKind::Fn(f) => {
                        // The resolver records `fn T.m`'s owner on the item node.
                        let owner = match self.r.tables[mi].uses.get(&item.id).map(|u| &u.res) {
                            Some(Res::Symbol(o)) => Some(*o),
                            _ => None,
                        };
                        self.fn_sig(m, f, &[], None, owner)
                    }
                    ItemKind::Interface(i) => {
                        self.params_of(m, &i.generics);
                        for f in &i.methods {
                            self.fn_sig(m, f, &[], None, None);
                        }
                    }
                    ItemKind::Impl(i) => {
                        // Target binders were declared with the argument types' ids.
                        let (target, binders) = match (&i.target.kind, self.r.tables[mi].uses.get(&i.target.id).map(|u| &u.res)) {
                            (TypeKind::Path(p), Some(Res::Symbol(t))) => {
                                let bs: Vec<ParamId> = p.args.iter().filter_map(|a| self.def(m, a.id)).map(|s| ParamId(s.0)).collect();
                                for (a, b) in p.args.iter().zip(&bs) {
                                    if let TypeKind::Path(ap) = &a.kind {
                                        self.decls.param_names.insert(*b, ap.segments[0].name.clone());
                                    }
                                }
                                (Some(*t), bs)
                            }
                            _ => (None, Vec::new()),
                        };
                        if let Some(t) = target {
                            self.inherit_bounds(t, &binders);
                        }
                        let self_ty = target.map(|t| Ty::Adt(t, binders.iter().map(|b| Ty::Param(*b)).collect()));
                        for f in &i.methods {
                            self.fn_sig(m, f, &binders, self_ty.clone(), None);
                        }
                    }
                    ItemKind::Import(_) | ItemKind::Error => {}
                }
            }
        }
    }

    fn fn_sig(&mut self, m: ModuleId, f: &FnDecl, outer: &[ParamId], impl_self: Option<Ty>, owner: Option<SymbolId>) {
        let Some(sym) = self.def(m, f.id) else { return };
        let mut generics = outer.to_vec();
        let mut self_ty = impl_self;
        if let Some(o) = &f.owner {
            let binders = self.params_of(m, &o.params);
            if let Some(owner) = owner {
                self_ty = Some(Ty::Adt(owner, binders.iter().map(|b| Ty::Param(*b)).collect()));
                self.inherit_bounds(owner, &binders);
            }
            generics.extend(binders);
        }
        generics.extend(self.params_of(m, &f.generics));
        let params = f.params.iter().map(|p| self.lower(m, &p.ty)).collect();
        let ret = f.ret.as_ref().map(|t| self.lower(m, t)).unwrap_or(Ty::Void);
        let sig = FnSig {
            generics,
            receiver: f.receiver.as_ref().map(|r| r.kind),
            self_ty,
            params,
            ret,
            abi: f.abi.clone(),
            module: m,
            span: f.name.span,
        };
        self.decls.fns.insert(sym, sig);
    }

    /// AST type → `Ty` for declarations; diagnostics go to `self.diags`.
    pub fn lower(&mut self, m: ModuleId, t: &Type) -> Ty {
        let mut diags = std::mem::take(&mut self.diags);
        let ty = self.lower_with(m, t, &mut diags);
        self.diags = diags;
        ty
    }

    /// AST type → `Ty`, using the resolver's `uses` table.
    pub fn lower_with(&self, m: ModuleId, t: &Type, diags: &mut Vec<Diagnostic>) -> Ty {
        let uses = &self.r.tables[m.0 as usize].uses;
        match &t.kind {
            TypeKind::Path(p) => match uses.get(&p.id).map(|u| u.res.clone()) {
                Some(Res::Symbol(s)) => {
                    let sym = self.r.symbol(s);
                    match &sym.kind {
                        SymbolKind::Primitive => primitive(&sym.name),
                        SymbolKind::PreludeType | SymbolKind::Struct | SymbolKind::Enum => {
                            let arity = self.r.type_arity.get(&s).copied().unwrap_or(0);
                            let mut args: Vec<Ty> = p.args.iter().map(|a| self.lower_with(m, a, diags)).collect();
                            args.resize(arity, Ty::Error);
                            // The prelude `Error` is a placeholder until the stdlib defines it.
                            if matches!(sym.kind, SymbolKind::PreludeType) && sym.name == "Error" {
                                return Ty::Opaque;
                            }
                            Ty::Adt(s, args)
                        }
                        SymbolKind::GenericParam => Ty::Param(ParamId(s.0)),
                        SymbolKind::Interface => {
                            diags.push(
                                Diagnostic::error("E3032", "interface_as_type", format!("interface `{}` cannot be used as a type directly", sym.name))
                                    .primary(p.span, "")
                                    .help(format!("use `any {}` for dynamic dispatch, or a generic parameter `<T: {}>`", sym.name, sym.name)),
                            );
                            Ty::Error
                        }
                        _ => Ty::Error,
                    }
                }
                Some(Res::External { .. }) => Ty::Opaque,
                _ => Ty::Error,
            },
            TypeKind::Ref { mutable, inner } => Ty::Ref(*mutable, Box::new(self.lower_with(m, inner, diags))),
            TypeKind::Slice(inner) => Ty::Slice(Box::new(self.lower_with(m, inner, diags))),
            TypeKind::Array { len, elem } => {
                let elem = self.lower_with(m, elem, diags);
                match len.kind {
                    tarn_ast::ExprKind::Int(n) => Ty::Array(Box::new(elem), n),
                    _ => {
                        diags.push(
                            Diagnostic::error("E3033", "array_length", "array length must be an integer literal")
                                .primary(len.span, "")
                                .note("constant expressions are not supported in v0"),
                        );
                        Ty::Error
                    }
                }
            }
            TypeKind::Fn { params, ret } => Ty::Fn(
                params.iter().map(|p| self.lower_with(m, p, diags)).collect(),
                Box::new(ret.as_ref().map(|r| self.lower_with(m, r, diags)).unwrap_or(Ty::Void)),
            ),
            TypeKind::Any(_) => match uses.get(&t.id).map(|u| &u.res) {
                Some(Res::Symbol(i)) => Ty::Any(*i),
                _ => Ty::Error,
            },
            TypeKind::Error => Ty::Error,
        }
    }

    /// Owner and impl binders carry the bounds declared on the type
    /// (ADR 0021): inside `fn Point<T>.m`, `T: Copy` holds if `Point<T: Copy>`.
    fn inherit_bounds(&mut self, ty: SymbolId, binders: &[ParamId]) {
        let declared: Vec<ParamId> = match (self.decls.structs.get(&ty), self.decls.enums.get(&ty)) {
            (Some(s), _) => s.generics.clone(),
            (_, Some(e)) => e.generics.clone(),
            _ => return,
        };
        for (b, d) in binders.iter().zip(declared) {
            let inherited = self.decls.bounds.get(&d).cloned().unwrap_or_default();
            self.decls.bounds.entry(*b).or_default().extend(inherited);
        }
    }

    pub fn has_impl(&self, iface: SymbolId, target: SymbolId) -> bool {
        self.r.impls.iter().any(|i| i.interface == Some(iface) && i.target == Some(target))
    }
}

pub fn primitive(name: &str) -> Ty {
    match name {
        "bool" => Ty::Bool,
        "i8" => Ty::Int(IntTy::I8),
        "i16" => Ty::Int(IntTy::I16),
        "i32" => Ty::Int(IntTy::I32),
        "i64" => Ty::Int(IntTy::I64),
        "isize" => Ty::Int(IntTy::Isize),
        "u8" => Ty::Int(IntTy::U8),
        "u16" => Ty::Int(IntTy::U16),
        "u32" => Ty::Int(IntTy::U32),
        "u64" => Ty::Int(IntTy::U64),
        "usize" => Ty::Int(IntTy::Usize),
        "f32" => Ty::Float(FloatTy::F32),
        "f64" => Ty::Float(FloatTy::F64),
        "string" => Ty::Str,
        "void" => Ty::Void,
        "never" => Ty::Never,
        _ => Ty::Error,
    }
}
