# Integrated CLI

Tarn currently targets Linux x86_64. `tarn build` and `tarn run` require `cc`
and system libraries explicitly granted with repeated `--link` flags.
The standard library and native runtime source are embedded in the compiler.

## Entries and commands

Source commands accept a file or directory. Without an entry, they use
`./main.tarn`; a directory selects its `main.tarn`. Imports remain relative
to the entry directory. Options may precede or follow the entry.

```sh
tarn check
tarn build app/ -o app/server
tarn run app/ --link sqlite3
tarn check --json
tarn run --watch
```

Unknown options, extra entries and missing option values exit 2 with usage.
Compilation, linking and test failures exit 1; successful commands exit 0.
Ordinary `run` propagates the program's exit status, including signals.
Program arguments after `--` are explicitly unsupported pending an argv API.

## Tests

```sh
tarn test
tarn test tests/stdlib
tarn test app/ --filter parser --timeout 2000
tarn test --json
tarn test --watch
```

The entry module and sibling `*_test.tarn` modules contribute `fn test_*()`
functions. A test is safe, synchronous, non-generic, takes no arguments and
returns void or `Result<void, E>`. Tests in a module can use that module's
private items. Sibling modules need ordinary public APIs to access application
items. Tests in imported helper modules are not discovered automatically.
The application's original `main` is not run by the harness.

The runner builds one executable and executes each selected test in a fresh
process, in module-qualified name order. `--filter` matches a substring of
that name. `--timeout` is a positive integer in milliseconds, default 10000.
No selected tests is a successful empty run. Panic, Err and timeout fail that
test and execution proceeds to the next. Tests do not need a main function.
No special assertion API is required: use conditionals and `panic`.

Output is captured concurrently from both streams. Failed text reports show
the captured output; JSON test records include `name`, `status` (`ok`, `FAILED`,
`timeout`), `duration_ms`, `stdout`, `stderr`, and nullable `exit_code`/`signal` fields. A final `test_summary` object
contains `total`, `passed` and `failed`. Output is decoded strictly: an invalid
UTF-8 stream has a null text field and a lossless `stdout_bytes` or `stderr_bytes`
array. The byte fields are null for valid text. Failed human reports show invalid
streams as byte arrays. Compiler diagnostics retain their usual JSON Lines.
Err reports include scalar values, enum variants with scalar payloads and
accessible scalar struct fields; nested non-scalar values are not recursively
formatted. There is no general debug reflection.

See [ADR 0049](adr/0049-integrated-test-runner.md).

## Watch sessions

`check`, `run` and `test` accept `--watch`, including with `--json`. Sessions
check immediately, poll timestamps and sizes every 250 ms, and wait for one
stable interval before rebuilding. Embedded stdlib files are not watched.
Headers and status messages go to stderr and identify time and changed files.

`run --watch` builds before replacing the program. Compile and link failures
keep the old program running. A successful build sends SIGTERM to its isolated
process group, waits up to two seconds, then uses SIGKILL if needed. Completed
children are reaped. Ctrl+C and SIGTERM stop the session and its owned child.
A program that has exited is started again only after a source change.

`test --watch` observes loaded imports and the sibling test-file set, including
new or removed test files. It keeps running after discovery, compilation or
test failures. Previously loaded files remain observed during failure so
restoration can recover. New missing imports that were never loaded require
an edit to a watched source after their creation.

`run --json` preserves the program's stdout/stderr unchanged; they are not JSON
records. CLI failures without source spans are `command_error` objects with
`stage` and `message`; usage errors remain text on stderr.

## Deferred commands

`fmt` belongs to Phase 20. `cache` and package commands are outside Phase 19.
`clean` remains a stub because build outputs live beside sources and the CLI
has no safe artifact inventory. It must never guess which files to delete.

## Formatting

`tarn fmt [file.tarn | directory] [--check]` formats recursively, defaulting
to `.`. `tarn fmt --stdin` formats a buffer to stdout. This command selects
source trees, rather than the `main.tarn` entry convention of compiler commands.
See [formatting](formatting.md) for selection, style and failure behavior.
