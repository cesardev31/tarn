# ADR 0018 — Reading through a reference: copy if copy, borrow otherwise

Status: accepted (2026-10-05). Affects `for`, `match` and patterns.

## Problem

When a value is reached *through* a reference — iterating `&[]T`, matching on
`&Shape`, destructuring `&User` — what type do the bindings get?

```tarn
fn area(s &Shape) f64 {
    match s {
        Circle(r) => return r * r      // is `r` an f64 or a &f64?
    }
}
```

## Options

1. **Always a reference** (`r: &f64`). Uniform, but then `r * r` needs either
   auto-deref in arithmetic (implicit, a second "magic" rule) or `*r` syntax
   (Tarn has no deref operator yet). Every numeric loop over a slice becomes
   noisy.
2. **Rust default binding modes**: references, plus operator impls for `&i64`
   and auto-deref in many places. Works in Rust because of its trait system;
   Tarn has no operator traits.
3. **Copy if copy, borrow otherwise**: a binding reached through `&`/`&mut`
   gets `T` when `T` is a copy type, and `&T` / `&mut T` when it is not.
4. **Always by value**, rejecting non-copy: forbids the common case of reading
   a `string` field in a `match` on `&User`.

## Decision: option 3

- Copy-ness is a *declared* property (primitives, `&T`, `copy struct`,
  `copy enum`, arrays of copy), so the rule is predictable from declarations,
  not from use.
- The binding is exactly what you could have obtained without the reference:
  copying a copy value out of a borrow is always safe; a non-copy value can
  only be observed by reference.
- Same rule for `for x in &xs`, `match` bindings, struct-pattern shorthand and
  nested patterns. One rule, three places.

## Consequences

- `for v in values` over `&[]u32` gives `v: u32`; over `&[]User` gives
  `u: &User`.
- Making a struct `copy` later changes binding types from `&T` to `T` in code
  that matches on references to it. Code that only reads keeps compiling in
  most cases; code that stored the reference may need a change. Accepted:
  adding `copy` is a deliberate API change.
- Calling a by-value (`self`) method through a reference is allowed only for
  copy types (E3030), by the same reasoning.

Evidence to revisit: large structs marked `copy` for convenience causing
silent expensive copies in loops (would argue for a size limit on `copy`).
