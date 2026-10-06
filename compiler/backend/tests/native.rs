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
        if path.ends_with("opaque.tarn") {
            assert!(res.diagnostics.iter().any(|d| d.code == "E3040"));
            continue;
        }
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
        ("shift_width", "a: u8 := 1\n    print(a << 8)"),
        ("shift_negative", "a := 1\n    print(a >> -1)"),
        ("float_u64_boundary", "a: f64 := 18446744073709551616.0\n    print(u64(a))"),
        ("float_i64_boundary", "a: f64 := 9223372036854775808.0\n    print(i64(a))"),
        ("float_f32_range", "a: f32 := 128.0\n    print(i8(a))"),
        ("float_negative_unsigned", "a: f64 := -1.0\n    print(u64(a))"),
        ("slice_reversed", "a := [2]i64{1, 2}\n    s := &a[2..1]\n    print(s.len())"),
        ("float_range", "a: f64 := 256.0\n    print(u8(a))"),
        ("float_nan", "a: f64 := 0.0\n    print(i64(a / a))"),
        ("float_infinity", "a: f64 := 1.0\n    b: f64 := 0.0\n    print(i64(a / b))"),
        ("slice_bounds", "a := [2]i64{1, 2}\n    s := &a[0..2]\n    print(s[2])"),
        ("slice_range", "a := [2]i64{1, 2}\n    s := &a[1..3]\n    print(s.len())"),
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

#[test]
fn generic_instances_are_reused_and_deterministic() {
    let (exe, res) = compile(
        "fn a<T>(x T, n i64) T { if n == 0 { return x }\n return b(x, n - 1) }\nfn b<T>(x T, n i64) T { return a(x, n) }\nfn unused<T>(x T) T { return x }\nfn main() { print(a(42, 2))\n print(a(1, 0))\n print(a(true, 0)) }",
        "mono-reuse",
    );
    let p = tarn_backend::mono::specialize(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap()).unwrap();
    assert_eq!(p.functions.len(), 5); // main, a<i64>, a<bool>, b<i64>, b<bool>
    assert!(p.functions.iter().all(|f| !f.decl.name.starts_with("unused")));
    let again = tarn_backend::mono::specialize(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap()).unwrap();
    assert_eq!(p.functions.iter().map(|f| &f.decl.name).collect::<Vec<_>>(), again.functions.iter().map(|f| &f.decl.name).collect::<Vec<_>>());
    let out = Command::new(&exe).output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"42\n1\ntrue\n");
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}
#[test]
fn generic_owned_drops_match_interpreter() {
    let source = std::fs::read_to_string("../../tests/native/pass/generic_adts.tarn")
        .unwrap()
        .lines()
        .filter(|line| !line.trim().starts_with("print("))
        .map(|line| line.to_owned() + "\n")
        .collect::<String>()
        .replace("Some(x) => print(x)", "Some(x) => {}")
        .replace("None => print(0)", "None => {}")
        .replace("Ok(x) => print(x)", "Ok(x) => {}")
        .replace("Err(e) => print(e.message)", "Err(e) => {}");
    let (exe, res) = compile(&source, "generic-drops");
    let p = tarn_backend::mono::specialize(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap()).unwrap();
    let (expected, aborted) = drop_machine::execute(&p, p.functions[0].decl.id, &[]);
    assert!(!aborted);
    let out = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(out.status.success());
    let trace: Vec<_> = String::from_utf8_lossy(&out.stderr).lines().filter_map(|s| s.strip_prefix("drop:").map(str::to_owned)).collect();
    assert_eq!(trace, expected);
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn dynamic_metadata_corruption_is_rejected() {
    use tarn_ir::{Callee, CoerceKind, Rvalue, StatementKind, Terminator, post_drop as post};
    let source = std::fs::read_to_string("../../tests/native/pass/dynamic.tarn").unwrap();
    let (exe, res) = compile(&source, "dynamic-corruption");
    let t = res.typed.as_ref().unwrap();
    let p = tarn_backend::mono::specialize(res.drops.as_ref().unwrap(), t).unwrap();
    tarn_backend::verify_dynamic(&p, t).unwrap();
    for mutation in 0..6 {
        let mut corrupt = post::Program { functions: p.functions.clone(), by_symbol: p.by_symbol.clone() };
        let table = corrupt
            .functions
            .iter_mut()
            .flat_map(|f| &mut f.blocks)
            .flat_map(|b| &mut b.stmts)
            .find_map(|s| match &mut s.op {
                post::Op::Plain(StatementKind::Assign(_, Rvalue::Coerce(CoerceKind::DynTable { interface, concrete, methods }, _, ty))) => Some((interface, concrete, methods, ty)),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => {
                table.2.pop();
            }
            1 => table.2.swap(0, 1),
            2 => *table.0 = tarn_resolve::SymbolId(u32::MAX),
            3 => *table.1 = tarn_types::Ty::Bool,
            4 => *table.3 = tarn_types::Ty::Bool,
            _ => table.2[0] = tarn_ir::FunctionId(u32::MAX),
        }
        assert!(tarn_backend::verify_dynamic(&corrupt, t).is_err(), "mutation {mutation}");
    }
    let mut corrupt = post::Program { functions: p.functions.clone(), by_symbol: p.by_symbol.clone() };
    let call = corrupt
        .functions
        .iter_mut()
        .flat_map(|f| &mut f.blocks)
        .find_map(|b| match &mut b.term {
            Terminator::Call { callee: Callee::Virtual { method, .. }, .. } => Some(method),
            _ => None,
        })
        .unwrap();
    *call = tarn_resolve::SymbolId(u32::MAX);
    assert!(tarn_backend::verify_dynamic(&corrupt, t).is_err());
    let mut corrupt = post::Program { functions: p.functions.clone(), by_symbol: p.by_symbol.clone() };
    for f in &mut corrupt.functions {
        for b in &mut f.blocks {
            b.stmts.retain(|s| !matches!(&s.op, post::Op::Plain(StatementKind::Assign(_, Rvalue::Coerce(CoerceKind::DynTable { .. }, _, _)))));
        }
    }
    assert!(tarn_backend::verify_dynamic(&corrupt, t).is_err());
    let mut corrupt = post::Program { functions: p.functions.clone(), by_symbol: p.by_symbol.clone() };
    corrupt.functions[0].decl.locals[0].ty = tarn_types::Ty::Any(tarn_resolve::SymbolId(0));
    assert!(tarn_backend::verify_dynamic(&corrupt, t).is_err());
    let out = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(out.status.success());
    let trace: Vec<_> = String::from_utf8_lossy(&out.stderr).lines().filter_map(|s| s.strip_prefix("drop:").map(str::to_owned)).collect();
    assert_eq!(trace, ["second", "first"]);
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn static_bounds_do_not_instantiate_unused_interface_methods() {
    let source = "interface I {\n fn used(&self) i64\n fn unused(&self)\n}\nstruct S { n i64 }\nimpl I for S {\n fn used(&self) i64 { return self.n }\n fn unused(&self) { spawn print(0) }\n}\nfn call<T:I>(x &T) i64 { return x.used() }\nfn main() { s := S{n:42}\n print(call(&s)) }\n";
    let (exe, res) = compile(source, "static-reachability");
    let p = tarn_backend::mono::specialize(res.drops.as_ref().unwrap(), res.typed.as_ref().unwrap()).unwrap();
    assert!(p.functions.iter().all(|f| !f.decl.name.contains("unused")));
    assert!(
        p.functions
            .iter()
            .flat_map(|f| &f.blocks)
            .flat_map(|b| &b.stmts)
            .all(|s| !matches!(&s.op, tarn_ir::post_drop::Op::Plain(tarn_ir::StatementKind::Assign(_, tarn_ir::Rvalue::Coerce(tarn_ir::CoerceKind::DynTable { .. }, _, _)))))
    );
    assert_eq!(Command::new(&exe).output().unwrap().stdout, b"42\n");
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn owned_closure_environments_destroy_each_capture_once() {
    for (name, trace) in [
        ("owned_closures", "drop:hello\ndrop:second\ndrop:first\ndrop:unused\ndrop:nested\ndrop:generic\n"),
        ("closure_lifetimes", "drop:borrow\ndrop:replacement\n"),
        ("closure_aggregates", "drop:inside\ndrop:shared\ndrop:conditional\ndrop:field\ndrop:copy-field\n"),
        ("closure_reinitialize", "drop:old\ndrop:new\ndrop:new\n"),
        ("closure_callbacks", "drop:callback\ndrop:generic-environment\n"),
        ("closure_clone", "drop:clone\ndrop:clone\ndrop:clone\n"),
        ("closure_own_callable", "drop:nested-callable\n"),
        ("closure_partial", "drop:second-field\ndrop:first-field\ndrop:first-field\ndrop:second-field\ndrop:fallback\ndrop:first-field\ndrop:second-field\n"),
        ("closure_enum_arrays", "drop:enum-consuming\ndrop:array-first\ndrop:array-second\ndrop:enum\n"),
    ] {
        let path = Path::new("../../tests/native/pass").join(format!("{name}.tarn"));
        let (exe, _) = compile(&std::fs::read_to_string(&path).unwrap(), &format!("trace-{name}"));
        let output = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
        assert!(output.status.success(), "{name}");
        assert_eq!(String::from_utf8(output.stderr).unwrap(), trace, "{name}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), std::fs::read_to_string(path.with_extension("stdout")).unwrap());
        std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
    }
}

#[test]
fn invalid_closure_environment_metadata_is_rejected() {
    let path = Path::new("../../tests/native/pass/owned_closures.tarn");
    let result = tarn_driver::check(path).unwrap();
    assert!(!result.has_errors());
    let source = result.drops.as_ref().unwrap();
    let t = result.typed.as_ref().unwrap();
    for mutation in 0..5 {
        let mut p = tarn_ir::post_drop::Program { functions: source.functions.clone(), by_symbol: source.by_symbol.clone() };
        let f = p.functions.iter_mut().find(|f| matches!(f.decl.kind, tarn_ir::FnKind::Closure { owned: true, consumes: false, .. })).unwrap();
        let tarn_ir::FnKind::Closure { environment, captures, destructor, owned, .. } = &mut f.decl.kind else { unreachable!() };
        match mutation {
            0 => environment.clear(),
            1 => captures[0] = tarn_ir::CaptureMode::SharedBorrow,
            2 => *destructor = None,
            3 => *destructor = Some(tarn_ir::FunctionId(u32::MAX)),
            4 => *owned = false,
            _ => unreachable!(),
        }
        assert!(!tarn_ir::post_drop::verify(&p, t).is_empty(), "mutation {mutation}");
        assert!(tarn_backend::emit_object(&p, t).is_err(), "mutation {mutation}");
    }
}

#[test]
fn executable_publication_replaces_the_inode_before_launch() {
    let (exe, result) = compile("fn main() { print(42) }", "atomic-publication");
    // This deterministically reproduces the executable-busy hazard of copying
    // onto the destination inode. Publication must replace that inode instead.
    let writer = std::fs::OpenOptions::new().write(true).open(&exe).unwrap();
    tarn_backend::build(result.drops.as_ref().unwrap(), result.typed.as_ref().unwrap(), &exe).unwrap();
    let output = Command::new(&exe).env_remove("TARN_TRACE_DROPS").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    drop(writer);
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn native_tasks_destroy_captures_and_results_once_on_all_exit_paths() {
    let path = Path::new("../../tests/native/pass/task_completion.tarn");
    let (exe, _) = compile(&std::fs::read_to_string(path).unwrap(), "task-completion-trace");
    let output = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let drops = stderr.lines().map(|line| line.strip_prefix("drop:").expect("unexpected task stderr")).collect::<Vec<_>>();
    assert_eq!(drops, ["capture-normal", "unused-normal", "outer-normal", "capture-return", "unused-return",
        "unused-break", "unused-continue", "unused-continue", "unused-inner", "unused-outer", "unused-expression",
        "unused-overwrite", "joined-replacement", "unused-conditional"]);
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();

    let source = std::fs::read_to_string("../../tests/native/pass/tasks.tarn").unwrap();
    let (exe, _) = compile(&source, "task-owned-results-trace");
    let output = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let mut drops = stderr.lines().map(|line| line.strip_prefix("drop:").unwrap()).collect::<Vec<_>>();
    drops.sort();
    assert_eq!(drops, ["enum", "generic", "hello", "left", "right"]);
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn worker_panic_aborts_the_process_and_task_metadata_is_verified() {
    use std::os::unix::process::ExitStatusExt;
    let (exe, result) = compile("fn main() { t := spawn move fn() { panic(\"worker panic\") }\n t.join() }", "task-worker-panic");
    let output = Command::new(&exe).env_remove("TARN_TRACE_DROPS").output().unwrap();
    assert_eq!(output.status.signal(), Some(6));
    assert!(String::from_utf8_lossy(&output.stderr).contains("worker panic"));
    let source = result.drops.as_ref().unwrap();
    let typed = result.typed.as_ref().unwrap();
    for kind in 0..4 {
        let mut corrupted = tarn_ir::post_drop::Program { functions: source.functions.clone(), by_symbol: source.by_symbol.clone() };
        let block = corrupted.functions.iter_mut().flat_map(|f| &mut f.blocks).find(|b|
            matches!(b.term, tarn_ir::Terminator::Call { callee: tarn_ir::Callee::TaskSpawn { .. }, .. })).unwrap();
        if let tarn_ir::Terminator::Call { callee: tarn_ir::Callee::TaskSpawn { worker, drop_result, .. }, args, spawn, .. } = &mut block.term {
            match kind {
                0 => *worker = tarn_ir::FunctionId(u32::MAX),
                1 => *drop_result = *worker,
                2 => args.clear(),
                _ => *spawn = false,
            }
        }
        assert!(!tarn_ir::post_drop::verify(&corrupted, typed).is_empty());
        assert!(tarn_backend::emit_object(&corrupted, typed).unwrap_err().to_string().starts_with("compiler bug"));
    }
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}

#[test]
fn scoped_completion_destroys_results_before_borrowed_storage_and_verifies_witnesses() {
    let path = "../../tests/native/pass/scoped_task_completion.tarn";
    let (exe, result) = compile(&std::fs::read_to_string(path).unwrap(), "scoped-completion-trace");
    let output = Command::new(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "42\n");
    let expected = ["normal-result", "normal-parent", "return-result", "return-parent", "try-result", "try-parent",
        "continue-result", "continue-parent", "continue-result", "continue-parent", "break-result", "break-parent",
        "inner-result", "inner-parent", "outer-result", "outer-parent"];
    let trace: Vec<_> = String::from_utf8(output.stderr).unwrap().lines().map(str::to_owned).collect();
    assert_eq!(trace, expected.map(|s| format!("drop:{s}")));
    let source = result.drops.as_ref().unwrap();
    let typed = result.typed.as_ref().unwrap();
    for kind in 0..3 {
        let mut corrupted = tarn_ir::post_drop::Program { functions: source.functions.clone(), by_symbol: source.by_symbol.clone() };
        if kind == 0 {
            let local = corrupted.functions.iter_mut().flat_map(|f| &mut f.decl.locals)
                .find(|l| l.kind == tarn_ir::LocalKind::TaskScopeWitness).unwrap();
            local.kind = tarn_ir::LocalKind::User;
        } else {
            let block = corrupted.functions.iter_mut().flat_map(|f| &mut f.blocks).find(|b|
                matches!(b.term, tarn_ir::Terminator::Call { callee: tarn_ir::Callee::TaskSpawn { scoped: true, .. }, .. })).unwrap();
            if let tarn_ir::Terminator::Call { callee: tarn_ir::Callee::TaskSpawn { scoped, .. }, args, .. } = &mut block.term {
                if kind == 1 { *scoped = false; } else { args.pop(); }
            }
        }
        assert!(!tarn_ir::post_drop::verify(&corrupted, typed).is_empty());
        assert!(tarn_backend::emit_object(&corrupted, typed).is_err());
    }
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}
