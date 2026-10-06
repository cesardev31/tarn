# Suspended execution completion report (Phase 12C)

## Poll/Pending/Ready semantics

The manual/bootstrap Operation<R> has exclusive mutable poll. Progress<R> is
Pending or Ready(R). Ready is produced once; repoll after completion aborts before
invoking the poller. Manual callers consume finish to release the completed
operation. The API is provisional, not a stable public Future ecosystem.

## Suspended state representation

Operation stores an ordinary owned mutable closure, a Waker and completion bit.
The closure's verified heap environment contains stored state, owned resources and
captured loans. write_all's offset is an ordinary mutable capture. Progress is an
ordinary enum; result transfer uses existing moves and post-drop plans. Neither C
nor the backend stores or guesses application types, reference provenance, result
ownership or state transitions. No pointers into one polling frame survive it.

## Wake identity

Each native Waker record has a process-wide unique integer, independent of fd or
movable Tarn storage. It shares the 12B nonwrapping identity allocator; exhaustion
aborts for infallible Waker creation. Records live in stable heap storage and are
retired on verified destruction. Native forwarding uses integers rather than
pointers to other Wakers; a retired forwarding target is inert. Wakers and
Execution are non-Copy, non-Transfer and non-Share. No cross-thread wake API exists.

## Lost-wakeup protocol

Canonical operations attempt nonblocking I/O, register after WouldBlock, retry,
and return Pending only after another WouldBlock. The existing level-triggered
Poll observes readiness occurring before registration. Waker associations use
12B tokens and SO_COOKIE checks, without a second epoll implementation. Ready and
abandonment remove registrations. Wakes during poll remain queued for another
turn because the queued bit is taken before invocation. Nested poll_with installs
a same-Execution, cycle-checked forwarding identity before polling the child.

## Executor model

Execution owns a Poll and mechanical wake bookkeeping. Executor separately owns
16 Option<Operation<void>> slots and borrows Execution while driving turns. This
split avoids a self-referential owner: operations retain a visible shared loan of
Execution, whose native registry has single-thread interior bookkeeping access.
Scheduling authority is not application-resource ownership. Wrong Execution
selection returns a normal error rather than waiting on the wrong reactor.

Executor insertion consumes and returns the table, so normal result provenance
carries new operation loans into the caller's holder. A mutable insertion API
would hide newly transferred loans in a callee side effect under the current
provenance model; no such escape hatch was introduced. run consumes the executor;
turn supports manual driving. Results are handled in verified poll bodies or by
manual Operation<R> callers; executor slots complete with void. Native pthread
Task<R> remains distinct and can provide readiness stimuli.

## Fairness

Each turn services readiness, then polls each occupied runnable slot at most once
in index order. A queued bit coalesces repeated wakes without duplicate queue
allocation. Readiness is serviced even while another operation self-wakes. An
idle turn waits using the 12B timeout/EINTR policy. There are no priorities, pool,
work stealing, detached tasks or scheduler optimizations.

## Borrowing across suspension

Owned environments use existing capture/loan/result provenance. A Waker's trusted
constructor returns a resource borrowing Execution. Declaration-aware loan-resource
containment extends the existing guard destruction/liveness mechanism to Wakers
and their aggregates; it is not a second borrow checker or runtime loan registry.
Operation destruction is consequently a use of retained stream/buffer/context
loans, even after the last poll. Across-poll buffer aliases, moves, stream mutation,
Execution destruction and escaped local buffers are rejected. finish or scope
exit releases those loans. No user reference-bearing aggregate restriction changes.

Completion clears readiness but conservatively keeps static captured loans until
finish/destruction. A returned result may itself borrow poller storage; eager
poller destruction would violate that provenance. Future lowering can consume a
completed holder at an appropriate source boundary; v0 does not derive loan ends
from a runtime Ready discriminant. There is no public pinning or lifetime syntax.

## Drop/abandonment semantics

Ordinary closure destruction handles all captured application values. Waker glue
removes registration, retires identity, unlinks bookkeeping and frees its record.
A socket captured by ownership may close before the Waker's cleanup; cookie/token
checks never deregister an unrelated recycled descriptor. Execution destroys wake
bookkeeping before its Poll, and verifies no live Waker remains. Executor drop
recursively destroys every initialized slot through normal post-drop plans. No
result is manufactured for abandoned Pending state and nothing detaches. Panic
aborts the process without transport or unwinding.

## Read/write/write_all state machines

read_operation and write_operation retain ordinary borrowed buffers and socket
loans, returning one actual count, EOF or Error. All executor I/O checks O_NONBLOCK
before attempting a syscall; blocking sockets produce an error. write_all_operation
retains the source loan and stored offset, performs at most one successful partial
write per poll, and self-wakes after partial progress. WouldBlock arms writable
interest and returns Pending; offsets resume without resending previous bytes.
WriteZero and ordinary errors complete normally. No kernel buffer is pending.

## Accept/connect integration

accept_operation retries WouldBlock and yields one owned TcpStream. Connecting
retains 12B's pending/status/SO_ERROR model. connect_operation borrows it and yields
Result<void, Error> on confirmed completion; after finish the caller consumes
Connecting.into_stream. This avoids moving a resource out of a reusable captured
borrow. recv_operation handles UDP, including empty datagrams as successful data.
All constructor and poll helpers are manual/bootstrap APIs, without async syntax.

## Runtime additions

Execution allocation/free, Waker allocation/free, queued-bit operations, arm/clear,
readiness-to-wake delivery, forwarding and owner checks are mechanical bridges.
C owns no poller callback, task table, application buffer, result or scheduling
loop. Tarn implements all task turns and network policies. Native declaration
identities, private shapes, borrow-mode/result contracts and canonical layouts are
verified; a numeric pointer substitution cannot grant intrinsic authority.

## Tests/stress

Ten dedicated native tests cover immediate Ready; Pending then Ready; TCP
read/write/write_all/accept/connect; UDP empty datagrams; nested executor readiness
with a native task producer; repeated/coalesced/spurious/reentrant wakes;
deterministic alternating fairness counters; wrong Execution; blocking rejection;
Pending read/write/write_all abandonment; generic ADT/string result destruction;
double-result repoll abort; seven metadata corruptions; loan visibility and local
escape rejection. Linker injection puts actual data between observed WouldBlock
and registration, and forces partial sends/EAGAIN to verify offset persistence.

Native ABI stress executes 2,048 registration/drop/reuse cycles with 128 coalesced
wakes each, 64 simultaneous registered sockets, stale parent forwarding and cycle
rejection. /proc/self/fd must return to baseline. Exact fd and string traces assert
cleanup/transfer once. The canonical memory-safety suite adds pending-buffer,
write_all-source and Execution-lifetime cases. Both mutation corpora include
suspended_io and executor_turns. No sleep is used as a correctness proof.

The pre-phase baseline and final `cargo test` both pass, including the 23-test
native backend suite, 12A/12B networking, Phase-11 runtime tests, memory_safety,
ownership/drop regressions and both source mutation corpora. The host-dependent
benchmark remains intentionally ignored.

`cargo check --workspace`, all ten dedicated execution tests, the additional
peer-close/EOF regression, CLI/LSP builds and `tools/lsp/tests/smoke.py` pass
without compiler warnings. The running VS Code LSP was restarted and its executable
inode matches the rebuilt binary.
`git diff --check` also passes. No dependency or optimization pass was added.

## Bugs found

Eagerly replacing a completed poller was rejected correctly: a generic result can
borrow its environment. Explicit consuming finish preserves those obligations.
Mutable executor insertion could conceal transferred reference-bearing state;
consuming insertion carries it through existing result provenance. A child
operation's readiness initially targeted only the child; explicit identity
forwarding connects it to the executor slot without raw parent pointers. Wrong
Execution selection now errors before polling/waiting. Queued-bit consumption
before invocation preserves a reentrant wake for the next turn.

## Decisions I would defend

Verified owned closure storage, explicit manual polling, shared reactor-loan
holders with single-thread native bookkeeping, consuming table insertion,
nonreused identity, level-triggered register/retry, bounded fair turns, one-write
progress steps and ordinary generated destruction. These choices exercise the
existing pipeline before new language syntax or an opaque runtime state machine.

## Decisions I still question

The bootstrap surface currently lives in net alongside readiness. The table limit
is 16, forwarding depth is bounded at 64, and lookups/scans are linear. Completed
operations require consuming finish for conservative loan release. Executor slots
return void; richer typed task results and precise reusable-callback contracts are
future decisions. Socket-mode errors currently use InvalidAddress/EINVAL because
there is no new public execution error taxonomy. Native allocation/wake exhaustion
abort. Private native cleanup remains a trusted compiler/runtime boundary.

## What must be solved before exposing async fn and await

Specify source-to-state lowering, source-position diagnostics around suspension,
which stored values remain initialized at each suspension, consuming completed
holders while preserving borrowed results, precise callback result contracts,
typed executor result handles and stack/heap storage choices justified by evidence.
Preserve the current manual semantics rather than deriving them from syntax.
No async fn, await, Future combinator ecosystem, multi-thread executor, work
stealing, io_uring, HTTP, TLS, channels, public cancellation, timers API, blocking
DNS integration, user Drop or optimization was implemented.
