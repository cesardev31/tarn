# Builder integer formatting optimization — 2026-10-09

`Builder.push_u64` previously initialized a 20-byte scratch buffer and ran a
digit extraction loop even for small integers. Values below 100 now use direct
one/two-digit encoding. Larger values retain the existing general conversion,
with one additional magnitude test. There is no new intrinsic or allocation
policy; UTF-8 validation in finish and owned output semantics are unchanged.
The scratch arrays are stack storage, not heap allocations.

## Alternating release comparison

The before compiler includes the previous split optimizations and was preserved
as `/tmp/tarn-before-builder-opt`. Both compilers built identical workload sources.
Nine runs per workload/binary alternated before/after ordering; perf_counter
measured process execution, with identical output required each time. No other
agent benchmark ran concurrently; the desktop remained active.

| Workload | Before median s | After median s | Time reduction |
|---|---:|---:|---:|
| Builder only, integers 0–19 | 0.230961 | 0.206425 | 10.6% |
| Construction plus owned split | 0.482630 | 0.468475 | 2.9% |
| Builder only, 20-digit integers | 0.455817 | 0.459440 | -0.8% |

The large-number difference lies within the observed run variation; there is
no demonstrated speedup there. A preliminary arrangement with two magnitude
tests measured a 1.9% increase; the retained arrangement has one test on the
general path. Small end-to-end differences should be treated cautiously.
Raw final samples: [JSON](builder-optimization-2026-10-09.json).

The new builder.tarn/builder.go pair builds the same 20-item text 200,000 times
without splitting it; both produce 30,000,000. Both start without reservation.
The 20-digit control replaces `builder.push_u64(i)` with
`builder.push_u64(10000000000000000000 + i)` in the same Tarn source, producing
104,000,000. The full strings case produces 4,200,000.

Run the isolated comparison with `./benchmarks/vs-go/run.sh builder`.
The runner now includes builder in its default suite. A preliminary five-run
Go comparison measured 0.20 seconds Tarn / 0.13 seconds Go; this preceded the
final condition arrangement and is not a new final parity claim.

## Validation

- Release offline build passed.
- Stdlib tests: 7 passed, including new unsigned/signed formatting boundary
  checks at 9/10, 99/100, zero, u64 maximum and i64 minimum.
- Native text_ranges, string_builder and string_essentials matched golden
  stdout exactly and produced no stderr.
- Formatting, shell syntax and diff whitespace checks passed.
- The release was installed; installed and build hashes match:
  `b6543404d8bfcead156a6b676417748856ca325c85f42da8303e1acafedad157`.
- The full workspace suite was not run; no Rust/C compiler/runtime code changed.

Remaining Builder opportunities include allocation growth and finish's copy and
validation. This iteration changes numeric conversion only, rather than claiming
to eliminate those separate costs.
