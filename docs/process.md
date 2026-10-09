# Blocking child processes

```tarn
import "process"
import "io"
fn main() Result<void, io.Error> {
    command := process.Command.new("/bin/printf").arg("hello %s").arg("日本")
    output := try command.output()
    print(try output.stdout_text())
    print(output.status.success())
    return Ok(())
}
```

`Command.new(string)` consumes its program. `arg(string)` and
`current_dir(string)` consume the command and text; `args(&[]string)` copies
arguments. Calls can be chained. `start()`, `status()` and `output()` borrow the
configuration, so one command can launch several children. `start` is used
because `spawn` is a language keyword. No implicit shell or string tokenization
occurs; a space, dollar sign or semicolon in an argument is ordinary data.
An explicit `/bin/sh` command remains possible when the application wants one.

The environment and executable lookup PATH are inherited; `Command.env(name,
value)` overrides one child variable (Phase 25) without touching the parent. `current_dir` affects
only the child, never Tarn's global current directory. NUL, empty program and
empty explicit directory are InvalidInput; missing executable/directory is
NotFound, denied execution is PermissionDenied. Other native errors retain
`Error.native_code()`. All fallible APIs return `io.Error` with `try` support.

`start() -> Result<Process, io.Error>` inherits stdin/stdout/stderr. Process is
owned, non-Copy, Transfer and not Share. `id()` borrows and returns a diagnostic
PID; `kill()` borrows mutably and requests SIGKILL, keeping ownership.
`wait()` consumes completion authority, including on error. Destruction waits
and reaps a live child; it never detaches or implicitly kills it. Moving a child
transfers this responsibility, including into/through a native Task.

`status()` starts and waits. ExitStatus is Copy, with `Exited(i32)` and
`Signaled(i32)` cases; `success()`, `code()` and `signal()` are explicit queries.
Nonzero exit and signal termination are observations, not I/O failures.

`output()` uses /dev/null stdin and captures both streams as owned `Vec<u8>`.
Two native tasks drain the pipes concurrently before joining them, so large
stdout/stderr cannot deadlock through sequential draining. `Output` exposes
`status`, `stdout`, `stderr`; `stdout_text()` and `stderr_text()` validate UTF-8
and return owned copies or InvalidData. Binary/NUL bytes remain intact.

All operations are blocking on the current pthread. Captures are unbounded;
a child that never exits or descendants that hold pipe writers can block
completion indefinitely. Calling these APIs from an async body also blocks
its executor thread. There are no process timeouts, async process APIs, public pipes/stdin writers, process groups or detach in 15D.
Reader-task creation and buffer allocation retain the runtime's existing
abort-on-resource-failure policy.

The child closes unrelated fds >=3, inherits only the configured stdio, has an
empty signal mask and default SIGPIPE. The implementation uses posix_spawnp
and GNU libc 2.34+ spawn actions on Linux x86_64. External C must respect child
ownership: foreign waitpid or SIGCHLD policy changes can invalidate it.
See [ADR 0045](adr/0045-blocking-processes.md).
