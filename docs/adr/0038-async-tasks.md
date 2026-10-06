# ADR 0038: cooperative async tasks (Phase 14A)

Status: accepted for Phase 14A. Timers/timeouts (14B) and buffered I/O (14C)
are separate decisions. Async concurrency reuses the Phase-12C executor,
Waker and ownership model; there is no second scheduler or task model.

## Prerequisite: `Vec<T>`

The bootstrap executor stored at most 16 operations in a fixed array. Phase 14A
adds `core.Vec<T>`: owned heap storage with `new`, `len`, `is_empty`, `push`,
`pop`, `get`, `get_mut`, `at` (Copy elements), `replace` and `swap_remove`.

- Destruction destroys every remaining element once, in index order, then
  frees storage. Capacity grows 4, 8, 16, ...; overflow and exhaustion abort.
  Out-of-range indices abort.
- A zero-size `[0]T` marker field makes copy semantics, Transfer/Share and
  reference/resource containment follow `T` with no special cases.
- Moving an element out (`pop`, `replace`, `swap_remove`) yields the loans the
  vector stores, not a borrow of the vector itself.
- The runtime only reallocates bytes; element ownership is compiled code.

### Store effects through `&mut` (soundness fix)

A callee may store an argument's loans behind a mutable reference it receives.
The borrow checker now unions every argument's loans into each local directly
mutably borrowed by an argument whose borrowed place can hold references.
Before this fix, `set(&mut holder, &y)` let `holder` keep a reference to `y`
after `y` died (regression test `E4105_store_through_mutable_argument`). This is
what makes `Vec<&T>.push` sound.

## Spawn

`execution.spawn_async(computation) AsyncTask<R>` is an ordinary method. `spawn`
remains the keyword for native OS threads; the different names make the two
models unambiguous without new syntax or parser changes.

The computation is wrapped by a private `_runner` async function in an ordinary
`Operation<void>` (own Waker identity) and moved into the Execution's spawn
mailbox. The executor drains the mailbox at the start of every turn. A mailbox
is needed because a running task cannot reach the executor that is polling it.

v0 restriction (E4209): a spawned computation may borrow only the Execution it
runs on. The mailbox and executor never outlive that Execution, so no other loan
can be hidden in runtime storage. Scoped async tasks with local borrows are
deferred until a program needs them.

## AsyncTask<R>

An owned, non-Copy handle; moving it moves the only authority over the result.
It holds a runtime record shared with the runner (two references). The record
stores only bytes, a completion bit, an abandonment bit, the owning Execution and
the identity of a joining Waker. Typed result bytes are written, read and
destroyed by compiled code that knows `R` after monomorphization.

`await task.join()` consumes the handle, so a second join or use after move is
an ordinary E4001. It takes the result if complete; otherwise it registers the
current Waker (which must belong to the same Execution, else abort) and parks.
Completion wakes that Waker by identity. A Result-returning computation yields
its Result unchanged; panics still abort the process.

## Destruction without detaching

- Handle dropped after completion: the unjoined result is destroyed once by the
  handle (typed).
- Handle dropped while pending: the task is marked abandoned; at the next turn
  the executor destroys its Operation, which runs the frame's verified
  abandonment path (no second async destructor).
- The runner's reference is a private `_TaskRef` stored in its frame; its
  compiler-known destruction releases the record when the frame is destroyed,
  whether it completed or was abandoned.
- `block_on` runs spawned tasks on an internal executor; when its operation
  completes, remaining tasks are abandoned: no task outlives `block_on`.
- Dropping an Execution with undrained spawned operations aborts.

## Fairness

One poll per runnable task per turn, readiness serviced every turn, as in 12C.
Completed tasks are removed with `swap_remove`; the moved task is visited in the
same turn. Native tests interleave tasks and run 500 concurrent tasks.

## Borrowed closures in async frames (frame fix)

A borrowed closure's environment used to be a native stack slot. Held across an
`await`, it dangled after resumption. Its storage witness is now treated as
address-taken by the frame pass, and the backend places the environment inside
the stable frame.

## Limits

- Spawned tasks may borrow only their Execution.
- No select/race, cancellation API, executor spawning from another thread, or
  timers (14B). `block_on` remains the single top-level entry.
