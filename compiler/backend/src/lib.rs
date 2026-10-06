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
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(Error::unsupported("target must be Linux x86_64"));
    }
    let bugs = post_drop::verify(p, t);
    if !bugs.is_empty() {
        return Err(Error::bug(bugs.join("\n")));
    }
    let concrete = inline::inline(mono::specialize(p, t)?, t);
    let bugs = post_drop::verify(&concrete, t);
    if !bugs.is_empty() {
        return Err(Error::bug(bugs.join("\n")));
    }
    codegen::emit(&concrete, t)
}

/// Build to the requested executable path. Object/runtime intermediates live in
/// a unique scratch directory and are removed on every return path. No shell.
pub fn build(p: &post_drop::Program, t: &Typed, output: &Path) -> Result<()> {
    let bytes = emit_object(p, t)?;
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
    let linked = Command::new("cc")
        .args(["-std=c11", "-O0", "-fno-strict-aliasing", "-no-pie", "-pthread"])
        .arg(&object)
        .arg(&runtime)
        .arg("-lm")
        .arg("-o")
        .arg(&executable)
        .output()
        .map_err(io_error)?;
    if !linked.status.success() {
        return Err(Error { message: format!("native linker failed: {}", String::from_utf8_lossy(&linked.stderr)) });
    }
    // Do not replace an existing executable until codegen and linking succeed.
    std::fs::rename(executable, output).map_err(io_error)?;
    Ok(())
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
