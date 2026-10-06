//! Exhaustiveness and reachability of `match` (ADR 0019), using the
//! usefulness algorithm over pattern matrices (Maranget, "Warnings for
//! pattern matching", 2007).
//!
//! A pattern vector `q` is *useful* with respect to a matrix `P` if some value
//! matches `q` and no row of `P`. Then:
//! - the match is exhaustive iff the wildcard row is not useful w.r.t. the
//!   unguarded arms; the algorithm also produces a *witness* (a missing value);
//! - arm `i` is unreachable iff it is not useful w.r.t. the unguarded arms
//!   before it.

use crate::check::{FnCx, subst};
use crate::ty::{ParamId, Ty};
use std::collections::HashMap;
use tarn_ast::{ExprKind, Pattern, PatternKind};
use tarn_resolve::SymbolId;

#[derive(Clone, Debug, PartialEq)]
pub enum Ctor {
    Variant(SymbolId),
    Bool(bool),
    /// The single constructor of a struct.
    Struct(SymbolId),
    /// A literal or range of an infinite domain (integers, strings, floats),
    /// compared by source text. Never forms a complete set.
    Lit(String),
}

#[derive(Clone, Debug)]
pub enum Pat {
    Wild,
    Ctor(Ctor, Vec<Pat>),
}

/// Constructors of a type, when the set is finite and known.
enum Ctors {
    Finite(Vec<(Ctor, Vec<Ty>)>),
    /// Integers, strings, unknown types: only a wildcard covers everything.
    Infinite,
}

fn peel(t: &Ty) -> Ty {
    let mut t = t.clone();
    while let Ty::Ref(_, inner) = t {
        t = *inner;
    }
    t
}

impl FnCx<'_, '_> {
    fn ctors(&self, t: &Ty) -> Ctors {
        match peel(&self.infer.zonk(t)) {
            Ty::Bool => Ctors::Finite(vec![(Ctor::Bool(true), vec![]), (Ctor::Bool(false), vec![])]),
            Ty::Adt(s, args) => {
                if let Some(def) = self.env.decls.enums.get(&s) {
                    let map: HashMap<ParamId, Ty> = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                    Ctors::Finite(def.variants.iter().map(|v| (Ctor::Variant(v.sym), v.fields.iter().map(|f| subst(f, &map)).collect())).collect())
                } else if let Some(def) = self.env.decls.structs.get(&s) {
                    let map: HashMap<ParamId, Ty> = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                    Ctors::Finite(vec![(Ctor::Struct(s), def.fields.iter().map(|f| subst(&f.ty, &map)).collect())])
                } else {
                    Ctors::Infinite
                }
            }
            _ => Ctors::Infinite,
        }
    }

    fn field_tys(&self, t: &Ty, c: &Ctor) -> Vec<Ty> {
        match self.ctors(t) {
            Ctors::Finite(cs) => cs.into_iter().find(|(x, _)| x == c).map(|(_, f)| f).unwrap_or_default(),
            Ctors::Infinite => Vec::new(),
        }
    }

    /// AST pattern → matrix pattern. Binding modes do not matter here.
    pub fn lower_pat(&self, p: &Pattern, t: &Ty) -> Pat {
        let z = peel(&self.infer.zonk(t));
        match &p.kind {
            PatternKind::Wildcard | PatternKind::Error => Pat::Wild,
            PatternKind::Ident(_) => match self.tables.pattern_variants.get(&p.id) {
                // A unit-form pattern for a variant with fields (E3020): treat
                // its fields as wildcards.
                Some(v) => {
                    let n = self.field_tys(&z, &Ctor::Variant(*v)).len();
                    Pat::Ctor(Ctor::Variant(*v), vec![Pat::Wild; n])
                }
                None => Pat::Wild,
            },
            PatternKind::Literal(e) => match &e.kind {
                ExprKind::Bool(b) => Pat::Ctor(Ctor::Bool(*b), Vec::new()),
                _ => Pat::Ctor(Ctor::Lit(literal_text(e)), Vec::new()),
            },
            PatternKind::Range { start, end, inclusive } => {
                Pat::Ctor(Ctor::Lit(format!("{}{}{}", literal_text(start), if *inclusive { "..=" } else { ".." }, literal_text(end))), Vec::new())
            }
            PatternKind::Variant { args, .. } => match self.tables.pattern_variants.get(&p.id) {
                Some(v) => {
                    let c = Ctor::Variant(*v);
                    let ftys = self.field_tys(&z, &c);
                    // Arity errors were reported (E3020); keep the matrix
                    // rectangular by padding/truncating to the real arity.
                    let sub = (0..ftys.len()).map(|i| args.get(i).map_or(Pat::Wild, |a| self.lower_pat(a, &ftys[i]))).collect();
                    Pat::Ctor(c, sub)
                }
                None => Pat::Wild,
            },
            PatternKind::Struct { fields, .. } => {
                let Ty::Adt(s, _) = &z else { return Pat::Wild };
                let Some(def) = self.env.decls.structs.get(s) else { return Pat::Wild };
                let ftys = self.field_tys(&z, &Ctor::Struct(*s));
                let sub = def
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| match fields.iter().find(|fp| fp.name.name == f.name) {
                        Some(fp) => match &fp.pattern {
                            Some(sp) => self.lower_pat(sp, &ftys[i]),
                            None => Pat::Wild,
                        },
                        None => Pat::Wild,
                    })
                    .collect();
                Pat::Ctor(Ctor::Struct(*s), sub)
            }
        }
    }

    /// Rows whose head is `c` (or a wildcard), with the head replaced by its fields.
    fn specialize(rows: &[Vec<Pat>], c: &Ctor, arity: usize) -> Vec<Vec<Pat>> {
        rows.iter()
            .filter_map(|row| match &row[0] {
                Pat::Wild => Some(std::iter::repeat_n(Pat::Wild, arity).chain(row[1..].iter().cloned()).collect()),
                Pat::Ctor(rc, args) if rc == c => Some(args.iter().cloned().chain(row[1..].iter().cloned()).collect()),
                Pat::Ctor(..) => None,
            })
            .collect()
    }

    /// Rows whose head is a wildcard, without the head.
    fn default_rows(rows: &[Vec<Pat>]) -> Vec<Vec<Pat>> {
        rows.iter().filter(|r| matches!(r[0], Pat::Wild)).map(|r| r[1..].to_vec()).collect()
    }

    /// Is `q` useful w.r.t. `rows`? (`tys` = column types)
    pub fn useful(&self, rows: &[Vec<Pat>], q: &[Pat], tys: &[Ty]) -> bool {
        if q.is_empty() {
            return rows.is_empty();
        }
        match &q[0] {
            Pat::Ctor(c, args) => {
                let ftys = self.field_tys(&tys[0], c);
                let ftys: Vec<Ty> = if ftys.len() == args.len() { ftys } else { vec![Ty::Error; args.len()] };
                let rows = Self::specialize(rows, c, args.len());
                let q: Vec<Pat> = args.iter().cloned().chain(q[1..].iter().cloned()).collect();
                let tys: Vec<Ty> = ftys.into_iter().chain(tys[1..].iter().cloned()).collect();
                self.useful(&rows, &q, &tys)
            }
            Pat::Wild => match self.complete_ctors(rows, &tys[0]) {
                Some(all) => all.into_iter().any(|(c, ftys)| {
                    let rows = Self::specialize(rows, &c, ftys.len());
                    let q: Vec<Pat> = std::iter::repeat_n(Pat::Wild, ftys.len()).chain(q[1..].iter().cloned()).collect();
                    let tys: Vec<Ty> = ftys.into_iter().chain(tys[1..].iter().cloned()).collect();
                    self.useful(&rows, &q, &tys)
                }),
                None => self.useful(&Self::default_rows(rows), &q[1..], &tys[1..]),
            },
        }
    }

    /// The constructors of the column's type, if the heads of `rows` use all
    /// of them (a complete signature).
    fn complete_ctors(&self, rows: &[Vec<Pat>], t: &Ty) -> Option<Vec<(Ctor, Vec<Ty>)>> {
        let Ctors::Finite(all) = self.ctors(t) else { return None };
        let used = |c: &Ctor| rows.iter().any(|r| matches!(&r[0], Pat::Ctor(rc, _) if rc == c));
        all.iter().all(|(c, _)| used(c)).then_some(all)
    }

    /// A value (as patterns) not matched by any row, if one exists.
    pub fn witness(&self, rows: &[Vec<Pat>], tys: &[Ty]) -> Option<Vec<Pat>> {
        if tys.is_empty() {
            return rows.is_empty().then(Vec::new);
        }
        if let Some(all) = self.complete_ctors(rows, &tys[0]) {
            for (c, ftys) in all {
                let n = ftys.len();
                let sub = Self::specialize(rows, &c, n);
                let tys2: Vec<Ty> = ftys.into_iter().chain(tys[1..].iter().cloned()).collect();
                if let Some(w) = self.witness(&sub, &tys2) {
                    let (args, rest) = w.split_at(n);
                    return Some(std::iter::once(Pat::Ctor(c, args.to_vec())).chain(rest.iter().cloned()).collect());
                }
            }
            return None;
        }
        let rest = self.witness(&Self::default_rows(rows), &tys[1..])?;
        // Name a missing constructor when the type has a finite set.
        let head = match self.ctors(&tys[0]) {
            Ctors::Finite(all) => all
                .into_iter()
                .find(|(c, _)| !rows.iter().any(|r| matches!(&r[0], Pat::Ctor(rc, _) if rc == c)))
                .map(|(c, f)| Pat::Ctor(c, vec![Pat::Wild; f.len()]))
                .unwrap_or(Pat::Wild),
            Ctors::Infinite => Pat::Wild,
        };
        Some(std::iter::once(head).chain(rest).collect())
    }

    pub fn show_pat(&self, p: &Pat) -> String {
        match p {
            Pat::Wild => "_".into(),
            Pat::Ctor(Ctor::Bool(b), _) => b.to_string(),
            Pat::Ctor(Ctor::Lit(s), _) => s.clone(),
            Pat::Ctor(Ctor::Struct(s), _) => format!("{}{{..}}", self.env.r.symbol(*s).name),
            Pat::Ctor(Ctor::Variant(v), args) => {
                let name = self.env.r.symbol(*v).name.clone();
                if args.is_empty() { name } else { format!("{name}({})", args.iter().map(|a| self.show_pat(a)).collect::<Vec<_>>().join(", ")) }
            }
        }
    }
}

fn literal_text(e: &tarn_ast::Expr) -> String {
    match &e.kind {
        ExprKind::Int(v) => v.to_string(),
        ExprKind::Float(s) => s.clone(),
        ExprKind::Str(s) => format!("{s:?}"),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Unary { operand, .. } => format!("-{}", literal_text(operand)),
        _ => "?".into(),
    }
}
