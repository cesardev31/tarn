# Phase 23 report: explicit error conversion

Design: [ADR 0052](adr/0052-explicit-error-conversion.md). Plan:
[phase-23-plan.md](phase-23-plan.md).

## What was implemented

- **23A**: `Option` and `Result` combinators in `core`, written in Tarn:
  `is_some`, `is_none`, `unwrap_or`, `ok_or`, `map`, `is_ok`, `is_err`, `ok`,
  `map_err`. Non-`move` closure literals adopt an expected `once fn`.
- **23B**: payload variants are function values where a function type is
  expected, in any invocation mode; lowered to generated capture-free
  constructor functions.
- **23C**: both evidence programs rewritten and measured; no new syntax.

## Measurements (23C)

| Program | Before | After | Helpers removed |
|---|---|---|---|
| `examples/http_crud.tarn` | 142 lines | 137 lines | `saved` (7 lines) |
| `examples/sqlite/crud.tarn` | 129 lines | 128 lines | `try_response` (7 lines, panicked) |

The SQLite CRUD now has an `AppError { Db(..), Http(..) }` enum and 16
`.map_err(...)` calls. 12 of them are `.map_err(AppError.Http)` on HTTP
response constructors whose statuses are constants, so they cannot actually
fail. That repetition is caused by the HTTP API, not by `try`. A `try ... else`
form would not remove it. Both programs were exercised over HTTP after the
rewrite (all CRUD paths, 400/404/405).

### Follow-up: infallible fixed responses

`http.json/text/empty/method_not_allowed` return `Response` and abort on an
invalid fixed status. The SQLite CRUD no longer needs `AppError` or any
`map_err`: its handler returns `Result<http.Response, sqlite.Error>` and uses
`try` directly.

| Program | Phase 22 | After 23A–C | After follow-up |
|---|---|---|---|
| `examples/sqlite/crud.tarn` | 129 lines, `try_response` (panicking) | 128, `AppError` + 16 `map_err` | 121, no helper, no `map_err` |

## Tests

- `tests/native/pass/result_combinators` (all combinators, a closure
  borrowing a local) and `tests/native/pass/variant_constructors` (`Some` as
  `fn`, a two-field variant as `mut fn`, `map_err(AppError.Io)`, generic
  `Some` through `once fn`).
- `compiler/backend/tests/combinators.rs`: every discarded owned value
  (unused fallback, dropped error, unused Ok payload, mapped error) is
  destroyed exactly once.
- `tests/native/pass/http_fixed_responses` and
  `http.rs::fixed_response_shorthands_abort_on_invalid_status` (low and high
  status, control character in Allow).
- `tests/types/fail/E3001_move_closure_for_once_fn`: a non-consuming `move fn`
  is still rejected where `once fn` is expected. The existing E3038 fixture
  still covers bare variants outside a function context.
- Full workspace: 263 passed, 0 failed, 1 ignored (existing benchmark), no
  warnings. Mutation tests now run in parallel (frontend 450 s to 93 s,
  native 554 s to 111 s; full suite about 20 min to about 7 min).

## Bugs found

1. `map`/`map_err` with `mut fn` callbacks failed provenance (E4201): a
   generic result may borrow the callable stored in a local. Using `once fn`
   parameters avoids storing the callable; recorded as a limitation.
2. The first `once fn` declaration rejected every ordinary closure literal
   (E3001), as the plan predicted; fixed by decision 2 of the ADR.

## Decisions I would defend

- Conversion stays on the `try` line, explicit and greppable.
- Non-`move` closures adopting `once fn` is sound: nothing is released by a
  consuming call of a stack or null environment.
- Measuring before adding syntax: the data showed the real friction is in the
  HTTP API.

## Decisions I still question

- Constructors are generated per use site instead of once per variant: simple
  and correct, but duplicates tiny functions in large programs.
- 68 golden snapshots grew by about 12,800 lines because every IR dump
  includes core function bodies. A future snapshot filter could omit unused
  core functions.

## Known limitations

- Calling a generic callable stored in a local and returning its result is
  rejected conservatively (E4201); pass it as a `once fn` parameter instead.
- Bare variants without a function context are still E3038.
- Two spellings exist for fixed responses (`http.json` and
  `Response.json`); kept so that no existing caller breaks.
