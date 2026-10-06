# Source async implementation checkpoint (Phase 13)

Superseded. This file recorded the intermediate syntax/type checkpoint and the
`MutexGuard.replace` stored-loan fix (E3051) found while probing frame storage.
Phase 13 is now implemented; see [the Phase 13 report](phase-13-report.md) and
[ADR 0037](adr/0037-source-async-lowering.md). E3062 (the checkpoint gate) is
retired. `MutexGuard.replace` is still not a frame-transfer mechanism: frames are
lowered after ownership checking, with no mutable stored-loan effects.
