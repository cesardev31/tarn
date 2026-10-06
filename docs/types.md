# Type checking (phase 4)

Implementation: `compiler/types`. Output: side tables (expression types,
local types, method targets, pattern variants) keyed by `NodeId`/`SymbolId`.

## Types

`bool`, `i8…i64`, `u8…u64`, `isize`, `usize`, `f32`, `f64`, `string`, `void`,
`never`, structs and enums (with type arguments), `&T`, `&mut T`, `[N]T`,
`[]T` (unsized, behind a reference), `fn(A) R`, type parameters, `any I`.

## Inference

- Local and bidirectional: each function is checked on its own with a
  unification table; signatures are never inferred.
- Integer literals get an *integer variable*: it unifies only with integer
  types and defaults to `i64`; float literals default to `f64`. A literal
  takes the type its context needs (`b: u8 := 2; c := b + 3` → `c: u8`) and is
  range-checked after inference (E3025).
- Generic functions, methods of generic types (ADR 0015) and variants are
  instantiated with fresh variables per use; the expected type flows into the
  call (`x: Option<i32> := None`).
- A binding whose type is still unknown at the end of its function is E3010.

## Conversions and coercions

- **No implicit numeric conversions.** `i32 + i64` is E3006. Conversions are
  explicit calls: `i64(x)` (checked; trapping at run time when out of range).
- Implicit coercions, only at "expected type" sites (arguments, annotations,
  returns, assignments): `&mut T → &T`, `&[N]T → &[]T`, `&T → &any I` when `T`
  implements `I`.

## Operators

| Operators | Operands | Result |
|-----------|----------|--------|
| `+ - * / %` | two numbers of the same type | that type |
| `+` | `string` / `&string` + `string` / `&string` | `string` |
| `& \| ^` | two integers of the same type | that type |
| `<< >>` | integers | left type |
| `&& \|\|` `!` | `bool` | `bool` |
| `== !=` | same type: numbers, strings, `bool` | `bool` |
| `< <= > >=` | same type: numbers, strings | `bool` |

Structs and enums have no `==` in v0 (needs an equality interface).

## Places and mutability (ADR 0017)

Assignment and `&mut` need a mutable place: rooted in a `var`, or reached
through `&mut`. Parameters, `self`, loop variables and pattern bindings are
immutable (E3015 / E3016). Calling a `&mut self` method borrows the receiver
mutably.

## Methods

Lookup on the receiver's type after removing references: inherent methods
(`fn T.m`), then impl methods; for a type parameter, the methods of its bound
interfaces; for `any I`, the methods of `I`. Receivers are adjusted
automatically: `&self` borrows a place, `&mut self` borrows mutably (requires
a mutable place or `&mut`), `self` moves (through a reference only for copy
types, E3030).

**Provisional intrinsics** until the stdlib exists: `len()` and `is_empty()`
on `string`, arrays and slices; `clone()` on `string`; `sqrt()`/`abs()` on
floats; `abs()` on signed integers.

## Patterns (ADR 0014, 0018)

Unqualified capitalized names resolve against the scrutinee's enum here
(E3019 unknown variant). Bindings reached through a reference follow ADR 0018:
copy types bind by value, others by reference.

## Exhaustiveness (conservative)

- Enums: every variant needs an unguarded arm whose sub-patterns are all
  irrefutable, or a catch-all arm.
- `bool`: `true` and `false`, or a catch-all.
- Everything else: a catch-all arm.
- Guarded arms and nested refutable patterns never count as covering: some
  exhaustive matches are rejected (add `_`). Precise nested analysis later.

## Returns

A function with a non-`void` return type must not reach the end of its body
(E3009). A statement *diverges* if it is a `return`, a call returning `never`
(`panic`), an `if` whose every branch diverges, an exhaustive `match` whose
every arm diverges, or `for { }` without a `break`.

## `try`

`try e` on `Result<T, E>` in a function returning `Result<_, E>` yields `T`;
on `Option<T>` in a function returning `Option<_>` yields `T`. The error types
must be the same: **no automatic conversion in v0** (E3017).

## Unsafe

Calling an `extern` function requires an enclosing `unsafe` block (E3031).

## Opaque standard library

Members of standard modules (`fs.read_text`) have the opaque type `<std>`,
which is compatible with everything and never reported. This keeps examples
that use the future stdlib checkable for everything else. It is a hole by
design and disappears when the stdlib declares real signatures.
