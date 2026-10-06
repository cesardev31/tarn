//! Type-checking suites: `tests/types/{pass,fail}`.
//!
//! - pass: no errors; `.types` = type of every local/parameter; `.diag` = warnings.
//! - fail: at least one `E3xxx`; `.diag` = all diagnostics.

mod common;

use common::*;

#[test]
fn types_pass_suite() {
    let files = entries("types", "pass");
    assert!(files.len() >= 8, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        if o.errors {
            failures.push(format!("{} should type-check:\n{}", f.display(), o.diags));
            continue;
        }
        golden(f, "types", &o.types, &mut failures);
        golden(f, "diag", &o.diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn types_fail_suite() {
    let files = entries("types", "fail");
    assert!(files.len() >= 25, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        if !o.codes.iter().any(|c| c.starts_with("E3") || *c == "E4203") {
            failures.push(format!("{} should fail type checking, got {:?}", f.display(), o.codes));
            continue;
        }
        // The file name starts with the code it is about.
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let expected_code = &name[..5];
        if f.ends_with("main.tarn") || !o.codes.contains(&expected_code) {
            if !f.ends_with("main.tarn") {
                failures.push(format!("{} should report {expected_code}, got {:?}", f.display(), o.codes));
            }
        }
        golden(f, "diag", &o.diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
