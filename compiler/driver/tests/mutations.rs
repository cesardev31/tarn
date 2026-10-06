//! Robustness: delete each line of every example and suite program, run the
//! whole frontend, and require no panic and structurally valid IR. Exercises resolver and type checker
//! on many odd-but-parseable programs that no hand-written test covers.

use std::path::{Path, PathBuf};

fn corpus() -> Vec<PathBuf> {
    let root = Path::new("../..");
    let mut out = Vec::new();
    for dir in ["examples", "tests/types/pass", "tests/types/fail", "tests/resolve/pass"] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|e| e == "tarn") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn line_deletions_never_panic() {
    let dir = std::env::temp_dir().join(format!("tarn-mutations-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("m.tarn");
    let mut runs = 0;
    for path in corpus() {
        let src = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = src.lines().collect();
        for i in 0..lines.len() {
            let mutated = [&lines[..i], &lines[i + 1..]].concat().join("\n");
            std::fs::write(&file, &mutated).unwrap();
            let result = std::panic::catch_unwind(|| tarn_driver::check(&file));
            assert!(result.is_ok(), "panic on {} without line {}:\n{mutated}", path.display(), i + 1);
            if let Ok(Ok(res)) = &result
                && let Some(ir) = &res.ir
            {
                let errs = tarn_ir::verify(ir);
                assert!(errs.is_empty(), "invalid IR for {} without line {}:\n{}", path.display(), i + 1, errs.join("\n"));
            }
            runs += 1;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(runs > 1000, "{runs}");
}
