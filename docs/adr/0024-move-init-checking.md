# ADR 0024 — Move/initialization checking (phase 6A)

Status: accepted (2026-10-05). Implementation: `compiler/ownership`.

Ownership is split in two subphases on the typed IR: **6A move/init
checking** (this ADR) and **6B borrow checking** (next). Both are dataflow
analyses over the IR's control-flow graph, never AST walks.

## Move paths

A tree per local: the local, plus one node per `Field(i)` and `Downcast(v)`
projection occurring in the function (`p`, `p.first`, `m.Text.0`). A place
through `Deref` or `Index` resolves to its tracked prefix and is *not*
tracked below it:

- moving out through a reference is an error (E4003);
- moving out by a dynamic index is an error (E4004), with one exception:
  the `IterArray` local of a consuming `for x in arr` loop.

Dynamic indices are never tracked: conservative by design.

## Domain and transfer

Per path: `maybe_init`, `maybe_uninit` (bits) and the set of move sites that
may reach this point (diagnostics only). Derived states:

| maybe_init | maybe_uninit | moves | state |
|-----------|--------------|-------|-------|
| 1 | 0 | – | Initialized |
| 0 | 1 | ∅ | Uninitialized |
| 0 | 1 | ≠∅ | Moved |
| 1 | 1 | – | possibly Moved / possibly Uninitialized |
| own bit init, a descendant not | | | PartiallyMoved |

Transfer: a `Move` of path P clears P and its descendants (records the
site); an assignment to P initializes P and its descendants; `StorageLive`,
`StorageDead` and `Drop` clear without a move site. A read needs P **and all
its descendants** initialized; reading a discriminant or a length needs only
P; reading through `Deref`/`Index` needs the reference/array itself.
Assigning `p.f` needs every ancestor of `p.f` to exist (E4006).

## Join (formal)

The state lattice is `(P(paths), P(paths), paths → P(sites))` ordered by
inclusion; join is pointwise union. Transfer functions are monotone, so the
worklist fixpoint exists and is reached. Consequence: a value moved on **any**
incoming path is *possibly moved* after the join, unless **every** path
reinitialized it (then `maybe_uninit` is 0 on all of them). Entry state:
parameters initialized, everything else uninitialized.

Loops need nothing special: a move in the body reaches the loop head through
the back edge, so the next iteration sees "possibly moved" (reported "in a
previous iteration of the loop"). `break`/`continue`/`return` are ordinary
edges.

## Uninitialized declarations

`var x: T` declares without a value (only `var`, only with a type: E1019).
Uses before every path assigned it are E4005; a `let`-style immutable
binding cannot be declared uninitialized because it could never be assigned.

## Consuming iteration (v0)

`for x in arr` over a copy array copies; over a non-copy array it moves the
**whole** array into an `IterArray` local first, so `arr` is simply moved
after the loop by the normal rules, and elements are moved out of that local
by index. `for x in &arr` iterates without consuming. Dropping the
`IterArray` drops the elements not yet yielded (a runtime concern of the
drop glue, not of the checker).

## Drop elaboration input

Every `Drop(place)` (drop-if-initialized) is classified from the state at
that point:

- `Static` — initialized on all paths: drop unconditionally;
- `Dead` — moved/uninitialized on all paths: remove;
- `Conditional` — initialized on some paths: needs a runtime drop flag;
- `Partial(fields)` — the value exists but these fields were moved on all
  paths: drop the remaining fields only.

`tarn ir` prints the decision next to each `drop`. The IR must not be
executed before drop elaboration consumes these decisions.

## Diagnostics

E4001 use of (possibly) moved value · E4002 use of (possibly) partially moved
value · E4003 move out of a reference · E4004 move out of an index · E4005
use of (possibly) uninitialized · E4006 assignment into a moved value.
Each path is reported once per function to avoid cascades.

## Not in 6A

Borrows: a borrow of a value that is later moved (`r := &a; consume(a);
read(r)`) is accepted by 6A and must be rejected by 6B.
