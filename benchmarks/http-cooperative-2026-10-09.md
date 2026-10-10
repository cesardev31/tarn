# Bounded cooperative HTTP workers — 2026-10-09

Design: [ADR 0064](../docs/adr/0064-bounded-cooperative-http-workers.md).
HTTP workers now use the existing runtime.Executor to advance several owned
connection Operations while the same Execution drives accept readiness. The
kernel still distributes connections through SO_REUSEPORT; operations stay on
their worker. No new scheduler, task migration, runtime intrinsic or compiler
semantics was added. The implementation change is ordinary stdlib Tarn.

`serve_parallel` retains its signature and defaults to 64 active connections per
worker. `serve_parallel_with_limit` exposes the limit; zero capacity/workers is
InvalidInput. Waiting connections remain in kernel queues at capacity. The
existing synchronous Handler interface, one request per connection and close
policy remain. Blocking handler work can still block its worker.

## Slow-client evidence

Both compilers built the same server source, configured with one worker. A first
connection sent an incomplete request head; a second sent a complete GET.

| Version | Second client's result |
|---|---|
| Previous sequential worker | No response within the 1-second client deadline |
| Cooperative worker | HTTP 200 in 0.337 ms |

This is one targeted observation, not a latency distribution or speedup ratio.
Raw data: [JSON](http-cooperative-slow-2026-10-09.json). The previous compiler was
preserved at `/tmp/tarn-before-cooperative-http`; the probe script remains at
`/tmp/tarn-cooperative-slow.py`.

Permanent regression tests cover one worker with capacities one and two: capacity
one keeps the second client queued until the first closes; capacity two answers
the second while the first remains incomplete. Zero limits are rejected before
even attempting address parsing/binding.

## Fast-request benchmark

Same `benchmarks/vs-go/http/run.sh`, 100,000 requests, 64 clients, eight workers,
new connection per request. Before and after runs were sequential; each runner
also measured Go. No other agent benchmark ran concurrently. Desktop applications
were active, with no CPU isolation. These are single load runs.

| Server/run | Requests/s | p50 ms | p99 ms | Failed | User + system CPU s | Peak RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| Tarn before | 10757 | 5.257 | 19.539 | 0 | 10.92 + 9.11 | 2492 |
| Go before control | 11751 | 4.630 | 19.160 | 0 | 6.14 + 8.29 | 16168 |
| Tarn after | 10799 | 5.111 | 20.211 | 0 | 11.67 + 9.39 | 4256 |
| Go after control | 10483 | 5.249 | 20.238 | 0 | 6.20 + 8.39 | 15868 |

Tarn's fast throughput is effectively unchanged (0.4% difference); Go's own
variation prevents claiming a win from the after run. CPU and memory overhead
increase with active connection operations. Admission bounds this increase;
the default 64 is not claimed to be optimal. The demonstrated benefit is
progress past slow clients, not greater peak fast-request throughput.

## Validation and installed state

- Release offline build passed.
- HTTP: 18 passed, including both new regression tests and existing shared-state,
  framing, slow-client deadlines and pending-frame destruction checks.
- Async runtime: 8 passed; async tasks: 6 passed; async buffered I/O: 3 passed.
  Total relevant backend integration tests: 35 passed, zero failed.
- Stdlib tests: 8 passed, zero failed.
- Diff whitespace checks passed. Existing Rust-test formatting was preserved;
  new test sections were formatted with rustfmt.
- Final release installed; build/installed SHA-256 match:
  `d7b26622232d8046c581390a623fa2282624be1c7220d53ed9b7f6479b2ae740`.
- Full workspace suite was not run. No graceful shutdown or cross-worker task
  migration was added. Performance conclusions remain limited to these loads.

Next measurements should vary connection limits and combine slow and fast
clients at realistic proportions, then profile executor scans and connection
allocations before optimizing scheduling further.
