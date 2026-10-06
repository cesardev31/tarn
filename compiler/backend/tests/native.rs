//! Real ELF objects, system linking, and child processes. No JIT or simulated
//! execution is substituted for native assertions.
#[path = "../../driver/tests/common/drop_machine.rs"]
mod drop_machine;
use std::path::Path;
use std::process::Command;

fn compile(src: &str, tag: &str) -> (std::path::PathBuf, tarn_driver::CheckResult) {
    let dir = std::env::temp_dir().join(format!("tarn-e2e-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn");
    std::fs::write(&file, src).unwrap();
    let res = tarn_driver::check(&file).unwrap();
    assert!(!res.has_errors(), "{}", res.diagnostics.iter().map(|d| d.render(&res.program.sources)).collect::<String>());
    let exe = dir.join("program");
    tarn_backend::build(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap(), &exe).unwrap();
    (exe, res)
}
#[test]
fn native_scalar_and_aggregate_suite() {
    let mut paths: Vec<_> = std::fs::read_dir("../../tests/native/pass").unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "tarn")).collect();
    paths.sort();
    assert!(paths.len() >= 10);
    for path in paths {
        let tag = path.file_stem().unwrap().to_str().unwrap();
        let (exe, _) = compile(&std::fs::read_to_string(&path).unwrap(), tag);
        let output = Command::new(&exe).env_remove("TARN_TRACE_DROPS").output().unwrap();
        assert!(output.status.success(), "{}: {}", path.display(), String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), std::fs::read_to_string(path.with_extension("stdout")).unwrap(), "{}", path.display());
        assert!(output.stderr.is_empty());
        std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
    }
}
#[test]
fn native_drops_match_post_drop_interpreter_on_every_boolean_path() {
    let mut paths: Vec<_> = std::fs::read_dir("../../tests/drops/pass").unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "tarn")).collect();
    paths.sort();
    let mut runs = 0;
    for path in paths {
        let original = tarn_driver::check(&path).unwrap();
        let p = original.drops.as_ref().unwrap();
        let f = p.functions.iter().find(|f| f.decl.name == "main").unwrap();
        for bits in 0..(1u32 << f.decl.param_count) {
            let args: Vec<_> = (0..f.decl.param_count).map(|i| bits & (1 << i) != 0).collect();
            let (expected, aborted) = drop_machine::execute(p, f.decl.id, &args);
            let source = std::fs::read_to_string(&path).unwrap().replace("fn main(", "fn native_case(");
            let source = format!("{source}\nfn main() {{\n    native_case({})\n}}\n", args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "));
            let tag = format!("drops-{}-{bits}", path.file_stem().unwrap().to_str().unwrap());
            let (exe, _) = compile(&source, &tag);
            let output = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            let drops: Vec<_> = stderr.lines().filter_map(|l| l.strip_prefix("drop:").map(str::to_string)).collect();
            assert_eq!(drops, expected, "{} path {bits}: {stderr}", path.display());
            assert_eq!(!output.status.success(), aborted, "{} path {bits}: {stderr}", path.display());
            assert!(output.stdout.is_empty());
            if !aborted {
                assert!(stderr.lines().all(|l| l.starts_with("drop:")));
            }
            std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
            runs += 1;
        }
    }
    assert!(runs >= 54);
}
#[test]
fn unsupported_valid_features_fail_without_panics() {
    for entry in std::fs::read_dir("../../tests/native/fail").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|x| x != "tarn") {
            continue;
        }
        let res = tarn_driver::check(&path).unwrap();
        if path.ends_with("opaque.tarn") {assert!(res.diagnostics.iter().any(|d|d.code=="E3040"));continue;}
        assert!(!res.has_errors(), "{} must be frontend-valid", path.display());
        let error = tarn_backend::emit_object(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap()).unwrap_err();
        assert!(error.to_string().starts_with("backend not implemented:"), "{error}");
    }
}
#[test]
fn checked_arithmetic_and_panic_abort() {
    for (name, body) in [
        ("overflow", "a: i8 := 127\n    b: i8 := 1\n    print(a + b)"),
        ("underflow", "a: u8 := 0\n    b: u8 := 1\n    print(a - b)"),
        ("mul_overflow", "a: u64 := 18446744073709551615\n    b: u64 := 2\n    print(a * b)"),
        ("divide_zero", "a := 10\n    b := 0\n    print(a / b)"),
        ("divide_min", "a: i64 := -9223372036854775808\n    b: i64 := -1\n    print(a / b)"),
        ("cast_range", "a: i16 := 300\n    print(u8(a))"),
        ("panic", "x := \"live\"\n    panic(\"stop\")"),
    ] {
        let (exe, _) = compile(&format!("fn main() {{\n    {body}\n}}\n"), name);
        let out = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
        assert!(!out.status.success(), "{name} must abort");
        assert!(String::from_utf8_lossy(&out.stderr).contains("panic:"));
        assert!(!String::from_utf8_lossy(&out.stderr).contains("drop:"), "abort must not clean stack");
        std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
    }
}
#[test]
fn canonical_layout_offsets_and_alignment() {
    let res = tarn_driver::check(Path::new("../../tests/native/pass/struct.tarn")).unwrap();
    let t = res.typed.unwrap();
    let f = res.drops.unwrap();
    let main = f.functions.iter().find(|f| f.decl.name == "main").unwrap();
    let ty = &main.decl.locals.iter().find(|l| l.name.as_deref() == Some("p")).unwrap().ty;
    let l = tarn_backend::layout::layout(&t, ty).unwrap();
    assert_eq!((l.size, l.align), (16, 8));
    assert_eq!(l.fields.iter().map(|(o, _)| *o).collect::<Vec<_>>(), vec![0, 8]);
}

#[test]
fn native_destruction_order_uses_distinct_resources() {
    let source = "struct Pair {\n    a string\n    b string\n}\nfn scenario(c bool) {\n    var p = Pair{a: \"first\", b: \"second\"}\n    x := p.a\n    if c {\n        p.a = \"replacement\"\n    }\n    y := \"last\"\n}\nfn main() {\n    scenario(true)\n    scenario(false)\n}\n";
    let (exe, res) = compile(source, "drop-order");
    let p = res.drops.as_ref().unwrap();
    let main = p.functions.iter().find(|f| f.decl.name == "main").unwrap();
    let (expected, abort) = drop_machine::execute(p, main.decl.id, &[]);
    assert!(!abort);
    assert_eq!(expected, vec!["last", "first", "replacement", "second", "last", "first", "second"]);
    let out = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(out.status.success());
    let actual: Vec<_> = String::from_utf8_lossy(&out.stderr).lines().map(|s| s.strip_prefix("drop:").unwrap().to_string()).collect();
    assert_eq!(actual, expected);
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn backend_rejects_invalid_call_and_return_abi() {
    let mut res = tarn_driver::check(Path::new("../../tests/native/pass/milestone.tarn")).unwrap();
    let p = res.drops.as_mut().unwrap();
    let main = p.functions.iter_mut().find(|f| f.decl.name == "main").unwrap();
    let call = main
        .blocks
        .iter_mut()
        .find_map(|b| match &mut b.term {
            tarn_ir::Terminator::Call { args, .. } if args.len() == 2 => Some(args),
            _ => None,
        })
        .unwrap();
    call[0] = tarn_ir::Operand::Const(tarn_ir::Const::Bool(true));
    let err = tarn_backend::emit_object(p, res.typed.as_ref().unwrap()).unwrap_err();
    assert!(err.to_string().contains("parameter ABI mismatch"));
}

#[test]
fn native_line_deletions_never_panic_or_emit_invalid_code() {
    let dir = std::env::temp_dir().join(format!("tarn-native-mutations-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    let mut runs = 0;
    for category in ["pass", "fail"] {
        for file in std::fs::read_dir(format!("../../tests/native/{category}")).unwrap() {
            let path = file.unwrap().path();
            if path.extension().is_none_or(|x| x != "tarn") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<_> = src.lines().collect();
            for i in 0..lines.len() {
                std::fs::write(&entry, [&lines[..i], &lines[i + 1..]].concat().join("\n")).unwrap();
                let res = tarn_driver::check(&entry).unwrap();
                if res.has_errors() {
                    continue;
                }
                let emitted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tarn_backend::emit_object(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap())));
                assert!(emitted.is_ok(), "backend panic in {} without line {}", path.display(), i + 1);
                if let Err(e) = emitted.unwrap() {
                    assert!(e.to_string().starts_with("backend not implemented:"), "{} without line {}: {e}", path.display(), i + 1);
                }
                runs += 1;
            }
        }
    }
    assert!(runs > 40);
    std::fs::remove_dir_all(dir).unwrap();
}
