# ADR 0030: borrowed native dynamic interfaces

Status: accepted for phase 9, Linux x86_64; extends ADRs 0027 and 0028.

## Ownership boundary

V0 supports `&any I` and `&mut any I`. A bare `any I` annotation is rejected
with E3041, including parameters, results and aggregate fields. No owned dynamic
construction, allocation, dynamic resource transfer or dynamic drop glue is added.
A dynamic reference borrows the same concrete storage as its source. Destruction
belongs to the concrete owner's existing post-drop plan; references own nothing.
Borrow checking and provenance remain authoritative. The backend never queries
loan, move or initialization state.

A dynamic reference is a 16-byte, 8-aligned pair: concrete data pointer at offset
0 and private vtable pointer at offset 8. It uses the existing private aggregate
ABI: pointer to the pair for parameters, copied into callee storage; hidden
first destination pointer for aggregate results. Shared/mutable reborrows preserve
the pair. Nested reference storage retains the ordinary thin-reference rule.
The pair is not a public C ABI, and `any` does not expose fields or unchecked
concrete casts. Concrete storage is accessed only by the selected implementation.

## Resolved metadata and specialization

The frontend declaration catalog records interface methods in AST declaration
order, each method's `(interface, index)`, and resolved implementation symbols for
`(interface, target ADT)`. Existing resolver/type/borrow checks validate missing
implementations, signatures, receiver mutability and result provenance. Backend
specialization reads these direct ID mappings; it does not search for an impl.

A concrete coercion becomes `CoerceKind::DynTable { interface, concrete, methods }`
in the separately specialized post-drop program. Each method ID refers to a
concrete reachable function instance. Generic ADT binders are substituted as in
ordinary monomorphization; vtables are concrete, never runtime generic dispatch.
The unspecialized frontend IR retains ToDyn. Abstract Virtual calls whose receiver
specializes to a concrete ADT become direct calls and instantiate only the called
method. Calls whose receiver is `&any I` remain indirect. A dynamic coercion makes
all of its method bodies reachable because their addresses enter the table.

## Vtable and calling convention

A vtable contains only method function pointers, 8 bytes each, in interface
**declaration order**. There is no tag, RTTI, size, alignment or destructor entry:
borrowed dispatch does not need them. Empty explicit interfaces have a one-byte
storage placeholder and no callable entries. Table identity is `(concrete Ty,
interface SymbolId)`, deduplicated by deterministic post-drop traversal. Symbols
are private `tarn_vtable_N`. They are immutable initialized object data with
function relocations, not BSS: a regression found that BSS initialization lost
those relocations and produced a null call target.

Each pointer targets the ordinary concrete implementation ABI. An indirect call
loads the pair, selects the declaration-index function pointer, and passes the
concrete data pointer in the thin receiver lane, followed by ordinary arguments.
An aggregate result pointer precedes the receiver. Shared/mutable method receivers
and all visible argument/result lanes must match. By-value receivers and generic
dynamic methods are reported limitations. Generic static methods can specialize
normally; interfaces with type parameters lack a complete dynamic model.

## Verification and safety assumptions

After ordinary post-drop verification, native checks validate source reference
mutability, pair destination, concrete/interface mapping, method count, resolved
symbols, concrete signatures and function IDs. Virtual calls must reference an
existing declaration-index method and a reachable table for their interface.
Owned dynamic slots, missing tables, wrong concrete types, swapped/missing/outside
method entries and invalid representations are rejected before object emission.
Cranelift verifies generated code as before. No verifier proves a malicious raw
pointer safe; valid source and trusted intrinsic/runtime implementations are the
boundary, not an independent backend ownership proof.

Native tests cover two implementations with multiple methods, mutable receivers,
function boundaries, slice arguments, borrowed string and fat-slice results,
concrete generic ADTs, generic-to-dynamic coercion and direct generic bound calls.
Metadata mutation tests corrupt table and call identities; source-line mutations
include the dynamic fixtures. Native trace checks confirm the concrete string
owners are destroyed in reverse binding order, without dynamic-reference drops.
Existing resource-token differential tests continue to cover destruction; their
interpreter does not simulate dynamic references.

## Alternatives and deferred questions

Owned boxes would require allocation, destruction and new lifetime semantics;
they are deferred. A pointer to a separate descriptor would make references thin
but require descriptor lifetimes and another indirection. The inline borrowed pair
fits the existing private aggregate ABI. No devirtualization, speculative caches,
thunk optimization, closure redesign or public ABI stabilization is introduced.
Whether a future owned dynamic object shares this table or uses a distinct table
with drop/layout entries remains open. This phase deliberately needs neither.
