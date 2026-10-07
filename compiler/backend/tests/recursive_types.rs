//! Phase 17: types that reach themselves through `Vec` storage.
use std::process::Command;

fn run(source: &str, tag: &str) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!("tarn-recursive-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    std::fs::write(&entry, source).unwrap();
    let result = tarn_driver::check(&entry).unwrap();
    assert!(
        !result.has_errors(),
        "{}",
        result.diagnostics.iter().map(|d| d.render(&result.program.sources)).collect::<String>()
    );
    let exe = dir.join("program");
    tarn_backend::build(result.drops.as_ref().unwrap(), result.typed.as_ref().unwrap(), &exe).unwrap();
    let output = Command::new("timeout").arg("30s").arg(&exe).env("TARN_TRACE_DROPS", "1").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::remove_dir_all(dir).unwrap();
    output
}

fn drops(output: &std::process::Output) -> Vec<String> {
    let mut drops: Vec<String> = String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter_map(|l| l.strip_prefix("drop:"))
        .map(str::to_string)
        .collect();
    drops.sort();
    drops
}

/// Direct (`List(Vec<Value>)`) and indirect (`Vec<Member>` holding a Value)
/// recursion: every owned string at every depth is destroyed exactly once,
/// including a value moved out of the tree before the tree is destroyed.
#[test]
fn recursive_enum_through_vec_destroys_every_level_once() {
    let source = r#"
struct Member {
    key string
    value Value
}
enum Value {
    Null
    Text(string)
    List(Vec<Value>)
    Object(Vec<Member>)
}
fn leaf(text string) Value { return Value.Text(text) }
fn count(value &Value) usize {
    match value {
        Value.List(items) => {
            var n: usize = 1
            for item in items.as_slice() { n = n + count(item) }
            return n
        }
        Value.Object(members) => {
            var n: usize = 1
            for member in members.as_slice() { n = n + count(&member.value) }
            return n
        }
        _ => { return 1 }
    }
}
fn main() {
    var inner: Vec<Value> = Vec.new()
    inner.push(leaf("deep"))
    inner.push(Value.Null)
    var members: Vec<Member> = Vec.new()
    members.push(Member{key: "list", value: Value.List(inner)})
    members.push(Member{key: "name", value: leaf("tarn")})
    var outer: Vec<Value> = Vec.new()
    outer.push(Value.Object(members))
    outer.push(leaf("kept"))
    kept := outer.pop()
    tree := Value.List(outer)
    print(count(&tree))
    match kept {
        Some(value) => { print(count(&value)) }
        None => {}
    }
}
"#;
    let output = run(source, "tree");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "6\n1\n");
    assert_eq!(drops(&output), ["deep", "kept", "list", "name", "tarn"]);
}
