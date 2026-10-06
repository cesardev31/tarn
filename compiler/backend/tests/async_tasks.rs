//! Cooperative async tasks (Phase 14A, ADR 0038): spawn, typed join, drop
//! policy, many tasks, fairness, executor identity and a concurrent server.
use std::{path::{Path, PathBuf}, process::{Command, Output}, collections::HashSet};
fn checked(src: &str, tag: &str) -> (PathBuf, tarn_driver::CheckResult) {
    let dir = std::env::temp_dir().join(format!("tarn-tasks-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn"); std::fs::write(&file, src).unwrap();
    (dir, tarn_driver::check(&file).unwrap())
}
fn compile(src: &str, tag: &str) -> PathBuf {
    let (dir, result) = checked(src, tag);
    assert!(!result.has_errors(), "{}", result.diagnostics.iter().map(|d| d.render(&result.program.sources)).collect::<String>());
    let exe = dir.join("program");
    tarn_backend::build(result.drops.as_ref().unwrap(), result.typed.as_ref().unwrap(), &exe).unwrap(); exe
}
fn run(exe: &Path) -> Output {
    let out = Command::new("timeout").arg("30s").arg(exe).env("TARN_TRACE_NET", "1").env("TARN_TRACE_DROPS", "1").output().unwrap();
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
    out
}
fn stdout(o: &Output) -> Vec<String> { String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect() }
fn drops(o: &Output, printed: &[&str]) -> Vec<String> {
    String::from_utf8_lossy(&o.stderr).lines().filter_map(|l| l.strip_prefix("drop:")).filter(|d| !printed.contains(d)).map(str::to_string).collect()
}
fn balanced(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let mut live = HashSet::new();
    for line in String::from_utf8_lossy(&o.stderr).lines().filter(|s| s.starts_with("net:")) {
        let parts: Vec<_> = line.split(':').collect(); let fd = parts[2].parse::<i32>().unwrap();
        if parts[1] == "open" { assert!(live.insert(fd)); } else { assert!(live.remove(&fd)); }
    }
    assert!(live.is_empty(), "fd leak: {live:?}");
}

#[test]
fn spawned_tasks_interleave_fairly_and_join_owned_results_once() {
    let out = run(&compile(r#"import "net"
fn pause(owner &net.Execution) net.Operation<void> {
    var polled = false
    return net.Operation.new(owner, move fn(waker &net.Waker) net.Progress<void> {
        if polled { return net.Progress.Ready(()) }
        polled = true
        waker.wake()
        return net.Progress.Pending
    })
}
async fn work(owner &net.Execution, name string, n i32) string {
    for i in 0..n {
        print(&name)
        await pause(owner)
    }
    return name
}
async fn parent(owner &net.Execution) i32 {
    a := owner.spawn_async(work(owner, "a", 3))
    b := owner.spawn_async(work(owner, "b", 2))
    first := await a.join()
    second := await b.join()
    print(&first)
    print(&second)
    return 7
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var app = net.Operation.new(&execution, parent(&execution))
    print(try execution.block_on(&mut app))
    return Ok(())
}
"#, "fair"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["a", "b", "a", "b", "a", "a", "b", "7"]);
    assert_eq!(drops(&out, &[]), ["b", "a"]);
}

#[test]
fn dropped_handles_destroy_results_or_abandon_pending_tasks_and_hundreds_complete() {
    let out = run(&compile(r#"import "net"
fn pause(owner &net.Execution) net.Operation<void> {
    var polled = false
    return net.Operation.new(owner, move fn(waker &net.Waker) net.Progress<void> {
        if polled { return net.Progress.Ready(()) }
        polled = true
        waker.wake()
        return net.Progress.Pending
    })
}
async fn slow(owner &net.Execution, name string) string {
    await pause(owner)
    await pause(owner)
    return name
}
async fn fast(name string) string { return name }
async fn counter(owner &net.Execution, n i32) i32 {
    await pause(owner)
    return n
}
async fn parent(owner &net.Execution) i32 {
    pending := owner.spawn_async(slow(owner, "pending-dropped"))
    done := owner.spawn_async(fast("completed-dropped"))
    await pause(owner)
    await pause(owner)
    print("dropping")
    var many = Vec.new()
    for i in 0..500 { many.push(owner.spawn_async(counter(owner, i))) }
    var sum = 0
    for {
        match many.pop() {
            Some(t) => { sum = sum + await t.join() }
            None => { return sum }
        }
    }
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var app = net.Operation.new(&execution, parent(&execution))
    print(try execution.block_on(&mut app))
    return Ok(())
}
"#, "drop-many"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["dropping", "124750"]);
    // Completed unjoined result destroyed by its handle; pending task abandoned
    // through its frame's verified destruction. Each exactly once.
    assert_eq!(drops(&out, &["dropping"]), ["completed-dropped", "pending-dropped"]);
}

#[test]
fn async_server_serves_connections_concurrently_without_threads_per_connection() {
    let out = run(&compile(r#"import "net"
async fn handle(stream net.TcpStream) Result<usize, net.Error> {
    var conn = stream
    try conn.set_nonblocking(true)
    var bytes = [16]u8{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    var total = usize(0)
    for {
        count := try await conn.read_async(&mut bytes)
        if count == usize(0) { return Ok(total) }
        try await conn.write_all_async(&bytes[0..count])
        total = total + count
    }
}
async fn serve(owner &net.Execution, listener &mut net.TcpListener, clients i32) Result<usize, net.Error> {
    var tasks = Vec.new()
    for i in 0..clients {
        stream := try await listener.accept_async()
        tasks.push(owner.spawn_async(handle(stream)))
    }
    var total = usize(0)
    for {
        match tasks.pop() {
            Some(task) => { total = total + try await task.join() }
            None => { return Ok(total) }
        }
    }
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var listener = try net.TcpListener.bind(&"127.0.0.1:0")
    try listener.set_nonblocking(true)
    address := try listener.local_addr()
    clients := spawn move fn() Result<void, net.Error> {
        // First client stays connected while the others are served.
        var first = try net.TcpStream.connect_addr(address)
        try first.write_all(&[2]u8{1, 2})
        var reply = [2]u8{0, 0}
        var got = usize(0)
        for got != usize(2) { got = got + try first.read(&mut reply[got..]) }
        for i in 0..2 {
            var other = try net.TcpStream.connect_addr(address)
            try other.write_all(&[3]u8{7, 7, 7})
            var echo = [3]u8{0, 0, 0}
            var n = usize(0)
            for n != usize(3) { n = n + try other.read(&mut echo[n..]) }
        }
        try first.write_all(&[1]u8{9})
        var last = [1]u8{0}
        n := try first.read(&mut last)
        print(last[0])
        return Ok(())
    }
    var app = net.Operation.new(&execution, serve(&execution, &mut listener, 3))
    total := try try execution.block_on(&mut app)
    try clients.join()
    print(total)
    return Ok(())
}
"#, "server"));
    balanced(&out);
    // The first connection stayed open while two others were fully served.
    assert_eq!(stdout(&out), ["9", "9"]);
}

#[test]
fn tasks_outliving_block_on_are_abandoned_structurally() {
    let out = run(&compile(r#"import "net"
fn forever(owner &net.Execution) net.Operation<void> {
    var turns = 0
    return net.Operation.new(owner, move fn(waker &net.Waker) net.Progress<void> {
        turns = turns + 1
        waker.wake()
        return net.Progress.Pending
    })
}
async fn background(owner &net.Execution, name string) {
    await forever(owner)
}
async fn app(owner &net.Execution) i32 {
    owner.spawn_async(background(owner, "abandoned-at-exit"))
    return 3
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var op = net.Operation.new(&execution, app(&execution))
    print(try execution.block_on(&mut op))
    print("after")
    return Ok(())
}
"#, "structured"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["3", "after"]);
    assert_eq!(drops(&out, &["after"]), ["abandoned-at-exit"]);
}

#[test]
fn joining_from_another_execution_aborts() {
    let out = run(&compile(r#"import "net"
async fn seven() i32 { return 7 }
async fn waiter(task net.AsyncTask<i32>) i32 { return await task.join() }
fn main() Result<void, net.Error> {
    first := try net.Execution.new()
    second := try net.Execution.new()
    task := first.spawn_async(seven())
    var op = net.Operation.new(&second, waiter(task))
    print(try second.block_on(&mut op))
    return Ok(())
}
"#, "wrong-owner"));
    assert!(!out.status.success());
}

#[test]
fn task_handles_are_owned_and_borrowing_tasks_are_rejected() {
    for (tag, source, code) in [
        ("double-join", "import \"net\"\nasync fn one() i32 { return 1 }\nasync fn f(owner &net.Execution) i32 {\n t := owner.spawn_async(one())\n a := await t.join()\n b := await t.join()\n return a + b\n}\nfn main() {}\n", "E4001"),
        ("moved-handle", "import \"net\"\nasync fn one() i32 { return 1 }\nasync fn f(owner &net.Execution) i32 {\n t := owner.spawn_async(one())\n moved := t\n return await t.join()\n}\nfn main() {}\n", "E4001"),
        ("borrowing-task", "import \"net\"\nasync fn uses(value &i32) i32 { return 1 }\nasync fn f(owner &net.Execution) i32 {\n x := 5\n t := owner.spawn_async(uses(&x))\n return await t.join()\n}\nfn main() {}\n", "E4209"),
    ] {
        let (dir, result) = checked(source, tag);
        std::fs::remove_dir_all(dir).unwrap();
        assert!(result.diagnostics.iter().any(|d| d.code == code), "{tag}: {:?}", result.diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>());
    }
}
