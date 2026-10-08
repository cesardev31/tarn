# Phase 28 report: native performance against Go

Design: [ADR 0056](adr/0056-native-performance-baseline.md). Benchmarks:
`benchmarks/vs-go` (identical output in both languages, median of 5 runs,
Go 1.26, release `tarn`, same machine).

## Results (Tarn time / Go time)

| Benchmark | Baseline | Optimizer on | No getenv per drop | Bulk primitives | Final |
|---|---|---|---|---|---|
| arith | 4.7x | 1.6x | 1.5x | 1.5x | **1.5x** |
| fib | 1.1x | 1.1x | 1.3x | 1.1x | **1.1x** |
| vec | 0.7x | 0.5x | 0.4x | 0.4x | **0.4x** (Tarn faster) |
| strings | 14.2x | 12.3x | 3.0x | 2.9x | **2.7x** |
| enums | 0.8x | 0.6x | 0.7x | 0.8x | **0.8x** (Tarn faster) |
| jsoncodec | 3.6x | 3.3x | 1.0x | 0.7x | **0.8x** (Tarn faster) |

Small differences between columns (for example fib 1.1x/1.3x) are run-to-run
noise at sub-0.1 s timings.

## Findings

1. The largest single cost was not code generation: `tarn_rt_drop_string`
   called `getenv("TARN_TRACE_DROPS")` on every string destruction, scanning
   the environment each time. Caching the trace switches cut `strings` from
   12.3x to 3.0x and `jsoncodec` from 3.3x to 1.0x.
2. Enabling Cranelift's optimizer took `arith` from 4.7x to 1.6x; it already
   strength-reduces `% 1000` to a multiply, like Go.
3. Byte-at-a-time copies (`Builder.push`, `http._copy`, the JSON input copy)
   became single `memcpy` calls.
4. Profiling needed readable symbols; `callgrind` (valgrind) is usable on
   this machine, `perf` is not (`perf_event_paranoid = 4`).

## Remaining gaps and why

- **arith 1.5x**: three overflow checks per iteration, which are Tarn's
  semantics (Go wraps silently).
- **strings 2.7x**: each `split` part is an owned allocation and free (ADR
  0042), while Go returns substrings that share memory; plus per-byte bounds
  checks in Tarn loops that Cranelift does not eliminate.

## Tests

- `tests/native/pass/bulk_primitives`: bulk append (bytes, empty slice,
  growth, `i64`), `copy_range` across a multibyte scalar, constant divisors
  (`/ 5`, `% 5`, `/ -1` still checked).
- Abort cases: `copy_range` inside a scalar, reversed and past the end; `%`
  by a zero variable.
- `E3022_extend_requires_copy`: bulk append rejects non-Copy elements.
- Every destruction, allocation and mutation oracle runs with the optimizer
  on.

## Final validation

`cargo test --workspace --no-fail-fast`: 267 passed, 0 failed, 1 ignored
(existing benchmark), no build warnings, with the optimizer enabled.
