# Phase 14C report: buffered async I/O, and Phase 14 summary

Design: [ADR 0040](adr/0040-buffered-io.md). Earlier subphases:
[14A](phase-14a-report.md) (ADR 0038) and [14B](phase-14b-report.md)
(ADR 0039).

## What was implemented

- `net.BufferedReader` provides `read`, `read_exact`, `read_until(delimiter,
  max)`, `buffered` and `capacity`.
- `net.BufferedWriter` provides `write`, `write_all`, `flush`, `pending` and
  `capacity`.
- All of it is ordinary Tarn async code over `read_async`/`write_async`.
- Compiler and runtime support: the `Vec<T>.as_slice` / `as_mut_slice`
  intrinsics (backend: build a slice of the vector's elements), and two new
  `ErrorKind` variants that are listed in the network ABI check.

## Tests

`compiler/backend/tests/async_buffered.rs`:

- **Reader.** Bytes arrive one per write, so delimiters are split across reads.
  It checks:
  - small lines and lines longer than the buffer (capacity grows 4 → 16);
  - `read_exact` and small `read`s;
  - a limit exceeded with the bytes kept buffered;
  - partial data at end of stream, then an empty vector, then `read_exact`
    returning `UnexpectedEof`.
- **Writer.**
  - Small writes stay buffered (strace shows a single 8-byte `sendto` for 3 + 5
    bytes).
  - A full buffer accepts only what fits.
  - The next write flushes first, and an explicit flush empties the buffer.
  - 8 MB go through a 64 KiB buffer. strace shows 12 `EAGAIN`s, with partial
    writes continuing from the exact offset.
  - A dropped, unflushed writer's bytes never arrive.
- **Concurrent echo.** 40 connections are open at once and receive 25
  interleaved lines each. There is one async task per connection with a
  buffered reader and writer, all on one thread. Every echoed byte is verified,
  and file descriptors are balanced.
- **Borrow fixture.** `E4101_vec_slice_then_push`.

The full workspace suite passes.

## Bugs found

None in the compiler for this subphase. The buffered layer exercised async
methods calling async methods on `&mut self`, with loans of owned buffers held
across `await`, and type-checked and ran first time.

## Decisions I would defend

- **The stream is passed per call** instead of being owned or referenced. This
  follows from the no-references-in-structs rule, and it gives a reader and a
  writer on one stream for free.
- **No flush on drop.** Drop cannot suspend, and hidden I/O in destruction
  contradicts the explicit model.
- **`read_until` always takes a limit**, and a line over the limit stays
  buffered, so the caller decides what to do with it.

## Decisions I still question

- Passing the stream per call is the most "un-Go-like" part. If real programs
  pass the wrong stream, or find it noisy, an owning `BufferedStream` (owning
  the stream plus both buffers) is the next candidate.
- Copies are byte loops. A `copy_from` slice intrinsic would be the first
  optimization.

## Known limitations

- `TcpStream` only. There is no async read/write interface yet.
- No peek, consume, string lines or vectored I/O.
- Timers and buffers still live in `net`. Splitting into `async`/`io`/`time`
  modules is recorded organizational debt.

## Phase 14 summary

| Area | State |
|---|---|
| Async spawn | `execution.spawn_async(computation)`. `spawn` remains native threads. |
| `AsyncTask<R>` | Owned, non-Copy handle. Join moves `R` exactly once, and `Result` passes through unchanged. |
| Drop | An unjoined completed result is destroyed once. A pending task is abandoned through its verified frame destruction. No detaching. |
| Executor storage | `Vec` plus a spawn mailbox; the 16-task limit is gone (500 tasks tested). |
| Fairness | One poll per runnable task per turn. |
| Loans | A spawned task may borrow only its Execution (E4209). |
| Timers and timeouts | Monotonic `timerfd` on the shared Poll. `join_timeout`: the result wins ties, and the loser is abandoned. |
| Buffered I/O | Tarn policy over the async primitives (this report). |
| Concurrent server | Accept loop with one async task per connection, buffered echo, single thread. |

### What remains before async HTTP/1.1

- Scoped async tasks (spawned tasks borrowing local data) if handlers need
  them.
- An async read/write interface, so buffered I/O and future TLS compose.
- Header parsing on top of `read_until` with limits, and a connection keep-alive
  policy with timeouts per read.
- The module split (`async`, `io`, `time`, `net`).
- A benchmark of the echo server against Go before any optimization work.
