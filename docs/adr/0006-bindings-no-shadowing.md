# ADR 0006 — Immutable `:=`, mutable `var`, no shadowing

Status: **amended by ADR 0009** (2026-10-05): the shadowing rule below was relaxed;
the binding forms are unchanged.

- `x := v` / `x T := v` immutable; `var x [T] = v` mutable. Mutability is
  visible at the declaration and required for `&mut x`.
- Shadowing is forbidden in all scopes (E2003). Every name in a function has
  exactly one declaration, which helps readers, refactoring tools, and
  agents editing partial context. Cost: occasional renames (`text`, `text_trimmed`).
