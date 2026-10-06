# ADR 0025 — Borrow checking (phase 6B): loans held by locals, NLL by liveness

Status: accepted (2026-10-05). Implementation: `compiler/ownership/src/borrows.rs`.

Built on the typed IR and on 6A: moves and assignments are read from the IR
(`Operand::Move`, `Assign`), exactly as 6A reads them; move state is not
recomputed, and functions with 6A errors are not borrow-checked (no cascades).

## Loans

```
Loan { kind: Shared | Mutable, place, block, stmt, span, holder, param }
```

*What* is borrowed (`place`), *how* (`kind`), *where* it was created, *who*
first holds the reference (`holder`). *Where it is still active* is derived:
wherever a live local may hold it. `param = Some(i)` marks a placeholder loan
standing for "whatever the caller lent through parameter i"; placeholders
never conflict and exist for provenance.

## Algorithm

Three dataflow analyses per function on the CFG:

1. **Liveness** (backward): a local is live where it may still be read
   (reading through `*r` reads `r`). Drops do not read references.
2. **Loan flow** (forward, union at joins): which loans each local may hold.
   `Ref(place)` creates a loan and also carries the loans of `place`'s root
   (a reborrow through `*r` keeps `r`'s loans alive). Copies, moves,
   coercions and aggregates (closures, `Option<&T>`) propagate loans; an
   assignment to a whole local replaces its set (strong update), to a part
   adds to it (weak update). A call result holds the loans of the arguments
   in the callee's provenance.
3. **Conflicts**: a loan is *active* at a point if some local live at that
   point may hold it. Every access is checked against active loans whose
   place overlaps the accessed place.

Why not region inference: Tarn has no lifetime syntax and no references in
struct fields (E4203), so a borrow can only survive by being held in a local.
"Origin = local" (close to Polonius' origins) is flow-sensitive, simple, and
gives non-lexical lifetimes directly.

## Non-lexical lifetimes

A loan ends at the last use of every local that may hold it — not at the end
of the scope. `r := &x; read(r); write(&mut x)` is valid.

## Overlap

`a` and `b` overlap iff they have the same root local and their projections
never diverge at two *different* `Field`s or `Downcast`s. One being a prefix
of the other overlaps (`p` / `p.first`). `p.first` / `p.second` are disjoint.
`Deref` and `Index` never prove disjointness: `arr[i]` and `arr[j]` overlap
(no reasoning about runtime values in v0). Places rooted at different locals
never overlap: accessing `(*r).f` while `r` holds a loan of `x` is the
intended use of the loan, not a conflict.

## Accesses and conflicts

| Access | Conflicts with active loans |
|--------|----------------------------|
| read (copy, discriminant, len) | mutable |
| shared borrow | mutable |
| mutable borrow, write, move, drop | any |
| end of a local's storage (scope exit) | any loan *into* that local |

Reads and borrows are checked against locals live *before* the statement;
writes against locals live *after* it (so `x = f(&x)` is fine when the
result does not borrow `x`). `Drop(p)` followed by `p = v` is reported as an
assignment; `Drop(x)`/`StorageDead(x)` at scope exit as "does not live long
enough".

## Reborrows (exact rule)

Passing or using a `&mut T` place `r` where a reference is expected creates a
new loan on `*r` (the IR's reborrow `&mut *r` / `&*r`). While that loan is
active, accessing `*r` (or anything through `r`) conflicts with it — the
original is *suspended*, not moved — and `r` itself cannot be moved or
overwritten (its place is a prefix of `*r`). When the reborrow's holders are
dead, `r` is fully usable again.

## Two-phase method calls

Method arguments are evaluated (and materialized into temporaries) before the
receiver is borrowed, so `c.add(c.value)` is accepted without a separate
two-phase-borrow concept.

## Provenance (ADR 0010, integrated)

- Bodies: each reference-holding parameter starts with a placeholder loan;
  the parameters whose placeholders reach `_0` at a `return` form the
  function's provenance. Recursion: fixpoint over all functions.
- Declarations without a body: elision (`&self` → `{self}`; one reference
  parameter → it; otherwise E4202; none → E4201).
- At a call, the result holds the loans of the arguments in provenance;
  unknown callees (closures through values, intrinsics) use all arguments.
- Returns: `_0` may carry placeholders and loans of places behind a
  parameter reference (`&b.data` with `b &Buffer`); a loan of a local or a
  by-value parameter is E4201 (E4205 when the returned value is a closure).
- `impl` methods must not return borrows their interface does not allow (E4204).
- Storing a borrow of a local into memory reached through a reference
  (`out.value = &local`) is E4207.

## Closures

Captures are `CaptureMode`s; v0 produces only borrows, whose loans the
closure value holds. A closure that escapes (returned) while capturing a
local by reference is E4205: v0 has no capture by value.

## Concurrency

`spawn` arguments may not hold references (E4206): v0 does not track borrows
across tasks.

## Opaque std

Values of opaque std calls are assumed to hold no loans (they are unchecked
anyway); the stdlib will declare real signatures with provenance.

## Diagnostics

Each conflict names the borrowed place, where the loan was created, the
conflicting operation, and the next use of the reference that keeps the loan
alive. Codes: E4101 conflicting borrows · E4102 assign while borrowed · E4103
move while borrowed · E4104 use while mutably borrowed · E4105 does not live
long enough / dropped while borrowed · E4201 returned reference to local ·
E4202 ambiguous provenance · E4203 reference in a field · E4204 impl
provenance mismatch · E4205 closure escapes a borrow · E4206 reference passed
to `spawn` · E4207 borrow escapes through a reference.
