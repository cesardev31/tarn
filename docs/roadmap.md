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
| 8 | Native feature completeness (ADRs 0028–0029) | **done within documented subset** |
| 9 | Borrowed dynamic interfaces and semantic contracts (ADRs 0030–0031) | **done within documented subset** |
| 10 | Callable invocation and owned closures (ADR 0032) | **done within documented subset** |
| 11 | Safe native concurrency (proposed ADR 0033) | **11A/11B implemented: pthread tasks, capabilities and scoped loans; 11C pending approval** |
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
[ADR 0029](adr/0029-checked-shifts-and-float-casts.md). Borrowed dynamic dispatch now executes (ADR 0030); owned dynamic objects remain
rejected. Optimization remains deferred. Phase 10 adds shared, mutable and consuming callable modes, `move fn` ownership captures and safe returned owned closures (ADR 0032).


## Dependency management and supply-chain security (design only)

[Dependency security](dependency-security.md) sets requirements for future
integrated `tarn` package commands, version-free imports, manifest/lock separation,
verified immutable content and explicitly constrained build authority. No package
manager, registry, resolver or sandbox implementation is scheduled by this update.
SemVer resolution, incompatible-major coexistence, release-age default,
signing/provenance formats, federation, sandbox enforcement and features remain
open. Optimization and concurrency remain deferred.
