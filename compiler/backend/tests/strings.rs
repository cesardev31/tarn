//! Phase 15A: real native execution, UTF-8 and ownership regression oracles.
use std::process::Command;

fn run(source: &str, tag: &str, trace: bool) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!("tarn-strings-{}-{tag}", std::process::id()));
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
fn strict_utf8_matches_the_rust_oracle() {
    let mut cases = vec![
        vec![],
        vec![0],
        vec![0xc0, 0x80],
        vec![0xc1, 0xbf],
        vec![0xe0, 0x80, 0x80],
        vec![0xed, 0xa0, 0x80],
        vec![0xf0, 0x80, 0x80, 0x80],
        vec![0xf4, 0x90, 0x80, 0x80],
        vec![0xf4, 0x8f, 0xbf, 0xbf],
        vec![0xf5, 0x80, 0x80, 0x80],
        vec![0xef, 0xbf, 0xbf],
    ];
    let mut source = String::from(
        "import \"string\"\nfn check(data &[]u8) {\n match string.from_utf8(data) {\n Some(text) => { print(text.len()) }\n None => { print(\"invalid\") }\n }\n}\nfn main() {\n",
    );
    for (i, bytes) in cases.iter().enumerate() {
        source.push_str(&format!(
            " {{ bytes{i} := [{}]u8{{{}}}\n check(&bytes{i}) }}\n",
            bytes.len(),
            bytes
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    // Exercise the large corpus at runtime, keeping the compilation fixture
    // small. Every leading byte and each continuation boundary is included.
    source.push_str(" tails := [4]u8{128, 159, 160, 191}\n var first: usize = 0\n for first < usize(256) {\n var bytes = [4]u8{u8(first), 128, 128, 128}\n check(&bytes[..1])\n for tail in &tails {\n bytes[1] = tail\n check(&bytes[..2])\n check(&bytes[..3])\n check(&bytes[..4])\n }\n first = first + 1\n }\n}\n");
    for first in 0..=255u8 {
        cases.push(vec![first]);
        for continuation in [0x80, 0x9f, 0xa0, 0xbf] {
            cases.push(vec![first, continuation]);
            cases.push(vec![first, continuation, 0x80]);
            cases.push(vec![first, continuation, 0x80, 0x80]);
        }
    }
    let mut expected = String::new();
    for bytes in cases {
        if std::str::from_utf8(&bytes).is_ok() {
            expected.push_str(&format!("{}\n", bytes.len()));
        } else {
            expected.push_str("invalid\n");
        }
    }
    let output = run(&source, "utf8", false);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn decimal_boundaries_round_trip_and_overflow_is_an_option() {
    let mut source = String::from("import \"string\"\nfn main() {\n");
    let mut expected = String::new();
    for n in [
        i64::MIN,
        i64::MIN + 1,
        -65536,
        -1,
        0,
        1,
        65535,
        i64::MAX - 1,
        i64::MAX,
    ] {
        source.push_str(&format!(" {{ text := string.from_i64(i64({n}))\n print(text)\n match string.parse_i64(&text) {{\n Some(n) => {{ print(n) }}\n None => {{ panic(\"round trip\") }}\n }} }}\n"));
        expected.push_str(&format!("{n}\n{n}\n"));
    }
    for n in [0u64, 1, 65535, u64::MAX - 1, u64::MAX] {
        source.push_str(&format!(" {{ n: u64 := {n}\n text := string.from_u64(n)\n print(text)\n match string.parse_u64(&text) {{\n Some(n) => {{ print(n) }}\n None => {{ panic(\"round trip\") }}\n }} }}\n"));
        expected.push_str(&format!("{n}\n{n}\n"));
    }
    for (method, invalid) in [
        ("parse_i64", "9223372036854775808"),
        ("parse_i64", "-9223372036854775809"),
        ("parse_u64", "18446744073709551616"),
        ("parse_u16", "65536"),
        ("parse_u64", "-1"),
        ("parse_i64", ""),
        ("parse_i64", "+"),
        ("parse_u64", "1_0"),
        ("parse_i64", " 1"),
        ("parse_i64", "1 "),
        ("parse_i64", "１２"),
    ] {
        source.push_str(&format!(" match string.{method}(&\"{invalid}\") {{\n Some(n) => {{ panic(\"invalid decimal accepted\") }}\n None => {{ print(\"invalid\") }}\n }}\n"));
        expected.push_str("invalid\n");
    }
    source.push_str("}\n");
    let output = run(&source, "decimal", false);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn owned_split_results_and_moves_destroy_each_string_once() {
    let output = run(
        "import \"string\"\nfn make() Vec<string> { value := \"a,b\"\n return string.split(&value, &\",\") }\nfn main() { parts := make()\n moved := parts\n for part in moved.as_slice() { print(part) } }",
        "drops",
        true,
    );
    assert_eq!(output.stdout, b"a\nb\n");
    let trace = String::from_utf8(output.stderr).unwrap();
    assert_eq!(trace.lines().filter(|s| *s == "drop:a,b").count(), 1);
    assert_eq!(trace.lines().filter(|s| *s == "drop:a").count(), 1);
    assert_eq!(trace.lines().filter(|s| *s == "drop:b").count(), 1);
    assert_eq!(trace.lines().filter(|s| *s == "drop:,").count(), 1);
    assert_eq!(trace, "drop:,\ndrop:a,b\ndrop:a\ndrop:b\n");
}

#[test]
fn corrupted_string_bridge_abi_is_rejected_before_codegen() {
    let dir = std::env::temp_dir().join(format!("tarn-string-verify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    std::fs::write(
        &entry,
        "fn main() { value := \"text\"\n bytes := value.bytes()\n print(bytes.len()) }",
    )
    .unwrap();
    let result = tarn_driver::check(&entry).unwrap();
    assert!(!result.has_errors());
    let mut drops = result.drops.unwrap();
    let main = drops
        .functions
        .iter_mut()
        .find(|f| f.decl.name == "main")
        .unwrap();
    let call = main
        .blocks
        .iter_mut()
        .find_map(|b| match &mut b.term {
            tarn_ir::Terminator::Call {
                callee: tarn_ir::Callee::Intrinsic(name),
                args,
                ..
            } if name == "string.bytes" => Some(args),
            _ => None,
        })
        .unwrap();
    call.clear();
    let typed = result.typed.unwrap();
    assert!(
        tarn_ir::post_drop::verify(&drops, &typed)
            .iter()
            .any(|s| s.contains("invalid string intrinsic ABI"))
    );
    let err = tarn_backend::emit_object(&drops, &typed).unwrap_err();
    assert!(err.to_string().contains("invalid string intrinsic ABI"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn strings_with_embedded_nul_compare_and_concatenate_by_length() {
    let source = "import \"string\"\nfn main() { bytes := [3]u8{65, 0, 66}\n match string.from_utf8(&bytes) {\n Some(text) => { cloned := text.clone()\n print(text == cloned)\n print(text != \"A\")\n joined := text + cloned\n print(joined.len())\n print(joined.bytes()[4]) }\n None => { panic(\"NUL is valid UTF-8\") }\n } }";
    let out = run(source, "nul", false);
    assert_eq!(out.stdout, b"true\ntrue\n6\n0\n");
    assert!(out.stderr.is_empty());
}
