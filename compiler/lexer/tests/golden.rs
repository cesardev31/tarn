//! Golden diagnostics: every `tests/lexer/fail/*.tarn` has a `.diag` file with
//! the exact rendered output. Run with `TARN_BLESS=1` to regenerate.

use tarn_diagnostics::SourceMap;
use tarn_lexer::lex;

#[test]
fn golden_lexer_diagnostics() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/lexer/fail");
    let bless = std::env::var_os("TARN_BLESS").is_some();
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tarn"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty());

    let mut failures = Vec::new();
    for path in paths {
        let src = std::fs::read_to_string(&path).unwrap();
        let name = format!("tests/lexer/fail/{}", path.file_name().unwrap().to_string_lossy());
        let mut map = SourceMap::new();
        let id = map.add(name, src);
        let res = lex(id, map.file(id));
        assert!(!res.diagnostics.is_empty(), "{} produced no diagnostics", path.display());
        let actual: String = res.diagnostics.iter().map(|d| d.render(&map) + "\n").collect();

        let expected_path = path.with_extension("diag");
        if bless {
            std::fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
        if expected != actual {
            failures.push(format!("{}\n--- expected\n{expected}--- actual\n{actual}", path.display()));
        }
    }
    assert!(failures.is_empty(), "golden mismatch (TARN_BLESS=1 to update):\n{}", failures.join("\n"));
}
