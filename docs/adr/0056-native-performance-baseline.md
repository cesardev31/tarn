# ADR 0056: native performance baseline against Go

Status: accepted. Phase 28; [report](../phase-28-report.md). Benchmarks:
[`benchmarks/vs-go`](../../benchmarks/vs-go/README.md).

## Context

The goal is parity with Go 1.26 on ordinary workloads without weakening any
safety guarantee. Measurement came first: six programs written identically
in both languages with identical output. The backend compiled with Cranelift
`opt_level = none` and the embedded C runtime with `-O0`.

## Decisions

1. **Optimize by default.** Cranelift `opt_level = speed`, runtime `-O2`.
   Semantics are unchanged; the full suite, including mutation and
   destruction-oracle tests, is the guard.
2. **Trace switches are read once.** `TARN_TRACE_*` were `getenv` calls on
   every string drop and every traced resource event. They are now cached
   atomically per switch; Tarn never mutates the environment.
3. **Two primitives for bulk byte work**, declared in `core`:
   `Vec<T>.extend_from_slice(&mut self, items &[]T)` (Copy only, one
   `memcpy`, growth through the traced `tarn_rt_vec_grow`) and
   `string.copy_range(&self, start, end)` (a copy at scalar boundaries
   without re-validating UTF-8; an invalid range aborts, so the UTF-8
   invariant cannot break). The stdlib uses them where it copied byte by byte.
4. **Constant divisors keep only the checks they can fail**: no zero check
   for a nonzero constant, no `MIN / -1` check unless the constant is `-1`.
   This reads Cranelift's own `iconst`, not a semantic fact.
5. **Fault blocks are cold**, so aborting paths leave the hot layout.
6. **Readable symbols** (`tarn_fn_<id>__<name>`) make profiles usable.

## Not changed (deliberately)

- **Checked arithmetic.** Overflow checks are Tarn's semantics; Go wraps.
  They account for most of the remaining 1.5x on `arith`.
- **Owned substrings.** `split` returns independently owned strings (ADR
  0042), so each part costs an allocation and a free where Go shares the
  original buffer. Closing the `strings` gap needs a borrowed-view design,
  which requires stored references (forbidden in v0) or a new owned-slice
  representation: a separate decision.

## Evidence that would change this

Profiles of real programs where checked arithmetic or owned substrings
dominate, or a backend change (inlining, bounds-check elimination) that
moves the remaining ratios.
