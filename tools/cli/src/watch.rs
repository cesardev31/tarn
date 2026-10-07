//! Dependency-free source polling. Compiler errors leave the watch session alive.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INTERVAL: Duration = Duration::from_millis(250);
type Stamp = Option<(SystemTime, u64)>;

fn stamp(path: &Path) -> Stamp {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[derive(Default)]
struct Sources {
    observed: BTreeMap<PathBuf, Stamp>,
    pending: BTreeSet<PathBuf>,
}

impl Sources {
    fn replace(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.observed = paths
            .into_iter()
            .map(|path| {
                let value = stamp(&path);
                (path, value)
            })
            .collect();
        self.pending.clear();
    }

    /// Emit one batch only after all watched files stay stable for an interval.
    fn poll(&mut self) -> Option<BTreeSet<PathBuf>> {
        let mut changed = false;
        for (path, previous) in &mut self.observed {
            let current = stamp(path);
            if current != *previous {
                *previous = current;
                self.pending.insert(path.clone());
                changed = true;
            }
        }
        if !changed && !self.pending.is_empty() {
            Some(std::mem::take(&mut self.pending))
        } else {
            None
        }
    }
}

pub fn check(entry: &Path, json: bool) -> ExitCode {
    let mut sources = Sources::default();
    sources.replace([entry.to_path_buf()]);
    let mut changed = BTreeSet::from([entry.to_path_buf()]);
    loop {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        eprintln!(
            "[watch {seconds}] checking after changes: {}",
            changed
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        // Keep the pre-check stamps: edits during compilation must cause a new check.
        match tarn_driver::check(entry) {
            Ok(result) => {
                let mut paths: BTreeSet<_> = result.program.disk_sources.iter().cloned().collect();
                paths.insert(entry.to_path_buf());
                if result.has_errors() {
                    // Retain deleted imports until recovery; the loader cannot read them.
                    paths.extend(sources.observed.keys().cloned());
                }
                sources.observed.retain(|path, _| paths.contains(path));
                for path in paths {
                    sources
                        .observed
                        .entry(path.clone())
                        .or_insert_with(|| stamp(&path));
                }
                for diagnostic in &result.diagnostics {
                    if json {
                        println!("{}", diagnostic.to_json(&result.program.sources));
                    } else {
                        eprint!("{}", diagnostic.render(&result.program.sources));
                    }
                }
                eprintln!(
                    "[watch] check {}",
                    if result.has_errors() {
                        "failed"
                    } else {
                        "succeeded"
                    }
                );
            }
            Err(error) => {
                if json {
                    super::native_error(true, "load", &error);
                } else {
                    eprintln!("error[E9001]: {error}");
                }
                eprintln!("[watch] check failed");
            }
        }
        let _ = std::io::stdout().flush();
        loop {
            std::thread::sleep(INTERVAL);
            if let Some(paths) = sources.poll() {
                changed = paths;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_edits_and_observes_deletion_and_recreation() {
        let dir = std::env::temp_dir().join(format!("tarn-watch-poll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("main.tarn");
        let second = dir.join("helper.tarn");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&second, "second").unwrap();
        let mut sources = Sources::default();
        sources.replace([first.clone(), second.clone()]);
        assert!(sources.poll().is_none());
        std::fs::write(&first, "first edit").unwrap();
        assert!(sources.poll().is_none());
        std::fs::write(&second, "second edit").unwrap();
        assert!(sources.poll().is_none());
        assert_eq!(
            sources.poll(),
            Some(BTreeSet::from([first.clone(), second.clone()]))
        );
        assert!(sources.poll().is_none());
        std::fs::remove_file(&second).unwrap();
        assert!(sources.poll().is_none());
        assert_eq!(sources.poll(), Some(BTreeSet::from([second.clone()])));
        std::fs::write(&second, "restored").unwrap();
        assert!(sources.poll().is_none());
        assert_eq!(sources.poll(), Some(BTreeSet::from([second])));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
