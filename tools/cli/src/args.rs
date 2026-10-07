//! Shared command argument validation and entry discovery.

pub fn normalize(args: Vec<String>) -> Result<Vec<String>, String> {
    let Some(command) = args.first() else {
        return Ok(args);
    };
    let native = matches!(command.as_str(), "build" | "run");
    let source = native
        || matches!(
            command.as_str(),
            "lex" | "ast" | "check" | "resolve" | "types" | "ir"
        );
    let usage = match command.as_str() {
        "check" => "tarn check [file.tarn | directory] [--json] [--watch]".into(),
        "lex" | "resolve" => format!("tarn {command} [file.tarn | directory] [--json]"),
        "ir" => "tarn ir [file.tarn | directory] [--drops]".into(),
        "build" => {
            "tarn build [file.tarn | directory] [-o path] [--link library]... [--json]".into()
        }
        "run" => "tarn run [file.tarn | directory] [--link library]... [--json]".into(),
        _ => format!(
            "tarn {command}{}",
            if source {
                " [file.tarn | directory]"
            } else {
                ""
            }
        ),
    };
    let fail = |message: String| format!("{message}\n\nusage: {usage}");
    let mut entry = None;
    let mut options = Vec::new();
    let mut output_seen = false;
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--" if command == "run" => return Err(fail("program arguments are not supported yet; the process-arguments API is not available".into())),
            "--json" if matches!(command.as_str(), "lex" | "check" | "resolve" | "build" | "run") => options.push(arg.clone()),
            "--watch" if command == "check" => options.push(arg.clone()),
            "--drops" if command == "ir" => options.push(arg.clone()),
            "-o" if command == "build" && !output_seen => {
                let value = rest.next().filter(|v| !v.starts_with('-')).ok_or_else(|| fail("-o requires an output path".into()))?;
                output_seen = true;
                options.extend([arg.clone(), value.clone()]);
            }
            "--link" if native => {
                let value = rest.next().ok_or_else(|| fail("--link requires a library".into()))?;
                if !tarn_backend::valid_library(value) {
                    return Err(fail(format!("invalid library `{value}`: use a name such as `sqlite3` or an exact file such as `:libsqlite3.so.0`")));
                }
                options.extend([arg.clone(), value.clone()]);
            }
            _ if arg.starts_with('-') => return Err(fail(format!("unknown or repeated option `{arg}`"))),
            _ if source && entry.is_none() => entry = Some(arg.clone()),
            _ => return Err(fail(format!("unexpected argument `{arg}`"))),
        }
    }
    if !source {
        return Ok(args);
    }
    let mut path = std::path::PathBuf::from(entry.unwrap_or_else(|| "main.tarn".into()));
    if path.is_dir() {
        path.push("main.tarn");
    }
    let mut normalized = vec![command.clone(), path.to_string_lossy().into_owned()];
    normalized.extend(options);
    Ok(normalized)
}
