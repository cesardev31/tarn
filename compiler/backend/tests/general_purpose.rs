use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
fn build(name: &str, source: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = tarn_driver::check(&root.join(source)).unwrap();
    assert!(
        !result.has_errors(),
        "{}",
        result
            .diagnostics
            .iter()
            .map(|d| d.render(&result.program.sources))
            .collect::<String>()
    );
    let dir = std::env::temp_dir().join(format!("tarn-foundations-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("program");
    tarn_backend::build(
        result.drops.as_ref().unwrap(),
        result.typed.as_ref().unwrap(),
        &exe,
    )
    .unwrap();
    exe
}
#[test]
fn text_numeric_maps_and_blocking_sleep_work_natively() {
    let exe = build("text", "tests/native/pass/general_purpose_text.tarn");
    let output = Command::new(&exe).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"general purpose text ok\n");
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}
#[test]
fn csv_cli_handles_quotes_chunk_boundaries_and_rejects_partial_reports() {
    let exe = build("csv", "examples/csv_report/main.tarn");
    let cases = vec![
        ("CATEGORY,value\r\nTools,1.25\r\ntools,2.5\r\n\"café, tea\",-3e1\r\n".as_bytes().to_vec(), Some("{\"records\":3,\"groups\":[{\"category\":\"café, tea\",\"count\":1,\"total\":-30},{\"category\":\"tools\",\"count\":2,\"total\":3.75}]}\n".to_string())),
        ("category,value\n\"a\r\nb\",2\n".as_bytes().to_vec(), Some("{\"records\":1,\"groups\":[{\"category\":\"a\\nb\",\"count\":1,\"total\":2}]}\n".to_string())),
        (b"category,value\nx,NaN\n".to_vec(), None),
        (b"category,value\nx,1\nx,1e309\n".to_vec(), None),
        (b"category,value\n\"unclosed,2\n".to_vec(), None),
        (b"category,value\n\"x\"z,2\n".to_vec(), None),
        (b"category,value\nx,\xff\n".to_vec(), None),
        (Vec::new(), None),
        (format!("category,value\n{},2", "é".repeat(4097)).into_bytes(), Some(format!("{{\"records\":1,\"groups\":[{{\"category\":\"{}\",\"count\":1,\"total\":2}}]}}\n", "é".repeat(4097)))),
    ];
    for (input, expected) in cases {
        let mut child = Command::new(&exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let output = child.wait_with_output().unwrap();
        if let Some(expected) = expected {
            assert!(output.status.success(), "{:?}", output);
            assert_eq!(output.stdout, expected.as_bytes());
            assert!(output.stderr.is_empty());
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.starts_with(b"{\"error\":"));
        }
    }
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}
#[test]
fn collector_handles_resets_pid_reuse_and_parentheses() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(root.join("evidence/status_device/collector.tarn"))
        .unwrap()
        .replace("fn main()", "fn original_main()");
    let dir = std::env::temp_dir().join(format!("tarn-collector-fixture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn");
    std::fs::write(
        &file,
        format!(
            "{source}\n{}",
            r#"
fn verify(value bool) { if !value { panic("collector assertion") } }
fn main() {
    verify(cpu_percent(&CpuTimes{total: 100, idle: 50}, &CpuTimes{total: 90, idle: 40}) == 0.0)
    verify(cpu_percent(&CpuTimes{total: 100, idle: 50}, &CpuTimes{total: 120, idle: 60}) == 50.0)
    verify(cpu_percent(&CpuTimes{total: 100, idle: 50}, &CpuTimes{total: 110, idle: 70}) == 0.0)
    previous := Previous{ticks: 10, start: 20}
    current := Proc{pid: 1, name: "a)b", ticks: 15, start: 20, rss_pages: 0}
    verify(process_percent(previous, &current, 20) == 25.0)
    verify(process_percent(Previous{ticks: 10, start: 19}, &current, 20) == 0.0)
    verify(process_percent(Previous{ticks: 16, start: 20}, &current, 20) == 0.0)
    verify(process_percent(previous, &current, 0) == 0.0)
    verify(string.rfind("1 (a)b) R", ")").unwrap_or(0) == 6)
    verify(field("R 10 20", 2) == 20)
    verify(field("R 10 20", 9) == 0)
}
"#
        ),
    )
    .unwrap();
    let result = tarn_driver::check(&file).unwrap();
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
    assert!(Command::new(&exe).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn bounded_lines_validate_utf8_crlf_eof_and_explicit_flush() {
    let exe = build("lines", "tests/integration/console_lines_fixture.tarn");
    for (input, expected) in [
        (b"".as_slice(), Some(b"".as_slice())),
        (b"abcd\r\n\nxy", Some(b"abcd||xy|")),
        ("éé\n".as_bytes(), Some("éé|".as_bytes())),
        (b"abc\r", Some(b"abc\r|")),
        (b"abcde\n", None),
        (b"abcd\r", None),
        (b"\xff\n", None),
    ] {
        let mut child = Command::new(&exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        if let Some(expected) = expected {
            assert!(output.status.success());
            assert_eq!(output.stdout, expected);
        } else {
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
        }
    }
    std::fs::remove_dir_all(exe.parent().unwrap()).unwrap();
}
