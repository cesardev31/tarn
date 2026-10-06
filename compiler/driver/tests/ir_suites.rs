//! IR: snapshot suite `tests/ir/*.tarn` (`.ir` = printed IR, `.diag` =
//! warnings) and the structural verifier on every program that type-checks.

mod common;

use common::*;
use std::path::{Path, PathBuf};

fn print_ir(path: &Path) -> (Option<String>, String, Vec<String>) {
    let res = tarn_driver::check(path).unwrap();
    let map = &res.program.sources;
    let diags = res.diagnostics.iter().map(|d| d.render(map) + "\n").collect::<String>().replace(PREFIX, "");
    match (&res.resolved, &res.typed, &res.ir) {
        (Some(r), Some(t), Some(ir)) => (Some(tarn_ir::print_program(ir, r, t)), diags, tarn_ir::verify(ir)),
        _ => (None, diags, Vec::new()),
    }
}

#[test]
fn ir_snapshot_suite() {
    let files = entries("ir", "pass");
    assert!(files.len() >= 8, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let (ir, diags, verify) = print_ir(f);
        let Some(ir) = ir else {
            failures.push(format!("{} did not reach IR:\n{diags}", f.display()));
            continue;
        };
        if !verify.is_empty() {
            failures.push(format!("{} fails the IR verifier:\n{}", f.display(), verify.join("\n")));
        }
        golden(f, "ir", &ir, &mut failures);
        golden(f, "diag", &diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every program in the repository that type-checks lowers to valid IR.
#[test]
fn every_checked_program_lowers_to_valid_ir() {
    let mut files: Vec<PathBuf> = Vec::new();
    for dir in ["examples", "tests/types/pass", "tests/resolve/pass"] {
        for e in std::fs::read_dir(Path::new(PREFIX).join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                files.push(p.join("main.tarn"));
            } else if p.extension().is_some_and(|e| e == "tarn") {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut lowered = 0;
    let mut failures = Vec::new();
    for f in &files {
        let (ir, _, verify) = print_ir(f);
        if ir.is_some() {
            lowered += 1;
        }
        if !verify.is_empty() {
            failures.push(format!("{}:\n{}", f.display(), verify.join("\n")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(lowered >= 40, "only {lowered} programs reached the IR");
}

/// Scope completion must dominate destruction of storage owned by that scope,
/// including the lowering's early-exit cleanup paths.
#[test]
fn scope_completion_precedes_local_destruction_on_all_exit_kinds() {
    use tarn_ir::{Builtin, Callee, StatementKind, Terminator};
    let path = Path::new(PREFIX).join("tests/ir/pass/scope_exit_join.tarn");
    let result = tarn_driver::check(&path).unwrap();
    assert!(!result.has_errors());
    let program = result.ir.as_ref().unwrap();
    for name in ["normal_exit", "return_exit", "break_exit", "continue_exit", "nested_return"] {
        let f = program.functions.iter().find(|f| f.name == name).unwrap();
        let mut pending = vec![(0usize, 0usize)];
        let mut visited = std::collections::HashSet::new();
        let mut drops = 0;
        while let Some((block, joins)) = pending.pop() {
            if !visited.insert((block, joins.min(2))) { continue; }
            let b = &f.blocks[block];
            for stmt in &b.stmts {
                if let StatementKind::Drop(place) = &stmt.kind {
                    let local = f.local(place.local);
                    if local.name.as_deref() == Some("kept") {
                        assert!(joins >= 1, "{name}: local destruction before join");
                        drops += 1;
                    }
                    if local.name.as_deref() == Some("outer") {
                        assert!(joins >= 2, "nested return must join both scopes");
                    }
                }
            }
            let joins = joins + usize::from(matches!(b.term,
                Terminator::Call { callee: Callee::Builtin(Builtin::JoinScope), .. }));
            match &b.term {
                Terminator::Goto(to) => pending.push((to.0 as usize, joins)),
                Terminator::Call { next: Some(to), .. } => pending.push((to.0 as usize, joins)),
                Terminator::Switch { cases, otherwise, .. } => {
                    for (_, to) in cases { pending.push((to.0 as usize, joins)); }
                    pending.push((otherwise.0 as usize, joins));
                }
                _ => {}
            }
        }
        assert!(drops > 0, "{name}: missing scoped destruction");
    }
}
