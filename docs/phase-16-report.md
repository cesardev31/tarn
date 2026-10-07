# Phase 16 report: bounded async HTTP/1.1 server

Design: [ADR 0046](adr/0046-bounded-async-http.md).
API and limits: [HTTP](http.md).
Stages and gates: [implementation plan](phase-16-plan.md).

## Implemented

Stages 16A–F supply ordinary stdlib/http, owned Connection/Request/Header/Response,
Copy limits/timeouts and structured errors. Server-side HTTP/1.1 handles strict
heads, Content-Length, incoming chunking/extensions/trailers, opaque binary bodies,
sequential keep-alive/pipelining, HEAD and 204/304/205. Outbound responses use
preflight plus generated Content-Length and explicit flush. No native HTTP parser,
core API, intrinsic ID or trusted-http authority was added. Local override and
official editor-source checking work without granting native privileges.

examples/http_server.tarn serves health and binary echo with explicit 404/405.
One Execution owns cooperative connection tasks. The example caps admission at
128 handles, awaits an owned handle when full and has a finite-count test mode.
It does not introduce a routing framework, worker pool or signal facility.

## Ownership, capabilities and state

Connection owns a non-Copy TcpStream and both net buffers. Its structural
capabilities are Transfer/not Share. Request/Header/Response are owned non-Copy
ordinary ADTs; immutable views use existing loans, and their structural
Transfer/Share supports safe shared workers. Optional header lookup owns a clone.
Moving a connection transfers the stream and buffers; close consumes it, and
post-drop closes a live stream exactly once. No runtime owner bit, backend
ownership query, user-defined Drop or raw mutable stream projection was added.

Connection caches request metadata so consuming a Request cannot alter HEAD or
close semantics. Failed input cannot be reused. Response preflight emits nothing;
partial write, timeout or future abandonment prevents another response. Protocol
state preserves transaction order but never decides whether native storage lives.
Drop discards unflushed output. Early exits and pending abandonment use existing
verified async-frame destruction and ordinary resource glue.

## Framing, errors and deadlines

Strict CRLF, ASCII method/target rules and exactly one lexical Host avoid ambiguous
input. Duplicate CL, signs/comma/overflow, TE+CL, repeated chunked, obs-fold,
controls and forbidden trailers are rejected. Safe trailers remain separate;
chunk extensions are validated but never interpreted. Requests do not use EOF
framing. Incomplete EOF is an error; fresh EOF is normal. Binary body bytes never
require UTF-8. Errors preserve io.Error or distinguish protocol/limit/timeout and
construction/state failures. Main and try retain their existing exact error rules.

Reject Expect with 417 before waiting for its body. Unsupported coding/CONNECT/
Upgrade get 501; unsupported versions 505; malformed syntax 400. Read deadlines
map to 408, body/chunk limits 413, start-line 414 and head/trailer limits 431.
Rejection is one best-effort empty close response before output begins. Closing
unread hostile input can result in a Linux reset; transport/write failure does
not attempt a second response or drain input indefinitely.

Defaults are finite and configurable: 32 KiB head, 1 MiB body, 100 fields,
bounded chunks/trailers, 8 KiB buffers and 100 requests per connection. Complete
head/body/write stages have monotonic 10/30/10 s deadlines; progress does not
reset them. Configuration rejects zero/unrepresentable budgets and durations.
Timeouts do not preempt handler CPU or unrelated application awaits.

## Timeout prerequisite and runtime boundary

The initial generic consuming Operation<R>.run_timeout proposal was rejected by
existing E4105/E4201: manual poller results can borrow their own storage. It would
be unsound to destroy every such owner while returning arbitrary R. No checker
was weakened. Final runtime.run_timeout consumes Operation<Result<void, io.Error>>,
returns Result<bool, io.Error> and writes owned stage results through normal
caller output loans. true is completion; false is timeout; errors remain errors.
Completion wins ties. The loser is destroyed, not detached, and loans are released
before caller access. HTTP stages use this helper without spawning local loans.

Parsing, state, encoding and deadline composition remain Tarn. Native calls are
existing TCP, timerfd, epoll/wake and allocation mechanics. Opt-in private
TARN_TRACE_EXEC observes allocation/free events for frames/closures/Vec storage,
Wakers and async task records; it does not infer ownership or expose public API.
The backend remains unchanged and executes verified post-drop.

## Tests

`cargo test --workspace --locked -j4`: 219 passed, zero failed, no warnings;
one existing benchmark ignored. Both frontend and native mutation suites passed,
as did canonical memory_safety and all prior Phase 11–15 regressions. HTTP adds
13 native Rust tests, one module/editor test and one local-timeout native test.
The application fixture produces its exact native golden output. An existing
curl client also successfully fetched /health over loopback HTTP/1.1; it is a
supplementary smoke check, not a new test dependency.

Independent loopback peers/oracles cover exact response bytes, HEAD suppression,
204/304/205, binary length/chunking, trailer separation, coalesced requests,
request ordering, every split position of a complete request and bytewise chunked
input. An exact 1 MiB payload is echoed. Limit tests cover exact/over head lines,
head bytes/fields, response bytes/fields/body, decoded body, chunk count,
chunk-metadata lines/aggregate and trailer count. Metadata uses exactly 64 and
65 bytes under a configured 64-byte cap. Request-count closure and invalid
transaction operations are tested.

Forty clients each send 25 requests (1000 deterministic echoes) through a
configured eight-handle admission cap. Stalled head/body clients time out while
a healthy peer progresses. Small receive windows plus pipelined large replies
force write backpressure; the observed outcome is specifically Write timeout.
There is no sleep-based protocol proof; watchdog polling only bounds hung tests.
Loopback needs no internet or external HTTP package.

Descriptor and allocation ledgers balance on completion, errors, timeouts,
local move, pending future drop and task completion. Unique request-target
string traces prove one destruction on success and early body timeout. An oracle
mutation test rejects missing/double close/free events. The local timeout native
test proves borrowed-state success/release, expiry without mutation and immediate
completion priority with a zero timer. Existing runtime mechanics are compiled
under -Wall -Wextra -Werror in their native regression fixtures.

Canonical memory_safety adds 16 HTTP ownership/loan/capability cases. The owned
HTTP fixture joins native goldens and both compiler mutation corpora; the server
example also participates in frontend mutations. Opaque http rejection fixtures
move to still-unimplemented tls; unused/shadowing imports use json so their
purpose and diagnostic goldens stay focused on unresolved bootstrap modules.

## Bugs and design issues found

The generic timeout proposal conflicted with safe manual-poller provenance, not
with HTTP specifically. Concrete-result operations and caller slots resolve the
phase without changing the ownership model. A future general result timeout
needs a deliberately justified contract, not a blanket lifetime exception.

Private fields with the same names as public methods shadowed method calls in
current Tarn resolution. Internal underscored storage fixes the library; no
language resolution change was needed. Await directly in a match keeps its
child loan until statement cleanup; the example binds the owned result first.
This remains a documented ergonomics limitation rather than a safety weakening.

Review caught an outbound head-budget subtraction that could underflow before
validation; preflight now checks fixed generated fields before subtracting.
Transfer-coding repetition now checks tokens independently of whitespace/case.
The test oracle initially demanded graceful EOF after rejecting unread data;
Linux can reset there or race shutdown, so negative peers accept those precise
close outcomes while valid replies still require exact EOF and no extra bytes.

GCC's strict native tests warned about passing newly allocated uninitialized
pointee storage to the trace helper. It now accepts only uintptr_t addresses;
no memory initialization, warning suppression or semantic change was introduced.

## Decisions I would defend

Owned bounded data fits current Tarn semantics and avoids storing borrowed
references or pinning request views to mutable transport buffers. Strict framing
and non-reuse protect the application boundary. Explicit preflight/flush and
separate trailers make wire behavior reviewable. Sequential connection handlers
and bounded admission reuse existing execution without scheduler redesign.
Whole-stage deadlines prevent indefinite slow progress from resetting the clock.
Concrete timeout results preserve manual-poller safety and ordinary provenance.

## Decisions I still question

Owned codec paths make many copies and allocations; optimize only from later
measurement. A single byte body cap is deliberately simple but not streaming.
Admission awaits an existing handle rather than whichever completes first;
finite I/O deadlines bound that choice, but it can reduce admission utilization.
The await/match binding ceremony deserves a later lowering/liveness review.
The strict authority/coding subset rejects some otherwise legal interoperable
forms; adding compatibility must preserve unambiguous framing and clear tests.

No wall-clock Date generation or blanket RFC conformance is promised. General
result timeouts, scoped async, streaming, client/proxy semantics, informational
responses, TLS, framework ergonomics and generic async I/O require separate
application evidence and decisions. Handler timeout/preemption and graceful
shutdown were not introduced. Allocation/resource exhaustion keeps prior abort
semantics. Connection operations must receive their driving Execution; the
existing nested-wake identity invariant still applies to a mismatched owner.
No Phase 17 or subsequent phase work was started.
