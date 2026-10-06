# ADR 0014 — In patterns, capitalized names are variants

Status: **experimental v0** (2026-10-05). Language rule affecting every
`match`; kept provisionally, explicitly *not* a permanent guarantee. Reviewed
before phase 4 (see "Future interactions").

## Problem

Language.md lets patterns name variants of the scrutinee's enum without
qualification (`Circle(r)`, `Empty`). A bare identifier pattern is then
ambiguous between "unit variant" and "new binding", and the resolver cannot
use the scrutinee's type (types come later). Rust resolves it by lexical
lookup, which silently turns a misspelled variant into a catch-all binding
(`Emtpy => ...` matches everything) — a classic bug.

## Options

1. **Lexical lookup** (Rust): binding unless a variant with that name is in
   scope. Unqualified variants are *not* in lexical scope in Tarn, so this
   would make `Empty` always a binding. Rejected.
2. **Defer everything to the type checker**: resolver cannot declare bindings
   until types are known; scopes become type-dependent. Rejected: couples
   phases, breaks "names first, then types".
3. **Always qualify variants in patterns** (`Shape.Empty`): unambiguous but
   verbose, and contradicts the existing design.
4. **Case rule**: an identifier starting with an uppercase ASCII letter is a
   variant; anything else (lowercase, `_x`) is a binding. Variant
   declarations must be capitalized (E2017).

## Decision: option 4

- Unqualified capitalized pattern names resolve to a lexically visible
  variant (prelude `Some/None/Ok/Err`) or become
  `Res::ScrutineeVariant(name)` for the type checker to look up in the
  scrutinee's enum. A wrong name is then a *type error* ("`Shape` has no
  variant `Emtpy`"), never a silent catch-all.
- Lowercase call-like patterns (`foo(x)`) are E2018.
- Purely local and syntactic: resolver, formatter, LSP and agents classify a
  pattern without types. Go already uses case for a semantic rule
  (visibility), so it is familiar to readers.

## Costs accepted

- Variants must be capitalized (E2017). Bindings in patterns cannot be
  capitalized (`N => ...` is read as a variant).
- Non-ASCII initial letters are not identifiers yet (ASCII-only), so the rule
  is well defined today; revisit if Unicode identifiers are added.

## Evidence that would change this

If real code needs capitalized bindings in patterns (e.g. constants used as
patterns), consider letting a lexically visible `const` win — that would be an
extension, not a reversal.

## Future interactions (review before phase 4)

The rule is better stated as: **a capitalized pattern name refers to something
that already exists; a lowercase pattern name introduces a binding.** Read that
way, the cases that looked risky fit naturally:

| Case | Behavior under the rule | Verdict |
|------|------------------------|---------|
| Constants in patterns (`MAX_SIZE => ...`, future) | Capitalized → "existing name". Lookup order: lexically visible `const` first, then the scrutinee's variants. A constant that collides with a variant name is reported as ambiguous. | Fits; an extension, not a reversal. |
| `EOF`, `MAX_SIZE`, `HTTP` as variant names | Capitalized → variants. Allowed (E2017 only demands an uppercase first letter, not CamelCase). | Fits. |
| Lowercase constants (`max_size`) in patterns | Would be read as bindings. Must be qualified (`config.max_size`) or renamed. | Accepted limitation; SCREAMING_CASE constants are the convention anyway. |
| Variants with other conventions (C enums via FFI: `c_int`, `SIGINT`) | A lowercase variant cannot be declared (E2017). FFI enums will be mapped to Tarn names by the binding generator, or matched qualified. | Limitation on *declaration*, not on matching qualified paths. |
| Qualified patterns (`Shape.Empty`, `geo.Shape.Empty`) | Not affected: a path with a `.` is always a reference, its last segment may have any case. | Unaffected; also the escape hatch for every edge case above. |
| Capitalized bindings (`N => ...`) | Impossible; read as a variant/constant. | Accepted; a binding named like a type is confusing in any language. |

No structural limitation found: every problematic case has a qualified-path
escape hatch, and the one future feature (constants) slots into the
"capitalized = existing" side without changing the meaning of existing code.

Evidence that would reopen it: FFI-heavy xlinux code needing lowercase
variants frequently, or users reporting confusion between constants and
variants in patterns.
