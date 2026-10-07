# ADR 0044: lexical owned POSIX paths

Status: accepted

## Decision

Phase 15C provides `stdlib/path`, an ordinary Tarn module importing `string`.
`Path` privately owns a string, is non-Copy and inherits Transfer/Share through
normal structural capabilities. Construction consumes text; `from_text` and
`clone` copy it. `into_string` consumes the path. `as_string` borrows its owner
using existing provenance. No lang item, native allocation wrapper, intrinsic,
resource catalog, new lifetime checker or runtime ABI is needed.

Paths preserve input verbatim, including empty strings and NUL. Pure lexical
operations do not promise filesystem validity or existence. `is_valid` means
nonempty and NUL-free; filesystem APIs independently enforce their boundary.
Unicode remains UTF-8; indices are byte offsets only at slash/dot boundaries.

Only POSIX slash syntax is recognized. Backslashes and drive letters are names.
Components own their strings: root is `/`, empty segments and `.` are omitted,
and `..` is retained. Repeated leading slashes collapse to one root, including
exactly two; implementation-defined POSIX double-root semantics are unsupported.

Join replaces the base for an absolute child, preserves it for an empty child,
and otherwise inserts a separator only if needed. It never cancels `..`.
Normalization is explicit, purely lexical: cancel preceding ordinary components,
preserve unmatched relative parents, clamp absolute parents at root, and produce
`.` for empty/all-cancelled relative paths. It is not canonicalization and can
change filesystem meaning through symlinks. Filesystem calls never normalize.

Parent/name/stem/extension return owned Options. A single relative name has an
empty parent; root/empty/dot-only paths have no parent. Final `..` has no file
name. Extensions use the last dot after the first byte: `.hidden` has no
extension, `a.` has an empty extension. Empty replacement removes an extension;
slash/NUL replacements are rejected. Replacement reconstructs lexical components.
Prefixes compare components, not character substrings; separators and `.` are
ignored, but `..` is never cancelled. Removing a whole matching prefix returns
an owned empty path.

`fs` continues taking borrowed strings. Applications pass `Path.as_string()`;
this keeps module layers simple and avoids implicit conversions/new overloads.
Bundled `path` uses the same untrusted fallback loading as bundled `string`.
Project-local modules can override it; editor loading recognizes the official
source without granting native privileges.

## Alternatives and consequences

A borrowed PathView would reduce copying, but stored references remain restricted
in v0. Owned components/results fit existing ownership without another lifetime
system. A native path representation would add no correctness benefit here.
Implicit normalization would be convenient but wrong for symlink-sensitive I/O.
Fallible construction would prevent arbitrary text representation without making
existence/permission valid. Explicit validation keeps that distinction clear.

Equality, filesystem canonicalization, Windows syntax, raw non-UTF-8 names,
current-directory mutation and process execution remain outside 15C. Copying and
repeated scans are accepted until application evidence justifies optimization.
