//! Source async facts and their lowering onto verified frames (ADR 0037).
use std::path::PathBuf;
use tarn_types::{IntTy, Ty};

fn check(source: &str, tag: &str) -> tarn_driver::CheckResult {
    let dir = std::env::temp_dir().join(format!("tarn-async-frontend-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry: PathBuf = dir.join("main.tarn");
    std::fs::write(&entry, source).unwrap();
    let result = tarn_driver::check(&entry).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    result
}

#[test]
fn async_call_and_await_have_distinct_types_and_generic_inference() {
    let result = check("async fn identity<T>(value T) T { return value }\nasync fn app() i32 { return await identity(42) }\nfn main() { computation := identity(i32(7)) }\n", "types");
    assert!(!result.has_errors(), "{:?}", result.diagnostics.iter().map(|d| (&d.code, &d.message)).collect::<Vec<_>>());
    let typed = result.typed.as_ref().unwrap();
    let resolved = result.resolved.as_ref().unwrap();
    let computation = typed.locals.iter().find(|(id, _)| resolved.symbol(**id).name == "computation").unwrap().1;
    assert_eq!(*computation, Ty::Async(Box::new(Ty::Int(IntTy::I32))));
    assert!(typed.tables[0].expr_types.values().any(|ty| *ty == Ty::Int(IntTy::I32)));
    // Calling constructs a frame lazily; the body is a separate suspended function.
    let ir = result.ir.as_ref().unwrap();
    let main = ir.functions.iter().find(|f| f.name == "main").unwrap();
    let constructs = main.blocks.iter().flat_map(|b| &b.stmts).any(|s| matches!(&s.kind, tarn_ir::StatementKind::Assign(_, tarn_ir::Rvalue::Aggregate(tarn_ir::Aggregate::AsyncFrame(..), _))));
    assert!(constructs && main.blocks.iter().all(|b| !matches!(b.term, tarn_ir::Terminator::Call { .. })), "async call must not execute its body");
    let app = ir.functions.iter().find(|f| f.name == "app").unwrap();
    assert!(app.asynchronous.is_some() && matches!(app.blocks[0].term, tarn_ir::Terminator::Suspend { .. }), "async bodies start suspended");
    let drops = result.drops.as_ref().unwrap();
    let frame = drops.functions.iter().find(|f| f.decl.name == "app").and_then(|f| f.decl.asynchronous.as_ref()).and_then(|a| a.frame.as_ref()).unwrap();
    assert_eq!(frame.done, 2, "initial state plus one await");
}

#[test]
fn await_requires_async_context_and_does_not_leak_into_ordinary_closures() {
    let result = check("fn main() { value := await 42 }\n", "outside");
    assert!(result.diagnostics.iter().any(|d| d.code == "E3060"));
    assert!(result.diagnostics.iter().all(|d| d.code != "E3061"));
    let result = check("async fn app() { f := fn() { await 42 }\n await 42 }\nfn main() {}\n", "closure");
    assert_eq!(result.diagnostics.iter().filter(|d| d.code == "E3060").count(), 1);
    assert_eq!(result.diagnostics.iter().filter(|d| d.code == "E3061").count(), 1);
}

#[test]
fn await_uses_trusted_operation_identity_and_stores_the_pending_child() {
    let result = check("import \"net\"\nasync fn read(owner &net.Execution, stream &mut net.TcpStream, bytes &mut []u8) Result<usize, net.Error> { return await net.read_operation(owner, stream, bytes) }\nfn main() {}\n", "operation");
    assert!(!result.has_errors(), "{:?}", result.diagnostics.iter().map(|d| (&d.code, &d.message)).collect::<Vec<_>>());
    let drops = result.drops.as_ref().unwrap();
    let read = drops.functions.iter().find(|f| f.decl.name == "read").unwrap();
    let frame = read.decl.asynchronous.as_ref().and_then(|a| a.frame.as_ref()).unwrap();
    // Reference parameters and the pending child Operation are frame state.
    assert_eq!(frame.params.len(), 3);
    assert!(frame.stored.iter().any(|l| matches!(&read.decl.local(*l).ty, Ty::Adt(..)) && read.decl.local(*l).kind == tarn_ir::LocalKind::Temp));
    let result = check("struct Operation<T> { value T }\nasync fn app() i32 { return await Operation{value: i32(1)} }\nfn main() {}\n", "fake");
    assert!(result.diagnostics.iter().any(|d| d.code == "E3061"));
}

#[test]
fn native_replacement_cannot_hide_new_payload_loans() {
    let result = check("struct Counter { value i32 }\nfn main() { outside := Counter{value: 1}\n mutex := Mutex.new(&outside)\n { local := Counter{value: 42}\n var guard = mutex.lock()\n guard.replace(&local) }\n var guard = mutex.lock()\n pointer := guard.read()\n print(pointer.value) }\n", "stored-local");
    assert!(result.diagnostics.iter().any(|d| d.code == "E3051"));
    assert!(result.drops.is_none());
    let result = check("struct Counter { value i32 }\nfn store(mutex &Mutex<&Counter>, value &Counter) { var guard = mutex.lock()\n guard.replace(value) }\nfn main() {}\n", "stored-parameter");
    assert!(result.diagnostics.iter().any(|d| d.code == "E3051"));
    let result = check("fn main() { mutex := Mutex.new(\"first\")\n var guard = mutex.lock()\n old := guard.replace(\"second\")\n print(&old) }\n", "owned-replacement");
    assert!(!result.has_errors(), "{:?}", result.diagnostics.iter().map(|d| (&d.code, &d.message)).collect::<Vec<_>>());
}

#[test]
fn frame_verifier_rejects_corrupted_state_machines() {
    let source = "import \"net\"\nasync fn value() i32 { return 1 }\nasync fn app(name string) i32 {\n    a := await value()\n    print(&name)\n    b := await value()\n    return a + b\n}\nfn main() {}\n";
    let result = check(source, "frame-mutations");
    assert!(!result.has_errors());
    let typed = result.typed.as_ref().unwrap();
    let original = result.drops.unwrap();
    assert!(tarn_ir::post_drop::verify(&original, typed).is_empty());
    let app = original.functions.iter().position(|f| f.decl.name == "app").unwrap();
    let frame = move |p: &mut tarn_ir::post_drop::Program| p.functions[app].decl.asynchronous.as_mut().unwrap().frame.as_mut().unwrap().clone();
    type Mutation = Box<dyn Fn(&mut tarn_ir::post_drop::Program)>;
    let mutations: Vec<(&str, Mutation)> = vec![
        ("missing state transition", Box::new(move |p| {
            if let tarn_ir::Terminator::Switch { cases, .. } = &mut p.functions[app].blocks[0].term { cases.pop(); }
        })),
        ("wrong suspension state", Box::new(move |p| {
            let state = frame(p).state;
            let done = frame(p).done;
            for b in &mut p.functions[app].blocks {
                for s in &mut b.stmts {
                    if let tarn_ir::post_drop::Op::Plain(tarn_ir::StatementKind::Assign(place, tarn_ir::Rvalue::Use(tarn_ir::Operand::Const(tarn_ir::Const::Int(v, _))))) = &mut s.op
                        && place.local == state { *v = done as i128 + 7; return; }
                }
            }
        })),
        ("lost construction parameter", Box::new(move |p| {
            let f = p.functions[app].decl.asynchronous.as_mut().unwrap().frame.as_mut().unwrap();
            let param = f.params[0];
            f.stored.retain(|l| *l != param);
        })),
        ("stored per-poll waker", Box::new(move |p| {
            let a = p.functions[app].decl.asynchronous.as_mut().unwrap();
            let w = a.waker;
            a.frame.as_mut().unwrap().stored.push(w);
        })),
        ("duplicate frame slot", Box::new(move |p| {
            let f = p.functions[app].decl.asynchronous.as_mut().unwrap().frame.as_mut().unwrap();
            let first = f.stored[0];
            f.stored.push(first);
        })),
        ("suspension survives lowering", Box::new(move |p| {
            let n = p.functions[app].blocks.len() as u32;
            p.functions[app].blocks[1].term = tarn_ir::Terminator::Suspend { resume: tarn_ir::BlockId(n - 1), abandon: tarn_ir::BlockId(n - 1) };
        })),
        ("construction arity", Box::new(move |p| {
            for f in &mut p.functions {
                for b in &mut f.blocks {
                    for s in &mut b.stmts {
                        if let tarn_ir::post_drop::Op::Plain(tarn_ir::StatementKind::Assign(_, tarn_ir::Rvalue::Aggregate(tarn_ir::Aggregate::AsyncFrame(..), ops))) = &mut s.op { ops.push(tarn_ir::Operand::Const(tarn_ir::Const::Unit)); }
                    }
                }
            }
        })),
    ];
    for (name, mutate) in mutations {
        let mut program = tarn_driver::check(&{
            let dir = std::env::temp_dir().join(format!("tarn-async-frontend-{}-mut", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let entry = dir.join("main.tarn");
            std::fs::write(&entry, source).unwrap();
            entry
        }).unwrap().drops.unwrap();
        mutate(&mut program);
        assert!(!tarn_ir::post_drop::verify(&program, typed).is_empty(), "verifier accepted: {name}");
    }
}
