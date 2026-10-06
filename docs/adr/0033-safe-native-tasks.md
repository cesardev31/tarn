# ADR 0033: safe native tasks and structured completion

Status: accepted. Phases 11A, 11B and 11C implement and validate the native v0
model below. Async I/O/networking (Phase 12) is not authorized by this ADR.

## Ownership boundary

Concurrency follows ordinary Copy, Move, shared and mutable loans. No second
thread-safety checker, backend ownership query or explicit lifetime syntax is
introduced. Phase-10 callable environments and verified destruction functions
remain authoritative. General reference-bearing user structs/enums retain E4203.

The 11A expression is `spawn move fn() R { ... }`, producing unique owned
`Task<R>` storage. The task body has zero explicit arguments. Spawn transfers its
callable; mode controls invocation, independently of environment ownership. A
reusable owned callable may be invoked once by the task and then destroyed; a
consuming callable transfers its capture fields into its verified body. A borrowed
stack environment must not cross an unscoped boundary. Moving a reference does
not extend its provenance. Unscoped result types may not contain loans whose
storage could die before task completion/result transfer.

The legacy `spawn call(...)` stays frontend-provisional and native-unsupported.
`scope { ... }` initially retained a provisional completion marker in 11A.
Phase 11B implements scoped completion using owned handles and scope witness
loans as specified below.
Compatibility or migration must be explicit;
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
The public 11A `join()` returns R directly. There is no fake Error contract or
recoverable panic transport for `try` syntax. Allocation/thread failures are
abort-only runtime faults.

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

Tarn capabilities are `Transfer` (owned move between tasks) and `Share`
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
not accidental Copy of a unique allocation pointer. The exact constructors and shared-use API are resolved by the 11C contract below.

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
That initial ordering fix alone did not establish loan retention or executable
scoped concurrency; the 11B implementation and report below provide that evidence.

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
expansion. Public join returns R directly and scoped spawn uses lexical scope.
Native authority is granted only by the trusted declaration catalog; a public
contract annotation language is deferred, not required by the implemented v0 API.
Shared synchronization uses ordinary borrowed owners and scoped completion; no
reference-counted owner is needed. Mutex and atomic decisions are resolved below.
Conservative callable and scoped-result restrictions are deliberate v0 limits.

## 11A private runtime allocation (implementation contract)

The initial runtime record owns a pthread ID, a worker adapter pointer, two
callable lanes (code/environment), a separately allocated canonically aligned
result buffer, a compiler-generated result destruction adapter and execution-only
joined/initialized bits. The worker adapter receives `(result, code, environment)`;
it invokes an ordinary verified Tarn wrapper and transfers its return into result
storage. It is responsible for callable destruction according to verified IR.
The C runtime never interprets Tarn types or destroys callable captures itself.

The result buffer allocates at least one byte for zero-size results. Source values
remain subject to the native layout limit. Worker return sets initialized; join
synchronizes that write through pthread_join. Explicit join waits, transfers the
result into caller storage, then releases the record without result destruction.
Implicit destruction waits, dispatches the result destruction adapter exactly once,
then releases both allocations. No operation can detach. Allocation/create/join
failure and self-join abort. `join()` returns R directly, with no panic Result.
The compiler now emits ordinary worker and unused-result destruction functions,
checks them through 6A/6B/post-drop, and specializes their explicit IDs. Backend
adapters only bridge canonical scalar/aggregate/zero-size ABI lanes. Core declares
a private usize handle field in non-Copy Task<R>; a trusted intrinsic join contract
transfers the stored result, rather than inventing borrowed-result inflow from a
bodyless generic declaration. Capturing move closures may have Shared, Mutable
or Once invocation; the wrapper borrows reusable inputs and destroys them after
calling, or moves a Once input. The runtime does not independently free captures.

At the 11A checkpoint, spawn accepted a direct zero-argument move-closure literal;
references/borrowed stack environments remain rejected, including captured loans
inside owned environments. 11B subsequently adds scoped borrowing and capability
contracts; general callable-value spawn remains deferred. Sequential execution
is never used as fallback. Private
function-pointer lanes assume the existing Linux x86_64 System V ABI.

11A validation covers real native scalar/string/generic-ADT/enum/array/zero-size
results, multiple/nested tasks, handle/capture moves, return/break/continue/nested
implicit completion, conditional initialization, overwrite/self-assignment,
exact capture/result destruction traces, worker SIGABRT, corrupted metadata and
both mutation corpora. Runtime tests assert distinct pthread identity, repeated
joins and deterministic injected allocation/create/join/self-join faults.

## 11B implementation decisions

Transfer and Share are semantic core declaration identities, not runtime dispatch
interfaces. Both are structural for scalars, strings, arrays and substituted ADT
fields/payloads. Neither implies Copy or the other capability. Shared references
require Share referents, mutable references require Transfer referents and are not
Share. Task<R> requires Transfer for R and is never Share. Recursive queries reject
cycles and stop at depth 64 or 4096 nodes. Unknown resources default to neither;
explicit NativeCapabilities entries in Decls grant trusted type/interface evidence.
Dynamic method sets and concrete implementations alone grant no erased capability.

Generic bounds use the existing obligation mechanism. Known callable values derive
capabilities from creation-site captures; Share additionally requires shared
invocation. Reassignment discards evidence. Erased signatures never establish
capture authority. Unscoped captures/results require Transfer and retain the
existing rejection of unresolved loans. A generic Transfer bound does not prove
loan-free storage.

Scoped spawn is explicit in typed metadata and carries an ordinary reference to
TaskScopeWitness storage. Scope-owned Task values, including retained discarded
expression temporaries, are the completion obligations. Unique handles cannot
escape their witness; no second runtime owner is required. Lowering destroys live
task-containing values before ending other scope storage on every normal exit,
including try propagation. A completion marker follows executable child destruction.
The same phase-11A pthread/runtime representation and verified worker/destruction
functions execute these obligations.

Scoped spawn transfers capture loans into Task holders. Joining/destruction is an
ordinary liveness use; whole ownership moves transfer holdings, and consuming join
or destruction releases them. Existing NLL overlap checks determine conflicts and
permit access after explicit whole-handle join. No backend loan query or second
borrow checker is involved. Borrowed scoped results remain rejected. Scoped handles
cannot be passed into ordinary functions or captured by other callables; projected
joins retain aggregate-level loans conservatively.

The completion report and validation scope are in
[phase 11B report](../scoped-tasks-report.md). Synchronization was subsequently
approved and implemented in 11C. The earlier stage reports remain historical
checkpoints; the complete current model is in [the Phase 11 report](../phase-11-report.md).


## 11C synchronization contract

### Mutex owner and capabilities

Mutex<T> owns its native mutex allocation and an inline, canonically aligned T.
Mutex.new(value) moves the payload in; its old binding cannot be reused. Ordinary
owner moves transport the initialized payload and unique private native lane;
they do not initialize another pthread mutex. Every initialized owner is destroyed
once. Neither the owner nor its private native pointer is exposed as a Copy value.

Both Transfer and Share for Mutex<T> require T: Transfer. Share for T is not
required: the mutex serializes payload access. This differs deliberately from
ordinary structural shared observation and is an explicit trusted synchronization
contract. It does not contradict the existing reference rules: &Mutex<T> transfers
when Mutex<T> is Share, while its ordinary loan must still cover completion.
Tests include shared Mutex<&mut Counter>, whose payload is Transfer and not Share.
Unknown/generic payloads without Transfer evidence remain rejected across tasks.

### Guard ownership, provenance and access

lock(&self) returns unique owned MutexGuard<T>, carrying a shared loan of the
mutex through ordinary result provenance. The guard is never Copy, Transfer or
Share. A whole guard move transfers both the held loan and unlock responsibility.
Guard-containing aggregates and returned guards retain that provenance. Its
verified destruction is a liveness use of the borrowed mutex, even if source code
never subsequently reads the guard. An owner cannot move, overwrite or die while
that destruction remains outstanding.

The minimal method API is:

```tarn
var guard = mutex.lock()
old := guard.read()           // T must be Copy
guard.replace(old + 1)       // returns the old T, moves the new T in
```

For an owned/aggregate payload use guard.value(), which returns &mut T borrowing
&mut guard. For example guard.value().count = guard.value().count + 1. Payload
references cannot outlive, move past or overlap incompatible use of that guard.
replace(&mut self, value T) T exchanges ownership; it does not clone or discard
an owned resource. The returned old value follows ordinary destruction. read's
Copy restriction is supplied by the trusted intrinsic catalog using existing
generic obligations; conditional method-owner bounds remain unsupported by
ADR 0015. No dereference syntax, public raw storage or separate lifetime checker
was added. Generic reference-bearing replace results retain conservative inflow
from receiver and arguments; they may keep loans longer than strictly necessary.

### Verified destruction and native mechanics

Existing post-drop Value and conditional Guard plans authorize destruction.
Complete Mutex destruction uses canonical T destruction glue, then native mutex
destruction/free. Guard destruction calls unlock, with no additional logical
ownership boolean in C. Dead/moved storage never executes a destruction plan.
Conditional initialization, moves, overwrite, self-assignment and all normal early
exits use existing initialization flags and drop elaboration. No user-defined Drop
or unwinding is introduced. The backend executes the selected plans and validates
ABI shapes; it never queries ownership, loans or provenance.

Private helpers tarn_rt_mutex_create/lock/unlock/destroy allocate and operate
pthread_mutex_t only. They never interpret generic T or run a T destructor.
Guard payload address lanes point into the borrowed owner; ordinary borrowing
keeps that address stable. Native failures abort. There is no poisoning because
panic aborts the process. Locks are not recursive/reentrant. Locking the same
mutex while retaining its guard, including waiting for a child that needs that
lock, may deadlock. No owner-thread tracking or deadlock detector is promised.

### Atomics

Only AtomicBool, AtomicI32, AtomicI64, AtomicU32, AtomicU64 and AtomicUsize exist.
Each is a unique non-Copy owner of private heap-backed C11 atomic scalar storage.
Each explicitly has Transfer and Share, independently of Copy and the ordinary
scalar capability derivation. The scalar cannot be projected or mutated through
ordinary references. Owner moves and destruction use ordinary resource semantics.

All Tarn v0 atomic operations are sequentially consistent, including unsuccessful
compare_exchange. There are no memory-order arguments. new initializes storage;
load observes, store replaces, swap returns the old value. Strong
compare_exchange(expected, replacement) returns bool: true exactly when comparison
succeeds and replacement occurs; false leaves storage unchanged. Returning success
alone avoids inventing an error type or changing Result/Option semantics. AtomicBool
supports those operations only. Integer atomics additionally offer fetch_add and
fetch_sub, each returning the previous scalar.

Fetch arithmetic is checked, matching ordinary Tarn integer arithmetic. Overflow
or underflow aborts before a write; neither signed nor unsigned values wrap.
Sequentially consistent CAS loops check each observed value and retry failed
exchanges. Signed bound tests avoid evaluating overflowing C expressions or
negating the minimum integer. An observed overflowing attempt may abort even if
another worker subsequently changes the scalar; the process has abort-only faults.
Private concrete runtime helpers use matching scalar ABI widths; free releases
storage. No generic atomics, public barriers, relaxed orders or synchronization
bypass exist.

### Acceptance evidence

The complete existing regression suite and added native synchronization gates pass.
Eight Tarn workers deterministically perform 40,000 mutex increments and 40,000
atomic increments. A private barrier-coordinated runtime gate performs 160,000 of
each with eight simultaneous workers under undefined-behavior instrumentation.
All six atomic APIs, success/failure comparisons and each integer boundary are
covered. Exact destruction traces cover guard/owner moves, early exits, conditional
initialization, overwrite, self-assignment, nested locks, and owned payload cleanup.
Canonical safety rejections and both mutation corpora cover synchronization;
ABI corruption rejects and trace oracles detect removed/duplicated unlocks.
No Phase 11 decision required by this v0 model remains unresolved. General native
annotation syntax, richer callable evidence and borrowed scoped results remain
future extensions, not promises made by acceptance.
