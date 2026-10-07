# HTTP/1.1 server foundation

`import "http"` loads an ordinary Tarn module over net/io/time/runtime/string.
It is not trusted, has no intrinsics and can be overridden by a local module.
Design: [ADR 0046](adr/0046-bounded-async-http.md).
Runnable application: [http_server.tarn](../examples/http_server.tarn).

## Application API

| Type | Public API |
|---|---|
| Connection | new(stream, limits, timeouts), async read_request(owner), async write_response(owner, response), async reject(owner, status), consuming close() |
| Request | method(), target(), headers(), body(), trailers(), header(name), consuming into_body() |
| Header | new(name, bytes), text(name, text), name(), value(), value_text(), clone() |
| Response | bytes(status, owned_bytes), text(status, borrowed_text), consuming header(header), consuming close_connection(), status() |
| Limits / Timeouts | defaults(); public Copy configuration fields |
| Error | Io, Protocol, Limit, Timeout, InvalidConfig, InvalidResponse, InvalidText, InvalidState; status() |

Connection.new consumes a TcpStream and enables nonblocking mode; failure drops
it normally. Connection owns the stream and both buffers. It is non-Copy,
Transfer and not Share, structurally inheriting the socket's capabilities.
Request/Header/Response own their data, are non-Copy and have ordinary structural
Transfer/Share. All returned views borrow their owner. header(name) returns an
owned clone of the first matching header, not a stored optional reference.
Header.value_text validates UTF-8 and returns an owned copy; bodies remain bytes.

The owner argument must be the Execution driving the current async operation.
A mismatched owner follows the existing runtime nested-wake invariant check;
HTTP does not introduce cross-execution migration.

The application uses net.TcpListener.accept_async and moves each accepted stream
into an owned connection task. The example serves GET/HEAD /health, POST /echo,
and explicit 404/405 with Allow. It owns at most 128 task handles; admission waits
for an existing handle when full. Tasks run on one Execution, not a thread pool.
There is no generic server/router framework.

read_request returns Result<Option<Request>, http.Error>. None is clean EOF before
any new request bytes, or an ended connection; partial-message EOF is an error.
Each complete request requires a response before another read. Take the await
result into a binding before handling an error and calling reject: statement
cleanup then releases the read future's mutable loan.

```tarn
received := await conn.read_request(owner)
match received {
    Ok(result) => match result {
        Some(request) => {
            response := try http.Response.bytes(u16(200), request.into_body())
            try await conn.write_response(owner, response)
        }
        None => { return Ok(()) }
    }
    Err(error) => {
        match error {
            http.Error.Io(original) => { return Err(error) }
            _ => {}
        }
        try await conn.reject(owner, error.status())
        return Err(error)
    }
}
```

This fragment belongs in an async function returning Result<void, http.Error>.
There is no implicit io/http error conversion or new special main ABI. The
example explicitly handles HTTP errors inside connection tasks and retains the
existing Result<void, io.Error> entry point.

## Ownership and connection state

Normal move/drop transfers or discharges exactly-once socket close. close consumes
the connection, including on error. No user destructor, owner registry, backend
move query or public mutable stream/buffer projection exists. Buffered output is
never flushed on drop: write_response explicitly writes and flushes successfully.

Connection retains HEAD/close metadata independently of the returned Request.
Dropping/consuming a Request cannot change how its response is framed. Transactions
and pipelined requests execute sequentially and preserve prefetched bytes.
Protocol state prevents a second read before response, reuse after bad input,
or a second response after output failure. It is not socket ownership state.
A dropped pending read/write future leaves the connection failed; close/drop
still releases its ordinary live stream. Invalid response construction/preflight
has emitted no bytes, allowing a corrected response or rejection.

reject accepts only 400–599, writes one empty response with Connection: close
and ends the connection. It is allowed before transmission, including after
read failure. It cannot recover failed input or follow partial output.
Rejection is best effort: closing a socket with unread peer data may produce a
Linux TCP reset even after the error response. Do not drain hostile input merely
to promise graceful EOF. Transport failure and write timeout close without a
second response.

## Protocol subset

Only server-side HTTP/1.1 is implemented. Require exact CRLF, token methods,
origin-form targets or OPTIONS *, valid ASCII URI characters/percent escapes,
and exactly one Host. Host accepts lexical hostname labels, strict IPv4 or
bracketed IPv6 and an optional port 0–65535. No DNS occurs during head parsing.
Numeric IPv4 components reject leading zeros; trailing-dot DNS names, IPvFuture,
IPv6 zones, userinfo and percent-encoded hosts are outside this initial subset.

Header names normalize ASCII case. Trim only surrounding SP/HTAB from values;
keep allowed opaque bytes and duplicates in order. Reject controls/DEL, obs-fold,
whitespace before a colon and framing/routing/auth/content-critical Connection
nominations. Reject Upgrade/CONNECT/unsupported coding with 501, Expect with 417
before body waiting, unsupported valid versions with 505 and malformed input with
400. No interim 100-continue, proxy forms or HTTP/1.0 fallback.

Requests without length/coding have empty bodies. Content-Length is one checked
decimal field; reject duplicates even when identical, comma lists, signs and
overflow. TE+CL is invalid. Transfer-Encoding supports exactly one chunked token;
multiple chunked tokens are invalid regardless of spacing/case. Other bare coding
tokens/chains get 501; parameters/malformed coding syntax get 400 in this subset.
Chunk sizes are checked hex; chunk extensions are validated and ignored. Require
all data terminators and the final zero chunk. Store bounded trailers separately;
never let them override the head. Validate Trailer declarations and retain safe
undeclared extension trailers. Forbidden trailer names include framing, routing,
authentication and content-interpretation fields listed in the plan.

Responses accept final 200–599 statuses. Header builders reject framing/control
fields and injection. Generate Content-Length; no outbound chunking/trailers.
HEAD sends no body and advertises representation length. 204/304 require empty
bodies and omit length/coding; 205 requires empty body and length zero. Empty
reason phrases are permitted for unlisted statuses. Preflight checks the complete
response before any output, including header/body budgets. Requests asking to
close, explicit response preference and the request-count cap end persistence.

## Limits and errors

Defaults: request/individual lines 8 KiB, aggregate head 32 KiB/100 fields, request
and response bodies 1 MiB, response head 32 KiB/100 fields, chunk metadata including
trailers 64 KiB, 4096 data chunks/20 trailers, 8 KiB reader/writer and 100 requests
per connection. Configurations require positive budgets; head covers request-line
and header-line caps, chunk metadata covers its line cap, and response head is at
least 64 bytes. Byte capacities are bounded at 1 GiB to keep allocation arithmetic
representable. These are bounds, not guarantees that allocation cannot fail.

Whole-stage monotonic deadlines: head including idle 10 s, body including chunks
and trailers 30 s, write plus flush 10 s. No per-byte reset; no CPU/handler
preemption. Durations must be 1–4294967295 milliseconds. There is no infinite
HTTP deadline. Allocation exhaustion retains existing abort semantics.

`runtime.run_timeout(operation, duration)` consumes an
Operation<Result<void, io.Error>> and returns Result<bool, io.Error>: true means
complete, false means timeout. Completion wins ties. Owned stage results are
written through ordinary output loans; completion/abandonment releases those
loans before access. This deliberately does not consume arbitrary generic-result
pollers, which may return a reference into their own storage. No borrowing rules,
scoped tasks, select API or cancellation facility were added.

Error.status gives a candidate rejection status, not permission to write another
response. Parsing maps to 400/501/505/417; read deadline 408; body/chunk metadata
413; start-line 414; header/trailer line/count/head 431; invalid application
response 500. I/O errors preserve io.Error and must be handled explicitly.
Connection state and transport viability decide whether rejection is possible.

## Deferred work

No client, TLS, HTTP/2/3, WebSocket, compression, multipart, streaming application
bodies, routing/middleware framework, cookie/auth framework, scoped async tasks,
generic async I/O, thread pool, cancellation or platform expansion. This is a
bounded server subset, not a blanket RFC-conformance claim.
