use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    process::{Command, Stdio},
};

#[test]
fn recursive_check_write_preflight_and_permissions() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-fmt-{}", std::process::id()));
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::create_dir_all(dir.join("target")).unwrap();
    fs::create_dir_all(dir.join(".ignored")).unwrap();
    let entry = dir.join("main.tarn");
    let side = dir.join("nested/side.tarn");
    let original = "fn main(){\nprint(1+2)\n}\n";
    fs::write(&entry, original).unwrap();
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o640)).unwrap();
    fs::write(&side, "fn f( { invalid\n").unwrap();
    fs::write(dir.join("target/broken.tarn"), "invalid").unwrap();
    fs::write(dir.join(".ignored/broken.tarn"), "invalid").unwrap();
    fs::write(dir.join("notes.txt"), "never touch").unwrap();
    let cli = env!("CARGO_BIN_EXE_tarn");
    let run = |check: bool| {
        let mut command = Command::new(cli);
        command.current_dir(&dir).arg("fmt");
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    assert_eq!(run(false).status.code(), Some(1));
    assert_eq!(
        fs::read_to_string(&entry).unwrap(),
        original,
        "preflight must prevent earlier edits"
    );
    fs::write(&side, "fn side(){ }\n").unwrap();
    let output = run(true);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("side.tarn"));
    assert_eq!(fs::read_to_string(&entry).unwrap(), original);
    assert!(run(false).status.success());
    assert_eq!(
        fs::read_to_string(&entry).unwrap(),
        "fn main() {\n    print(1 + 2)\n}\n"
    );
    assert_eq!(
        fs::metadata(&entry).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        fs::read_to_string(dir.join("notes.txt")).unwrap(),
        "never touch"
    );
    assert!(run(true).status.success());
    assert!(
        run(false).stdout.is_empty(),
        "unchanged files should not be rewritten"
    );
    assert!(!fs::read_dir(&dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tarn-fmt-")
    }));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stdin_usage_symlinks_and_missing_sources() {
    let cli = env!("CARGO_BIN_EXE_tarn");
    let mut child = Command::new(cli)
        .args(["fmt", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"fn main(){print(1)}")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"fn main() { print(1) }\n");
    for args in [
        vec!["--stdin", "--check"],
        vec!["--stdin", "main.tarn"],
        vec!["--check", "--check"],
        vec!["--unknown"],
        vec!["a", "b"],
    ] {
        assert_eq!(
            Command::new(cli)
                .arg("fmt")
                .args(args)
                .output()
                .unwrap()
                .status
                .code(),
            Some(2)
        );
    }
    let dir = std::env::temp_dir().join(format!("tarn-cli-fmt-links-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("original.tarn"), "fn f(){ }\n").unwrap();
    symlink("original.tarn", dir.join("link.tarn")).unwrap();
    symlink(".", dir.join("cycle")).unwrap();
    assert_eq!(
        Command::new(cli)
            .arg("fmt")
            .arg(dir.join("link.tarn"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(1)
    );
    assert!(
        Command::new(cli)
            .arg("fmt")
            .arg(&dir)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        fs::symlink_metadata(dir.join("link.tarn"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        Command::new(cli)
            .arg("fmt")
            .arg(dir.join("missing.tarn"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(1)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn formatted_native_program_preserves_execution() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-fmt-native-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    fs::write(&entry, "fn add(a i32,b i32) i32{\nreturn a+\nb\n}\nfn main(){\nvalues:=[3]i32{1,2,3}\nprint(add(values[0],values[2]))\nprint(\"literal // 😀\")\n}\n").unwrap();
    let cli = env!("CARGO_BIN_EXE_tarn");
    let before = Command::new(cli).arg("run").arg(&entry).output().unwrap();
    assert!(
        before.status.success(),
        "{}",
        String::from_utf8_lossy(&before.stderr)
    );
    assert!(
        Command::new(cli)
            .arg("fmt")
            .arg(&entry)
            .output()
            .unwrap()
            .status
            .success()
    );
    let after = Command::new(cli).arg("run").arg(&entry).output().unwrap();
    assert!(
        after.status.success(),
        "{}",
        String::from_utf8_lossy(&after.stderr)
    );
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(before.stderr, after.stderr);
    fs::remove_dir_all(dir).unwrap();
}
