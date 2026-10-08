# ADR 0052: explicit error conversion through combinators

Status: accepted. Phase 23; [plan](../phase-23-plan.md),
[report](../phase-23-report.md). Refines ADR 0005; `try` itself is unchanged.

## Context

`try` requires the operand's error type to equal the function's (E3017). Two
real programs needed hand-written helpers only to change error types: `saved`
in `examples/http_crud.tarn` (io.Error to http.Error) and `try_response` in
`examples/sqlite/crud.tarn` (12 uses, panicking on http.Error). `Result` and
`Option` had no methods, and payload variants were not function values (E3038),
so no compact explicit conversion existed.

## Decisions

1. **Combinators in `core`, written in Tarn**: `Option.is_some/is_none/
   unwrap_or/ok_or/map` and `Result.is_ok/is_err/ok/unwrap_or/map/map_err`.
   Callbacks are `once fn`: each runs at most once, and callers may pass
   closures that move their captures.
2. **Closure literals adopt `once fn`** when they are not `move` closures. A
   borrowed environment lives on the stack (or is null), so consuming
   invocation releases nothing. A non-consuming `move fn` owns a heap
   environment that a consuming call would not free, so it is still rejected
   (E3001). This extends the Phase 17 `mut fn` adoption (ADR 0048).
3. **Payload variants are function values where a function is expected**:
   `map_err(http.Error.Io)`, `apply(Some, 3)`. The constructor captures
   nothing, so it adopts the expected invocation mode. Lowering generates a
   capture-free constructor function referenced like a capture-free closure.
   Outside a function context, `Shape.Circle` without arguments is still E3038.
4. **No new `try` syntax and no implicit conversion.** Stage 23C measured both
   programs after 1–3 (see the report). The remaining repetition comes from
   fallible HTTP response constructors with constant statuses, an API issue,
   not from `try`. `try expr else f` would save `.map_err(` per call site and
   nothing else; an implicit `From`-style conversion would hide which error a
   `try` returns behind impl lookup. Neither is justified by current evidence.

5. **Infallible responses for fixed statuses** (follow-up, chosen over
   changing the existing constructors or adding a validated `Status` type):
   module functions `http.json/text/empty/method_not_allowed` return
   `Response` and abort on an invalid fixed status or header. The `Response.*`
   constructors keep returning `Result` for data-driven statuses, so no
   existing caller breaks; the cost is two spellings for the same response.

## Consequences

- Conversion is visible on the same line as `try`:
  `try store.save().map_err(http.Error.Io)`.
- Applications define small error enums (`AppError { Db(..), Http(..) }`)
  and convert explicitly; no helper that panics is needed.
- Generic callables stored in locals still meet a conservative provenance
  rule (a result of generic type may borrow the callable's storage); passing
  the callable as a `once fn` parameter avoids it. Recorded as a limitation.

## Evidence that would change this

- Many call sites converting the same error pair in one module would justify
  a declared, still explicit, per-module conversion. Repeated
  `.map_err(AppError.Http)` on response constructors should first be solved
  in the HTTP API (infallible constructors for constant statuses).
