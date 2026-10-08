# ADR 0058: multi-core async execution (design)

Status: proposed design only. Not implemented. AGENTS.md requires a separate
approved phase for a multi-thread scheduler.

## Problem

`runtime.Execution` runs cooperative async tasks on one thread (ADR 0038)
and `http.serve` handles one connection at a time (ADR 0048). On an 8-core
machine a Tarn server uses one core where Go's `net/http` uses all of them.
CPU-bound native tasks already scale (Phase 29: 6.5 of 8 cores).

## Options

1. **N independent executors, one per core, sharing a listening socket
   (`SO_REUSEPORT`).** Each worker owns its connections and its Execution; no
   task migrates between threads, so the existing single-thread ownership
   model is untouched. Handlers that need shared state use `Mutex<T>` (which
   already requires `T: Transfer`). Simplest and closest to current rules.
2. **Work-stealing executor.** Better load balance, but every spawned async
   task must be `Transfer`, wakers become cross-thread, and frames move
   between threads: a broad change to ADRs 0036–0038.
3. **Thread-per-connection with blocking I/O.** Simple, but memory and
   context switches grow with connections; contrary to the resource goal.

## Recommendation

Start with option 1: `http.serve_parallel(address, workers, handler)` where
the handler is a `Transfer + Share` factory or each worker builds its own
handler state. It needs `SO_REUSEPORT` in `net`, one native thread per worker
(the Phase 29 thread cache), and no change to async ownership. Measure
against Go `net/http` with a load generator before considering option 2.

## Open questions

- How a handler shares application state across workers ergonomically
  (closure factory vs. `Mutex` passed in).
- Graceful shutdown across workers.
