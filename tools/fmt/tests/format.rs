use std::path::{Path, PathBuf};

fn formatted(source: &str) -> String {
    let output = tarn_fmt::format(source).unwrap_or_else(|error| panic!("{error:?}\n{source}"));
    assert_eq!(
        tarn_fmt::format(&output).unwrap(),
        output,
        "formatter must be idempotent"
    );
    output
}

#[test]
fn canonical_spacing_indentation_comments_and_crlf() {
    let source = "\r\n/// Keep 😀 and documentation exactly.\r\nfn   add(a   i32,b i32) i32{\r\n\t// keep comment\r\nreturn   a+b  // suffix\r\n}\r\n\r\n\r\nfn main(){print( add(1,2))}\r\n";
    assert_eq!(
        formatted(source),
        "/// Keep 😀 and documentation exactly.\nfn add(a i32, b i32) i32 {\n    // keep comment\n    return a + b  // suffix\n}\n\nfn main() { print(add(1, 2)) }\n"
    );
}

#[test]
fn preserves_operator_roles_generics_literals_and_statement_boundaries() {
    let source = "struct Box<T> { value T }\nfn f(a i32,b i32,x & &Box<Box<i32> >) {\n var total = -a*-b\n if a<b && a>b { total = a<<2 }\n text := \"// literal \\\" 😀\"\n number := 0x_FF\n}\n".replace("0x_FF", "0xff");
    let result = formatted(&source);
    assert!(result.contains("-a * -b"), "{result}");
    assert!(result.contains("a < b && a > b"), "{result}");
    assert!(result.contains("a << 2"), "{result}");
    assert!(result.contains("& &Box<Box<i32> >"), "{result}");
    assert!(result.contains("0xff"), "literal spelling must survive");
}

#[test]
fn multiline_calls_chains_and_binary_continuations() {
    let source =
        "fn main() {\nvalue := call(\n1,\n2,\n)\n.other()\nsum := 1 +\n2\nreturn\ncall()\n}\n";
    let result = formatted(source);
    assert!(
        result.contains("call(\n        1,\n        2,\n    )\n        .other()"),
        "{result}"
    );
    assert!(result.contains("sum := 1 +\n        2"), "{result}");
    assert!(result.contains("return\n    call()"), "{result}");
}

#[test]
fn invalid_sources_are_refused_and_empty_sources_are_stable() {
    for source in [
        "fn main() {",
        "fn main() { print(\"bad)",
        "fn main() { x := 1; }",
        "fn main() { x := 0x_ }",
    ] {
        assert!(
            matches!(
                tarn_fmt::format(source),
                Err(tarn_fmt::FormatError::Syntax(_))
            ),
            "{source}"
        );
    }
    assert_eq!(formatted("\n \t\n"), "");
    assert_eq!(
        formatted("// standalone comment"),
        "// standalone comment\n"
    );
}

fn sources(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            sources(&entry.path(), files);
        } else if entry.path().extension().is_some_and(|ext| ext == "tarn") {
            files.push(entry.path());
        }
    }
}

#[test]
fn repository_syntax_corpus_round_trips_and_is_idempotent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for directory in ["examples", "stdlib", "tests"] {
        sources(&root.join(directory), &mut files);
    }
    let mut accepted = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        match tarn_fmt::format(&text) {
            Ok(output) => {
                accepted += 1;
                assert_eq!(
                    tarn_fmt::format(&output).unwrap(),
                    output,
                    "{}",
                    file.display()
                );
            }
            Err(tarn_fmt::FormatError::Syntax(_)) => {} // Deliberately invalid compiler fixtures.
            Err(error) => panic!("{}: {error:?}", file.display()),
        }
    }
    eprintln!("formatter corpus: {accepted} valid source files");
    assert!(accepted > 200, "too little corpus coverage: {accepted}");
}

#[test]
fn generic_lists_and_comments_in_incomplete_expressions() {
    let source = "fn f<\nT,\nU,\n>(x T) {\nvalue :=\n// between operator and operand\ncall(x)\nsum := 1 +\n// still incomplete\n2\n}\n";
    let result = formatted(source);
    assert!(result.contains("f<\n    T,\n    U,\n>"), "{result}");
    assert!(
        result.contains("value :=\n        // between operator and operand\n        call(x)"),
        "{result}"
    );
    assert!(
        result.contains("sum := 1 +\n        // still incomplete\n        2"),
        "{result}"
    );
}
