# Phase 18 report: minimal native C FFI

Design: [ADR 0047](adr/0047-native-c-ffi.md). Plan: [phase-18-plan.md](phase-18-plan.md).
User guide: [C interop](ffi.md).

## What was implemented

- **18A**: native calls to `extern "C"` functions with System V scalar
  signatures, with sign or zero extension for narrow integers and bool.
  Non-scalar boundary types fail native builds and stay valid for ADR 0031
  contract checks (decision recorded in ADR 0047).
- **18B**: raw pointer types `*T` / `*mut T`; `unsafe fn` (E3071); unsized
  pointees rejected (E3072); trusted `ffi` layer with safe pointer
  intrinsics, `unsafe fn copy_bytes`/`string_from_c`, and `CString`.
- **18C**: `tarn build/run --link <lib>` as a validated build-time grant;
  linker failures summarized by missing symbol or library. Opaque C types
  need no syntax: empty structs behind pointers.
- **18D**: `examples/sqlite/sqlite.tarn`, a safe SQLite wrapper written in
  Tarn (no pointers or `unsafe` in its API), a demo, and the items CRUD over
  SQLite (`examples/sqlite/crud.tarn`).

## Tests

- Native pass cases `ffi_scalar` and `ffi_pointers` (real libc/libm), which
  are also covered by the line-deletion mutation test.
- Type fail cases: E3047 (pointer captured by a native task, inside a Copy
  struct), E3071, E3072. Parser case for pointer types and `unsafe fn`.
- CLI test: `--link m`, five rejected injection-like names, missing symbol
  and missing library messages, and (when libsqlite3 is installed) a direct
  SQLite call plus the full wrapper demo with exact output.
- The SQLite CRUD was exercised with curl: create, list, get, update, delete,
  repeated delete 404, invalid body 400, persistence across restart.
- Full workspace: see the final validation line below.

## Bugs found

1. With an inferred unsized `T`, `ffi.of(&slice)` would have returned the
   address of a temporary (pointer, length) pair instead of the data. Fixed
   in two places: the frontend rejects `*[]T`/`*any I` (E3072) and the
   backend rejects `of` on fat references.
2. Linker diagnostics were localized (Spanish on the reference machine), so
   summaries could not be produced. The linker now runs with `LC_ALL=C`.
3. Naming collision: an `address` parameter shadowed the `ffi.address`
   function (W2002) in the new module.

## Decisions I would defend

- The boundary check lives in the native build, not in the frontend: it is an
  ABI fact, and moving it would invalidate ADR 0031's contract tests for no
  safety gain.
- No dereference syntax and no pointer arithmetic: the only reads are two
  `unsafe fn` copies, which covered SQLite and libc string APIs completely.
- `sqlite3_close_v2` makes a safe wrapper possible without destructors or
  stored references: statements keep the connection alive inside SQLite.
- Linking stays a CLI grant; source code and imports cannot acquire native
  authority.

## Decisions I still question

- Empty structs as opaque types can technically be constructed and moved by
  value. This caused no problem in the wrapper because their constructors
  are private by convention, not enforced.
- `Db` and `Statement` leak their C resources when dropped without
  close/finalize. The CRUD keeps one connection for the process lifetime, so
  this did not matter here; programs that open connections per request would
  need verified foreign-resource destruction.
- `--link :libsqlite3.so.0` exposes a distribution detail to users. A future
  `tarn.toml` should own this.

## Known limitations

- No callbacks from C, variadic functions, structs by value, errno or
  thread interaction.
- **Ceremony evidence**: the SQLite CRUD needs a `try_response` helper,
  because the handler mixes `sqlite.Error` with fallible `http.Response`
  constructors, and `try` has no error conversion. This is the second
  independent program (after Phase 17's `saved`) where error conversion would
  remove code. It is now the strongest open language question.
- The wrapper lives in `examples/`, not the standard library (deliberate: no
  C dependency in the stdlib; a future package).

## Final validation

Recorded after the full `cargo test --workspace --no-fail-fast` run.
