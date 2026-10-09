use std::path::PathBuf;
#[test]
fn map_fallback_provenance_keeps_both_origins_alive() {
    let dir = std::env::temp_dir().join(format!("tarn-map-provenance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn");
    for assignment in ["fallback = 6", "map.insert(2, 6)"] {
        std::fs::write(
            &file,
            format!(
                r#"
import "collections"
fn main() {{
    var map: collections.Map<u64, u64> = collections.Map.new()
    var fallback: u64 = 5
    value := map.get_or(&1, &fallback)
    {assignment}
    print(*value)
}}
"#
            ),
        )
        .unwrap();
        let result = tarn_driver::check(&file).unwrap();
        assert!(result.has_errors(), "loan origin lost: {assignment}");
        assert!(
            result.diagnostics.iter().any(|d| d.code.starts_with("E41")),
            "{:?}",
            result.diagnostics
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn evidence_programs_check_without_warnings() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for source in [
        "evidence/status_device/collector.tarn",
        "examples/csv_report/main.tarn",
        "tests/native/pass/general_purpose_text.tarn",
        "tests/integration/console_lines_fixture.tarn",
    ] {
        let result = tarn_driver::check(&root.join(source)).unwrap();
        assert!(
            result.diagnostics.is_empty(),
            "{source}: {}",
            result
                .diagnostics
                .iter()
                .map(|d| d.render(&result.program.sources))
                .collect::<String>()
        );
    }
}

#[test]
fn recoverable_copy_lookup_rejects_owned_noncopy_values() {
    let dir = std::env::temp_dir().join(format!("tarn-map-copy-bound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("main.tarn");
    std::fs::write(
        &file,
        r#"
import "collections"
fn main() {
    var map: collections.Map<u64, string> = collections.Map.new()
    result := collections.get_copy(&map, &1)
    print(result.is_some())
}
"#,
    )
    .unwrap();
    let result = tarn_driver::check(&file).unwrap();
    assert!(result.diagnostics.iter().any(|d| d.code == "E3022"));
    std::fs::remove_dir_all(dir).unwrap();
}
