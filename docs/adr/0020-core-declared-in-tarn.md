# ADR 0020 — `core` is declared in Tarn; intrinsics are an implementation detail

Status: accepted (2026-10-05).

## Problem

The phase-4 checker had a Rust table of methods (`len`, `sqrt`, ...) and
`Option`/`Result` were constructed in Rust. Every new API meant compiler code,
and tools could not see the signatures: documentation, the LSP and agents
would each need their own copy of a list that lives in a `match` statement.

## Decision

- `stdlib/core/core.tarn` is an ordinary Tarn module, embedded in the
  compiler and always part of the program. Its `pub` items form the prelude.
- `Option`, `Result` and `Copy` are declared there. The variants of
  `Option`/`Result` are usable unqualified (the one prelude rule left).
- APIs whose implementation must come from the compiler are declared with an
  ordinary signature and the `extern "intrinsic"` ABI:

  ```tarn
  pub extern "intrinsic" fn string.len(&self) usize
  ```

  No new syntax: `extern` already means "no Tarn body". `intrinsic` calls are
  safe (no `unsafe` needed), unlike `extern "C"`.
- Methods on primitive types (`fn string.len`) and `extern "intrinsic"` are
  only allowed inside `core` (E2011, E2027).

## Names the compiler still knows ("lang items")

| Name | Why it is still special | Plan |
|------|-------------------------|------|
| primitive types | the type system itself | permanent |
| `Option`, `Result` | `try`, prelude variants | permanent, but declared in core |
| `Copy` | the `copy` keyword implements it | permanent, declared in core |
| `print`, `panic` | `print` accepts several types without a `Display` interface; `panic` returns `never` | move to core once interfaces like `Display` exist |
| `channel`, `Channel`, `Sender` | concurrency runtime not designed | redesign in the concurrency phase |
| `Error` | placeholder (opaque) | replaced by a real stdlib type |
| `len`/`is_empty` on arrays and slices | `[]T` cannot be named as a method owner yet | needs an owner syntax for `[]T` (open) |

The intrinsic table in the type checker is closed: new APIs go to `core`.

## Consequences

- The type checker, `tarn` tools and future docs read the same signatures.
- `Option`/`Result` resolve to real declarations (go-to-definition works).
- The compiler must load and check `core` on every build; it is small, and
  will be cached with the global cache (phase 17).
