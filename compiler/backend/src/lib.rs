//! Ahead-of-time Linux x86_64 backend. Ownership semantics come exclusively
//! from post-drop IR. No move checker, loans or provenance enter this crate.
mod codegen;
pub mod layout;
mod inline;
pub mod mono;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tarn_ir::post_drop;
use tarn_types::Typed;

#[derive(Debug)]
pub struct Error {
    pub message: String,
}
impl Error {
    pub(crate) fn unsupported(s: impl Into<String>) -> Self {
        Self { message: format!("backend not implemented: {}", s.into()) }
    }
    pub(crate) fn bug(s: impl Into<String>) -> Self {
        Self { message: format!("compiler bug in native backend: {}", s.into()) }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
const RUNTIME: &str = include_str!("../../../runtime/native.c");

/// Check executable dynamic metadata on a specialized post-drop program.
pub fn verify_dynamic(p: &post_drop::Program, t: &Typed) -> Result<()> {
    let bugs = post_drop::verify(p, t);
    if !bugs.is_empty() {
        return Err(Error::bug(bugs.join("\n")));
    }
    codegen::verify_dynamic(p, t)
}

pub fn emit_object(p: &post_drop::Program, t: &Typed) -> Result<Vec<u8>> {
    emit_entry_object(p, t, None)
}

fn emit_entry_object(p: &post_drop::Program, t: &Typed, entry: Option<tarn_ir::FunctionId>) -> Result<Vec<u8>> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(Error::unsupported("target must be Linux x86_64"));
    }
    let bugs = post_drop::verify(p, t);
    if !bugs.is_empty() {
        return Err(Error::bug(bugs.join("\n")));
    }
    let specialized = if let Some(entry) = entry { mono::specialize_entry(p, t, entry)? } else { mono::specialize(p, t)? };
    let concrete = inline::inline(specialized, t);
    let bugs = post_drop::verify(&concrete, t);
    if !bugs.is_empty() {
        return Err(Error::bug(bugs.join("\n")));
    }
    if entry.is_some() { codegen::emit_entry(&concrete, t, true) } else { codegen::emit(&concrete, t) }
}

/// Build to the requested executable path. Object/runtime intermediates live in
/// a unique scratch directory and are removed on every return path. No shell.
pub fn build(p: &post_drop::Program, t: &Typed, output: &Path) -> Result<()> {
    build_linked(p, t, output, &[])
}

/// Valid `--link` library: `name` (`-lname`) or `:file` (`-l:file`), never an
/// option or a path. Linking is an explicit build-time grant (ADR 0047).
pub fn valid_library(name: &str) -> bool {
    let file = name.strip_prefix(':').unwrap_or(name);
    !file.is_empty() && !file.starts_with('-') && file.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
}

/// `build` plus system libraries granted with `tarn build --link`.
pub fn build_linked(p: &post_drop::Program, t: &Typed, output: &Path, libraries: &[String]) -> Result<()> {
    build_entry(p, t, output, libraries, None)
}

/// Build a frontend-generated test entry with its private argv adapter.
pub fn build_tests(p: &post_drop::Program, t: &Typed, output: &Path, libraries: &[String], entry: tarn_ir::FunctionId) -> Result<()> {
    build_entry(p, t, output, libraries, Some(entry))
}

fn build_entry(p: &post_drop::Program, t: &Typed, output: &Path, libraries: &[String], entry: Option<tarn_ir::FunctionId>) -> Result<()> {
    if let Some(bad) = libraries.iter().find(|l| !valid_library(l)) {
        return Err(Error { message: format!("invalid library `{bad}`: use a name such as `sqlite3` or an exact file such as `:libsqlite3.so.0`") });
    }
    let bytes = emit_entry_object(p, t, entry)?;
    let scratch = Scratch::new()?;
    let object = scratch.0.join("program.o");
    let runtime = scratch.0.join("runtime.c");
    // Link beside the destination, then publish by rename. The compiler must
    // never open the final executable for writing: concurrent child launches
    // can otherwise inherit a writer transiently and fail with ETXTBSY.
    let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let publication = Scratch::in_directory(parent)?;
    let executable = publication.0.join("program");
    std::fs::write(&object, bytes).map_err(io_error)?;
    std::fs::write(&runtime, RUNTIME).map_err(io_error)?;
    // Untranslated linker diagnostics keep linker_message deterministic.
    let linked = Command::new("cc")
        .env("LC_ALL", "C")
        .args(["-std=c11", "-O0", "-fno-strict-aliasing", "-no-pie", "-pthread"])
        .arg(&object)
        .arg(&runtime)
        .args(libraries.iter().map(|l| format!("-l{l}")))
        .arg("-lm")
        .arg("-o")
        .arg(&executable)
        .output()
        .map_err(io_error)?;
    if !linked.status.success() {
        return Err(Error { message: linker_message(&String::from_utf8_lossy(&linked.stderr), libraries) });
    }
    // Do not replace an existing executable until codegen and linking succeed.
    std::fs::rename(executable, output).map_err(io_error)?;
    Ok(())
}
/// Summarize the system linker's failure in source-level terms: missing C
/// symbols and missing libraries, with the raw output kept for anything else.
fn linker_message(stderr: &str, libraries: &[String]) -> String {
    let mut symbols: Vec<&str> = stderr
        .lines()
        .filter_map(|l| l.split("undefined reference to `").nth(1))
        .filter_map(|rest| rest.split('\'').next())
        .collect();
    symbols.sort_unstable();
    symbols.dedup();
    let missing: Vec<&str> = stderr.lines().filter_map(|l| l.split("cannot find -l").nth(1)).map(|l| l.split(':').next().unwrap_or(l).trim()).collect();
    if !missing.is_empty() {
        return format!("native linker cannot find library {}; install it or pass the exact file, e.g. `--link :libname.so.0`", missing.iter().map(|m| format!("`{m}`")).collect::<Vec<_>>().join(", "));
    }
    if !symbols.is_empty() {
        let hint = if libraries.is_empty() { "declare the library with `--link <name>`" } else { "check the extern \"C\" names and the `--link` libraries" };
        return format!("native linker found no definition for extern \"C\" {}; {hint}", symbols.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "));
    }
    format!("native linker failed: {stderr}")
}
fn io_error(e: std::io::Error) -> Error {
    Error { message: format!("native build I/O: {e}") }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        Self::in_directory(&std::env::temp_dir())
    }
    fn in_directory(directory: &Path) -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..1000 {
            let p = directory.join(format!(".tarn-native-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            match std::fs::create_dir(&p) {
                Ok(()) => return Ok(Self(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io_error(e)),
            }
        }
        Err(Error { message: "cannot allocate native build directory".into() })
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
