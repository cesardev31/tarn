# ADR 0042: owned UTF-8 string essentials (Phase 15A)

Status: accepted.

## Problem and evidence

Strings had length, cloning and whole-owner views, but no usable text module.
Examples 11 and 27 depended on undefined parsing/splitting methods. The JSON
and word-count evidence programs use byte arrays because string bytes were
unavailable. The frontend already lowered string operators to borrowed
intrinsics; the native backend did not execute them.

## Representation and ownership

Keep the existing owned, non-Copy UTF-8 heap string. `len` counts bytes. Moving
transfers its existing post-drop destruction responsibility. No mutable byte
projection is provided. `bytes(&string) &[]u8` borrows the string using the
ordinary result contract, provenance and loan checker. It cannot outlive the
owner or coexist with moving/overwriting that owner while still used.

`from_utf8(&[]u8) Option<string>` strictly validates Unicode scalar encodings
and copies valid input into a new owner. Empty input and embedded NUL are
valid. Invalid encodings return None; allocation failure still aborts under
the existing allocation policy. It never borrows the source buffer in its
result. `slice` also returns an owned Option and rejects bounds inside a
scalar, including empty ranges at continuation bytes.

## Application API

High-level functions live in the existing ordinary `string` module. Primitive
methods remain declared in core, as required by ADRs 0020 and 0041. No special
method-owner exception or iterator language feature is introduced.

- Search: contains, starts_with, ends_with, find (first byte offset).
- Transformation: trim (ASCII whitespace), split, lines and slice.
- Conversion: from_utf8, parse_u16, parse_u64, parse_i64, from_u64, from_i64.
- Existing len, is_empty, clone, view and choose remain available.

`split` and `lines` return `Vec<string>` with independently owned elements.
This deliberately allocates: ordinary ADTs cannot store references in v0.
Iterate with `parts.as_slice()`. A borrowed iterator must wait for a coherent
language-level reference-storage design; no separate lifetime checker is used.

Split uses non-overlapping exact UTF-8 matches and retains empty fields.
An empty separator yields scalar strings with no artificial empty fields.
Lines recognize LF and CRLF, preserve bare CR, and omit a phantom final line.
Empty input has zero lines. Trim recognizes bytes 09..0D and 20 only.
No Unicode normalization, grapheme segmentation or locale-dependent behavior.

Numeric parsing is strict ASCII decimal, with an optional '+' and, for signed
parsing, '-'. Empty/sign-only input, whitespace, invalid digits and overflow
return None. Checked arithmetic remains authoritative: explicit range checks
prevent overflow before arithmetic executes. Decimal formatting covers the
full integer ranges, including i64::MIN. Float formatting and generic Display
are outside this phase.

## Operators and native boundary

`+` allocates a new string and borrows both operands. Equality compares bytes
and lengths; ordering is unsigned lexicographic UTF-8 byte ordering, independent
of locale. Embedded NUL is ordinary content, never a C terminator.

Tarn implements search, trim, split, lines, slicing policy, parsing and decimal
formatting. Three private C helpers validate/copy UTF-8, concatenate storage,
and compare storage. The existing allocator/destructor remains unchanged apart
from avoiding zero-length memcpy on a potentially null empty slice.

Core declares `string.bytes` and the public bootstrap bridge
`string_from_utf8`. The ordinary module wraps the latter as `string.from_utf8`.
This bridge is safe but visible in the prelude: acknowledged bootstrap debt,
not a new resource or application type in core. Moving allocation/validation
behind a more general stable string constructor may be justified later.

The post-drop verifier checks the concrete signatures of these bridges and
operators before native execution. Runtime/backend have no ownership inference.

## Alternatives and limits

Borrowed split elements would violate the current stored-reference rule.
Adding primitive methods from arbitrary modules would change coherence for
one library feature. Unicode whitespace/case/normalization requires a separate
scope and data policy. Returning detailed parsing errors is possible later;
Option is sufficient for the existing application examples and has no new
error-model dependency. The allocation cost of copying substrings remains an
explicit tradeoff, not a performance claim.

Filesystem, path, process, HTTP and new execution features remain outside 15A.
