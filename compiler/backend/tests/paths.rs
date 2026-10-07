//! Phase 15C: lexical paths use ordinary owned Tarn values.
use std::process::Command;

fn run(source: &str, tag: &str, trace: bool) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!("tarn-paths-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    std::fs::write(&entry, source).unwrap();
    let result = tarn_driver::check(&entry).unwrap();
    assert!(
        !result.has_errors(),
        "{}",
        result
            .diagnostics
            .iter()
            .map(|d| d.render(&result.program.sources))
            .collect::<String>()
    );
    let exe = dir.join("program");
    tarn_backend::build(
        result.drops.as_ref().unwrap(),
        result.typed.as_ref().unwrap(),
        &exe,
    )
    .unwrap();
    let mut cmd = Command::new("timeout");
    cmd.arg("30s").arg(&exe).env_remove("TARN_TRACE_DROPS");
    if trace {
        cmd.env("TARN_TRACE_DROPS", "1");
    }
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
    output
}

#[test]
fn posix_edge_cases_are_explicit_and_normalization_is_idempotent() {
    let cases = [
        ("", "."),
        (".", "."),
        ("/", "/"),
        ("//", "/"),
        ("///a//b/", "/a/b"),
        ("a/..", "."),
        ("../..", "../.."),
        ("../../a/..", "../.."),
        ("/../../a", "/a"),
        ("a/../b/./c", "b/c"),
        ("café/../日本/", "日本"),
        ("a/.../..", "a"),
        ("a\\b", "a\\b"),
        ("C:/a", "C:/a"),
        (".hidden/..", "."),
    ];
    let literals = cases
        .iter()
        .map(|(input, _)| format!("{input:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "import \"path\"\nfn main() {{ values := [{}]string{{{literals}}}\n for value in &values {{ normalized := path.normalize(value)\n print(normalized)\n print(path.normalize(&normalized) == normalized) }}\n }}",
        cases.len()
    );
    let expected = cases
        .iter()
        .map(|(_, output)| format!("{output}\ntrue\n"))
        .collect::<String>();
    assert_eq!(
        String::from_utf8(run(&source, "normalize", false).stdout).unwrap(),
        expected
    );
}

#[test]
fn suffixes_prefixes_and_join_preserve_lexical_intent() {
    let output = run(
        r#"import "path"
fn show(value Option<string>) { match value { Some(text) => { print("some:" + text) }
 None => { print("none") } } }
fn show_path(value Option<path.Path>) { match value { Some(p) => { print("path:" + p.into_string()) }
 None => { print("none") } } }
fn main() {
 p := path.Path.from_text(&"/a/file.txt")
 show_path(p.parent())
 show_path(p.with_extension(&"md"))
 show_path(p.strip_prefix(&"/a"))
 print(path.join(&"a/../b", &"./c"))
 print(path.join(&"a", &"/b"))
 print(path.join(&"a//", &""))
 show(path.parent(&"name"))
 show(path.parent(&"/"))
 show(path.file_name(&"a/.."))
 show(path.file_name(&"a/./"))
 show(path.extension(&".hidden"))
 show(path.extension(&"a."))
 show(path.extension(&".a.b"))
 show(path.file_stem(&"café.tar.gz"))
 show(path.with_extension(&"a.txt", &""))
 show(path.with_extension(&"a", &"bad/name"))
 show(path.with_extension(&"a", &"bad\0name"))
 show(path.strip_prefix(&"/apple", &"/app"))
 show(path.strip_prefix(&"a/../b", &"a"))
 show(path.strip_prefix(&"/a//./b", &"/a/b"))
 print(path.is_valid(&""))
 print(path.is_valid(&"a\0b"))
 print(path.is_valid(&"日本"))
}
"#,
        "edges",
        false,
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "path:/a\npath:/a/file.md\npath:file.txt\na/../b/./c\n/b\na//\nsome:\nnone\nnone\nsome:a\nnone\nsome:\nsome:b\nsome:café.tar\nsome:a\nnone\nnone\nnone\nsome:../b\nsome:\nfalse\nfalse\ntrue\n"
    );
}

#[test]
fn owned_paths_transfer_and_payload_drops_once() {
    let output = run(
        r#"import "path"
fn main() {
 original := path.Path.new("unique-path-payload")
 moved := original
 task := spawn move fn() path.Path { return moved }
 returned := task.join()
 scope { spawn fn() { print(returned.is_relative()) }
 spawn fn() { print(returned.is_valid()) } }
 print(returned.into_string())
}
"#,
        "ownership",
        true,
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "true\ntrue\nunique-path-payload\n"
    );
    let trace = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        trace
            .lines()
            .filter(|line| *line == "drop:unique-path-payload")
            .count(),
        1,
        "{trace}"
    );
}

#[test]
fn path_text_integrates_with_real_filesystem_without_normalizing() {
    let location = std::env::temp_dir().join(format!("tarn-path-file-{}", std::process::id()));
    let source = format!(
        r#"import "path"
import "fs"
import "io"
fn main() Result<void, io.Error> {{
 p := path.Path.new({:?})
 try fs.write_text(p.as_string(), &"path payload")
 print(try fs.read_text(p.as_string()))
 try fs.remove_file(p.as_string())
 return Ok(())
}}
"#,
        location.to_str().unwrap()
    );
    assert_eq!(
        String::from_utf8(run(&source, "filesystem", false).stdout).unwrap(),
        "path payload\n"
    );
    assert!(!location.exists());
}
