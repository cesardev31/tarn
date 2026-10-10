# ADR 0064: bounded cooperative connections within HTTP workers

Status: accepted. Extends option 1 of ADR 0058 using the existing execution model.

## Problem

SO_REUSEPORT distributes connections across native workers, but each worker's
sequential accept/answer loop lets an incomplete request prevent that worker
from answering another connection. Async readiness alone does not provide
connection concurrency when the accept loop awaits every answer to completion.

## Decision

Each HTTP worker retains one Execution and uses its existing Executor to drive
owned connection Operations. The accept Operation uses the same Execution and
readiness machinery. Every operation stays on its worker. No new scheduler,
task-record model, frame migration or cross-thread waker is introduced.

`serve_parallel` retains its signature and admits at most 64 connections per
worker. `serve_parallel_with_limit(address, workers, connections_per_worker,
handler)` makes admission explicit; zero workers or zero capacity is InvalidInput.
The worker polls connections before accepting another. Completed Operations are
removed by the existing executor, releasing their sockets and frames through
verified destruction. At capacity, the worker stops accepting and continues
driving current connections; additional connections wait in kernel queues.

All handlers retain the Handler + Share contract. Calls on one worker remain
sequential cooperative calls, while different workers may invoke the same handler
concurrently. Handler computation is synchronous: blocking or long CPU work still
blocks that worker. Shared application state requires ordinary synchronization.

Executor.add propagates the connection operation's ordinary provenance. Borrowed
handler and Execution storage outlive all operations. The accept operation is
finished before another exclusive listener borrow is created. The executor must
be destroyed before its Execution on any worker exit. No lifetime restriction is
relaxed and spawn_async's capture restrictions remain unchanged.

## Limits

One request per connection and explicit close remain. Graceful shutdown, work
stealing, task migration and async handler APIs are separate future decisions.
Per-worker admission is not global load balancing; SO_REUSEPORT distribution may
leave uneven queues. The default 64 is an admission bound, not a demonstrated
optimal tuning value. Application-level CPU/memory limits still matter.

## Evidence

Regression tests use one worker and an incomplete first request. With capacity
two, a complete second request must receive a response before the first finishes.
With capacity one, the second remains queued until the first closes. Existing
parallel-handler tests retain the shared counter and ordinary bind guarantees.
HTTP/async execution and abandonment tests remain the validation boundary.
Throughput benchmarks must distinguish fast requests from slow-client workloads;
concurrency is not automatically an improvement in every case.
