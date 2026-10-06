# Phase 9: borrowed dynamic interfaces and semantic contracts

## What was implemented

Native shared/mutable interface dispatch, concrete-to-dynamic coercions, multiple
method tables, function-boundary calls, borrowed string/fat-slice results and
concrete generic implementation instances. Static generic bound calls resolve to
direct methods, including owned receivers, without allocating tables or compiling
unreferenced interface methods. Declaration-derived contracts and checked
`borrows(...)` clauses expose invisible implementation provenance. Standard string
helpers now load as checked Tarn source; missing APIs retain E3040.

## Dynamic object representation

Borrowed references are `(concrete data pointer, vtable pointer)`, 16 bytes,
8-aligned. They use the existing private aggregate parameter/result ABI. Reborrows
preserve the pair; the concrete owner's storage remains authoritative. See
[ADR 0030](adr/0030-borrowed-dynamic-interfaces.md).

## Vtable design

Immutable initialized object data contains exactly the method pointer vector in
interface declaration order. Frontend metadata supplies resolved implementation
IDs and method indices. Each table belongs to a concrete `(Ty, interface)` pair.
No backend impl search, RTTI, size/alignment/destructor entries, devirtualization
or cache is introduced. Ordinary concrete method ABIs handle hidden result
pointers before the thin concrete receiver.

## Ownership/destruction policy for any

Only `&any I` and `&mut any I` execute. Bare owned dynamic annotations reject
E3041; no ownership construction, allocator or dynamic drop glue is invented.
The concrete owner follows its existing post-drop plans. Native trace checks for
two owned strings behind borrowed interfaces confirm reverse binding destruction
without any dynamic-reference destruction. The existing resource-token differential
corpus still checks all 54 boolean destruction paths; its interpreter does not
model references/dynamic dispatch, so the new dynamic trace check is an explicit
native expectation rather than an interpreter simulation.

## Semantic contract model

Canonical Tarn declarations produce `FnSig.contract`: Copy/Move/shared/mutable
parameter modes, receiver position zero, and Copy/Owned/Borrowed/InferredBorrow
result modes. Bodyless `borrows(a, b)` clauses supply an allowed source union;
body results still use the existing borrow-checker fixpoint. Invalid clauses are
E3042; ambiguous unannotated declarations remain E4202. Interface bodies must
respect declared sources (E4204). See [ADR 0031](adr/0031-declaration-semantic-contracts.md).

The embedded-on-import string module supplies len/is_empty/clone/view/choose as
Tarn wrappers. Core view/choose now have Tarn bodies rather than native magic
names. Importing string preserves the primitive in single-component type paths.
The [core audit](core-intrinsic-audit.md) classifies remaining hardcoded names and
behaviors as required intrinsic, bootstrap debt, runtime ABI or declaration
candidate. External C calls still require unsafe; contracts do not remove that
boundary or implement native external ABIs.

## Remaining trusted boundary

The C runtime and compiler intrinsics must obey their declared ownership,
provenance, Copy/resource and receiver promises. A malicious/incorrect native
implementation is outside the proof. Unknown fs/process/network and concurrency
APIs are rejected, not assigned empty result loans. No new runtime helper or
external dependency was added. Full end-to-end memory safety is not claimed.

## Tests

Executable fixtures cover two implementations, shared/mutable receivers, several
methods, slices as arguments and results, borrowed strings, function parameters,
generic concrete ADTs and static/dynamic generic paths. Native tests corrupt
missing/swapped/outside table entries, interface/concrete identity, pair types,
missing tables, virtual methods and owned dynamic slots. Static reachability tests
leave an unsupported unused method uncompiled. External dynamic construction
without native ABI support fails cleanly. Source-line mutation tests include the
new native fixtures; compiler panic/invalid code remains forbidden.

Parser and diagnostic snapshots cover provenance clauses, missing implementations,
owned any rejection, contract source errors and impl provenance mismatch. Contract
metadata tests cover every passing mode, result modes and primitive receiver
identity. Borrow tests keep all union-source loans live, allow mutation of excluded
inputs, distinguish owned results and preserve dynamic borrowed-result loans.
Existing parser/resolution/types/move/borrow/memory_safety/drop/native/CLI/mutation
regression suites pass cargo test. IR/provenance/drop snapshots were updated to
include the two now-executable core bodies.

The ignored baseline collector measured one million shared dynamic calls at
35.060 ms median (including startup/one print), 48.290 ms codegen, 151.875 ms
runtime compile/link and a 17,232-byte ELF. Other tests were running; this is an
uncontrolled local baseline, not isolated dispatch latency or optimization evidence.
Reproduction and limits: [baseline notes](../benchmarks/native/README.md).

## Bugs found

Initialized method relocations were initially emitted in BSS and became zero
addresses; tables now use initialized read-only data. Primitive method self types
were represented as ADTs, causing incorrect Copy-mode metadata; they now retain
canonical primitive types. Static bound dispatch initially instantiated all
interface methods; only the referenced method is now instantiated. Native
unsupported-body/entry checks precede missing-table checks, so unsupported external
dynamic producers are limitations rather than false compiler bugs. Importing
string hid the primitive type in annotations; the narrow type-path rule fixes it.
Provenance diagnostics were generalized to reflect explicit non-receiver sources.

## Decisions I would defend

Borrowed-only dynamic ownership; declaration-ordered minimal tables; frontend
resolved metadata; preserving direct static dispatch; reusing the private aggregate
ABI and the existing borrow checker; declarations plus a small provenance clause
instead of an unrelated external schema; real executable and corrupted-metadata
checks; preserving conservative rejection when contracts are insufficient.

## Decisions I still question

The borrowed pair-copy cost, instance limits and eventual owned-object table shape
need real program evidence. Explicit provenance syntax increases the language
surface but solves multi-input declarations without lifetime variables. The
specialized DynTable operation and table-presence check may need revision when
extern dynamic ABIs exist. Reference-bearing aggregates, resource handles and
callback/escape effects will need more expressive contracts. These are documented
tradeoffs, not reasons to optimize this phase opportunistically.

## Remaining reasons Tarn cannot yet claim complete end-to-end memory safety

Native runtime/intrinsic correctness remains trusted; unknown external APIs,
C reference/aggregate ABIs, raw-pointer proofs, dynamic generic methods and generic
interfaces, owned any, reference-containing structs, owned/escaping closure
captures, custom destructors, concurrency and unwinding are incomplete. Fn values
remain non-Copy: calling through a local consumes it; direct named function calls
do not consume a callable local. No reusable-call redesign was needed. No move
closures, concurrency, public ABI stabilization or optimization work follows.
