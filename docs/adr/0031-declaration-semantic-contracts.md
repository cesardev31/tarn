# ADR 0031: declaration-derived semantic contracts

Status: accepted for phase 9. Extends ADR 0010's bodyless declaration rules.

## Canonical model

Tarn declarations remain the canonical API description. `FnSig.contract` exposes
one machine-readable model derived by the frontend:

- parameter mode: Copy, Move, SharedBorrow or MutableBorrow;
- receiver: ordinary signature receiver kind, occupying parameter position zero;
- result: Copy, Owned, Borrowed(source positions), InferredBorrow, Ambiguous or
  NoSource.

Shared/mutable reference types select borrow modes; value types use the existing
Copy capability rule. Results without references use their canonical Copy/owned
semantics. For a body, reference provenance is InferredBorrow and the existing
6B fixpoint supplies its actual sources. A bodyless borrowed result uses the
receiver/single-input elision rules from ADR 0010 unless explicitly specified.
This does not create a second external registry of ownership rules.

## Bodyless provenance clause

A contextual trailing clause supplies the information an invisible implementation
cannot expose through a body:

```tarn
extern "C" fn choose(a &string, b &string, first bool) &string borrows(a, b)
interface SliceView {
    fn slice(&self, data &[]i64) &[]i64 borrows(data)
}
```

The clause names distinct reference parameters or a borrowed receiver `self`.
Names become checked parameter positions; order is normalized. Sources describe
all inputs that **may** supply the result, not a promise of which runtime branch
will be taken. A caller keeps every permitted source loan alive. A clause can
narrow receiver default provenance to a different input or express a union.
Unknown/duplicate/non-reference sources, empty clauses, owned results and clauses
on functions with a body are E3042. Body provenance remains inferred, never
silently overridden. Unannotated ambiguous bodyless declarations remain E4202;
source-less borrowed results remain E4201. Interface implementations must still
have inferred sources within the declaration's allowed set (E4204).

This is a small explicit **provenance** clause, not lifetime parameters. The new
need is multi-input external/interface APIs that previously had to be rejected.
It supersedes ADR 0010's blanket deferral of explicit provenance syntax; inferred
body lifetimes and the absence of public lifetime variables remain unchanged.
Existing semantic support for extern C declarations under `unsafe` is preserved;
the native extern aggregate/reference ABI is still unimplemented, and no public
C ABI for Tarn reference/string types is promised.

## Standard library and trusted boundary

`stdlib/string/string.tarn` is embedded on demand for `import "string"`, after
local files/overlays, and contains ordinary Tarn implementations for len,
is_empty, clone, view and choose. Core's view and choose have actual Tarn bodies;
only representation/primitive operations stay intrinsic. A module import named
`string` no longer hides the primitive in a single-component type path; member
expressions still address the imported module. This narrowly scoped namespace
rule leaves other shadowing rules intact.

Core and external contracts share this model. A native implementation **must obey
its declaration**: Copy/resource mode, borrowed storage lifetime, allowed result
sources and receiver effects are trusted promises. Incorrect or malicious C or
intrinsic code is outside the safety proof. `extern "C"` still requires `unsafe`;
a contract does not make such calls automatically safe. Native C external calls
remain unsupported; semantic contract checks nevertheless apply before backend.

The native runtime retains allocated unique strings, abort-only faults, printing
and math helpers; no new runtime symbol or dependency is added. Known intrinsic
lowering conservatively retains input loans. Missing external APIs/types and
unmodeled channel operations still produce E3040. Opaque recovery does not mean
empty provenance. Fs/process/network APIs have not acquired invented contracts.

## Validation and open decisions

Parser/diagnostic snapshots cover clauses and invalid declarations. Semantic
metadata tests cover all parameter modes, owned/copy/borrowed results and primitive
receiver types. Borrow regressions check union sources, permitted input narrowing,
owned results and dynamic result loans. Native string and dynamic fixtures exercise
both declared owned and borrowed results, and old safety/drop/mutation suites stay
mandatory. IR/provenance/drop snapshots now include the two real core bodies.

The explicit clause is deliberate syntax growth; more complex storage/aliasing
contracts are not invented here. Reference-containing structs remain rejected.
Resource-bearing external handles, escaping captures, concurrency, raw-pointer
proofs, callbacks and stable FFI need further contracts and executable semantics.
Complete end-to-end memory safety is not claimed.
