# ADR 0026: executable drop elaboration

Status: accepted for v0 (phase 6C).

## Boundary and representation

The pipeline is typed non-SSA IR → move/init checking (6A) → borrow checking
(6B) → drop elaboration. Elaboration runs only after all diagnostics are
error-free. It does not recompute ownership or loan conflicts. The original
IR, move paths, loans and provenance remain available for inspection.

`tarn_ir::post_drop::Program` is a separate, self-contained destruction IR.
Each function carries its signature, locals, flags and one CFG. `decl.blocks`
is empty. Ordinary assignments, storage statements and terminators retain
their existing representation. Abstract `Drop` is forbidden by its verifier.
`CheckResult.drops` exposes the result; `tarn ir file.tarn --drops` prints it.
A compiler-invariant failure returns a compiler-bug error, not an ownership
diagnostic for the user.

Executable destruction uses these explicit operations:

- `Value(place)`: destroy the complete, initialized value. Type-directed drop
  glue recursively destroys owned resources; no initialization inquiry.
- `Guard(flag, drop)`: execute the nested operation only if that boolean bit
  is true. A false guard does not inspect the place.
- `Fields { place, fields }`: partial struct destruction; execute only the
  enumerated child plans. Never run the complete parent's drop.
- `Variants { place, variants }`: read the live enum tag and execute exactly
  the active variant's child plan. This is explicit runtime tag dispatch,
  not a move-checker query. A maybe-uninitialized enum has an outer guard
  before any tag read.
- `Remaining { place, flag }`: destroy only array elements whose bitmap bit
  is true. Never read a consumed element.

These structured operations encode their own control flow. They intentionally
remain structured for now; a backend can expand them into branches without
consulting 6A or 6B. No drop optimization or custom destructor is implemented.

## Analysis input and the four cases

6A still reports `Static`, `Dead`, `Conditional`, `Partial`. It additionally
exports initialization state at each abstract drop site: `Live`, `Dead` or
`Maybe`, per structural move path. This is captured during its existing
reporting pass after the fixpoint, not a second analysis.

Move paths are sparse: only projected places actually used in the function
are interned. A missing field inherits its nearest tracked ancestor's state.
Elaboration uses the type's declaration vector and generic substitution to
complete all unmentioned siblings when splitting an aggregate.

| Case | Transformation |
|------|----------------|
| Static | An unconditional `Value`; no boolean flag needed. Copy places disappear. |
| Dead | Delete the abstract drop; perform no destruction. |
| Conditional | An explicit boolean guard, or child plans with individual guards when some fields may have moved. |
| Partial | Recurse through declaration-ordered fields or active-variant payloads; omit dead children, drop live children, guard maybe-live children. Never destroy the complete parent. |

Two `Maybe` states are **not correlated**. At a join a parent and child may
both be `Maybe`, while one incoming edge moved the whole value and another
moved only the child. Therefore any tracked maybe/dead descendant forces
aggregate splitting, even if its state equals the parent's state. A single
parent bit would otherwise double-drop the moved field. Definitely live
children can use complete drops; dead subtrees are omitted.

## Runtime flags and updates

Flags have dedicated function-local `FlagId`s; they are not user-visible Tarn
locals and are disjoint from local IDs. `FlagKind::Value(place)` stores one
initialization bit. Allocate these only for guards that an executable plan
actually needs. Static and dead sites introduce no boolean flag themselves.
All allocated flags are initialized on entry: false for non-parameters, true
for parameter paths. They are then maintained at **every** mutation, including
ones textually before the first guarded drop.

An event on place `p` updates flags for `p` and all its structural descendants:

| Event | Flag action |
|-------|-------------|
| StorageLive / StorageDead | false |
| RHS move / moved call argument / moved switch operand | false |
| Assignment completes | true, after all RHS move updates |
| Returning call writes its destination | true on the successful return edge only |
| Abstract scope drop or overwrite drop completes | false, including when its guard was false |

A partial field move clears that field's descendant bits and leaves sibling
bits alone. A field reinitialization sets that field's descendant bits; if all
fields are live, 6A already proves the subsequent parent drop static. No runtime
"fully initialized parent" calculation is needed. A whole initialization sets
all descendant bits, and a whole move clears all of them.

Overwrite uses the existing abstract drop immediately before Assign. Its old
state selects unconditional, omitted, guarded or partial destruction. This
keeps uninitialized slots safe and destroys the old live value before writing
the new one. Overlapping owned RHS operands (`x = x`, `p.f = p.f`) must first be
materialized into a temporary: constructing a Move operand does not evaluate
it. This corrects a demonstrable lowering bug without changing move or borrow
checking.

For calls, flags for moved arguments are cleared before the terminator, and
flags for the result are set only on its normal return edge. A fresh bridge
block is inserted when result updates are needed; a join's other predecessors
must not receive those updates. Block IDs are deterministic (original blocks,
then bridges in traversal order). Calls with `next = None` get no bridge.

### Consuming arrays

6A deliberately permits indexed moves only from compiler-generated `IterArray`
slots. Those holes cannot be expressed by one whole-array boolean. An
`Elements(place, length)` flag is an explicit bitmap of exactly that array's
length. It starts empty, whole initialization sets all bits, an indexed move
clears the selected bit, and whole destruction/storage end clears all bits.
`Remaining` destroys set bits in increasing index order. This correctness-first
representation also handles break, continue, early return and zero iterations.
It requires no changes to 6A's indexed-move exception or to 6B's loans.

## Destruction order and panic

Named scope bindings retain the lowering's reverse declaration order, including
return, break and continue exits. Complete and partial structs destroy fields
in **increasing declaration index**, recursively depth-first. Enums dispatch
the active variant, then use the same payload declaration order. Arrays use
increasing element index, skipping consumed elements. All these orders depend
on declaration vectors, never HashMap iteration. Existing statement-temporary
lifetimes and scheduling remain intact.

Panic **aborts**. No unwind edges, no stack cleanup during panic, no exceptions,
custom RAII hooks, async cancellation, pinning or generators are added. Flag
writes around RHS/call evaluation assume this abort-only semantics. Copy values
need no owned-resource destruction. If user destructors are introduced later,
Copy and Drop must remain conceptually incompatible; their design is deferred.
References in structs remain rejected by 6B.

## Structural verifier

The post-drop verifier runs the existing CFG/local/call verifier on a structural
mirror and additionally checks:

- no abstract drops or second CFG in metadata;
- drop and flag places exist, including typed field/downcast/index projections;
- guards reference existing boolean flags for the same place;
- bitmaps belong to an IterArray of the declared length and index type;
- partial children are direct fields, unique, in declaration order, with no
  parent/child overlap; variants have the correct number of payload plans;
- complete resource drops are not applied to Copy places;
- all referenced flags are definitely initialized before reading. This is a
  must-initialized forward dataflow with intersection at joins and entry false,
  so later writes and loop back-edges cannot initialize the first read.

This is a compiler invariant check, not another borrow checker. It validates
structure and flag initialization, not a new proof of all semantic drop plans.
The execution tests separately validate resource destruction on actual paths.

## Validation and remaining decisions

`tests/drops/pass` contains source fixtures and elaborated IR snapshots. A
resource-token interpreter executes every boolean combination of each main
function, asserts no live overwrite, no read/drop of a moved resource, no
resource loss at scope/return, and exactly one destruction per created resource
on normal exits. It covers nested partial moves (two and three field levels),
conditional whole/partial joins, reinitialization, overwrites/self-assignment,
diamonds, loops/back-edges, break/continue, enum match/return, consuming arrays,
Copy, unreachable code and abort-only panic. Separate tests exercise call-result
bridge edges and corrupted flags, projections, partial plans and CFG targets.
The mutation corpus includes these fixtures and checks both IR verifiers;
compiler failure is rejected as well as panic. Existing 6A/6B and
`memory_safety.rs` remain regression gates.

Defensible choices: a separate post-drop boundary, sparse analysis plus typed
field completion, explicit per-path guards, structured variant dispatch, and
correctness-first array bitmaps. Open tradeoffs: whether a future backend wants
flat branch CFGs earlier; whether iteration bitmaps should later become a
consumption cursor; and how much redundant flag resetting can safely be removed.
None of these choices blocks executable destruction semantics, and none is
optimized in this phase. Native code generation remains unimplemented.
