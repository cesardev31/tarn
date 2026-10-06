//! Borrow checking (phase 6B, ADR 0025) on the typed IR.
//!
//! Three dataflow analyses per function:
//!
//! 1. **Liveness** (backward): at each point, which locals may be used later.
//! 2. **Loan flow** (forward, union at joins): which loans each local may
//!    hold. A loan is created by `Ref(kind, place)`; it flows through copies,
//!    moves, coercions, aggregates (closures, `Option<&T>`) and, across calls,
//!    through the callee's *provenance* (which arguments the result may
//!    borrow from).
//! 3. **Conflicts**: a loan is *active* at a point if a local that may hold
//!    it is live there — this is what makes lifetimes non-lexical: a loan ends
//!    with the last use of the references that carry it. Every access to a
//!    place is checked against the active loans of overlapping places.
//!
//! Returned references are checked against the function's own frame, and
//! provenance summaries are inferred bottom-up (fixpoint for recursion).

use crate::util::{BitSet, may_hold_refs, place_name};
use std::collections::{HashMap, HashSet, VecDeque};
use tarn_diagnostics::{Diagnostic, Span};
use tarn_ir::{BlockId, Callee, Function, FunctionId, LocalId, Operand, Place, Program, Proj, RETURN, Rvalue, StatementKind, Terminator};
use tarn_resolve::{Resolved, SymbolKind};
use tarn_types::{FnSig, Ty, Typed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoanKind {
    Shared,
    Mutable,
}

/// A borrow: *what* is borrowed (`place`), *how* (`kind`), *where* it was
/// created, and *who* initially holds the reference. Where it is still active
/// is derived: wherever a live local may hold it.
#[derive(Clone, Debug)]
pub struct Loan {
    pub kind: LoanKind,
    pub place: Place,
    pub block: BlockId,
    pub stmt: usize,
    pub span: Span,
    pub holder: LocalId,
    /// Placeholder for "whatever the caller lent through parameter `i`".
    /// Never conflicts; used for provenance and returns.
    pub param: Option<usize>,
}

/// Argument positions (receiver first) whose loans a function's result may carry.
pub type Provenance = Vec<usize>;

#[derive(Debug, Default)]
pub struct FnBorrows {
    pub loans: Vec<Loan>,
    pub provenance: Provenance,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    Read,
    BorrowShared,
    BorrowMut,
    Write,
    Move,
    Drop,
}

/// Place overlap (ADR 0025): same root local, and the projections never
/// diverge at two *different* fields or variants. `Deref` and `Index` never
/// prove disjointness (conservative: no reasoning about runtime indices).
pub fn overlap(a: &Place, b: &Place) -> bool {
    if a.local != b.local {
        return false;
    }
    for (x, y) in a.proj.iter().zip(&b.proj) {
        match (x, y) {
            (Proj::Field(i), Proj::Field(j)) | (Proj::Downcast(i), Proj::Downcast(j)) if i != j => {
                return false;
            }
            _ => {}
        }
    }
    true
}

// ---------------------------------------------------------------- provenance of signatures

/// ADR 0010 elision for declarations without a body: receiver → {self};
/// exactly one reference parameter → it; otherwise none can be inferred.
pub(crate) enum Elided {
    None,
    Prov(Provenance),
    Ambiguous(usize),
    NoSource,
}

pub(crate) fn elide(sig: &FnSig) -> Elided {
    match &sig.contract.result {
        tarn_types::ResultContract::Copy | tarn_types::ResultContract::Owned | tarn_types::ResultContract::InferredBorrow => Elided::None,
        tarn_types::ResultContract::Borrowed(sources) => Elided::Prov(sources.clone()),
        tarn_types::ResultContract::Ambiguous(n) => Elided::Ambiguous(*n),
        tarn_types::ResultContract::NoSource => Elided::NoSource,
    }
}

// ---------------------------------------------------------------- per-function analysis

struct Fx<'a> {
    f: &'a Function,
    t: &'a Typed,
    prov: &'a HashMap<FunctionId, Provenance>,
    loans: Vec<Loan>,
    /// Loan created at (block, statement).
    loan_at: HashMap<(usize, usize), usize>,
    nl: usize,
    /// `live[b][i]`: locals live before statement `i` of block `b`
    /// (`i == stmts.len()`: before the terminator); `live_out[b]` after it.
    live: Vec<Vec<BitSet>>,
    live_out: Vec<BitSet>,
    diags: Vec<Diagnostic>,
    reported: HashSet<usize>,
}

type Holds = Vec<BitSet>;

fn operand_place(o: &Operand) -> Option<&Place> {
    match o {
        Operand::Copy(p) | Operand::Move(p) => Some(p),
        Operand::Const(_) => None,
    }
}

impl<'a> Fx<'a> {
    fn new(f: &'a Function, t: &'a Typed, prov: &'a HashMap<FunctionId, Provenance>) -> Self {
        let mut fx =
            Fx { f, t, prov, loans: Vec::new(), loan_at: HashMap::new(), nl: f.locals.len(), live: Vec::new(), live_out: Vec::new(), diags: Vec::new(), reported: HashSet::new() };
        // Placeholder loans: one per reference-holding parameter.
        for (i, l) in f.params().enumerate() {
            if t.decls.may_contain_references(&f.local(l).ty) || t.decls.contains_task(&f.local(l).ty) {
                let kind = if matches!(f.local(l).ty, Ty::Ref(true, _)) { LoanKind::Mutable } else { LoanKind::Shared };
                fx.loans.push(Loan { kind, place: Place::local(l).project(Proj::Deref), block: BlockId(0), stmt: 0, span: f.local(l).span, holder: l, param: Some(i) });
            }
        }
        for (bi, b) in f.blocks.iter().enumerate() {
            for (si, s) in b.stmts.iter().enumerate() {
                if let StatementKind::Assign(dest, rv) = &s.kind {
                    let (kind, place) = match rv {
                        Rvalue::Ref(m, p) => (*m, p),
                        Rvalue::SliceRef { mutable, base, .. } => (*mutable, base),
                        Rvalue::Aggregate(tarn_ir::Aggregate::Closure(_, Some(storage)), _) => (false, storage),
                        _ => continue,
                    };
                    let kind = if kind { LoanKind::Mutable } else { LoanKind::Shared };
                    fx.loan_at.insert((bi, si), fx.loans.len());
                    fx.loans.push(Loan { kind, place: place.clone(), block: BlockId(bi as u32), stmt: si, span: s.span, holder: dest.local, param: None });
                }
            }
        }
        fx.liveness();
        fx
    }

    // ------------------------------------------------------------ liveness

    fn place_uses(p: &Place, live: &mut BitSet) {
        if p.proj.contains(&Proj::Deref) {
            live.insert(p.local.0 as usize);
        }
        for pr in &p.proj {
            if let Proj::Index(l) = pr {
                live.insert(l.0 as usize);
            }
        }
    }

    fn read_uses(p: &Place, live: &mut BitSet) {
        live.insert(p.local.0 as usize);
        Self::place_uses(p, live);
    }

    fn rvalue_uses(rv: &Rvalue, live: &mut BitSet) {
        let op = |o: &Operand, live: &mut BitSet| {
            if let Some(p) = operand_place(o) {
                Self::read_uses(p, live);
            }
        };
        match rv {
            Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Coerce(_, o, _) => op(o, live),
            Rvalue::Binary(_, a, b) => {
                op(a, live);
                op(b, live);
            }
            Rvalue::Aggregate(kind, os) => {
                os.iter().for_each(|o| op(o, live));
                if let tarn_ir::Aggregate::Closure(_, Some(storage)) = kind { Self::read_uses(storage, live); }
            },
            Rvalue::Ref(_, p) | Rvalue::Discriminant(p) | Rvalue::Len(p) => Self::read_uses(p, live),
            Rvalue::SliceRef { base, start, end, .. } => {
                Self::read_uses(base, live);
                start.iter().chain(end.iter()).for_each(|o| op(o, live));
            }
        }
    }

    /// Backward transfer of one statement: `live` goes from after to before.
    fn stmt_back(&self, kind: &StatementKind, live: &mut BitSet) {
        match kind {
            StatementKind::Assign(dest, rv) => {
                if dest.proj.is_empty() {
                    live.remove(dest.local.0 as usize);
                } else {
                    Self::place_uses(dest, live);
                }
                Self::rvalue_uses(rv, live);
            }
            StatementKind::StorageLive(l) | StatementKind::StorageDead(l) => live.remove(l.0 as usize),
            // Resource destruction can use retained loans: joining a task or
            // unlocking a guard or retiring a wake keeps borrowed storage live until this use.
            StatementKind::Drop(p) => {
                Self::place_uses(p, live);
                if self.t.decls.contains_task(&self.f.local(p.local).ty) || self.t.decls.contains_loan_resource(&self.f.local(p.local).ty) { Self::read_uses(p, live); }
            },
        }
    }

    fn term_back(term: &Terminator, live: &mut BitSet) {
        match term {
            Terminator::Call { callee, args, dest, .. } => {
                if dest.proj.is_empty() {
                    live.remove(dest.local.0 as usize);
                } else {
                    Self::place_uses(dest, live);
                }
                for a in args {
                    if let Some(p) = operand_place(a) {
                        Self::read_uses(p, live);
                    }
                }
                if let Callee::Value(o) = callee
                    && let Some(p) = operand_place(o)
                {
                    Self::read_uses(p, live);
                }
            }
            Terminator::Switch { discr, .. } => {
                if let Some(p) = operand_place(discr) {
                    Self::read_uses(p, live);
                }
            }
            Terminator::Return => {
                live.insert(RETURN.0 as usize);
            }
            _ => {}
        }
    }

    fn liveness(&mut self) {
        let n = self.f.blocks.len();
        let mut live_in = vec![BitSet::new(self.nl); n];
        let mut changed = true;
        while changed {
            changed = false;
            for b in (0..n).rev() {
                let mut live = BitSet::new(self.nl);
                for s in self.f.blocks[b].term.successors() {
                    live.union(&live_in[s.0 as usize]);
                }
                Self::term_back(&self.f.blocks[b].term, &mut live);
                for s in self.f.blocks[b].stmts.iter().rev() {
                    self.stmt_back(&s.kind, &mut live);
                }
                if live_in[b] != live {
                    live_in[b] = live;
                    changed = true;
                }
            }
        }
        self.live = Vec::with_capacity(n);
        self.live_out = Vec::with_capacity(n);
        for b in 0..n {
            let blk = &self.f.blocks[b];
            let mut out = BitSet::new(self.nl);
            for s in blk.term.successors() {
                out.union(&live_in[s.0 as usize]);
            }
            let mut points = vec![BitSet::new(self.nl); blk.stmts.len() + 1];
            let mut live = out.clone();
            Self::term_back(&blk.term, &mut live);
            points[blk.stmts.len()] = live.clone();
            for (i, s) in blk.stmts.iter().enumerate().rev() {
                self.stmt_back(&s.kind, &mut live);
                points[i] = live.clone();
            }
            self.live.push(points);
            self.live_out.push(out);
        }
    }

    fn live_after(&self, b: usize, i: usize) -> &BitSet {
        if i + 1 < self.live[b].len() { &self.live[b][i + 1] } else { &self.live_out[b] }
    }

    // ------------------------------------------------------------ loan flow

    fn entry_holds(&self) -> Holds {
        let mut h = vec![BitSet::new(self.loans.len()); self.nl];
        for (i, l) in self.loans.iter().enumerate() {
            if l.param.is_some() {
                h[l.holder.0 as usize].insert(i);
            }
        }
        h
    }

    fn holds_of(&self, h: &Holds, o: &Operand, into: &mut BitSet) {
        if let Some(p) = operand_place(o) {
            into.union(&h[p.local.0 as usize]);
        }
    }

    fn rvalue_inflow(&self, h: &Holds, b: usize, si: usize, rv: &Rvalue) -> BitSet {
        let mut inflow = BitSet::new(self.loans.len());
        match rv {
            Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Coerce(_, o, _) => self.holds_of(h, o, &mut inflow),
            Rvalue::Binary(..) | Rvalue::Discriminant(_) | Rvalue::Len(_) => {}
            Rvalue::Aggregate(_, os) => {
                os.iter().for_each(|o| self.holds_of(h, o, &mut inflow));
                if let Some(&l) = self.loan_at.get(&(b, si)) { inflow.insert(l); }
            },
            Rvalue::Ref(_, p) | Rvalue::SliceRef { base: p, .. } => {
                if let Some(&l) = self.loan_at.get(&(b, si)) {
                    inflow.insert(l);
                }
                // A reference into `p` also depends on what `p` borrows
                // (reborrows through `*r` keep `r`'s loans alive).
                inflow.union(&h[p.local.0 as usize]);
            }
        }
        inflow
    }

    fn write_holds(&self, h: &mut Holds, dest: &Place, inflow: BitSet) {
        let d = dest.local.0 as usize;
        if dest.proj.is_empty() {
            h[d] = if self.t.decls.may_contain_references(&self.f.local(dest.local).ty) || self.t.decls.contains_task(&self.f.local(dest.local).ty) { inflow } else { BitSet::new(self.loans.len()) };
        } else if !dest.proj.contains(&Proj::Deref) {
            h[d].union(&inflow);
        }
    }

    /// Loans a call result may hold: those of the arguments in the callee's
    /// provenance (all arguments when the callee is not statically known).
    fn call_inflow(&self, h: &Holds, callee: &Callee, args: &[Operand], dest: &Place) -> BitSet {
        let mut inflow = BitSet::new(self.loans.len());
        let dty = &self.f.local(dest.local).ty;
        if dest.proj.is_empty() && !self.t.decls.may_contain_references(dty) && !self.t.decls.contains_task(dty) {
            return inflow;
        }
        let positions: Option<Provenance> = match callee {
            Callee::Fn(id, _) => self.prov.get(id).cloned(),
            Callee::Virtual { method, .. } => self.t.decls.fns.get(method).map(|s| match elide(s) {
                Elided::Prov(p) => p,
                _ => Vec::new(),
            }),
            Callee::TaskSpawn { scoped: true, .. } => None,
            Callee::Builtin(_) | Callee::TaskSpawn { .. } => Some(Vec::new()),
            // Source calls without contracts are rejected with E3040. Keep
            // manually constructed/recovery IR conservative as well.
            Callee::Opaque(_) => None,
            Callee::Intrinsic(_) | Callee::Value(_) => None,
        };
        match positions {
            Some(ps) => ps.iter().filter_map(|&i| args.get(i)).for_each(|a| self.holds_of(h, a, &mut inflow)),
            None => args.iter().for_each(|a| self.holds_of(h, a, &mut inflow)),
        }
        if let Callee::Value(o) = callee {
            self.holds_of(h, o, &mut inflow);
        }
        inflow
    }

    fn consume_resource_operand(&self, h: &mut Holds, operand: &Operand) {
        if let Operand::Move(place) = operand
            && place.proj.is_empty() && (self.t.decls.contains_task(&self.f.local(place.local).ty) || self.t.decls.contains_loan_resource(&self.f.local(place.local).ty)) {
            h[place.local.0 as usize] = BitSet::new(self.loans.len());
        }
    }

    fn block_forward(&mut self, b: usize, h: &mut Holds, check: bool) {
        let blk = &self.f.blocks[b];
        for (si, s) in blk.stmts.iter().enumerate() {
            if check {
                self.check_stmt(b, si, &s.kind, s.span, h);
            }
            match &s.kind {
                StatementKind::Assign(dest, rv) => {
                    let inflow = self.rvalue_inflow(h, b, si, rv);
                    match rv {
                        Rvalue::Use(o) | Rvalue::Cast(o, _) | Rvalue::Coerce(_, o, _) => self.consume_resource_operand(h, o),
                        Rvalue::Aggregate(_, operands) => for o in operands { self.consume_resource_operand(h, o); },
                        _ => {},
                    }
                    self.write_holds(h, dest, inflow);
                }
                StatementKind::StorageLive(l) | StatementKind::StorageDead(l) => h[l.0 as usize] = BitSet::new(self.loans.len()),
                StatementKind::Drop(place) => {
                    if place.proj.is_empty() && (self.t.decls.contains_task(&self.f.local(place.local).ty) || self.t.decls.contains_loan_resource(&self.f.local(place.local).ty)) {
                        h[place.local.0 as usize] = BitSet::new(self.loans.len());
                    }
                }
            }
        }
        if check {
            self.check_term(b, h);
        }
        if let Terminator::Call { callee, args, dest, .. } = &blk.term {
            let inflow = self.call_inflow(h, callee, args, dest);
            for operand in args { self.consume_resource_operand(h, operand); }
            self.write_holds(h, dest, inflow);
        }
    }

    /// Forward fixpoint; returns the holds at every block entry.
    fn flow(&mut self) -> Vec<Option<Holds>> {
        let n = self.f.blocks.len();
        let mut ins: Vec<Option<Holds>> = vec![None; n];
        ins[0] = Some(self.entry_holds());
        let mut work: VecDeque<usize> = VecDeque::from([0]);
        while let Some(b) = work.pop_front() {
            let mut h = ins[b].clone().unwrap();
            self.block_forward(b, &mut h, false);
            for s in self.f.blocks[b].term.successors() {
                let s = s.0 as usize;
                let changed = match &mut ins[s] {
                    Some(cur) => {
                        let mut ch = false;
                        for (c, x) in cur.iter_mut().zip(&h) {
                            ch |= c.union(x);
                        }
                        ch
                    }
                    slot @ None => {
                        *slot = Some(h.clone());
                        true
                    }
                };
                if changed && !work.contains(&s) {
                    work.push_back(s);
                }
            }
        }
        ins
    }

    /// Provenance of this function's result: the parameters whose placeholder
    /// loans reach `_0` at a `return`.
    fn summary(&mut self, ins: &[Option<Holds>]) -> Provenance {
        let mut out = Vec::new();
        for (b, blk) in self.f.blocks.iter().enumerate() {
            if !matches!(blk.term, Terminator::Return) {
                continue;
            }
            let Some(mut h) = ins[b].clone() else { continue };
            self.block_forward(b, &mut h, false);
            for l in h[RETURN.0 as usize].iter() {
                if let Some(i) = self.loans[l].param
                    && !out.contains(&i)
                {
                    out.push(i);
                }
            }
        }
        out.sort_unstable();
        out
    }

    // ------------------------------------------------------------ conflicts

    /// Real loans held by locals in `live`, with the locals holding each.
    fn active(&self, h: &Holds, live: &BitSet) -> Vec<(usize, Vec<LocalId>)> {
        let mut out: HashMap<usize, Vec<LocalId>> = HashMap::new();
        for l in live.iter() {
            for loan in h[l].iter() {
                if self.loans[loan].param.is_none() {
                    out.entry(loan).or_default().push(LocalId(l as u32));
                }
            }
        }
        let mut v: Vec<_> = out.into_iter().collect();
        v.sort_by_key(|(l, _)| *l);
        v
    }

    fn conflicts(kind: LoanKind, access: Access) -> bool {
        match access {
            Access::Read | Access::BorrowShared => kind == LoanKind::Mutable,
            _ => true,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn access(&mut self, b: usize, i: usize, place: &Place, access: Access, span: Span, h: &Holds, after: bool) {
        let live = if after { self.live_after(b, i).clone() } else { self.live[b][i].clone() };
        for (loan, holders) in self.active(h, &live) {
            let ln = &self.loans[loan];
            // Holds *before* this statement never contain the loan it creates,
            // except from a previous loop iteration — a real conflict.
            if !overlap(&ln.place, place) || !Self::conflicts(ln.kind, access) {
                continue;
            }
            // Dropping a local whose borrow is only kept alive by the returned
            // value: reported as a returned reference instead (clearer).
            if access == Access::Drop && holders.iter().all(|x| *x == RETURN) {
                continue;
            }
            // Reading or writing *through* the reference that holds a loan is
            // its intended use, not a conflict (e.g. `(*r).f` while `r` holds
            // a loan of `x`): such places are rooted at the reference.
            if self.reported.insert(loan) {
                self.report(b, i, loan, &holders, place, access, span);
            }
        }
    }

    fn check_stmt(&mut self, b: usize, i: usize, kind: &StatementKind, span: Span, h: &Holds) {
        match kind {
            StatementKind::Assign(dest, rv) => {
                if matches!(rv, Rvalue::Aggregate(tarn_ir::Aggregate::Closure(..), _)) {
                    let inflow = self.rvalue_inflow(h, b, i, rv);
                    if inflow.iter().any(|l| self.f.local(self.loans[l].place.local).kind == tarn_ir::LocalKind::TaskScopeWitness) {
                        self.diags.push(Diagnostic::error("E4208", "scoped_task_escape", "a scoped task handle cannot be captured by another callable")
                            .primary(span, "join the task inside its scope before capturing its result"));
                    }
                }
                self.rvalue_accesses(b, i, rv, span, h);
                self.access(b, i, dest, Access::Write, span, h, true);
                // Storing a borrow of a local into memory behind a reference
                // would let it escape the frame.
                if dest.proj.contains(&Proj::Deref) {
                    let inflow = self.rvalue_inflow(h, b, i, rv);
                    if let Some(l) = inflow.iter().find(|&l| self.loans[l].param.is_none() && !self.loans[l].place.proj.contains(&Proj::Deref)) {
                        self.report_escape_store(l, dest, span);
                    }
                }
            }
            StatementKind::Drop(p) => {
                // `Drop(p)` immediately followed by `p = v` is one overwrite;
                // `Drop(x)` followed by `StorageDead(x)` is the end of `x`'s scope.
                let rest = &self.f.blocks[b].stmts[i + 1..];
                let overwrite = matches!(rest.first().map(|s| &s.kind), Some(StatementKind::Assign(q, _)) if q == p);
                let scope_end = p.proj.is_empty() && rest.iter().any(|s| s.kind == StatementKind::StorageDead(p.local));
                if scope_end {
                    self.storage_end(b, i, p.local, span, h);
                } else {
                    self.access(b, i, p, if overwrite { Access::Write } else { Access::Drop }, span, h, true);
                }
            }
            StatementKind::StorageDead(l) => self.storage_end(b, i, *l, span, h),
            StatementKind::StorageLive(_) => {}
        }
    }

    /// The value and storage of `l` end (scope exit): no live loan may point
    /// into it. A reference only kept alive by `return` is reported as a
    /// returned reference instead (clearer).
    fn storage_end(&mut self, b: usize, i: usize, l: LocalId, span: Span, h: &Holds) {
        let live = self.live_after(b, i).clone();
        for (loan, holders) in self.active(h, &live) {
            let ln = &self.loans[loan];
            let into_l = ln.place.local == l && !ln.place.proj.contains(&Proj::Deref);
            let only_return = holders.iter().all(|x| *x == RETURN);
            if into_l && !only_return && self.reported.insert(loan) {
                self.report_dead(b, i, loan, &holders, span);
            }
        }
    }

    fn rvalue_accesses(&mut self, b: usize, i: usize, rv: &Rvalue, span: Span, h: &Holds) {
        let op = |o: &Operand| match o {
            Operand::Copy(p) => Some((p.clone(), Access::Read)),
            Operand::Move(p) => Some((p.clone(), Access::Move)),
            Operand::Const(_) => None,
        };
        let mut accs: Vec<(Place, Access)> = Vec::new();
        match rv {
            Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Coerce(_, o, _) => accs.extend(op(o)),
            Rvalue::Binary(_, a, c) => {
                accs.extend(op(a));
                accs.extend(op(c));
            }
            Rvalue::Aggregate(kind, os) => {
                accs.extend(os.iter().filter_map(op));
                if let tarn_ir::Aggregate::Closure(_, Some(storage)) = kind { accs.push((storage.clone(), Access::BorrowShared)); }
            },
            Rvalue::Ref(m, p) | Rvalue::SliceRef { mutable: m, base: p, .. } => {
                accs.push((p.clone(), if *m { Access::BorrowMut } else { Access::BorrowShared }));
                if let Rvalue::SliceRef { start, end, .. } = rv {
                    accs.extend(start.iter().chain(end.iter()).filter_map(op));
                }
            }
            Rvalue::Discriminant(p) | Rvalue::Len(p) => accs.push((p.clone(), Access::Read)),
        }
        for (p, a) in accs {
            self.access(b, i, &p, a, span, h, false);
        }
    }

    fn check_term(&mut self, b: usize, h: &Holds) {
        let blk = &self.f.blocks[b];
        let i = blk.stmts.len();
        let tspan = blk.term_span;
        match &blk.term {
            Terminator::Call { callee, args, arg_spans, dest, spawn, .. } => {
                if let Callee::Value(o) = callee
                    && let Some(p) = operand_place(o)
                {
                    self.access(b, i, p, Access::Read, tspan, h, false);
                }
                for (k, a) in args.iter().enumerate() {
                    let span = arg_spans.get(k).copied().unwrap_or(tspan);
                    match a {
                        Operand::Copy(p) => self.access(b, i, p, Access::Read, span, h, false),
                        Operand::Move(p) => self.access(b, i, p, Access::Move, span, h, false),
                        Operand::Const(_) => {}
                    }
                    if *spawn && !matches!(callee, Callee::TaskSpawn { scoped: true, .. }) {
                        let mut held = BitSet::new(self.loans.len());
                        self.holds_of(h, a, &mut held);
                        if held.iter().next().is_some() {
                            self.diags.push(
                                Diagnostic::error("E4206", "reference_to_spawned_task", "a spawned task cannot receive a reference")
                                    .primary(span, "this argument holds a reference")
                                    .note("v0 does not track borrows across tasks; pass owned values to `spawn`"),
                            );
                        }
                    }
                }
                if !matches!(callee, Callee::Intrinsic(name) if name == "Task.join") && !matches!(callee, Callee::TaskSpawn { .. }) {
                    if args.iter().filter_map(operand_place).any(|p| self.t.decls.contains_task(&self.f.local(p.local).ty)
                        && h[p.local.0 as usize].iter().any(|l| self.f.local(self.loans[l].place.local).kind == tarn_ir::LocalKind::TaskScopeWitness)) {
                        self.diags.push(Diagnostic::error("E4208", "scoped_task_escape", "a scoped task handle must complete inside its creating scope")
                            .primary(tspan, "join the handle locally before passing its result"));
                    }
                }
                self.access(b, i, dest, Access::Write, tspan, h, true);
            }
            Terminator::Switch { discr, .. } => {
                if let Some(p) = operand_place(discr) {
                    self.access(b, i, p, Access::Read, tspan, h, false);
                }
            }
            Terminator::Return => self.check_return(h, tspan),
            _ => {}
        }
    }

    /// A returned value may only carry loans of the caller's memory: loans
    /// through a reference parameter (`*p…`) or the parameters' own placeholders.
    fn check_return(&mut self, h: &Holds, span: Span) {
        let storage_loan = |ln: &Loan| self.f.local(ln.place.local).name.as_deref() == Some("closure environment");
        let capture_error = h[RETURN.0 as usize].iter().any(|l| {
            let ln = &self.loans[l];
            ln.param.is_none() && !ln.place.proj.contains(&Proj::Deref) && !storage_loan(ln)
        });
        for l in h[RETURN.0 as usize].iter() {
            let ln = &self.loans[l];
            if ln.param.is_some() || ln.place.proj.contains(&Proj::Deref) || !self.reported.insert(l) {
                continue;
            }
            if storage_loan(ln) && capture_error { continue; }
            let name = place_name(self.f, self.t, &ln.place);
            let d = if self.f.local(ln.place.local).kind == tarn_ir::LocalKind::TaskScopeWitness {
                Diagnostic::error("E4208", "scoped_task_escape", "a scoped task handle cannot escape its creating scope")
                    .primary(span, "join the task inside its scope and return its owned result")
                    .secondary(ln.span, "the task starts in this scope")
            } else if storage_loan(ln) {
                Diagnostic::error("E4205", "closure_escapes_borrow", "borrowed closure cannot escape its stack environment")
                    .primary(span, "the closure escapes here")
                    .secondary(ln.span, "this environment ends when its creating scope exits")
                    .help("use `move fn` to create an owned environment; moved references must still remain valid")
            } else if matches!(self.f.ret, Ty::Fn(..)) {
                Diagnostic::error("E4205", "closure_escapes_borrow", format!("returned closure captures `{name}` by reference, but `{name}` is dropped when the function returns"))
                    .primary(span, "the closure escapes here")
                    .secondary(ln.span, format!("`{name}` is captured by reference here"))
                    .note("captured references must remain valid for the entire returned closure lifetime")
                    .help("capture an owned value with `move fn`, or borrow from a parameter")
            } else {
                Diagnostic::error("E4201", "reference_escapes", format!("cannot return a reference to `{name}`"))
                    .primary(span, "returned here")
                    .secondary(ln.span, format!("`{name}` is borrowed here"))
                    .note(format!("`{name}` belongs to this function and is dropped when it returns"))
                    .help("return an owned value instead, or borrow from a parameter")
            };
            self.diags.push(d);
        }
    }

    // ------------------------------------------------------------ diagnostics

    /// The first later use (in CFG order) of any of `holders` after (b, i).
    fn later_use(&self, b: usize, i: usize, holders: &[LocalId]) -> Option<Span> {
        let uses = |live: &mut BitSet| holders.iter().any(|l| live.contains(l.0 as usize));
        let mut seen = HashSet::new();
        let mut queue: VecDeque<(usize, usize)> = VecDeque::from([(b, i + 1)]);
        while let Some((blk, start)) = queue.pop_front() {
            let block = &self.f.blocks[blk];
            for si in start..block.stmts.len() {
                let mut u = BitSet::new(self.nl);
                if let StatementKind::Assign(_, rv) = &block.stmts[si].kind {
                    Self::rvalue_uses(rv, &mut u);
                    if uses(&mut u) {
                        return Some(block.stmts[si].span);
                    }
                }
            }
            if start <= block.stmts.len() {
                let mut u = BitSet::new(self.nl);
                if let Terminator::Call { args, arg_spans, .. } = &block.term {
                    for (k, a) in args.iter().enumerate() {
                        let mut ua = BitSet::new(self.nl);
                        if let Some(p) = operand_place(a) {
                            Self::read_uses(p, &mut ua);
                        }
                        if uses(&mut ua) {
                            return Some(arg_spans.get(k).copied().unwrap_or(block.term_span));
                        }
                    }
                }
                Self::term_back(&block.term, &mut u);
                if !matches!(block.term, Terminator::Call { .. } | Terminator::Return) && uses(&mut u) {
                    return Some(block.term_span);
                }
            }
            for s in block.term.successors() {
                if seen.insert(s.0) {
                    queue.push_back((s.0 as usize, 0));
                }
            }
        }
        None
    }

    fn kind_word(k: LoanKind) -> &'static str {
        match k {
            LoanKind::Shared => "shared",
            LoanKind::Mutable => "mutable",
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn report(&mut self, b: usize, i: usize, loan: usize, holders: &[LocalId], place: &Place, access: Access, span: Span) {
        let ln = self.loans[loan].clone();
        let acc = place_name(self.f, self.t, place);
        let bor = place_name(self.f, self.t, &ln.place);
        let (code, kind, msg, label) = match (access, ln.kind) {
            (Access::BorrowMut, LoanKind::Shared) => {
                ("E4101", "conflicting_borrow", format!("cannot borrow `{acc}` as mutable because it is also borrowed as shared"), "mutable borrow conflicts here")
            }
            (Access::BorrowMut, LoanKind::Mutable) => ("E4101", "conflicting_borrow", format!("cannot borrow `{acc}` as mutable more than once at a time"), "second mutable borrow here"),
            (Access::BorrowShared, _) => ("E4101", "conflicting_borrow", format!("cannot borrow `{acc}` as shared because it is mutably borrowed"), "shared borrow conflicts here"),
            (Access::Read, _) => ("E4104", "use_while_mutably_borrowed", format!("cannot use `{acc}` while it is mutably borrowed"), "used here"),
            (Access::Write, _) => ("E4102", "assign_while_borrowed", format!("cannot assign to `{acc}` because it is borrowed"), "assigned here while borrowed"),
            (Access::Move, _) => ("E4103", "move_while_borrowed", format!("cannot move out of `{acc}` because it is borrowed"), "moved here while borrowed"),
            (Access::Drop, _) => ("E4105", "dropped_while_borrowed", format!("`{acc}` is dropped while it is still borrowed"), "dropped here"),
        };
        let mut d = Diagnostic::error(code, kind, msg).primary(span, label).secondary(ln.span, format!("{} borrow of `{bor}` starts here", Self::kind_word(ln.kind)));
        if let Some(u) = self.later_use(b, i, holders) {
            d = d.secondary(u, "the borrow is still used here");
        }
        d = d.note("a borrow lasts until the last use of the reference that holds it");
        if holders.iter().any(|l| self.t.decls.contains_task(&self.f.local(*l).ty)) {
            d = d.note("a scoped worker keeps its borrow until join or scope completion");
        }
        self.diags.push(d);
    }

    fn report_dead(&mut self, b: usize, i: usize, loan: usize, holders: &[LocalId], span: Span) {
        let ln = self.loans[loan].clone();
        let root = Place::local(ln.place.local);
        let name = place_name(self.f, self.t, &root);
        let mut d = Diagnostic::error("E4105", "does_not_live_long_enough", format!("`{name}` does not live long enough"))
            .primary(span, format!("`{name}` goes out of scope here while still borrowed"))
            .secondary(ln.span, format!("`{}` is borrowed here", place_name(self.f, self.t, &ln.place)));
        if let Some(u) = self.later_use(b, i, holders) {
            d = d.secondary(u, "the borrow is still used here");
        }
        self.diags.push(d.help(format!("declare `{name}` in an outer scope so it outlives the reference")));
    }

    fn report_escape_store(&mut self, loan: usize, dest: &Place, span: Span) {
        if !self.reported.insert(loan) {
            return;
        }
        let ln = self.loans[loan].clone();
        let (name, target) = (place_name(self.f, self.t, &ln.place), place_name(self.f, self.t, dest));
        self.diags.push(
            Diagnostic::error("E4207", "borrow_escapes_through_reference", format!("cannot store a reference to `{name}` into `{target}`"))
                .primary(span, "the reference would outlive this function")
                .secondary(ln.span, format!("`{name}` is borrowed here"))
                .note(format!("`{target}` belongs to the caller; `{name}` is dropped when this function returns")),
        );
    }
}

// ---------------------------------------------------------------- program

#[derive(Debug, Default)]
pub struct BorrowResults {
    pub functions: HashMap<FunctionId, FnBorrows>,
}

impl BorrowResults {
    /// `name(args) -> borrows from: …` for functions returning references.
    pub fn provenance_lines(&self, p: &Program) -> Vec<String> {
        let mut out: Vec<String> = p
            .functions
            .iter()
            .filter(|f| !f.blocks.is_empty() && may_hold_refs(&f.ret))
            .map(|f| {
                let prov = self.functions.get(&f.id).map(|b| b.provenance.clone()).unwrap_or_default();
                let names: Vec<String> = prov.iter().map(|&i| f.local(LocalId(i as u32 + 1)).name.clone().unwrap_or_else(|| format!("#{i}"))).collect();
                format!("{} -> borrows from {{{}}}", f.name, names.join(", "))
            })
            .collect();
        out.sort();
        out
    }
}

/// Run the borrow checker. Functions in `skip` (move errors) are only used
/// for provenance, not checked, to avoid cascading diagnostics.
pub fn check_borrows(p: &Program, r: &Resolved, t: &Typed, skip: &HashSet<FunctionId>) -> (BorrowResults, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    // 1. Provenance of declarations without a body (ADR 0010 elision).
    let mut prov: HashMap<FunctionId, Provenance> = HashMap::new();
    for f in &p.functions {
        if !f.blocks.is_empty() {
            prov.insert(f.id, Vec::new());
            continue;
        }
        if let Some(sig) = f.symbol.and_then(|s| t.decls.fns.get(&s))
            && let Elided::Prov(ps) = elide(sig)
        {
            prov.insert(f.id, ps);
        }
    }
    check_declarations(r, t, &mut diags);
    // 2. Bodies: provenance summaries to a fixpoint (recursion), then checking.
    for _ in 0..32 {
        let mut changed = false;
        for f in p.functions.iter().filter(|f| !f.blocks.is_empty()) {
            let mut fx = Fx::new(f, t, &prov);
            let ins = fx.flow();
            let s = fx.summary(&ins);
            if prov.get(&f.id) != Some(&s) {
                prov.insert(f.id, s);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut results = BorrowResults::default();
    for f in p.functions.iter().filter(|f| !f.blocks.is_empty()) {
        let mut fx = Fx::new(f, t, &prov);
        let ins = fx.flow();
        if !skip.contains(&f.id) {
            for (b, h) in ins.iter().enumerate() {
                if let Some(mut h) = h.clone() {
                    fx.block_forward(b, &mut h, true);
                }
            }
        }
        diags.append(&mut fx.diags);
        let provenance = prov.get(&f.id).cloned().unwrap_or_default();
        results.functions.insert(f.id, FnBorrows { loans: fx.loans, provenance });
    }
    check_impl_provenance(p, r, t, &prov, &mut diags);
    (results, diags)
}

/// E4202 for interface methods and extern functions whose returned reference
/// has no unambiguous source (ADR 0010).
fn check_declarations(r: &Resolved, t: &Typed, diags: &mut Vec<Diagnostic>) {
    let mut syms: Vec<_> = t.decls.fns.iter().collect();
    syms.sort_by_key(|(s, _)| **s);
    for (s, sig) in syms {
        let sym = r.symbol(*s);
        let bodyless = matches!(sym.kind, SymbolKind::InterfaceMethod { .. }) || sig.abi.is_some();
        if !bodyless {
            continue;
        }
        match elide(sig) {
            Elided::Ambiguous(n) => diags.push(
                Diagnostic::error("E4202", "ambiguous_provenance", format!("cannot infer where the reference returned by `{}` comes from", sym.name))
                    .primary(sig.span, format!("{n} reference parameters could be the source"))
                    .note("without an explicit borrows(...) clause, only a borrowed receiver or a single reference parameter determines it")
                    .help("declare borrows(input, ...), return an owned value, or keep only one reference input"),
            ),
            Elided::NoSource => diags.push(
                Diagnostic::error("E4201", "reference_escapes", format!("`{}` returns a reference but has no reference parameter to borrow from", sym.name))
                    .primary(sig.span, "")
                    .help("return an owned value"),
            ),
            _ => {}
        }
    }
}

/// E4204: an `impl` method may not return borrows its interface does not allow.
fn check_impl_provenance(p: &Program, r: &Resolved, t: &Typed, prov: &HashMap<FunctionId, Provenance>, diags: &mut Vec<Diagnostic>) {
    for f in &p.functions {
        let Some(sym) = f.symbol else { continue };
        let SymbolKind::ImplMethod { interface: Some(iface), .. } = &r.symbol(sym).kind else { continue };
        let Some(decl) = r.member(*iface, &r.symbol(sym).name) else { continue };
        let Some(dsig) = t.decls.fns.get(&decl) else { continue };
        let Elided::Prov(allowed) = elide(dsig) else { continue };
        let have = prov.get(&f.id).cloned().unwrap_or_default();
        if have.iter().any(|i| !allowed.contains(i)) {
            diags.push(
                Diagnostic::error("E4204", "provenance_mismatch", format!("`{}` returns a borrow its interface declaration does not allow", r.symbol(sym).name))
                    .primary(t.decls.fns[&sym].span, "")
                    .secondary(dsig.span, "the interface restricts which inputs may supply the result")
                    .help("return a reference derived only from the declared allowed inputs"),
            );
        }
    }
}
