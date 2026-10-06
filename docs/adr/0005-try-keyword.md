# ADR 0005 — Prefix `try` for error propagation

Status: accepted (ergonomics to be re-evaluated while porting xlinux doctor)

`value := try op()` instead of Rust's postfix `op()?`.

- Visible at the start of the expression: early returns are easy to spot.
- Greppable (`try `), unambiguous for models generating code.
- Chains: `try (try a()).b()` is uglier than `a()?.b()?`; we accept this, and
  idiomatic Tarn binds intermediate results instead.
