# Roadmap

Work proceeds in vertical slices. A phase is done only when it is functional,
tested and documented.

| # | Phase | Status |
|---|-------|--------|
| 0 | Design docs, examples, name, extension, workspace | **done** |
| 1 | Lexer | **done** |
| 2 | Parser + AST | **done** |
| 3 | Name resolution (incl. shadowing rules, ADR 0009) | **done** |
| 4 | Types (primitives, structs, enums, generics, interfaces, mutability) | **done** |
| 5 | Diagnostics (text + JSON, golden tests) | done for phases 1–4 |
| 6 | Typed IR (ADR 0023) | **done** |
| 6A | Move/init checking on the IR (ADR 0024) | **done** |
| 6B | Borrow checking on the IR (ADR 0025) | **done** |
| 6C | Drop elaboration (ADR 0026) | **done** |
| 7 | Cranelift backend, `tarn build`/`run` (ADR 0027) | **done for the initial Linux x86_64 subset** |
| 7A | Native feature completeness (ADRs 0028–0029) | **done within documented subset; dynamic dispatch deferred** |
| 8 | Structs | |
| 9 | Ownership (moves) | |
| 18 | CLI `build run test check fmt clean` | started (`lex`, `ast`, `check`, `resolve`) |
| 19 | Formatter (basic) | |
| 20 | VS Code extension (TextMate) | **started**: language + highlighting |

## Milestones

1. `add(20, 22)` program compiles to a native binary through the full pipeline.
2. Structs + move semantics; `print(a)` after `b := a` is rejected (E4001).
3. Borrowing basics with conflict detection.
4. Result/Option/enums/match/try.
5. fs/path/process ─▶ start porting `xlinux doctor`.

After milestone 2 plus CLI, fmt and VS Code basics (the "first big milestone"
definition of done): **stop and review** architecture, syntax and ownership
before borrowing, generics or a large stdlib.

## Open design questions

- Value-producing `if` (currently: no).
- Equality for structs/enums (needs an `Equal` interface).
- Error conversion in `try` (currently exact match only).
- Method-owner syntax for `[]T` / `[N]T` so `len` can move to `core` (ADR 0020).
- `Self` in interface signatures (needed by `Eq`, ADR 0022).
- Well-formedness of type annotations w.r.t. bounds (`x: Point<string>`).
- Exact iteration protocol for `for x in xs`.
- `try` error conversion (`Error.from`) vs. explicit mapping.
- Reachable monomorphization is implemented (ADR 0028); shared code remains a future measured alternative.
- Interfaces: explicit `impl I for T` chosen; revisit after xlinux.
- Does `for x in &[]T` yield `T` (copy types) or `&T`? Examples assume copies of copy types.

Native feature completion after phase 7 adds reachable generic specialization,
concrete generic ADTs, borrowed fat slices, nonescaping borrowed closures and
checked shifts/float-to-int casts. Unknown bootstrap stdlib APIs now fail with
E3040. See [ADR 0028](adr/0028-native-feature-completeness.md) and
[ADR 0029](adr/0029-checked-shifts-and-float-casts.md). Dynamic dispatch and
optimization remain deferred; callable locals retain current consuming semantics.
