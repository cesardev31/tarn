use std::process::Command;
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tarn"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn catalog_uses_public_embedded_declarations_and_stable_identity() {
    let output = run(&["stdlib", "string", "--json"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["stdlib_hash"].as_str().unwrap().len(), 64);
    let entries = value["entries"].as_array().unwrap();
    assert!(entries.iter().any(|e| e["name"] == "string.rfind"
        && e["signature"].as_str().unwrap().contains("Option<usize>")
        && e["docs"].as_str().unwrap().contains("Last match")));
    assert!(!entries.iter().any(|e| e["name"] == "string.equal_at"));
    let list: serde_json::Value =
        serde_json::from_slice(&run(&["stdlib", "--json"]).stdout).unwrap();
    assert_eq!(list["stdlib_hash"], value["stdlib_hash"]);
    for name in list["modules"].as_array().unwrap() {
        let output = run(&["stdlib", name.as_str().unwrap(), "--json"]);
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let catalog: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(!catalog["entries"].as_array().unwrap().is_empty(), "{name}");
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/schema/stdlib.schema.json")).unwrap();
        let entry_schema = &schema["properties"]["entries"]["items"];
        for entry in catalog["entries"].as_array().unwrap() {
            for key in entry_schema["required"].as_array().unwrap() {
                assert!(entry.get(key.as_str().unwrap()).is_some());
            }
            for key in ["name", "kind", "signature", "docs"] {
                assert!(entry[key].is_string());
            }
            assert!(entry["line"].as_u64().unwrap() >= 1);
            assert!(
                entry_schema["properties"]["kind"]["enum"]
                    .as_array()
                    .unwrap()
                    .contains(&entry["kind"])
            );
        }
    }
}
#[test]
fn catalog_errors_are_machine_readable_usage_errors() {
    for args in [
        ["stdlib", "unknown", "--json"],
        ["stdlib", "--bad", "--json"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .lines()
                .any(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
        );
    }
}
