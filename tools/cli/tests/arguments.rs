//! Public command-surface regressions.
use std::process::Command;

#[test]
fn usage_errors_are_rejected_before_loading_sources() {
    for command in [
        "lex", "ast", "check", "resolve", "types", "ir", "build", "run", "version", "help", "test",
        "fmt", "clean", "cache",
    ] {
        for args in [vec!["--unknown"], vec!["first.tarn", "second.tarn"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_tarn"))
                .arg(command)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2), "{command}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(&format!("usage: tarn {command}"))
            );
        }
    }
    for args in [
        vec!["build", "-o"],
        vec!["build", "--link"],
        vec!["build", "-o", "one", "-o", "two"],
        vec!["run", "-o", "out"],
        // Program arguments exist only for `run` (Phase 24, ADR 0053).
        vec!["build", "--", "hello"],
        vec!["check", "--", "hello"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tarn"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
    }
}

#[test]
fn discovers_entries_and_accepts_options_before_the_entry() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-entry-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.tarn"), "fn main() {\n    print(19)\n}\n").unwrap();
    let cli = env!("CARGO_BIN_EXE_tarn");
    for command in [
        "lex", "ast", "check", "resolve", "types", "ir", "build", "run",
    ] {
        for explicit_directory in [false, true] {
            let mut cmd = Command::new(cli);
            cmd.current_dir(&dir).arg(command);
            if explicit_directory {
                cmd.arg(&dir);
            }
            let output = cmd.output().unwrap();
            assert!(
                output.status.success(),
                "{command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if command == "run" {
                assert_eq!(output.stdout, b"19\n");
            }
        }
    }
    let output = Command::new(cli)
        .args(["build", "--link", "m", "--link", "m", "-o"])
        .arg(dir.join("custom"))
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.join("custom").exists());
    let output = Command::new(cli)
        .args(["check", "--json"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    std::fs::remove_file(dir.join("main.tarn")).unwrap();
    let output = Command::new(cli)
        .current_dir(&dir)
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    std::fs::remove_dir_all(dir).unwrap();
}
