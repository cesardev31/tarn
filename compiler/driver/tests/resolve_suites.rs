//! Name-resolution suites.
//!
//! Entries are `tests/resolve/{pass,fail}/*.tarn` and `*/main.tarn` (a
//! directory is a multi-module project). Expected files sit next to the entry:
//!
//! - pass: no errors; `.res` = resolution dump; `.diag` = warnings (if any).
//! - fail: at least one error; `.diag` = all diagnostics.
//!
//! `TARN_BLESS=1 cargo test` rewrites the expected files.

use std::path::{Path, PathBuf};

const PREFIX: &str = "../../";

fn entries(kind: &str) -> Vec<PathBuf> {
    let dir = Path::new(PREFIX).join("tests/resolve").join(kind);
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.push(p.join("main.tarn"));
        } else if p.extension().is_some_and(|e| e == "tarn") {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn golden(entry: &Path, ext: &str, actual: &str, failures: &mut Vec<String>) {
    let path = entry.with_extension(ext);
    if std::env::var_os("TARN_BLESS").is_some() {
        if actual.is_empty() {
            let _ = std::fs::remove_file(&path);
        } else {
            std::fs::write(&path, actual).unwrap();
        }
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    if expected != actual {
        failures.push(format!("{}\n--- expected\n{expected}--- actual\n{actual}", path.display()));
    }
}

fn run(entry: &Path) -> (String, String, bool) {
    let res = tarn_driver::check(entry).unwrap();
    let map = &res.program.sources;
    let diags: String = res.diagnostics.iter().map(|d| d.render(map) + "\n").collect::<String>().replace(PREFIX, "");
    let dump = res.resolved.as_ref().map(|r| tarn_resolve::dump_resolution(r, map)).unwrap_or_default();
    (dump, diags, res.has_errors())
}

#[test]
fn resolve_pass_suite() {
    let files = entries("pass");
    assert!(files.len() >= 10, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let (dump, diags, errors) = run(f);
        if errors {
            failures.push(format!("{} should resolve without errors:\n{diags}", f.display()));
            continue;
        }
        golden(f, "res", &dump, &mut failures);
        golden(f, "diag", &diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn resolve_fail_suite() {
    let files = entries("fail");
    assert!(files.len() >= 15, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let (_, diags, errors) = run(f);
        if !errors {
            failures.push(format!("{} should fail to resolve", f.display()));
            continue;
        }
        golden(f, "diag", &diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every example resolves without errors; the only warnings are the ones the
/// examples document on purpose.
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
                failures.push(d.render(&res.program.sources));
            } else {
                warnings.push(line);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(warnings, ["27_shadowing.tarn W2001"]);
}
