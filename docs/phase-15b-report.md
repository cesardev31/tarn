# Phase 15B report: owned blocking filesystem

Design: [ADR 0043](adr/0043-blocking-filesystem.md).
API and application example: [filesystem](filesystem.md).

## Implemented

Regular-file open/create/append/exclusive creation/read-write opening, slice
read/write, exact/all loops, seek, metadata, explicit sync and consuming close.
Whole-file bytes/text helpers, exists, directory creation/removal, unlink,
rename and eager owned directory entries. Updated examples 12 and 23 now use
real declarations instead of future filesystem/path APIs. Core is unchanged.

## Ownership and capabilities

File and the private directory cursor use ordinary non-Copy ownership,
post-drop destruction, move checking and loan tracking. Both have explicit
Transfer and no Share. Position-changing File operations require &mut self;
metadata requires &self. Files move into native tasks and return via join.
Close consumes a File. Entry name/path references borrow their owned entry.
No runtime ownership boolean, registry, backend borrow logic or user Drop.

## Errors and text

Public APIs return io.Error with useful filesystem categories and original
native codes. UTF-8 conversion uses 15A; invalid file text or directory names
return InvalidData. NUL is supported in contents and rejected in paths. Exists
hides only absence, not permission errors. Partial progress and EOF are explicit.
Filesystem errno mapping uses its own private domain, preserving network rules.

## Runtime and verification

The fs trusted layer accesses io internals only. Filesystem resource and
intrinsic IDs have a separate validated catalog. C handles syscalls, stat
encoding, path termination and copying libc directory names; Tarn handles
retries, error mapping, buffers, whole-operation loops and ownership. Close is
never retried; descriptors are close-on-exec. Regular-file validation happens
after immediate owned wrapping, so rejected resources use verified cleanup.

## Tests

`cargo test --workspace --locked -j4`: 188 passed, no warnings, one existing
benchmark ignored. Runtime and fault-injection C compile with
`-std=c11 -Wall -Wextra -Werror`.

Eight new native tests cover real temporary files, 131,073-byte payloads,
empty/invalid UTF-8/NUL contents, seek/append, metadata, directory names,
exclusive creation, path errors and rejected FIFOs. Fault injection covers
EINTR, partial reads/writes, WriteZero, permission denial and consuming close
that reports EINTR. Trace balance covers return/break/continue/conditional init,
move/overwrite/self-assignment/try and task result ownership. Corrupted resource
shape, Copy, Share and intrinsic catalogs are rejected. Canonical memory safety
adds use-after-move, double close, loan conflicts, task capabilities and entry
reference escape. A filesystem fixture joins both mutation corpora. Existing
opaque-contract tests now use the still-unimplemented process module.

## Bugs found

The main error reporter had names for only ten ErrorKind variants; even the
existing UnexpectedEof and LimitExceeded could abort instead of exiting with
an ordinary error. It now covers the complete validated enum and reports
'I/O error'. Filesystem EINVAL needs InvalidInput while networking retains
InvalidAddress, so error domains remain distinct.

## Decisions I would defend

Reuse post-drop and existing capability/loan rules. Keep fs separate from
network resource metadata. Consume explicit close even on error. Read to EOF
rather than trusting stat size. Reject invalid UTF-8/NUL paths explicitly,
keep directory entries independent of libc storage, and propagate permission
errors from exists. Keep core small and all high-level loops in Tarn.

## Decisions I still question

Whole-file and directory helpers eagerly allocate without a configurable
limit. UTF-8 paths cannot represent every Linux filename. The private raw
result's address field carries stat-kind bytes, and raw numeric acquisition
handles remain bootstrap debt. There is no transactional write, sandbox/path
race guarantee, permission framework, timestamps, recursive operations or
public directory iterator. These require evidence and deliberate API choices.

## Next

15C should define path manipulation without changing file ownership or hiding
OS errors. 15D owns process execution. No path/process/async filesystem work
was started in 15B.
