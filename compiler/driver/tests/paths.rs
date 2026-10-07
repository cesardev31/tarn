//! Ordinary bundled path modules do not introduce native privileges.
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn local_path_module_can_override_the_ordinary_bundle() {
    let dir = std::env::temp_dir().join(format!("tarn-path-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("path.tarn"), "pub fn marker() i64 { return 42 }\n").unwrap();
    std::fs::write(
        dir.join("main.tarn"),
        "import \"path\"\nfn main() { print(path.marker()) }\n",
    )
    .unwrap();
    let result = tarn_driver::check(&dir.join("main.tarn")).unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn official_path_source_is_available_to_editor_analysis() {
    let entry = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../stdlib/path/path.tarn")
        .canonicalize()
        .unwrap();
    let source = std::fs::read_to_string(&entry).unwrap();
    let overlays = HashMap::from([(entry.clone(), source)]);
    let result = tarn_driver::check_editor_with_overlays(&entry, &overlays).unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
}
