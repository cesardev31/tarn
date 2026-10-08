# Phase 30 report: performance tooling and multi-core HTTP

Designs: [ADR 0059](adr/0059-performance-tooling.md),
[ADR 0058](adr/0058-multicore-executor-design.md) (option 1).

## What was implemented

- `tarn test --bench [--runs n]`: `bench_*` functions measured in isolated
  processes (median wall, median CPU, peak RSS via runtime `getrusage`).
- `tarn profile`: callgrind with Tarn function names (15 costliest).
- `net.TcpListener.bind_shared` (SO_REUSEPORT, via `setsockopt` in Tarn),
  `http.Handler` and `http.serve_parallel(address, workers, &handler)`.

## Results

Parallel HTTP, 8 Tarn workers vs Go `net/http`, 64 clients, 100,000 GET
requests, one connection per request, load generator on the same machine
(`benchmarks/vs-go/http/run.sh`):

| Server | Requests/s | p50 | p99 | Peak memory |
|---|---|---|---|---|
| Tarn `serve_parallel` | 12,364 | 4.8 ms | 13.9 ms | ~2.3 MB |
| Go `net/http` | 10,293 | 5.4 ms | 20.7 ms | 7.1 MB |

Connection setup is kernel-heavy in this setup, so this shows parity rather
than a general advantage; keep-alive comparisons need `serve_parallel` to
support persistent connections first.

## Design notes and findings

1. Closures cannot cross threads (erased callables carry no Transfer/Share
   evidence, E3047): the parallel handler is a nominal `http.Handler`
   implemented by a struct, checked `Share` structurally and called by static
   dispatch.
2. Dropping a task handle joins it (ADR 0033), and scoped handles cannot be
   stored in a `Vec` (E4208): workers start recursively, each handle held in
   its own scope frame until the later workers have started.
3. Assigning through a guard's `&mut` (`guard.value() = ...`) is not
   expressible (no deref assignment); `guard.replace(...)` is used. Evidence
   for ergonomic Mutex updates.
4. Background shells ignore SIGINT, so the benchmark script stops servers
   with SIGTERM.
5. `bench_split` showed 14.5 MB peak RSS with zero bytes leaked (valgrind):
   glibc keeps freed memory; an allocator concern, not a leak.

## Tests

- CLI: bench runner (JSON fields, failing benchmark fails the run, `--runs 0`
  rejected, plain `tarn test` ignores benches); profile names Tarn functions
  (skipped without valgrind).
- HTTP: `serve_parallel` with 8 concurrent clients and 400 requests sees
  every counter value 1..=400 exactly once; a plain `bind` on the shared
  address fails.

## Known limitations

- `serve_parallel` closes each connection after one response, like `serve`.
- No graceful shutdown; workers run until a listener fails.
- Benchmarks are per-function processes, not calibrated iterations.
