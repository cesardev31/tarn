//! `tarn` — the single official CLI.

use std::process::ExitCode;
use tarn_diagnostics::SourceMap;

const USAGE: &str = "\
tarn — the Tarn language toolchain

usage:
    tarn lex <file.tarn> [--json]    print the token stream (debugging)
    tarn ast <file.tarn>             parse and print the syntax tree
    tarn check <file.tarn> [--json]  lex, parse and resolve names; report diagnostics
    tarn resolve <file.tarn>         print what every name resolves to
    tarn types <file.tarn>           print the type of every local and parameter
    tarn ir <file.tarn>              print typed IR; --drops prints executable drops
    tarn version                     print the compiler version

planned: build, run, test, check, fmt, clean, cache
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("lex") => cmd_lex(&args[1..]),
        Some("ast") => cmd_ast(&args[1..]),
        Some("check") => cmd_check(&args[1..], false),
        Some("resolve") => cmd_check(&args[1..], true),
        Some("types") => cmd_types(&args[1..]),
        Some("ir") => cmd_ir(&args[1..]),
        Some("version" | "--version" | "-V") => {
            println!("tarn {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("help" | "--help" | "-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(cmd @ ("build" | "run" | "test" | "fmt" | "clean" | "cache")) => {
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
