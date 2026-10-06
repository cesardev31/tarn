//! Compiler driver: loads a program from disk and runs the frontend phases.
//!
//! Module loading (ADR 0004): the entry file's directory is the project root;
//! `import "a/b"` loads `<root>/a/b.tarn` if it exists. Imports that are not
//! local files are left to the resolver (standard modules or E2007).

use std::path::{Path, PathBuf};
use tarn_ast::{ItemKind, Module};
use tarn_diagnostics::{Diagnostic, Severity, SourceMap};
use tarn_resolve::{ModuleInput, Resolved};

const STRING_SOURCE: &str = include_str!("../../../stdlib/string/string.tarn");

const CORE_SOURCE: &str = include_str!("../../../stdlib/core/core.tarn");

pub struct Program {
    pub sources: SourceMap,
    /// (module name, AST); the entry module is first.
    pub modules: Vec<(String, Module)>,
}

pub struct CheckResult {
    pub program: Program,
    /// `None` when the parse had errors (resolution is skipped).
    pub resolved: Option<Resolved>,
    /// `None` when parsing or resolution had errors.
    pub typed: Option<tarn_types::Typed>,
    /// `None` when any earlier phase had errors.
    pub ir: Option<tarn_ir::Program>,
    /// Move/init checking results (drop elaboration input).
    pub moves: Option<tarn_ownership::MoveResults>,
    /// Borrow checking results (loans, provenance).
    pub borrows: Option<tarn_ownership::BorrowResults>,
    /// Self-contained executable destruction IR, only after successful ownership checking.
    pub drops: Option<tarn_ir::post_drop::Program>,
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
    load_with_overlays(entry, &std::collections::HashMap::new())
}

/// Read open editor buffers before falling back to disk. Paths must be absolute.
pub fn load_with_overlays(entry: &Path, overlays: &std::collections::HashMap<PathBuf, String>) -> Result<(Program, Vec<Diagnostic>), String> {
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
        let text = match overlays.get(&path) {
            Some(text) => text.clone(),
            None if name == "string" && !path.is_file() => STRING_SOURCE.to_string(),
            None => std::fs::read_to_string(&path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?,
        };
        let display = path.strip_prefix(".").unwrap_or(&path).display().to_string();
        let id = sources.add(display, text);
        let res = tarn_parser::parse_file(id, sources.file(id));
        diags.extend(res.diagnostics);
        for item in &res.module.items {
            if let ItemKind::Import(imp) = &item.kind {
                let file = root.join(format!("{}.tarn", imp.path));
                let valid = imp.path.split('/').all(|s| !s.is_empty() && s != "." && s != "..");
                if valid && (file.is_file() || overlays.contains_key(&file) || imp.path == "string") {
                    queue.push((imp.path.clone(), file));
                }
            }
        }
        modules.push((name, res.module));
    }
    // `core` is always part of the program (ADR 0020). Embedded in the
    // binary so the compiler is self-contained.
    if !modules.iter().any(|(n, _)| n == "core") {
        let id = sources.add("stdlib/core/core.tarn", CORE_SOURCE);
        let res = tarn_parser::parse_file(id, sources.file(id));
        diags.extend(res.diagnostics);
        modules.push(("core".to_string(), res.module));
    }
    Ok((Program { sources, modules }, diags))
}

/// Lex, parse and resolve. Resolution only runs on syntactically valid
/// programs so that recovery artifacts never produce name errors.
pub fn check(entry: &Path) -> Result<CheckResult, String> {
    check_with_overlays(entry, &std::collections::HashMap::new())
}

/// Check a program including unsaved buffers of imported modules.
pub fn check_with_overlays(entry: &Path, overlays: &std::collections::HashMap<PathBuf, String>) -> Result<CheckResult, String> {
    let (program, mut diagnostics) = load_with_overlays(entry, overlays)?;
    let mut resolved = None;
    let mut typed = None;
    let mut ir = None;
    let mut moves = None;
    let mut borrows = None;
    let mut drops = None;
    let errors = |ds: &[Diagnostic]| ds.iter().any(|d| d.severity == Severity::Error);
    if !errors(&diagnostics) {
        let inputs: Vec<ModuleInput> = program.modules.iter().map(|(n, m)| ModuleInput { name: n.clone(), ast: m }).collect();
        let (r, d) = tarn_resolve::resolve(&inputs);
        diagnostics.extend(d);
        // Types only on name-clean programs, for the same reason as above.
        if !errors(&diagnostics) {
            let (t, d) = tarn_types::check(&inputs, &r);
            diagnostics.extend(d);
            if !errors(&diagnostics) {
                let (p, d) = tarn_ir::lower_program(&inputs, &r, &t);
                diagnostics.extend(d);
                let (m, d) = tarn_ownership::check_moves(&p, &r, &t);
                diagnostics.extend(d);
                let (bo, d) = tarn_ownership::check_borrows(&p, &r, &t, &m.failed());
                diagnostics.extend(d);
                if !errors(&diagnostics) {
                    drops = Some(tarn_ownership::elaborate_drops(&p, &t, &m).map_err(|bugs| format!("compiler bug in drop elaboration:\n{}", bugs.join("\n")))?);
                }
                borrows = Some(bo);
                moves = Some(m);
                ir = Some(p);
            }
            typed = Some(t);
        }
        resolved = Some(r);
    }
    diagnostics.sort_by_key(|d| d.primary_span().map(|s| (s.file, s.start)));
    Ok(CheckResult { program, resolved, typed, ir, moves, borrows, drops, diagnostics })
}
