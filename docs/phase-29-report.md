# Phase 29 report: fair benchmarks, thread cache and proven arithmetic

Designs: [ADR 0057](adr/0057-proven-arithmetic.md) (proven arithmetic);
thread cache below; benchmarks in `benchmarks/vs-go`.

## What was implemented

1. **Fair benchmarks.** `Vec.with_capacity` in `core`; `vec` and `enums` now
   preallocate in both languages, and the Go JSON benchmark decodes into
   typed structs instead of `[]any`. Earlier "Tarn wins" there came from
   different allocation strategies and Go doing more work, not from code
   generation.
2. **Thread cache for `spawn`.** A spawn runs on an idle cached thread when
   one waits and on a new thread otherwise, so there are never fewer runnable
   threads than with thread-per-task: nested joins cannot deadlock (regression
   test with 32 joining parents on 8 CPUs). At most one idle thread per CPU
   is kept. Join waits on a per-task condition instead of `pthread_join`.
   Ownership, join-on-destruction, no detach and abort semantics (ADR 0033)
   are unchanged.
3. **Proven arithmetic.** Interval analysis on the IR; `AddProven`,
   `SubProven`, `MulProven` skip checks that cannot fail (ADR 0057).

## Results (Tarn / Go, median of 5, Go 1.26)

| Benchmark | Phase 28 (fair) | Phase 29 |
|---|---|---|
| arith | 1.5x | **1.0x** |
| fib | 1.1x | **1.0x** |
| vec | 1.1x | 1.2x (noise range) |
| strings | 2.5x | 2.6x |
| enums | 1.3x | 1.3x |
| jsoncodec | 0.9x | **0.8x** |
| parallel (8 workers) | 1.6x | 1.4x |
| tasks (10,000 small) | 15x | **2.5x** |

Peak memory stays at or below Go in every benchmark (Phase 28 measurements:
2.8x to 3.8x less in allocation-heavy ones; `tasks` 2.5 MB vs 3.3 MB).

## Tests

- `tests/native/pass/task_cache_nested`: more blocked joiners than CPUs.
- Task runtime fault injection updated: `calloc`, `pthread_create`,
  `pthread_cond_init` failures and a double join all abort.
- Range proofs: four abort cases at the edges (`i <= MAX` then `i + 1`,
  unbounded `*`, accumulator, signed lower bound) still abort; the CLI
  differential test runs every eligible native fixture with and without
  proofs and requires identical stdout and status.
- Goldens: six IR operations became `*_proven`; nothing else changed.

## Bugs found

1. Widening at every revisited block erased the loop condition's bound in
   the loop body, so nothing was proven. Widening now applies only at loop
   heads (back-edge targets); unvisited blocks count as heads.
2. Bodyless functions (externs, intrinsics) crashed the analysis; skipped.
3. The task fault test depended on `pthread_join` and the removed `thread`
   field; rewritten for the new failure points.

## Remaining gaps

- `strings` 2.6x: owned substrings (allocation per `split` part) and
  per-byte bounds checks in Tarn loops.
- `parallel` 1.4x and `enums` 1.3x: checks on values whose ranges come from
  parameters or data, which the analysis rightly does not prove.
- `tasks` 2.5x: a mutex and condition per task versus Go's scheduler.
- Server concurrency: `http.serve` and the async executor use one core
  (design in [ADR 0058](adr/0058-multicore-executor-design.md)).

## Final validation

`cargo test --workspace --no-fail-fast`: 268 passed, 0 failed, 1 ignored
(existing benchmark), no build warnings.
