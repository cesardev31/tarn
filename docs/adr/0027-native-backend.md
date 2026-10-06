# ADR 0027: Cranelift AOT backend and internal runtime ABI v0

Status: accepted for phase 7, Linux x86_64 only.

Historical phase-7 boundary. ADR 0028 extends specialization, slices and callables;
ADR 0029 supersedes the shift and float-to-integer limitations below.

## Boundary and architecture

`compiler/backend` consumes `tarn_ir::post_drop::Program` and the type declaration
catalog for layout. It never receives MoveResults, BorrowResults, loans or
provenance. Moves, copies, references, calls, control flow and destruction plans
are already decided. The original frontend remains independent of Cranelift.

The backend walks reachable direct functions from `fn main()`, computes layouts
and internal signatures, lowers each IR block to Cranelift, verifies the generated
functions, emits an ELF object through ObjectModule, and links with system `cc`.
Unused stdlib declarations and generic helper bodies are not compiled. Reachable
unsupported code returns `backend not implemented: ...`; structural or ABI
inconsistencies return `compiler bug in native backend: ...`. No diagnostic code
is repurposed for these toolchain limitations.

Cranelift's FunctionBuilder constructs SSA internally. Unaddressed scalar locals
use SSA variables; address-taken scalars and aggregates use stack slots. No SSA
assumptions are exposed to the frontend. StorageLive/Dead are semantic lifetime
markers, not allocation/free operations for stack slots. SSA slots receive
arbitrary zero entry definitions; valid IR does not read uninitialized source
values, and runtime drop guards prevent dead-place reads. This accommodates
non-SSA joins without consulting ownership state.

## Dependencies

The five direct external dependencies are pinned to Cranelift 0.125.3:
`cranelift-codegen`, `cranelift-frontend`, `cranelift-module`, `cranelift-object`,
`cranelift-native`. Codegen is configured for std and host-arch without default
features. Together they provide instruction selection, SSA construction, native
ISA selection, symbol declarations/relocations and ELF output. Reimplementing
these facilities in the standard library would be a new compiler backend and
object writer, outside this phase. There are no convenience dependencies, JIT,
LLVM or Wasmtime runtime dependencies. The resolved lockfile adds 37 external
packages (including build dependencies); Cranelift brings the register allocator,
assembler, object writer, layout containers and math helpers. Compatible internal
Cranelift components resolve to 0.125.4; direct APIs remain pinned. `cargo tree -p
tarn_backend` records the exact dependency tree.

## Canonical layout

`compiler/backend/src/layout.rs` is the sole native layout authority. The type
checker and IR represent semantic declaration-index fields; they do not choose
physical offsets. Both place lowering and drop glue consume this layout layer.

Target: little-endian Linux x86_64, 64-bit pointers, System V calling convention.

| Type | Size | Alignment / representation |
|------|------|----------------------------|
| bool | 1 | 1; canonical 0 or 1 |
| i8/u8, i16/u16, i32/u32, i64/u64 | 1, 2, 4, 8 | natural alignment |
| isize/usize | 8 | 8 |
| f32/f64 | 4/8 | 4/8, IEEE floating point |
| void/never | 0 | 1; no ABI value |
| thin reference | 8 | 8; pointer to the referred value's storage |
| string | 8 | 8; unique owned pointer to a runtime string allocation |
| struct | computed | declaration-order fields, individually aligned, final tail padding to maximum field alignment |
| enum | computed | u32 declaration-index tag at offset 0, then aligned payload union; payload fields in declaration order |
| array | count × element size | element alignment; stride includes element's tail padding |

Generic ADT arguments are substituted when computing concrete layouts. Recursive
by-value layouts, unsized/fat references, type parameters, dynamic interfaces,
function/closure values and opaque resources are unsupported. Zero-size values
have no ABI lanes, including empty structs. Current limits are 64 KiB per value
and 4096 elements per fixed array: explicit correctness-first copy/drop expansion
must not explode for huge valid types. These are reported limitations, not silent
layout changes. Padding is never a resource and has no destruction semantics.

## Internal function ABI

This is **not a public stable C ABI for Tarn aggregate functions**.

- All calls use System V through Cranelift signatures.
- Integers/bools/floats/thin references/string pointers pass as their exact scalar
  type. Signedness selects operations and printing, not implicit conversion.
- Aggregate parameters pass a pointer; the callee copies bytes into its own
  layout-sized slot. A Copy operand preserves the caller's value; Move transfers
  ownership as already described by post-drop IR. No reference into this temporary
  incoming argument storage may escape (the existing frontend enforces this).
- Aggregate results use a first hidden destination pointer; the callee copies its
  return place there and returns no scalar result. Caller/callee use the same
  canonical layout. Zero-size parameters/results are omitted.
- Direct function IDs map to private object symbols `tarn_fn_<id>`. Arity, argument
  types and destination/return types are checked before emitting the call.
- An exported C `main` shim calls Tarn's zero-parameter, void-returning main and
  returns C integer 0. libc supplies process startup and shutdown.
- Generic function monomorphization, extern function ABIs, function pointers,
  virtual dispatch, closures and spawn are deferred.

## Place and CFG lowering

Local, Field, Deref, Index (fixed arrays) and Downcast projections are supported.
Field/downcast offsets come from layout, never from an independent table. Array
indices are checked at runtime and multiplied by the canonical stride. References
to scalar storage force only those locals to stack slots. Slices and fat references
are explicitly unsupported.

Goto, Switch, Call, Return and Unreachable lower the IR's actual control flow;
there is no reconstruction of if/for/match. Switch uses typed equality branches;
SSA sealing occurs after all original and generated drop/checked-operation edges
are inserted. Unreachable traps. Recursive and mutually recursive direct calls
are declared before definition.

Integer +, -, * check signed/unsigned overflow at the exact source width in every
build profile. Division and remainder reject zero; signed minimum divided or
remaindered by -1 aborts. Negation/abs reject the signed minimum. Comparisons use
the operand's signedness. Boolean short-circuit evaluation is already lowered to
IR branches. Bitwise and/or/xor/not are supported. Shifts are unsupported pending
native validation of their exact language policy.

f32/f64 +, -, *, /, comparisons, negation, abs, sqrt and remainder execute at their
source type; remainder uses the small runtime's fmodf/fmod wrappers. Integer-to-
integer conversions check target range before narrowing/sign-changing;
integer-to-float and float-width conversions are explicit IR casts. Float-to-
integer checked conversion is deferred and rejected, rather than silently using
Cranelift's saturating/trapping behavior as Tarn policy.

## Runtime ABI and owned values

`runtime/native.c` is embedded in the compiler and compiled by system cc during
linking. It has no VM, scheduler, GC, unwinding or user destructor hooks.

A runtime string is an allocation containing `uint64_t len` followed by exactly
`len` UTF-8 bytes (no NUL requirement). A Tarn string slot holds its unique
allocation pointer. Literal evaluation allocates a fresh string, Move transfers
it, `string.clone` allocates a copy, and Value destruction calls free exactly
once. `string.len` and `is_empty` observe a borrowed string. String arithmetic and
comparison intrinsics are not yet supported.

Runtime symbols:

- `tarn_rt_string(bytes, len) -> pointer`: allocate/copy literal or clone bytes;
- `tarn_rt_drop_string(pointer)`: release owned allocation;
- `tarn_rt_print_i64/u64/f64/bool/string`: print one value and newline;
- `tarn_rt_panic(string)` and `tarn_rt_fault()`: write stderr, flush and abort;
- `tarn_rt_rem_f32/f64`: libm remainder wrappers.

Primitive print receives Copy scalars by value. Owned strings arrive through the
frontend's explicit reference; the backend loads and observes them. Printing
never consumes ownership. Narrow signed/unsigned integers are extended only to
the runtime print lane; f32 is promoted only for the f64 printing helper.

Allocation failure aborts. Panic and checked-operation faults terminate the
process via abort, with no unwinding or stack drops. SIGABRT produces nonzero
`tarn run` status. `TARN_TRACE_DROPS=1` enables stderr `drop:<bytes>` lines before
string free for compiler tests; it is opt-in observation, not a destructor API.

## Executable destruction

All five post-drop operations are implemented for supported layouts:

- Value: unconditional type-directed resource destruction;
- Guard: runtime boolean branch before any nested place read;
- Fields: only explicitly listed children, preserving order;
- Variants: tag dispatch, then only the active payload plan;
- Remaining: bitmap tests, then only still-owned array elements.

Value glue recursively destroys struct fields and active enum payloads in
increasing declaration order; arrays use increasing indices. Thin references
own nothing. Flag writes/bitmap fills/element clears follow post-drop operations
literally. No backend inference of whether a place is initialized is permitted.

## Object, linking and CLI

Requirements: Linux x86_64, system `cc` with C11 headers/libc and libm development
files, and a working system linker. No external Tarn runtime installation is
needed. The linker command is equivalent to:

```
cc -std=c11 -O0 -fno-strict-aliasing -no-pie program.o runtime.c -lm -o program
```

`tarn build path/file.tarn` writes `path/file` beside the source. `-o path` selects
an explicit output, including paths with spaces. Source overwrite is rejected.
Codegen/link failures preserve any existing executable. Intermediates use a
unique process-local temporary directory and are removed by RAII.

`tarn run path/file.tarn` builds to the temporary directory's
`tarn-run-<compiler-pid>`, executes with inherited stdin/stdout/stderr, removes it,
and propagates normal exit status or 128+signal. It does not write beside source.
No shell parses compiler/linker arguments. Object emission is also exposed through
`emit_object`; executable construction through `build`.

## Verification, tests and limitations

The backend first runs the post-drop verifier. Layout/signature/place/call checks
reject unsupported valid constructs or impossible ABI mismatches. Cranelift
verification checks every generated function before object definition. Neither
phase rechecks loans or move state.

Real executable tests cover the 42 milestone, primitive/string printing, all
integer widths/sign behavior, arithmetic/floats, checked failures, direct/recursive
calls, if, loop/continue, return, locals, Copy and owned structs, mutable references,
enums/match, arrays, zero-size aggregates and selected core intrinsics. Public CLI
tests cover default/explicit paths, ELF output, run, panic, source protection and
preserving an existing executable on failure. Backend mutation tests delete each
native-fixture line and require no panic or invalid generated code.

Native drop tests reuse the **same** resource-token interpreter as 6C and compare
actual stderr destruction traces and abort status on all 54 boolean paths of its
25 fixtures. A separate distinct-string fixture checks destruction order, partial
move, conditional reinitialization and live overwrite. Frontend, 6A/6B/6C,
memory_safety and frontend mutation suites remain regression gates.

Defensible choices: independent post-drop backend, one layout layer, selective
SSA/stack storage, explicit private aggregate ABI, real allocated strings to test
destruction, standard object/linker pipeline, and graceful unsupported-feature
errors. Open tradeoffs: replacing bytewise aggregate copies and unrolled drop
plans with loops/glue functions; stable FFI aggregates; float conversion policy;
PIE; faster runtime build reuse; richer source-position runtime panic messages.
All are deferred. No optimization passes, LTO, PGO, JIT, LLVM, platform expansion,
async runtime or custom destructors were added. Opaque stdlib bootstrap behavior
still prevents claiming complete end-to-end language memory safety.
