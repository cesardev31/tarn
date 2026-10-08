# Phase 24 report: process arguments, environment and the xlinux doctor port

Design: [ADR 0053](adr/0053-process-arguments-and-environment.md). This is
roadmap milestone 5: a real tool (`xlinux doctor`) ported to Tarn.

## What was implemented

- **24A**: bundled `os` module (`args`, `env`, `env_bytes`, `env_or`) and
  `tarn run file -- args...`, including under `--watch`; tool flags are read
  only before `--`.
- **24B**: `xlinux doctor` ported to Tarn in a separate project,
  `~/Documentos/proyectos/xlinux-tarn` (the xlinux repository is untouched).
  Read-only by design: the Python version may start the WiFi bridge as a
  side effect when no USB device is found; the port only reports.

## Validation

- Standard output of the port is byte-identical to the Python `xlinux
  doctor` on the reference machine with the external drive unmounted, and
  with `XLINUX_DATA=/tmp` simulating a present data directory (including the
  `statvfs` free-space figure). Exit status matches (1 when a required check
  fails).
- CLI test `program_arguments_and_environment`: arguments with spaces,
  UTF-8, empty strings and tool-like flags reach `os.args()` verbatim;
  `os.env` fallback, invalid UTF-8 error and `=` names; `--` rejected for
  `build`.
- Full workspace: see the final line.

## Evidence from the port (24C)

The port is 356 lines of Tarn against roughly 145 lines of Python for the
same behavior (doctor, the config and device helpers it uses, and the
adapter checks). Where the extra lines come from, most impactful first:

1. **Missing standard library facts, bridged with FFI** (`sys.tarn`, 49
   lines): executable permission (`access`), free disk space (`statvfs`,
   with manual struct offsets). `fs.Metadata` exposes neither permissions
   nor filesystem statistics.
2. **No process exit code.** `main` can only fail through `Result`, so the
   port prints `I/O error: not found` on stderr where Python exits silently
   with status 1. An `os.exit(code)` (or a main returning a status) is
   needed.
3. **No `Option`/`Result` equality** (`==` on enums is E3006 until an
   `Eq`-style interface exists): `x.text(&"k").unwrap_or("") == "USB"`.
4. **No hashing**: the macro stamp check shells out to `sha256sum`.
5. **No child environment** on `process.Command`: the port runs tools by
   absolute path instead of setting `PATH` for the child.
6. **Error plumbing**: probes that should just be false on failure still
   need `match` on `fs` results; `map`/`unwrap_or` (Phase 23) removed most of
   them in helpers, but nested `match` remains for JSON documents.
7. **String building**: `print("  ❌ " + label + &"\n       → " + hint)`
   needs `&` on every right operand; formatting/interpolation does not exist.

## Bugs found

1. Tool flags were detected anywhere in the arguments, so `tarn run app --
   --watch` would have switched the tool to watch mode. Fixed with a single
   `tool_args` boundary before `--`; covered by the CLI test.

## Decisions I would defend

- `os` as ordinary Tarn over `/proc` and `getenv`: zero compiler surface for
  a Linux-only toolchain, exact text handling.
- Porting into a separate project and comparing byte-for-byte against the
  original: the comparison, not the line count, is the acceptance criterion.

## Decisions I still question

- Reading `/proc/self/cmdline` ties `os.args()` to procfs.
- `env` returning `Result<Option<string>>` is precise but heavy at call
  sites; `env_or` helps, yet the port still wrapped it once more.

## Known limitations

- No `os.exit`, child environment, file permissions, filesystem statistics,
  hashing or string formatting in the standard library (see evidence).
- WiFi device discovery is reported, not started, by the port.

## Final validation

`cargo test --workspace --no-fail-fast`: 264 passed, 0 failed, 1 ignored
(existing benchmark), no build warnings. One existing CLI test asserted the
old rejection of `tarn run -- args`; it now asserts `--` is rejected for
`build` and `check`, where program arguments do not apply.
