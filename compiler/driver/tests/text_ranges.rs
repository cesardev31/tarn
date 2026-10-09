#[test]
fn borrowed_text_ranges_retain_owner_provenance() {
    let cases = [
        ("escape", "fn bad(text string) &[]u8 { return string.range_bytes(&text, string.Range{start: 0, end: 0}) }", Some("E4201")),
        ("overwrite", "fn bad() { var text = \"abc\"\n bytes := string.range_bytes(&text, string.Range{start: 0, end: 1})\n text = \"def\"\n print(bytes.len()) }", Some("E4102")),
        ("copy_bound", "fn good() { var spans: Vec<string.Range> = Vec.new()\n spans.push(string.Range{start: 0, end: 0})\n span := spans.at(0)\n print(span.start) }", None),
        ("forward", "fn good(text &string) &[]u8 { return string.range_bytes(text, string.Range{start: 0, end: 0}) }", None),
    ];
    let dir = std::env::temp_dir().join(format!("tarn-text-loans-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body, expected) in cases {
        let file = dir.join(format!("{name}.tarn"));
        std::fs::write(&file, format!("import \"string\"\n{body}\n")).unwrap();
        let result = tarn_driver::check(&file).unwrap();
        if let Some(code) = expected {
            assert!(result.diagnostics.iter().any(|d| d.code == code), "{name}: {:?}", result.diagnostics);
        } else { assert!(!result.has_errors(), "{name}: {:?}", result.diagnostics); }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
