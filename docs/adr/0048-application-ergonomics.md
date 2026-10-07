# ADR 0048: application ergonomics for small HTTP/JSON programs

Status: accepted. Phase 17; [plan](../phase-17-plan.md),
[report](../phase-17-report.md).

## Problem and evidence

A JSON CRUD written against Phase 16 needed 286 lines in eight files (a first
single-file version needed about 250). An equivalent Go program is 60–80
lines. AGENTS.md treats that gap as a design problem unless the ceremony buys a
safety guarantee. Here it did not. The extra code was:

- a hand-written byte-copy helper, because there was no text accumulator;
- manual JSON escaping and object writing, and no JSON parser at all, so
  request bodies were plain text;
- three to six lines per response for content types, empty bodies and Allow;
- about 60 lines of accept/read/reject/write loop per application;
- `u16(200)` / `usize(0)` casts copied from examples although contextual
  literal typing already made them unnecessary.

Two compiler defects blocked library solutions:

1. A recursive owned type through `Vec` (`enum Value { List(Vec<Value>) }`)
   failed in the backend: layout descended into the zero-size `[0]T` marker,
   and vector element destruction was expanded inline without bound.
2. A closure that only reads its captures could not be passed where
   `mut fn` was expected, forcing callers to know invocation modes.

A third defect appeared once the server closed connections: restarting a
server failed with EADDRINUSE for about 60 seconds because listeners did not
set SO_REUSEADDR.

## Decisions

1. **`string.Builder`** in the ordinary `string` module: owned, linear-time
   accumulation of complete strings (`push`, `push_u64`, `push_i64`, `len`,
   consuming `finish`). It never accepts raw bytes, so its result is valid
   UTF-8 by construction. Plus `string.strip_prefix`.
2. **New ordinary bundled `json` module** (like `http` and `path`: no
   intrinsics, overridable by a local module):
   - `interface Encode { fn encode(&self, out &mut Writer) }`, implemented
     explicitly per type. No reflection and no `derive` (still excluded by
     AGENTS.md); field lists stay visible in source.
   - `Writer` inserts separators and escapes text (`"`, `\`, control
     characters as short escapes or `\u00XX`). `encode(&T)` and
     `encode_list(&[]T)` cover the common cases.
   - `parse`/`parse_bytes` implement strict RFC 8259 into an owned `Value`
     tree: no trailing commas, leading zeros, raw control characters, lone
     surrogates or trailing content; nesting is limited to 128. Errors carry
     the byte offset. Numbers keep their validated text and convert through
     `as_u64`/`as_i64`, so no precision is lost and no float formatting is
     needed. Accessors (`get`, `text`, `u64`, `i64`, `bool`) return owned
     values, because v0 forbids references stored in enums (`Option<&Value>`).
3. **`http` response helpers**: `Response.json(status, body string)`,
   `Response.empty(status)` and `Response.method_not_allowed(allow)`. `http`
   does not import `json`; JSON text is the boundary.
4. **`http.serve(address, handler mut fn(&Request) Result<Response, Error>)`**:
   a blocking call that owns a private `Execution` and serves connections one
   at a time, so the handler can mutably capture application state without
   locks. Each response closes its connection, so one idle keep-alive client
   cannot stall the sequential server. Handler errors become their HTTP status
   (500 for I/O and construction errors). Concurrency and keep-alive control
   remain available through `Connection` directly. The `Execution` is created
   and destroyed by the call; this is not a hidden global executor.
5. **Recursive types through `Vec`**: `[0]T` markers have layout size 0 and
   alignment 1 without inspecting `T`. Every marker sits beside `usize` header
   fields and no native type aligns beyond 8, so no existing layout changes.
   Vector element destruction calls a per-element-type function declared on
   demand (`tarn_drop_glue_N`). Recursion therefore happens at run time, and
   the generated code stays finite. Direct by-value recursion is still
   rejected.
6. **A closure literal adopts an expected `mut fn`** when it does not consume
   its captures. Exclusive invocation is strictly stronger than shared
   invocation, and capture modes are unchanged (still shared borrows or
   moves). A closure inferred as `once` is still rejected (E3001), because
   consuming invocation also changes when the environment is released. This
   applies to closure literals only; there is no general subtyping between
   callable values.
7. **TCP listeners set SO_REUSEADDR** before bind, inside the existing native
   bind bridge, for stream sockets only. On Linux this permits rebinding over
   TIME_WAIT connections and still rejects a second active listener (tested).
   Rust's standard library and Go do the same on Unix.

## Alternatives considered

- **Reflection or `derive(Encode)`**: shortest call sites, but it needs
  compile-time introspection and generated impls. AGENTS.md defers `derive`
  until normal behavior is defined. Explicit `Encode` costs about seven lines
  per type and keeps the wire format visible.
- **`impl Encode for Vec<T: Encode>`**: impl type arguments are binders
  without bounds (ADR 0016), so `encode_list` and `Writer.list` provide the
  collection case instead of extending coherence for one library.
- **`http.json(status, &value)` with http depending on json**: one call
  shorter, but it couples the modules; `Response.json(status,
  json.encode(&v))` keeps them independent.
- **A router/framework**: rejected. `serve` plus explicit functions keeps
  routing in plain code, as Go's `net/http` does.
- **Implicit error conversion in `try`**: AGENTS.md requires real evidence.
  The CRUD shows the need (`io.Error` into `http.Error` at one call site) but
  the mechanism changes the language. It is recorded as open evidence, not
  implemented here.
- **Value-producing `if`/`match` to remove `var x: T` plus `match`**: an
  explicit non-goal of v0 syntax; recorded as evidence only.

## Consequences

The CRUD acceptance program (`examples/http_crud.tarn`) is 140 lines in one
file and covers more than the 286-line version: JSON input as well as output,
and JSON persistence. The JSON parser copies its input once and accessors
clone. Both are measured costs to revisit with benchmarks, not correctness
issues. The sequential server is a deliberate simplicity limit and is
documented next to `serve`.

## Evidence that would change this

- Repeated `match` blocks that only convert error types in real programs
  would justify a `try` conversion design.
- Encode boilerplate across many types would justify `derive` after
  equality and capabilities define normal derived behavior.
- Throughput needs for concurrent request handling with shared state would
  justify a concurrent `serve` variant over Mutex or scoped async.
