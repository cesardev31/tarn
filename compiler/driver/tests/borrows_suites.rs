//! Borrow checking suites: `tests/borrows/{pass,fail}`.
//!
//! - pass: no errors; `.prov` = inferred provenance of every function that
//!   returns a reference (what its result may borrow from).
//! - fail: the file name starts with the expected code; `.diag` = output.

mod common;

use common::*;

#[test]
fn borrows_pass_suite() {
    let files = entries("borrows", "pass");
    assert!(files.len() >= 12, "{}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let o = run(f);
        if o.errors {
            failures.push(format!("{} should pass borrow checking:\n{}", f.display(), o.diags));
            continue;
        }
        let res = tarn_driver::check(f).unwrap();
        let prov = match (&res.ir, &res.borrows, &res.resolved) {
            (Some(ir), Some(b), Some(r)) => {
                let keep = relevant_functions(ir, r);
                b.provenance_lines(ir).into_iter().filter(|l| keep.contains(l.split(" -> ").next().unwrap_or(""))).map(|l| l + "\n").collect::<String>()
            }
            _ => String::new(),
        };
        golden(f, "prov", &prov, &mut failures);
        golden(f, "diag", &o.diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn borrows_fail_suite() {
    let files = entries("borrows", "fail");
    assert!(files.len() >= 14, "{}", files.len());
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
