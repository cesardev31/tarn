# ADR 0062: bounded synchronous HTTPS through system libcurl

Status: accepted scoped implementation (Phase 33).

## Evidence and dependency review

The existing clasificador-tickets Go application calls Chatwoot REST and TypeSafe
System One. TLS, certificate validation, HTTP framing and DNS are complex standards;
a small internal TLS implementation is inappropriate. Select system libcurl's
stable C ABI, loaded lazily as libcurl.so.4. No new Cargo dependencies or executable
subprocess are introduced. The measured installation is libcurl 8.18.0 using
OpenSSL 3.5.5. Its distribution build loads roughly 28 shared dependencies,
including TLS/crypto, IDN, HTTP/2, compression, LDAP, SSH and authentication
libraries. This is substantial native dependency weight, justified by delegated
protocol correctness; actual build/backend and security updates belong to the
system package manager. Future smaller TLS backends require separate evidence.

The adapter uses a narrow stable subset of option IDs declared by the public
curl header. Missing library/symbol/global initialization returns unavailable;
setopt errors propagate. Library/global state lives for the process, initialized
with pthread_once and never unloaded beneath concurrent tasks. No native request
owner escapes a synchronous call.

## Public contract

An ordinary overrideable https module exposes owned Response/status/body,
Options, GET and GET/POST request with one optional application header. Timeout
must be positive, response limit is bounded to 64 MiB (default 1 MiB), CA path
empty uses system trust. HTTPS is the only permitted protocol. Certificate-chain
and hostname verification are always on. There are no automatic redirects,
proxy-environment use, cookies, retries or transparent compression. HTTP status
errors remain ordinary responses. Query encoding follows RFC 3986 byte percent
encoding. NUL in C inputs and CR/LF in headers are rejected.

Tarn allocates/owns the output vector. The native callback copies only into that
exclusive buffer, checks multiplication and remaining capacity and reports limit
failure. Request handles/header lists are destroyed on every native exit. Inputs
are live only during the call; raw pointers never become public loans/resources.
Blocking remains visible: no executor/reactor/scheduler change. A configured
libcurl timeout includes connection/transfer; DNS deadline behavior depends on
the system libcurl resolver build, especially with NOSIGNAL enabled. Do not
claim a new independently enforced real-time deadline.

Native Error codes: -1 dependency/init unavailable, -2 response bound, -3 invalid
input; positive values preserve libcurl's documented errors. No retry is implicit.

## Acceptance port and boundaries

examples/ticket_classifier ports a selected-conversation workflow: authenticated
Chatwoot GETs, private/system/blank-message filtering, chronological ordering,
last 20 messages, 2,000 UTF-8 scalar text bound, three TypeSafe questions in one
request, and lossless JSON result output. Credentials are environment variables.
Tests use local mock HTTPS endpoints and fixture credentials only.

This is not the complete Go GUI/service. Pagination, poll workers, cached views,
label/priority writes, auto-apply and retry/backoff remain separate work. Equal
message timestamps use unstable ordering. Scores/probabilities retain exact JSON
number text; no float parser is added just to display a response.

Typed JSON Values now implement Encode. Number text is validated before writing,
so even manually constructed Number values cannot inject arbitrary JSON.

## Sources

- [libcurl callback contract](https://curl.se/libcurl/c/CURLOPT_WRITEFUNCTION.html)
- [TLS hostname verification](https://curl.se/libcurl/c/CURLOPT_SSL_VERIFYHOST.html)
- [Stable ABI declarations](https://github.com/curl/curl/blob/curl-7_88_1/include/curl/curl.h)
- [TypeSafe API](https://docs.typesafe.ai/api)

## Deferred

Response headers, streaming, reusable connections, redirect/proxy policies,
async HTTPS, richer errors and portable native dependency packaging are future
work. Supporting HTTPS does not claim full Internet-client completeness.
