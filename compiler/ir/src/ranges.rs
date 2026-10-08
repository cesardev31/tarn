//! Proven integer arithmetic (Phase 29, ADR 0057).
//!
//! A conservative interval analysis over the typed IR. Every integer `+`, `-`
//! and `*` stays checked unless the interval of its result provably fits its
//! type in every execution; those are rewritten to `AddProven`, `SubProven` or
//! `MulProven`, so the decision is explicit in the IR and the backend only
//! omits a check the IR says cannot fail. Anything not understood is the full
//! range of its type, which never proves anything.
//!
//! Rules that keep it sound:
//! - only locals with no projections and whose address is never taken
//!   (`&x`, `&mut x`, slice references, projected writes) are tracked;
//! - a branch on `t = lt/le/gt/ge(a, b)` refines `a` on each edge, only when
//!   `t` is computed in the same block and neither operand is reassigned
//!   between the comparison and the branch;
//! - loop heads widen to the type bounds after a few visits, so the fixpoint
//!   terminates and every value is covered.

use crate::*;
use std::collections::HashSet;
use tarn_types::{IntTy, Ty};

type Range = (i128, i128);

fn bounds(i: IntTy) -> Range {
    let bits = match i {
        IntTy::I8 | IntTy::U8 => 8,
        IntTy::I16 | IntTy::U16 => 16,
        IntTy::I32 | IntTy::U32 => 32,
        _ => 64,
    };
    if i.signed() { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) } else { (0, (1i128 << bits) - 1) }
}

fn int_ty(f: &Function, l: LocalId) -> Option<IntTy> {
    match f.local(l).ty {
        Ty::Int(i) => Some(i),
        _ => None,
    }
}

/// Rewrite provably non-overflowing integer arithmetic in every function.
pub fn prove_arithmetic(p: &mut Program) {
    if std::env::var_os("TARN_NO_RANGE_PROOFS").is_some() {
        return;
    }
    for f in &mut p.functions {
        prove_function(f);
    }
}

struct Analysis {
    tracked: Vec<Option<IntTy>>,
}

type State = Vec<Option<Range>>;

impl Analysis {
    fn top(&self, l: usize) -> Option<Range> {
        self.tracked[l].map(bounds)
    }
    fn operand(&self, s: &State, o: &Operand) -> Option<Range> {
        match o {
            Operand::Const(Const::Int(v, _)) => Some((*v, *v)),
            Operand::Copy(p) | Operand::Move(p) if p.proj.is_empty() && self.tracked[p.local.0 as usize].is_some() => s[p.local.0 as usize],
            _ => None,
        }
    }
    /// Interval of an arithmetic result and whether it fits `ty`.
    fn arithmetic(&self, s: &State, op: BinOp, a: &Operand, b: &Operand, ty: IntTy) -> (Option<Range>, bool) {
        let (Some((al, ah)), Some((bl, bh))) = (self.operand(s, a), self.operand(s, b)) else { return (None, false) };
        let (lo, hi) = match op {
            BinOp::Add | BinOp::AddProven => (al.checked_add(bl), ah.checked_add(bh)),
            BinOp::Sub | BinOp::SubProven => (al.checked_sub(bh), ah.checked_sub(bl)),
            BinOp::Mul | BinOp::MulProven => {
                let products = [al.checked_mul(bl), al.checked_mul(bh), ah.checked_mul(bl), ah.checked_mul(bh)];
                if products.iter().any(Option::is_none) {
                    return (None, false);
                }
                let values: Vec<i128> = products.into_iter().flatten().collect();
                (values.iter().min().copied(), values.iter().max().copied())
            }
            BinOp::Rem if !ty.signed() => match self.operand(s, b) {
                Some((d, _)) if d > 0 && bl == bh => return (Some((0, d - 1)), false),
                _ => return (None, false),
            },
            _ => return (None, false),
        };
        let (Some(lo), Some(hi)) = (lo, hi) else { return (None, false) };
        let (tl, th) = bounds(ty);
        if lo >= tl && hi <= th { (Some((lo, hi)), true) } else { (None, false) }
    }
    fn assign(&self, s: &mut State, place: &Place, rv: &Rvalue) {
        let l = place.local.0 as usize;
        if !place.proj.is_empty() || self.tracked[l].is_none() {
            return;
        }
        let ty = self.tracked[l].unwrap();
        let value = match rv {
            Rvalue::Use(o) => self.operand(s, o),
            Rvalue::Binary(op, a, b) => self.arithmetic(s, *op, a, b, ty).0,
            _ => None,
        };
        s[l] = value.or_else(|| self.top(l));
    }
}

fn refine(s: &mut State, l: usize, lo: Option<i128>, hi: Option<i128>) {
    if let Some((cl, ch)) = s[l] {
        let (nl, nh) = (lo.map_or(cl, |v| cl.max(v)), hi.map_or(ch, |v| ch.min(v)));
        s[l] = Some((nl, nh));
    }
}

fn prove_function(f: &mut Function) {
    // Extern and intrinsic declarations have no body.
    if f.blocks.is_empty() {
        return;
    }
    let n = f.locals.len();
    // Locals whose address is taken or written through a projection can
    // change without an assignment this analysis sees.
    let mut escaped: HashSet<usize> = HashSet::new();
    for b in &f.blocks {
        for st in &b.stmts {
            if let StatementKind::Assign(p, rv) = &st.kind {
                if !p.proj.is_empty() {
                    escaped.insert(p.local.0 as usize);
                }
                match rv {
                    Rvalue::Ref(_, q) => {
                        escaped.insert(q.local.0 as usize);
                    }
                    Rvalue::SliceRef { base, .. } => {
                        escaped.insert(base.local.0 as usize);
                    }
                    _ => {}
                }
            }
        }
    }
    let analysis = Analysis {
        tracked: (0..n).map(|l| if escaped.contains(&l) || l == 0 { None } else { int_ty(f, LocalId(l as u32)) }).collect(),
    };
    // Parameters and every other local start unconstrained.
    let entry_state: State = (0..n).map(|l| analysis.top(l)).collect();
    let blocks = f.blocks.len();
    let heads = loop_heads(f);
    let mut input: Vec<Option<State>> = vec![None; blocks];
    let mut visits = vec![0u32; blocks];
    input[0] = Some(entry_state);
    let mut work = vec![0usize];
    while let Some(b) = work.pop() {
        let Some(mut s) = input[b].clone() else { continue };
        let block = &f.blocks[b];
        for st in &block.stmts {
            if let StatementKind::Assign(p, rv) = &st.kind {
                analysis.assign(&mut s, p, rv);
            }
        }
        let mut edges: Vec<(usize, State)> = Vec::new();
        match &block.term {
            Terminator::Switch { discr, cases, otherwise } => {
                let condition = comparison(block, discr, &analysis);
                for (value, target) in cases {
                    let mut t = s.clone();
                    if let Some((op, a, bnd)) = &condition
                        && *value == 0
                    {
                        apply(&mut t, &analysis, negate(*op), *a, bnd);
                    }
                    edges.push((target.0 as usize, t));
                }
                let mut t = s.clone();
                if let Some((op, a, bnd)) = &condition
                    && cases.iter().all(|(v, _)| *v == 0)
                {
                    apply(&mut t, &analysis, *op, *a, bnd);
                }
                edges.push((otherwise.0 as usize, t));
            }
            Terminator::Call { dest, next, .. } => {
                let mut t = s.clone();
                let l = dest.local.0 as usize;
                if dest.proj.is_empty() && analysis.tracked[l].is_some() {
                    t[l] = analysis.top(l);
                }
                if let Some(nb) = next {
                    edges.push((nb.0 as usize, t));
                }
            }
            other => {
                for nb in other.successors() {
                    edges.push((nb.0 as usize, s.clone()));
                }
            }
        }
        for (target, state) in edges {
            let merged = match &input[target] {
                None => state,
                Some(old) => {
                    visits[target] += 1;
                    // Widen only where a back edge enters (loop heads), so
                    // branch refinements inside a loop body are kept.
                    let widen = heads[target] && visits[target] > 3;
                    (0..n)
                        .map(|l| match (old[l], state[l]) {
                            (Some((ol, oh)), Some((nl, nh))) => {
                                let (tl, th) = analysis.top(l).unwrap_or((ol.min(nl), oh.max(nh)));
                                let lo = if nl < ol { if widen { tl } else { nl } } else { ol };
                                let hi = if nh > oh { if widen { th } else { nh } } else { oh };
                                Some((lo, hi))
                            }
                            _ => analysis.top(l),
                        })
                        .collect()
                }
            };
            if input[target].as_ref() != Some(&merged) {
                input[target] = Some(merged);
                work.push(target);
            }
        }
    }
    // Rewrite with the fixpoint states.
    for (b, state) in input.into_iter().enumerate() {
        let Some(mut s) = state else { continue };
        for st in &mut f.blocks[b].stmts {
            if let StatementKind::Assign(p, rv) = &mut st.kind {
                if let Rvalue::Binary(op, a, bop) = rv
                    && matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul)
                    && p.proj.is_empty()
                    && let Some(ty) = int_ty_of_place(&analysis, p)
                    && analysis.arithmetic(&s, *op, a, bop, ty).1
                {
                    *op = match op {
                        BinOp::Add => BinOp::AddProven,
                        BinOp::Sub => BinOp::SubProven,
                        _ => BinOp::MulProven,
                    };
                }
                let (p, rv) = (p.clone(), rv.clone());
                analysis.assign(&mut s, &p, &rv);
            }
        }
    }
}

/// Targets of back edges in a depth-first walk from the entry.
fn loop_heads(f: &Function) -> Vec<bool> {
    let n = f.blocks.len();
    let (mut heads, mut state) = (vec![false; n], vec![0u8; n]);
    let mut stack = vec![(0usize, 0usize)];
    state[0] = 1;
    while let Some((b, i)) = stack.pop() {
        let next = f.blocks[b].term.successors();
        if i < next.len() {
            stack.push((b, i + 1));
            let s = next[i].0 as usize;
            match state[s] {
                0 => {
                    state[s] = 1;
                    stack.push((s, 0));
                }
                1 => heads[s] = true,
                _ => {}
            }
        } else {
            state[b] = 2;
        }
    }
    // A block not proven acyclic is treated as a head (never unbounded).
    for (b, s) in state.iter().enumerate() {
        if *s == 0 {
            heads[b] = true;
        }
    }
    heads
}

fn int_ty_of_place(a: &Analysis, p: &Place) -> Option<IntTy> {
    a.tracked.get(p.local.0 as usize).copied().flatten()
}

/// `t = cmp(copy a, b)` as the last write to `t` in this block, with `a`
/// tracked and neither operand reassigned after the comparison.
fn comparison(block: &BasicBlock, discr: &Operand, a: &Analysis) -> Option<(BinOp, usize, Operand)> {
    let (Operand::Copy(t) | Operand::Move(t)) = discr else { return None };
    if !t.proj.is_empty() {
        return None;
    }
    let at = block.stmts.iter().rposition(|st| matches!(&st.kind, StatementKind::Assign(p, _) if p.local == t.local))?;
    let StatementKind::Assign(_, Rvalue::Binary(op, lhs, rhs)) = &block.stmts[at].kind else { return None };
    if !matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
        return None;
    }
    let (Operand::Copy(l) | Operand::Move(l)) = lhs else { return None };
    if !l.proj.is_empty() || a.tracked[l.local.0 as usize].is_none() {
        return None;
    }
    let written = |local: LocalId| block.stmts[at + 1..].iter().any(|st| matches!(&st.kind, StatementKind::Assign(p, _) if p.local == local));
    if written(l.local) || matches!(rhs, Operand::Copy(r) | Operand::Move(r) if written(r.local)) {
        return None;
    }
    Some((*op, l.local.0 as usize, rhs.clone()))
}

fn negate(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Ge,
        BinOp::Le => BinOp::Gt,
        BinOp::Gt => BinOp::Le,
        _ => BinOp::Lt,
    }
}

/// Refine `a` with `a <op> bound` holding.
fn apply(s: &mut State, an: &Analysis, op: BinOp, a: usize, bound: &Operand) {
    let Some((bl, bh)) = an.operand(s, bound) else { return };
    match op {
        BinOp::Lt => refine(s, a, None, Some(bh - 1)),
        BinOp::Le => refine(s, a, None, Some(bh)),
        BinOp::Gt => refine(s, a, Some(bl + 1), None),
        _ => refine(s, a, Some(bl), None),
    }
    // An empty interval means the edge is unreachable; keep it conservative.
    if let Some((lo, hi)) = s[a]
        && lo > hi
    {
        s[a] = an.top(a);
    }
}
