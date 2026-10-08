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

## Test maintenance: snapshot relevance filter

Golden IR, post-drop, move and provenance snapshots printed every
standard-library function with a body, so each `core` change rewrote dozens
of unrelated goldens (12,800 lines in Phase 23). The driver test helpers now
print a stdlib function only when an application function reaches it
(`relevant_functions`/`only_functions` in `compiler/driver/tests/common`;
references found through `FunctionId(n)` in the IR's Debug form). Compiler
output is unchanged: `tarn ir` still prints everything.

- 67 goldens lost 13,789 lines, all of them unreached stdlib bodies
  (Option/Result combinators, `string.view/choose`, five runtime helpers);
  no application function changed and no line was added.
- `tests/ir/pass/stdlib_reachable` pins the other direction: a reached
  combinator (`Option.unwrap_or`) is still printed, unreached ones are not.
- The four snapshot suites now hold 2,530 lines in total.
- Full workspace after the filter: 267 passed, 0 failed, 1 ignored.
