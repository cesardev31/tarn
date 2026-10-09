# ADR 0061: offset text traversal and allocation-free integer append

Status: accepted scoped implementation (Phase 32).

## Evidence

The Go comparison constructs text and splits it. Tarn's owned split allocates
one string per part; Go's parts retain views of its immutable source. Builder
integer append also unnecessarily allocates a temporary decimal string. A real
stdin word-frequency CLI needs field traversal, owned map keys and deterministic
output, not a new lifetime model.

## Decision

Add ordinary Copy offset Range and SplitCursor types in string, next_field,
next_line, next_split and split_ranges. Offsets store no loans or source identity.
range_bytes validates bounds and UTF-8 scalar boundaries and returns an ordinary
immutable byte-slice loan of its source. Invalid access aborts like indexing;
valid_range and copy_span provide validation/fallible copying. Cursors must refer
to scalar boundaries; field/line EOF equals byte length. SplitCursor also stores
a finished flag so final empty fields are unambiguous. Changing inputs between
cursor calls is memory-safe, but is not a meaningful stable traversal contract.

Existing split/lines/fields retain owned results and their contracts. No stored
references, borrowed struct fields, UTF-8 mutable views or new &string layout are
introduced. Byte slices are deliberately explicit; general borrowed string
values/grapheme iteration remain future design. parse_u64_bytes accepts strict
ASCII decimal directly. split_count counts parts without materializing them.

Builder.push_u64 formats into a stack array then bulk-appends its used slice;
push_i64 handles the minimum signed value without negation overflow. This removes
per-number heap allocation without new intrinsics or unchecked arithmetic.

The CLI bounds input to 8 MiB, validates UTF-8 and owns map keys. Repeated-word
lookup still allocates a temporary key. Do not claim fully allocation-free CLI.

## Compiler regression

Copy-bound dispatch must consult the semantic Copy capability before nominal ADT
impl lookup. A declared copy struct does not need a manual impl Copy. Tests cover
Vec<Range>.at and owner escape/overwrite rejection for returned byte views.

## Validation and limits

See the Phase 32/33 report for measurements, commands and limits. Existing byte
bounds checks stay enabled; no unsafe optimization or aliasing relaxation follows
from a faster benchmark. General formatting, floats and Unicode classification
remain outside this scoped phase.
