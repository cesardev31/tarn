//! The fundamental memory-safety cases of 6A + 6B, run together through the
//! whole pipeline. Unsafe programs must be rejected with the given code; safe
//! ones must be accepted. This table is the contract: the ownership model is
//! not considered complete unless every row holds.

const PRELUDE: &str = "struct Buffer {\n    data string\n}\n\nfn Buffer.new() Buffer {\n    return Buffer{data: \"\"}\n}\n\nfn read(b &Buffer) {\n}\n\nfn write(b &mut Buffer) {\n}\n\nfn consume(b Buffer) {\n}\n\n";

const CASES: &[(&str, Option<&str>, &str)] = &[
    ("task capture requires transfer bound", Some("E3047"), "fn launch<T>(value T) { t := spawn move fn() { value }\n t.join() }\nfn main() {}"),
    ("share bound does not grant result transfer", Some("E3047"), "fn launch<T: Share>(value T) { t := spawn move fn() T { return value }\n t.join() }\nfn main() {}"),
    ("owned structural transfer capture", None, "fn main() { value := Buffer.new()\n task := spawn move fn() Buffer { return value }\n result := task.join()\n read(&result) }"),
    ("task double join", Some("E4001"), "fn main() { t := spawn move fn() i64 { return 42 }\n t.join()\n t.join() }") ,
    ("task handle use after move", Some("E4001"), "fn main() { t := spawn move fn() i64 { return 42 }\n u := t\n t.join()\n u.join() }") ,
    ("task capture use after move", Some("E4001"), "fn main() { s := \"owned\"\n t := spawn move fn() string { return s }\n print(s)\n t.join() }") ,
    ("unscoped borrowed task closure", Some("E4206"), "fn main() { x := 42\n t := spawn fn() i64 { return x }\n t.join() }") ,
    ("owned task containing local loan", Some("E4206"), "fn main() { x: i64 := 42\n r := &x\n t := spawn move fn() i64 { return r.abs() }\n t.join() }") ,
    ("task result cannot borrow its environment", Some("E4201"), "fn main() { x := 42\n t := spawn move fn() &i64 { return &x }\n t.join() }") ,
    ("task handle ownership transfer", None, "fn main() { t := spawn move fn() i64 { return 42 }\n u := t\n print(u.join()) }") ,
    ("opaque std return rejected", Some("E3040"), "import \"fs\"\nfn main() {\n    x := fs.open(\"file\")\n}"),
    ("opaque std type rejected", Some("E3040"), "import \"fs\"\nfn observe(x &fs.File) {}"),
    ("opaque prelude error rejected", Some("E3040"), "fn observe(x &Error) {}"),
    (
        "modeled result loan remains live",
        Some("E4102"),
        "fn view(x &Buffer) &Buffer { return x }\nfn main() {\n    var b = Buffer.new()\n    r := view(&b)\n    b = Buffer.new()\n    read(r)\n}",
    ),
    ("use after move", Some("E4001"), "fn main() {\n    a := Buffer.new()\n    b := a\n    read(&a)\n}"),
    ("double move", Some("E4001"), "fn main() {\n    a := Buffer.new()\n    consume(a)\n    consume(a)\n}"),
    ("possibly moved at join", Some("E4001"), "fn main(c bool) {\n    x := Buffer.new()\n    if c {\n        consume(x)\n    }\n    read(&x)\n}"),
    ("possibly uninitialized", Some("E4005"), "fn main(c bool) {\n    var x: Buffer\n    if c {\n        x = Buffer.new()\n    }\n    read(&x)\n}"),
    ("move while borrowed", Some("E4103"), "fn main() {\n    a := Buffer.new()\n    r := &a\n    consume(a)\n    read(r)\n}"),
    ("assign while borrowed", Some("E4102"), "fn main() {\n    var a = Buffer.new()\n    r := &a\n    a = Buffer.new()\n    read(r)\n}"),
    ("shared + mutable", Some("E4101"), "fn main() {\n    var a = Buffer.new()\n    r := &a\n    write(&mut a)\n    read(r)\n}"),
    ("mutable + mutable", Some("E4101"), "fn main() {\n    var a = Buffer.new()\n    r := &mut a\n    write(&mut a)\n    write(r)\n}"),
    ("dangling after scope", Some("E4105"), "fn main() {\n    var r = &Buffer.new()\n    {\n        b := Buffer.new()\n        r = &b\n    }\n    read(r)\n}"),
    ("return reference to local", Some("E4201"), "fn bad() &Buffer {\n    b := Buffer.new()\n    return &b\n}"),
    ("move out of reference", Some("E4003"), "fn take(b &Buffer) string {\n    return b.data\n}"),
    ("closure escapes borrow", Some("E4205"), "fn make() fn(i64) i64 {\n    k := 1\n    return fn(x) i64 { return x + k }\n}"),
    ("shared + shared", None, "fn main() {\n    a := Buffer.new()\n    r := &a\n    s := &a\n    read(r)\n    read(s)\n}"),
    ("non-lexical end of borrow", None, "fn main() {\n    var a = Buffer.new()\n    r := &a\n    read(r)\n    write(&mut a)\n}"),
    ("reborrow does not move", None, "fn f(x &mut Buffer) {\n    write(x)\n    write(x)\n}"),
    ("return borrow of input", None, "fn first(x &Buffer) &Buffer {\n    return x\n}"),
    ("reinitialize after move", None, "fn main() {\n    var a = Buffer.new()\n    consume(a)\n    a = Buffer.new()\n    read(&a)\n}"),
];

#[test]
fn fundamental_cases_hold_together() {
    let dir = std::env::temp_dir().join(format!("tarn-safety-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for (i, (name, expected, body)) in CASES.iter().enumerate() {
        let file = dir.join(format!("case{i}.tarn"));
        std::fs::write(&file, format!("{PRELUDE}{body}\n")).unwrap();
        let res = tarn_driver::check(&file).unwrap();
        let codes: Vec<&str> = res.diagnostics.iter().filter(|d| d.severity == tarn_diagnostics::Severity::Error).map(|d| d.code).collect();
        match expected {
            Some(code) if !codes.contains(code) => failures.push(format!("{name}: expected {code}, got {codes:?}")),
            None if !codes.is_empty() => failures.push(format!("{name}: expected no errors, got {codes:?}")),
            None => {
                let post = res.drops.as_ref().expect("accepted program must reach drop elaboration");
                assert!(tarn_ir::post_drop::verify(post, res.typed.as_ref().unwrap()).is_empty());
            }
            _ => {}
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn semantic_contracts_expose_passing_and_result_modes() {
    use tarn_types::{PassingMode as P, ResultContract as R};
    let dir = std::env::temp_dir().join(format!("tarn-contract-metadata-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.tarn");
    std::fs::write(&path, "extern \"C\" fn select(a &string, b &string, choose bool) &string borrows(a, b)\nextern \"C\" fn consume(x string, y &mut i64) string\nfn main() {}\n")
        .unwrap();
    let res = tarn_driver::check(&path).unwrap();
    assert!(!res.has_errors(), "{:?}", res.diagnostics);
    let r = res.resolved.as_ref().unwrap();
    let t = res.typed.as_ref().unwrap();
    let select = t.decls.fns.iter().find(|(s, _)| r.symbol(**s).name == "select").unwrap().1;
    assert_eq!(select.contract.parameters, [P::SharedBorrow, P::SharedBorrow, P::Copy]);
    assert_eq!(select.contract.result, R::Borrowed(vec![0, 1]));
    let consume = t.decls.fns.iter().find(|(s, _)| r.symbol(**s).name == "consume").unwrap().1;
    assert_eq!(consume.contract.parameters, [P::Move, P::MutableBorrow]);
    assert_eq!(consume.contract.result, R::Owned);
    let sqrt = t.decls.fns.values().find(|f| f.self_ty == Some(tarn_types::Ty::Float(tarn_types::FloatTy::F32))).unwrap();
    assert_eq!(sqrt.contract.parameters, [P::Copy]);
    std::fs::remove_dir_all(dir).unwrap();
}
