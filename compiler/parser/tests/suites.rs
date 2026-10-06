//! File-based suites:
//!
//! - `tests/parser/pass/*.tarn` must parse without diagnostics; the AST dump
//!   must match the sibling `.ast` file.
//! - `tests/parser/fail/*.tarn` must produce diagnostics; their rendering must
//!   match the sibling `.diag` file.
//! - every `examples/**/*.tarn` must parse without diagnostics.
//!
//! `TARN_BLESS=1 cargo test` rewrites the expected files.

use std::path::{Path, PathBuf};
use tarn_diagnostics::SourceMap;
use tarn_parser::parse_file;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tarn_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "tarn") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn run(path: &Path) -> (String, String, usize) {
    let rel = path.strip_prefix(root()).unwrap_or(path).to_string_lossy().replace("../", "");
    let mut map = SourceMap::new();
    let id = map.add(rel, std::fs::read_to_string(path).unwrap());
    let res = parse_file(id, map.file(id));
    let diags: String = res.diagnostics.iter().map(|d| d.render(&map) + "\n").collect();
    (tarn_ast::dump_module(&res.module), diags, res.diagnostics.len())
}

fn check_golden(path: &Path, ext: &str, actual: &str, failures: &mut Vec<String>) {
    let expected_path = path.with_extension(ext);
    if std::env::var_os("TARN_BLESS").is_some() {
        std::fs::write(&expected_path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
    if expected != actual {
        failures.push(format!("{}\n--- expected\n{expected}--- actual\n{actual}", expected_path.display()));
    }
}

#[test]
fn parser_pass_suite() {
    let files = tarn_files(&root().join("tests/parser/pass"));
    assert!(files.len() >= 10, "pass suite too small: {}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let (ast, diags, n) = run(f);
        if n > 0 {
            failures.push(format!("{} should parse cleanly:\n{diags}", f.display()));
            continue;
        }
        check_golden(f, "ast", &ast, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn parser_fail_suite() {
    let files = tarn_files(&root().join("tests/parser/fail"));
    assert!(files.len() >= 10, "fail suite too small: {}", files.len());
    let mut failures = Vec::new();
    for f in &files {
        let (_, diags, n) = run(f);
        if n == 0 {
            failures.push(format!("{} should fail to parse", f.display()));
            continue;
        }
        check_golden(f, "diag", &diags, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn all_examples_parse() {
    let files = tarn_files(&root().join("examples"));
    assert!(files.len() >= 28, "found {} examples", files.len());
    let failures: Vec<_> = files
        .iter()
        .filter_map(|f| {
            let (_, diags, n) = run(f);
            (n > 0).then(|| format!("{}:\n{diags}", f.display()))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every truncation of every example must parse (with errors) without
/// panicking or looping: exercises recovery at arbitrary cut points.
#[test]
fn truncated_examples_never_panic() {
    for f in tarn_files(&root().join("examples")) {
        let src = std::fs::read_to_string(&f).unwrap();
        for cut in (0..src.len()).filter(|&i| src.is_char_boundary(i)).step_by(3) {
            let mut map = SourceMap::new();
            let id = map.add("t.tarn", &src[..cut]);
            let res = parse_file(id, map.file(id));
            let _ = tarn_ast::dump_module(&res.module);
            for d in &res.diagnostics {
                let _ = d.render(&map);
            }
        }
    }
}
