# ADR 0022 — Equality of user-defined types goes through a capability

Status: accepted (2026-10-05) — decision of *direction*; not implemented.

- `==` / `!=` are defined in v0 for numbers, strings and `bool` only.
  Structs and enums are rejected (E3006).
- No structural equality is hard-coded: it would silently compare fields that
  should not be compared (caches, handles), and it would decide by accident
  what equality means for every type.
- Equality of user types will be an interface in `core` (working name `Eq`),
  implemented with an ordinary `impl Eq for T { fn eq(&self, other &T) bool }`.
  `a == b` then means `a.eq(&b)` when `T: Eq`.
- Automatic derivation (`derive`-like sugar) is explicitly postponed: first the
  normal implementation path must exist and be used; sugar is debated after.

Open: whether `Eq` needs `Self` in signatures (`other &Self`), which Tarn's
interfaces cannot express yet — that is the first real need for `Self`.
