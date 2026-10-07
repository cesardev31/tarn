//! Sequential isolated test execution over one frontend-generated executable.
use std::io::Read;
use std::path::Path;
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

pub struct Options {
    pub json: bool,
    pub filter: String,
    pub timeout: Duration,
    pub libraries: Vec<String>,
}
impl Options {
    pub fn parse(args: &[String]) -> Self {
        let mut options = Self {
            json: false,
            filter: String::new(),
            timeout: Duration::from_secs(10),
            libraries: Vec::new(),
        };
        let mut rest = args[1..].iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--json" => options.json = true,
                "--filter" => options.filter = rest.next().unwrap().clone(),
                "--timeout" => {
                    options.timeout = Duration::from_millis(rest.next().unwrap().parse().unwrap())
                }
                "--link" => options.libraries.push(rest.next().unwrap().clone()),
                "--watch" => {}
                _ => unreachable!("validated test option"),
            }
        }
        options
    }
}

fn capture(mut stream: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        bytes
    })
}
fn quoted(text: &str) -> String {
    let mut result = String::new();
    super::json_str(&mut result, text);
    result
}

/// Return loaded paths even after diagnostic failure so watch can refresh.
pub fn run(entry: &Path, options: &Options) -> (ExitCode, Vec<std::path::PathBuf>) {
    run_with_sources(entry, options, |_, _| {})
}

pub fn run_with_sources(
    entry: &Path,
    options: &Options,
    mut loaded: impl FnMut(&[std::path::PathBuf], bool),
) -> (ExitCode, Vec<std::path::PathBuf>) {
    let (result, cases) = match tarn_driver::testing::check_tests(entry) {
        Ok(result) => result,
        Err(error) => {
            let mut paths = tarn_driver::load(entry)
                .map(|(program, _)| program.disk_sources)
                .unwrap_or_default();
            paths.push(entry.to_path_buf());
            paths.extend(tarn_driver::testing::test_files(entry).unwrap_or_default());
            loaded(&paths, true);
            super::native_error(options.json, "test_discovery", &error);
            return (ExitCode::from(1), paths);
        }
    };
    let paths = result.program.disk_sources.clone();
    loaded(&paths, result.has_errors());
    for diagnostic in &result.diagnostics {
        if options.json {
            println!("{}", diagnostic.to_json(&result.program.sources));
        } else {
            eprint!("{}", diagnostic.render(&result.program.sources));
        }
    }
    if result.has_errors() {
        return (ExitCode::from(1), paths);
    }
    let mut selected: Vec<_> = cases
        .iter()
        .filter(|case| case.name.contains(&options.filter))
        .collect();
    selected.sort_by(|left, right| left.name.cmp(&right.name));
    let scratch = match super::process::Scratch::new() {
        Ok(scratch) => scratch,
        Err(error) => {
            super::native_error(options.json, "output", &error.to_string());
            return (ExitCode::from(1), paths);
        }
    };
    let executable = scratch.0.join("tests");
    if !selected.is_empty() {
        let drops = result.drops.as_ref().expect("checked post-drop IR");
        let entry = drops
            .functions
            .iter()
            .find(|function| function.decl.name == tarn_driver::testing::ENTRY)
            .expect("generated test entry")
            .decl
            .id;
        if let Err(error) = tarn_backend::build_tests(
            drops,
            result.typed.as_ref().unwrap(),
            &executable,
            &options.libraries,
            entry,
        ) {
            super::native_error(options.json, "native", &error.to_string());
            return (ExitCode::from(1), paths);
        }
    }
    let mut failed = 0;
    let mut completed = 0;
    for case in selected {
        if super::process::stopping() {
            break;
        }
        let started = Instant::now();
        let mut child = match super::process::command(&executable)
            .arg(case.index.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                super::native_error(options.json, "execute", &error.to_string());
                return (ExitCode::from(1), paths);
            }
        };
        let stdout = capture(child.stdout.take().unwrap());
        let stderr = capture(child.stderr.take().unwrap());
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    super::process::send(&child, 9);
                    break Some(status);
                }
                Ok(None) if started.elapsed() < options.timeout && !super::process::stopping() => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                _ => {
                    timed_out = !super::process::stopping();
                    super::process::terminate(&mut child, Duration::ZERO);
                    break None;
                }
            }
        };
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        let stdout_text = std::str::from_utf8(&stdout).ok();
        let stderr_text = std::str::from_utf8(&stderr).ok();
        let ok = status.is_some_and(|status| status.success());
        completed += 1;
        if !ok {
            failed += 1;
        }
        let label = if ok {
            "ok"
        } else if timed_out {
            "timeout"
        } else {
            "FAILED"
        };
        use std::os::unix::process::ExitStatusExt;
        let exit_code = status.and_then(|status| status.code());
        let signal = status.and_then(|status| status.signal());
        if options.json {
            println!(
                "{{\"kind\":\"test\",\"name\":{},\"status\":{},\"duration_ms\":{},\"stdout\":{},\"stderr\":{},\"exit_code\":{},\"signal\":{},\"stdout_bytes\":{},\"stderr_bytes\":{}}}",
                quoted(&case.name),
                quoted(label),
                started.elapsed().as_millis(),
                stdout_text.map_or_else(|| "null".into(), quoted),
                stderr_text.map_or_else(|| "null".into(), quoted),
                exit_code.map_or_else(|| "null".into(), |value| value.to_string()),
                signal.map_or_else(|| "null".into(), |value| value.to_string()),
                if stdout_text.is_some() {
                    "null".into()
                } else {
                    format!("{stdout:?}")
                },
                if stderr_text.is_some() {
                    "null".into()
                } else {
                    format!("{stderr:?}")
                }
            );
        } else {
            println!("{} ... {label}", case.name);
            if !ok {
                if timed_out {
                    eprintln!("timeout after {} ms", options.timeout.as_millis());
                } else if let Some(signal) = signal {
                    eprintln!("terminated by signal {signal}");
                } else if let Some(code) = exit_code {
                    eprintln!("exited with code {code}");
                }
                if let Some(text) = stdout_text {
                    print!("{text}");
                } else {
                    println!("stdout (bytes): {stdout:?}");
                }
                if let Some(text) = stderr_text {
                    eprint!("{text}");
                } else {
                    eprintln!("stderr (bytes): {stderr:?}");
                }
            }
        }
    }
    if options.json {
        println!(
            "{{\"kind\":\"test_summary\",\"total\":{completed},\"passed\":{},\"failed\":{failed}}}",
            completed - failed
        );
    } else {
        println!(
            "test result: {} passed; {failed} failed",
            completed - failed
        );
    }
    (
        if failed == 0 && !super::process::stopping() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        },
        paths,
    )
}
