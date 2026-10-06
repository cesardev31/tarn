# ADR 0010 — Inferred reference provenance; no lifetime syntax

Status: accepted (2026-10-05). Formalizes ADR 0007.

ADR 0031 extends the bodyless rules with a checked `borrows(...)` clause.
Body provenance and lifetimes remain inferred; the historical explicit-provenance
deferral below is superseded for declarations without bodies. Native extern C
reference/aggregate execution remains unimplemented.

Lifetimes remain an internal concept with **no public syntax**. Instead of
lifetime parameters, every function that returns a reference has a
**provenance**: the set of its reference parameters the returned reference may
borrow from. Provenance is part of the function's *semantic interface*.

## Definitions

- *Reference parameter*: a parameter (including the receiver `&self` /
  `&mut self`) whose type is or contains a reference.
- *Candidates*: the reference parameters of a function.
- `prov(f) ⊆ candidates(f)`: the provenance of `f`'s returned reference.

## Functions with a body

`prov(f)` is **inferred from the body**: the parameters whose borrows can flow
into any returned value, computed on the IR by a dataflow analysis. Calls to
other functions use their provenance (transitively). Recursive functions are
solved as a fixed point per strongly connected component, starting from ∅.

- A returned reference that may derive from a local or a temporary is
  rejected: **E4201** `returned reference may outlive its owner`.
- `prov(f) = ∅` while returning a reference is only possible with `'static`
  data (future: string literals, statics); until then it is E4201.

Callers treat the result as borrowing from every argument passed in a
provenance position.

## Declarations without a body

Interface methods, `extern` declarations and (future) function types have no
body, so provenance is taken from the signature, *only when unambiguous*:

1. **Receiver rule**: if there is a `&self` or `&mut self`, `prov = {self}`.
2. **Single candidate**: if exactly one parameter is a reference,
   `prov = {that parameter}`.
3. **Otherwise** (several candidates, no receiver) the declaration is rejected:
   **E4202** `cannot infer where the returned reference comes from`, with a
   help suggesting to return an owned value or to split the function.

Implementations must conform: an `impl` method's inferred provenance must be a
subset of the provenance its interface declaration implies (**E4204**).

`extern "C"` functions may not return Tarn references at all (they return raw
pointers, `*T`, under `unsafe`).

## Interface and caching

Provenance is printed by `tarn check --json` / `tarn symbols --json`, and is
included in the module's interface hash: changing a body so that its
provenance changes is an interface change and invalidates dependents. This is
deliberate — it is the same information a lifetime annotation would carry,
made visible by tools instead of syntax.

## Not now

- References stored in struct fields (E4203). Revisit with xlinux evidence.
- Explicit lifetime/provenance syntax. Only if real code shows E4202 is
  common and the workarounds are worse than syntax.
