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
| 11 | Safe native concurrency (ADR 0033) | **done within documented v0 subset: tasks, capabilities, scoped loans, Mutex guards and sequentially consistent atomics** |
| 12A | Blocking native TCP/UDP, owned sockets, resolution and Result errors (ADR 0034) | **done within documented blocking Linux v0 subset** |
| 12B | Nonblocking I/O and level-triggered epoll (ADR 0035) | **done within documented Linux v0 subset** |
| 12C | Manual suspended operations and single-thread execution (ADR 0036) | **done within documented bootstrap limits; no async syntax** |
| 13 | Source async lowering over Phase 12C (ADR 0037) | **done within documented v0 limits; see [the Phase 13 report](phase-13-report.md)** |
| 14A | Vec, cooperative async tasks, AsyncTask join and executor storage (ADR 0038) | **done; [report](phase-14a-report.md)** |
| 14B | Monotonic timers and timeouts (ADR 0039) | **done; [report](phase-14b-report.md)** |
| 14C | Buffered async I/O and a concurrent buffered server (ADR 0040) | **done; [report](phase-14c-report.md)** |
| 14R | Trusted stdlib module layers: io, time, net, runtime (ADR 0041) | **done; [report](phase-14r-report.md)** |
| 15A | Owned UTF-8 string essentials (ADR 0042) | **done; [report](phase-15a-report.md)** |
| 15B | Owned blocking filesystem (ADR 0043) | **done; [report](phase-15b-report.md)** |
| 15C | Owned lexical paths (ADR 0044) | **done; [report](phase-15c-report.md)** |
| 15D | Owned blocking child processes (ADR 0045) | **done; [report](phase-15d-report.md)** |
| 16 | Bounded async HTTP/1.1 server over buffered I/O | **done within documented server subset**: [16A–F plan](phase-16-plan.md), [report](phase-16-report.md), [ADR 0046](adr/0046-bounded-async-http.md) |
| 17 | Application ergonomics: string.Builder, json, http.serve and response helpers (ADR 0048) | **done**: [plan](phase-17-plan.md), [report](phase-17-report.md) |
| 18 | Minimal native C FFI: scalar extern calls, raw pointers, `--link`, SQLite acceptance (ADR 0047) | **done within documented v0 limits**: [plan](phase-18-plan.md), [report](phase-18-report.md) |
| 19 | CLI: command surface, `test`, `--watch`, `clean` | implemented (19A/B/C); `clean` explicitly deferred; [plan](phase-19-plan.md), [ADR](adr/0049-integrated-test-runner.md) |
| 20 | Formatter (basic) | **done within documented basic subset**: [plan](phase-20-plan.md), [report](phase-20-report.md), [ADR 0050](adr/0050-conservative-source-formatting.md) |
| 21 | VS Code extension (TextMate) | **done within documented editor subset**: [plan](phase-21-plan.md), [report](phase-21-report.md); highlighting, snippets, compiler LSP and shared formatting |

| 22 | Verified pure-source package manager | implemented initial subset: [guide](packages.md), [ADR 0051](adr/0051-verified-source-packages.md); public registry infrastructure and signed provenance deferred |
| 23 | Explicit error conversion: core combinators, variant constructors as functions, then measured syntax decision | **done**: [plan](phase-23-plan.md), [report](phase-23-report.md), [ADR 0052](adr/0052-explicit-error-conversion.md) |

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


## Dependency management and supply-chain security

The initial manager implements integrated commands, manifest/lock separation,
SemVer resolution, immutable local publication, HTTPS consumption, verified
content-addressed source storage and dependency-scoped imports. See
[package management](packages.md). [Dependency security](dependency-security.md)
remains the long-term contract. Public registry deployment, signing/provenance,
federation, build sandboxes, features and incompatible-major coexistence remain
open. Dependencies receive no build execution authority.

Blocking networking is covered by Phase 12A and [ADR 0034](adr/0034-blocking-networking.md);
readiness by [ADR 0035](adr/0035-nonblocking-readiness.md) and
[the Phase 12B report](readiness-report.md).
The manual execution model is covered by [ADR 0036](adr/0036-suspended-execution.md)
and [the Phase 12C report](suspended-execution-report.md); source-level async
lowering by [ADR 0037](adr/0037-source-async-lowering.md) and
[the Phase 13 report](phase-13-report.md).
