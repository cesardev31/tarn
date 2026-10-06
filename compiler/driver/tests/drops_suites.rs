//! Snapshots plus a resource-token interpreter: verifies executed paths rather
//! than counting syntactic drops. This is a test oracle, not a Tarn backend.
mod common;
use common::*;
use std::collections::HashSet;
use tarn_ir::{post_drop as post, *};

#[derive(Clone, Debug)]
enum Value {
    Unit,
    Number(i128),
    Token(usize),
    Fields(Vec<Option<Value>>),
    Variant(usize, Vec<Option<Value>>),
}
#[derive(Clone, Debug)]
enum Flag {
    Bit(bool),
    Bits(Vec<bool>),
}
struct Frame {
    locals: Vec<Option<Value>>,
    flags: Vec<Option<Flag>>,
}
struct Machine<'a> {
    p: &'a post::Program,
    created: usize,
    destroyed: HashSet<usize>,
    trace: Vec<usize>,
    steps: usize,
    aborted: bool,
}
impl Machine<'_> {
    fn token(&mut self) -> Value {
        let id = self.created;
        self.created += 1;
        Value::Token(id)
    }
    fn destroy(&mut self, v: Value) {
        match v {
            Value::Token(id) => {
                assert!(self.destroyed.insert(id), "double drop {id}");
                self.trace.push(id);
            }
            Value::Fields(xs) | Value::Variant(_, xs) => {
                for x in xs {
                    self.destroy(x.expect("complete drop of partial value"));
                }
            }
            _ => {}
        }
    }
    fn slot<'a>(fr: &'a mut Frame, p: &Place) -> &'a mut Option<Value> {
        let mut slot = &mut fr.locals[p.local.0 as usize];
        for pr in &p.proj {
            match pr {
                Proj::Downcast(v) => assert!(matches!(slot, Some(Value::Variant(tag, _)) if *tag == *v as usize)),
                Proj::Field(i) => {
                    let xs = match slot.as_mut().expect("projection through moved place") {
                        Value::Fields(xs) | Value::Variant(_, xs) => xs,
                        v => panic!("field on {v:?}"),
                    };
                    slot = &mut xs[*i as usize];
                }
                Proj::Index(i) => {
                    // The index itself is always a scalar; read before borrowing
                    // the array slot by restarting the walk with a constant.
                    panic!("index must be resolved before slot: {i:?}");
                }
                Proj::Deref => panic!("reference interpreter not used in drop fixtures"),
            }
        }
        slot
    }
    fn take(fr: &mut Frame, p: &Place, copy: bool) -> Value {
        if let Some(Proj::Index(i)) = p.proj.last() {
            let Some(Value::Number(idx)) = fr.locals[i.0 as usize] else { panic!("bad index") };
            let base = Place { local: p.local, proj: p.proj[..p.proj.len() - 1].to_vec() };
            let Some(Value::Fields(xs)) = Self::slot(fr, &base) else { panic!("index on non-array") };
            return if copy { xs[idx as usize].clone().unwrap() } else { xs[idx as usize].take().unwrap() };
        }
        let slot = Self::slot(fr, p);
        if copy { slot.clone().expect("copy uninit") } else { slot.take().expect("move uninit") }
    }
    fn op(&mut self, fr: &mut Frame, o: &Operand) -> Value {
        match o {
            Operand::Copy(p) => Self::take(fr, p, true),
            Operand::Move(p) => Self::take(fr, p, false),
            Operand::Const(Const::Int(n, _)) => Value::Number(*n),
            Operand::Const(Const::Bool(b)) => Value::Number(i128::from(*b)),
            Operand::Const(Const::Str(_)) => self.token(),
            Operand::Const(Const::Unit) => Value::Unit,
            x => panic!("unsupported operand {x:?}"),
        }
    }
    fn rv(&mut self, fr: &mut Frame, rv: &Rvalue) -> Value {
        match rv {
            Rvalue::Use(o) => self.op(fr, o),
            Rvalue::Aggregate(a, os) => {
                let xs = os.iter().map(|o| Some(self.op(fr, o))).collect();
                match a {
                    Aggregate::Variant(_, v, _) => Value::Variant(*v as usize, xs),
                    _ => Value::Fields(xs),
                }
            }
            Rvalue::Binary(op, a, b) => {
                let Value::Number(a) = self.op(fr, a) else { panic!("non-number") };
                let Value::Number(b) = self.op(fr, b) else { panic!("non-number") };
                Value::Number(match op {
                    BinOp::Add => a + b,
                    BinOp::Lt => (a < b).into(),
                    BinOp::Eq => (a == b).into(),
                    _ => panic!("binary {op:?}"),
                })
            }
            Rvalue::Discriminant(p) => {
                let Some(Value::Variant(v, _)) = Self::slot(fr, p) else { panic!("bad discriminant") };
                Value::Number(*v as i128)
            }
            Rvalue::Len(p) => {
                let Some(Value::Fields(xs)) = Self::slot(fr, p) else { panic!("bad length") };
                Value::Number(xs.len() as i128)
            }
            x => panic!("unsupported rvalue {x:?}"),
        }
    }
    fn drop_plan(&mut self, fr: &mut Frame, d: &post::Drop) {
        match d {
            post::Drop::Value(p) => self.destroy(Self::take(fr, p, false)),
            post::Drop::Guard(id, d) => {
                let Some(Flag::Bit(b)) = fr.flags[id.0 as usize] else { panic!("uninit flag") };
                if b {
                    self.drop_plan(fr, d);
                }
            }
            post::Drop::Fields { fields, .. } => {
                for (_, d) in fields {
                    self.drop_plan(fr, d);
                }
            }
            post::Drop::Variants { place, variants } => {
                let Some(Value::Variant(tag, _)) = Self::slot(fr, place) else { panic!("dead discriminant") };
                let tag = *tag;
                for (_, d) in &variants[tag] {
                    self.drop_plan(fr, d);
                }
            }
            post::Drop::Remaining { place, flag } => {
                let Some(Flag::Bits(bits)) = fr.flags[flag.0 as usize].clone() else { panic!("uninit bitmap") };
                let Some(Value::Fields(xs)) = Self::slot(fr, place).take() else { panic!("dead array") };
                for (live, v) in bits.into_iter().zip(xs) {
                    if live {
                        self.destroy(v.expect("drop hole"));
                    } else {
                        assert!(v.is_none(), "lost live array element");
                    }
                }
            }
        }
    }
    fn call(&mut self, id: FunctionId, args: Vec<Value>) -> Value {
        let f = &self.p.functions[id.0 as usize];
        let mut fr = Frame { locals: vec![None; f.decl.locals.len()], flags: vec![None; f.flags.len()] };
        for (i, v) in args.into_iter().enumerate() {
            fr.locals[i + 1] = Some(v);
        }
        let mut bi = 0;
        loop {
            self.steps += 1;
            assert!(self.steps < 10000, "non-terminating fixture");
            let b = &f.blocks[bi];
            for s in &b.stmts {
                match &s.op {
                    post::Op::Set(id, v) => {
                        fr.flags[id.0 as usize] = Some(match &f.flags[id.0 as usize] {
                            post::FlagKind::Value(_) => Flag::Bit(*v),
                            post::FlagKind::Elements(_, n) => Flag::Bits(vec![*v; *n as usize]),
                        })
                    }
                    post::Op::ClearElement(id, idx) => {
                        let Some(Value::Number(i)) = fr.locals[idx.0 as usize] else { panic!("bad bitmap index") };
                        let Some(Flag::Bits(bits)) = &mut fr.flags[id.0 as usize] else { panic!("uninit bitmap") };
                        assert!(bits[i as usize], "double take array element");
                        bits[i as usize] = false;
                    }
                    post::Op::Destroy(d) => self.drop_plan(&mut fr, d),
                    post::Op::Plain(StatementKind::Assign(p, rv)) => {
                        let v = self.rv(&mut fr, rv);
                        let slot = Self::slot(&mut fr, p);
                        assert!(!slot.as_ref().is_some_and(has_resource), "overwrite without destruction: {p:?}");
                        *slot = Some(v);
                    }
                    post::Op::Plain(StatementKind::StorageLive(l) | StatementKind::StorageDead(l)) => {
                        assert!(!fr.locals[l.0 as usize].as_ref().is_some_and(has_resource), "scope lost live resource");
                        fr.locals[l.0 as usize] = None;
                    }
                    x => panic!("unexpected {x:?}"),
                }
            }
            match &b.term {
                Terminator::Goto(next) => bi = next.0 as usize,
                Terminator::Switch { discr, cases, otherwise } => {
                    let Value::Number(v) = self.op(&mut fr, discr) else { panic!("bad switch") };
                    bi = cases.iter().find(|(n, _)| *n == v).map(|(_, b)| *b).unwrap_or(*otherwise).0 as usize;
                }
                Terminator::Call { callee, args, dest, next, .. } => {
                    let args = args.iter().map(|o| self.op(&mut fr, o)).collect();
                    let value = match callee {
                        Callee::Fn(id, _) => self.call(*id, args),
                        Callee::Builtin(Builtin::Panic) => {
                            self.aborted = true;
                            return Value::Unit;
                        }
                        c => panic!("unsupported callee {c:?}"),
                    };
                    if self.aborted {
                        return Value::Unit;
                    }
                    let slot = Self::slot(&mut fr, dest);
                    assert!(!slot.as_ref().is_some_and(has_resource), "call overwrite live");
                    *slot = Some(value);
                    bi = next.expect("returning call without edge").0 as usize;
                }
                Terminator::Return => {
                    assert!(fr.locals.iter().skip(1).all(|v| !v.as_ref().is_some_and(has_resource)), "return leaked resources: {:?}", fr.locals);
                    return fr.locals[0].take().unwrap_or(Value::Unit);
                }
                Terminator::Unreachable => panic!("executed unreachable"),
            }
        }
    }
}
fn has_resource(v: &Value) -> bool {
    match v {
        Value::Token(_) => true,
        Value::Fields(xs) | Value::Variant(_, xs) => xs.iter().flatten().any(has_resource),
        _ => false,
    }
}

#[test]
fn snapshots_and_all_boolean_paths_destroy_exactly_once() {
    let mut failures = Vec::new();
    let files = entries("drops", "pass");
    assert!(files.len() >= 20);
    for path in files {
        let res = tarn_driver::check(&path).unwrap();
        assert!(!res.has_errors(), "{}: {}", path.display(), res.diagnostics.iter().map(|d| d.render(&res.program.sources)).collect::<String>());
        let p = res.drops.as_ref().expect("missing post-drop IR");
        let t = res.typed.as_ref().unwrap();
        assert!(post::verify(p, t).is_empty());
        if ["initialized.tarn", "moved.tarn", "partial.tarn", "partial_two.tarn", "partial_three.tarn", "overwrite.tarn", "copy.tarn"].iter().any(|name| path.ends_with(name)) {
            let main = p.functions.iter().find(|f| f.decl.name == "main").unwrap();
            assert!(main.flags.is_empty(), "static/dead/definite partial drops need no flags");
        }
        golden(&path, "drops", &post::print_program(p, res.resolved.as_ref().unwrap(), t), &mut failures);
        let main = p.functions.iter().find(|f| f.decl.name == "main").unwrap();
        for bits in 0..(1 << main.decl.param_count) {
            let mut m = Machine { p, created: 0, destroyed: HashSet::new(), trace: Vec::new(), steps: 0, aborted: false };
            m.call(main.decl.id, (0..main.decl.param_count).map(|i| Value::Number(((bits >> i) & 1).into())).collect());
            if !m.aborted {
                assert_eq!(m.created, m.destroyed.len(), "{} path {bits}: resource leak", path.display());
            } else {
                assert!(path.ends_with("panic.tarn"));
                assert!(m.destroyed.is_empty(), "panic performed stack destruction");
            }
            if path.ends_with("initialized.tarn") {
                assert_eq!(m.trace, vec![1, 0], "reverse local declaration order");
            }
            if path.ends_with("partial_three.tarn") {
                assert_eq!(m.trace, vec![0, 1, 2, 3], "nested declaration field order after x");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn verifier_rejects_corrupt_post_drop_ir_without_panicking() {
    let path = std::path::Path::new("../../tests/drops/pass/partial_conditional.tarn");
    let mut res = tarn_driver::check(path).unwrap();
    let t = res.typed.as_ref().unwrap();
    let p = res.drops.as_mut().unwrap();
    let fi = p.functions.iter().position(|f| f.decl.name == "main").unwrap();
    let good = p.functions[fi].clone();
    let span = good.decl.span;
    let flag = post::FlagId(0);
    let place = match &good.flags[0] {
        post::FlagKind::Value(p) => p.clone(),
        _ => panic!("expected bool"),
    };
    let mutations: Vec<post::Op> = vec![
        post::Op::Destroy(post::Drop::Value(Place::local(LocalId(u32::MAX)))),
        post::Op::Destroy(post::Drop::Value(place.project(Proj::Field(u32::MAX)))),
        post::Op::Set(post::FlagId(u32::MAX), true),
        post::Op::Destroy(post::Drop::Guard(post::FlagId(u32::MAX), Box::new(post::Drop::Value(place.clone())))),
        post::Op::Destroy(post::Drop::Guard(flag, Box::new(post::Drop::Value(Place::local(LocalId(0)))))),
        post::Op::Destroy(post::Drop::Fields { place: Place::local(place.local), fields: vec![(0, post::Drop::Value(place.clone())), (0, post::Drop::Value(place.clone()))] }),
        post::Op::Destroy(post::Drop::Fields { place: Place::local(place.local), fields: vec![(0, post::Drop::Value(Place::local(place.local)))] }),
        post::Op::Destroy(post::Drop::Variants { place: Place::local(place.local), variants: Vec::new() }),
        post::Op::Plain(StatementKind::Drop(place.clone())),
        post::Op::ClearElement(flag, LocalId(u32::MAX)),
    ];
    for op in mutations {
        p.functions[fi] = good.clone();
        p.functions[fi].blocks[0].stmts.push(post::Statement { op, span });
        let errors = post::verify(p, t);
        assert!(!errors.is_empty(), "verifier accepted malformed drop IR");
    }
    p.functions[fi] = good.clone();
    p.functions[fi].blocks[0].term = Terminator::Goto(BlockId(u32::MAX));
    assert!(post::verify(p, t).iter().any(|e| e.contains("missing bb")));
    // Remove all initialization and put a guard at entry: neither a back-edge
    // nor later flag writes may make this first read initialized.
    p.functions[fi] = good.clone();
    p.functions[fi].blocks[0].stmts.insert(0, post::Statement { op: post::Op::Destroy(post::Drop::Guard(flag, Box::new(post::Drop::Value(place)))), span });
    assert!(post::verify(p, t).iter().any(|e| e.contains("read before initialization")));
    p.functions[fi] = good;
    assert!(post::verify(p, t).is_empty());
}

#[test]
fn conditional_assignment_flags_change_only_on_successful_call_edges() {
    let path = std::path::Path::new("../../tests/drops/pass/conditional.tarn");
    let res = tarn_driver::check(path).unwrap();
    let p = res.drops.unwrap();
    let f = p.functions.iter().find(|f| f.decl.name == "main").unwrap();
    // The call produces a temporary, then Assign initializes the conditional
    // local. Its bit must follow Assign, not the call's incoming edges.
    let flag = post::FlagId(0);
    assert!(
        f.blocks
            .iter()
            .any(|b| b.stmts.windows(2).any(|xs| matches!(&xs[0].op, post::Op::Plain(StatementKind::Assign(..))) && matches!(xs[1].op, post::Op::Set(id, true) if id == flag)))
    );
}

#[test]
fn call_destination_is_initialized_on_a_split_success_edge() {
    let mut res = tarn_driver::check(std::path::Path::new("../../tests/drops/pass/conditional.tarn")).unwrap();
    let p = res.ir.as_mut().unwrap();
    let fi = p.functions.iter().position(|f| f.name == "main").unwrap();
    let constructor = p.functions.iter().find(|f| f.name == "Buffer.new").unwrap().id;
    let f = &mut p.functions[fi];
    let x = LocalId(f.locals.iter().position(|l| l.name.as_deref() == Some("x")).unwrap() as u32);
    let span = f.span;
    f.blocks = vec![
        BasicBlock {
            stmts: vec![ir_stmt(StatementKind::StorageLive(x), span)],
            term: Terminator::Switch { discr: Operand::Copy(Place::local(LocalId(1))), cases: vec![(0, BlockId(2))], otherwise: BlockId(1) },
            term_span: span,
        },
        BasicBlock {
            stmts: Vec::new(),
            term: Terminator::Call {
                callee: Callee::Fn(constructor, Vec::new()),
                args: Vec::new(),
                arg_spans: Vec::new(),
                dest: Place::local(x),
                next: Some(BlockId(3)),
                spawn: false,
            },
            term_span: span,
        },
        BasicBlock { stmts: Vec::new(), term: Terminator::Goto(BlockId(3)), term_span: span },
        BasicBlock { stmts: vec![ir_stmt(StatementKind::Drop(Place::local(x)), span), ir_stmt(StatementKind::StorageDead(x), span)], term: Terminator::Return, term_span: span },
    ];
    let r = res.resolved.as_ref().unwrap();
    let t = res.typed.as_ref().unwrap();
    let (moves, errors) = tarn_ownership::check_moves(p, r, t);
    assert!(errors.is_empty());
    let (_, errors) = tarn_ownership::check_borrows(p, r, t, &moves.failed());
    assert!(errors.is_empty());
    let post = tarn_ownership::elaborate_drops(p, t, &moves).unwrap();
    let f = &post.functions[fi];
    assert_eq!(f.blocks.len(), 5);
    assert!(matches!(f.blocks[1].term, Terminator::Call { next: Some(BlockId(4)), .. }));
    assert!(matches!(f.blocks[4].term, Terminator::Goto(BlockId(3))));
    assert!(matches!(f.blocks[4].stmts[0].op, post::Op::Set(_, true)));
    assert!(f.blocks[1].stmts.is_empty());
    assert!(f.blocks[2].stmts.is_empty());
    for c in [0, 1] {
        let mut m = Machine { p: &post, created: 0, destroyed: HashSet::new(), trace: Vec::new(), steps: 0, aborted: false };
        m.call(FunctionId(fi as u32), vec![Value::Number(c)]);
        assert_eq!(m.created, m.destroyed.len());
        assert_eq!(m.created, c as usize);
    }
}
fn ir_stmt(kind: StatementKind, span: tarn_diagnostics::Span) -> tarn_ir::Statement {
    tarn_ir::Statement { kind, span }
}
