//! Name-resolution suites.
//!
//! Entries are `tests/resolve/{pass,fail}/*.tarn` and `*/main.tarn` (a
//! directory is a multi-module project). Expected files sit next to the entry:
//!
//! - pass: no errors; `.res` = resolution dump; `.diag` = warnings (if any).
//! - fail: at least one error; `.diag` = all diagnostics.
//!
//! `TARN_BLESS=1 cargo test` rewrites the expected files.

mod common;

use common::*;
use std::path::{Path, PathBuf};

#[test]
fn resolve_pass_suite() {
    let files = entries("resolve", "pass");
    assert!(files.len() >= 10, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        // Type errors are fine here: this suite is about names.
        if o.codes.iter().any(|c| c.starts_with("E1") || c.starts_with("E2")) {
            failures.push(format!("{} should resolve without errors:\n{}", f.display(), o.diags));
            continue;
        }
        golden(f, "res", &o.resolution, &mut failures);
        let name_diags: String = o.diags.split("\n\n").filter(|d| d.contains("[E2") || d.contains("[W2")).map(|d| format!("{d}\n\n")).collect();
        golden(f, "diag", &name_diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn resolve_fail_suite() {
    let files = entries("resolve", "fail");
    assert!(files.len() >= 15, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        if !o.codes.iter().any(|c| c.starts_with("E2")) {
            failures.push(format!("{} should fail to resolve", f.display()));
            continue;
        }
        golden(f, "diag", &o.diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Example depending on filesystem APIs reserved for phase 15B.
const NEEDS_STDLIB: &[&str] = &["12_try.tarn"];

/// Every example checks without errors (except missing stdlib methods in the
/// examples listed above); the only warnings are the documented ones.
#[test]
fn examples_resolve() {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let dir = Path::new(PREFIX).join("examples");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| if p.is_dir() { p.join("main.tarn") } else { p })
        .filter(|p| p.extension().is_some_and(|e| e == "tarn"))
        .collect();
    files.sort();
    for f in &files {
        let res = tarn_driver::check(f).unwrap();
        for d in &res.diagnostics {
            let line = format!("{} {}", f.file_name().unwrap().to_string_lossy(), d.code);
            if d.severity == tarn_diagnostics::Severity::Error {
                // Examples that need the standard library (not built yet).
                if d.code == "E3005" && NEEDS_STDLIB.iter().any(|n| line.starts_with(n)) {
                    continue;
                }
                // Bootstrap examples are now explicitly rejected, rather than
                // accepted through opaque ownership/provenance assumptions.
                if d.code == "E3040" && ["12_try.tarn", "23_filesystem.tarn", "24_process.tarn", "25_concurrency.tarn"].iter().any(|n| line.starts_with(n)) {
                    continue;
                }
                failures.push(d.render(&res.program.sources));
            } else {
                warnings.push(line);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(warnings, ["27_shadowing.tarn W2001"]);
}
