# ADR 0023 — Typed IR: a non-SSA control-flow graph of places

Status: accepted (2026-10-05). Implementation: `compiler/ir`; printed with `tarn ir`.

## Goal

A representation where ownership/borrow analysis and backends find every
semantic decision already made. Ownership must not re-resolve methods, infer
types, or decide whether a read copies or borrows.

## Shape

```
Program   functions, by_symbol: SymbolId → FunctionId
Function  id, name, kind (Body | Extern | Intrinsic | Closure{parent, captures}),
          generics, param_count, ret, locals, blocks
LocalDecl ty, kind (Return | Param | User | Temp), name, symbol, mutable
Block     statements, terminator
Statement Assign(Place, Rvalue) | StorageLive(l) | StorageDead(l) | Drop(Place)
Place     local + projections [Deref | Field(i) | Index(local) | Downcast(variant)]
Operand   Copy(Place) | Move(Place) | Const
Rvalue    Use | Ref(mut?, Place) | SliceRef | Binary | Unary | Aggregate
          | Cast | Coerce(kind) | Discriminant(Place) | Len(Place)
Terminator Goto | Switch | Call{callee, args, dest, next?, spawn} | Return | Unreachable
Callee    Fn(FunctionId, type args) | Virtual{interface method} | Intrinsic
          | Builtin | Value(operand) | Opaque
```

Local `_0` is the return place; parameters are `_1.._n`.

## Decisions

1. **Not SSA.** Locals are mutable slots; borrow checking is about *places*
   (`(*r).f[i]`), which SSA would scatter across versions. SSA can be built
   later for optimization from this IR if benchmarks call for it.
2. **Copy vs move is explicit.** A read of a place is `Copy` when its type is
   copy (`Decls::is_copy`, the type checker's single definition), `Move`
   otherwise. Moving out of a place behind a reference (`move (*r).f`) can
   appear in the IR; *rejecting* it is ownership's job.
3. **Borrows and auto-deref are explicit.** `Ref(shared|mut, place)`; field
   access through references becomes `Deref` projections; method receivers
   follow the recorded adjustment (derefs + auto-ref / copy / move).
4. **Implicit reborrow.** A `&mut T` place passed as an argument, a method
   receiver, or coerced to `&T` is reborrowed (`&mut *p` / `&*p`), not moved,
   so it stays usable. Binding it to a new name (`m2 := m`) moves it.
5. **Coercions are explicit** `Coerce(Unsize | ToDyn(I))` rvalues, from the
   checker's coercion table; `&mut → &` is a shared reborrow.
6. **Calls are resolved.** Concrete functions/methods/impl methods →
   `Fn(FunctionId, type args)`; interface methods on `T: I` and `any I` →
   `Virtual` (monomorphization or vtables decide later); `core` intrinsics →
   `Intrinsic(name)`; `print`/`panic` → `Builtin`. `panic` has no successor.
7. **Temporaries.** Every intermediate value is a `Temp` local. Non-copy
   temporaries are dropped at the end of their statement, except the operand
   of `x := &<temporary>`, which lives until the end of `x`'s scope.
8. **Drops are explicit and conditional by definition.** `Drop(place)` means
   "drop the value in `place` if it is (still) initialized". Lowering emits
   them for every owned local at every scope exit (fallthrough, `return`,
   `break`, `continue`, `try`), in reverse declaration order, and before
   overwriting an owned place on assignment. *Drop elaboration* after
   ownership analysis turns each into unconditional / flagged / removed, and
   drops only the still-initialized fields of partially moved places.
   References, functions and closures own nothing: no drops. `panic` aborts
   (no unwinding), so no drops on panic paths.
9. **`StorageLive`/`StorageDead`** delimit user locals' lifetimes for the
   borrow checker (a borrow may not outlive the storage of its place).
10. **Control flow:** `if`/`while`/`&&`/`||` become `Switch` on booleans;
    `for x in a..b` a counter loop (`..=` stops at the bound without computing
    `bound + 1`); `for x in &xs` an index loop with `Len`/`Index` and the
    recorded binding mode; `match` a chain of tests per arm (discriminant
    switches, literal comparisons), bindings created only after all tests of
    the arm pass, `Unreachable` after the last arm (exhaustiveness proven).
11. **Closures** are separate functions whose first parameters are the
    captures; `FnKind::Closure` records a `CaptureMode` per capture
    (`SharedBorrow`, `MutableBorrow`, `Move`). v0 only produces borrows
    (`&mut` if the body writes or mutably borrows the capture); `Move` exists
    so returned closures and `spawn` need no new representation. No public
    syntax for move closures yet. The closure value is
    `Aggregate::Closure(id)[captures]`.
12. **Generic code stays generic** (types contain parameters); calls carry
    their type arguments.
13. **Observers borrow.** `print` takes a shared borrow of its argument and
    never consumes an owned value; string `==`, `<`, `+`… call intrinsics
    with borrowed operands. The same rule applies to every purely observing
    operation on strings and collections (`len`, comparisons…): reading never
    moves. Methods of opaque std values also borrow their receiver (their
    signature is unknown; assuming a move would invent errors).
    `scope { }` ends with `JoinScope`; `spawn f(x)` is a `Call` with `spawn`.
14. **Opaque std** values and calls stay as `Const::Opaque` /
    `Callee::Opaque`: such programs check but cannot be compiled.

## Verification

`tarn_ir::verify` checks structural invariants (ids, locals, block targets,
reachability). Tests run it on every program that type-checks and on every
line-deletion mutant.

## Risks

- Match lowering re-reads the discriminant per arm (no decision tree): simple
  and obviously correct, slower code. Decision trees are a backend concern.
- Drops are over-approximated by design; until drop elaboration exists, the IR
  must not be executed as is.
