# ADR 0002 — Rust bootstrap, own IR, Cranelift first

Status: accepted

- The compiler (`compiler0`) is written in Rust: algebraic data types and
  pattern matching fit ASTs/IRs, memory safety, direct Cranelift/LLVM access.
- The frontend lowers to a **Tarn IR** (typed CFG). Ownership/borrow analysis
  runs on it. Backends consume it; only `tarn_backend` depends on Cranelift.
- Cranelift is the first backend (fast compile, pure Rust, modest dependency
  tree). LLVM for `--release` is evaluated later with benchmarks, not assumed.

Consequence: one extra lowering step versus generating from the AST, accepted
for analysis quality and backend independence.
