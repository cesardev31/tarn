# Phase 12A report: owned blocking networking

Phase 12A is complete within the documented Linux x86_64 v0 subset. Phase 12B has
not started. No async/await, futures, reactor, epoll, scheduler, thread pool,
HTTP, TLS, channels, cancellation or optimization was added.

## Public API implemented

The embedded `stdlib/net/net.tarn` declares TcpListener, TcpStream, UdpSocket,
SocketAddr, IpAddr, Error/ErrorKind, Shutdown and RecvFrom. Public networking lives
outside core. The exact signatures and a usable application example are in
[networking.md](networking.md).

Listeners bind/accept/query local addresses. Streams connect, read, write,
write_all, query both addresses and shutdown directions. UDP binds, sends and
receives with peer addresses. Every owner has consuming close. Applications use
ordinary Result/try, byte arrays and slices, not syscall casts or manual errno.
Main may return Result<void, net.Error> with normal failure exit status 1.

The existing syntax requires explicit borrows for string inputs, mutable
bindings for mutable receivers, explicit array initialization and Ok(()) for a
successful Result<void, Error>. These are documented language ergonomics limits;
no new buffer framework or special ownership syntax was introduced.

## Socket ownership model

Each owner privately contains an i32 descriptor, has no Copy capability, and
uses ordinary move/init checking. A move transfers close responsibility. Resource
destruction is compiler-backed bootstrap glue executing verified post-drop IR,
not a runtime owner registry or backend query of loans/moves. Explicit close
consumes even on failure. Linux close EINTR is never retried. Normal drop ignores
recoverable close errors; impossible EBADF during live-owner cleanup aborts.

Close coverage includes normal exit, return, break, continue, nested scopes,
multiple local moves, overwrite, self-assignment, conditional initialization and
try error propagation. The owner is installed immediately after acquisition;
bind/listen/connect failures destroy it. No valid borrow survives an owner move,
overwrite or scope destruction. Owned resources returned through Result/tasks
retain their ordinary destruction plans.

## Error model

Public ErrorKind has ten small categories: AddressInUse, ConnectionRefused,
ConnectionReset, BrokenPipe, TimedOut, WouldBlock, InvalidAddress, DnsFailure,
OtherOs and WriteZero. Native errno/EAI domain/code remain private with diagnostic
accessors. Mapping lives in Tarn; the native bridge only captures status.
Normal network faults return Result and never abort. Tests inject all represented
native error categories and DNS failure without internet.

Read returns the actual byte count; zero on a nonempty TCP read means EOF. Empty
TCP reads return immediately and are not evidence of EOF. Write exposes partial
counts. Tarn write_all repeats until complete/error and reports WriteZero for zero
progress with remaining bytes. UDP zero means an empty datagram; oversized packets
truncate/discard excess, and zero-capacity receives consume one packet.

## Transfer and Share

All three native owners explicitly have Transfer and not Share, independently of
Copy and their scalar descriptor representation. I/O and TCP shutdown use &mut
self; address queries use &self. Cross-task ownership transfer is safe, while
direct shared socket captures are rejected. Mutex<Socket> can serialize shared
access under Phase 11's existing Transfer-based rule. No second concurrency or
socket lifetime checker was introduced.

## Address resolution

Blocking getaddrinfo supports numeric IPv4, bracketed IPv6, host:port, localhost,
wildcard empty hosts and numeric ports 0–65535. Port zero enables race-free native
fixtures by querying the bound owner's local address. Invalid syntax/NULs never
escape as sockaddr. Copy SocketAddr has private 24-byte normalized storage,
including IPv6 scope metadata; IpAddr exposes family-specific bytes.

Resolution returns one IPv4-preferred candidate, otherwise IPv6. Explicit
connect_addr/bind_addr avoid rebuilding endpoint text. Candidate collections and
automatic fallback, formatting and public scope-ID access remain deferred and
are documented rather than hidden behind a large new abstraction.

## Runtime/FFI boundary

Private C wrappers perform socket ABI calls, sockaddr conversion, endpoint
parsing for getaddrinfo and native status capture. Public wrappers, error mapping,
retry loops, write_all, owner construction and cleanup remain in Tarn/verified IR.
General extern C execution is not yet a usable backend replacement for these
small declared intrinsic bridges.

The private outcome layout is 40 bytes/alignment 8: i32 domain/code, i64 value,
24-byte address. Slice pointer/length lanes are explicit. C static assertions and
IR structural validation check layouts, signatures, private fd owners, explicit
capabilities, error tags and operand ownership modes. Copying a consuming socket
or mutable buffer reference in forged intrinsic metadata is rejected.

Intrinsic authority records embedded-source provenance. Local core/net files,
entry filenames or overlays cannot manufacture authority or override bundled
contracts. A reserved entry filename can still import the actual stdlib.
The raw successful fd is briefly Copy inside trusted bootstrap code; its unique
wrapping is an audited native/stdlib contract, not a newly invented resource checker.

Safe EINTR retries execute in Tarn. Interrupted connect discards its incomplete
fd and retries with a fresh owner; close never retries. EAGAIN/EWOULDBLOCK are
represented as WouldBlock without implementing nonblocking loops. Sends use
MSG_NOSIGNAL. Tests restore default fatal SIGPIPE to verify the actual strategy.

## Integration with tasks

Native TCP fixtures move accepted connections into owned workers and return
Result values through join. Further tests return TcpStream/UDP ownership from
tasks into the caller. Scoped mutable references and Mutex serialization pass
canonical capability/loan checking; shared direct socket references fail.

The shipped tcp_echo example echoes one connection until EOF. The updated
25_concurrency example transfers an accepted connection to a native task and
joins it. A native test shares Mutex<UdpSocket> across two scoped workers,
verifies all 64 datagrams and closes the owned payload exactly once. External clients exercise both examples with 16 KiB payloads. Blocking
I/O blocks the current pthread; task destruction continues to join normally.

## Tests and mutation testing

Validation: cargo test --workspace --locked -j 4 passed 128 tests, zero failures,
with one explicitly ignored host-dependent benchmark. All previous Phase 11 and
older suites remain green. cargo build/check --workspace, strict C warnings and
the real stdio LSP smoke test also pass.

Sixteen dedicated networking tests and three native pass fixtures cover TCP,
UDP, IPv4/IPv6, DNS localhost, actual read/write counts, partial/zero writes, EOF,
empty reads, empty/truncated/zero-capacity datagrams, peer/local addresses, all
shutdown directions, errors, task ownership, example programs and destruction.
IPv6 ran on this environment; its loopback test only skips when infrastructure
cannot bind ::1. No correctness assertion relies on sleep or internet.

Linker wrappers inject EINTR into resolution, socket, bind/listen, accept,
connect, recv/send, UDP I/O, address queries and shutdown; they inject close EINTR
after consuming the descriptor. Native code remains the real syscall target after
injection. Descriptor traces balance actual opens/closes. A private process fd
audit independently detects leaked descriptors before process exit. Drop mutants
with missing close fail the oracle/audit; duplicate close aborts. Removing
MSG_NOSIGNAL makes the restored-default-SIGPIPE mutant die with signal 13.

Canonical memory_safety adds fifteen safe/rejected networking cases: moves,
consuming close, original use after spawn, live socket loans, buffer aliases and
escaping buffer references, fd/privacy bypass, capabilities, returned task owners,
scoped mutable transfer, Mutex serialization and slice coercions. Networking
fixtures and examples participate in frontend mutations; native line-deletion
mutations emit actual verified objects. Malformed intrinsic metadata is rejected
without compiler panics or malformed code.

## Bugs found

1. Method argument materialization used the expression's original type after
   array-to-slice coercion. Native code saw a thin/fat reference mismatch. The
   lowering now uses the actual materialized operand's type; read/write fixtures
   and canonical safety cases exercise this regression.
2. Name-based intrinsic authority was insufficient for a trusted net module and
   could also let a user core filename impersonate lang items. Embedded-source
   provenance, reserved-entry separation and imported core/net precedence close
   that boundary; dedicated negative loader tests cover it.
3. Concurrent prints made a SIGPIPE test's output assertion intermittent. The
   worker now returns its checked result and the main task prints after join.
4. Rust's test parent can inherit an ignored SIGPIPE disposition into C children,
   weakening a naive test. The private helper restores SIG_DFL and a strategy
   removal mutant proves the test detects a fatal signal.
5. Reserving/closing a port before launching an external echo example introduced
   an unnecessary race. Test-only listen readiness now reports the actual port
   zero binding; clients connect only after successful native listen.
6. A close event logged after the syscall could appear after another task had
   already reused the descriptor, producing a false duplicate-owner trace. The
   test event now precedes the release attempt, while the syscall remains the
   actual release. A private two-barrier native test forces fd reuse before close
   returns and verifies open/close/open/close ordering under UBSan.

## Decisions I would defend

Non-Copy owners with consuming close; explicit Transfer/not-Share contracts and
mutable I/O receivers; safe ordinary byte slices; Result errors with native
context; partial basic writes and Tarn write_all; EOF/datagram distinctions;
MSG_NOSIGNAL instead of process-wide signal policy; no retry of Linux close;
fresh-owner interrupted connect; structural intrinsic verification; no backend
ownership inference; trusted stdlib provenance; and loopback/fault/mutation
oracles using real native executables.

## Decisions I still question

One-address IPv4-preferred DNS resolution is intentionally small but misses
candidate fallback. Not-Share serializes more than Linux strictly requires,
though it gives a clear first API. Address formatting/scope-ID access will matter
for applications. Native bootstrap resource declarations and brief raw fd
outcomes still depend on an audited trusted implementation; a reusable safe FFI
resource contract would eventually reduce that compiler-specific debt. Drop
cannot report OS close errors; applications needing them must consume close.

## What 12B must resolve

Before implementation: nonblocking ownership/state transitions, preserving
partial I/O progress across WouldBlock, readiness/retry semantics, EOF/shutdown
behavior with pending operations, address-candidate policy and native ABI
contracts. Mutable buffer loans must survive any future deferred I/O until
completion; no runtime backend may infer ownership. The existing blocking API
and native task regressions must remain intact. Reactor/async integration,
cancellation and scheduling require their own approved model rather than being
smuggled into networking.

ADR 0034 is accepted for this constrained blocking model. Phase 12B remains
unstarted and requires separate authorization.
