# ADR 0054: less ceremony without runtime cost

Status: accepted for 26A–26C; 26D (string formatting) designed and deferred.
Phase 26; [report](../phase-26-report.md). Implements ADR 0022 (equality).

## Evidence

Patterns counted in 2,898 lines of real Tarn (CRUDs, SQLite wrapper, the
`xlinux doctor` port, the HTTP/JSON/OS/string/fs/process stdlib):

| Pattern | Count |
|---|---|
| Literal conversions such as `u16(200)`, already redundant | 399 |
| `&"literal"` passed where `&string` is expected | 121 |
| `var x: T` then `match` assigning it | 75 |
| `==` impossible on enums (E3006) | 2 programs |
| Text built with `+` concatenation | 22 lines |
| Number-to-text calls (`from_u64`, `push_u64`) | 12 |

## Decisions

1. **26A: remove redundant literal conversions** from the stdlib (449
   removed, compiler-guided: a removal stays only if the program still
   type-checks). Binding initializers such as `x := u16(0)` are kept, since
   there the conversion chooses the binding's type. No language change.
2. **26B: a string literal may be passed where `&string` is expected.**
   `f("GET")` is checked and lowered exactly as `f(&"GET")`: a temporary is
   borrowed (CoercionKind::BorrowLiteral). In a `let` with an explicit
   `&string` type the temporary lives for the binding's scope, as for
   `r := &"x"`. Only shared `&string`: never `&mut`, never other types.
   Same allocations and drops as the written `&`, so no runtime cost.
3. **26C: `Self` and `core.Eq`.**
   - `Self` inside an `interface` is an implicit type parameter; inside an
     `impl` it is the target type with its binders. Calls through a bound
     (`T: I`) bind `Self` to the receiver type.
   - A method that uses `Self` outside its receiver cannot be called through
     `any I` (E3073): a dynamic object has no single concrete `Self`.
   - `pub interface Eq { fn eq(&self, other &Self) bool }` in `core`.
     `a == b` on a struct, enum or type parameter implementing `Eq` lowers
     to `Eq.eq(&a, &b)`, resolved statically like any bound call; `!=`
     negates it. Operands are borrowed, never moved. Types without an `Eq`
     implementation keep E3006: no structural default, no derive.
   - `io.ErrorKind` implements `Eq`, so `error.kind == io.ErrorKind.NotFound`
     works.
4. **26D: string formatting is deferred.** The evidence (22 lines, 12
   conversions) is the weakest of the set, and the design has a breaking
   hazard: `{` already appears in string literals (JSON). If evidence grows,
   the proposed form is a distinct literal prefix, `f"{label}: {count}"`,
   accepting `string`, `&string`, integers and `bool`, lowered at compile
   time to one builder (no runtime parsing, no allocation per piece beyond
   number formatting). Plain literals keep their meaning.

## Not chosen

- **Value-producing `if`/`match`** for the 75 `var x: T` + `match` sites:
  an explicit v0 non-goal (AGENTS.md); combinators (Phase 23) remove part of
  it.
- **Implicit borrowing of named strings** (`f(name)` for `&string`): it
  would hide whether a value is moved or borrowed, which matters for
  ownership; literals have no owner, so borrowing them hides nothing.
- **Structural or derived equality**: ADR 0022 stands.

## Limitations and future evidence

- `Option<T>` and `Result<T, E>` do not implement `Eq`: that needs `impl`
  bounds (`impl Eq for Option<T: Eq>`), which coherence (ADR 0016) does not
  allow yet. Compare with `match` or `unwrap_or`.
- `Eq` for fieldless enums is written by hand (a match per variant); repeated
  boilerplate across many enums would be evidence for an explicit,
  per-type opt-in helper.
- Each string literal used to allocate when evaluated; Phase 27 made
  literals static (ADR 0055).
