//! The fundamental memory-safety cases of 6A + 6B, run together through the
//! whole pipeline. Unsafe programs must be rejected with the given code; safe
//! ones must be accepted. This table is the contract: the ownership model is
//! not considered complete unless every row holds.

const PRELUDE: &str = "struct Buffer {\n    data string\n}\n\nfn Buffer.new() Buffer {\n    return Buffer{data: \"\"}\n}\n\nfn read(b &Buffer) {\n}\n\nfn write(b &mut Buffer) {\n}\n\nfn consume(b Buffer) {\n}\n\n";

const CASES: &[(&str, Option<&str>, &str)] = &[
    ("suspended read buffer held until destruction", Some("E4102"), "import \"net\"\nfn bad(owner &net.Execution, stream &mut net.TcpStream) { var bytes = [2]u8{0, 0}\n var op = net.read_operation(owner, stream, &mut bytes)\n op.poll()\n bytes[0] = 1 }"),
    ("suspended read buffer released by consuming finish", None, "import \"net\"\nfn good(owner &net.Execution, stream &mut net.TcpStream) { var bytes = [2]u8{0, 0}\n var op = net.read_operation(owner, stream, &mut bytes)\n op.poll()\n op.finish()\n bytes[0] = 1 }"),
    ("suspended write_all retains source loan", Some("E4102"), "import \"net\"\nfn bad(owner &net.Execution, stream &mut net.TcpStream) { var bytes = [2]u8{0, 0}\n var op = net.write_all_operation(owner, stream, &bytes)\n op.poll()\n bytes[0] = 1 }"),
    ("execution cannot die before its wake holder", Some("E4103"), "import \"net\"\nfn bad(owner net.Execution, stream &mut net.TcpStream) { var bytes = [2]u8{0, 0}\n op := net.read_operation(&owner, stream, &mut bytes)\n moved := owner }"),
    ("poll double close", Some("E4001"), "import \"net\"\nfn bad(poll net.Poll) { poll.close()\n poll.close() }"),
    ("poll event buffer conflicting alias", Some("E4101"), "import \"net\"\nfn bad(poll &mut net.Poll) { var events = [1]net.Event{net.Event.empty()}\n loan := &events\n poll.wait(&mut events, 0)\n print(loan[0].readable) }"),
    ("poll registration retains no socket loan", None, "import \"net\"\nfn good(poll &mut net.Poll, socket net.UdpSocket) { token := poll.register_udp(&socket, net.Interest.Readable)\n socket.close() }") ,
    ("poll event buffer released after wait", None, "import \"net\"\nfn good(poll &mut net.Poll) { var events = [1]net.Event{net.Event.empty()}\n poll.wait(&mut events, 0)\n events[0] = net.Event.empty() }"),
    ("socket use after move", Some("E4001"), "import \"net\"\nfn take(value net.TcpStream) {}\nfn bad(conn net.TcpStream) { other := conn\n conn.local_addr() }"),
    ("socket double close", Some("E4001"), "import \"net\"\nfn bad(conn net.TcpStream) { conn.close()\n conn.close() }"),
    ("socket original after spawn", Some("E4001"), "import \"net\"\nfn bad(conn net.TcpStream) { task := spawn move fn() { conn.close() }\n conn.local_addr()\n task.join() }"),
    ("socket move while borrowed", Some("E4103"), "import \"net\"\nfn bad(conn net.TcpStream) { loan := &conn\n other := conn\n loan.local_addr() }"),
    ("socket overwrite while borrowed", Some("E4102"), "import \"net\"\nfn bad(conn net.TcpStream, other net.TcpStream) { var socket = conn\n loan := &socket\n socket = other\n loan.local_addr() }"),
    ("read buffer conflicting alias", Some("E4101"), "import \"net\"\nfn bad(conn &mut net.TcpStream) { var buffer = [2]u8{0, 0}\n loan := &buffer\n conn.read(&mut buffer)\n print(loan[0]) }"),
    ("network buffer reference escape", Some("E4201"), "import \"net\"\nfn bad(conn &mut net.TcpStream) &[]u8 { var buffer = [2]u8{0, 0}\n conn.read(&mut buffer)\n return &buffer[..] }"),
    ("socket raw descriptor inaccessible", Some("E3014"), "import \"net\"\nfn bad(conn &net.TcpStream) { print(conn.fd) }"),
    ("socket cannot be fabricated", Some("E3014"), "import \"net\"\nfn main() { stream := net.TcpStream{fd: 3} }"),
    ("network intrinsic private", Some("E2006"), "import \"net\"\nfn main() { net._accept(3) }"),
    ("socket shared across tasks rejected", Some("E3047"), "import \"net\"\nfn bad(conn &net.TcpStream) { scope { spawn move fn() { conn.local_addr() } } }"),
    ("socket task transfer and return", None, "import \"net\"\nfn bad(conn net.TcpStream) { task := spawn move fn() net.TcpStream { return conn }\n result := task.join()\n result.close() }"),
    ("socket mutable scoped transfer", None, "import \"net\"\nfn bad(conn &mut net.TcpStream) { scope { task := spawn move fn() { conn.shutdown(net.Shutdown.Both) }\n task.join() } }"),
    ("socket serialized with mutex", None, "import \"net\"\nfn bad(conn net.TcpStream) { mutex := Mutex.new(conn)\n scope { spawn fn() { var guard = mutex.lock()\n guard.value().local_addr() } } }"),
    ("network slice call coercions", None, "import \"net\"\nfn bad(conn &mut net.TcpStream) { var buffer = [2]u8{0, 0}\n conn.read(&mut buffer)\n conn.write_all(&buffer) }"),
    ("unused guard destruction retains mutex loan", Some("E4103"), "fn main() { m := Mutex.new(i64(0))\n g := m.lock()\n moved := m }"),
    ("returned guard lifetime follows borrowed owner", Some("E4105"), "fn lock(m &Mutex<i64>) MutexGuard<i64> { return m.lock() }\nfn main() { var g: MutexGuard<i64>\n { m := Mutex.new(i64(0))\n g = lock(&m) }\n print(g.read()) }"),
    ("guard aggregate retains mutex loan", Some("E4103"), "struct Box<T> { value T }\nfn main() { m := Mutex.new(i64(0))\n b := Box{value: m.lock()}\n moved := m }"),
    ("mutex Share requires Transfer, not Share", None, "struct Counter { value i64 }\nfn main() { var c = Counter{value: 0}\n { m := Mutex.new(&mut c)\n scope { spawn fn() { var g = m.lock()\n g.value().value = 1 }\n spawn fn() { var g = m.lock()\n g.value().value = 2 } } } }"),

    ("mutex input is moved", Some("E4001"), "fn main() { value := Buffer.new()\n mutex := Mutex.new(value)\n read(&value) }"),
    ("guard outlives mutex", Some("E4105"), "fn main() { var guard: MutexGuard<i64>\n { mutex := Mutex.new(i64(0))\n guard = mutex.lock() }\n print(guard.read()) }"),
    ("mutex move while guard live", Some("E4103"), "fn main() { mutex := Mutex.new(i64(0))\n guard := mutex.lock()\n moved := mutex\n print(guard.read()) }"),
    ("mutex overwrite while guard live", Some("E4102"), "fn main() { var mutex = Mutex.new(i64(0))\n guard := mutex.lock()\n mutex = Mutex.new(i64(1))\n print(guard.read()) }"),
    ("payload reference after guard drop", Some("E4105"), "fn main() { mutex := Mutex.new(Buffer.new())\n var ref: &mut Buffer\n { var guard = mutex.lock()\n ref = guard.value() }\n write(ref) }"),
    ("payload reference escapes guard", Some("E4201"), "fn escape(mutex &Mutex<Buffer>) &mut Buffer { var guard = mutex.lock()\n return guard.value() }\nfn main() {}"),
    ("guard cannot transfer", Some("E3047"), "fn main() { mutex := Mutex.new(i64(0))\n guard := mutex.lock()\n scope { spawn move fn() { guard.read() } } }"),
    ("guard cannot share", Some("E3047"), "fn main() { mutex := Mutex.new(i64(0))\n guard := mutex.lock()\n scope { spawn fn() { guard.read() } } }"),
    ("guard payload mutable aliases", Some("E4101"), "fn main() { mutex := Mutex.new(Buffer.new())\n var guard = mutex.lock()\n a := guard.value()\n b := guard.value()\n write(a)\n write(b) }"),
    ("payload reference forbids guard move", Some("E4103"), "fn main() { mutex := Mutex.new(Buffer.new())\n var guard = mutex.lock()\n a := guard.value()\n moved := guard\n write(a) }"),
    ("non Transfer mutex payload across workers", Some("E3047"), "fn worker<T>(value T) { mutex := Mutex.new(value)\n scope { spawn fn() { mutex.lock() } } }\nfn main() {}"),
    ("Copy read rejects owned payload", Some("E3022"), "fn main() { mutex := Mutex.new(Buffer.new())\n guard := mutex.lock()\n guard.read() }"),
    ("atomic bool arithmetic rejected", Some("E3005"), "fn main() { a := AtomicBool.new(false)\n a.fetch_add(1) }"),
    ("atomic private mutation rejected", Some("E3014"), "fn main() { var a = AtomicI32.new(0)\n a.native = 42 }"),
    ("mutex private payload rejected", Some("E3014"), "fn main() { var m = Mutex.new(i64(0))\n g := m.lock()\n m.payload = 42 }"),
    ("guard private native storage rejected", Some("E3014"), "fn main() { m := Mutex.new(i64(0))\n var g = m.lock()\n g.native = 0 }"),
    ("shared mutex workers", None, "fn main() { mutex := Mutex.new(i64(0))\n scope { spawn fn() { var g = mutex.lock()\n g.replace(1) }\n spawn fn() { var g = mutex.lock()\n g.replace(2) } } }"),
    ("guard exclusive payload mutation", None, "fn main() { mutex := Mutex.new(Buffer.new())\n var guard = mutex.lock()\n write(guard.value()) }"),
    ("shared atomic counter", None, "fn main() { a := AtomicI64.new(0)\n scope { spawn fn() { a.fetch_add(1) }\n spawn fn() { a.fetch_add(1) } }\n print(a.load()) }"),
    ("guard moved locally", None, "fn main() { m := Mutex.new(i64(0))\n g := m.lock()\n moved := g\n print(moved.read()) }"),
    ("guard returned from borrowed mutex", None, "fn lock(m &Mutex<i64>) MutexGuard<i64> { return m.lock() }\nfn main() { m := Mutex.new(i64(0))\n g := lock(&m)\n print(g.read()) }"),
    ("guard retained in generic aggregate", None, "struct Box<T> { value T }\nfn main() { m := Mutex.new(i64(0))\n b := Box{value: m.lock()}\n print(b.value.read()) }"),

    ("scoped mutable parent access before completion", Some("E4104"), "fn main() { scope { var value: i64 = 0\n spawn fn() { value = value + 1 }\n print(value) } }"),
    ("scoped concurrent mutable aliases", Some("E4101"), "fn main() { scope { var value: i64 = 0\n spawn fn() { value = value + 1 }\n spawn fn() { value = value + 1 } } }"),
    ("scoped parent assignment while worker active", Some("E4102"), "fn main() { scope { var value: i64 = 0\n spawn fn() { value = value + 1 }\n value = 42 } }"),
    ("scoped handle escape", Some("E4208"), "fn escape() Task<i32> { scope { task := spawn move fn() i32 { return 42 }\n return task } }\nfn main() {}"),
    ("shared cross-thread reference requires evidence", Some("E3047"), "interface Observe { fn read(&self) i32 }\nfn inspect(value &any Observe) { scope { task := spawn move fn() i32 { return value.read() }\n task.join() } }\nfn main() {}"),
    ("multiple shared scoped workers", None, "fn main() { scope { value := Buffer.new()\n spawn fn() { read(&value) }\n spawn fn() { read(&value) } } }"),
    ("access after scoped explicit join", None, "fn main() { scope { var value: i64 = 0\n task := spawn fn() { value = value + 1 }\n task.join()\n print(value) } }"),
    ("scoped mutable disjoint fields", None, "struct Pair { first Buffer\n second Buffer }\nfn main() { var pair = Pair{first: Buffer.new(), second: Buffer.new()}\n scope { first := &mut pair.first\n second := &mut pair.second\n spawn move fn() { write(first) }\n spawn move fn() { write(second) } } }"),
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
    let lock = t.decls.fns.values().find(|f| f.self_ty.as_ref().is_some_and(|ty| matches!(ty, tarn_types::Ty::Adt(s, _) if Some(*s) == t.decls.mutex)) && f.receiver.is_some()).unwrap();
    assert_eq!(lock.contract.parameters, [P::SharedBorrow]);
    assert_eq!(lock.contract.result, R::Borrowed(vec![0]));
    let value = t.decls.fns.values().find(|f| f.self_ty.as_ref().is_some_and(|ty| matches!(ty, tarn_types::Ty::Adt(s, _) if Some(*s) == t.decls.mutex_guard)) && matches!(&f.ret, tarn_types::Ty::Ref(true, _))).unwrap();
    assert_eq!(value.contract.parameters, [P::MutableBorrow]);
    assert_eq!(value.contract.result, R::Borrowed(vec![0]));
    std::fs::remove_dir_all(dir).unwrap();
}
