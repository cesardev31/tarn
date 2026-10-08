# ADR 0053: process arguments and environment

Status: accepted. Phase 24A; [report](../phase-24-report.md).

## Context

Two independent needs: the Phase 19 CLI plan had to reject `tarn run -- args`
because no process-arguments API existed, and the `xlinux doctor` port needs
`HOME`, `PATH`, `XDG_*` and `XLINUX_DATA` plus its subcommand argument.

## Decisions

1. **An ordinary bundled module `os`** (like `json` and `http`): Tarn source
   over existing `fs` and `ffi`, with no intrinsics and no compiler or runtime
   change. `os.args()` reads `/proc/self/cmdline`, which on Linux is exactly
   the argv the kernel gave the process. `os.env*` calls libc `getenv`.
2. **Text is exact.** `args()` returns `Result<Vec<string>, io.Error>` and
   fails with InvalidData on non-UTF-8 arguments. `env(name)` returns
   `Result<Option<string>, io.Error>`: None when unset, InvalidData when not
   UTF-8. `env_bytes` gives raw bytes; `env_or` supplies a fallback. Names
   containing `=` or NUL are treated as unset. Nothing is decoded lossily.
3. **No environment mutation.** There is no `setenv`: `getenv` is safe with
   Tarn's native tasks only while nothing writes the environment.
4. **`tarn run file -- args...`** forwards everything after `--` verbatim,
   also under `--watch`. Tool flags are read only before `--`, so
   `tarn run app.tarn -- --watch` passes `--watch` to the program.

## Alternatives considered

- **Capture argc/argv in the generated `main`.** Independent of `/proc`, but
  needs backend, runtime and verified intrinsic-catalog changes for a value
  the kernel already exposes. Revisit if Tarn runs where `/proc` is not
  mounted.
- **Lossy decoding or skipping invalid arguments.** Rejected: it silently
  changes data (AGENTS.md string and filesystem rules).
- **A trusted stdlib layer.** Unnecessary: nothing here needs private native
  authority beyond public `ffi`.

## Consequences

`os.args()` costs one file read per call. Programs without `/proc` (rare
containers) get an io.Error rather than empty arguments.
