//! Buffered I/O over async networking (Phase 14C, ADR 0040): owned buffers,
//! delimiter limits, EOF policy, explicit flush, partial writes under
//! WouldBlock, drop without flush, and a concurrent buffered echo server.
use std::{path::{Path, PathBuf}, process::{Command, Output}, collections::HashSet};
fn compile(src: &str, tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tarn-buffered-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn"); std::fs::write(&file, src).unwrap();
    let result = tarn_driver::check(&file).unwrap();
    assert!(!result.has_errors(), "{}", result.diagnostics.iter().map(|d| d.render(&result.program.sources)).collect::<String>());
    let exe = dir.join("program");
    tarn_backend::build(result.drops.as_ref().unwrap(), result.typed.as_ref().unwrap(), &exe).unwrap(); exe
}
fn run(exe: &Path) -> Output {
    let out = Command::new("timeout").arg("60s").arg(exe).env("TARN_TRACE_NET", "1").output().unwrap();
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
    out
}
fn stdout(o: &Output) -> Vec<String> { String::from_utf8_lossy(&o.stdout).lines().map(str::to_string).collect() }
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
fn buffered_reader_lines_exact_reads_limits_and_end_of_stream() {
    let out = run(&compile(r#"import "net"
fn show(result Result<Vec<u8>, net.Error>) {
    match result {
        Ok(line) => { print(line.len()) }
        Err(error) => match error.kind {
            net.ErrorKind.LimitExceeded => { print("limit") }
            _ => { print("error") }
        }
    }
}
fn sum(bytes &[]u8) u64 {
    var total = u64(0)
    for b in bytes { total = total + u64(b) }
    return total
}
async fn consume(stream net.TcpStream) Result<void, net.Error> {
    var conn = stream
    try conn.set_nonblocking(true)
    var reader = net.BufferedReader.new(usize(4))
    show(await reader.read_until(&mut conn, u8(10), usize(64)))
    show(await reader.read_until(&mut conn, u8(10), usize(64)))
    print(reader.capacity())
    var exact = [8]u8{0, 0, 0, 0, 0, 0, 0, 0}
    try await reader.read_exact(&mut conn, &mut exact)
    print(sum(&exact))
    var small = [2]u8{0, 0}
    var got = usize(0)
    var total = u64(0)
    for got != usize(6) {
        var want = usize(2)
        if usize(6) - got < want { want = usize(6) - got }
        n := try await reader.read(&mut conn, &mut small[0..want])
        total = total + sum(&small[0..n])
        got = got + n
    }
    print(total)
    show(await reader.read_until(&mut conn, u8(10), usize(5)))
    show(await reader.read_until(&mut conn, u8(10), usize(64)))
    show(await reader.read_until(&mut conn, u8(10), usize(64)))
    show(await reader.read_until(&mut conn, u8(10), usize(64)))
    match await reader.read_exact(&mut conn, &mut exact) {
        Ok(value) => { print("data") }
        Err(error) => match error.kind {
            net.ErrorKind.UnexpectedEof => { print("eof") }
            _ => { print("error") }
        }
    }
    return Ok(())
}
async fn serve(listener &mut net.TcpListener) Result<void, net.Error> {
    stream := try await listener.accept_async()
    return await consume(stream)
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var listener = try net.TcpListener.bind(&"127.0.0.1:0")
    try listener.set_nonblocking(true)
    address := try listener.local_addr()
    client := spawn move fn() Result<void, net.Error> {
        var stream = try net.TcpStream.connect_addr(address)
        message := [45]u8{97, 98, 10, 104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 10, 1, 2, 3, 4, 5, 6, 7, 8, 10, 20, 30, 40, 50, 60, 116, 111, 111, 108, 111, 110, 103, 108, 105, 110, 101, 10, 116, 97, 105, 108}
        // One byte per write: many small chunks, delimiters split across reads.
        for i in 0..45 {
            try stream.write_all(&message[usize(i)..usize(i) + usize(1)])
        }
        return Ok(())
    }
    var app = net.Operation.new(&execution, serve(&mut listener))
    try try execution.block_on(&mut app)
    try client.join()
    return Ok(())
}
"#, "reader"));
    balanced(&out);
    // Lines of 3 and 12 bytes (capacity grew 4 -> 16), read_exact, small reads,
    // a too-long line that stays buffered, then partial data at EOF, EOF, and
    // read_exact failing at EOF.
    assert_eq!(stdout(&out), ["3", "12", "16", "36", "210", "limit", "12", "4", "0", "eof"]);
}

#[test]
fn buffered_writer_batches_flushes_explicitly_survives_would_block_and_drops_unflushed() {
    let out = run(&compile(r#"import "net"
copy struct Totals {
    count u64
    sum u64
}
async fn produce(stream net.TcpStream) Result<void, net.Error> {
    var conn = stream
    try conn.set_nonblocking(true)
    var writer = net.BufferedWriter.new(usize(8))
    try await writer.write_all(&mut conn, &[3]u8{1, 1, 1})
    print(writer.pending())
    accepted := try await writer.write(&mut conn, &[6]u8{1, 1, 1, 1, 1, 1})
    print(accepted)
    print(writer.pending())
    one := try await writer.write(&mut conn, &[1]u8{1})
    print(writer.pending())
    try await writer.flush(&mut conn)
    print(writer.pending())
    var chunk = Vec.new()
    for i in 0..1000 { chunk.push(u8(1)) }
    // 8 MB through a 64 KiB buffer: flushes hit WouldBlock and partial writes.
    var big = net.BufferedWriter.new(usize(65536))
    for i in 0..8000 { try await big.write_all(&mut conn, chunk.as_slice()) }
    try await big.flush(&mut conn)
    // Never flushed: destruction discards these bytes without I/O.
    var dropped = net.BufferedWriter.new(usize(8))
    try await dropped.write_all(&mut conn, &[5]u8{9, 9, 9, 9, 9})
    print(dropped.pending())
    return Ok(())
}
async fn serve(listener &mut net.TcpListener) Result<void, net.Error> {
    stream := try await listener.accept_async()
    return await produce(stream)
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var listener = try net.TcpListener.bind(&"127.0.0.1:0")
    try listener.set_nonblocking(true)
    address := try listener.local_addr()
    client := spawn move fn() Result<Totals, net.Error> {
        var stream = try net.TcpStream.connect_addr(address)
        var bytes = [64]u8{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
        var totals = Totals{count: u64(0), sum: u64(0)}
        for {
            n := try stream.read(&mut bytes)
            if n == usize(0) { return Ok(totals) }
            for b in &bytes[0..n] { totals.sum = totals.sum + u64(b) }
            totals.count = totals.count + u64(n)
        }
    }
    var app = net.Operation.new(&execution, serve(&mut listener))
    try try execution.block_on(&mut app)
    totals := try client.join()
    print(totals.count)
    print(totals.sum)
    return Ok(())
}
"#, "writer"));
    balanced(&out);
    // Small writes stay buffered, a full buffer accepts only what fits, the next
    // write flushes first, explicit flush empties it; unflushed 9s never arrive.
    assert_eq!(stdout(&out), ["3", "5", "8", "1", "0", "5", "8000009", "8000009"]);
}

#[test]
fn concurrent_buffered_echo_serves_every_connection_in_one_thread() {
    let out = run(&compile(r#"import "net"
async fn handle(stream net.TcpStream) Result<i32, net.Error> {
    var conn = stream
    try conn.set_nonblocking(true)
    var reader = net.BufferedReader.new(usize(16))
    var writer = net.BufferedWriter.new(usize(64))
    var lines = 0
    for {
        line := try await reader.read_until(&mut conn, u8(10), usize(256))
        if line.is_empty() {
            try await writer.flush(&mut conn)
            return Ok(lines)
        }
        try await writer.write_all(&mut conn, line.as_slice())
        // Flush once no more input is buffered (batching pipelined lines).
        if reader.buffered() == usize(0) { try await writer.flush(&mut conn) }
        lines = lines + 1
    }
}
async fn serve(owner &net.Execution, listener &mut net.TcpListener, clients i32) Result<i32, net.Error> {
    var tasks = Vec.new()
    for i in 0..clients {
        stream := try await listener.accept_async()
        tasks.push(owner.spawn_async(handle(stream)))
    }
    var total = 0
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
    client := spawn move fn() Result<i32, net.Error> {
        // Every connection is open at once; lines are interleaved across them.
        var streams = Vec.new()
        for c in 0..40 { streams.push(try net.TcpStream.connect_addr(address)) }
        for round in 0..25 {
            for c in 0..40 {
                line := [4]u8{u8(65 + c % 26), u8(48 + round % 10), u8(33), u8(10)}
                try streams.get_mut(usize(c)).write_all(&line)
            }
        }
        for c in 0..40 { try streams.get_mut(usize(c)).shutdown(net.Shutdown.Write) }
        var verified = 0
        for c in 0..40 {
            var reply = [100]u8{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
            var got = usize(0)
            for {
                n := try streams.get_mut(usize(c)).read(&mut reply[got..])
                if n == usize(0) { break }
                got = got + n
            }
            if got != usize(100) { return Ok(-1) }
            for round in 0..25 {
                at := usize(round) * usize(4)
                if reply[at] != u8(65 + c % 26) || reply[at + usize(1)] != u8(48 + round % 10) || reply[at + usize(3)] != u8(10) { return Ok(-2) }
            }
            verified = verified + 1
        }
        return Ok(verified)
    }
    var app = net.Operation.new(&execution, serve(&execution, &mut listener, 40))
    lines := try try execution.block_on(&mut app)
    print(lines)
    print(try client.join())
    return Ok(())
}
"#, "echo"));
    balanced(&out);
    assert_eq!(stdout(&out), ["1000", "40"]);
}
