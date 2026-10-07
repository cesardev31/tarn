# Phase 18 implementation plan: minimal native C FFI

Status: complete; [report](phase-18-report.md). Design decisions:
[ADR 0047](adr/0047-native-c-ffi.md).
Baseline: Phase 17 (application ergonomics, ADR 0048).

## 1. Goal and scope

Let ordinary Tarn code call C libraries on Linux x86_64 with scalar and raw
pointer arguments, link them through an explicit build-time grant, and prove the
result by wrapping SQLite in Tarn and porting the external HTTP CRUD to it.

Included: native extern calls, `*T`/`*mut T`, opaque C types, `ffi.CString`,
`--link`, boundary diagnostics, a Tarn-written SQLite wrapper as an example
and acceptance test.

Excluded: callbacks into Tarn, variadics, structs/unions by value, C headers or
bindgen, errno, dlopen, C threads interacting with Tarn tasks, foreign-resource
destructors, `tarn.toml`, package registry, Windows/macOS, a stdlib `sqlite`
module. Do not start Phase 19 work.

## 2. Current constraints

- `Ty` has no pointer kind; `docs/language.md` section 14 describes `*T` only
  as provisional text.
- `codegen.rs` declares only bodies and known intrinsics; external user
  symbols have no `FuncId`.
- The linker command is fixed; `cc` is the trusted toolchain boundary.
- On the development machine only `libsqlite3.so.0` is installed, without the
  unversioned dev symlink or `sqlite3.h`. Tarn declares prototypes itself, so
  headers are unnecessary; `--link` must accept an exact file name
  (`--link :libsqlite3.so.0`, passed as `-l:libsqlite3.so.0`).

## 3. Stages

### 18A: scalar extern calls (done)

- Declare each reachable `extern "C"` function as an imported Cranelift
  symbol with System V scalar signature.
- Unsupported boundary types (reference, slice, string, ADT, array, closure,
  `any`) fail the native build naming function and type; the frontend keeps
  accepting them for ADR 0031 contract checks (ADR 0047 decision 1).
- Reject variadic declarations (syntax has none; keep it that way).
- Tests: `getpid`, `abs`, `labs`, `sqrt` from libc/libm; mixed int/float
  argument registers; `void` results; E3031 still required.

Gate: native tests pass; no change to existing IR/borrow snapshots.

### 18B: raw pointers (done)

- Parser/AST/types for `*T` and `*mut T`; Copy, neither Transfer nor Share.
- Safe creation in the trusted `ffi` layer: `null`, `of`, `of_mut`, `slice`,
  `slice_mut`, `to_const`, `address`, `from_address`, `is_null`.
- `unsafe fn` (E3071) for readers `copy_bytes` and `string_from_c`; no deref
  syntax and no pointer offsetting until real code needs them. `*[]T` and
  `*any I` rejected (E3072).
- Borrow model: creating a pointer does not create a loan that survives the
  expression; tests document that use-after-free through raw pointers is
  only possible inside `unsafe`.
- Capability tests: a struct containing `*T` is neither Transfer nor Share;
  spawn rejects it with the existing diagnostic.

Gate: memory-safety contract suite unchanged; new pass/fail suites for pointer
rules; mutation tests do not panic.

### 18C: linking (done; opaque types not needed)

- Opaque C types: ordinary empty structs behind pointers; no new syntax
  (ADR 0047 decision 3).
- `stdlib/ffi` CString and string_from_c: done early in 18B.
- `tarn build/run --link <name>` repeatable; forwarded as `-l<name>` after the
  runtime; linker failures become a Tarn diagnostic listing undefined symbols.
- LSP/`check` ignore link flags (no native step).

Gate: libc `strlen`/`getenv` round trips; missing library and missing symbol
diagnostics have golden tests.

### 18D: SQLite acceptance (done)

- Example `examples/sqlite/` with a Tarn wrapper: `Db.open`, `exec`,
  `prepare`, `bind_text`/`bind_i64`, `step`, `column_*`, consuming
  `finalize` and `close`; errors as `Result<_, sqlite.Error>`.
- Native test gated on the presence of libsqlite3 (skipped, reported, when
  absent).
- Port tarn-crud from `items.db` text to SQLite; record ceremony relative to
  an equivalent Go program in the report.

Gate: CRUD passes the same curl scenario with SQLite; wrapper exposes no raw
pointers in its public API.

## 4. Diagnostics

Allocate new codes in the E30xx range (opaque type misuse, pointer deref
outside unsafe reuses E3031 wording style) and a driver code for
link failures. Document each in `docs/errors.md`; never reuse codes.

## 5. Report

Write `docs/phase-18-report.md` with the standard sections, especially leak
behavior without destructors and wrapper ceremony measured in the CRUD port.
