# ADR 0033: safe native tasks and structured completion

Status: proposed for phase 11; implementation in progress. This document is not
an assertion that native tasks or synchronization are available today.

## Ownership boundary

Concurrency follows ordinary Copy, Move, shared and mutable loans. No second
thread-safety checker, backend ownership query or explicit lifetime syntax is
introduced. Phase-10 callable environments and verified destruction functions
remain authoritative. General reference-bearing user structs/enums retain E4203.

The intended expression is `spawn move fn() R { ... }`, producing unique owned
`Task<R>` storage. The task body has zero explicit arguments. Spawn transfers its
callable; mode controls invocation, independently of environment ownership. A
reusable owned callable may be invoked once by the task and then destroyed; a
consuming callable transfers its capture fields into its verified body. A borrowed
stack environment must not cross an unscoped boundary. Moving a reference does
not extend its provenance. Unscoped result types may not contain loans whose
storage could die before task completion/result transfer.

Existing `spawn call(...)` and `scope { ... }` are provisional syntax/IR, not a
native concurrency implementation. Compatibility or migration must be explicit;
ordinary calls must never accidentally execute synchronously as spawn recovery.

## Handle policy and results

A handle is non-Copy. Explicit join consumes it and transfers the result through
ordinary ownership into caller storage. Dropping an unjoined handle waits, then
destroys an unused result through a generated verified destruction function. A
moved handle has no second join or destruction. The worker and handle never both
own the same result simultaneously; successful join is the transfer boundary.

Implicit join is chosen over detach (harder lifetimes), cancellation (new cleanup
semantics), or mandatory explicit join (application ceremony and path complexity).
It can block indefinitely if the application deadlocks or its worker never ends.
This is visible resource destruction behavior, not cancellation. Self-join and
OS thread failures must abort rather than hang on an impossible runtime invariant.
Whether the public join returns a plain result or a modeled Result is still open;
no fake Error contract or recoverable panic transport may be introduced merely
for `try` syntax. Allocation/thread failures can remain abort-only in v0.

## Scoped tasks

Keep lexical `scope { ... }` as the minimal structured spelling initially; within
it, scoped spawn must be explicitly distinguishable from unscoped spawn in typed
IR. A scope owns completion records even if the application discards a task value.
The implementation must join workers before their captured storage, stack
environments or scope locals are destroyed. Every normal exit includes return,
try propagation, break and continue, in nested inner-to-outer order.

Borrowed callable captures and environment-storage witness loans must remain live
until completion. Loan flow must be carried by real IR values/operations through
the existing NLL checker, not guessed by backend scheduling. A join may shorten a
loan only when it proves worker completion. Scoped handles/environments/results
must not escape their scope or introduce child-storage loans into its parent.

Disjoint struct fields use existing place-overlap rules. Dynamic array/slice
indices remain conservative. A future split API requires a separately verified
non-overlap contract; do not infer partition arithmetic in this phase. Concurrent
shared access is allowed only for share-capable data. Concurrent access including
mutation requires exclusive loans or declared synchronization.

## Cross-thread capabilities

Proposed Tarn capabilities are `Transfer` (owned move between tasks) and `Share`
(concurrent shared access). Neither implies Copy, and neither is size-based.
Primitive values and strings qualify. Arrays and normal ADTs qualify structurally
only when every substituted field/payload qualifies. Recursive queries need a
bounded/cycle-safe implementation; generic code requires explicit bounds.

Shared references transfer only when their referent is Share and their loan lasts
through completion; mutable references require Transfer referents and an exclusive
loan through completion. Callable capability checks must use capture metadata;
a function signature alone cannot establish what its erased environment contains.
Unknown/opaque native resources and dynamic interfaces need explicit trusted
capability contracts, never optimistic defaults. Compiler-generated reference
fields do not authorize user reference-bearing aggregates.

## Synchronization bootstrap

Use only a small runtime-backed synchronization surface. A shared mutex owner
must keep allocation and payload alive through all locks. `lock` creates a unique
nontransferable guard; its ordinary result provenance borrows the mutex. Guard
payload borrowing is exclusive and cannot outlive the guard. Verified guard
destruction unlocks exactly once; moving the guard does not unlock twice.
A lock must not yield an independent unrestricted reference after guard disposal.
This is compiler/runtime-backed resource semantics, not user custom Drop.
Reentrant locks are not promised. Share for Mutex<T> requires Transfer for T;
allocation/lock failure is abort-only unless an explicit error contract is added.

Start with a small concrete atomic set rather than a generic operation on arbitrary
T. All v0 operations use documented sequential consistency. Relaxed/acquire/release
selection and a richer memory model remain deferred. Atomic storage cannot be
accessed through ordinary unsynchronized mutable projections. Sharing an owner
needs deliberate storage ownership (for example a narrowly modeled shared owner),
not accidental Copy of a unique allocation pointer. Exact public constructors and
shared-owner API remain implementation design questions.

## Runtime and post-drop

Use Linux x86_64 pthreads, one native thread per task; no pool or scheduler.
Private runtime helpers provide start, join, storage allocation, mutex operations
and atomics. Thread entry invokes a compiler-generated wrapper. The wrapper calls
the existing callable ABI, publishes result storage and destroys transferred
resources using ordinary IR/post-drop functions. Runtime only manages thread and
allocation mechanics; it does not independently guess capture/result destruction.
Linking will require `-pthread`; no external library dependency is necessary.

Panic in any thread aborts the whole process. There is no unwinding, exception
transport, cancellation or recovery. Synchronization must establish publication
of initialized result bytes before a successful join observes them.

## Required validation and current evidence

Before implementation, the full phase-10 `cargo test` baseline passed, including
15 native backend tests. Inspection found provisional scope lowering emitted
JoinScope after local destruction and omitted it on early exits. Initial lowering
work moves scope completion before local drops on normal, return, break, continue
and nested exits; source/IR fixtures and a CFG traversal regression validate order.
This does not establish loan retention or executable scoped concurrency.

Completion requires real native tests for scalar/owned results, implicit join,
multiple/nested workers, generic/dynamic captures, exactly-once capture/result
traces, abort in workers, scoped shared/exclusive borrows and disjoint fields.
Rejections must cover caller use-after-move, escaped captures/results, concurrent
mutable aliases and guard misuse. Stress uses counters/barriers/joins, never sleep
as proof. Extend memory_safety and source mutation corpora and retain all existing
callable, dynamic, drop and native regression gates.

## Deferred and unresolved

No async/await, futures, reactor, green threads, work stealing, cancellation,
detach, user destructors, unwinding, optimizer, LLVM, package manager or platform
expansion. Public join/error API, scoped spawn spelling, capability declarations
for trusted native/dynamic types, mutex shared ownership and exact atomic surface
must be resolved and tested before this ADR can become accepted. Runtime execution,
thread capability enforcement, scoped loan retention and synchronization remain
unimplemented at this checkpoint.
