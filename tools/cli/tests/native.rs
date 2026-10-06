//! Exercise the public commands, output placement, exit propagation and errors.
use std::process::Command;
#[test]
fn build_run_and_failure_paths() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-native-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("hello.tarn");
    std::fs::write(&source, "fn main() {\n    print(42)\n}\n").unwrap();
    let cli = env!("CARGO_BIN_EXE_tarn");
    let output = Command::new(cli).arg("build").arg(&source).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let executable = source.with_extension("");
    let bytes = std::fs::read(&executable).unwrap();
    assert_eq!(&bytes[..4], b"\x7fELF");
    let output = Command::new(&executable).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    let output = Command::new(cli).arg("run").arg(&source).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    let explicit = dir.join("custom program");
    assert!(Command::new(cli).arg("build").arg(&source).arg("-o").arg(&explicit).status().unwrap().success());
    assert!(explicit.is_file());
    let original = std::fs::read(&source).unwrap();
    assert!(!Command::new(cli).arg("build").arg(&source).arg("-o").arg(&source).output().unwrap().status.success());
    assert_eq!(std::fs::read(&source).unwrap(), original);
    // A rejected feature must preserve an existing compiled output.
    std::fs::write(&source, "fn id<T>(x T) T {\n    return x\n}\nfn main() {\n    print(id(42))\n}\n").unwrap();
    let output = Command::new(cli).arg("build").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("backend not implemented"));
    assert_eq!(std::fs::read(&executable).unwrap(), bytes);
    std::fs::write(&source, "fn main() {\n    panic(\"stop\")\n}\n").unwrap();
    let output = Command::new(cli).arg("run").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("panic: stop"));
    std::fs::remove_dir_all(dir).unwrap();
}
