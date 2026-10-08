# Phase 25 report: standard library gaps found by the xlinux doctor port

Design: [ADR 0053](adr/0053-process-arguments-and-environment.md), Phase 25
extension. Evidence: [Phase 24 report](phase-24-report.md).

## What was implemented

- `os.exit(code)`, `os.can_execute(path)`, `os.disk_space(path)` in the
  bundled `os` module, written in Tarn over `ffi`.
- `io.Error.from_os(code)` in the trusted `io` module (ordinary Tarn, reusing
  the filesystem errno table).
- `process.Command.env(key, value)` through the verified native spawn
  bridge: Tarn declaration, ABI verifier, backend symbol and C runtime.

## Effect on the port

| `xlinux-tarn` | Phase 24 | Phase 25 |
|---|---|---|
| Lines of Tarn | 356 | 303 |
| Own FFI module | `sys.tarn` (49 lines) | none |
| Exit on failed checks | prints `I/O error: not found` | silent status 1, as Python |
| stdout vs Python | identical | identical |

## Tests

- `tests/native/pass/os_system`: executable permission (executable,
  regular file, missing, NUL), sane `disk_space` for `/`, NotFound with errno
  2 for a missing path.
- `tests/native/pass/process_environment`: two overrides, last value wins,
  unset in the child without overrides, three invalid inputs fail to spawn,
  parent environment unchanged.
- CLI `os_exit_sets_the_status_and_flushes_output`: status 7, earlier output
  flushed, later output not run.
- The existing C runtime harness (`process_runtime.c`) was updated for the
  new spawn signature; the `imports` resolution golden only shifted lines.

## Bugs and evidence found

1. A Tarn `extern "C"` name is also the C symbol, so `os.exit` cannot sit
   beside a libc `exit` declaration. Worked around with `fflush` and
   `_exit`. A link-name alias for extern declarations is now evidenced.
2. `==` on enums (`e.kind == io.ErrorKind.NotFound`) is still E3006; a
   second program hit it (evidence for the planned `Eq`).

## Decisions I would defend

- Keeping these in Tarn over `ffi` instead of new intrinsics: no compiler
  surface, and they are trivially replaceable later.
- `Command.env` inherits and overrides (no implicit clearing): matches what
  the Python tool did with `os.environ.copy()` and avoids losing `PATH`.

## Decisions I still question

- `os.exit` skips Tarn destruction; correct for a process-ending call, but a
  buffered writer that was not flushed loses data, as documented.
- The statvfs layout is hard-coded for glibc x86_64, matching the only
  supported platform.

## Known limitations

- No `env_clear`/`env_remove` for children; no file mode bits beyond the
  executable check; no hashing or string formatting.

## Final validation

`cargo test --workspace --no-fail-fast`: 265 passed, 0 failed, 1 ignored
(existing benchmark), no build warnings.
