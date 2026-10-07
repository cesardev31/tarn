//! Linux child process groups and watch-session shutdown, without a dependency.
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static STOP: AtomicBool = AtomicBool::new(false);
unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
    fn signal(number: i32, handler: usize) -> usize;
}
extern "C" fn stop(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}
pub fn install_handlers() {
    STOP.store(false, Ordering::Relaxed);
    // The handler only stores an atomic flag; children have their own process group.
    unsafe {
        signal(2, stop as *const () as usize);
        signal(15, stop as *const () as usize);
    }
}
pub fn stopping() -> bool {
    STOP.load(Ordering::Relaxed)
}
pub fn command(path: &Path) -> Command {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(path);
    command.process_group(0);
    command
}
pub fn send(child: &Child, number: i32) {
    // Only the process group created for this owned child is addressed.
    unsafe {
        kill(-(child.id() as i32), number);
    }
}
pub fn terminate(child: &mut Child, grace: Duration) {
    send(child, 15);
    let deadline = Instant::now() + grace;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                send(child, 9);
                return;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => break,
        }
    }
    send(child, 9);
    let _ = child.wait();
}

pub struct Scratch(pub PathBuf);
impl Scratch {
    pub fn new() -> std::io::Result<Self> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tarn-cli-{}-{time}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
