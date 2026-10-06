# Compiler architecture

```
Source → Lexer → Parser → AST → Resolve → Types → Typed IR
    → Move/init (6A) → Borrows (6B) → Post-drop IR (6C)
    → Cranelift backend → ELF object → system cc + runtime → executable
```

## Crates (Cargo workspace)

| Path | Crate | Role | Status |
|------|-------|------|--------|
| `compiler/diagnostics` | `tarn_diagnostics` | `SourceMap`, `FileId`, `Span`, `Diagnostic`, text + JSON rendering | done (v0) |
| `compiler/lexer` | `tarn_lexer` | tokens with spans, preserved newlines, comment trivia | done (v0) |
| `compiler/ast` | `tarn_ast` | syntax tree with spans + `NodeId`s, S-expr dump | done (v0) |
| `compiler/parser` | `tarn_parser` | recursive descent + precedence climbing, error recovery | done (v0) |
| `compiler/resolve` | `tarn_resolve` | scopes, symbols, `NodeId` side tables (docs/resolution.md) | done (v0) |
| `compiler/types` | `tarn_types` | type checking, local inference, mutability, exhaustiveness (docs/types.md) | done (v0) |
| `compiler/driver` | `tarn_driver` | load modules from disk, run phases, sort diagnostics | done (v0) |
| `compiler/ir` | `tarn_ir` | typed CFG IR, lowering, verifier, printer (ADR 0023) | done (v0) |
| `compiler/ownership` | `tarn_ownership` | 6A move/init + drop decisions (ADR 0024), 6B borrows + provenance (ADR 0025), 6C executable drop elaboration (ADR 0026) | done (v0) |
| `compiler/backend` | `tarn_backend` | post-drop IR → Cranelift → ELF object/link | initial Linux x86_64 subset (ADR 0027) |
| `stdlib/core` | — | `core.tarn`: prelude declarations, embedded in the compiler (ADR 0020) | started |
| `runtime` | — | embedded C runtime: primitive print, strings, panic/abort; libc startup | initial (ADR 0027) |
| `tools/cli` | `tarn` | the single CLI | started |
| `tools/fmt` | `tarn_fmt` | canonical formatter | planned |
| `tools/lsp` | `tarn-lsp` | stdio LSP: diagnostics, hover, definition, unsaved buffers | initial |

Rules:

- Dependencies only point "down" the pipeline. `diagnostics` depends on nothing.
- Only `backend` knows about Cranelift. The frontend never imports it.
- The parser produces syntax only (grammar: `docs/grammar.md`). Name binding, types and ownership state live
  in side tables keyed by node ids, never inside the AST.
- Every token, node and IR instruction carries a `Span`.
- Platform-specific code (Linux syscalls, linker invocation) lives in `runtime`
  and in a `platform` module of the driver, not in the frontend.

## Dependency policy

Each external crate needs a line in this table with a justification.

| Crate | Used by | Why |
|-------|---------|-----|
| `serde_json` | tools/lsp | JSON-RPC messages and robust JSON encoding/decoding |
| `url` | tools/lsp | Correct file URI encoding/decoding, including escaped paths |
| `cranelift-codegen/frontend/module/object/native` | backend | ISA/codegen, SSA builder, symbols, ELF object emission; pinned 0.125.3, dependency audit in ADR 0027 |

## Linking

Cranelift ELF object + embedded `runtime/native.c`, compiled/linked through system
`cc -std=c11 -O0 -fno-strict-aliasing -no-pie ... -lm`. Linux x86_64 only;
requires a C toolchain with libc/libm headers. No installed runtime archive needed.
ABI, canonical layout, output paths and limits: [ADR 0027](adr/0027-native-backend.md).

## Caching (planned, phase 17)

Global cache at `~/.tarn/cache/`, keys = hash(compiler version, target, source
hash, dependency interface hashes, flags). Per-module first; finer later only
if measurements justify it.

## Executable destruction boundary

After successful ownership checking, the driver produces `CheckResult.drops`: a
separate `tarn_ir::post_drop::Program` with explicit destruction plans, runtime
flags and verified CFG edges. A backend consumes it without reading move/borrow
results. See [ADR 0026](adr/0026-drop-elaboration.md). Inspect it with
`tarn ir file.tarn --drops`. Native code generation consumes this boundary; see [ADR 0027](adr/0027-native-backend.md).

Native feature completion after phase 7 adds reachable generic specialization,
concrete generic ADTs, borrowed fat slices, nonescaping borrowed closures and
checked shifts/float-to-int casts. Unknown bootstrap stdlib APIs now fail with
E3040. See [ADR 0028](adr/0028-native-feature-completeness.md) and
[ADR 0029](adr/0029-checked-shifts-and-float-casts.md). Optimization remains deferred; callable locals retain current consuming semantics.
Borrowed dynamic references use private `(data, vtable)` pairs and resolved
frontend implementation IDs (ADR 0030). Declaration-derived passing/result
contracts and `borrows(...)` clauses expose invisible implementation promises
(ADR 0031); the backend still receives no move/borrow results.
