//! Exercise a live check session with imports, failures and source-set refresh.
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

struct Session {
    child: Child,
    stderr: Receiver<String>,
    stdout: Receiver<String>,
}

fn lines(reader: impl std::io::Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            if sender.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    receiver
}

impl Session {
    fn start(entry: &Path, json: bool) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tarn"));
        command.args(["check", "--watch"]).arg(entry);
        if json {
            command.arg("--json");
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = lines(child.stderr.take().unwrap());
        let stdout = lines(child.stdout.take().unwrap());
        Self {
            child,
            stderr,
            stdout,
        }
    }

    fn wait(&self, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        loop {
            let line = self
                .stderr
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| panic!("missing {expected}; output: {seen:?}"));
            if line.contains(expected) {
                return;
            }
            seen.push(line);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn watches_imports_recovers_and_refreshes_the_loaded_set() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    let helper = dir.join("helper.tarn");
    std::fs::write(&helper, "pub fn value() i32 { return 1 }\n").unwrap();
    std::fs::write(
        &entry,
        "import \"helper\"\nfn main() { print(helper.value()) }\n",
    )
    .unwrap();
    let mut session = Session::start(&entry, false);
    session.wait("check succeeded");
    std::fs::write(&helper, "pub fn value() i32 { return unknown_name }\n").unwrap();
    session.wait("helper.tarn");
    session.wait("check failed");
    assert!(session.child.try_wait().unwrap().is_none());
    std::fs::remove_file(&helper).unwrap();
    session.wait("check failed");
    std::fs::write(&helper, "pub fn value() i32 { return 123 }\n").unwrap();
    session.wait("check succeeded");
    let next = dir.join("next.tarn");
    std::fs::write(&next, "pub fn value() i32 { return 2 }\n").unwrap();
    std::fs::write(
        &entry,
        "import \"next\"\nfn main() { print(next.value()) }\n",
    )
    .unwrap();
    session.wait("check succeeded");
    std::fs::write(&helper, "broken unused source").unwrap();
    assert!(
        session
            .stderr
            .recv_timeout(Duration::from_millis(900))
            .is_err()
    );
    std::fs::write(&next, "pub fn value() i32 { return another_unknown }\n").unwrap();
    session.wait("check failed");
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_entry_recovers_and_json_stdout_contains_only_records() {
    let dir = std::env::temp_dir().join(format!("tarn-cli-watch-json-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.tarn");
    let session = Session::start(&entry, true);
    session.wait("check failed");
    let record = session.stdout.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(record.starts_with("{\"kind\":\"command_error\",\"stage\":\"load\""));
    std::fs::write(&entry, "fn main() { print(unknown_name) }\n").unwrap();
    session.wait("check failed");
    let record = session.stdout.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(record.starts_with("{\"code\":"));
    assert!(record.contains("\"severity\":\"error\""));
    std::fs::write(&entry, "fn main() {}\n").unwrap();
    session.wait("check succeeded");
    assert!(
        session
            .stdout
            .recv_timeout(Duration::from_millis(750))
            .is_err()
    );
    drop(session);
    std::fs::remove_dir_all(dir).unwrap();
}
