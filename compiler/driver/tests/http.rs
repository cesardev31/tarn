//! HTTP is an ordinary module, never an intrinsic authority.
use std::{collections::HashMap, path::PathBuf};
#[test]
fn local_http_override_and_official_editor_source_have_no_native_trust() {
    let dir = std::env::temp_dir().join(format!("tarn-http-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("http.tarn"), "pub fn marker() i64 { return 42 }\n").unwrap();
    let entry = dir.join("main.tarn");
    std::fs::write(
        &entry,
        "import \"http\"\nfn main() { print(http.marker()) }\n",
    )
    .unwrap();
    let result = tarn_driver::check(&entry).unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert!(!result.program.trusted_modules.contains("http"));
    std::fs::write(
        dir.join("http.tarn"),
        "pub extern \"intrinsic\" fn forge() usize\n",
    )
    .unwrap();
    std::fs::write(
        &entry,
        "import \"http\"\nfn main() { print(http.forge()) }\n",
    )
    .unwrap();
    assert!(tarn_driver::check(&entry).unwrap().has_errors());
    std::fs::remove_dir_all(&dir).unwrap();
    let entry = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../stdlib/http/http.tarn")
        .canonicalize()
        .unwrap();
    let result = tarn_driver::check_editor_with_overlays(
        &entry,
        &HashMap::from([(entry.clone(), std::fs::read_to_string(&entry).unwrap())]),
    )
    .unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert!(!result.program.trusted_modules.contains("http"));
}
