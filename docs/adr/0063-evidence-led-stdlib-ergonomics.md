# ADR 0063: Evidence-led text, console and map ergonomics

Status: accepted implementation boundary.

The independent status-device port identified manual reverse search, unsafe
missing-key lookup, allocation-heavy splitting and missing synchronous sleep.
A CSV-to-JSON CLI adds finite decimal parsing, quoted records and explicit
buffering pressure. These programs justify ordinary library APIs, not new syntax.

## Decisions

- `string.rfind` returns the last byte offset; an empty pattern matches the end.
- `collections.get_copy` returns `Option<V>` for Copy values. It is a free
  generic function because conditional method bounds are not supported in v0.
  `Map.get_or` borrows either the map or the caller's fallback. `next_slot`
  stores only a cursor offset; mutation may invalidate traversal ordering.
  Existing get/get_mut retain their documented abort-on-missing contracts.
- `time.sleep_blocking` blocks the calling native task and retries EINTR with
  the remaining timespec. Async sleep retains its existing executor contract.
- Finite decimal grammar is checked in Tarn. libc performs conversion using
  a dedicated C numeric locale, never changing the process-wide locale.
  Formatting uses 17 significant digits, not a shortest-decimal algorithm.
  JSON rejects nonfinite numbers before writing. Native allocation failures
  remain abort-only, as elsewhere in the runtime.
- Formatting accepts strings and sequential `{}` placeholders; doubled braces
  escape themselves. Syntax/argument errors include a byte offset. No implicit
  conversions or new interpolation syntax are introduced.
- LineReader owns buffering, strips LF/CRLF, validates UTF-8 and enforces an
  explicit returned-byte limit. Errors poison readers. Output buffering needs
  explicit flush/finish; destruction never performs I/O. Successful partial
  writes are retained when flush is retried.
- CSV owns its fields. Quoting, doubled quotes and multiline records are
  supported; quoted physical CRLF normalizes to LF in streaming input.
  Record and field bounds are explicit. Invalid input poisons the reader.
- `tarn stdlib [module] --json` extracts public declarations with the real
  parser. Its SHA-256 source identity is a freshness identifier, not a trust
  attestation. Private bridges stay private. No semantic provenance is guessed
  from syntax; future semantic queries must use compiler tables.

## Limits

No stored references, implicit error conversions, expression blocks, Unicode
case folding, grapheme replacement semantics, calendar/time-zone API or desktop
framework follows from this change. Finite formatting is round-trip oriented,
not a human presentation policy. Public catalogs do not yet expose inferred
provenance or type capabilities.
