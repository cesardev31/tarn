# Phase 14A report: Vec and cooperative async tasks

Design: [ADR 0038](adr/0038-async-tasks.md). Timers/timeouts (14B) and buffered
I/O (14C) are not part of this subphase.

## Async spawn syntax/API
`execution.spawn_async(computation) AsyncTask<R>`: an ordinary method, distinct
by name from the native `spawn` keyword. No parser or syntax changes.

## AsyncTask<R>
Owned, non-Copy handle over a runtime record shared with the task's runner.
`await task.join()` consumes the handle and yields `R` exactly once. Results
that are `Result<T, E>` pass through unchanged; panics abort.

## Ownership and drop of the handle
Dropped after completion: the unjoined result is destroyed once. Dropped while
pending: the task is abandoned through its frame's verified abandonment path.
Tasks never detach; tasks left when `block_on` finishes are abandoned.

## Executor task storage and fairness
`Vec<_Spawned>` replaces the 16-slot array. A spawn mailbox in the Execution is
drained every turn. One poll per runnable task per turn; 500 concurrent tasks
and interleaving are tested.

## Loans in spawned tasks
v0: a spawned computation may borrow only its Execution (E4209).

## Concurrent socket server
A single-threaded async server accepts connections and spawns one task per
connection; a client that stays connected does not block others (tested with
fd-balanced traces).

## Tests
`backend/tests/async_tasks.rs` (6 native tests), Vec drop/loan scenarios, the
E4105 store-through-`&mut` regression, updated provenance golden.

## Bugs found
- Borrow checker: storing a loan through a `&mut` argument was not propagated
  to the borrowed holder (use after scope end accepted). Fixed for all calls.
- Phase 13 frame lowering: borrowed closure environments lived on the per-poll
  native stack and dangled after `await`. Now stored in the frame.
- A private field named like a method (`len`) hides the method. Worked around
  by renaming; language-level fix still open.

## Decisions I would defend
Runtime records hold bytes only; typed work is compiled. Abandonment reuses the
verified frame path. A name, not new syntax, distinguishes cooperative spawn.

## Decisions I still question
The spawn mailbox lives in C (storage only). Spawned tasks cannot borrow local
data yet. `spawn_async` requires passing `&Execution` explicitly.

## Next
14B timers/timeouts (timerfd on the same Poll), 14C buffered reader/writer.
