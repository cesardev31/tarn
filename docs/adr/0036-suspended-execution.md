# ADR 0036: manually polled operations and a single-thread executor

Status: accepted for Phase 12C, within the manual Linux v0 boundary.

## Semantic boundary

Manual `Operation<R>` stores an owned mutable closure, a unique Waker resource
and a completion bit. Its closure environment contains all across-poll owned
values and loans; ordinary capture/provenance/move/drop analysis remains decisive.
Progress is Pending or Ready(R). Exclusive polling can return exactly one Ready;
repoll after completion aborts. Completion clears registration. The poller and its captures are destroyed by
consuming finish/drop; this also supports results borrowing poller storage. A consuming
finish/drop releases the operation's conservative static loans; v0 does not
infer loan release from a returned runtime discriminant.

The closure environment and Waker records are stable heap allocations; no raw
self-pointer or pointer into a polling frame survives polling. Kernel I/O retains
no application buffer. Readiness registrations belong to Waker bookkeeping,
never socket ownership. Waker destruction removes interest and retires identity.

## Explicit owners

Execution owns a 12B Poll and mechanical native wake bookkeeping. It is not Share
or Transfer. Waker has an ordinary inferred shared loan of Execution and is
non-Copy/non-Transfer/non-Share. Destruction is a liveness use for Waker-containing
values, reusing the existing task/guard resource-loan mechanism. No generic user
reference-bearing aggregate exemption, public pinning or lifetime syntax exists.

Executor owns a bounded table of 16 Operation<void> slots. It borrows Execution
when driving turns; operations themselves keep Execution alive. Splitting reactor
storage from the task table avoids self-referential owners. Task results are
handled inside verified manual poll bodies or through direct Operation<R> polling;
there is no native Task handle reuse or detached executor task.

## Wake and fairness

A process-wide unique integer identifies each Waker, independently of fd/address;
exhaustion aborts, never wraps. Mechanical wake coalesces into one queued bit.
Taking the bit before polling preserves reentrant wake during the poll for the
next turn. One poll per occupied runnable slot per turn gives bounded fairness.
Every turn services the existing Poll, even if another task remains runnable.
C provides only wake-bit/identity/registration mechanics; Tarn implements task
storage, turns, completion, fairness and network state machines.

## Readiness and lost wakes

Operations attempt a nonblocking syscall, arm interest after WouldBlock, retry,
and return Pending only when the retry still WouldBlock. Level-triggered 12B Poll
preserves readiness occurring before registration. Arm uses the existing token,
SO_COOKIE and fd-reuse protection; there is no second epoll implementation.
Stale events cannot reach a retired Waker or unrelated recycled identity.
Wake means poll again, never completion. Spurious/coalesced wakes are safe.

Read/write complete on one successful count; write_all stores its offset and
performs at most one successful write per poll, self-waking after partial progress.
Accept retries WouldBlock; connect reuses Connecting and its latched SO_ERROR
policy; UDP empty datagrams remain Ready results. Blocking sockets are rejected
before executor-driven I/O. DNS is outside this model.

## Destruction and remaining policy

Ordinary post-drop destruction handles poller captures, registrations, Waker and
Execution storage. Abandonment produces no result and is not a cancellation API.
Panic aborts the process. The manual surface is bootstrap/internal, not a stable
Future ecosystem. No async/await syntax, scheduler pool, multithread executor,
io_uring, HTTP, TLS, channels, timers API or optimization is introduced.

## Validation and conservative limits

The executable model and limits are recorded in
[the Phase 12C report](../suspended-execution-report.md). Executor insertion uses
consuming result provenance; completed holders release static loans by finish/drop.
A nested operation explicitly forwards wakes by identity before polling. All wake
and scheduling access remains on one thread; native tasks coexist through sockets.
