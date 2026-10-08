# Phase 27 report: static string literals

Design: [ADR 0055](adr/0055-static-string-literals.md).

## What was implemented

- Backend: literals become deduplicated read-only `TarnString` images in the
  `tarn_strings` section; a literal's value is its address (no runtime call).
- Runtime: `tarn_rt_drop_string` skips `free` for addresses inside that
  section, after the optional destruction trace.

## Measurements

Loop of 10,000,000 calls passing `"GET /items HTTP/1.1"` to a function,
same source, previous commit (`aa8675a`) vs this phase:

| | `malloc` calls | Time |
|---|---|---|
| Before | 10,000,001 | 1.04 s |
| After | 1 | 0.94 s |

The time gain is modest because the backend does not optimize yet; the
allocation count is the structural improvement (no allocator pressure or
fragmentation from literals in loops and handlers).

## Tests

- `combinators.rs::string_literals_do_not_allocate_per_evaluation`: 100,000
  literal evaluations make fewer than 100 allocations in total (measured with
  an `LD_PRELOAD` malloc counter), the literal is still destroyed 100,000
  times as a value, and values built from literals (moved into a `Vec`,
  cloned, concatenated) are each destroyed exactly once.
- The whole existing suite exercises literals through every path (drop
  traces, async frames, closures, HTTP/JSON/SQLite programs).

## Decisions I would defend

- Releasing by address in the single release function keeps ownership
  semantics and post-drop verification untouched.

## Decisions I still question

- Correctness depends on GNU linker bracket symbols; documented, and the
  toolchain is already fixed to the system `cc` on Linux.

## Known limitations

- Strings produced at run time (clone, concatenation, formatting, I/O) still
  allocate, as they must.

## Final validation

`cargo test --workspace --no-fail-fast`: 267 passed, 0 failed, 1 ignored
(existing benchmark), no build warnings.
