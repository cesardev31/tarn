# ADR 0035: nonblocking sockets and level-triggered readiness

Status: accepted for Phase 12B, Linux x86_64 v0.

## Ownership and mode

Existing socket owners and blocking operations remain unchanged. Explicit
`set_nonblocking(bool)` mutates Linux O_NONBLOCK with an exclusive receiver;
mode is descriptor state, not type state. `write_all` is blocking-only and rejects
a nonblocking descriptor before writing anything, including an empty buffer.
Single read/write and datagram calls preserve partial counts and WouldBlock;
there is no implicit readiness wait. Accepted streams start blocking on Linux.

Readiness is evidence that an operation may make progress, not permission to
bypass normal socket error handling. Registration never transfers ownership of
a socket or application buffer to the kernel/runtime. Tarn keeps no borrowed
application buffer pending across a readiness wait.

## Poll and identity

`Poll` owns a private native pointer to an epoll descriptor and registration
records. It is non-Copy, Transfer, not Share; all registry operations use a mutable
receiver. Drop closes the epoll descriptor exactly once and frees records without
closing sockets. Records are bookkeeping, never socket ownership or liveness.

Registration borrows a socket only during the call and returns a Copy `Token`
with a private u64. Tokens come from a process-wide monotonic counter, never fd
numbers or movable stack pointers; exhaustion fails rather than wraps. A token
cannot access a socket. Modify/deregister additionally require a socket borrow
and compare its Linux SO_COOKIE with the registration's cookie. New socket owners
at a reused fd acquire new tokens. Copied events after deregistration/close are
inert observations of old tokens, never authority over a new socket. Closing the
last owner removes kernel interest; stale bookkeeping may persist until Poll drop
or replacement registration at that fd. No descriptor duplication is exposed.

## Events and waits

Readable, Writable and ReadableWritable map to level-triggered epoll interest.
No edge/one-shot/exclusive mode exists. Copy events contain token plus readable,
writable, error and hangup booleans; HUP/RDHUP never suppress readable bytes or
replace normal EOF/error handling. `wait(&mut []Event, timeout_ms)` returns a count;
-1 waits indefinitely, zero polls, positive values bound waiting with monotonic
milliseconds. Empty storage and values below -1 return errors. C translates at
most 64 kernel events into caller storage per call, without allocating; a larger
buffer is valid and later calls drain additional ready descriptors fairly.

Tarn retries EINTR without exposing an empty set and recomputes the remaining
finite timeout against a monotonic deadline. Signals cannot restart the full
finite timeout. No syscall retains application memory after returning.

## Connection progression

`TcpStream.connect_nonblocking[_addr]` returns an owned `Connecting` containing
an ordinary TcpStream. EINPROGRESS (or interrupted initiation) leaves it pending.
`poll_connected()` observes SO_ERROR and confirms getpeername rather than treating
writability or zero SO_ERROR alone as success. It returns false while pending,
true after completion, and latches terminal errors so consumed SO_ERROR cannot
later appear successful. `into_stream()` transfers the stream only after success;
otherwise the connection is destroyed normally. Connecting can be registered for
writable readiness, moved or dropped; there is no background operation.

## Boundary and tradeoffs

Public wrappers, mode/retry/connection policy live in Tarn. C bridges fcntl,
epoll, monotonic time, SO_COOKIE and SO_ERROR, plus mechanical token/registration
bookkeeping required by its opaque owner. Canonical Event and Poll layouts and
all private signatures are verified. Existing embedded-source authority remains
required. This does not add async, scheduler, io_uring, HTTP, TLS or optimization.

The v0 cookie strategy is Linux-specific and requires SO_COOKIE support. Registry
allocation is per registration, not per wait; closed records can retain memory
until reuse or Poll destruction. A future backend may place registry policy in
Tarn once collections and richer native-resource declarations are available.

## Validation

Real loopback/native ABI, exact close traces, /proc/self/fd leak checks,
fd-reuse/token-exhaustion stress, corrupted metadata, task producers, ordinary
loan contracts and mutation corpora validate this boundary. Test evidence and
remaining bootstrap limitations: [Phase 12B report](../readiness-report.md).
Linux contracts were checked against [connect(2)](https://man7.org/linux/man-pages/man2/connect.2.html),
[epoll(7)](https://man7.org/linux/man-pages/man7/epoll.7.html) and
[epoll_wait(2)](https://man7.org/linux/man-pages/man2/epoll_wait.2.html).
