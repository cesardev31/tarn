//! Move/init checking (phase 6A) as a forward dataflow analysis on the IR.
//!
//! State per move path: `maybe_init`, `maybe_uninit` and the move sites that
//! may reach this point. Join is pointwise union (ADR 0024), so a value moved
//! on *any* incoming path is "possibly moved" unless every path reinitialized
//! it. After the fixpoint, a second pass checks every use against the state
//! at that point and classifies every `Drop` for drop elaboration.

use crate::paths::{Lookup, MovePathId, MovePaths, PathElem};
use crate::{DropDecision, FnMoves, InitState};
use std::collections::{HashMap, HashSet};
use tarn_diagnostics::{Diagnostic, Span};
use tarn_ir::{BlockId, Callee, Function, LocalKind, Operand, Place, Rvalue, StatementKind, Terminator};
use tarn_resolve::Resolved;
use tarn_types::{Ty, Typed};

#[derive(Clone, PartialEq, Debug)]
struct State {
    init: Vec<bool>,
    uninit: Vec<bool>,
    /// Move sites (indices into `Cx::sites`) that may have moved each path.
    moves: Vec<Vec<u32>>,
}

impl State {
    fn join(&mut self, o: &State) -> bool {
        let before = self.clone();
        for i in 0..self.init.len() {
            self.init[i] |= o.init[i];
            self.uninit[i] |= o.uninit[i];
            for m in &o.moves[i] {
                if !self.moves[i].contains(m) {
                    self.moves[i].push(*m);
                }
            }
            self.moves[i].sort_unstable();
        }
        *self != before
    }
}

/// Where a use happens, for diagnostics.
#[derive(Clone, Copy)]
struct At {
    span: Span,
    /// Key of a potential move site: (block, statement or terminator, operand).
    key: (u32, u32, u32),
}

struct Cx<'a> {
    f: &'a Function,
    r: &'a Resolved,
    t: &'a Typed,
    mp: MovePaths,
    sites: HashMap<(u32, u32, u32), u32>,
    site_info: Vec<(Span, MovePathId)>,
    report: bool,
    reported: HashSet<MovePathId>,
    diags: Vec<Diagnostic>,
    drops: Vec<(BlockId, usize, DropDecision)>,
    drop_states: HashMap<(BlockId, usize), Vec<(Place, InitState)>>,
}

pub fn check_function(f: &Function, r: &Resolved, t: &Typed) -> (FnMoves, Vec<Diagnostic>) {
    if f.blocks.is_empty() {
        return (FnMoves::default(), Vec::new());
    }
    let mp = MovePaths::build(f);
    let mut cx = Cx { f, r, t, mp, sites: HashMap::new(), site_info: Vec::new(), report: false, reported: HashSet::new(), diags: Vec::new(), drops: Vec::new(), drop_states: HashMap::new() };
    let entry = cx.entry_state();
    let n = f.blocks.len();
    let mut ins: Vec<Option<State>> = vec![None; n];
    ins[0] = Some(entry);
    let mut work: Vec<usize> = vec![0];
    while let Some(b) = work.pop() {
        let mut st = ins[b].clone().unwrap();
        cx.block(b, &mut st);
        for s in f.blocks[b].term.successors() {
            let s = s.0 as usize;
            let changed = match &mut ins[s] {
                Some(cur) => cur.join(&st),
                slot @ None => {
                    *slot = Some(st.clone());
                    true
                }
            };
            if changed && !work.contains(&s) {
                work.push(s);
            }
        }
    }
    // Reporting pass over the fixpoint.
    cx.report = true;
    for (b, st) in ins.into_iter().enumerate() {
        if let Some(mut st) = st {
            cx.block(b, &mut st);
        }
    }
    (FnMoves { drops: cx.drops, drop_states: cx.drop_states, has_errors: false }, cx.diags)
}

impl Cx<'_> {
    fn entry_state(&self) -> State {
        let n = self.mp.len();
        let mut st = State { init: vec![false; n], uninit: vec![true; n], moves: vec![Vec::new(); n] };
        for l in self.f.params() {
            for &p in self.mp.subtree(self.mp.root(l)) {
                st.init[p.0 as usize] = true;
                st.uninit[p.0 as usize] = false;
            }
        }
        st
    }

    fn set_init(&self, st: &mut State, p: MovePathId) {
        for &q in self.mp.subtree(p) {
            let i = q.0 as usize;
            st.init[i] = true;
            st.uninit[i] = false;
            st.moves[i].clear();
        }
    }

    fn set_uninit(&self, st: &mut State, p: MovePathId, site: Option<u32>) {
        for &q in self.mp.subtree(p) {
            let i = q.0 as usize;
            st.init[i] = false;
            st.uninit[i] = true;
            st.moves[i] = site.into_iter().collect();
        }
    }

    fn site(&mut self, at: At, p: MovePathId) -> u32 {
        if let Some(&s) = self.sites.get(&at.key) {
            return s;
        }
        let id = self.site_info.len() as u32;
        self.site_info.push((at.span, p));
        self.sites.insert(at.key, id);
        id
    }

    // ------------------------------------------------------------ transfer

    fn block(&mut self, b: usize, st: &mut State) {
        let blk = &self.f.blocks[b];
        for (si, s) in blk.stmts.iter().enumerate() {
            let at = |k: u32| At { span: s.span, key: (b as u32, si as u32, k) };
            match &s.kind {
                StatementKind::Assign(place, rv) => {
                    self.rvalue(rv, st, at(0), s.span);
                    self.write(place, st, s.span);
                }
                StatementKind::StorageLive(l) | StatementKind::StorageDead(l) => {
                    let root = self.mp.root(*l);
                    self.set_uninit(st, root, None);
                }
                StatementKind::Drop(place) => {
                    if let Lookup::Exact(p) = self.mp.lookup(place) {
                        if self.report {
                            let d = self.drop_decision(st, p);
                            self.drops.push((BlockId(b as u32), si, d));
                            let states = self.mp.paths.iter().enumerate().map(|(i, path)| {
                                let mut proj = Vec::new();
                                let mut cur = MovePathId(i as u32);
                                while let Some(parent) = self.mp.path(cur).parent {
                                    proj.push(match self.mp.path(cur).elem.unwrap() {
                                        PathElem::Field(f) => tarn_ir::Proj::Field(f),
                                        PathElem::Downcast(v) => tarn_ir::Proj::Downcast(v),
                                    });
                                    cur = parent;
                                }
                                proj.reverse();
                                let state = match (st.init[i], st.uninit[i]) {
                                    (true, false) => InitState::Live,
                                    (true, true) => InitState::Maybe,
                                    _ => InitState::Dead,
                                };
                                (Place { local: path.local, proj }, state)
                            }).collect();
                            self.drop_states.insert((BlockId(b as u32), si), states);
                        }
                        self.set_uninit(st, p, None);
                    } else if self.report {
                        self.drops.push((BlockId(b as u32), si, DropDecision::Static));
                        self.drop_states.insert((BlockId(b as u32), si), vec![(place.clone(), InitState::Live)]);
                    }
                }
            }
        }
        let tspan = blk.term_span;
        match &blk.term {
            Terminator::Call { callee, args, arg_spans, dest, .. } => {
                if let Callee::Value(o) = callee {
                    self.operand(o, st, At { span: tspan, key: (b as u32, u32::MAX, u32::MAX) });
                }
                for (i, a) in args.iter().enumerate() {
                    let span = arg_spans.get(i).copied().unwrap_or(tspan);
                    self.operand(a, st, At { span, key: (b as u32, u32::MAX, i as u32) });
                }
                self.write(dest, st, tspan);
            }
            Terminator::Switch { discr, .. } => self.operand(discr, st, At { span: tspan, key: (b as u32, u32::MAX, 0) }),
            _ => {}
        }
    }

    fn rvalue(&mut self, rv: &Rvalue, st: &mut State, at: At, span: Span) {
        let at_k = |k: u32| At { span: at.span, key: (at.key.0, at.key.1, k) };
        match rv {
            Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Coerce(_, o, _) => self.operand(o, st, at_k(0)),
            Rvalue::Binary(_, a, b) => {
                self.operand(a, st, at_k(0));
                self.operand(b, st, at_k(1));
            }
            Rvalue::Aggregate(_, os) => {
                for (i, o) in os.iter().enumerate() {
                    self.operand(o, st, at_k(i as u32));
                }
            }
            Rvalue::Ref(_, p) => self.read(p, st, span, true),
            Rvalue::SliceRef { base, start, end, .. } => {
                self.read(base, st, span, true);
                for (i, o) in start.iter().chain(end.iter()).enumerate() {
                    self.operand(o, st, at_k(i as u32));
                }
            }
            // Reading a discriminant or a length only needs the place itself.
            Rvalue::Discriminant(p) | Rvalue::Len(p) => self.read(p, st, span, false),
        }
    }

    fn operand(&mut self, o: &Operand, st: &mut State, at: At) {
        match o {
            Operand::Const(_) => {}
            Operand::Copy(p) => self.read(p, st, at.span, true),
            Operand::Move(p) => {
                self.read(p, st, at.span, true);
                match self.mp.lookup(p) {
                    Lookup::Exact(path) => {
                        let site = self.site(at, path);
                        self.set_uninit(st, path, Some(site));
                    }
                    Lookup::Deref(_) => self.err_move_out_of_ref(p, at.span),
                    Lookup::Index(base) => {
                        // Consuming iteration moves elements out of its own
                        // array (ADR 0024); any other move by index is an error.
                        if self.f.local(self.mp.path(base).local).kind != LocalKind::IterArray {
                            self.err_move_out_of_index(at.span);
                        }
                    }
                }
            }
        }
    }

    /// A read of `place`. `deep`: the whole value must be initialized
    /// (reads, borrows); otherwise only the place itself (discriminant, len).
    fn read(&mut self, place: &Place, st: &State, span: Span, deep: bool) {
        let (path, deep) = match self.mp.lookup(place) {
            Lookup::Exact(p) => (p, deep),
            // Reading through a reference or index needs the reference /
            // array itself to be initialized.
            Lookup::Deref(p) | Lookup::Index(p) => (p, false),
        };
        if !self.report || self.reported.contains(&path) {
            return;
        }
        let own = path.0 as usize;
        if st.uninit[own] {
            self.reported.insert(path);
            self.err_use(path, st, span);
            return;
        }
        if deep && let Some(&d) = self.mp.subtree(path)[1..].iter().find(|d| st.uninit[d.0 as usize]) {
            self.reported.insert(path);
            self.err_partial(path, d, st, span);
        }
    }

    fn write(&mut self, place: &Place, st: &mut State, span: Span) {
        match self.mp.lookup(place) {
            Lookup::Exact(p) => {
                // Assigning a field needs its parents to exist.
                if self.report
                    && let Some(&a) = self.mp.ancestors(p).iter().find(|a| st.uninit[a.0 as usize])
                    && !self.reported.contains(&a)
                {
                    self.reported.insert(a);
                    self.err_assign_into_moved(p, a, st, span);
                }
                self.set_init(st, p);
            }
            Lookup::Deref(p) | Lookup::Index(p) => {
                // `(*r).f = v` / `a[i] = v`: the reference / array must exist.
                if self.report && st.uninit[p.0 as usize] && !self.reported.contains(&p) {
                    self.reported.insert(p);
                    self.err_use(p, st, span);
                }
            }
        }
    }

    fn drop_decision(&self, st: &State, p: MovePathId) -> DropDecision {
        let sub = self.mp.subtree(p);
        let maybe = |q: &MovePathId| st.init[q.0 as usize] && st.uninit[q.0 as usize];
        let def_init = |q: &MovePathId| st.init[q.0 as usize] && !st.uninit[q.0 as usize];
        let def_uninit = |q: &MovePathId| !st.init[q.0 as usize] && st.uninit[q.0 as usize];
        if sub.iter().all(def_init) {
            DropDecision::Static
        } else if def_uninit(&p) {
            if maybe(&p) { DropDecision::Conditional } else { DropDecision::Dead }
        } else if sub.iter().any(maybe) {
            DropDecision::Conditional
        } else {
            // The root is initialized; some fields were moved on every path:
            // drop only what remains. Report the moved fields.
            let moved: Vec<String> = sub[1..]
                .iter()
                .filter(|q| def_uninit(q) && self.mp.path(**q).parent.is_some_and(|par| !def_uninit(&par)))
                .map(|q| self.name(*q))
                .collect();
            DropDecision::Partial(moved)
        }
    }

    // ------------------------------------------------------------ names

    /// Source-like name of a path: `p.first`, `s.Named.0`, `value` for temps.
    fn name(&self, p: MovePathId) -> String {
        self.name_ty(p).0
    }

    fn name_ty(&self, p: MovePathId) -> (String, Ty) {
        let path = self.mp.path(p);
        match (path.parent, path.elem) {
            (None, _) => {
                let l = self.f.local(path.local);
                let n = match l.kind {
                    LocalKind::User | LocalKind::Param => l.name.clone().unwrap_or_else(|| "value".into()),
                    _ => "value".into(),
                };
                (n, l.ty.clone())
            }
            (Some(parent), Some(elem)) => {
                let (pn, pty) = self.name_ty(parent);
                let pty = peel(&pty);
                match (elem, &pty) {
                    (PathElem::Field(i), Ty::Adt(s, args)) if self.t.decls.structs.contains_key(s) => {
                        let def = &self.t.decls.structs[s];
                        let map: HashMap<_, _> = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                        let f = &def.fields[i as usize];
                        (format!("{pn}.{}", f.name), tarn_types::subst(&f.ty, &map))
                    }
                    (PathElem::Downcast(v), Ty::Adt(e, _)) => {
                        let vn = self.t.decls.enums.get(e).and_then(|d| d.variants.get(v as usize)).map(|x| x.name.clone()).unwrap_or_default();
                        (format!("{pn}.{vn}"), pty.clone())
                    }
                    (PathElem::Field(i), _) => {
                        // Field `i` of a variant: the parent is a `Downcast`.
                        let ty = self.variant_field_ty(parent, i).unwrap_or(Ty::Error);
                        (format!("{pn}.{i}"), ty)
                    }
                    _ => (pn, Ty::Error),
                }
            }
            _ => ("value".into(), Ty::Error),
        }
    }

    fn variant_field_ty(&self, downcast: MovePathId, i: u32) -> Option<Ty> {
        let dp = self.mp.path(downcast);
        let Some(PathElem::Downcast(v)) = dp.elem else { return None };
        let (_, ety) = self.name_ty(dp.parent?);
        let Ty::Adt(e, args) = peel(&ety) else { return None };
        let def = self.t.decls.enums.get(&e)?;
        let map: HashMap<_, _> = def.generics.iter().copied().zip(args.iter().cloned()).collect();
        def.variants.get(v as usize).and_then(|x| x.fields.get(i as usize)).map(|f| tarn_types::subst(f, &map))
    }

    fn ty_name(&self, t: &Ty) -> String {
        self.t.display(t, self.r)
    }

    // ------------------------------------------------------------ diagnostics

    fn move_labels(&self, mut d: Diagnostic, path: MovePathId, st: &State, use_span: Span, verb: &str) -> Diagnostic {
        let mut in_loop = false;
        for &m in &st.moves[path.0 as usize] {
            let (span, _) = self.site_info[m as usize];
            if span.start >= use_span.start {
                in_loop = true;
                d = d.secondary(span, format!("{verb} here, in a previous iteration of the loop"));
            } else {
                d = d.secondary(span, format!("{verb} here"));
            }
        }
        if in_loop {
            d = d.note("a value moved inside a loop is gone in the next iteration");
        }
        d
    }

    fn err_use(&mut self, path: MovePathId, st: &State, span: Span) {
        let (name, ty) = self.name_ty(path);
        let i = path.0 as usize;
        let possibly = st.init[i];
        let moved = !st.moves[i].is_empty();
        let d = if moved {
            let msg = if possibly { format!("use of possibly moved value `{name}`") } else { format!("use of moved value `{name}`") };
            let label = if possibly { "used here, but it may have been moved" } else { "value used here after move" };
            let mut d = Diagnostic::error("E4001", "use_after_move", msg).primary(span, label);
            d = self.move_labels(d, path, st, span, "value moved");
            let tn = self.ty_name(&ty);
            d = d.note(format!("`{tn}` is not a copy type, so assigning or passing it moves it"));
            if possibly {
                d.help("move it on every path or on none, or reassign it before this use")
            } else {
                d.help(format!("to keep using `{name}`, borrow it where it is moved (`&{name}`) instead of moving it"))
            }
        } else {
            let msg = if possibly { format!("use of possibly uninitialized `{name}`") } else { format!("use of uninitialized `{name}`") };
            let mut d = Diagnostic::error("E4005", "uninitialized", msg).primary(span, if possibly { "may not be initialized here" } else { "not initialized here" });
            let decl = self.f.local(self.mp.path(path).local).span;
            d = d.secondary(decl, "declared here without a value");
            d.help("assign it on every path before this use")
        };
        self.diags.push(d);
    }

    fn err_partial(&mut self, path: MovePathId, moved: MovePathId, st: &State, span: Span) {
        let name = self.name(path);
        let mname = self.name(moved);
        let possibly = st.init[moved.0 as usize];
        let msg = if possibly { format!("use of possibly partially moved value `{name}`") } else { format!("use of partially moved value `{name}`") };
        let mut d = Diagnostic::error("E4002", "use_of_partially_moved", msg).primary(span, "the whole value is used here");
        d = self.move_labels(d, moved, st, span, &format!("`{mname}` moved"));
        d = d.note(format!("the fields of `{name}` that were not moved can still be used one by one"));
        self.diags.push(d);
    }

    fn err_assign_into_moved(&mut self, field: MovePathId, parent: MovePathId, st: &State, span: Span) {
        let (fname, pname) = (self.name(field), self.name(parent));
        let moved = !st.moves[parent.0 as usize].is_empty();
        let what = if moved { "has been moved" } else { "is not initialized" };
        let mut d = Diagnostic::error("E4006", "assign_into_moved", format!("cannot assign to `{fname}`: `{pname}` {what}")).primary(span, "");
        d = self.move_labels(d, parent, st, span, &format!("`{pname}` moved"));
        self.diags.push(d.help(format!("assign the whole value instead: `{pname} = ...`")));
    }

    fn err_move_out_of_ref(&mut self, _p: &Place, span: Span) {
        if !self.report {
            return;
        }
        self.diags.push(
            Diagnostic::error("E4003", "move_out_of_reference", "cannot move a value out from behind a reference")
                .primary(span, "this moves a non-copy value out of a borrowed place")
                .help("borrow it instead (`&...`), or work with a copy if the type allows it")
                .note("a reference only lends the value; the owner keeps it"),
        );
    }

    fn err_move_out_of_index(&mut self, span: Span) {
        if !self.report {
            return;
        }
        self.diags.push(
            Diagnostic::error("E4004", "move_out_of_index", "cannot move an element out of an array by index")
                .primary(span, "moving this element would leave a hole in the array")
                .help("borrow it (`&arr[i]`), or consume the whole array with `for x in arr`"),
        );
    }
}

fn peel(t: &Ty) -> Ty {
    match t {
        Ty::Ref(_, inner) => peel(inner),
        t => t.clone(),
    }
}
