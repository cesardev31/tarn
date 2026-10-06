# Native tasks completion report (phase 11A)

Phase 11A implements the handle-owned native task boundary. Phase 11B
Transfer/Share and scoped loans, then phase 11C Mutex/atomics, require separate
approval. ADR 0033 remains proposed until the full phase is complete.

## What was implemented

`spawn move fn() R { ... }` creates a real Linux x86_64 pthread. Its operand is
initially a direct zero-parameter owned closure literal. `Task<R>` is owned,
non-Copy and movable. `task.join()` consumes the handle and returns R directly;
ordinary move checking rejects double join and use after handle/capture moves.
Dropping an initialized unjoined handle waits and destroys its unused result.
Discarding a spawn expression joins its statement temporary immediately.

Unscoped borrowed closures and captured loans remain conservatively rejected with
E4206. Borrowed results into worker/environment storage remain rejected by the
existing provenance/escape checker. E4203 for user reference-bearing aggregates
is unchanged. No Transfer/Share enforcement or scoped borrowing is claimed.

## Task representation and runtime ABI

Core declares a private usize handle in Task<R>, with the ordinary eight-byte
canonical struct layout. Runtime storage owns the pthread ID, code/environment
lanes, worker and result-destruction adapter pointers, result allocation and
execution-only initialized/joined bits. There is no task-specific move-checker
state. Result allocations include one byte for zero-sized values.

Private helpers are `tarn_rt_task_spawn`, `tarn_rt_task_wait`,
`tarn_rt_task_release` and `tarn_rt_task_drop`; linking uses `-pthread`.
Successful pthread_join publishes worker writes. Explicit join moves initialized
result bytes into caller storage before freeing allocations, without destroying
the transferred result. Implicit join calls verified result destruction, then
frees storage. There is no simultaneous logical ownership of the result.

Worker panic aborts the process; it is not transported in a Result. Allocation,
create/join failure and impossible self-join are runtime faults that abort.
No detach, cancellation, unwinding, scheduler, async or synchronization library
was added. The runtime's stderr trace output locks each line to avoid interleaving
resource observations from concurrent workers; this is not a public mutex API.

## Verified worker and destruction integration

Lowering generates ordinary IR functions for callable invocation and unused
result destruction. Both pass through move checking, borrow checking and
post-drop elaboration. Reusable Shared/Mutable callables are borrowed, invoked
once and then destroyed; Once callables are moved into invocation. Existing
closure thunks handle ownership captures and environment release.

`Callee::TaskSpawn` carries explicit worker/destruction IDs and generic arguments.
The post-drop verifier checks their types, arity, owned argument and destination.
Reachable specialization reserves their instances through its existing FIFO work
list. Backend-generated C ABI adapters only bridge ordinary canonical ABI lanes;
C neither inspects Tarn types nor guesses capture/result destruction. Backend
code does not query moves, loans or provenance.

## Join and early exits

Initialized Task destruction uses existing Value/Guard plans and normal move/init
state. Conditional initialization, reinitialization, overwrite and self-assignment
therefore use existing destruction/flag machinery. Normal exit, return, break,
continue and nested exits join unused handles exactly once.

The preceding join-before-scope-local-drop fix remains a separate IR regression
including early exits. In 11A the provisional lexical JoinScope marker has no
child records; native tasks inside `scope { ... }` remain unscoped handle-owned
tasks. Scoped loan retention and completion records are 11B work. Legacy
`spawn call(...)` remains rejected by native codegen, never run synchronously.

## Tests

`tests/native/pass/tasks.tarn` exercises the 42 milestone, string, generic ADT,
owned enum, array, zero-size result, moved handle, nested workers, all three
callable invocation modes and repeated pairs of workers. Assertions use joins,
not sleeps. `task_completion.tarn` covers unused results and normal/return/break/
continue/nested completion, discarded expressions, overwrite/self-assignment and
conditional initialization.

Native trace tests assert exactly-once destruction of captures and transferred
or unused resources and nested unused-result order. Worker panic must produce
SIGABRT. Four corrupted worker/destruction metadata cases must fail verification.
Runtime ABI tests verify a different pthread identity, 512 repeated worker starts,
owned string transfer/drop and deterministic linker-injected allocation/create/
join failures plus self-join. Canonical memory_safety cases cover double join,
handle/capture use-after-move, unscoped borrowed captures and invalid result
provenance. Both source mutation corpora include task fixtures.

Validation: `cargo test`, `cargo check --workspace` and working/staged
`git diff --check` pass without compiler warnings. The native backend suite has
17 tests; the separate pthread runtime suite has two. The host-dependent
benchmark remains intentionally ignored.

## Bugs found

The generic bodyless join declaration was initially inferred to require a borrowed
result source. Its core Task lang-item contract now explicitly transfers an owned
stored result; this is a narrow trusted intrinsic boundary, not a relaxation of
ordinary declaration provenance. The new spawn metadata also required explicit
specialization reachability and verifier checks for both generated functions.
Existing source/IR early-exit ordering regression remains intact.

## Decisions I would defend

One pthread per task; unique movable handles with join on destruction; direct R
results and abort-only faults; existing callable representations and ordinary
verified worker/result destruction functions; explicit task metadata before
backend execution; conservative borrowed-capture rejection until scoped loans and
capabilities are implemented. These choices give executable ownership semantics
without a scheduler or a second borrow checker.

## Decisions I still question and known limitations

Spawn currently requires a literal rather than an arbitrary callable value.
Unbounded generic/reference-bearing capture contracts await 11B. Join-on-drop can
block indefinitely if application work never finishes; there is no cancellation.
Each task allocates record/result storage and starts an OS thread; there is no
pool or allocation optimization. Runtime function-pointer lanes and layout limits
are private to the existing Linux x86_64 ABI.

11A does not authorize progressing to 11B without approval. Scoped loans,
Transfer/Share, trusted native/dynamic capability contracts, Mutex/atomics and
HTTP/async I/O remain pending. This does not establish complete end-to-end
language memory safety.
