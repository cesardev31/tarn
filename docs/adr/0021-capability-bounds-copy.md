# ADR 0021 — Minimal bounds; `Copy` as a capability

Status: accepted (2026-10-05).

## Problem

`copy struct Point<T> { x T  y T }` was impossible: a type parameter is never
known to be copy. More generally, generic types need to state simple
requirements on their parameters.

## Decision

- Reuse the existing bound syntax on **type declarations**:
  `copy struct Point<T: Copy>`, `struct Map<K: Hash, V>`. Same `<T: A + B>`
  as on functions; no new syntax.
- **Bounds are interfaces.** A *capability* is an interface with no methods
  (a marker). `Copy` is the first, declared in `core`.
- `Copy` is special in exactly one way: it is not implemented with `impl`
  (E2026). A type is `Copy` when it is a primitive, a shared reference, an
  array of `Copy`, a type declared `copy`, or a parameter bounded by `Copy`.
- **Implied bounds:** inside `fn Point<T>.m` and `impl I for Point<T>`, the
  binders carry the bounds declared on `Point` (ADR 0015 forbids repeating
  them on binders). Without this, no method of a bounded type could be written.
- Bounds are checked where a generic type or function is instantiated
  (struct literals, variant constructors, calls): E3022.

## Copy stays semantic (no size limit)

A type declared `copy` whose components are all `Copy` is `Copy`, whatever its
size. If large copies become a real performance problem, the answer is a lint
(e.g. "copying a 4 KiB value in a loop"), never a change in what `copy` means.

## Not now

- No trait solver: no associated types, no blanket impls, no negative bounds.
- `Eq` (equality) will be the next capability; see ADR 0022.
- Bounds on type annotations (`x: Point<string>`) are not checked yet, only
  instantiations. A well-formedness pass will cover annotations.
