# ADR 0034: owned blocking sockets

Status: accepted. Phase 12A implements and validates this blocking Linux v0
model. This ADR authorizes no Phase 12B/async work.

## Decision

Networking lives in the imported, embedded stdlib/net module. Core gains no
application networking types. The driver distinguishes trusted embedded source
from local source; a local net.tarn must not acquire intrinsic authority by name.
Private net intrinsics bridge Linux syscalls. Public APIs, error mapping, safe
retry policy, write_all and owned fd construction live in ordinary Tarn.

TcpListener, TcpStream and UdpSocket are non-Copy owned descriptors. Moves and
post-drop destruction are authoritative. Explicit close consumes the owner and
returns Result<void, Error>; drop closes once without propagating ordinary close
errors. Linux close releases a valid descriptor even on EINTR: never retry it.
No fd duplication, shared owner, ownership registry or user Drop is introduced.

All three owners explicitly have Transfer and not Share. Accept, stream I/O,
shutdown and UDP I/O require &mut self; address queries use &self. This deliberately
serializes each owner's application operations; safe shared serialization can use
Mutex. A moved accepted connection may execute on another native task.

SocketAddr is a Copy value with private, validated address bytes, not sockaddr.
IpAddr distinguishes IPv4 and IPv6. Numeric ports and bracketed IPv6 endpoints
are supported. Blocking getaddrinfo returns one IPv4-preferred address, otherwise
IPv6; full candidate iteration is deferred. connect_addr/bind_addr make the chosen
endpoint explicit and allow ephemeral port tests without text formatting.

Read returns Result<usize, Error>; zero denotes EOF only for a nonempty TCP read.
A zero-length TCP buffer returns zero immediately. Write returns the actual partial
count. Tarn write_all loops until complete, returning WriteZero if a nonempty
write makes no progress. UDP recv_from returns a Copy record with count and peer;
zero is a valid empty datagram. Oversized datagrams are truncated to the buffer.

Error has a useful public ErrorKind and private original native domain/code.
Categories are AddressInUse, ConnectionRefused, ConnectionReset, BrokenPipe,
TimedOut, WouldBlock, InvalidAddress, DnsFailure, OtherOs and WriteZero. Normal
network faults return Result, never abort. Linux errno/EAI mappings live in Tarn;
C only captures syscall/resolver status. Blocking operations retry EINTR in Tarn
where safe. Interrupted connect discards its descriptor and retries with a fresh
socket because its state is unspecified; close is never retried. Socket syscalls
use MSG_NOSIGNAL, preventing accidental SIGPIPE termination.

Shutdown Read/Write/Both changes TCP directions while preserving ownership.
No nonblocking flags, reactor, async, epoll, scheduler, pool, HTTP, TLS, extensive
socket options or sophisticated timeouts are part of this decision.

## Native boundary and validation

A fixed private raw outcome contains native domain/code, scalar result and a
24-byte address encoding. Structural declaration/signature/layout checks validate
that ABI. Syscalls receive explicit slice address/length lanes; no borrowed runtime
storage is returned. Post-drop native glue closes only initialized owned values.
The backend never queries moves, loans or provenance. Intrinsics must be keyed to
trusted resolved declarations rather than an unvalidated module/type name.

Normal main remains supported; main returning Result<void, net.Error> reports an
error and returns exit status 1 rather than aborting, allowing application-level try.

Acceptance requires real IPv4/IPv6 loopback TCP/UDP, DNS via localhost, tasks,
partial/zero-write and EINTR fault injection, SIGPIPE protection, fd cleanup traces,
ABI corruption, ownership rejections and the complete Phase 11 regressions.

## Bootstrap debt and constrained choices

Private raw outcomes briefly carry a Copy numeric fd inside trusted net code;
each successful acquisition is wrapped in exactly one non-Copy owner before any
fallible follow-up. User source cannot access those intrinsics or fields. The
compiler does not prove raw C acquisition correctness: trusted bridge/library
contracts are audited and exercised by native tests. Opaque-resource close glue
remains a compiler bootstrap rule, not user Drop. General extern C execution is
still unsupported; getaddrinfo's pointer-rich ABI additionally needs safe address
normalization, so a narrow declared intrinsic bridge is the current choice.

Core and net always use bundled declarations for imports. Reserved entry
filenames remain untrusted entry modules and can import the real stdlib. No user
file or editor overlay gains authority merely by being named core/net.

One-address DNS selection, address formatting/scope-ID access, and stronger
reusable native-resource contracts remain deliberate limitations. They do not
weaken socket ownership, slice borrowing or explicit errors. Operations can block
indefinitely, and dropping tasks still joins; there is no implicit scheduler.

## Evidence

The complete workspace regression passed: 128 tests, zero failures, one explicitly
ignored host-dependent benchmark. Sixteen dedicated networking tests plus three
native fixtures cover real IPv4/IPv6 loopback, localhost resolution, TCP EOF and
shutdown, UDP empty/truncated/zero-capacity payloads, address queries, accepted and
returned task-owned sockets, and external echo clients with 16 KiB payloads.

Private linker fault injection verifies EINTR, fresh-fd connect retries, partial
writes, zero progress, native error categories and DNS faults. Default fatal
SIGPIPE is restored in the child: removing MSG_NOSIGNAL kills the test mutant.
Exact descriptor traces and a private /proc/self/fd audit detect omitted closes;
duplicate close aborts on the impossible EBADF invariant. Canonical safety cases,
structural ABI mutations and both frontend/native mutation corpora remain green.
All Phase 11 synchronization/capability/stress regressions pass unchanged.
