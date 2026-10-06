//! Type checking for Tarn (phase 4).
//!
//! Input: parsed modules and the resolver's output. Output: side tables keyed
//! by `NodeId` / `SymbolId` (expression types, local types, method targets,
//! pattern variants) — the AST stays untouched. Rules: `docs/types.md`.

mod check;
mod env;
mod ty;

pub use env::{Env, FnSig};
pub use ty::{FloatTy, IntTy, ParamId, Ty};

use std::collections::HashMap;
use tarn_ast::{ItemKind, NodeId};
use tarn_diagnostics::{Diagnostic, SourceMap};
use tarn_resolve::{ModuleId, ModuleInput, Resolved, SymbolId, SymbolKind};

#[derive(Clone, Debug, PartialEq)]
pub enum MethodTarget {
    Symbol(SymbolId),
    /// Provisional built-in method (`len`, `sqrt`, ...).
    Intrinsic(String),
    /// Method of an opaque std value; unchecked.
    Opaque,
}

#[derive(Default, Debug)]
pub struct TypeTables {
    pub expr_types: HashMap<NodeId, Ty>,
    /// Method-call expression → method.
    pub method_calls: HashMap<NodeId, MethodTarget>,
    /// Pattern node → enum variant it matches (resolves `ScrutineeVariant`).
    pub pattern_variants: HashMap<NodeId, SymbolId>,
}

pub struct Typed {
    pub tables: Vec<TypeTables>,
    /// Types of locals, parameters and bindings.
    pub locals: HashMap<SymbolId, Ty>,
    pub param_names: HashMap<ParamId, String>,
}

/// Type-check a resolved program.
pub fn check(inputs: &[ModuleInput], r: &Resolved) -> (Typed, Vec<Diagnostic>) {
    let mut env = Env::new(inputs, r);
    let mut diags = std::mem::take(&mut env.diags);
    validate_copy(&env, &mut diags);
    validate_impls(&env, &mut diags);
    let mut tables: Vec<TypeTables> = (0..inputs.len()).map(|_| TypeTables::default()).collect();
    let mut locals = HashMap::new();
    for (mi, input) in inputs.iter().enumerate() {
        let m = ModuleId(mi as u32);
        let mut fns = Vec::new();
        for item in &input.ast.items {
            match &item.kind {
                ItemKind::Fn(f) => fns.push(f),
                ItemKind::Impl(i) => fns.extend(i.methods.iter()),
                _ => {}
            }
        }
        for f in fns {
            let Some(sym) = r.tables[mi].defs.get(&f.id) else { continue };
            let Some(sig) = env.fns.get(sym).cloned() else { continue };
            let mut cx = check::FnCx::new(&env, m);
            cx.function(f, &sig);
            cx.finish();
            diags.append(&mut cx.diags);
            locals.extend(cx.locals);
            let t = &mut tables[mi];
            t.expr_types.extend(cx.tables.expr_types);
            t.method_calls.extend(cx.tables.method_calls);
            t.pattern_variants.extend(cx.tables.pattern_variants);
        }
    }
    (Typed { tables, locals, param_names: env.param_names.clone() }, diags)
}

/// `copy struct` / `copy enum` may only contain copy types (E3024).
fn validate_copy(env: &Env, diags: &mut Vec<Diagnostic>) {
    let cx = check::FnCx::new(env, ModuleId(0));
    let mut check = |sym: SymbolId, tys: Vec<&Ty>| {
        for t in tys {
            if !cx.is_copy(t) {
                let s = env.r.symbol(sym);
                let shown = display(t, env);
                let mut d = Diagnostic::error("E3024", "copy_with_non_copy", format!("`copy` type `{}` contains `{shown}`, which is not copy", s.name))
                    .primary(s.span.unwrap(), "")
                    .help("remove `copy`, or use only copy types (numbers, bool, `&T`, copy structs and enums)");
                if matches!(t, Ty::Param(_)) {
                    d = d.note("type parameters are never copy in v0");
                }
                diags.push(d);
                return;
            }
        }
    };
    let mut syms: Vec<_> = env.structs.keys().chain(env.enums.keys()).copied().collect();
    syms.sort();
    for s in syms {
        if let Some(d) = env.structs.get(&s).filter(|d| d.is_copy) {
            check(s, d.fields.iter().map(|f| &f.ty).collect());
        }
        if let Some(d) = env.enums.get(&s).filter(|d| d.is_copy) {
            check(s, d.variants.iter().flat_map(|v| v.fields.iter()).collect());
        }
    }
}

/// Impl methods must match their interface declaration (E3023).
fn validate_impls(env: &Env, diags: &mut Vec<Diagnostic>) {
    for imp in &env.r.impls {
        let Some(iface) = imp.interface else { continue };
        for &m in &imp.methods {
            let name = &env.r.symbol(m).name;
            let Some(decl) = env.r.member(iface, name) else { continue };
            let (Some(have), Some(want)) = (env.fns.get(&m), env.fns.get(&decl)) else { continue };
            let mut problems = Vec::new();
            if have.receiver != want.receiver {
                problems.push(format!("receiver is `{}`, the interface declares `{}`", recv_name(have.receiver), recv_name(want.receiver)));
            }
            if have.params.len() != want.params.len() {
                problems.push(format!("{} parameters, the interface declares {}", have.params.len(), want.params.len()));
            } else {
                for (i, (a, b)) in have.params.iter().zip(&want.params).enumerate() {
                    if a != b {
                        problems.push(format!("parameter {} is `{}`, the interface declares `{}`", i + 1, display(a, env), display(b, env)));
                    }
                }
            }
            if have.ret != want.ret {
                problems.push(format!("returns `{}`, the interface declares `{}`", display(&have.ret, env), display(&want.ret, env)));
            }
            if !problems.is_empty() {
                let iname = &env.r.symbol(iface).name;
                let mut d = Diagnostic::error("E3023", "impl_signature_mismatch", format!("method `{name}` does not match its declaration in `{iname}`"))
                    .primary(have.span, "")
                    .secondary(want.span, "declared here");
                for p in problems {
                    d = d.note(p);
                }
                diags.push(d);
            }
        }
    }
}

fn recv_name(r: Option<tarn_ast::ReceiverKind>) -> &'static str {
    match r {
        None => "none",
        Some(tarn_ast::ReceiverKind::Value) => "self",
        Some(tarn_ast::ReceiverKind::Ref) => "&self",
        Some(tarn_ast::ReceiverKind::RefMut) => "&mut self",
    }
}

/// `a.b[i]` as written, for diagnostics; `None` for other expressions.
pub(crate) fn place_text(e: &tarn_ast::Expr) -> Option<String> {
    use tarn_ast::ExprKind;
    match &e.kind {
        ExprKind::Ident(n) => Some(n.clone()),
        ExprKind::Field { base, name } => Some(format!("{}.{}", place_text(base)?, name.name)),
        ExprKind::Index { base, .. } => Some(format!("{}[…]", place_text(base)?)),
        ExprKind::Paren(inner) => place_text(inner),
        _ => None,
    }
}

pub(crate) fn binding_kind(k: &SymbolKind) -> &'static str {
    match k {
        SymbolKind::Param => "parameter",
        SymbolKind::SelfParam => "`self` parameter",
        SymbolKind::PatternBinding => "pattern binding",
        SymbolKind::LoopBinding => "loop variable",
        SymbolKind::ClosureParam => "closure parameter",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Interface => "interface",
        SymbolKind::Primitive => "type",
        SymbolKind::PreludeType => "type",
        SymbolKind::GenericParam => "type parameter",
        SymbolKind::Builtin => "builtin",
        _ => "item",
    }
}

/// Source-like rendering of a type.
pub fn display(t: &Ty, env: &Env) -> String {
    show(t, env.r, &env.param_names, &|_| "_")
}

/// Like `display`, naming unresolved literal variables `{integer}` / `{float}`.
pub(crate) fn display_vars(t: &Ty, env: &Env, infer: &ty::Infer) -> String {
    show(t, env.r, &env.param_names, &|v| match infer.kind(v) {
        ty::VarKind::Int => "{integer}",
        ty::VarKind::Float => "{float}",
        ty::VarKind::General => "_",
    })
}

fn show(t: &Ty, r: &Resolved, names: &HashMap<ParamId, String>, var: &dyn Fn(u32) -> &'static str) -> String {
    let show = |t: &Ty| show(t, r, names, var);
    let list = |ts: &[Ty]| ts.iter().map(show).collect::<Vec<_>>().join(", ");
    match t {
        Ty::Bool => "bool".into(),
        Ty::Int(i) => i.name().into(),
        Ty::Float(FloatTy::F32) => "f32".into(),
        Ty::Float(FloatTy::F64) => "f64".into(),
        Ty::Str => "string".into(),
        Ty::Void => "void".into(),
        Ty::Never => "never".into(),
        Ty::Adt(s, args) if args.is_empty() => r.symbol(*s).name.clone(),
        Ty::Adt(s, args) => format!("{}<{}>", r.symbol(*s).name, list(args)),
        Ty::Ref(m, x) => format!("&{}{}", if *m { "mut " } else { "" }, show(x)),
        Ty::Array(x, n) => format!("[{n}]{}", show(x)),
        Ty::Slice(x) => format!("[]{}", show(x)),
        Ty::Fn(ps, ret) => {
            let ret = match &**ret {
                Ty::Void => String::new(),
                x => format!(" {}", show(x)),
            };
            format!("fn({}){ret}", list(ps))
        }
        Ty::Param(p) => names.get(p).cloned().unwrap_or_else(|| "?".into()),
        Ty::Any(i) => format!("any {}", r.symbol(*i).name),
        Ty::Var(v) => var(*v).into(),
        Ty::Opaque => "<std>".into(),
        Ty::Error => "<error>".into(),
    }
}

/// `L:C name: type` for every local, parameter and binding (tests, tools).
pub fn dump_types(typed: &Typed, r: &Resolved, sources: &SourceMap) -> String {
    let mut out = String::new();
    for (mi, info) in r.modules.iter().enumerate() {
        out.push_str(&format!("== {}\n", info.name));
        let mut rows: Vec<(u32, String)> = typed
            .locals
            .iter()
            .filter(|(s, _)| r.symbol(**s).module == Some(ModuleId(mi as u32)))
            .filter_map(|(s, t)| {
                let sym = r.symbol(*s);
                let span = sym.span?;
                let lc = sources.line_col(span);
                Some((span.start, format!("{}:{} {}: {}", lc.line, lc.column, sym.name, show(t, r, &typed.param_names, &|_| "_"))))
            })
            .collect();
        rows.sort();
        for (_, row) in rows {
            out.push_str(&row);
            out.push('\n');
        }
    }
    out
}
