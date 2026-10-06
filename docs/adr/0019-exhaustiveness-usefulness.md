# ADR 0019 — Exhaustiveness and reachability by usefulness

Status: accepted (2026-10-05). Replaces the conservative phase-4 check.

## Problem

The first checker only looked at top-level constructors; nested refutable
patterns never counted as covering. It rejected correct code:

```tarn
match value {
    Some(Ok(x))  => ...
    Some(Err(e)) => ...
    None         => ...
}
```

and the only workaround was a `_` arm — which then hides a real missing case
the day a variant is added. A checker limitation must never be "fixed" with
`_` in user code.

## Options

1. Keep top-level only (rejects valid code, forces `_`).
2. **Usefulness over pattern matrices** (Maranget 2007; used by OCaml, Rust,
   Swift): precise for nested constructor patterns, gives a witness (a value
   not covered) and unreachable arms from the same computation.
3. Decision-tree compilation first, exhaustiveness as a by-product: couples
   checking to code generation strategy.

## Decision: option 2, implemented now (`compiler/types/src/exhaust.rs`)

- Constructor sets: enum variants (finite), `bool` (finite), structs (one
  constructor); integers, floats and strings are infinite (only a binding or
  `_` covers them). References are transparent for matching.
- Arms with an `if` guard never count as covering (a guard can be false).
- `E3018` names a concrete witness: "`match` does not cover `Some(Err(_))`".
- `W3001` reports arms that can never match.

## Known limits (documented, not hidden)

- Integer ranges are opaque constructors: `0..=127` and `128..=255` on `u8`
  are not recognized as complete; a final arm is needed. Range splitting is a
  later refinement of this same algorithm.
- Overlapping literal ranges are not reported as unreachable.

Evidence to revisit: none expected for the algorithm; range splitting when
real code matches on byte ranges (likely in parsers in xlinux).
