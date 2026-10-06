# Source async completion report (Phase 13)

Design: [ADR 0037](adr/0037-source-async-lowering.md). Phase 13 lowers `async fn`
and `await` onto the Phase-12C Pending/Ready/Waker model (ADR 0036). The
executor, Waker runtime, readiness model and polling semantics are unchanged.

## Source syntax

`async fn name(params) T { ... }` on free functions and inherent methods, and the
prefix operator `await expr` at `try` precedence (`try await f()` is
`try (await f())`). Async blocks, async closures, select/join/race and yield are
not part of the language. The AST keeps the modifier and an explicit `Await` node;
resolution uses the ordinary item namespace. No formatter exists yet.

## Async call semantics

Lazy. A call moves its arguments into a new stable heap frame (state 0) and
returns an owned, non-Copy `async computation<T>`; no body statement runs until
the first poll. Ownership sees an ordinary aggregate: the computation holds
exactly the loans of its arguments.

## Await semantics

`await e` (only in an async body: E3060; only on a source computation or the
trusted `net.Operation<R>`: E3061) polls `e` with the current poll's waker until
Ready. Pending suspends the parent; Ready moves the value out once and destroys
the completed child at the end of the statement.

## Generated frame representation

Ownership phases analyze the async body as an ordinary IR function with a hidden
`&Waker` parameter and explicit `Suspend { resume, abandon }`/`Abandon` edges.
After verified drop elaboration, `ir::async_frame` turns it into a poll function
`(frame, waker, abandon) -> Progress<T>` with an explicit state dispatch. The
frame is `[destruction header][stored locals][all drop flags]`, allocated once
and never moved; the computation value is a `(poll code, frame)` pair.

## Suspension-state lowering

Each `await` is a poll loop whose Pending edge is a `Suspend`; every body starts
with a `Suspend` (the unstarted state). States are numbered in block order, so
IDs are deterministic and loop suspensions are reused across iterations. Abandon
edges carry the same drops as an early return from that point.

## Stored-local analysis

Stored: construction parameters, the state word, locals live on entry to any
resume or abandonment edge (destruction counts as a use) and every address-taken
local. Not stored: the per-poll waker, the abandon flag, the poll result and
values dead at every suspension. Drop flags are all frame-resident, so
initialization persists and entry flag setup runs once.

## Borrowing across await

Loans live across a suspension stay live (ordinary NLL over the suspension
edge): a `&mut` held across `await` conflicts with other access (E4104); a
pending computation keeps its borrowed inputs unavailable until destroyed
(E4104); a computation borrowing a local cannot escape (E4201); returning a
reference to frame storage is rejected (E4201); a reference to a frame local is
valid across `await` because the frame never moves. Polling a source computation
yields the loans it captured, not a loan of the computation or the waker.

## Child completion and wake forwarding

Source children are polled directly with the parent's waker; manual Operations
use `poll_with`. No new wake mechanism exists. Completed stdlib leaves release
their single readiness registration (preserving a queued wake), as completed
Operations do. Coalesced, spurious, immediate-Ready and multi-turn Pending wakes
are covered by native tests; fairness stays the executor's bounded turn order.

## Frame/drop semantics

Destroying a computation runs the body's own verified abandonment path for the
current state and frees the frame. Native traces cover: before first poll,
pending at the first and second await (no resurrection of a moved field), inside
loops (continue/break), early return, `try` propagation and completion (result
transferred exactly once). Repolling a completed computation aborts.

## Async networking API

`TcpStream.read_async`, `write_async`, `write_all_async`, `TcpListener.accept_async`,
`Connecting.finish_async`, `UdpSocket.recv_from_async`: ordinary `async fn` in the
trusted `net` module over the Phase-12C poll functions, using two private
primitives (`_with_waker`, `_async_park`). Blocking methods are unchanged. Async
I/O on a blocking socket returns an error; calling blocking I/O or DNS from async
code blocks the executor (no hidden thread pool).

## Executor entry model

Explicit, no global executor and no `async main`. An async computation coerces
one-way to the trusted manual poller type, so it can be wrapped in
`net.Operation.new(&execution, app())`, added to `net.Executor`, or driven by
`execution.block_on(&mut operation)`.

## Recursive async policy

Accepted. A child computation is a `(code, frame)` pair to a separate heap frame,
so no frame contains itself (existing owned-environment indirection). Generic
expanding recursion is bounded by the monomorphization limit.

## Diagnostics/LSP

Diagnostics are reported on source code; generated locals/states never appear.
The LSP smoke test checks hover of `async computation<T>` and awaited values,
definition, E3060, and a source-positioned E4001 across `await`. E3062
(checkpoint gate) is retired.

## Tests and mutation evidence

- Driver: frontend/lowering facts (lazy construction, initial suspension, frame
  states, stored child Operation), plus the existing E3060/E3061/E3051 checks;
  a frame-verifier mutation test rejects a missing state transition, a wrong
  state write, a lost parameter slot, a stored waker, a duplicate slot, a
  surviving suspension edge and a wrong construction arity.
- Borrow/move fixtures with golden diagnostics: retained buffer, mutable borrow
  across await, frame reference escape, computation escape, move before await,
  stream moved while a pending computation borrows it (E4103);
  a pass fixture with provenance of borrowed async results.
- Native pass suite: `async_block_on`, `async_control_flow` (loops, match,
  generics, closures, `&mut` across awaits, borrowed returns, `try await`),
  `async_recursion`, `async_dynamic` (`&any I` across await), and
  `examples/28_async.tarn`.
- `backend/tests/async_runtime.rs`: state destruction traces, early exits, TCP
  accept/read/write_all/EOF with a pthread peer, connect + UDP receive, 1 MiB
  write_all with real partial progress, nested wake variants, executor fairness
  with two source computations, repoll abort. Network traces are fd-balanced.
- Mutation corpora include the async fixtures (driver line deletions/truncation
  and native line deletions that must not panic or emit invalid code).

## Bugs found

- A Waker holds one readiness registration; source leaves share the top-level
  Waker sequentially, so the second leaf on another fd failed with EINVAL. Fixed
  in Tarn by releasing the registration on leaf completion (`_disarm`).
- Block renumbering after pruning ignored the new suspension edges (crash in the
  move checker); fixed with `Terminator::targets_mut`.
- `block_on` returning an owned generic result tied to a local Operation is
  correctly rejected by the borrow checker; the API takes a caller-owned
  Operation instead.
- Earlier in Phase 13: `MutexGuard.replace` accepted a stored reference to a
  dead local (now E3051).

## Decisions I would defend

- Ownership runs on the source CFG with explicit suspension edges; the physical
  frame pass only relocates. No async ownership model, no backend decisions.
- Abandonment edges reuse early-return drop lowering, so every state's
  destruction is an ordinary verified post-drop plan.
- Child frames are separate heap allocations: stable addresses without pinning,
  and recursion without infinite types.
- Explicit `*_async` names and explicit `block_on`, no hidden executor.

## Decisions I still question

- `block_on(&mut Operation<R>)` is two lines where one would do; an owned-result
  form needs a way to state that a generic result carries no frame loans.
- The stored set over-approximates (every address-taken local, every flag).
- One-way coercion of computations to manual pollers exposes the representation
  to the manual layer; it is trusted-identity based but still a coupling.
- Allowing recursion goes against the request's stated preference; evidence of
  deep await chains overflowing the native stack would reverse it.

## Known limitations

One computation tree per Operation (no select/join, no executor spawning);
no async methods in interfaces/impls; no async closures/blocks; no cancellation
API; blocking calls block the executor; `_with_waker` is a trusted-module
primitive whose callback must not leak the Waker reference.

## What remains before an async HTTP server

Accepting connections concurrently needs executor-level spawning of source
computations from async code (or a join/select combinator) with ownership of
per-connection state; timeouts need a timer API; buffered readers/writers over
`read_async`/`write_all_async`; HTTP parsing in Tarn; graceful shutdown and
cancellation semantics; and evidence on frame size and allocation cost.
