# ADR 0016 — Impl coherence: the impl lives with the interface or the type

Status: accepted (2026-10-05).

## Problem

If any module could write `impl I for T`, two modules that do not know each
other could both implement `I` for `T`, and a program importing both would
have two incompatible meanings for `x.m()`. Rust prevents this with orphan
rules that grew complex (fundamental types, local type parameters, the
`#[fundamental]` escape, upstream-crate reasoning). Tarn needs the minimum
that guarantees **at most one implementation of `I` for `T` in any program**.

## Rule

`impl I for T` is accepted only if it is written in **the module that
declares `I` or the module that declares `T`** (E2023). In addition:

1. `T` must be a struct or enum (named type). `impl I for i32`,
   `impl I for &T`, `impl I for []u8` are not allowed in user modules (E2024).
   Phase 31 implements the core-only primitive exception described below
   ([ADR 0060](0060-general-purpose-foundations.md)).
2. Type arguments of `T` are binders, as in ADR 0015:
   `impl Shape for Pair<A, B>` implements `Shape` for every `Pair`. No
   specialized impls (`impl Shape for Pair<i32, i32>`) in v0 (E2022).
3. At most one `impl I for T` per (interface, type) (E2020). With rules 1–2
   this is a purely syntactic check: no overlap analysis needed.

Why this is sufficient: the modules that can declare `impl I for T` are
those declaring `I` or `T`; both must be imported by any module that can
name the pair; a program sees each module once; rule 3 then rejects
duplicates inside them. Uniqueness becomes a local check.

## Alternatives

- *Rust orphan rules*: more permissive at crate granularity (any module of the
  crate that defines `I` or `T`), plus generic blanket impls. We have no
  packages yet, and blanket impls require overlap checking. Not now.
- *Package-level rule* (any module in the same package as `I` or `T`):
  friendlier for large codebases (impls in an `adapters` module). Tarn has no
  package concept yet; when it does, relaxing module → package is backwards
  compatible (every program valid today stays valid). Tightening later would
  not be. Hence: start strict.
- *No rule, global uniqueness check at link time*: errors appear only when two
  libraries are combined, far from the code that caused them. Rejected.

## Consequences

- Implementing a std interface for a std type (e.g. a future `Display for
  i64` variant) is impossible in user code; the idiom is a wrapper struct.
- Primitives can only get interface impls inside `core`.
- No blanket impls (`impl<T: A> B for T`) in v0.

Evidence to revisit: real code forced into many wrapper types, or the package
system landing (relax to package granularity).
