# ADR 0059: integrated performance tooling

Status: accepted. Phase 30.

## Decision

- **`tarn test --bench [--runs n] [--filter text] [--json]`** discovers
  `bench_*` functions with the same rules as tests (ADR 0049: safe,
  synchronous, non-generic, no parameters, void or `Result<void, E>`), builds
  one harness, and runs each benchmark in its own process `n` times (default
  5). It reports the median wall time measured by the runner, the median CPU
  time (user + system, all threads) and the peak resident memory, both read
  by the runtime with `getrusage` at normal exit when the runner sets
  `TARN_BENCH_RUSAGE`. A failing or aborting benchmark fails the run. Plain
  `tarn test` ignores benchmarks.
- **`tarn profile <file> [--link lib]... [-- args]`** builds like `run`,
  executes under valgrind's callgrind and lists the 15 functions with the
  most instructions, using Tarn names (the backend's readable symbols,
  Phase 28). It requires valgrind and says so when it is missing; `perf` is
  often unavailable without privileges.

## Why

Optimization must start from measurement (Phases 28–29 found their largest
wins by profiling). Integrated commands make that the default workflow for
people and agents, with JSON output for tooling.

## Limits

- Benchmarks measure whole functions per process, not calibrated
  iterations of tiny operations (Go's `b.N`); a benchmark loops itself.
- Profile counts instructions, not wall time; it is proportional and
  deterministic, which suits comparisons between versions.
- Inlined functions are attributed to their caller.
