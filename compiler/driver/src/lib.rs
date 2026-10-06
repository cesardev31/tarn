//! Compiler driver: loads a program from disk and runs the frontend phases.
//!
//! Module loading (ADR 0004): the entry file's directory is the project root;
//! `import "a/b"` loads `<root>/a/b.tarn` if it exists. Imports that are not
//! local files are left to the resolver (standard modules or E2007).

use std::path::{Path, PathBuf};
use tarn_ast::{ItemKind, Module};
use tarn_diagnostics::{Diagnostic, Severity, SourceMap};
use tarn_resolve::{ModuleInput, Resolved};

pub struct Program {
    pub sources: SourceMap,
    /// (module name, AST); the entry module is first.
    pub modules: Vec<(String, Module)>,
}

pub struct CheckResult {
    pub program: Program,
    /// `None` when the parse had errors (resolution is skipped).
    pub resolved: Option<Resolved>,
    /// Sorted by file, then position.
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

/// Load the entry file and every local module it transitively imports.
pub fn load(entry: &Path) -> Result<(Program, Vec<Diagnostic>), String> {
    let root = entry.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let stem = entry.file_stem().and_then(|s| s.to_str()).ok_or("invalid file name")?.to_string();
    let mut sources = SourceMap::new();
    let mut modules: Vec<(String, Module)> = Vec::new();
    let mut diags = Vec::new();
    let mut queue = vec![(stem, entry.to_path_buf())];
    while let Some((name, path)) = queue.pop() {
        if modules.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
        let display = path.strip_prefix(".").unwrap_or(&path).display().to_string();
        let id = sources.add(display, text);
        let res = tarn_parser::parse_file(id, sources.file(id));
        diags.extend(res.diagnostics);
        for item in &res.module.items {
            if let ItemKind::Import(imp) = &item.kind {
                let file = root.join(format!("{}.tarn", imp.path));
                let valid = imp.path.split('/').all(|s| !s.is_empty() && s != "." && s != "..");
                if valid && file.is_file() {
                    queue.push((imp.path.clone(), file));
                }
            }
        }
        modules.push((name, res.module));
    }
    Ok((Program { sources, modules }, diags))
}

/// Lex, parse and resolve. Resolution only runs on syntactically valid
/// programs so that recovery artifacts never produce name errors.
pub fn check(entry: &Path) -> Result<CheckResult, String> {
    let (program, mut diagnostics) = load(entry)?;
    let mut resolved = None;
    if !diagnostics.iter().any(|d| d.severity == Severity::Error) {
        let inputs: Vec<ModuleInput> = program.modules.iter().map(|(n, m)| ModuleInput { name: n.clone(), ast: m }).collect();
        let (r, d) = tarn_resolve::resolve(&inputs);
        diagnostics.extend(d);
        resolved = Some(r);
    }
    diagnostics.sort_by_key(|d| d.primary_span().map(|s| (s.file, s.start)));
    Ok(CheckResult { program, resolved, diagnostics })
}
