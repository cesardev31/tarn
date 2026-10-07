//! JSON Lines preserve frontend diagnostics and encode native failures.
use std::process::{Command, Output};

fn invoke(command: &str, source: &std::path::Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tarn"))
        .args([command, "--json"])
        .arg(source)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn native_json_preserves_diagnostics_and_execution() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-json-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("main.tarn");
    std::fs::write(&source, "fn main() {\n    print(unknown_name)\n}\n").unwrap();
    let checked = invoke("check", &source, &[]);
    assert_eq!(checked.status.code(), Some(1));
    assert!(!checked.stdout.is_empty());
    for command in ["build", "run"] {
        let output = invoke(command, &source, &[]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout, checked.stdout);
        assert!(output.stderr.is_empty());
    }
    std::fs::write(&source, "fn main() {\n    print(19)\n}\n").unwrap();
    let output = invoke("build", &source, &[]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let output = invoke("run", &source, &[]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"19\n");
    let output = Command::new(env!("CARGO_BIN_EXE_tarn"))
        .args(["build", "--json", "-o"])
        .arg(&source)
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"{\"kind\":\"command_error\",\"stage\":\"output\",\"message\":\"output would overwrite the source file\"}\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_and_load_failures_are_single_records() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-json-errors-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("quoted\"name.tarn");
    for command in ["build", "run"] {
        let output = invoke(command, &source, &[]);
        assert_eq!(output.status.code(), Some(1));
        let record = String::from_utf8(output.stdout).unwrap();
        assert_eq!(record.lines().count(), 1);
        assert!(record.starts_with("{\"kind\":\"command_error\",\"stage\":\"load\",\"message\":"));
        assert!(record.contains("quoted\\\"name.tarn"));
        assert!(output.stderr.is_empty());
    }
    for (program, extra, message) in [
        (
            "fn work() {}\nfn main() {\n scope { spawn work() }\n}\n",
            vec![],
            "backend not implemented",
        ),
        (
            "fn main() {}\n",
            vec!["--link", "tarn_missing_library"],
            "cannot find library",
        ),
        (
            "extern \"C\" fn tarn_no_such_symbol() i32\nfn main() { unsafe { print(tarn_no_such_symbol()) } }\n",
            vec![],
            "no definition for extern \\\"C\\\"",
        ),
    ] {
        std::fs::write(&source, program).unwrap();
        for command in ["build", "run"] {
            let output = invoke(command, &source, &extra);
            assert_eq!(output.status.code(), Some(1));
            let record = String::from_utf8(output.stdout).unwrap();
            assert_eq!(record.lines().count(), 1, "{record}");
            assert!(
                record.starts_with("{\"kind\":\"command_error\",\"stage\":\"native\",\"message\":"),
                "{record}"
            );
            assert!(record.contains(message), "{record}");
            assert!(record.ends_with("\"}\n"));
            assert!(output.stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
