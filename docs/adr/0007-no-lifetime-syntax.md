# ADR 0007 — No lifetime syntax; inferred borrow summaries

Status: accepted; **formalized by ADR 0010** (reference provenance), which
defines the inference and the rules for declarations without bodies.

Lifetimes are inferred with the elision-like rules in `docs/ownership.md`,
plus body-derived *borrow summaries* for functions with several reference
parameters. Struct fields cannot hold references in draft 0. When inference
cannot prove safety, the program is rejected with a diagnostic explaining
which parameters the result may borrow from.

Trade-off: a function's borrow summary depends on its body, so a body change
can be an interface change; tooling (`tarn check --json`, interface hashes in
the cache) makes this explicit.
