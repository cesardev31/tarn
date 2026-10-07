//! Real child processes, dual-pipe capture, owned completion and private ABI.
use std::{
    collections::HashSet,
    path::PathBuf,
    process::{Command, Output},
};

fn checked(src: &str, tag: &str) -> (PathBuf, tarn_driver::CheckResult) {
    let dir = std::env::temp_dir().join(format!("tarn-process-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let helper = dir.join("child");
    let cc = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/process_child.c"))
        .arg("-o")
        .arg(&helper)
        .output()
        .unwrap();
    assert!(
        cc.status.success(),
        "{}",
        String::from_utf8_lossy(&cc.stderr)
    );
    let src = src.replace("HELPER", helper.to_str().unwrap());
    let entry = dir.join("main.tarn");
    std::fs::write(&entry, src.replace("ROOT", dir.to_str().unwrap())).unwrap();
    let result = tarn_driver::check(&entry).unwrap();
    assert!(
        !result.has_errors(),
        "{}",
        result
            .diagnostics
            .iter()
            .map(|d| d.render(&result.program.sources))
            .collect::<String>()
    );
    (dir, result)
}
fn build(dir: &std::path::Path, res: &tarn_driver::CheckResult) -> PathBuf {
    let exe = dir.join("program");
    tarn_backend::build(
        res.drops.as_ref().unwrap(),
        res.typed.as_ref().unwrap(),
        &exe,
    )
    .unwrap();
    exe
}
fn run(exe: &PathBuf) -> Output {
    let input = exe.parent().unwrap().join("parent-stdin");
    std::fs::write(&input, "parent input").unwrap();
    Command::new("timeout")
        .arg("30s")
        .arg(exe)
        .env("TARN_TRACE_PROCESS", "1")
        .env("TARN_PROCESS_TEST_ENV", "inherited")
        .stdin(std::fs::File::open(input).unwrap())
        .output()
        .unwrap()
}
fn balanced(out: &Output) -> usize {
    let mut children = HashSet::new();
    let mut waiting = HashSet::new();
    let mut live = HashSet::new();
    let mut count = 0;
    for line in String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter(|s| s.starts_with("process:"))
    {
        let parts: Vec<_> = line.split(':').collect();
        let fd: i32 = parts[2].parse().unwrap();
        match parts[1] {
            "fd_open" => {
                assert!(live.insert(fd), "duplicate owner: {line}");
                count += 1;
            }
            "fd_close" => {
                assert!(live.remove(&fd), "double close: {line}");
            }
            "spawn" => {
                assert!(children.insert(fd), "duplicate child: {line}");
            }
            "wait" => {
                assert!(
                    children.contains(&fd) && waiting.insert(fd),
                    "double/unowned wait: {line}"
                );
            }
            "reap" => {
                assert!(
                    children.remove(&fd) && waiting.remove(&fd),
                    "double/unowned reap: {line}"
                );
            }
            _ => panic!("unknown trace: {line}"),
        }
    }
    assert!(waiting.is_empty(), "unfinished waits: {waiting:?}");
    assert!(children.is_empty(), "unreaped children: {children:?}");
    assert!(live.is_empty(), "descriptor leak: {live:?}");
    count
}
fn success(out: &Output, expected: &str) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .all(|s| s.starts_with("process:"))
    );
    balanced(out);
}
#[test]
fn arguments_environment_working_directory_and_status_are_application_level() {
    let (dir, res) = checked(
        r#"import "process"
import "io"
fn main() Result<void, io.Error> {
 args := [3]string{"space arg", ";$(literal)", "日本"}
 print((try process.Command.new("/bin/sh").arg("-c").arg("read value; test \"$value\" = \"parent input\"").status()).success())
 command := process.Command.new("HELPER").arg("args").args(&args)
 output := try command.output()
 print((try output.stdout_text()) == "space arg\n;$(literal)\n日本\n")
 print(output.status.success())
 print(output.stderr.len())
 before := try process.Command.new("HELPER").arg("cwd").output()
 cwd := try process.Command.new("HELPER").arg("cwd").current_dir("ROOT/data").output()
 print((try cwd.stdout_text()) == "ROOT/data\n")
 after := try process.Command.new("HELPER").arg("cwd").output()
 print((try before.stdout_text()) == (try after.stdout_text()))
 env := try process.Command.new("HELPER").arg("env").output()
 print((try env.stdout_text()) == "inherited\n")
 input := try process.Command.new("HELPER").arg("stdin").output()
 print((try input.stdout_text()) == "eof\n")
 reusable := process.Command.new("true")
 print((try reusable.status()).success())
 print((try reusable.status()).success())
 print((try process.Command.new("/bin/false").status()).success())
 status := try process.Command.new("/bin/sh").arg("-c").arg("kill -TERM $$").status()
 match status.signal() { Some(value) => { print(value) }
 None => { panic("expected signal") } }
 var child = try process.Command.new("HELPER").arg("pause").start()
 print(child.id() > u32(0))
 try child.kill()
 match (try child.wait()).signal() { Some(value) => { print(value) }
 None => { panic("expected kill") } }
 return Ok(())
}
"#,
        "application",
    );
    success(
        &run(&build(&dir, &res)),
        "true\ntrue\ntrue\n0\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\nfalse\n15\ntrue\n9\n",
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn large_dual_output_and_binary_capture_do_not_deadlock_or_decode_lossily() {
    let (dir, res) = checked(
        r#"import "process"
import "io"
fn main() Result<void, io.Error> {
 output := try process.Command.new("HELPER").arg("large").output()
 print(output.stdout.len())
 print(output.stderr.len())
 print(output.stdout.get(usize(0)))
 print(output.stderr.get(usize(131072)))
 match output.status.code() { Some(value) => { print(value) }
 None => { panic("expected exit") } }
 binary := try process.Command.new("HELPER").arg("binary").output()
 print(binary.stdout.len())
 print(binary.stderr.len())
 print((try binary.stderr_text()).len())
 match binary.stdout_text() { Err(error) => { match error.kind { io.ErrorKind.InvalidData => { print("invalid UTF-8") }
 _ => { panic("wrong error") } } }
  Ok(text) => { panic("lossy conversion") } }
 return Ok(())
}
"#,
        "capture",
    );
    success(
        &run(&build(&dir, &res)),
        "131073\n131073\n111\n101\n7\n3\n1\n1\ninvalid UTF-8\n",
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn acquisition_errors_preserve_categories_and_close_all_temporary_fds() {
    let (dir, res) = checked(
        r#"import "process"
import "io"
fn check(command process.Command) {
 match command.output() { Err(error) => { print(error.native_code())
  match error.kind { io.ErrorKind.NotFound => { print("missing") }
   io.ErrorKind.InvalidInput => { print("invalid") }
   io.ErrorKind.InvalidData => { print("format") }
   io.ErrorKind.PermissionDenied => { print("denied") }
   _ => { panic("unexpected error") } } }
  Ok(output) => { panic("unexpected process") } }
}
fn main() {
 check(process.Command.new("/tarn-missing-process-15d"))
 check(process.Command.new(""))
 check(process.Command.new("/bin/true\0ignored"))
 check(process.Command.new("/bin/true").arg("bad\0arg"))
 check(process.Command.new("/bin/true").current_dir(""))
 check(process.Command.new("/bin/true").current_dir("ROOT/missing"))
 check(process.Command.new("ROOT/data/not-a-binary"))
 check(process.Command.new("ROOT/data/not-executable"))
}
"#,
        "errors",
    );
    use std::os::unix::fs::PermissionsExt;
    for (name, mode) in [("not-a-binary", 0o755), ("not-executable", 0o644)] {
        let file = dir.join("data").join(name);
        std::fs::write(&file, "echo implicit-shell-must-not-run\n").unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    success(
        &run(&build(&dir, &res)),
        "2\nmissing\n22\ninvalid\n22\ninvalid\n22\ninvalid\n22\ninvalid\n2\nmissing\n8\nformat\n13\ndenied\n",
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_on_all_normal_exits_and_task_transfer_is_exactly_once() {
    let (dir, res) = checked(
        r#"import "process"
import "io"
fn child() process.Process {
 match process.Command.new("/bin/true").start() { Ok(value) => { return value }
  Err(error) => { panic("spawn failed") } }
}
fn early() { p := child()
 return }
fn failure() Result<void, io.Error> { p := child()
 try process.Command.new("/missing-tarn-child").status()
 return Ok(()) }
fn main() {
 { p := child() }
 early()
 for { p := child()
  break }
 var count = 0
 for count < 2 { p := child()
  count = count + 1
  continue }
 for condition in &[2]bool{true, false} { var p: process.Process
  if condition { p = child() } }
 { a := child()
  b := a
  b.wait() }
 { var p = child()
  p = child()
  p = p }
 failure()
 t := spawn move fn() process.Process { return child() }
 returned := t.join()
 returned.wait()
 p := child()
 worker := spawn move fn() { p.wait() }
 worker.join()
 scope { spawn fn() { process.Command.new("HELPER").arg("large").output() }
  spawn fn() { process.Command.new("HELPER").arg("large").output() } }
 print("complete")
}
"#,
        "completion",
    );
    let output = run(&build(&dir, &res));
    success(&output, "complete\n");
    let trace = String::from_utf8(output.stderr.clone()).unwrap();
    let wait = trace
        .lines()
        .find(|line| line.starts_with("process:wait:"))
        .unwrap();
    let mut duplicated = output.clone();
    duplicated
        .stderr
        .extend_from_slice(format!("{wait}\n").as_bytes());
    assert!(std::panic::catch_unwind(|| balanced(&duplicated)).is_err());
    let mut missing = output.clone();
    missing.stderr = trace
        .lines()
        .filter(|line| *line != wait)
        .map(|line| format!("{line}\n"))
        .collect::<String>()
        .into_bytes();
    assert!(std::panic::catch_unwind(|| balanced(&missing)).is_err());

    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| line.starts_with("process:spawn:"))
            .count(),
        14
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupted_resource_and_intrinsic_contracts_are_rejected() {
    let (dir, mut res) = checked(
        "import \"process\"\nimport \"fs\"\nfn observe(f &fs.File) { f.metadata() }\nfn main() { process.Command.new(\"/bin/true\").status() }",
        "abi",
    );
    let drops = res.drops.as_ref().unwrap();
    let typed = res.typed.as_mut().unwrap();
    for owner in [
        typed.decls.process_owner.unwrap(),
        typed.decls.process_pipe.unwrap(),
    ] {
        typed
            .decls
            .native_capabilities
            .get_mut(&owner)
            .unwrap()
            .share = true;
        assert!(
            tarn_backend::emit_object(drops, typed)
                .unwrap_err()
                .to_string()
                .contains("process declaration ABI")
        );
        typed
            .decls
            .native_capabilities
            .get_mut(&owner)
            .unwrap()
            .share = false;
        typed.decls.structs.get_mut(&owner).unwrap().is_copy = true;
        assert!(!tarn_ir::post_drop::verify(drops, typed).is_empty());
        typed.decls.structs.get_mut(&owner).unwrap().is_copy = false;
        typed.decls.structs.get_mut(&owner).unwrap().fields[0].is_pub = true;
        assert!(!tarn_ir::post_drop::verify(drops, typed).is_empty());
        typed.decls.structs.get_mut(&owner).unwrap().fields[0].is_pub = false;
    }
    let pipe = typed.decls.process_pipe.unwrap();
    // Both resources have scalar storage; identities must still be tied to
    // consuming signatures rather than accepted just for their field layout.
    typed.decls.process_pipe = Some(typed.decls.fs_file.expect("same-shaped filesystem owner"));
    assert!(!tarn_ir::post_drop::verify(drops, typed).is_empty());
    typed.decls.process_pipe = Some(pipe);
    typed.decls.process_intrinsics.remove("process._wait");
    assert!(!tarn_ir::post_drop::verify(drops, typed).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn simultaneous_native_workers_capture_large_children_deterministically() {
    let (dir, res) = checked(
        r#"import "process"
import "io"
fn main() {
 var workers: Vec<Task<usize>> = Vec.new()
 var index: usize = 0
 for index < usize(16) {
  worker := spawn move fn() usize {
   match process.Command.new("HELPER").arg("large").output() { Ok(output) => { return output.stdout.len() }
    Err(error) => { panic("capture failed") } }
  }
  workers.push(worker)
  index = index + 1
 }
 var total: usize = 0
 for !workers.is_empty() { match workers.pop() { Some(worker) => { total = total + worker.join() }
  None => { panic("worker count invariant") } } }
 print(total)
}
"#,
        "stress",
    );
    success(&run(&build(&dir, &res)), "2097168\n");
    std::fs::remove_dir_all(dir).unwrap();
}

fn link_faults(dir: &std::path::Path, res: &tarn_driver::CheckResult) -> PathBuf {
    let object = dir.join("program.o");
    std::fs::write(
        &object,
        tarn_backend::emit_object(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap())
            .unwrap(),
    )
    .unwrap();
    let exe = dir.join("program");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("cc")
        .args([
            "-std=c11",
            "-O0",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fno-strict-aliasing",
            "-no-pie",
            "-pthread",
        ])
        .arg(object)
        .arg(root.join("../../runtime/native.c"))
        .arg(root.join("tests/process_faults.c"))
        .args([
            "-Wl,--wrap=pipe2",
            "-Wl,--wrap=fcntl",
            "-Wl,--wrap=read",
            "-Wl,--wrap=waitpid",
            "-Wl,--wrap=close",
            "-Wl,--wrap=posix_spawnp",
            "-lm",
            "-o",
        ])
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    exe
}

#[test]
fn interrupted_partial_reads_and_acquisition_failures_preserve_cleanup() {
    let (dir, res) = checked(
        "import \"process\"\nimport \"io\"\nfn main() Result<void, io.Error> { try process.Command.new(\"/denied-process\").output()\n return Ok(()) }",
        "permission",
    );
    let output = run(&link_faults(&dir, &res));
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("permission denied"));
    balanced(&output);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn private_runtime_never_leaks_fds_or_inherits_unrelated_descriptors() {
    let (dir, _) = checked("fn main() {}", "runtime");
    let source = dir.join("runtime.c");
    std::fs::write(
        &source,
        format!(
            "{}\n{}",
            include_str!("../../../runtime/native.c"),
            include_str!("process_runtime.c")
        ),
    )
    .unwrap();
    let exe = dir.join("runtime-test");
    let output = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pthread"])
        .arg(source)
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/process_faults.c"))
        .args([
            "-Wl,--wrap=pipe2",
            "-Wl,--wrap=fcntl",
            "-Wl,--wrap=read",
            "-Wl,--wrap=waitpid",
            "-Wl,--wrap=close",
            "-Wl,--wrap=posix_spawnp",
            "-lm",
            "-o",
        ])
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new("timeout")
        .arg("30s")
        .arg(exe)
        .arg(dir.join("child"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
