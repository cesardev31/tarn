# ADR 0046: bounded async HTTP/1.1 server foundation

Status: accepted. Phase 16 matches the documented bounded server subset;
[implementation evidence](../phase-16-report.md).
Detailed stages, API, limits, error policies and tests:
[Phase 16 plan](../phase-16-plan.md).

## Context

Phases 11–15 supply owned native tasks/resources, nonblocking TCP, source async,
Execution/AsyncTask, timers, TCP-specific buffered I/O and owned UTF-8 utilities.
The phase begins without an HTTP library. Scoped async loans and generic async I/O
remain deferred; HTTP must fit the existing ownership and module architecture.

## Decisions

1. Implement server-side HTTP/1.1 request decoding and response encoding in
   ordinary `stdlib/http`, using only public lower-layer APIs. No native HTTP
   parser, intrinsic catalog, core type or trusted-http privilege.
2. Connection owns its stream and both buffers. Request/Header/Response own
   their text/byte data; references use existing loans. Structural capabilities
   remain authoritative: Connection inherits Transfer/not Share from TcpStream;
   no special HTTP capability inference is added.
3. Complete request bodies are bounded owned bytes. Support Content-Length and
   incoming chunked coding/trailers. No automatic text decoding or streaming
   application body API. Keep trailer data separate from primary headers.
4. Require strict CRLF, a single supported Host authority and unambiguous
   framing. Reject duplicate Content-Length even when values match, TE+CL,
   invalid framing, obs-fold and forbidden trailer fields. Do not resynchronize
   or reuse failed input. Default budgets/deadlines are explicit in the plan.
5. Response bodies use library-generated Content-Length. Validate all outbound
   fields before output; prevent users from overriding framing/control fields.
   Implement HEAD and 204/304/205 body rules. No informational, upgrade or
   outbound chunked response API in v0.
6. Keep-alive/pipelining is sequential per connection. Connection remembers
   transaction metadata independently of application requests. Explicit flush
   completes a response; destruction discards output and closes normally.
7. Use monotonic absolute deadlines per complete head/body/write stage. Add
   only consuming runtime.run_timeout for Operation<Result<void, io.Error>> over
   existing poll/Timer/Waker semantics, with owned stage results in caller output
   slots. Completion wins ties; timeout abandons through verified frame
   destruction. No spawned task borrows a local Connection, no detached loser,
   scoped async feature, scheduler redesign or general cancellation API.
8. Use structured http.Error with explicit io conversion. Normal peer errors
   do not abort. Best-effort empty error responses are allowed only before
   response transmission, followed by close. Do not add main/try semantics.
9. Reject Expect before body reading with 417; no 100-continue. Reject CONNECT,
   unsupported transfer coding and Upgrade; support origin-form/OPTIONS * only.
   HTTP/1.0 and client/proxy functionality are outside this server subset.
10. Example server owns bounded connection-task handles on one Execution.
    Admission backpressure, stage timeouts and ordinary drop contain resources.
    No routing framework, TLS, HTTP/2/3, thread pool, signals or optimizer work.

## Alternatives and consequences

A native HTTP parser would duplicate validation outside Tarn and expand the
runtime boundary before application evidence. A borrowed Request would require
stored references and constrain buffer reuse. Owned bounded data fits current
Tarn semantics at the cost of copies and memory proportional to configured caps.

A permissive framing parser improves compatibility but risks disagreeing with
other HTTP components. Strict rejection is the v0 policy; this is a documented
subset, not blanket RFC conformance. Incoming chunking is included because it
is a basic HTTP/1.1 framing mechanism, not a compression/framework feature.

Spawning per-stage tasks borrowing Connection contradicts E4209. The originally
proposed generic consuming timeout is unsafe for arbitrary
manual poller results that may borrow poller storage. The concrete void-result
helper and ordinary output loans avoid that contradiction without weakening
provenance. Native tests prove local completion/expiry release and abandonment.
Future timeout generalization must preserve those borrowing guarantees. Generic async interfaces and scoped tasks are unnecessary
for a TCP-only server with owned connection handlers.

A client, router, streaming bodies and TLS would expand the phase substantially.
Separate them until a safe interoperable protocol core is tested. Head deadlines
include idle waiting; body/write deadlines cover whole stages. No CPU preemption
or handler-timeout promise is implied by I/O deadlines.

## Acceptance evidence

`cargo test --workspace --locked -j4`: 219 passed, no warnings, one existing
benchmark ignored. Native HTTP byte oracles, adversarial loopback/fragmentation,
1000 concurrent echoes, stage timeouts/backpressure, exact cleanup ledgers,
canonical ownership/capability cases and both mutation suites pass. Prior Phase
11–15 regression suites remain green. Existing curl also verifies the server on
loopback without an internet or package dependency.

The generic timeout proposal was narrowed deliberately to concrete void-result
operations and borrowed caller output slots, with native proof of loan release,
completion priority and abandonment. No capability/lifetime safety was weakened.
The accepted API, defaults, error/EOF/reset policy, authority subset and ownership
model are documented in [HTTP](../http.md). This accepts the specified server
subset, not every HTTP/1.1 feature or a client/framework. Stop after Phase 16.
