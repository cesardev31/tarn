//! Move/init checking suites: `tests/moves/{pass,fail}`.
//!
//! - pass: no errors; `.drops` lists the drop-elaboration decision of every
//!   `Drop` (function, block, place, decision).
//! - fail: at least one `E4xxx`; `.diag` = all diagnostics; the file name
//!   starts with the code it is about.

mod common;

use common::*;
use std::path::Path;

fn drops(path: &Path) -> String {
    let res = tarn_driver::check(path).unwrap();
    let (Some(r), Some(t), Some(ir), Some(m)) = (&res.resolved, &res.typed, &res.ir, &res.moves) else { return String::new() };
    let text = only_functions(&tarn_ir::print_program_annotated(ir, r, t, &m.drop_notes()), &relevant_functions(ir, r));
    let mut out = String::new();
    let mut func = String::new();
    let mut block = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("fn ") {
            func = rest.split('(').next().unwrap_or("").to_string();
        } else if line.starts_with("  bb") {
            block = line.trim().trim_end_matches(':').to_string();
        } else if let Some((stmt, note)) = line.trim().split_once("    // drop: ") {
            out.push_str(&format!("{func} {block}: {stmt} -> {note}\n"));
        }
    }
    out
}

#[test]
fn moves_pass_suite() {
    let files = entries("moves", "pass");
    assert!(files.len() >= 8, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        if o.errors {
            failures.push(format!("{} should pass move checking:\n{}", f.display(), o.diags));
            continue;
        }
        golden(f, "drops", &drops(f), &mut failures);
        golden(f, "diag", &o.diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn moves_fail_suite() {
    let files = entries("moves", "fail");
    assert!(files.len() >= 10, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let code = &name[..5];
        if !o.codes.contains(&code) {
            failures.push(format!("{} should report {code}, got {:?}\n{}", f.display(), o.codes, o.diags));
            continue;
        }
        golden(f, "diag", &o.diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
