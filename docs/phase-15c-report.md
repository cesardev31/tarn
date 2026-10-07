# Phase 15C report: owned lexical paths

Design: [ADR 0044](adr/0044-lexical-owned-paths.md).
Application API: [path module](path.md).

## Implemented

An ordinary `stdlib/path` module with owned Path construction, cloning, borrowed
text, consuming text extraction, validation, absolute/relative queries, join,
components, explicit normalization, parent, file name/stem/extension,
extension replacement and component-prefix queries/removal. Free functions
operate on borrowed strings. The filesystem application example now uses Path;
filesystem APIs keep their existing borrowed-string signatures.

## Ownership and capabilities

Path privately owns immutable UTF-8 text. Construction transfers that ownership;
normal post-drop destroys it. Moving a path or extracting its text cannot
produce duplicate owners. Text references borrow the path; owned components
and optional results can outlive it. Normal structural rules provide Transfer
and Share without granting Copy or adding type-specific capability exceptions.
Paths move into and return from native tasks and support shared scoped readers.

## Module/runtime boundary

Bundled path source uses ordinary untrusted fallback loading, like string.
Project-local path modules can override it. Official-source editor analysis is
supported. There are no new runtime functions, compiler intrinsics, language
syntax, native resources or backend ownership decisions. High-level operations
are written in Tarn and use existing UTF-8 slice validation.

## Tests

`cargo test --workspace --locked -j4`: 194 passed, no warnings, one existing
benchmark ignored. The filesystem application example also passes `tarn check`.
Six new Rust tests cover native
POSIX edge cases, Unicode, idempotent normalization, dotfiles/empty suffixes,
invalid suffixes, component-prefix boundaries, real filesystem integration,
owned task transfer, shared readers, exact payload destruction, local module
overrides and official-source editor analysis. A native golden fixture covers
the public API and joins both mutation corpora. Eight canonical memory-safety
cases cover constructor moves, consuming extraction, owner move/overwrite with
live text loans, escaping borrowed text, owned components and task capabilities.
Existing resolution goldens use the still-opaque http placeholder where they
specifically test unused imports/module shadowing rather than the path API.

## Bugs found

No compiler/runtime ownership bug was found. Development fixtures exposed two
existing language constraints: Result success requires explicit Ok construction,
and match arms need statement boundaries. Tests and documentation use the actual
language, without changing it. Path implementation also respects matching borrow
depths in string comparisons and cannot move indexed owned slice elements.

## Decisions I would defend

Pure Tarn implementation keeps the backend small and proves existing ADT
ownership useful for applications. Owned outputs avoid a parallel lifetime
framework. Explicit normalization keeps ordinary filesystem access faithful to
symlink-sensitive paths. Whole-component prefixes avoid character-prefix
mistakes but deliberately do not constitute a filesystem containment guarantee.
POSIX-only UTF-8 matches the supported platform and the existing filesystem API.

## Decisions I still question

Owned components and repeated scans copy more than a future iterator/view API
might require. Application measurements should decide that redesign. Direct
Path overloads for fs may later be useful if the language gains a clean conversion
model; today `as_string()` is predictable. Double-leading-slash semantics and
raw non-UTF-8 names would need an explicit platform/path representation decision.

## Known limitations and next phase

No filesystem canonicalization, symlink resolution, current-directory mutation,
Windows syntax, path equality interface or raw-byte path representation.
Construction does not assert filesystem validity or existence. Lexical `..`
cancellation can change the meaning of physical filesystem paths; it is opt-in.
Process execution remains Phase 15D and was not implemented here. Phase 16 HTTP
also remains untouched.
