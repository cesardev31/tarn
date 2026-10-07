# Phase 16 implementation plan: bounded async HTTP/1.1

Status: complete within the documented server subset; all six stage gates passed.
Final API: [HTTP](http.md). Implementation evidence: [report](phase-16-report.md).
Design decisions: [ADR 0046](adr/0046-bounded-async-http.md).
Baseline: Phase 15D, commit `f262308`, 204 workspace tests passed, no warnings,
one existing benchmark ignored. Final Phase 16 validation is recorded in the report.

## 1. Goal and scope

Build an application-level HTTP/1.1 server foundation in ordinary Tarn over
existing nonblocking TCP, buffered I/O, async frames, Execution and timers.
A connection must parse complete bounded requests, send valid responses,
reuse its buffers safely and release all resources on success/error/timeout.

Implement the server-side request decoder and response encoder. There is no
public HTTP client in Phase 16; native test peers can generate raw requests and
independently inspect responses. This is a deliberate HTTP/1.1 subset, not a
claim of implementing every optional HTTP feature.

Included:

- HTTP/1.1 request lines, headers and binary bodies;
- Content-Length and incoming chunked framing, including bounded trailers;
- owned Request, Response and Header data;
- async Connection over one owned TcpStream plus both buffers;
- sequential keep-alive and sequential processing of pipelined requests;
- correct HEAD and body-forbidden response semantics;
- explicit protocol/I/O/limit/timeout/application-construction errors;
- header/body/write deadlines and bounded example-server admission;
- native interoperability, fragmentation, ownership, cleanup and mutation tests.

Excluded: HTTP client, HTTP/1.0 compatibility, HTTP/2/3, TLS, WebSocket, CONNECT,
protocol upgrades, compression, multipart, streaming application bodies,
response chunking/trailers, routing framework, middleware, JSON, URL-decoding
framework, cookies/auth framework, connection pools, signals/graceful-shutdown
framework, scoped async tasks, generic async I/O interfaces, new scheduler,
thread pool, cancellation API, package manager, optimizer and platform expansion.
Do not start Phase 17 or broaden this phase to satisfy an optional feature.

## 2. Current implementation constraints

BufferedReader/Writer are TCP-specific and live in net. The module split was
completed in 14R; it must not be repeated based on the older 14C report.
References stored in ordinary ADTs remain forbidden. Spawned async tasks may
borrow Execution only. Existing join_timeout consumes a spawned task; it cannot
be applied to a task borrowing a local Connection under current rules.

Use owned connection tasks and local direct awaits for request handlers.
Do not add scoped async tasks or a lifetime checker for HTTP. Keep buffers in
net until a future generic async I/O phase justifies moving them.

The implementation gate resolved a concrete issue with the initially proposed
`Operation<R>.run_timeout`: an arbitrary manual poller may return a borrow of
its own storage. Consuming it cannot safely return every possible R under the
current generic model. The final prerequisite is ordinary Tarn
`runtime.run_timeout(operation Operation<Result<void, io.Error>>, duration)`
returning `Result<bool, io.Error>` (true completed, false timed out). Stages
write owned results through ordinary borrowed caller output slots. This avoids
new capability bounds or changes to provenance; native tests prove loan release,
completion priority and abandonment. Timer errors remain io.Error.

This helper is the first implementation gate. Prove loan preservation/release,
frame cleanup and timer/Waker deregistration before using it in HTTP. Fix only
concrete compiler bugs with regressions; do not weaken provenance or E4209.
No new intrinsic, native timer ABI or general select/cancellation abstraction
is planned. If existing semantics cannot support the helper safely, report the
concrete blocker before changing the ownership model.

## 3. Module and API

Add `stdlib/http/http.tarn` as an ordinary, untrusted bundled module, with the
same local-module fallback policy as string/path. Import public io, string,
net, time and runtime APIs. Do not grant http intrinsic/private stdlib access.
Recognize the official source in editor analysis without granting native trust.
Core and the native HTTP ABI remain unchanged: there is no native HTTP ABI.

Implemented public shapes (exact method spellings are documented in HTTP):

| Type | Ownership and application API |
|---|---|
| Header | Private owned lowercase ASCII name and opaque `Vec<u8>` value; validated byte/text constructors, name/value loans, strict owned value_text conversion. |
| Request | Private owned method, raw target, headers, body and trailers; immutable getters, explicit owned body extraction, first-header owned clone. |
| Response | Owned status/body/headers; text and byte constructors, consuming header builders and explicit close_connection preference. |
| Connection | Owned stream, reader, writer, configuration and private protocol state; new, read_request, write_response, reject and consuming close. |
| Limits | Copy numeric configuration with defaults and constructor validation. |
| Timeouts | Copy header/body/write Durations, validated as finite positive spans. |
| Error | Copy enum distinguishing I/O, protocol, limits, timeout stage, invalid configuration/response/text and invalid connection state. |

Core connection contracts (implemented):

- `Connection.new(stream, limits, timeouts) -> Result<Connection, Error>`:
  consumes the stream, validates configuration and enables nonblocking mode;
  failure drops the transferred stream normally.
- `read_request(&mut self, owner &Execution)` async:
  `Result<Option<Request>, Error>`. None means clean EOF before any new request
  bytes. Partial-message EOF is an error. A successful request is fully owned.
- `write_response(&mut self, owner &Execution, response Response)` async:
  `Result<void, Error>`. Consumes the response; validates it before emitting
  bytes and flushes explicitly within its write deadline.
- `reject(&mut self, owner &Execution, status u16)` async:
  one empty final error response (status 400–599) with Connection: close. Only permitted before
  response transmission has begun, including after a request-reading failure.
- `close(self) -> Result<void, Error>`: consumes and closes the stream;
  buffered bytes are discarded, never implicitly flushed.

Connection caches HEAD/close/framing metadata from its own parsed transaction.
Writing a response does not trust a caller-supplied Request, so a consumed,
modified or unrelated request cannot alter framing decisions.
Header/Request references borrow their owners; optional lookup returns an owned
Header clone, not Option of a stored reference. No raw mutable stream/buffer
projection is public. Payload bytes are not automatically decoded as text.

Normal application shape: accept a stream, construct Connection, repeatedly
read an owned request, call an application handler, write an owned response,
and drop/close when the transaction or connection ends. Example handlers stay
simple synchronous functions; direct awaited handlers are possible without
spawning borrowed local data. No generic Server.serve(handler) framework is
required for the phase.

## 4. Request head policy

Use byte scanning before text conversion. Accept only CRLF line endings and
exact HTTP/1.1 syntax. Do not use Unicode whitespace or lossy decoding.

- Method: nonempty HTTP token; preserve case and extension methods.
- Target: origin-form starting with `/`, or `*` for OPTIONS. Preserve encoded
  path/query, validate percent escapes, reject fragments/control/space bytes.
  Non-ASCII target bytes require percent encoding. No automatic decoding.
- CONNECT, authority-form and proxy absolute-form are outside the subset.
  Recognized CONNECT is rejected with 501; other invalid forms get 400.
- Syntactically valid unsupported versions receive 505; malformed versions
  receive 400. No HTTP/1.0 fallback.
- Require exactly one nonempty Host. Validate supported DNS/IPv4/bracketed IPv6
  authority and optional decimal port lexically, without DNS lookup. Support
  normal hostname labels, IPv4 and bracketed IPv6; IPvFuture, zone identifiers,
  userinfo and percent-encoded host names are outside v0 and are rejected.
- Header names are nonempty ASCII tokens and normalize to lowercase.
- Header values retain bytes, including permitted obs-text. Strip surrounding
  SP/HTAB only; reject NUL, DEL and other controls, embedded CR/LF, whitespace
  before the colon and obsolete line folding.
- Preserve repeated ordinary fields and their order. Do not flatten duplicates
  indiscriminately or merge trailers with primary headers.
- Parse Connection tokens case-insensitively across fields. `close` wins.
  Reject framing/routing-critical hop-by-hop nominations.
- Upgrade requests are rejected, closed and never passed to a different protocol.
- Reject any Expect with 417 before waiting for the body. There is no interim
  100-continue support, avoiding the corresponding client/server deadlock.

Errors should retain useful structured distinctions rather than one catch-all
bad-request branch. Unit tables cover every delimiter, token and boundary.

## 5. Body and framing policy

Determine framing once from the validated head, before allocation/body reads.

- No Content-Length or Transfer-Encoding: empty request body; never use EOF to
  delimit a request. Preserve buffered bytes for the next request.
- Content-Length: exactly one field, decimal digits only, checked parsing.
  Leading zeros are accepted. Reject signs, comma lists, duplicate fields
  (even identical values), overflow and lengths above configured limits.
- Both Transfer-Encoding and Content-Length: reject 400 and close.
- Transfer-Encoding: one field and a single case-insensitive chunked coding
  without parameters. Coding parameters/malformed coding syntax are rejected
  with 400 in the v0 subset. Repeated chunked is invalid; other coding combinations
  are unsupported (501), with no fallback to length or close-delimited input.
- Chunk sizes: checked hexadecimal parsing, exact CRLF after each chunk and
  terminal zero chunk. Bound metadata and chunk count as well as decoded bytes.
- Validate bounded token/quoted-string chunk-extension syntax, then ignore its
  content. Never let extensions affect routing or framing.
- Parse bounded trailers into a separate owned vector. Reject forbidden
  framing/routing/auth/content-interpretation trailers, including Host,
  Content-Length, Transfer-Encoding, Connection, Trailer, Expect, Upgrade,
  Authorization, Proxy-Authorization, Cookie, Content-Type, Content-Encoding
  and Content-Range. Never override the original headers. Validate Trailer
  declarations; safe undeclared extension trailers may be retained separately.
- Decoded bodies remain arbitrary bytes, including NUL/invalid UTF-8.
- EOF before the declared body or chunk terminator is incomplete-message error.

Do not guess ambiguous framing to recover a connection. Any parsing, framing,
limit or read-timeout failure makes it non-reusable. The server rejects when
safe and then drops it; low-level callers retain ownership until close/drop.

## 6. Response policy

Validate the complete outbound response before passing bytes to BufferedWriter.
Accept final status codes 200–599; informational responses and 101 are excluded.
Use known reason phrases or a valid empty reason phrase for unlisted codes.

- Generate exactly one Content-Length from owned body bytes, except where
  forbidden. Users cannot supply Content-Length, Transfer-Encoding, Connection,
  Trailer or Upgrade through general header builders.
- Reject invalid names/value controls/CRLF injection and excessive output
  headers/body. Preserve validated byte values without lossy transformation.
- Ordinary responses send exactly the declared bytes.
- HEAD sends no body but advertises the length of the selected representation.
- 204 and 304 reject a supplied nonempty body and omit Content-Length/TE.
- 205 requires an empty body and sends Content-Length: 0.
- Connection close is generated from request policy, response preference or
  request-count limit. Otherwise HTTP/1.1 persistence is the default.
- Completion means explicit flush succeeded, not merely bytes were buffered.
- On partial write/error/timeout, discard pending output through normal drop
  and close. Do not restart serialization or send a second response.

## 7. Connection state and deadlines

Private state: awaiting request, ready to respond, transmitting, ended, failed
before response, failed during response. Only the correct next operation is
permitted. State is protocol bookkeeping, never an alternative socket-owner bit.
The descriptor remains an ordinary owned resource in every protocol state.

One task processes each connection sequentially. Request pipelining may place
multiple messages in the reader, but replies preserve request order and there
are no parallel handlers on one socket. A new request is not read while the
previous response remains unresolved. Early EOF with no partial request is
normal; half-close after a complete request may still receive its response.

Defaults, adjustable within checked configuration:

| Budget | Default |
|---|---:|
| Request-line bytes, including CRLF | 8 KiB |
| Individual header/trailer/chunk-metadata line | 8 KiB |
| Aggregate request head, including start line/terminators | 32 KiB |
| Main header fields | 100 |
| Decoded request body | 1 MiB |
| Response body | 1 MiB |
| Response head / fields | 32 KiB / 100 |
| Aggregate chunk metadata including trailers | 64 KiB |
| Data chunks / trailer fields | 4096 / 20 |
| Reader / writer capacity | 8 KiB each |
| Requests per connection | 100 |
| Example-server active connections | 128 |
| Header deadline, including initial/keep-alive idle | 10 seconds |
| Entire body deadline, including chunk metadata/trailers | 30 seconds |
| Entire response write + flush deadline | 10 seconds |

Deadlines are monotonic, per whole stage, not reset for each byte/chunk. Reuse
consuming runtime.run_timeout; all pending loans/frames are released on timeout
before marking the connection failed. No detached timeout loser. Duration spans outside 1–4294967295 milliseconds are invalid configuration,
not an infinite timeout.
Handler CPU time/application awaits are not preempted by these I/O deadlines.
The server example uses bounded handlers; this is not CPU preemption or rate
limiting. Admission caps active connection tasks; when full, await an existing
owned handle while other tasks continue, instead of accepting indefinitely.

## 8. Error and rejection behavior

Use `http.Error`, not errno or abort for malformed/unsupported peer input.
Suggested variants: Io(io.Error), Protocol(ProtocolError), Limit(LimitKind),
Timeout(Stage), InvalidConfig, InvalidResponse, InvalidText, InvalidState.
Use explicit matches at io/http boundaries; do not implement general try error
conversion or a new special main return type.

Best-effort empty rejection before response bytes have begun:

| Condition | Status |
|---|---:|
| Malformed head/framing/chunks/trailers | 400 |
| Header-stage/body-stage timeout | 408 |
| Decoded body, chunk-count or aggregate chunk-metadata limit | 413 |
| Target/request-line limit | 414 |
| Unsupported expectation | 417 |
| Request header/trailer line, count or head budget | 431 |
| Unsupported transfer coding, CONNECT or Upgrade | 501 |
| Unsupported HTTP version | 505 |
| Application/response construction failure, before transmission | 500 |

Transport reset/broken pipe/response-write timeout: close without another
response. Error responses themselves use a write deadline, have length zero
and Connection: close. Never reuse or try to resynchronize failed input.
Internal impossible invariants retain the existing panic/abort policy; existing
allocation/task failures are not made fallible by this phase.

## 9. Implementation stages and gates

| Stage | Deliverable | Gate before proceeding |
|---|---|---|
| 16A | ADR/API skeleton, timeout prerequisite, owned types and module/editor loading | Native concrete-result timeout success/tie/expiry/abandonment and loan rejection/release tests; no new trust or syntax. |
| 16B | Pure head codec and bounded async head reading | Complete valid/invalid head corpus; byte fragmentation and clean/partial EOF; strict Host/Expect/framing decisions. |
| 16C | Length/chunked body decoding and trailers | Deterministic binary payloads, extension/trailer boundaries, overflow/limits, leftover bytes preserved for a following request. |
| 16D | Response encoder and Connection transaction state | Exact independent wire oracle for ordinary/HEAD/204/304/205 replies, reserved-header rejection, partial-write/flush behavior and close decisions. |
| 16E | Native example `examples/http_server.tarn`, keep-alive and bounded task supervision | One execution thread, one owned task per connection, parallel progress, capacity/timeout isolation, clean termination in bounded test mode. |
| 16F | Adversarial stress/mutation/memory-safety, documentation and final review | Complete workspace regression gate, balanced descriptors/frames, no accepted unsafe cases; accept ADR only if actual behavior matches. |

Do not introduce routing/server frameworks to connect these stages. The example
serves GET/HEAD health and a bounded POST echo; application routes are a small
explicit handler, with ordinary 404/405 responses and an appropriate Allow field.
Development runs may listen normally; tests use port 0 and a finite connection
count, without an internet dependency or signal/cancellation mechanism.

## 10. Validation matrix

Pure codec tests (same-module private test helpers, no public testing API):

- every CR/LF split, malformed separators/token bytes, obs-fold/control/NUL;
- case-insensitive names, legal duplicate ordinary headers and opaque values;
- missing/duplicate/malformed Host and supported IPv4/IPv6 authority forms;
- CL overflow/sign/comma/duplicates, TE+CL, repeated/unsupported coding;
- chunk hex overflow, CRLF, zero chunks, valid/invalid extensions and trailers;
- exact-limit and one-byte-over cases for every budget;
- invalid outbound status, reserved fields and response-splitting attempts.

Native loopback tests, raw independent peer and response oracle:

- GET, HEAD, POST echo, OPTIONS *, extension methods and 404/405;
- binary length and chunked bodies; multiple keep-alive/pipelined requests;
- multiple messages in one write and every meaningful fragmentation boundary;
- EOF/half-close/reset before/after head, mid-body and mid-response;
- 100-continue rejection without waiting for the body;
- clients that stop sending/reading, and absolute deadline enforcement;
- at least 40 concurrent clients, 25 requests each, deterministic final payloads;
- admission saturation above the configured cap and progress of other clients;
- large writes exercising partial progress/WouldBlock without busy loops;
- exact close and timer/Waker/frame cleanup on completion, error, timeout,
  handle move/drop and pending server abandonment.

No sleep-based correctness proof. Coordinate through sockets/private test
helpers and monotonic timers; external test timeouts are only hang guards.
Real timeout tests use broad watchdog margins, not sub-millisecond timing
assertions. Deterministic timer/readiness injection covers tie/boundary cases.
Compare wire bytes/status/body against an independently implemented host oracle;
a local existing HTTP client may be a supplementary interoperability check,
not a mandatory network dependency or an excuse to omit raw adversarial tests.

Canonical memory_safety and pass/fail cases:

- request/header/body views cannot escape owners or survive overwrite/move;
- one Connection cannot have overlapping read/write async loans;
- borrowed local request/connection cannot be sent to an async spawned task;
- owned accepted connections can enter connection tasks correctly;
- borrowed Operation timeout releases storage on completion/abandonment;
- dropping HTTP futures cannot release resources still held by a valid loan;
- Response builder moves do not duplicate body/header resources.

Add HTTP fixtures to both mutation corpora. Panic/hang on malformed compiler
input, malformed code, missing/double close, invalid abandonment, ownership
weakening or wire-framing invariant violations in controlled fixtures fail.
Extend IR/native metadata checks only if a concrete prerequisite changes them;
HTTP itself must not add magic declaration IDs. Preserve the purpose of existing
opaque-http tests by moving them to a still-unimplemented placeholder module.

## 11. Completion, documentation and publication

Run `cargo test --workspace --locked -j4` and relevant native/protocol checks;
keep passes, failures, ignored tests and unrun checks distinct. Fix regressions
with focused tests; all previous Phase 11–15 behavior must remain green.

Deliver `docs/http.md`, `docs/phase-16-report.md`, the native example, updated
roadmap/architecture/language links and stable AGENTS rules after implementation.
Record API, ownership/capabilities, framing, deadlines, runtime boundary, tests,
bugs, decisions defended/questioned and remaining limitations. Keep ADR proposed
until every implemented rule and prerequisite is backed by evidence.

Phase 16 is done when the documented HTTP/1.1 subset serves real bounded async
connections correctly, hostile input returns errors, no framing ambiguity is
accepted, all resources/loans clean up correctly and regression/mutation suites
pass. Publish authorized changes to main and stop. Do not begin the next phase.
