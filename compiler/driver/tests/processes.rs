//! Process authority belongs to embedded declarations, never a file name.
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn local_process_source_cannot_replace_native_contracts() {
    let dir = std::env::temp_dir().join(format!("tarn-process-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("process.tarn");
    std::fs::write(&fake, "pub extern \"intrinsic\" fn stolen() i32\n").unwrap();
    std::fs::write(
        dir.join("main.tarn"),
        "import \"process\"\nfn main() { process.stolen() }\n",
    )
    .unwrap();
    let result = tarn_driver::check(&dir.join("main.tarn")).unwrap();
    assert!(result.diagnostics.iter().any(|d| d.code == "E2005"));
    let result = tarn_driver::check(&fake).unwrap();
    assert!(result.diagnostics.iter().any(|d| d.code == "E2027"));
    assert!(
        result
            .typed
            .as_ref()
            .is_none_or(|t| t.decls.process_owner.is_none())
    );
    std::fs::write(
        &fake,
        "import \"process\"\nfn main() { process.Command.new(\"/bin/true\").status() }\n",
    )
    .unwrap();
    assert!(!tarn_driver::check(&fake).unwrap().has_errors());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn official_process_source_is_checked_by_editor_with_real_trust() {
    let entry = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../stdlib/process/process.tarn")
        .canonicalize()
        .unwrap();
    let overlays = HashMap::from([(entry.clone(), std::fs::read_to_string(&entry).unwrap())]);
    let result = tarn_driver::check_editor_with_overlays(&entry, &overlays).unwrap();
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
}
