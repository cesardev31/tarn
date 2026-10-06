# ADR 0028: reachable specialization, fat slices and borrowed closures

Status: accepted implementation boundary after phase 7; Linux x86_64.

Callable invocation and ownership-capture decisions below are superseded by
[ADR 0032](0032-callable-ownership.md), phase 10.

## Boundary and specialization

The backend continues to consume post-drop IR and the semantic declaration
catalog only. `mono::specialize` creates a separate concrete post-drop program;
no move state, loan set or initialization analysis is consulted. The original
program remains available. Both programs pass the post-drop verifier.

An instance key is `(original FunctionId, concrete type arguments)`. A deterministic
FIFO walk starts at main, substitutes function signatures, locals, embedded type
arguments and call targets, and reserves each instance before walking its body.
Identical keys reuse an instance, including recursive and mutually recursive
calls. Only reachable instances and referenced closure/function bodies exist in
the resulting program. Private IDs follow traversal order. Hash maps never choose
symbol order. Existing type-directed destruction is specialized as well: concrete
Copy values require no resource glue, so their generic destruction is removed.
This is a type substitution, not another ownership analysis.

Dynamic, unsized by-value and unresolved substitutions fail explicitly. A borrowed
slice is a supported sized substitution. Limits are 256 reachable instances,
64 levels and 4096 nodes per type argument; expanding polymorphic recursion fails
with a backend limitation rather than hanging. Native aggregate layout also has
a 64-level ADT limit. These are correctness limits, not language layout rules.

Concrete generic structs and enums use the existing canonical layout authority,
including substitution in nested fields, arrays and variant payloads. Post-drop
plans retain their declaration order and guards. No independent layout table or
monomorphization in the frontend is introduced.

## Slices

A `&[]T` or `&mut []T` is 16 bytes, aligned to 8: data pointer at offset 0,
u64 element length at offset 8. It passes through the private aggregate ABI
(pointer to a pair copied by the callee; hidden destination pointer for results).
The data pointer borrows storage; copying the pair never copies or owns elements.
No owned slices or allocator operation is introduced.

Array unsizing constructs the pair. Range slicing checks `start <= end <= len`,
then offsets the pointer by canonical element stride. Indexing checks index < len.
Invalid ranges or indices abort through the existing fault helper. Len/is_empty,
iteration and mutable indexing execute IR operations without consulting loans.
A regression corrected frontend iteration over nested references, including
`for x in &slice`: type checking uses existing reference peeling and lowering
emits each required Deref. Mutability is intersected across the reference chain.

## Closures and function values

A callable is a 16-byte, 8-aligned pair: code pointer at 0, environment pointer
at 8. Borrow captures are laid out in declaration/capture-vector order using the
canonical layout of each capture parameter. The environment resides in the
creating function's stack frame; the existing escape checker is responsible for
rejecting escape. No heap environment, GC or ownership capture is added.

Each reachable callable has a private thunk whose ABI places the environment
pointer after any hidden result pointer and before visible arguments. The thunk
loads captures and calls the ordinary concrete function body. A function value
without captures uses a null environment. Indirect calls use exactly that thunk
signature. Nested environments may borrow outer captures while the outer call is
active. Shared, mutable and multiple captures preserve existing capture metadata
and semantic checks; the backend does not infer capture modes.

Current function values are non-Copy and invoking a local callable consumes it
according to existing IR. Tests deliberately preserve this one-shot behavior;
reusable closure call semantics need a separate frontend decision. Environments
are allocated in function stack slots and reused when the creation site executes
again; nonescaping semantic lifetime restrictions remain essential.

Alternatives considered: heap-owned environments require new ownership/drop
semantics; inline variable-sized environments complicate uniform call signatures.
A borrowed stack environment is the smallest representation for existing valid
forms. Generating thunks even for direct-only functions and bytewise pair copies
is intentionally not optimized in this phase.

## Standard-library semantic closure

Unknown bootstrap stdlib APIs, external types and external constructors now emit
E3040 rather than minting opaque values with invented ownership/provenance. The
placeholder prelude Error is rejected; use a declared error struct/enum. Channel
construction and methods also reject missing contracts without implementing
concurrency. Existing core declarations/intrinsic markers expose their parameter
and return types and retain existing provenance analysis. Declared Tarn wrappers
returning references propagate the input loan; a regression forbids mutation of
that input while the result is later used.

As defense in depth, an Opaque type may hold references, and an opaque IR call
uses conservative argument-loan inflow instead of the former empty inflow. Opaque
recovery paths remain for already-invalid input and range bookkeeping. No accepted
unknown stdlib call gains safety through those recovery paths. Opaque bootstrap
examples for fs/net/process/concurrency now fail conservatively. This is a
compatibility reduction justified by the absence of semantic contracts.

This does not certify complete end-to-end memory safety. Native runtime C and
intrinsic implementations remain a trusted boundary; externally implemented APIs,
concurrency, dynamic objects and user destructors lack complete executable
contracts. Stdlib breadth must grow through explicit declarations and checked
implementations, not opaque fallback acceptance.

## Validation and deferred work

Executable fixtures cover generic scalars, Pair, Option, Result with owned strings,
nested generic ADTs, arrays, recursive calls, fat slice operations and bounds,
shared/mutable/multiple/nested closures and declared borrowed returns. Tests check
instance reuse, deterministic names, unreachable templates and mutual recursion.
Generic resource traces are compared with the same resource-token interpreter as
6C (extended only with numeric subtraction). Existing 54-path drop comparisons,
mutation suites, both IR verifiers and memory-safety gates remain mandatory.

Virtual `any I` dispatch is explicitly deferred before designing vtables: this
phase stops at semantic stdlib closure as permitted by the request. The numeric
completion is recorded in ADR 0029. No runtime symbol or external dependency was
added. No optimization work follows these baselines.

Defended choices: backend-local FIFO specialization, one layout layer, borrowed
stack environments, explicit pair ABI, conservative E3040 rejection. Open questions:
instance/type limits, one-shot callable semantics, thunk cost, metadata for external
APIs, and the future lifetime/destruction model for dynamic objects.
