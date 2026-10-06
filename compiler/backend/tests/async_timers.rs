//! Monotonic timers and timeouts (Phase 14B, ADR 0039): timerfd readiness on
//! the shared Poll, sleep ordering, completion/timeout races decided once,
//! structured abandonment of losers, many timers and fd-balanced cleanup.
use std::{path::{Path, PathBuf}, process::{Command, Output}, collections::HashSet};
fn compile(src: &str, tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tarn-timers-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn"); std::fs::write(&file, src).unwrap();
    let result = tarn_driver::check(&file).unwrap();
    assert!(!result.has_errors(), "{}", result.diagnostics.iter().map(|d| d.render(&result.program.sources)).collect::<String>());
    let exe = dir.join("program");
    tarn_backend::build(result.drops.as_ref().unwrap(), result.typed.as_ref().unwrap(), &exe).unwrap(); exe
}
fn run(exe: &Path) -> (Output, std::time::Duration) {
    let start = std::time::Instant::now();
    let out = Command::new("timeout").arg("30s").arg(exe).env("TARN_TRACE_NET", "1").env("TARN_TRACE_DROPS", "1").output().unwrap();
    let elapsed = start.elapsed();
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
    (out, elapsed)
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
fn timers_order_races_and_abandon_losers_exactly_once() {
    let (out, elapsed) = run(&compile(r#"import "net"
async fn nap(ms u64, name string) Result<string, net.Error> {
    try await net.sleep(net.Duration.milliseconds(ms))
    return Ok(name)
}
async fn instant(name string) Result<string, net.Error> { return Ok(name) }
async fn app(owner &net.Execution) Result<i32, net.Error> {
    // Ordering: shorter sleep finishes first, independent of spawn order.
    slow := owner.spawn_async(nap(u64(60), "slow"))
    quick := owner.spawn_async(nap(u64(10), "quick"))
    a := try await quick.join()
    print(&a)
    b := try await slow.join()
    print(&b)
    // Operation wins.
    match try await owner.spawn_async(nap(u64(5), "op-wins")).join_timeout(net.Duration.milliseconds(500)) {
        Some(r) => { print(&(try r)) }
        None => { print("unexpected timeout") }
    }
    // Timeout wins: the pending task is abandoned, its string destroyed once.
    match try await owner.spawn_async(nap(u64(5000), "abandoned")).join_timeout(net.Duration.milliseconds(20)) {
        Some(r) => { print("unexpected result") }
        None => { print("timed out") }
    }
    // Both ready in the same poll: completion has priority.
    tie := owner.spawn_async(instant("tie"))
    try await net.sleep(net.Duration.milliseconds(5))
    match try await tie.join_timeout(net.Duration.milliseconds(0)) {
        Some(r) => { print(&(try r)) }
        None => { print("tie lost") }
    }
    // Many timers.
    var tasks = Vec.new()
    for i in 0..200 { tasks.push(owner.spawn_async(nap(u64(i % 10), "t"))) }
    var done = 0
    for {
        match tasks.pop() {
            Some(t) => {
                r := try await t.join()
                done = done + 1
            }
            None => { break }
        }
    }
    print(done)
    // Left running at exit: abandoned with block_on, timer fd closed.
    owner.spawn_async(nap(u64(10000), "left-at-exit"))
    return Ok(0)
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var op = net.Operation.new(&execution, app(&execution))
    print(try try execution.block_on(&mut op))
    return Ok(())
}
"#, "races"));
    balanced(&out);
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
    assert_eq!(lines, ["quick", "slow", "op-wins", "timed out", "tie", "200", "0"]);
    // The 5 s and 10 s tasks were abandoned, not awaited.
    assert!(elapsed < std::time::Duration::from_secs(4), "{elapsed:?}");
    let mut drops: Vec<String> = String::from_utf8_lossy(&out.stderr).lines().filter_map(|l| l.strip_prefix("drop:")).filter(|d| *d != "t").map(str::to_string).collect();
    drops.sort();
    let mut expected = ["abandoned", "left-at-exit", "op-wins", "quick", "slow", "tie", "timed out"].map(str::to_string).to_vec();
    expected.sort();
    assert_eq!(drops, expected, "each owned string destroyed exactly once");
}

#[test]
fn dropped_timer_closes_without_firing_and_zero_fires_immediately() {
    let (out, _) = run(&compile(r#"import "net"
async fn app() Result<i32, net.Error> {
    {
        unused := try net.Timer.after(net.Duration.seconds(u64(60)))
    }
    var now = try net.Timer.after(net.Duration.milliseconds(u64(0)))
    try await now.wait_async()
    return Ok(1)
}
fn main() Result<void, net.Error> {
    execution := try net.Execution.new()
    var op = net.Operation.new(&execution, app())
    print(try try execution.block_on(&mut op))
    return Ok(())
}
"#, "drop-zero"));
    balanced(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}
