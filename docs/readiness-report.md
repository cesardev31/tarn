# Nonblocking I/O and readiness completion report (Phase 12B)

## Nonblocking socket model

All three existing socket owners gain exclusive `set_nonblocking(bool)` without
changing ownership or types. Blocking constructors and accepted streams keep
12A behavior. Descriptor mode is checked before write_all, which rejects
nonblocking mode before sending any bytes. Actual partial writes stay explicit.
No dependency, platform expansion or optimization was introduced.

## WouldBlock semantics

Accept/read/write/recv/send return WouldBlock without hidden waiting. Successful
partial writes return counts, including when the next retry blocks. A readiness
event never guarantees progress; retries retain ordinary socket and buffer loans.
Zero-length UDP datagrams remain data and nonempty TCP reads return zero at EOF.

## Poll representation

A canonical eight-byte private handle owns a native record containing an epoll
fd and registration list. Poll is non-Copy, Transfer, not Share. Existing Value
and Guard post-drop plans close it and free bookkeeping exactly once. Poll never
closes registered sockets. Event is a canonical 16-byte Copy struct: a u64 token
and four bools, with padding; native layouts and private signatures are verified.
Caller-provided slices hold events, with up to 64 results per syscall and no
per-wait allocation. Level-triggered readable/writable interest is the only mode.

## Registration/token model

Registration takes a short shared socket borrow, returns a private Copy Token,
and retains no socket loan. Process-wide monotonic tokens never wrap or reuse;
exhaustion returns an error. Modify/deregister require both socket and token.
Readiness records contain no pointers into movable Tarn values or application
buffers. Saved event tokens are observations rather than I/O authority.

## fd reuse strategy

Records also store Linux SO_COOKIE identity. An old token cannot modify or
remove a new socket at the same integer fd. Closing the last socket owner removes
kernel interest; reused fds replace old bookkeeping only after successful add.
Closed records can remain until fd reuse or Poll destruction, without retaining
socket resources. No socket duplication or hidden socket owner is exposed.

## Borrowing model

All Poll registry/wait methods require mutable receivers. Actual I/O still
requires exclusive socket access; mutable byte/event buffers use the existing
borrow checker. No application buffer is pending after a syscall returns.
Registration is independent of actual I/O authority. Canonical safety tests
reject double close and conflicting event-buffer aliases, and accept socket close
while registered plus event-buffer reuse after wait.

## Connect progression

Nonblocking connect constructs an owned Connecting wrapper with an ordinary
TcpStream and explicit status. Immediate success and EINPROGRESS have separate
paths. Interrupted initiation stays pending. poll_connected checks SO_ERROR and
getpeername; zero error without a peer remains pending, and terminal errors latch
before SO_ERROR can be consumed again. into_stream consumes successful storage;
consuming before completion returns an error and closes the connection. No hidden
background operation or successful-connect assumption follows writable readiness.
DNS remains blocking.

## Timeout/EINTR semantics

wait accepts -1 infinite, zero immediate and positive i32 milliseconds. Empty
storage or values below -1 return errors. Tarn transparently retries interrupted
waits, recomputing remaining finite time from a monotonic deadline. Repeated EINTR
cannot restart the timeout or become a spurious empty set. Linux scheduling may
overrun deadlines. No larger time subsystem was added.

## Runtime ABI additions

Eight private net declarations bridge mode changes/query, connection completion,
monotonic time, Poll construction/control/wait/close. Existing private _Raw results
and normal Result error mapping remain. Runtime drop releases the Poll record.
C handles Linux syscall representation and mechanically checks opaque registry
identity; public policy, retries and connection state remain in Tarn. Embedded
source metadata remains required; names net/core/poll/runtime confer no authority.
Backend layout checks supplement structural Token/Event/Poll and signature checks.

## Tests and stress evidence

Nine dedicated readiness tests exercise actual Linux executables and the C ABI:
TCP/UDP/listener readiness; WouldBlock; mode restoration; partial writes; HUP with
readable data followed by EOF; empty datagrams; modification/deregistration;
closed registrations; failed and successful pending connects; immediate-success
branch injection after actual loopback completion; zero/finite/infinite waits;
monotonic EINTR deadline and expiry injection; multiple worker producers;
Poll/socket transfer; normal/return/try/break/continue cleanup; registration errors;
eight declaration corruptions and two illegal Copy operands; private authority.

Native ABI stress runs 1,024 fd close/reuse cycles with old-token attacks, 80
simultaneous registrations with 64-event batching/fairness, token exhaustion,
128 TCP accepts and actual send-buffer saturation/drain/retry. /proc/self/fd
counts must return to baseline. Tarn loopback runs 128 UDP readiness cycles;
two native task producers send 256 datagrams. Fault-injected partial sends run
128 partial-success/WouldBlock pairs. No sleeps establish correctness; real
poll/readiness with bounded timeouts drives progress. Resource traces assert
balanced acquisition/close, including failed connect and every normal exit.
The existing 16 networking regressions retain SIGPIPE, blocking, EINTR and fd
leak checks. Both source mutation corpora include the readiness fixture.

Validation: the complete `cargo test` workspace run passed, including both
mutation corpora and all Phase 11/12A regressions. The final expanded readiness
suite passes all nine tests; the updated canonical memory-safety suite passes.
`cargo check --workspace`, CLI/LSP builds, LSP stdio smoke tests and
`git diff --check` pass without compiler warnings. The host-dependent benchmark
remains intentionally ignored. VS Code's running LSP was refreshed and its
executable inode checked against the rebuilt binary.

## Bugs found

SO_ERROR is consumptive: failing to latch it can turn a second completion check
into a misleading zero error. Connecting preserves the failure explicitly.
A bare fd/token registry can act on an unrelated recycled descriptor; cookie
validation and global nonreused tokens prevent that. An initial TCP stress test
spun faster than kernel delivery and exhausted iterations; it now waits for
actual progress with bounded readiness instead of treating CPU iterations as time.

## Decisions I would defend

Descriptor mode without type-state generics; blocking-only write_all; explicit
partial progress and connection status; level-triggered events; unique integer
identity with cookie validation; short registration loans; ordinary exclusive I/O
loans and post-drop cleanup; monotonic interrupted-wait deadlines. These choices
supply readiness without committing the language to async syntax or scheduling.

## Decisions I still question

SO_COOKIE makes the boundary Linux-specific and requires kernel support. Opaque
C registration bookkeeping and socket destruction remain trusted bootstrap code,
not user-defined Drop or a compiler proof of native syscalls. Registry allocation
is per registration, and closed entries may occupy memory until reuse/drop.
Token lookup is linear and wait batches 64 events; both are correctness-first
limits, without optimization. Public owner-type-specific methods avoid a new
socket interface/extern contract but have some naming repetition.

## What Phase 12C must solve before async/await

Design suspended-operation ownership, result/cancellation cleanup, wakeup identity
and delivery races, task scheduling authority, polling fairness, and how captured
loans outlive suspension without escaping storage. Readiness alone grants no I/O
permission. Preserve explicit WouldBlock and partial progress. Any future pending
kernel-buffer API requires a separate lifetime design; this phase keeps none.
No async/await, Future, executor, scheduler, pool, green threads, io_uring, HTTP,
TLS, channels, cancellation or custom user destructors were implemented.
