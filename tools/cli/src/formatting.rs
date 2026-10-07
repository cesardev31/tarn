//! Recursive source formatting with syntax preflight and atomic per-file writes.
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

pub fn run(args: &[String]) -> ExitCode {
    if args[0] == "--stdin" {
        let mut text = String::new();
        if let Err(error) = io::stdin().read_to_string(&mut text) {
            eprintln!("error: cannot read stdin: {error}");
            return ExitCode::from(1);
        }
        return match format("<stdin>", &text) {
            Ok(text) => match io::stdout().write_all(text.as_bytes()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("error: cannot write stdout: {error}");
                    ExitCode::from(1)
                }
            },
            Err(()) => ExitCode::from(1),
        };
    }
    let check = args.iter().any(|arg| arg == "--check");
    let mut paths = Vec::new();
    if let Err(error) = collect(Path::new(&args[0]), &mut paths) {
        eprintln!("error: {error}");
        return ExitCode::from(1);
    }
    paths.sort();
    let mut changes = Vec::new();
    // A syntax error anywhere prevents edits anywhere in this invocation.
    for path in paths {
        let original = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("error: cannot read `{}`: {error}", path.display());
                return ExitCode::from(1);
            }
        };
        let formatted = match format(&path.display().to_string(), &original) {
            Ok(text) => text,
            Err(()) => return ExitCode::from(1),
        };
        if formatted != original {
            changes.push((path, original, formatted));
        }
    }
    if check {
        for (path, _, _) in &changes {
            println!("{}", path.display());
        }
        return if changes.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    for (index, (path, original, formatted)) in changes.iter().enumerate() {
        if let Err(error) = replace(path, original, formatted, index) {
            eprintln!("error: cannot format `{}`: {error}", path.display());
            return ExitCode::from(1);
        }
        println!("{}", path.display());
    }
    ExitCode::SUCCESS
}
fn format(name: &str, text: &str) -> Result<String, ()> {
    tarn_fmt::format(text).map_err(|error| match error {
        tarn_fmt::FormatError::Syntax(diagnostics) => {
            let mut map = tarn_diagnostics::SourceMap::new();
            map.add(name, text);
            for diagnostic in diagnostics {
                eprint!("{}", diagnostic.render(&map));
            }
        }
        tarn_fmt::FormatError::Invariant => eprintln!(
            "error: formatter could not preserve syntax in `{name}`; source was not modified"
        ),
    })
}
fn collect(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "refusing to format a symbolic link: {}",
            path.display()
        )));
    }
    if metadata.is_file() {
        if path.extension().is_some_and(|ext| ext == "tarn") {
            files.push(path.into());
            return Ok(());
        }
        return Err(io::Error::other(
            "formatter input must be a .tarn file or a directory",
        ));
    }
    if !metadata.is_dir() {
        return Err(io::Error::other(
            "formatter input must be a regular file or directory",
        ));
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        if kind.is_dir() {
            if name.to_string_lossy().starts_with('.')
                || matches!(
                    name.to_str(),
                    Some("target" | "node_modules" | "graphify-out")
                )
            {
                continue;
            }
            collect(&entry.path(), files)?;
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "tarn") {
            files.push(entry.path());
        }
    }
    Ok(())
}
fn replace(path: &Path, original: &str, formatted: &str, index: usize) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() || fs::read_to_string(path)? != original
    {
        return Err(io::Error::other("source changed during formatting; retry"));
    }
    let temporary = path.with_file_name(format!(".tarn-fmt-{}-{index}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.set_permissions(fs::metadata(path)?.permissions())?;
        file.write_all(formatted.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if fs::symlink_metadata(path)?.file_type().is_symlink()
            || fs::read_to_string(path)? != original
        {
            return Err(io::Error::other("source changed during formatting; retry"));
        }
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
