# Phase 19 report: integrated CLI

Status: 19A/B/C implemented and validated.
19D is deferred under the plan's explicit output-placement condition.

## Delivered behavior

- One internal command parser, entry/directory discovery, consistent usage
  errors and native JSON failures (19A).
- An isolated test runner with deterministic module-qualified names, one
  executable, argv dispatch, filters, timeouts, concurrent stdout/stderr capture,
  JSON records and aggregate failure status (19B, ADR 0049).
- Check/run/test watch sessions. Run rebuild failures preserve the existing
  program; successful builds replace it using SIGTERM, a two-second grace period,
  SIGKILL and child reaping. Signals also clean up session-owned processes.
- Test watch observes imported sources plus added/removed sibling test roots and
  recovers from discovery, compilation and execution failures (19C).
- Five tests written in Tarn for string trimming/splitting/parsing and JSON
  parsing, invalid input and writing.

The generated harness stays ordinary source. It passes through the real
resolver, type checker, move/borrow analysis and post-drop elaboration. The
backend's explicit alternate entry reuses the normal specialization, validation
and linking pipeline. No external dependency or user-visible argv API was added.
Private application items retain their normal visibility across test modules.
Generated references bind explicitly to stable prelude symbols, so application
functions named `print` or `panic` cannot change harness reporting or failure
semantics; original application references are untouched.

## Deliberate boundaries

`clean` remains a stub: executables still live beside source files, with no
CLI-owned artifact inventory. The plan expressly defers 19D in this situation;
no guessed output deletion or new build-directory policy was introduced.
`fmt`, cache, package management, incremental compilation and platform expansion
remain outside this phase.

Err reporting displays scalars, enum variants with scalar payloads, and visible
scalar struct fields; it is not recursive debug reflection. CLI test output
validates UTF-8 and preserves invalid streams as byte arrays. Core collection is disabled
inside the private test entry after exec, preserving abort-only panic while
avoiding external collector delays being misreported as timeouts.

## Validation

- `cargo test -p tarn`: 19 Rust tests pass, including usage, JSON/native regression,
  one-link/multiple-test dispatch, panic and Err isolation, hanging-test timeout,
  simultaneous pipe draining, private-item rules, and live watch-process tests.
- `tarn test tests/stdlib`: all five Tarn-written behavior tests pass.
- Native watch tests prove compile/link failure keeps the existing PID alive,
  SIGTERM-resistant programs are killed after the grace period, replacement runs
  new source, and session shutdown reaps the current program.
- Test watch tests prove sibling additions, invalid-signature recovery, source
  edits and removals trigger new results, including edits immediately after a
  preceding summary.
- A strict C-runtime test caught an unsigned-comparison warning in the new argv
  selector. It was corrected and
  `cargo test -p tarn_backend --test execution thousands_of_coalesced_wakes_registration_cycles_and_retired_identities`
  passes, including the runtime's `-Werror` compilation.
- `cargo build -p tarn --release` passes. The release binary is installed in
  `~/.local/bin/tarn`; its SHA-256 matches `target/release/tarn`.
- An external project under `/tmp` passes installed `tarn run`, `tarn test --json`
  and filtered sibling-test execution. JSON records were decoded with Python's
  JSON parser, independently of the CLI's serializer.

The complete workspace run initially encountered sandbox EPERM in loopback
socket tests. Those targets pass when rerun outside the sandbox. The final aggregate
`cargo test --workspace --no-fail-fast -- --skip native_line_deletions_never_panic_or_emit_invalid_code --skip line_deletions_never_panic`
passes. Both skipped mutation tests already passed in the preceding complete
run (568 and 448 seconds respectively); that run exposed the corrected C warning.
All six affected native-runtime targets were also rerun successfully. One existing
benchmark remains ignored. The CLI and driver were rerun after the final harness
shadowing regression fix.

User guide: [integrated CLI](cli.md). Design: [ADR 0049](adr/0049-integrated-test-runner.md).
