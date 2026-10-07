# Phase 19 plan: integrated CLI

Status: 19A complete. Stages 19B and 19C have not started; 19D remains
deferred while outputs live beside sources. An ADR is required before 19B
(test discovery conventions) because they become part of the language contract.
Baseline: Phase 18.

## 1. Goal and scope

Make `tarn` the single daily tool for small programs: check, build, run,
test, and iterate on change, without shell scripts or a second tool. It
must stay machine-friendly (stable exit codes, `--json`) as AGENTS.md
requires for coding agents.

Current state after 19A:

- Working: `check [--json]`, `resolve`, `types`, `ir`, `ast`, `lex`,
  `build [-o] [--link] [--json]`, `run [--link] [--json]`, `version`.
- Stubbed with exit 2: `test`, `fmt`, `clean`, `cache`.
- Shared internal argument validation; source entries default to `main.tarn`.

Included: consistent argument handling, a project entry convention, `tarn
test`, `--watch`, JSON output for native commands, `clean`.

Excluded: `fmt` (Phase 20 formatter; the command stays stubbed), `tarn.toml`,
package commands (`add`, `update`, `deps`, `audit`, `verify`, `publish`),
caching (`cache`), incremental compilation, parallel test scheduling beyond
processes, Windows/macOS.

## 2. Stages

### 19A: command surface

- One small internal argument parser (no external crate): positional entry,
  `--flag value`, repeated flags, `--` before program arguments for `run`.
  Unknown flags exit 2 with the command's usage.
- Entry discovery: with no file, use `./main.tarn`; a directory argument
  means `<dir>/main.tarn`. This matches existing local imports, which are
  already relative to the entry directory.
- `tarn run app.tarn -- arg1 arg2`: forward program arguments, which
  requires an `os.args`-style API decision. If that API is not ready, reject
  `--` with a clear message rather than ignoring arguments.
- Exit codes documented: 0 success, 1 compile/link/test failure, 2 usage
  error; `run` propagates the program's code (existing behavior).
- `build --json` and `run --json`: compiler diagnostics as the existing JSON
  Lines, plus one JSON object for native backend and link failures (today
  plain text).

Gate: CLI tests for every command's usage errors, entry discovery, JSON
output shape, and unchanged behavior of existing commands.

### 19B: `tarn test` (requires ADR)

Proposed convention, to be settled in the ADR:

- Test functions are `fn test_<name>()` with no parameters, returning void
  or `Result<void, E>`. Files: the entry module and any `*_test.tarn` next
  to it, so tests can live beside the code they exercise.
- Each test runs in its own process. Panic is abort-only (no unwinding), so
  process isolation is the only way to keep running after a failing test;
  it also isolates sockets, files and threads.
- Build once: one executable whose generated entry selects the test by
  name (`argv`), instead of one compilation per test.
- Output: one line per test (`ok`, `FAILED`, with panic message or Err
  value), a summary, exit 1 on any failure; `--json` emits one object per
  test; `--filter <substring>`.
- Assertions use existing `panic` plus small helpers in the reserved
  `testing` module (`testing.equal`, `testing.check`) only if the CRUD and
  stdlib tests show they remove real repetition.

Open questions for the ADR: naming convention vs. an attribute (Tarn has no
attributes today), whether tests may access private items of the module
under test (same module: yes; `*_test.tarn` files: they are separate
modules today), and timeouts per test.

Gate: the stdlib `string`/`json` behaviors gain Tarn-written tests; a
failing test, a panicking test, an Err-returning test and a hanging test
with a timeout are all reported correctly.

### 19C: `--watch`

`tarn run --watch`, `tarn test --watch`, `tarn check --watch`.

- Watched set: exactly the source files the driver loaded (entry plus local
  imports), refreshed after each successful load, plus `tarn.toml` later.
  Embedded stdlib sources are not watched.
- Detection: poll modification times and sizes every 250 ms. It needs no
  dependency, works over any filesystem, and its latency is acceptable for
  edit-save loops. inotify only if measurements show polling cost matters.
- Debounce: wait until the set is stable for one poll interval, so editors
  that write files in several steps trigger one rebuild.
- Restart policy for `run`: build first; on failure print diagnostics and
  keep the old process running; on success send SIGTERM, wait up to 2 s,
  then SIGKILL, and start the new binary. Listeners rebind immediately
  thanks to SO_REUSEADDR (Phase 17).
- Clear separation in the output between rebuilds (a header line with time
  and changed file).

Gate: a test that edits a watched file and observes the restarted program's
new output; a compile error keeps the old process alive; a program that
ignores SIGTERM is killed after the grace period.

### 19D: `clean`

There is no build cache yet. `clean` removes `tarn build` outputs that the
CLI itself recorded, never arbitrary files. If 19A keeps outputs next to the
source (current behavior), defer `clean` until a `target/` directory or the
planned content-addressed store exists, and keep the stub.

## 3. Risks

- Test discovery by name prefix is a convention that is hard to change
  later; hence the ADR.
- Forwarding program arguments needs a process-arguments API that does not
  exist yet; it should be designed with the `process` module owners in mind.
- `--watch` adds the CLI's first long-running loop and child management;
  keep it in one module with its own tests.

## 4. Order and estimate

19A first (unblocks the rest), then 19C (small, high daily value), then 19B
(largest; needs the ADR), then 19D only if a build directory exists.

## 5. Implementation progress

Completed 19A:

- Shared internal argument validation for all commands; unknown options, extra
  positional arguments and missing flag values exit 2 with command usage.
- Source commands default to `main.tarn` and accept directories. Options may
  precede the entry; native commands preserve repeated `--link` grants.
- `run -- ...` explicitly rejects program arguments until their API is designed.
- Exit codes are described in CLI help. Existing output placement is preserved.
- Public CLI regressions cover usage errors, entry discovery and option ordering.

- Native `build --json` / `run --json` preserve frontend diagnostic JSON Lines.
  Failures without source spans emit one object on stdout with `kind` equal to
  `command_error`, a `stage` (`load`, `internal`, `output`, `native`, or `execute`),
  and an escaped `message`. `native` covers backend and linker errors.
- Successful builds emit no JSON record. `run` inherits the program's stdout
  and stderr unchanged and propagates its exit status; program output is not
  wrapped in JSON. Usage errors remain text on stderr with exit 2.
- Validation: `cargo test -p tarn` passes command-surface, native JSON, build/run,
  and link regressions. The JSON gate covers compiler diagnostics, escaped
  messages, source-overwrite protection, unsupported backend operations,
  missing libraries and unresolved symbols.

Next: 19C watch support, then the ADR and implementation for 19B. 19D stays
stubbed because build outputs still live beside their sources.
