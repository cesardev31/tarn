# Phase 26 report: less ceremony without runtime cost

Design: [ADR 0054](adr/0054-less-ceremony.md).

## What was implemented

- **26A**: 449 redundant literal conversions removed from the stdlib with a
  compiler-guided script; two compound initializers rewritten explicitly
  (`max: usize := 18446744073709551615`).
- **26B**: string literals accepted where `&string` is expected
  (`CoercionKind::BorrowLiteral`), lowered exactly as `&"..."`, including
  scope extension for typed `let` bindings. 157 `&` removed from the stdlib,
  examples and the doctor port, again compiler-guided (comparisons such as
  `x == &"GET"` correctly keep theirs).
- **26C**: `Self` in interfaces and impls, E3073 for `Self` through
  `any I`, `core.Eq`, `==`/`!=` lowered to `Eq.eq` with borrowed operands,
  `Eq` for `io.ErrorKind`.
- **26D**: formatting designed (`f"..."`) and deferred for lack of evidence.

## Effect (same 2,898-line corpus)

| Pattern | Before | After |
|---|---|---|
| Literal conversions (`u16(200)`) | 399 | 21 (binding initializers) |
| `&"literal"` | 121 | 16 (comparisons) |
| `==` on enums | impossible | `e.kind == io.ErrorKind.NotFound` |

`http.tarn` shrank from 38,409 to 36,587 bytes. Line counts barely change:
the gain is less noise per line, not fewer lines. Generated code is the
same: no new allocation or copy was introduced.

## Tests

- `compiler/backend/tests/combinators.rs`: literal temporaries in a loop
  and in a typed `let` are each destroyed exactly once.
- `tests/native/pass/eq_interface`: structs (equality ignoring a field by
  the author's choice), enums with payloads, references, a temporary on the
  left, a generic `T: Eq` count, `io.ErrorKind`.
- Type failures: E3001 (literal where `&mut string` is expected), E3073,
  E3006 (struct without `Eq`), E3023 (wrong `Self` in an impl).
- Goldens: two resolution fixtures changed only by line numbers or by the
  removed conversions in the dumped `string` module.
- Full workspace: 266 passed, 0 failed, 1 ignored (existing benchmark), no
  warnings. The doctor port still matches Python byte for byte; the SQLite
  CRUD passes its HTTP checks.

## Bugs found

1. Lowering a borrowed literal recursed forever (the temporary store
   re-applied the same coercion). Fixed by storing the literal raw.
2. `r: &string := "x"` dropped the temporary at the end of the statement
   (E4105), unlike `r := &"x"`. Fixed; covered by the destruction test.
3. The new `eq_ops` table was not merged across modules, so `==` silently
   fell back to the primitive comparison and moved its operands (E4001).
   Fixed when the first Eq program failed.

## Decisions I would defend

- Borrowing literals, not named values: a literal has no owner to move, so
  nothing about ownership is hidden.
- `Self` as an implicit type parameter reuses the generic machinery; the only
  new rule is E3073.
- Deferring formatting: the numbers do not justify new literal syntax yet.

## Decisions I still question

- `Eq` for fieldless enums is a hand-written match per variant (21 arms for
  `ErrorKind`).
- `Option`/`Result` still lack `Eq` until impls can carry bounds.

## Known limitations

- No `Eq` for `Option`/`Result`, no formatting, string literals still
  allocate per evaluation.
