# ADR 0012 — No explicit generic arguments in expressions (v0 restriction)

Status: accepted as a **v0 restriction, not a permanent language promise**.

In v0, generic arguments are written only in type position (`Option<i32>`,
`fn f<T>(…)`). Expressions cannot supply them (no `f<T>(x)`, no turbofish).
When inference cannot determine them, the programmer annotates a binding or a
parameter: `x: Option<i32> := None`.

Why now: `<` in expression position stays an unambiguous comparison, which
keeps the parser simple while the type system does not exist yet.

Why not forever: some APIs (`parse<T>()`, `size_of<T>()`, empty generic
constructors) are awkward without explicit arguments. Once the type checker
and real code (xlinux) show how often annotations are insufficient, we will
evaluate a syntax. Candidates must be decidable with bounded lookahead and
must not reintroduce backtracking (ADR 0011). Examples to evaluate then:
`f[T](x)`, `f.<T>(x)`, `f::<T>(x)`.

No syntax is reserved or implemented for it today.
