# Phase 11 completion report: safe native concurrency

Phase 11A native tasks, 11B Transfer/Share and scoped loans, and 11C synchronization
are implemented and validated for the Linux x86_64 v0 model. ADR 0033 is accepted.
Phase 12 has not started. No async/await, networking, schedulers, pools, detach,
cancellation, optimization, channels, public barriers, RwLock, Condvar, Semaphore,
user-defined Drop, unwinding, LLVM or package manager implementation was added.

The earlier [11A](native-tasks-report.md) and [11B](scoped-tasks-report.md) reports
record their historical checkpoints. This report states the final Phase 11 model.

## 11A native tasks

A direct zero-argument `spawn move fn() R { ... }` transfers its owned callable
into a real pthread and produces a unique non-Copy Task<R>. One OS thread executes
one task. There is no pool or scheduler. Invocation mode remains independent of
capture ownership: reusable shared/mutable callables are borrowed for execution,
then destroyed; consuming callables move their captures into their verified body.

Explicit join consumes the handle and returns R. Dropping an unjoined initialized
handle waits, destroys its unused result through a generated verified destruction
function, and frees native storage. A handle move transfers this obligation.
Worker panic aborts the whole process. Allocation, pthread and impossible self-join
faults abort; there is no recoverable task error or panic transport contract.

## 11B Transfer/Share and scoped loans

Within lexical `scope { ... }`, direct borrowed or owned closure literals can
capture loans whose storage survives completion. Scoped spawn carries an ordinary
TaskScopeWitness reference in typed IR. The scope's owned handles, including
retained discarded-expression temporaries, are its completion obligations. There
is no second runtime scope owner or concurrency borrow checker.

Existing forward loan flow and backward liveness retain capture/storage loans
until consuming whole-handle join or destruction. Moves carry the holdings.
Lowering completes task-containing resources before borrowed storage ends on
normal exit, return, try propagation, break, continue and nested cleanup. Existing
place overlap allows disjoint fields and remains conservative for dynamic indexes.
Scoped handles cannot escape the witness. Borrowed scoped results remain rejected.

## 11C synchronization

Core now declares only Mutex<T>, MutexGuard<T>, AtomicBool, AtomicI32, AtomicI64,
AtomicU32, AtomicU64 and AtomicUsize as the synchronization surface. The guard is
the result of locking, not an independent constructor or general borrowing API.
All public signatures live in core; trusted declaration identities provide the
resource, capability and intrinsic contracts needed during bootstrap.

A mutex owns its private native allocation and an inline T payload. Construction
moves T in. Normal owner movement transports its initialized payload and private
native lane; it never initializes a second native mutex. Payload access is private
and serialized by a guard. An atomic owns private C11 synchronized scalar storage,
with no ordinary mutable scalar projection. No synchronization operation grants
permission to bypass ownership or extend a loan.

## Final task ownership model

Every task has one logical handle owner. Explicit join transfers its result;
implicit completion destroys an unused result. Neither worker and caller nor
moved and original handle storage simultaneously own those resources. Ordinary
move/init state selects executable post-drop destruction; conditional paths use
the existing explicit initialization flags.

Scoped handles complete before their borrowed storage ends. A scope does not
make arbitrary escaped references valid. Unscoped workers still cannot receive
local loans. Owned closure environments, worker wrappers and result destruction
functions pass through the ordinary semantic pipeline before code generation.

## Final capability model

| Type | Transfer | Share | Copy |
|---|---|---|---|
| Scalars and strings | yes | yes | scalars only |
| Ordinary arrays/ADTs | all substituted components qualify | all substituted components qualify | existing Copy rules |
| &T | T: Share, with valid provenance | T: Share | yes |
| &mut T | T: Transfer, with an exclusive valid loan | no | no |
| Task<R> | R: Transfer | no | no |
| Mutex<T> | T: Transfer | T: Transfer | no |
| MutexGuard<T> | no | no | no |
| All six atomic owners | explicit yes | explicit yes | no |

Mutex Share deliberately requires Transfer of the payload, not Share of the
payload: exclusive guard access serializes it. Shared Mutex<&mut Counter> workers
are a concrete regression for that distinction. Unknown native/dynamic authority
is absent unless trusted declaration metadata supplies it. Copy and byte size
never grant cross-thread authority. Generic callers need existing capability
bounds; bounded/cycle-safe structural queries reject insufficient evidence.

Known callable values use creation-site capture evidence and invocation mode.
Erased signatures, arbitrary returned callable values and callable ADT fields do
not acquire inferred capture authority. A Mutex can own and execute an owned
closure locally; sharing a Mutex of an erased callable remains conservative when
its Transfer evidence is unavailable. This retains the approved 11B limitation.

## Mutex/guard semantics

`Mutex.new(value)` moves the old value into the owner. `mutex.lock()` returns an
owned, non-Copy, non-Transfer, non-Share guard. Its ordinary result provenance
borrows the mutex. The private guard lanes contain native and payload addresses;
the held loan keeps the inline payload address stable. Guard containment is known
to ordinary loan flow, including functions, arrays and generic aggregate holders.

Payload access uses a minimal method API:

```tarn
mutex := Mutex.new(i64(0))
{
    var guard = mutex.lock()
    guard.replace(guard.read() + 1)
}
```

`read(&self) T` requires Copy and copies the protected payload. Its trusted
intrinsic contract registers the restriction through existing generic obligations;
no conditional method-owner syntax was added. `replace(&mut self, value T) T`
transfers a new payload in and the old payload out without duplication. An unused
old owned value is destroyed by ordinary cleanup. `value(&mut self) &mut T` lends
exclusive access for owned/aggregate payloads, such as
`guard.value().count = guard.value().count + 1`. Its result borrows the guard.
No payload loan may outlive, move past or conflict with that guard.

Guard destruction is an ordinary liveness use of its mutex loan. A never-read
guard therefore still prevents moving or destroying its mutex before unlock.
Whole guard moves carry both the loan and unlock responsibility and clear the
moved holder. Existing post-drop Value/Guard plans unlock only live initialized
storage. Complete owner destruction destroys T through canonical destruction glue,
then destroys/frees the native mutex. C never interprets or destroys T. There is
no separate runtime ownership boolean and no general user Drop mechanism.

Exact traces cover normal exit, return, break, continue, nested guards on different
mutexes, owner/guard moves, generic guard aggregates, conditional initialization,
overwrite and self-assignment. Moved or uninitialized storage does not unlock.
Owned payload tests cover string, generic ADT, enum, array and owned closure.

There is no poisoning: panic aborts. There is no reentrancy guarantee or owner-thread
tracking. Locking the same mutex twice while retaining the first guard may
block indefinitely. Holding a guard while joining a child that needs that mutex
can likewise deadlock; end the guard scope before waiting for that child.

## Atomic semantics

All Tarn v0 atomic operations are sequentially consistent. No memory-order argument
or relaxed/acquire/release API exists. Each concrete owner is Transfer and Share,
never Copy. Normal ownership and loans keep storage alive through worker use.
The underlying scalar and native lane cannot be accessed through public fields.

Every type supports new, load, store, swap and strong compare_exchange. Integer
types additionally support fetch_add and fetch_sub; AtomicBool has no arithmetic
methods. swap and integer fetch operations return the previous scalar. store
returns void. compare_exchange(expected, replacement) returns true exactly when
comparison succeeds and replacement occurs; false leaves storage unchanged.
It does not invent a language-level error or change Result/Option behavior.

Arithmetic checks overflow/underflow and aborts before writing, matching ordinary
Tarn integer arithmetic. Sequentially consistent CAS loops check each observed
value and retry after contention. Signed checks avoid C overflow and avoid
negating the minimum integer. An observed overflowing attempt may abort even if
a different worker later changes the scalar. No lock-free/wait-free promise is
made by this API; implementation tuning remains outside Phase 11.

## Runtime ABI

The ABI is private Linux x86_64 System V. Task helpers create/wait/release/drop
native records; generated adapters bridge canonical callable/result lanes.
pthread_join publishes the initialized result. Ordinary verified functions
execute capture and unused-result destruction.

Mutex helpers create, lock, unlock and destroy allocated pthread_mutex_t storage.
The compiler owns the inline generic payload and guard address lanes. Complete
post-drop destruction authorizes native unlock and recursive payload destruction;
the backend executes these plans without querying moves, loans or provenance.
Native status failures abort rather than hiding an invariant violation.

Concrete atomic helpers allocate/initialize, access and exchange C11 `_Atomic`
storage with matching bool/i32/i64/u32/u64/usize ABI widths. Integer fetch helpers
use checked sequentially consistent CAS loops. The private destruction helper
frees their allocations. Linking continues to use the system C11 toolchain and
`-pthread`; no external dependency or lock-order framework was added.

Opt-in TARN_TRACE_SYNC and existing TARN_TRACE_DROPS are private test observations.
A private pthread barrier coordinates the runtime stress test; it is not a public
Tarn primitive. Runtime tracing is not ownership state or a synchronization API.

## Stress testing and validation

The full Cargo workspace suite passes: **112 passed, 0 failed, 1 intentionally
ignored host-dependent benchmark**. Required checks pass without Rust warnings:
`cargo test --workspace --locked -j 4`, `cargo check --workspace --locked -j 4`,
`cargo build --workspace --locked -j 4`, the stdio LSP smoke test, and
`git diff --check`. All 11A/11B, callable, dynamic-interface, move/borrow,
destruction, native runtime and CLI regression suites remain green.

Eight concurrently retained Tarn workers perform 5,000 increments each, producing
exactly 40,000 through a shared mutex and 40,000 through a shared atomic. Four
shared mutex readers produce the deterministic sum 160,000. The separate private
runtime gate starts eight barrier-coordinated workers and performs 20,000 increments
each: both counters end at 160,000. That gate uses undefined-behavior instrumentation
and tests signed arithmetic endpoint transitions. No sleeps establish correctness;
timeouts are watchdogs for regressions that deadlock.

All six concrete APIs and compare/exchange success/failure are exercised. Ten
native overflow/underflow cases require SIGABRT across all five integer types.
Exact native traces prove unlock count, resource count, and guard-before-owner
cleanup. Owned payload traces include replacement and the old value's destruction.
ABI mutation tests reject malformed synchronization calls without compiler panic.
Removed/duplicated post-drop unlock mutants fail the exact trace oracle or runtime
status gate; the verifier is not claimed to prove arbitrary destruction-plan
completeness from already-corrupted post-drop IR.

Canonical memory_safety now includes shared mutex and atomic workers, local guard
moves, returned/aggregate-held guard provenance, exclusive payload mutation,
non-Share Transfer payloads, constructor moves, never-read guard destruction,
owner move/overwrite/destruction while locked, escaped payload loans, cross-thread
guards, missing Transfer/Copy evidence and synchronization bypass. Existing error
codes are reused; no new concurrency-specific code was needed. Both source
mutation corpora include mutex, owned payload and atomic fixtures. Accepted native
mutants must still emit structurally valid code without panic; trace mutation
gates separately ensure the cleanup oracle detects missing/double unlock.

## Bugs found across Phase 11

11A's bodyless generic join initially acquired an invalid borrowed-result contract;
its trusted stored-result transfer is explicit. Worker and result destruction IDs
needed specialization reachability and verifier checks. Early scope completion
had been emitted after local destruction and omitted on some early exits.

11B found discarded spawn temporaries completed too early to expose concurrent
conflicts. Retained scope temporaries and task-drop liveness now keep captured
loans alive. Whole moves and consuming joins must release the old holder's loans.
Task containment must inspect actual ADT declarations; closure bodies need their
own scope context, and scoped metadata must survive final table merging.

11C exposed that generic arguments alone cannot identify an ownership-bearing
guard with private scalar address lanes. Declaration-aware reference containment
and destruction-time liveness now preserve its mutex loan, including through
functions and generic aggregates. Moves release the old holder without losing the
new guard's obligation. A generic owned synchronization constructor also needs an
owned-result contract instead of bodyless reference-source elision.

Implementation checks also prevented aggregate replace results from aliasing the
newly written payload: the old aggregate is snapshotted before installation.
Existing syntax does not support conditional method-owner bounds, so the read
Copy restriction uses trusted ordinary obligation metadata rather than expanding
language syntax. Regression tests cover these boundaries; no unresolved test
failure or capability contradiction remains in the implemented model.

## Decisions I would defend

Unique task/mutex/guard/atomic ownership; completion and unlock through verified
destruction; ordinary loan provenance rather than a second lifetime checker;
Mutex Share requiring payload Transfer; explicit synchronized scalar storage;
sequential consistency; checked integer fetch arithmetic; success-only strong
compare/exchange; one pthread per task; abort-only panic/faults; no poisoning or
reentrancy promise; a private runtime that never interprets generic payloads.
These choices keep the frontend authoritative and the supported surface small.

## Decisions I still question

A mutex keeps T inline and allocates native mutex storage separately. This avoids
a generic runtime payload owner, at the cost of moving large inline values through
the existing canonical copy path. No alternative allocation or optimization was
introduced without evidence. The ergonomic API uses read/replace/value instead of
special field projection; Copy-only read needs a trusted bootstrap obligation
because conditional owner methods remain unsupported.

General callable-value spawn, richer erased capture evidence, borrowed scoped
results, projected completion precision and passing scoped handles through
ordinary functions remain deliberately conservative. Reference-bearing generic
replace results may keep receiver/argument loans longer than necessary.
Loan-bearing replacement inputs now reject with E3051 after Phase-13 review
demonstrated a scope-local reference escaping through a discarded replacement.
Constructors and reference-free replacements retain their existing behavior.
General native contract annotation syntax remains future work; unknown contracts reject.
Join-on-drop and nonrecursive locking can deadlock on application dependencies.
These are explicit v0 limits, not unresolved promises blocking ADR acceptance.

## Remaining blockers before Phase 12 async I/O / networking

Phase 12 needs separate authorization and an agreed resource/lifetime contract
for suspended I/O, buffers and native handles. Existing unmodeled filesystem and
network APIs still reject with E3040; no working networking or async API is claimed.
Native contract boundaries and documented bootstrap safety gaps need review for
that new surface. A green Phase 11 suite establishes the implemented concurrency
model, not complete end-to-end memory safety for all future language features.

Work stops at Phase 11. No Phase 12 implementation or architectural expansion
was begun.
