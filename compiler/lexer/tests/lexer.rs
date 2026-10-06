use tarn_diagnostics::SourceMap;
use tarn_lexer::{LexResult, TokenKind, lex};

fn run(src: &str) -> (SourceMap, LexResult) {
    let mut map = SourceMap::new();
    let id = map.add("t.tarn", src);
    let res = lex(id, map.file(id));
    (map, res)
}

/// Compact rendering of the token stream: names, with payloads for literals.
fn kinds(src: &str) -> String {
    let (_, res) = run(src);
    assert!(res.diagnostics.is_empty(), "unexpected diagnostics: {:?}", res.diagnostics);
    render(&res)
}

fn render(res: &LexResult) -> String {
    res.tokens
        .iter()
        .map(|t| match &t.kind {
            TokenKind::Ident(s) => format!("id:{s}"),
            TokenKind::Int(v) => format!("int:{v}"),
            TokenKind::Float(s) => format!("float:{s}"),
            TokenKind::Str(s) => format!("str:{s:?}"),
            TokenKind::Newline => "NL".to_string(),
            k => k.name().to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn codes(src: &str) -> Vec<&'static str> {
    run(src).1.diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn empty_file() {
    assert_eq!(kinds(""), "eof");
    assert_eq!(kinds("\n\n  \n"), "eof");
}

#[test]
fn hello_world() {
    assert_eq!(
        kinds("fn main() {\n    print(\"hello\")\n}\n"),
        "fn id:main ( ) { NL id:print ( str:\"hello\" ) NL } NL eof"
    );
}

#[test]
fn keywords_vs_identifiers() {
    assert_eq!(
        kinds("fn return if else for in break continue match struct enum interface impl copy var pub import true false try spawn unsafe extern mut"),
        "fn return if else for in break continue match struct enum interface impl copy var pub import true false try spawn unsafe extern mut eof"
    );
    assert_eq!(kinds("fns _x x1 self Return"), "id:fns id:_x id:x1 id:self id:Return eof");
}

#[test]
fn operators_longest_match() {
    assert_eq!(
        kinds(":= : = == => -> - ! != < <= << > >= >> & && | || ^ + * / % . .. ..= , ( ) [ ] { }"),
        ":= : = == => -> - ! != < <= << > >= >> & && | || ^ + * / % . .. ..= , ( ) [ ] { } eof"
    );
}

#[test]
fn compound_assignment_does_not_exist() {
    assert_eq!(kinds("x += 1"), "id:x + = int:1 eof");
}

#[test]
fn integers() {
    assert_eq!(
        kinds("0 42 1_000 0xff 0xFF_FF 0o755 0b1010 18446744073709551615"),
        "int:0 int:42 int:1000 int:255 int:65535 int:493 int:10 int:18446744073709551615 eof"
    );
}

#[test]
fn floats() {
    assert_eq!(kinds("1.5 2.0e-3 1e9 3E+2 1_000.25"), "float:1.5 float:2.0e-3 float:1e9 float:3E+2 float:1000.25 eof");
}

#[test]
fn ranges_and_member_access_are_not_floats() {
    assert_eq!(kinds("0..10"), "int:0 .. int:10 eof");
    assert_eq!(kinds("0..=9"), "int:0 ..= int:9 eof");
    assert_eq!(kinds("1.max"), "int:1 . id:max eof");
    assert_eq!(kinds("1._5"), "int:1 . id:_5 eof");
}

#[test]
fn strings_and_escapes() {
    assert_eq!(kinds(r#""a\n\t\r\0\\\"b""#), r#"str:"a\n\t\r\0\\\"b" eof"#);
    assert_eq!(kinds(r#""\u{48}\u{1F600}""#), "str:\"H😀\" eof");
    assert_eq!(kinds("\"héllo\""), "str:\"héllo\" eof");
}

#[test]
fn newlines_are_preserved_and_collapsed() {
    // Every line break after a token is kept; the parser decides (ADR 0008).
    assert_eq!(kinds("a\nb"), "id:a NL id:b eof");
    assert_eq!(kinds("a +\nb"), "id:a + NL id:b eof");
    assert_eq!(kinds("f(a,\n  b)"), "id:f ( id:a , NL id:b ) eof");
    assert_eq!(kinds("x :=\n 1"), "id:x := NL int:1 eof");
    // Leading blank lines produce nothing; runs of blank and comment lines collapse.
    assert_eq!(kinds("\n\n// c\na\n\n// c\n\nb\n\n"), "id:a NL id:b NL eof");
}

#[test]
fn leading_dot_is_left_to_the_parser() {
    assert_eq!(
        kinds("cmd := Command(\"x\")\n    .arg(\"v\")\nnext"),
        "id:cmd := id:Command ( str:\"x\" ) NL . id:arg ( str:\"v\" ) NL id:next eof"
    );
}

#[test]
fn comments_are_trivia() {
    let (_, res) = run("// one\n/// doc\n//// not doc\nx // trailing\n");
    assert_eq!(render(&res), "id:x NL eof");
    let c: Vec<_> = res.comments.iter().map(|c| (c.doc, c.text.as_str())).collect();
    assert_eq!(c, [(false, " one"), (true, " doc"), (false, "// not doc"), (false, " trailing")]);
}

#[test]
fn spans_lines_and_columns() {
    let (map, res) = run("fn main() {\n  x := \"é\" + 10\n}");
    let t: Vec<_> = res.tokens.iter().map(|t| (t.kind.name(), t.line, t.column, map.snippet(t.span))).collect();
    assert_eq!(
        t,
        [
            ("fn", 1, 1, "fn"),
            ("ident", 1, 4, "main"),
            ("(", 1, 8, "("),
            (")", 1, 9, ")"),
            ("{", 1, 11, "{"),
            ("newline", 1, 12, "\n"),
            ("ident", 2, 3, "x"),
            (":=", 2, 5, ":="),
            ("string", 2, 8, "\"é\""),
            ("+", 2, 12, "+"),
            ("int", 2, 14, "10"),
            ("newline", 2, 16, "\n"),
            ("}", 3, 1, "}"),
            ("eof", 3, 2, ""),
        ]
    );
}

#[test]
fn crlf_line_endings() {
    assert_eq!(kinds("a\r\nb\r\n"), "id:a NL id:b NL eof");
}

// ---- Errors ----

#[test]
fn unexpected_character_recovers() {
    let (_, res) = run("a $ b");
    assert_eq!(render(&res), "id:a id:b eof");
    assert_eq!(res.diagnostics[0].code, "E0001");
    assert_eq!(codes("x := 1;"), ["E0001"]);
    assert_eq!(codes("x := 'a'"), ["E0001", "E0001"]);
    assert_eq!(codes("ñ"), ["E0001"]);
}

#[test]
fn unterminated_string() {
    let (_, res) = run("x := \"abc\ny");
    assert_eq!(res.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), ["E0002"]);
    assert_eq!(render(&res), "id:x := str:\"abc\" NL id:y eof");
    assert_eq!(codes("\"abc"), ["E0002"]);
    assert_eq!(codes("\"abc\\"), ["E0003", "E0002"]);
}

#[test]
fn invalid_escapes() {
    assert_eq!(codes(r#""\q""#), ["E0003"]);
    assert_eq!(codes(r#""\u{}""#), ["E0003"]);
    assert_eq!(codes(r#""\u{110000}""#), ["E0003"]);
    assert_eq!(codes(r#""\u{D800}""#), ["E0003"]);
    assert_eq!(codes(r#""\u41""#), ["E0003"]);
}

#[test]
fn invalid_numbers() {
    for src in ["12abc", "0x", "0xZZ", "0b102", "1_", "1__0_", "0o8", "1_.5", "1e5x"] {
        assert_eq!(codes(src), ["E0004"], "{src}");
    }
    assert_eq!(codes("18446744073709551616"), ["E0005"]);
    // Interior repeated underscores are allowed.
    assert_eq!(codes("1__0"), Vec::<&str>::new());
}

#[test]
fn all_examples_lex_cleanly() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
    let mut n = 0;
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "tarn") {
                let src = std::fs::read_to_string(&path).unwrap();
                let (map, res) = run(&src);
                let errors: Vec<_> = res.diagnostics.iter().map(|d| d.render(&map)).collect();
                assert!(errors.is_empty(), "{}:\n{}", path.display(), errors.join("\n"));
                n += 1;
            }
        }
    }
    assert!(n >= 20, "expected the example corpus, found {n} files");
}
