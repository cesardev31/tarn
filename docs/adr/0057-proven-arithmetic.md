# ADR 0057: proven integer arithmetic

Status: accepted. Phase 29; [report](../phase-29-report.md). Refines the
checked-arithmetic rule without weakening it.

## Context

Every integer `+`, `-` and `*` is checked and aborts on overflow. After
Phase 28, those checks were most of the remaining gap to Go on arithmetic
loops (1.5x), including checks that cannot fail, such as the increment of a
loop counter bounded by its condition.

## Decision

- A conservative interval analysis runs on the typed IR right after lowering
  (`compiler/ir/src/ranges.rs`), before ownership analysis.
- An operation is rewritten to `AddProven`, `SubProven` or `MulProven` only
  when the interval of its result provably fits its type in every execution.
  The IR printer shows the decision (`add_proven`); the backend omits only
  those checks. Every other operation keeps its check, exactly as before.
- Soundness rules: only plain locals whose address is never taken and never
  written through a projection are tracked; branch refinement uses a
  comparison computed in the same block with unchanged operands; loop heads
  (back-edge targets) widen to the type bounds after a few visits; anything
  unknown is the full range of its type; unreachable or empty intervals fall
  back to the full range.
- `TARN_NO_RANGE_PROOFS=1` disables the rewrite (debugging and differential
  testing).

## Why this is safe

Removing a check that cannot fail changes no observable behavior: a program
that overflows still aborts, because the overflowing operation is never
proven. Unlike Go, which wraps silently, Tarn keeps detecting every
overflow that can happen.

The risk is a bug in the analysis. Mitigations: conservative rules, the
decision visible in the IR, abort tests at the edges (`i <= MAX` then
`i + 1`, unbounded multiplication, accumulators, signed lower bound), and a
differential test that runs every native fixture with and without proofs
and requires identical output and status.

## Effect

`benchmarks/vs-go/arith`: 1.5x to 1.0x of Go. In that loop, `i * 7`, `+ 3`
and `i + 1` are proven; `total + ...` keeps its check.

## Evidence that would change this

A counterexample (an aborting program that stops aborting) must be treated
as a soundness bug: fix by making the rule more conservative, never by
removing the test.
