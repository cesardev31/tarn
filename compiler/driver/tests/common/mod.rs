//! Shared helpers for the file-based suites.
//!
//! Entries: `tests/<phase>/{pass,fail}/*.tarn` and `*/main.tarn` (a directory
//! is a multi-module project). Expected files sit next to the entry.
//! `TARN_BLESS=1 cargo test` rewrites them.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub const PREFIX: &str = "../../";

pub fn entries(phase: &str, kind: &str) -> Vec<PathBuf> {
    let dir = Path::new(PREFIX).join("tests").join(phase).join(kind);
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

pub fn golden(entry: &Path, ext: &str, actual: &str, failures: &mut Vec<String>) {
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

pub struct Outcome {
    pub resolution: String,
    pub types: String,
    pub diags: String,
    pub errors: bool,
    /// Codes of all diagnostics, in order.
    pub codes: Vec<&'static str>,
}

pub fn run(entry: &Path) -> Outcome {
    let res = tarn_driver::check(entry).unwrap();
    let map = &res.program.sources;
    let diags = res.diagnostics.iter().map(|d| d.render(map) + "\n").collect::<String>().replace(PREFIX, "");
    let resolution = res.resolved.as_ref().map(|r| tarn_resolve::dump_resolution(r, map)).unwrap_or_default();
    let types = match (&res.resolved, &res.typed) {
        (Some(r), Some(t)) => tarn_types::dump_types(t, r, map),
        _ => String::new(),
    };
    Outcome { resolution, types, diags, errors: res.has_errors(), codes: res.diagnostics.iter().map(|d| d.code).collect() }
}
