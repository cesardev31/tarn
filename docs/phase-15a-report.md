# Phase 15A report: string essentials

Design: [ADR 0042](adr/0042-string-essentials.md).
Public API and example: [strings](strings.md).

## Implemented API

Read-only byte views, checked UTF-8 conversion, owned slicing, first-match
search, contains/starts_with/ends_with, ASCII trim, owned split/lines, decimal
u16/u64/i64 parsing, and full-range u64/i64 decimal formatting. Existing
primitive length/clone/view helpers remain. String concatenation and all six
comparison operators execute natively. High-level APIs use `import "string"`.
Examples 11 and 27 now use implemented functions and need no missing-string
API exception.

## Ownership and capabilities

Strings remain ordinary non-Copy owned resources with their existing
Transfer/Share rules. Byte views use the existing borrowed result contracts
and provenance. Live views prevent move/overwrite and cannot escape a local
owner. Conversion, substrings and Vec elements own independent copies.
Moving a split result transfers ordinary Vec/string destruction responsibility.
No new resource tracking, stored-reference exception or lifetime checker.

## UTF-8 and errors

Strict Unicode scalar validation; embedded NUL and empty input are valid.
Indices are bytes, with UTF-8 boundary checks for slicing. Invalid conversion,
invalid ranges and numeric parse/overflow return None. Normal text errors do
not abort. Internal impossible boundaries and allocation exhaustion retain
the existing abort policy. ASCII trimming is explicit; no Unicode normalization.

## Runtime and verification

C handles storage validation/copy, concatenation and comparison only. Search,
line/split behavior, numeric conversion and range policies are ordinary Tarn.
Post-drop verifies string intrinsic argument/result shapes before codegen.
Backend does not consult move/borrow state. Zero-length allocation copying
avoids invoking memcpy with a possibly null empty-slice source.

## Tests

Validation: `cargo test --workspace --locked -j4` passed 180 tests; one existing
benchmark remains ignored. Native C also compiles with `-Wall -Wextra -Werror`.

- Full workspace regression, including native mutation and frontend mutation
  corpora, memory safety, Phase 11 concurrency and all async/networking suites.
- Native fixture covers UTF-8 search/slicing, split, lines, comparisons,
  concatenation, parse overflow and formatting boundaries.
- Native oracle checks 3,339 byte sequences against Rust's UTF-8 validator.
- Signed/unsigned decimal round trips at the full-range extrema; malformed
  inputs and overflow return None without aborting.
- Embedded NUL comparisons and concatenation use lengths correctly.
- Exact destruction trace proves source/owned split elements/moved Vec cleanup.
- Canonical memory safety includes byte-view mutation/escape, live-owner move/overwrite,
  parameter provenance, owned UTF-8 conversion and owned split results.
- Corrupted string ABI is rejected before native emission.

## Bugs found

String operators already had correct borrowing in frontend IR but were missing
native execution. Empty slices can have a null data pointer: zero-length
copying now avoids that pointer. Checking substring UTF-8 content alone would
accept an empty range inside a scalar; explicit boundary checks prevent it.

## Decisions I would defend

Preserve ordinary ownership and loan checking. Owned split/line elements avoid
inventing reference-containing ADTs. Option models the current text failures
without depending on I/O errors. Range checks precede checked numeric arithmetic.
Keep high-level behavior in Tarn and preserve method-owner coherence.

## Decisions I still question

Owned split/lines eagerly allocate; later real-program evidence may justify a
borrowed iterator once reference storage has a coherent language design. The
public core `string_from_utf8` bridge is bootstrap debt. Detailed parse/UTF-8
error positions, Unicode whitespace and float formatting need separate API
choices. This phase makes no performance claim.

## Next phase

15B must add filesystem resources and explicit I/O errors, then use these text
and byte APIs for safe read_text/write_text behavior. It must choose how invalid
UTF-8 is reported at the file API boundary. No filesystem/path/process/HTTP
implementation was started in 15A.
