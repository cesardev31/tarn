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

/// Standard-library modules whose function bodies appear in snapshots only
/// when the program reaches them (Phase 27 maintenance): an unused `core`
/// combinator or `string` helper must not churn every golden file.
const STDLIB_MODULES: &[&str] = &["core", "io", "time", "net", "runtime", "fs", "process", "ffi", "string", "path", "http", "json", "os", "collections", "console", "https"];

/// Names of the functions worth printing: every function of an application
/// module, and standard-library or generated functions reachable from one.
/// References are found through `FunctionId(n)` in the IR's Debug form, which
/// covers calls, function values, closures, environment destructors and tasks.
#[allow(dead_code)]
pub fn relevant_functions(ir: &tarn_ir::Program, r: &tarn_resolve::Resolved) -> std::collections::HashSet<String> {
    let application = |f: &tarn_ir::Function| {
        f.symbol
            .and_then(|s| r.symbol(s).module)
            .is_some_and(|m| !STDLIB_MODULES.contains(&r.modules[m.0 as usize].name.as_str()))
    };
    let mut kept = vec![false; ir.functions.len()];
    let mut work: Vec<usize> = (0..ir.functions.len()).filter(|&i| application(&ir.functions[i])).collect();
    while let Some(i) = work.pop() {
        if std::mem::replace(&mut kept[i], true) {
            continue;
        }
        let f = &ir.functions[i];
        let text = format!("{:?}{:?}", f.kind, f.blocks);
        for part in text.split("FunctionId(").skip(1) {
            if let Some(id) = part.split(')').next().and_then(|n| n.parse::<usize>().ok())
                && id < kept.len()
                && !kept[id]
            {
                work.push(id);
            }
        }
    }
    ir.functions.iter().zip(kept).filter(|(_, k)| *k).map(|(f, _)| f.name.clone()).collect()
}

/// Keep only the printed functions in `keep`: a function starts at a line
/// `fn <name>(` or `fn <name> [` and runs until the next one.
#[allow(dead_code)]
pub fn only_functions(text: &str, keep: &std::collections::HashSet<String>) -> String {
    let mut out = String::new();
    let mut keeping = true;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("fn ") {
            let name = rest.split(['(', ' ']).next().unwrap_or("");
            keeping = keep.contains(name);
        }
        if keeping {
            out.push_str(line);
        }
    }
    out
}
