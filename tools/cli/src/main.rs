//! `tarn` — the single official CLI.

mod args;
mod watch;

use std::process::ExitCode;
use tarn_diagnostics::SourceMap;

const USAGE: &str = "\
tarn — the Tarn language toolchain

usage:
    tarn lex [entry] [--json]    print the token stream (debugging)
    tarn ast [entry]             parse and print the syntax tree
    tarn check [entry] [--json] [--watch]  lex, parse and resolve names; report diagnostics
    tarn resolve [entry]         print what every name resolves to
    tarn types [entry]           print the type of every local and parameter
    tarn ir [entry]              print typed IR; --drops prints executable drops
    tarn build [entry] [-o path] [--link lib]... [--json]
                                     emit a Linux x86_64 executable; each
                                     --link grants a system C library
    tarn run [entry] [--link lib]... [--json]
                                     build temporarily and execute
    tarn version                     print the compiler version

Entry defaults to ./main.tarn; directories select <dir>/main.tarn.
Exit codes: 0 success, 1 compilation/link failure, 2 usage error.
run propagates the program exit code.

planned: test, fmt, clean, cache
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match args::normalize(args) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };
    match args.first().map(String::as_str) {
        Some("lex") => cmd_lex(&args[1..]),
        Some("ast") => cmd_ast(&args[1..]),
        Some("check") => cmd_check(&args[1..], false),
        Some("resolve") => cmd_check(&args[1..], true),
        Some("types") => cmd_types(&args[1..]),
        Some("ir") => cmd_ir(&args[1..]),
        Some("build") => cmd_native(&args[1..], false),
        Some("run") => cmd_native(&args[1..], true),
        Some("version" | "--version" | "-V") => {
            println!("tarn {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("help" | "--help" | "-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(cmd @ ("test" | "fmt" | "clean" | "cache")) => {
            eprintln!("error: `tarn {cmd}` is not implemented yet (see docs/roadmap.md)");
            ExitCode::from(2)
        }
        Some(other) => {
            eprintln!("error: unknown command `{other}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn cmd_lex(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("error: missing file\n\nusage: tarn lex <file.tarn> [--json]");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error[E9001]: cannot read `{path}`: {e}");
            return ExitCode::from(1);
        }
    };
    let mut map = SourceMap::new();
    let id = map.add(path.clone(), text);
    let res = tarn_lexer::lex(id, map.file(id));

    for t in &res.tokens {
        let text = map.snippet(t.span);
        if json {
            let mut s = String::new();
            json_str(&mut s, text);
            println!(
                "{{\"kind\":\"{}\",\"line\":{},\"column\":{},\"start\":{},\"end\":{},\"text\":{s}}}",
                t.kind.name(),
                t.line,
                t.column,
                t.span.start,
                t.span.end
            );
        } else {
            println!("{}:{}\t{}\t{}", t.line, t.column, t.kind.name(), text.escape_debug());
        }
    }
    for d in &res.diagnostics {
        if json {
            println!("{}", d.to_json(&map));
        } else {
            eprint!("{}", d.render(&map));
        }
    }
    if res.diagnostics.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn cmd_ast(args: &[String]) -> ExitCode {
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("error: missing file\n\nusage: tarn ast <file.tarn>");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error[E9001]: cannot read `{path}`: {e}");
            return ExitCode::from(1);
        }
    };
    let mut map = SourceMap::new();
    let id = map.add(path.clone(), text);
    let res = tarn_parser::parse_file(id, map.file(id));
    print!("{}", tarn_ast::dump_module(&res.module));
    for d in &res.diagnostics {
        eprint!("{}", d.render(&map));
    }
    if res.diagnostics.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn cmd_check(args: &[String], dump: bool) -> ExitCode {
    if args.iter().any(|arg| arg == "--watch") {
        return watch::check(std::path::Path::new(&args[0]), args.iter().any(|arg| arg == "--json"));
    }
    let json = args.iter().any(|a| a == "--json");
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("error: missing file\n\nusage: tarn check <file.tarn> [--json]");
        return ExitCode::from(2);
    };
    let res = match tarn_driver::check(std::path::Path::new(path)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error[E9001]: {e}");
            return ExitCode::from(1);
        }
    };
    let map = &res.program.sources;
    if dump && let Some(r) = &res.resolved {
        print!("{}", tarn_resolve::dump_resolution(r, map));
    }
    for d in &res.diagnostics {
        if json {
            println!("{}", d.to_json(map));
        } else {
            eprint!("{}", d.render(map));
        }
    }
    if res.has_errors() { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

fn cmd_types(args: &[String]) -> ExitCode {
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("error: missing file\n\nusage: tarn types <file.tarn>");
        return ExitCode::from(2);
    };
    let res = match tarn_driver::check(std::path::Path::new(path)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error[E9001]: {e}");
            return ExitCode::from(1);
        }
    };
    let map = &res.program.sources;
    if let (Some(r), Some(t)) = (&res.resolved, &res.typed) {
        print!("{}", tarn_types::dump_types(t, r, map));
    }
    for d in &res.diagnostics {
        eprint!("{}", d.render(map));
    }
    if res.has_errors() { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

fn cmd_ir(args: &[String]) -> ExitCode {
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("error: missing file\n\nusage: tarn ir <file.tarn>");
        return ExitCode::from(2);
    };
    let res = match tarn_driver::check(std::path::Path::new(path)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error[E9001]: {e}");
            return ExitCode::from(1);
        }
    };
    let map = &res.program.sources;
    if let (Some(r), Some(t), Some(ir)) = (&res.resolved, &res.typed, &res.ir) {
        if args.iter().any(|a| a == "--drops") {
            if let Some(drops) = &res.drops { print!("{}", tarn_ir::post_drop::print_program(drops, r, t)); }
        } else {
            let notes = res.moves.as_ref().map(|m| m.drop_notes()).unwrap_or_default();
            print!("{}", tarn_ir::print_program_annotated(ir, r, t, &notes));
        }
    }
    for d in &res.diagnostics {
        eprint!("{}", d.render(map));
    }
    if res.has_errors() { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn cmd_native(args: &[String], run: bool) -> ExitCode {
    let Some(path) = args.first().filter(|p| !p.starts_with('-')) else {
        eprintln!("error: usage: tarn {} file.tarn{} [--link library]...", if run { "run" } else { "build" }, if run { "" } else { " [-o path]" });
        return ExitCode::from(2);
    };
    // Arguments have already been validated by the shared parser.
    let json = args.iter().any(|arg| arg == "--json");
    let mut explicit = None;
    let mut libraries = Vec::new();
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-o" => explicit = rest.next().map(std::path::PathBuf::from),
            "--link" => libraries.push(rest.next().expect("validated library").clone()),
            "--json" => {},
            _ => unreachable!("validated native option"),
        }
    }
    let res = match tarn_driver::check(std::path::Path::new(path)) {
        Ok(r) => r, Err(e) => { native_error(json, "load", &e.to_string()); return ExitCode::from(1); }
    };
    for d in &res.diagnostics {
        if json { println!("{}", d.to_json(&res.program.sources)); }
        else { eprint!("{}", d.render(&res.program.sources)); }
    }
    if res.has_errors() { return ExitCode::from(1); }
    let (Some(p), Some(t)) = (&res.drops, &res.typed) else { native_error(json, "internal", "compiler bug: missing post-drop IR"); return ExitCode::from(1); };
    let output = if run { std::env::temp_dir().join(format!("tarn-run-{}", std::process::id())) }
        else { explicit.unwrap_or_else(|| std::path::Path::new(path).with_extension("")) };
    if std::path::Path::new(path).canonicalize().ok() == output.canonicalize().ok() && output.exists() {
        native_error(json, "output", "output would overwrite the source file"); return ExitCode::from(1);
    }
    if let Err(e) = tarn_backend::build_linked(p, t, &output, &libraries) { native_error(json, "native", &e.to_string()); return ExitCode::from(1); }
    if !run { return ExitCode::SUCCESS; }
    let status = std::process::Command::new(&output).status();
    let _ = std::fs::remove_file(&output);
    match status {
        Ok(s) => { use std::os::unix::process::ExitStatusExt; ExitCode::from(s.code().unwrap_or_else(|| 128 + s.signal().unwrap_or(1)).clamp(0,255) as u8) }
        Err(e) => { native_error(json, "execute", &format!("cannot run executable: {e}")); ExitCode::from(1) }
    }
}

/// CLI failures without source spans use a separate, stable JSON Lines record.
fn native_error(json: bool, stage: &str, message: &str) {
    if json {
        let mut encoded = String::new();
        json_str(&mut encoded, message);
        println!("{{\"kind\":\"command_error\",\"stage\":\"{stage}\",\"message\":{encoded}}}");
    } else {
        eprintln!("error: {message}");
    }
}
