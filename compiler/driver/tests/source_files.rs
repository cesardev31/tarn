//! Disk-source metadata is authoritative for CLI watch sessions.
#[test]
fn records_transitive_disk_sources_and_excludes_embedded_modules() {
    let dir = std::env::temp_dir().join(format!("tarn-driver-sources-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    let helper = dir.join("helper.tarn");
    let leaf = dir.join("leaf.tarn");
    std::fs::write(
        &entry,
        "import \"helper\"\nimport \"io\"\nimport \"string\"\nfn main() {}\n",
    )
    .unwrap();
    std::fs::write(&helper, "import \"leaf\"\n").unwrap();
    std::fs::write(&leaf, "pub fn value() i32 { return 1 }\n").unwrap();
    let (program, _) = tarn_driver::load(&entry).unwrap();
    let actual: std::collections::BTreeSet<_> = program.disk_sources.into_iter().collect();
    assert_eq!(
        actual,
        std::collections::BTreeSet::from([entry.clone(), helper, leaf])
    );
    // A local fallback module really is a disk dependency, despite its stdlib name.
    let local_string = dir.join("string.tarn");
    std::fs::write(&local_string, "pub fn value() i32 { return 2 }\n").unwrap();
    let (program, _) = tarn_driver::load(&entry).unwrap();
    assert!(program.disk_sources.contains(&local_string));
    std::fs::remove_dir_all(dir).unwrap();
}
