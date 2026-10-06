//! Source async facts before executable frame lowering becomes available.
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
    assert!(result.diagnostics.iter().all(|d| d.code == "E3062"), "{:?}", result.diagnostics.iter().map(|d| (&d.code, &d.message)).collect::<Vec<_>>());
    let typed = result.typed.as_ref().unwrap();
    let resolved = result.resolved.as_ref().unwrap();
    let computation = typed.locals.iter().find(|(id, _)| resolved.symbol(**id).name == "computation").unwrap().1;
    assert_eq!(*computation, Ty::Async(Box::new(Ty::Int(IntTy::I32))));
    assert!(typed.tables[0].expr_types.values().any(|ty| *ty == Ty::Int(IntTy::I32)));
    assert!(result.ir.is_none(), "unsupported state lowering must not run synchronously");
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
fn await_uses_trusted_operation_identity_and_lazy_code_is_not_emitted() {
    let result = check("import \"net\"\nasync fn read(owner &net.Execution, stream &mut net.TcpStream, bytes &mut []u8) Result<usize, net.Error> { return await net.read_operation(owner, stream, bytes) }\nfn main() {}\n", "operation");
    assert!(result.diagnostics.iter().all(|d| d.code == "E3062"));
    assert!(result.drops.is_none());
    let result = check("struct Operation<T> { value T }\nasync fn app() i32 { return await Operation{value: i32(1)} }\nfn main() {}\n", "fake");
    assert!(result.diagnostics.iter().any(|d| d.code == "E3061"));
}
