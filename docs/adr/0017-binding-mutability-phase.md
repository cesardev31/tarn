# ADR 0017 — Binding mutability is checked by the type checker, not ownership

Status: accepted (2026-10-05).

Two different questions, two phases:

| Question | Example | Phase |
|----------|---------|-------|
| *Can this place be written at all?* (mutability of the binding, or of the reference it is reached through) | `x := 1; x = 2`; `&mut x` with `x := …`; `p.x = 1` with `p: &Point` | **Type/semantic checking** (phase 4): local, no flow analysis, depends on declarations and types. |
| *Is writing it now compatible with other live borrows / moves?* | `r := &v; v = 2; print(r)` | **Ownership/borrow analysis** (phases 9–10), on the IR CFG. |

Rules (phase 4):

- A place is **mutable** if its root is a `var` binding, or the path from the
  root goes through a `&mut` reference (`self` in `&mut self`, a `&mut T`
  parameter, `*` auto-deref of `&mut`).
- Assignment requires a mutable place (E3015). `&mut place` requires a
  mutable place (E3016). Calling a `&mut self` method on a place auto-borrows
  it mutably and therefore requires a mutable place (E3016).
- Parameters (including by-value `self`) are immutable bindings. To modify a
  by-value parameter, copy it into a `var`.

Why here: the check is cheap, local and its errors are about declarations
(`add var`), which is what the type checker already reports. Keeping it out of
the borrow checker keeps that analysis about *time* (liveness), not about
*permissions*.
