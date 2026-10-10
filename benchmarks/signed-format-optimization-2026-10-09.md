# Signed decimal formatting optimization — 2026-10-09

Negative `string.from_i64` previously formatted an owned unsigned magnitude,
then concatenated the sign, allocating and copying another owned string.
The negative encoder now writes sign and magnitude into a 21-byte stack buffer
and creates one owned string via the existing checked UTF-8 conversion.
The minimum i64 magnitude still uses `u64(-(value + 1)) + 1`, avoiding overflow.
Positive values continue to use from_u64. A small public dispatch delegates to
the private negative encoder, retaining a small inlining candidate.
No compiler Rust/C code, intrinsic, dependency or ownership rule changed.

## Final alternating comparison

Before compiler: `/tmp/tarn-before-signed-format-opt`, including prior Builder
and split changes. Both compilers are release builds and compile identical
sources. Nine executions per compiler alternate ordering, measured using
perf_counter with exact stdout assertions. No other agent benchmark/profile
ran during the alternating comparison; desktop applications remained active.

| Workload | Before median s | After median s | Time reduction |
|---|---:|---:|---:|
| Eight negative values, 1 million rounds | 0.574910 | 0.445576 | 22.5% |
| Corresponding positive control | 0.413253 | 0.415261 | -0.5% |

The positive difference is within observed sample variation, not a demonstrated
speedup. A preliminary version with the encoder inside from_i64 measured a 3.2%
positive slowdown; the small-dispatch version reduced that difference while
retaining the substantial negative improvement. The inliner has a size-based
candidate limit; exact causality was not isolated through an inlining-disabled
experiment. Raw samples: [JSON](signed-format-optimization-2026-10-09.json).

The negative benchmark formats -1, -9, -10, -99, -100, -123456789,
-1000000000000000000 and i64 minimum. It produces 64,000,000. The positive control
uses their positive magnitudes, replacing i64 minimum with i64 maximum, and
produces 56,000,000. `./benchmarks/vs-go/run.sh format-i64` checks Tarn/Go output
parity and measured 0.44/0.38 seconds (1.2x). This is a targeted formatting
benchmark, not a general text-performance claim.

## Allocation evidence

Valgrind ran the same negative workload reduced to 1,000 rounds (8,000 conversions).
Both versions printed 64,000; both had zero errors and zero bytes live at exit.

| Version | Allocations | Frees | Total bytes allocated |
|---|---:|---:|---:|
| Before | 16,001 | 16,001 | 252,096 |
| After | 8,001 | 8,001 | 132,096 |

The extra single allocation is process overhead. Conversion allocation count
halved; total allocated bytes are not peak RSS. Logs remain under
`/tmp/tarn-signed-alloc/` (before-final.log and after-final.log).

## Validation and installation

- Offline release build passed.
- Stdlib suite: 8 passed, 0 failed. New expected-text and parse-roundtrip checks
  cover signed boundaries, including i64 maximum/minimum and decimal transitions.
- Native text_ranges, string_builder and string_essentials matched exact golden
  stdout with empty stderr.
- Formatting, shell syntax and whitespace checks passed.
- The final release was installed. Installed and build SHA-256 match:
  `92db282dc85fba3398e3d21ac01a681bc078b86c75c974a6786cdaa539d54cfc`.
- The full workspace suite was not run. Measurements are not CPU-isolated.
