# Phase 19 plan: integrated CLI

Status: 19A, 19B and 19C implemented. 19D is explicitly deferred under its
output-placement condition: build outputs still live beside sources.
Test conventions are settled in [ADR 0049](adr/0049-integrated-test-runner.md).
Baseline: Phase 18.

## 1. Goal and scope

Make `tarn` the single daily tool for small programs: check, build, run,
test, and iterate on change, without shell scripts or a second tool. It
must stay machine-friendly (stable exit codes, `--json`) as AGENTS.md
requires for coding agents.

Current state after Phase 19:

- Working: `check [--json]`, `resolve`, `types`, `ir`, `ast`, `lex`,
  `build [-o] [--link] [--json]`, `run [--link] [--json]`, `version`.
- Working: `test [--filter] [--timeout] [--json] [--link]`, and
  `check/run/test --watch`.
- Stubbed with exit 2: `fmt`, `clean`, `cache`.
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

First 19C delivery:

- `tarn check [entry] --watch [--json]` checks immediately and keeps running
  after compilation or load failures. Stop the session with Ctrl+C.
- The driver records files read from disk, including transitive local imports;
  embedded stdlib sources are excluded. No parser or import-resolution logic
  is duplicated in the watcher.
- Polling compares modification times and sizes every 250 ms. Changes are
  batched until every watched file is stable for one interval.
- Successful checks refresh the watched set. Failed checks retain previously
  observed files so deleting and restoring an import can recover the session.
  Initially missing imports that have never loaded are outside the watched set;
  edit a watched source after creating them to trigger another check.
- Rebuild headers include Unix time and changed paths on stderr. Status lines
  also use stderr; `--json` leaves stdout for diagnostic/error JSON Lines only.
- Tests cover live imported-file edits, compile-error recovery, deletion and
  recreation, source-set refresh, initially missing entry recovery, JSON output,
  debounce batching and exclusion of embedded stdlib sources.

Completed 19B and remaining 19C:

- ADR 0049 fixes test discovery, ordinary module visibility, process isolation,
  one build, argv selection and a configurable per-test timeout (10000 ms by
  default). No assertion module or external Rust dependency was introduced.
- The driver generates normal source harnesses in test-owning modules. Original
  application functions remain intact; a distinct verified entry reaches the
  existing monomorphization and post-drop backend pipeline.
- `tarn test` supports filters, JSON Lines, repeated link grants, panic/Err
  reporting, timeout, concurrent pipe draining and aggregate exit status.
- `run --watch` preserves the old process on compile/link failure, then uses
  SIGTERM, a two-second grace period, SIGKILL and reaping on replacement.
  Session shutdown cleans up owned process groups and temporary artifacts.
- `test --watch` reruns on source changes and sibling test-file additions or
  removals, and recovers from discovery/compilation/test failures.
- Tarn-written string/JSON behavior tests live in `tests/stdlib`; run them with
  `tarn test tests/stdlib`. CLI tests prove one native build for multiple tests,
  isolation after panic/Err, a hanging-test timeout, stream-draining safety,
  watch recovery and forced process replacement.

19D decision: keep `clean` stubbed, exactly as the plan permits. There is no
CLI-owned target directory or artifact inventory; deleting beside-source
outputs by guessing would violate the stage's safety requirement.

User guide: [integrated CLI](cli.md). Completed validation and deliberate
boundaries are recorded in the [Phase 19 report](phase-19-report.md).
