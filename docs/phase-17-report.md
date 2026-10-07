z# Phase 17 report: application ergonomics

Design: [ADR 0048](adr/0048-application-ergonomics.md). Plan:
[phase-17-plan.md](phase-17-plan.md).

## What was implemented

- `string.Builder` (push, push_u64, push_i64, len, finish) and
  `string.strip_prefix`, in ordinary Tarn.
- New ordinary bundled `json` module: `Encode`, `Writer`, `encode`,
  `encode_list`, strict RFC 8259 `parse`/`parse_bytes` into an owned `Value`
  with owned accessors. Registered in the driver like `http`/`path`
  (overridable, editor-visible).
- `http.Response.json`, `Response.empty`, `Response.method_not_allowed` and
  `http.serve(address, handler)`.
- Backend: `[0]T` markers no longer inspect `T`; vector element destruction
  calls per-type functions declared on demand. Owned types may now recurse
  through `Vec`.
- Type checker: a non-consuming closure literal adopts an expected `mut fn`.
  The backend accepts the identical function-item representation for it.
- Runtime: TCP binds set SO_REUSEADDR.
- `examples/http_crud.tarn`: 142 lines in one file, replacing a 286-line,
  eight-file program with less functionality (plain-text input, ad hoc file
  format).

## Tests

- `compiler/backend/tests/recursive_types.rs`: direct and indirect
  recursion; every string at every depth is destroyed exactly once, including
  a value popped out before the tree is destroyed.
- Native pass cases: `json_codec` (escaping, literals, surrogates, duplicates,
  integer limits, depth limit, 14 invalid inputs with exact offsets, invalid
  UTF-8), `string_builder`, `closure_mut_adoption` (borrowed, owned,
  capture-free and mutating closures), `network_rebind`.
- `tests/types/fail/E3001_once_closure_for_mut_fn`: a consuming closure is
  still rejected where `mut fn` is expected.
- `http.rs::serve_runs_a_mutable_handler_sequentially_and_closes_each_connection`:
  an independent Rust peer checks shared state, JSON/204/405/500/400 framing,
  close after each response and continued service after failures.
- New native cases also run through the existing line-deletion mutation test;
  the new example joins the resolution, IR and mutation suites.
- Full workspace: see the final validation line below.

## Bugs found

1. `enum Value { List(Vec<Value>) }` failed with "recursive by-value
   layout": layout descended into `Vec`'s zero-size `[0]T` marker.
2. After that fix, code generation overflowed the stack: vector element
   destruction was expanded inline for every nesting level.
3. Restarting a server failed with EADDRINUSE for about 60 seconds once the
   server, not the client, closed connections. The rebind test fails without
   the fix and passes with it (verified both ways).
4. Found by the existing mutation test: a capture-free closure literal typed
   `mut fn` reached the backend as a shared function item, which the backend
   reported as an "assignment ABI mismatch". Fixed and covered directly.
5. Example ceremony: `u16(200)` and `usize(0)` in `examples/http_server.tarn`
   were never necessary, because contextual literal typing already works. The
   pattern was copied into application code. The example is unchanged because
   its tests rewrite those exact strings.

## Decisions I would defend

- Explicit `Encode` over reflection or derive: about seven visible lines per
  type, no new language machinery, and the wire format is reviewable.
- `http` does not import `json`; JSON text is the boundary, so neither module
  forces the other on users.
- Sequential `serve` with close-per-response: it lets handlers capture
  `&mut` state with no locks and no new concurrency model, and avoids
  keep-alive starvation. The limit is stated next to the API.
- Owned JSON accessors: forced by the v0 rule against references inside
  enums, and simple to reason about; costs are measurable later.
- Out-of-line glue only for vector elements: the smallest change that makes
  recursion finite; other destruction paths are unchanged.

## Decisions I still question

- `Value.text(key)` and similar names read well at call sites but mix
  "member lookup" and "conversion" in one method. Real JSON-heavy code may
  prefer explicit `get(key)` chains.
- Copying the whole input in `parse_bytes` (because structs cannot hold a
  borrowed slice) is a measurable cost on large bodies.
- Adopting `mut fn` only for closure literals is deliberately narrow. Named
  functions passed as `mut fn` are still rejected; that may also deserve the
  same treatment.
- `string + &string` moves its left operand, so `prefix + p` inside a closure
  silently makes it `once`. The type error then mentions `once fn`, which is
  surprising. A diagnostic hint may be worth adding.

## Known limitations

- `try` cannot convert `io.Error` to `http.Error`; the CRUD needs one explicit
  `match` (`saved`). This is the strongest remaining evidence for an error
  conversion design.
- `var x: T` plus `match` remains common because `if`/`match` do not produce
  values (an explicit v0 decision).
- `serve` handles one connection at a time.
- No float formatting, so JSON numbers are written only from integers.
- SQLite remains unavailable until Phase 18 FFI.

## Final validation

`cargo test --workspace --no-fail-fast`: 221 passed, 0 failed, 1 ignored (the
existing benchmark), no build warnings. Baseline was 219.

Two resolution fixtures (`unused_import`, `shadowing_warnings`) used `json`
as a placeholder module name; now that it is real they would dump the whole
module into their goldens. They use the still-unimplemented `collections`
instead, with unchanged warnings apart from the name. The `imports` golden
grew by the new `string` declarations.
