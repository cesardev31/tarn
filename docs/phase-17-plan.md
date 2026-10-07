# Phase 17 plan: application ergonomics

Status: complete. Design: [ADR 0048](adr/0048-application-ergonomics.md).
Evidence: [report](phase-17-report.md).
Baseline: Phase 16, commit `1b38624`, 219 workspace tests passed.

## 1. Goal and scope

Make a small JSON-over-HTTP CRUD about as short as the equivalent Go program
without a framework, using ordinary library code wherever possible. Acceptance
program: `examples/http_crud.tarn` (also the external `tarn-crud` evaluation
application). The phase was reprioritized ahead of C FFI, now Phase 18
([plan](phase-18-plan.md), [ADR 0047](adr/0047-native-c-ffi.md)), because FFI
gives a database but does not shorten application code.

Included: string accumulation, JSON encoding/parsing, HTTP response helpers,
a sequential handler-based server, and the compiler fixes those libraries
need.

Excluded: reflection/derive, `try` error conversion, value-producing
`if`/`match`, routers or middleware, concurrent `serve`, HTTP client, float
formatting, borrowed JSON views, FFI/SQLite and package management.

## 2. Stages and gates

### 17A: compiler prerequisites

- Recursive owned types through `Vec`: layout of `[0]T` markers and
  out-of-line vector element destruction.
- Closure literals adopt an expected `mut fn` when non-consuming.

Gate: native regression for direct and indirect recursion with exact string
destruction counts; native pass case and E3001 fail case for closure modes.

### 17B: strings

`string.Builder` (push, push_u64, push_i64, len, finish) and
`string.strip_prefix`.

Gate: native pass case including full integer ranges, multibyte text and a
1000-push accumulation.

### 17C: json

Bundled ordinary module with Encode, Writer, encode/encode_list and strict
parse/parse_bytes into owned Value.

Gate: native pass case covering escaping, all literal kinds, surrogate pairs,
duplicate members, integer limits, the depth limit and offsets for 14 invalid
inputs.

### 17D: http

Response.json/empty/method_not_allowed and http.serve; TCP SO_REUSEADDR.

Gate: integration test with an independent Rust peer: shared handler state,
JSON framing, 204, 405 with Allow, handler error 500, protocol 400, close after
each response (pipelined second request unanswered) and service continuing
after failures. Native rebind test fails without SO_REUSEADDR and passes with
it.

### 17E: acceptance

Rewrite the CRUD; it must keep identical behavior over curl and persist across
restarts. It joins the example suites (resolution, IR, mutations).
