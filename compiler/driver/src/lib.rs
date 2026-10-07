//! Compiler driver: loads a program from disk and runs the frontend phases.
//!
//! Module loading (ADR 0004): the entry file's directory is the project root;
//! `import "a/b"` loads `<root>/a/b.tarn` if it exists. Imports that are not
//! local files are left to the resolver (standard modules or E2007).

use std::path::{Path, PathBuf};
use tarn_ast::{ItemKind, Module};
use tarn_diagnostics::{Diagnostic, Severity, SourceMap};
use tarn_resolve::{ModuleInput, Resolved};

const HTTP_SOURCE: &str = include_str!("../../../stdlib/http/http.tarn");

const PATH_SOURCE: &str = include_str!("../../../stdlib/path/path.tarn");

const STRING_SOURCE: &str = include_str!("../../../stdlib/string/string.tarn");

const JSON_SOURCE: &str = include_str!("../../../stdlib/json/json.tarn");

const CORE_SOURCE: &str = include_str!("../../../stdlib/core/core.tarn");

/// Trusted standard-library modules (ADR 0041), embedded so the compiler is
/// self-contained. Only these may declare intrinsics, and only their sources
/// carry trusted semantic contracts. Entry files never acquire these names.
const TRUSTED_STDLIB: &[(&str, &str)] = &[
    ("core", CORE_SOURCE),
    ("io", include_str!("../../../stdlib/io/io.tarn")),
    ("time", include_str!("../../../stdlib/time/time.tarn")),
    ("runtime", include_str!("../../../stdlib/runtime/runtime.tarn")),
    ("net", include_str!("../../../stdlib/net/net.tarn")),
    ("fs", include_str!("../../../stdlib/fs/fs.tarn")),
    ("process", include_str!("../../../stdlib/process/process.tarn")),
    ("ffi", include_str!("../../../stdlib/ffi/ffi.tarn")),
];

fn trusted_source(name: &str) -> Option<&'static str> {
    TRUSTED_STDLIB.iter().find(|(n, _)| *n == name).map(|(_, source)| *source)
}

fn official_module(name: &str) -> bool { (name == "string" || name == "path" || name == "http" || name == "json") || trusted_source(name).is_some() }

pub struct Program {
    pub sources: SourceMap,
    /// Files read from disk during loading; embedded stdlib sources are excluded.
    pub disk_sources: Vec<PathBuf>,
    /// (module name, AST); the entry module is first.
    pub modules: Vec<(String, Module)>,
    /// Embedded source provenance retained for semantic contracts.
    pub trusted_modules: std::collections::HashSet<String>,
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
    load_editor_sources(entry, overlays, false)
}

fn official_stdlib_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../stdlib/{name}/{name}.tarn"))
}

fn load_editor_sources(entry: &Path, overlays: &std::collections::HashMap<PathBuf, String>, editor: bool) -> Result<(Program, Vec<Diagnostic>), String> {
    let official_entry = if editor {
        entry.canonicalize().ok().and_then(|entry| TRUSTED_STDLIB.iter().map(|(name, _)| *name).chain(["string", "path", "http", "json"]).find(|name|
            official_stdlib_path(name).canonicalize().ok().as_ref() == Some(&entry)))
    } else { None };
    let root = entry.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let stem = entry.file_stem().and_then(|s| s.to_str()).ok_or("invalid file name")?.to_string();
    // Entry filenames do not confer reserved stdlib module identity.
    let stem = if let Some(name) = official_entry { name.to_string() }
        else if trusted_source(&stem).is_some() { format!("entry/{stem}") } else { stem };
    let mut sources = SourceMap::new();
    let mut disk_sources = Vec::new();
    let mut modules: Vec<(String, Module)> = Vec::new();
    let mut diags = Vec::new();
    let mut trusted_modules = std::collections::HashSet::new();
    let mut queue = vec![(stem, entry.to_path_buf())];
    while let Some((name, path)) = queue.pop() {
        if modules.iter().any(|(n, _)| *n == name) {
            continue;
        }
        let official_path = official_stdlib_path(&name).canonicalize().ok();
        let editor_text = if editor && (path != entry || official_entry == Some(name.as_str())) && official_module(&name) {
            official_path.as_ref().and_then(|p| overlays.get(p)).cloned()
        } else { None };
        let has_editor_text = editor_text.is_some();
        let text = if let Some(text) = editor_text {
            if trusted_source(&name).is_some() { trusted_modules.insert(name.clone()); }
            text
        } else if let Some(source) = trusted_source(&name) {
            trusted_modules.insert(name.clone());
            source.to_string()
        } else {
            match overlays.get(&path) {
                Some(text) => text.clone(),
                None if name == "string" && !path.is_file() => STRING_SOURCE.to_string(),
                None if name == "path" && !path.is_file() => PATH_SOURCE.to_string(),
                None if name == "http" && !path.is_file() => HTTP_SOURCE.to_string(),
                None if name == "json" && !path.is_file() => JSON_SOURCE.to_string(),
                None => {
                    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
                    disk_sources.push(path.clone());
                    text
                },
            }
        };
        let display = if editor && (official_entry == Some(name.as_str()) || has_editor_text) {
            official_path.as_ref().unwrap_or(&path).display().to_string()
        } else if trusted_modules.contains(&name) { format!("stdlib/{name}/{name}.tarn") } else { path.strip_prefix(".").unwrap_or(&path).display().to_string() };
        let id = sources.add(display, text);
        let res = tarn_parser::parse_file(id, sources.file(id));
        diags.extend(res.diagnostics);
        for item in &res.module.items {
            // Async syntax depends on the validated execution declarations,
            // even when application source does not explicitly import net.
            let needs_execution = match &item.kind {
                ItemKind::Fn(function) => function.is_async,
                ItemKind::Impl(implementation) => implementation.methods.iter().any(|f| f.is_async),
                _ => false,
            };
            if needs_execution {
                queue.push(("runtime".to_string(), root.join("runtime.tarn")));
            }
            if let ItemKind::Import(imp) = &item.kind {
                let file = root.join(format!("{}.tarn", imp.path));
                let valid = imp.path.split('/').all(|s| !s.is_empty() && s != "." && s != "..");
                if valid && (file.is_file() || overlays.contains_key(&file) || official_module(&imp.path)) {
                    queue.push((imp.path.clone(), file));
                }
            }
        }
        modules.push((name, res.module));
    }
    // `core` is always part of the program (ADR 0020). Embedded in the
    // binary so the compiler is self-contained.
    if !modules.iter().any(|(n, _)| n == "core") {
        let official_core = official_stdlib_path("core").canonicalize().ok();
        let buffer = if editor { official_core.as_ref().and_then(|p| overlays.get(p)) } else { None };
        let display = if buffer.is_some() { official_core.as_ref().unwrap().display().to_string() } else { "stdlib/core/core.tarn".to_string() };
        let id = sources.add(display, buffer.map(String::as_str).unwrap_or(CORE_SOURCE));
        let res = tarn_parser::parse_file(id, sources.file(id));
        diags.extend(res.diagnostics);
        modules.push(("core".to_string(), res.module));
        trusted_modules.insert("core".to_string());
    }
    Ok((Program { sources, disk_sources, modules, trusted_modules }, diags))
}

/// Lex, parse and resolve. Resolution only runs on syntactically valid
/// programs so that recovery artifacts never produce name errors.
pub fn check(entry: &Path) -> Result<CheckResult, String> {
    check_with_overlays(entry, &std::collections::HashMap::new())
}

/// Check a program including unsaved buffers of imported modules.
pub fn check_with_overlays(entry: &Path, overlays: &std::collections::HashMap<PathBuf, String>) -> Result<CheckResult, String> {
    check_loaded(load_with_overlays(entry, overlays)?)
}

/// Editor-only checking of this checkout's official stdlib source buffers.
/// Normal CLI entry files never gain intrinsic authority from their path/name.
pub fn check_editor_with_overlays(entry: &Path, overlays: &std::collections::HashMap<PathBuf, String>) -> Result<CheckResult, String> {
    check_loaded(load_editor_sources(entry, overlays, true)?)
}

fn check_loaded((program, mut diagnostics): (Program, Vec<Diagnostic>)) -> Result<CheckResult, String> {
    let mut resolved = None;
    let mut typed = None;
    let mut ir = None;
    let mut moves = None;
    let mut borrows = None;
    let mut drops = None;
    let errors = |ds: &[Diagnostic]| ds.iter().any(|d| d.severity == Severity::Error);
    if !errors(&diagnostics) {
        let inputs: Vec<ModuleInput> = program.modules.iter().map(|(n, m)| ModuleInput { name: n.clone(), ast: m, trusted_stdlib: program.trusted_modules.contains(n) }).collect();
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
