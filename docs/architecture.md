# Compiler architecture

```
Source ─▶ Lexer ─▶ Parser ─▶ AST ─▶ Resolve ─▶ Types ─▶ Ownership/Borrow ─▶ IR ─▶ Backend ─▶ binary
                                                          (on IR CFG)          (Cranelift)
```

## Crates (Cargo workspace)

| Path | Crate | Role | Status |
|------|-------|------|--------|
| `compiler/diagnostics` | `tarn_diagnostics` | `SourceMap`, `FileId`, `Span`, `Diagnostic`, text + JSON rendering | done (v0) |
| `compiler/lexer` | `tarn_lexer` | tokens with spans, preserved newlines, comment trivia | done (v0) |
| `compiler/ast` | `tarn_ast` | syntax tree with spans + `NodeId`s, S-expr dump | done (v0) |
| `compiler/parser` | `tarn_parser` | recursive descent + precedence climbing, error recovery | done (v0) |
| `compiler/resolve` | `tarn_resolve` | scopes, symbols, `NodeId` side tables (docs/resolution.md) | done (v0) |
| `compiler/driver` | `tarn_driver` | load modules from disk, run phases, sort diagnostics | done (v0) |
| `compiler/types` | `tarn_types` | type checking, local inference | planned |
| `compiler/ir` | `tarn_ir` | typed CFG IR | planned |
| `compiler/ownership` | `tarn_ownership` | move/borrow analysis on IR | planned |
| `compiler/backend` | `tarn_backend` | IR ─▶ Cranelift ─▶ object | planned |
| `runtime` | `tarn_runtime` | `print`, `panic`, startup (staticlib) | planned |
| `tools/cli` | `tarn` | the single CLI | started |
| `tools/fmt` | `tarn_fmt` | canonical formatter | planned |
| `tools/lsp` | `tarn_lsp` | LSP on top of the frontend | later |

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
| *(none yet)* | | JSON output and CLI parsing are hand-written: small and dependency-free. |
| `cranelift-*` (planned) | backend | native code generation; the reason the backend exists |

## Linking (planned)

Debug builds: Cranelift object file + prebuilt `libtarn_runtime.a`, linked with
the system `cc` (Linux x86_64 only). Evaluate invoking `ld` directly later.

## Caching (planned, phase 17)

Global cache at `~/.tarn/cache/`, keys = hash(compiler version, target, source
hash, dependency interface hashes, flags). Per-module first; finer later only
if measurements justify it.
