//! Source async/await lowered onto Phase-12C suspended execution (ADR 0037):
//! real native executables, exact destruction traces, sockets and wakes.
use std::{path::{Path, PathBuf}, process::{Command, Output}, collections::HashSet};
fn checked(src: &str, tag: &str) -> (PathBuf, tarn_driver::CheckResult) {
    let dir = std::env::temp_dir().join(format!("tarn-async-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn"); std::fs::write(&file, src).unwrap();
    let result = tarn_driver::check(&file).unwrap();
    (dir, result)
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
fn stdout(output: &Output) -> Vec<String> { String::from_utf8_lossy(&output.stdout).lines().map(str::to_string).collect() }
/// Destroyed traced strings, excluding temporaries of  itself.
fn drops(output: &Output, printed: &[&str]) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr).lines().filter_map(|l| l.strip_prefix("drop:")).filter(|d| !printed.contains(d)).map(str::to_string).collect()
}
fn balanced(output: &Output) {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let mut live = HashSet::new();
    for line in String::from_utf8_lossy(&output.stderr).lines().filter(|s| s.starts_with("net:")) {
        let parts: Vec<_> = line.split(':').collect(); let fd = parts[2].parse::<i32>().unwrap();
        if parts[1] == "open" { assert!(live.insert(fd)); } else { assert!(live.remove(&fd)); }
    }
    assert!(live.is_empty(), "fd leak: {live:?}");
}
const PAUSE: &str = r#"import "io"
import "runtime"
// A leaf that is Pending once (waking itself first), then Ready.
fn pause(owner &runtime.Execution) runtime.Operation<void> {
    var polled = false
    return runtime.Operation.new(owner, move fn(waker &io.Waker) io.Progress<void> {
        if polled { return io.Progress.Ready(()) }
        polled = true
        waker.wake()
        return io.Progress.Pending
    })
}
"#;

#[test]
fn abandoned_frames_destroy_exactly_their_current_state() {
    let exe = compile(r#"import "io"
import "runtime"
struct Pair { first string
    second string }
fn pause(owner &runtime.Execution) runtime.Operation<void> {
    var polled = false
    return runtime.Operation.new(owner, move fn(waker &io.Waker) io.Progress<void> {
        if polled { return io.Progress.Ready(()) }
        polled = true
        waker.wake()
        return io.Progress.Pending
    })
}
fn consume(s string) { print(s) }
async fn work(owner &runtime.Execution, pair Pair, tag string) string {
    print("start")
    await pause(owner)
    consume(pair.first)
    await pause(owner)
    consume(pair.second)
    return tag
}
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    {
        computation := work(&execution, Pair{first: "a1", second: "b1"}, "t1")
    }
    print("-- unstarted done")
    {
        var op = runtime.Operation.new(&execution, work(&execution, Pair{first: "a2", second: "b2"}, "t2"))
        op.poll()
    }
    print("-- first await done")
    {
        var op = runtime.Operation.new(&execution, work(&execution, Pair{first: "a3", second: "b3"}, "t3"))
        op.poll()
        op.poll()
    }
    print("-- second await done")
    var op = runtime.Operation.new(&execution, work(&execution, Pair{first: "a4", second: "b4"}, "t4"))
    result := try execution.block_on(&mut op)
    print(result)
    return Ok(())
}
"#, "states");
    let out = run(&exe);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["-- unstarted done", "start", "-- first await done", "start", "a3", "-- second await done", "start", "a4", "b4", "t4"]);
    // Unstarted: parameters only. Pending at the first await: all parameters.
    // Pending at the second: the moved field is not resurrected. Completion:
    // the result transfers once and is destroyed by its new owner.
    let printed = ["start", "-- unstarted done", "-- first await done", "-- second await done"];
    assert_eq!(drops(&out, &printed), ["t1", "a1", "b1", "t2", "a2", "b2", "a3", "t3", "b3", "a4", "b4", "t4"]);
}

#[test]
fn early_exits_inside_frames_destroy_owned_locals_once() {
    let source = format!("{PAUSE}{}", r#"
fn fail(flag bool) Result<i32, string> {
    if flag { return Err("failure") }
    return Ok(1)
}
async fn early(owner &runtime.Execution, mode i32) Result<i32, string> {
    kept := "kept"
    await pause(owner)
    if mode == 0 { return Ok(0) }
    value := try fail(mode == 1)
    var i = 0
    for {
        inner := "inner"
        await pause(owner)
        i = i + 1
        if i == 2 { break }
        if i == 1 { continue }
    }
    print(&kept)
    return Ok(value + i)
}
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    for mode in 0..3 {
        var op = runtime.Operation.new(&execution, early(&execution, mode))
        match try execution.block_on(&mut op) {
            Ok(v) => { print(v) }
            Err(e) => { print(e) }
        }
    }
    return Ok(())
}
"#);
    let out = run(&compile(&source, "exits"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["0", "failure", "kept", "3"]);
    // Early return drops `kept`; `try` drops `kept` and transfers the error,
    // destroyed once by its new owner; each loop iteration (continue, break)
    // destroys its own local; normal completion drops `kept` last.
    assert_eq!(drops(&out, &[]), ["kept", "kept", "failure", "inner", "inner", "kept"]);
}

#[test]
fn async_tcp_server_reads_echoes_and_observes_eof_from_native_task() {
    let out = run(&compile(r#"import "io"
import "runtime"
import "net"
// Echo one message, then observe peer EOF.
async fn serve(listener &mut net.TcpListener) Result<usize, io.Error> {
    var stream = try await listener.accept_async()
    try stream.set_nonblocking(true)
    var bytes = [64]u8{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    var total = usize(0)
    for {
        count := try await stream.read_async(&mut bytes)
        if count == usize(0) { break }
        total = total + count
        try await stream.write_all_async(&bytes[0..count])
    }
    return Ok(total)
}
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    var listener = try net.TcpListener.bind(&"127.0.0.1:0")
    try listener.set_nonblocking(true)
    address := try listener.local_addr()
    // A native pthread task produces readiness while async code waits.
    client := spawn move fn() Result<void, io.Error> {
        var stream = try net.TcpStream.connect_addr(address)
        try stream.write_all(&[5]u8{104, 101, 108, 108, 111})
        var reply = [5]u8{0, 0, 0, 0, 0}
        var got = usize(0)
        for {
            if got == usize(5) { break }
            got = got + try stream.read(&mut reply[got..])
        }
        print(reply[0])
        print(reply[4])
        return Ok(())
    }
    var app = runtime.Operation.new(&execution, serve(&mut listener))
    total := try try execution.block_on(&mut app)
    try client.join()
    print(total)
    return Ok(())
}
"#, "tcp"));
    balanced(&out);
    assert_eq!(stdout(&out), ["104", "111", "5"]);
}

#[test]
fn async_connect_read_and_udp_receive() {
    let out = run(&compile(r#"import "io"
import "runtime"
import "net"
async fn fetch(address net.SocketAddr, socket &mut net.UdpSocket) Result<u8, io.Error> {
    var connecting = try net.TcpStream.connect_nonblocking_addr(address)
    try await connecting.finish_async()
    var stream = try connecting.into_stream()
    try stream.set_nonblocking(true)
    var reply = [1]u8{0}
    count := try await stream.read_async(&mut reply)
    print(count)
    var datagram = [4]u8{0, 0, 0, 0}
    packet := try await socket.recv_from_async(&mut datagram)
    print(datagram[0])
    return Ok(reply[0])
}
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    var listener = try net.TcpListener.bind(&"127.0.0.1:0")
    address := try listener.local_addr()
    var socket = try net.UdpSocket.bind(&"127.0.0.1:0")
    try socket.set_nonblocking(true)
    udp := try socket.local_addr()
    server := spawn move fn() Result<void, io.Error> {
        var stream = try listener.accept()
        try stream.write_all(&[1]u8{77})
        var sender = try net.UdpSocket.bind(&"127.0.0.1:0")
        sent := try sender.send_to(&[1]u8{9}, udp)
        return Ok(())
    }
    var app = runtime.Operation.new(&execution, fetch(address, &mut socket))
    value := try try execution.block_on(&mut app)
    try server.join()
    print(value)
    return Ok(())
}
"#, "connect-udp"));
    balanced(&out);
    assert_eq!(stdout(&out), ["1", "9", "77"]);
}

#[test]
fn async_write_all_retains_offset_across_partial_progress() {
    // 1 MiB exceeds the socket buffers: write_all suspends after partial
    // progress until the native reader drains it, then resumes at its offset.
    let zeros = format!("[4096]u8{{{}0}}", "0, ".repeat(4095));
    let source = r#"import "io"
import "runtime"
import "net"
async fn send(stream &mut net.TcpStream, bytes &[]u8, rounds i32) Result<void, io.Error> {
    for i in 0..rounds { try await stream.write_all_async(bytes) }
    return Ok(())
}
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    var listener = try net.TcpListener.bind(&"127.0.0.1:0")
    address := try listener.local_addr()
    reader := spawn move fn() Result<usize, io.Error> {
        var stream = try listener.accept()
        var chunk = ZEROS
        var total = usize(0)
        for {
            count := try stream.read(&mut chunk)
            if count == usize(0) { return Ok(total) }
            total = total + count
        }
    }
    var stream = try net.TcpStream.connect_addr(address)
    try stream.set_nonblocking(true)
    payload := ZEROS
    {
        var op = runtime.Operation.new(&execution, send(&mut stream, &payload, 256))
        try try execution.block_on(&mut op)
    }
    try stream.shutdown(net.Shutdown.Write)
    print(try reader.join())
    return Ok(())
}
"#.replace("ZEROS", &zeros);
    let out = run(&compile(&source, "write-all"));
    balanced(&out);
    assert_eq!(stdout(&out), [(4096 * 256).to_string()]);
}

#[test]
fn nested_awaits_reuse_wake_forwarding_for_coalesced_spurious_and_immediate_wakes() {
    let source = format!("{PAUSE}{}", r#"
// Pending for several turns; wakes many times per poll (coalesced) and the
// executor may poll it spuriously after an unrelated wake.
fn noisy(owner &runtime.Execution, turns i32) runtime.Operation<i32> {
    var seen = 0
    return runtime.Operation.new(owner, move fn(waker &io.Waker) io.Progress<i32> {
        seen = seen + 1
        if seen > turns { return io.Progress.Ready(seen) }
        for i in 0..100 { waker.wake() }
        return io.Progress.Pending
    })
}
fn ready(owner &runtime.Execution) runtime.Operation<i32> {
    var polls = 0
    return runtime.Operation.new(owner, move fn(waker &io.Waker) io.Progress<i32> {
        polls = polls + 1
        return io.Progress.Ready(6 + polls)
    })
}
async fn leaf(owner &runtime.Execution) i32 { return await noisy(owner, 3) }
async fn middle(owner &runtime.Execution) i32 {
    a := await ready(owner)
    b := await leaf(owner)
    await pause(owner)
    return a + b
}
async fn top(owner &runtime.Execution) i32 { return 1 + await middle(owner) }
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    var op = runtime.Operation.new(&execution, top(&execution))
    print(try execution.block_on(&mut op))
    return Ok(())
}
"#);
    let out = run(&compile(&source, "wakes"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["12"]);
}

#[test]
fn source_computations_share_the_executor_with_bounded_fairness() {
    let source = format!("{PAUSE}{}", r#"
async fn worker(owner &runtime.Execution, name string) {
    for i in 0..3 {
        print(&name)
        await pause(owner)
    }
}
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    var executor = runtime.Executor.new()
    executor = executor.add(runtime.Operation.new(&execution, worker(&execution, "a")))
    executor = executor.add(runtime.Operation.new(&execution, worker(&execution, "b")))
    try executor.run(&execution)
    return Ok(())
}
"#);
    let out = run(&compile(&source, "fairness"));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(stdout(&out), ["a", "b", "a", "b", "a", "b"]);
    assert_eq!(drops(&out, &[]), ["a", "b"]);
}

#[test]
fn repolling_a_completed_computation_aborts() {
    let out = run(&compile(r#"import "io"
import "runtime"
async fn one() i32 { return 1 }
fn main() Result<void, io.Error> {
    execution := try runtime.Execution.new()
    var op = runtime.Operation.new(&execution, one())
    print(try execution.block_on(&mut op))
    op.poll()
    return Ok(())
}
"#, "repoll"));
    assert!(!out.status.success());
    assert_eq!(stdout(&out), ["1"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("polled after completion"));
}
