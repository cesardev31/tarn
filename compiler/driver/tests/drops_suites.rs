//! Snapshots plus a resource-token interpreter: verifies executed paths rather
//! than counting syntactic drops. This is a test oracle, not a Tarn backend.
mod common;
use common::*;
use std::collections::HashSet;
use tarn_ir::{post_drop as post, *};

#[path = "common/drop_machine.rs"]
mod drop_machine;
use drop_machine::{Machine, Value};

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
            let mut m = Machine { p, created: 0, labels: Vec::new(), destroyed: HashSet::new(), trace: Vec::new(), steps: 0, aborted: false };
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
        let mut m = Machine { p: &post, created: 0, labels: Vec::new(), destroyed: HashSet::new(), trace: Vec::new(), steps: 0, aborted: false };
        m.call(FunctionId(fi as u32), vec![Value::Number(c)]);
        assert_eq!(m.created, m.destroyed.len());
        assert_eq!(m.created, c as usize);
    }
}
fn ir_stmt(kind: StatementKind, span: tarn_diagnostics::Span) -> tarn_ir::Statement {
    tarn_ir::Statement { kind, span }
}
