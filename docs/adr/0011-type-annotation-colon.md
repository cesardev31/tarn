# ADR 0011 — Type annotations on bindings use `:`

Status: accepted (2026-10-05). Changes the binding forms of ADR 0006.

## Options

- **A** (draft 0): `x T := value`, `var x T = value`
- **B**: `x: T := value`, `var x: T = value`

## Evaluation

| Criterion | A: `x T :=` | B: `x: T :=` |
|-----------|-------------|--------------|
| Parser determinism | Statement start `IDENT <type-start>` is ambiguous with expressions (`x[0] = 1`, `a & b`, `f fn…`). Needs speculative parse of a whole type, then backtrack if no `:=`. It was the **only** backtracking in the parser, and it forced an undo log for split tokens (`>>`, `&&`). | `IDENT :` at statement start is decided with 2 tokens of lookahead; no statement or expression starts that way (no labels, no type ascription). LL(2), no backtracking. |
| Error recovery | A malformed type inside a typed binding (`x Option<i32 := 1`) makes the speculation fail silently and the line is re-parsed as an *expression*, producing a misleading error far from the cause. | After `x:` the parser *knows* a type follows; type errors are reported as type errors (E1003) at the right token. |
| Future type syntax | Every new type form (`*T`, tuples, `?T`, `[N]T` variants, `dyn`-like keywords, function types with effects…) must be checked against every expression that can start with `IDENT` + that token. Grows quadratically with features. | A new type form only has to be unambiguous *within type position*. Expressions and types evolve independently. |
| Formatter | Must re-derive the speculation to know whether `x [4]i64 := …` is a binding; spacing rules depend on it. | Token-level: `IDENT ":" type ":="` is recognizable without a full parse; canonical spacing `x: T := v`. |
| LSP | Partial input while typing (`x Opt|`) is an expression until `:=` appears: completion offers values, not types. | After `x:` the LSP knows to offer types — even with a broken rest of the line. |
| Agent tooling | Agents and regex-level tools cannot tell `a b` (error) from `a B := …` (binding) without a parser. | `name:` is a stable, greppable marker for "annotated binding". |
| Readability | Compact; reads like Go's `var x T`. Two adjacent identifiers (`limit u64`) can look like a typo. | Explicit; `limit: u64 := 100` reads "limit of type u64 is 100". Matches Rust, TS, Swift, Kotlin, Python annotations — the forms LLMs see most. |
| Coherence with Tarn | Same juxtaposition as parameters/fields (`a i32`). | Bindings differ from parameters/fields (see rule below). |
| Migration cost | — | Now: 4 example lines, 2 docs, a few tests. Later: every source file with an annotated binding plus every agent prompt/doc that learned the old form. |

## Decision: B

Typed bindings are written `x: T := value` and `var x: T = value`.
Unannotated forms are unchanged: `x := value`, `var x = value`.

Coherence rule, stated once so it stays predictable:

> **In statement position, a type annotation is introduced by `:`.
> In declaration lists — parameters, closure parameters, struct fields —
> the type follows the name directly (`a i32`), because there the grammar
> position already guarantees a type comes next.**

`:` now has three meanings, each in a distinct position: annotation
(`x: T :=`), generic bound (`<T: Writer>`), field initializer (`P{x: 1}`).
None of them can appear where another is valid.

## Consequences

- The speculative path and its undo log are removed from the parser: it is
  now fully deterministic (bounded lookahead, no backtracking).
- The old form gets a targeted diagnostic, E1018, with a fix-it, so humans and
  agents trained on draft 0 are corrected in one step.
- Parameters and fields keep `name Type`. If one day we want colons there too,
  that is a separate decision; this ADR does not depend on it.
