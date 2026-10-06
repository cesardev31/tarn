# Blocking networking (Phase 12A)

`import "net"` loads the embedded `stdlib/net/net.tarn`. The current target is
Linux x86_64. All operations, including DNS, may block the calling native task.
The API uses ordinary Tarn ownership, `Result`, `try` and borrowed byte slices.

```tarn
import "net"

fn main() Result<void, net.Error> {
    var listener = try net.listen_tcp(&"127.0.0.1:8080")
    var stream = try listener.accept()
    var buffer = [8]u8{0, 0, 0, 0, 0, 0, 0, 0}
    count := try stream.read(&mut buffer)
    try stream.write_all(&buffer[0..count])
    return Ok(())
}
```

The binding must be `var` for a mutable receiver. String arguments are borrowed
explicitly, and successful `Result` returns use `Ok(())`. Repeated array literals
are not part of the current syntax; examples use explicit initializers. Passing
`&buffer` or `&mut buffer` to slice parameters safely coerces the array reference.

## Public surface

All fallible operations return `Result<..., net.Error>`.

| Type | Operations | Receiver/result |
| --- | --- | --- |
| `TcpListener` | `bind(&string)`, `bind_addr(SocketAddr)` | owned listener |
| | `accept()` | `&mut self`, owned `TcpStream` |
| | `local_addr()` | `&self`, `SocketAddr` |
| | `close()` | consumes `self`, `void` |
| `TcpStream` | `connect(&string)`, `connect_addr(SocketAddr)` | owned stream |
| | `read(&mut []u8)`, `write(&[]u8)` | `&mut self`, `usize` |
| | `write_all(&[]u8)` | `&mut self`, `void` |
| | `local_addr()`, `peer_addr()` | `&self`, `SocketAddr` |
| | `shutdown(Shutdown)` | `&mut self`, `void` |
| | `close()` | consumes `self`, `void` |
| `UdpSocket` | `bind(&string)`, `bind_addr(SocketAddr)` | owned socket |
| | `recv_from(&mut []u8)` | `&mut self`, `RecvFrom { count, peer }` |
| | `send_to(&[]u8, SocketAddr)` | `&mut self`, `usize` |
| | `local_addr()` | `&self`, `SocketAddr` |
| | `close()` | consumes `self`, `void` |

`net.listen_tcp(&string)` is a small convenience for `TcpListener.bind`.
`net.resolve(&string)` returns a `SocketAddr`. The Copy address exposes `port()`,
`ip()`, `is_ipv4()`, `is_ipv6()` and `with_port(u16)`. `IpAddr` is a Copy enum with
`V4([4]u8)` and `V6([16]u8)` variants. `Shutdown` is `Read`, `Write` or `Both`.
Descriptors, address bytes and native error domain/code fields are private.

## Ownership and concurrency

Each socket owner is non-Copy and explicitly Transfer, without Share. Moving it
transfers the close responsibility; use after move, double consuming close,
move/overwrite during a live borrow and sharing it directly across tasks are
compile-time errors. I/O uses `&mut self`; address queries use `&self`.
`Mutex<TcpStream>` can serialize shared access using Phase 11's existing rules.

Automatic close follows verified post-drop IR on normal scope exit, return,
break, continue, overwrite, conditional initialization and `try` error exits.
Explicit `close` consumes its owner even if the OS returns an error. Linux close
releases a valid descriptor on EINTR, so it must never be retried. Drop discards
ordinary close errors; explicit close reports them. EBADF during destruction is
an impossible live-owner invariant and aborts. The runtime neither tracks owner
liveness nor consults the move checker. Native tasks still join on destruction.

TCP shutdown changes communication directions and preserves the owner. Reading
into a nonempty buffer returns zero for EOF. An empty TCP read returns zero
immediately and is not evidence of EOF. `write` returns the actual partial count;
`write_all` retries until complete or error and reports `WriteZero` on zero progress
with remaining bytes. An empty `write_all` succeeds without a syscall.

UDP is datagram-oriented: zero means a valid empty datagram, not EOF. Receives
truncate oversized datagrams to the buffer and discard the excess. A zero-capacity
receive still consumes one datagram. No borrowed runtime storage escapes.

## Resolution and errors

Endpoints use `host:port`, `IPv4:port` or `[IPv6]:port`. Ports are decimal numeric
values 0–65535; port zero supports ephemeral binding. An empty host (`:port`)
resolves a wildcard address. Embedded NULs, missing ports and malformed endpoints
produce `InvalidAddress`. IPv6 scope IDs resolved by getaddrinfo remain encoded.

Blocking getaddrinfo selects one IPv4-preferred candidate, otherwise IPv6. This
API deliberately returns one address, not a candidate collection. Applications
can use explicit numeric IPv6 endpoints or `connect_addr`; connection fallback
across DNS candidates is not implemented. Tests resolve `localhost` without
internet. Address display/formatting and public scope-ID access are deferred.

`Error.kind` distinguishes `AddressInUse`, `ConnectionRefused`, `ConnectionReset`,
`BrokenPipe`, `TimedOut`, `WouldBlock`, `InvalidAddress`, `DnsFailure`, `OtherOs`,
and `WriteZero`. `native_code()` and `is_dns_code()` preserve diagnostic access
to the original errno or resolver code. There is no public errno-based API or
implicit error conversion in `try`. Network errors do not abort. A main returning
`Result<void, net.Error>` prints the error category/native code and exits with
status 1 on failure, or 0 on success.

Safe EINTR retries live in Tarn: resolution, socket creation, bind/listen,
accept, reads/writes, address queries, shutdown and UDP I/O. Interrupted connect
drops the incomplete connection and retries using a fresh descriptor because its
state is unspecified. Interrupted close is reported and never retried. EAGAIN and
EWOULDBLOCK map to WouldBlock even though 12A sockets are blocking. All sends use
Linux `MSG_NOSIGNAL`, returning an error instead of accidental SIGPIPE termination.

## Private bridge and examples

C handles syscall ABI conversion, endpoint parsing for getaddrinfo, native status
capture and address encoding. Public wrappers, retries, error categories,
write_all, owned construction and resource cleanup remain in Tarn/verified IR.
Trusted intrinsic signatures and layouts are structurally verified. A local
`net.tarn` cannot override this embedded module or gain intrinsic authority.

The fixed private outcome has `i32 domain`, `i32 code`, `i64 value` and a 24-byte
address; slice pointers/lengths are explicit ABI lanes. Native declarations are
bootstrap compiler resources, not general user-defined destructors. See
[ADR 0034](adr/0034-blocking-networking.md) for the boundary and bootstrap debt.

[`tcp_echo.tarn`](../examples/tcp_echo.tarn) serves one connection until EOF.
[`25_concurrency.tarn`](../examples/25_concurrency.tarn) moves an accepted
connection into a native task and joins it. Both listen on loopback port 8080.

12A includes no nonblocking flags, async, reactor, scheduler, thread pool,
cancellation, HTTP, TLS, channels or sophisticated timeouts. 12B remains deferred.
