# String split optimization — 2026-10-09

The previous release rerun identified repeated split scanning and cursor overhead.
This change keeps owned split results and offset-only range results, including
empty fields, non-overlapping separators and empty-separator Unicode scalar splits.

## Implementation

- `split_ranges` scans in one function instead of repeatedly calling `next_split`.
  Its cursor cannot be replaced or used with another source between steps, so
  public-cursor validation is unnecessary inside this complete traversal.
  The public `next_split` retains its existing validation.
- `split` and `split_ranges` short-circuit full separator comparison after a
  matching first byte when the separator has exactly one byte. A valid UTF-8
  string with one byte is ASCII, so this preserves scalar boundaries.
- No new intrinsic, unsafe conversion, ownership exception or dependency.

## Measurements

Both compilers were built in release. The previous compiler was preserved at
`/tmp/tarn-before-string-opt`. Both compiled the same unchanged benchmark sources
before measuring. Nine executions per binary alternated before/after order,
using Python perf_counter and checking every output equals 4,200,000.
No other agent benchmark ran concurrently. Desktop applications remained active.
Raw samples: [JSON](string-optimization-2026-10-09.json).

| Benchmark | Before median s | After median s | Time reduction |
|---|---:|---:|---:|
| strings | 0.506336 | 0.484424 | 4.3% |
| strings-ranges | 0.468624 | 0.346677 | 26.0% |

The separate five-run Tarn/Go runner measured strings at 0.51/0.23 seconds
(2.2x) and ranges at 0.34/0.23 seconds (1.5x). The small owned-string improvement
should be treated cautiously; the ranges improvement is much larger than sample
variation. This does not claim parity with Go or solve substring allocation cost.

## Validation and installation

- `cargo build --release -p tarn --offline` passed.
- `tarn test tests/stdlib --json`: 6 passed, 0 failed, including the new test
  covering 56 source/separator combinations, Unicode boundaries, empty inputs,
  repeated/overlapping separators and parity with the unchanged public cursor.
- Native text_ranges, string_builder and string_essentials passed, with exact
  golden stdout and empty stderr.
- Test formatting and diff whitespace checks passed.
- The optimized release was installed in `/home/cesar/.local/bin/tarn`; its hash
  was checked against target/release/tarn.
- The full workspace suite was not run. No compiler/runtime Rust/C code changed.

Task synchronization, vector loop optimization and HTTP concurrency investigations
remain separate work. This is the first measured optimization of that effort.
