# Strings (Phase 15A)

Strings own immutable UTF-8 storage. Lengths and indices count **bytes**, not
characters. Use `import "string"` for text operations.

```tarn
import "string"

fn main() {
    text := "  café,hello  "
    clean := string.trim(&text)
    words := string.split(&clean, &",")
    for word in words.as_slice() { print(word) }
    match string.parse_u16(&"8080") {
        Some(port) => { print(port) }
        None => { print("invalid port") }
    }
}
```

| API | Result and policy |
|---|---|
| `value.len()` / `string.len(&value)` | usize byte length |
| `value.is_empty()` | bool |
| `value.clone()` | independent owned copy |
| `value.bytes()` / `string.bytes(&value)` | immutable borrowed `&[]u8` |
| `string.from_utf8(data)` | `Option<string>`, validated owned copy |
| `string.slice(&value, start, end)` | `Option<string>`, owned copy, UTF-8 boundaries required |
| `string.find(&value, &pattern)` | `Option<usize>`, first byte offset, empty pattern at zero |
| `string.contains`, `starts_with`, `ends_with` | bool; empty patterns match |
| `string.trim(&value)` | owned copy, trims ASCII whitespace 09..0D and 20 |
| `string.split(&value, &separator)` | owned `Vec<string>`, keeps empty fields |
| `string.lines(&value)` | owned `Vec<string>`, LF/CRLF, no final phantom line |
| `string.parse_u16`, `parse_u64`, `parse_i64` | Option; strict decimal, overflow/invalid input are None |
| `string.from_u64`, `from_i64` | owned decimal string, full numeric ranges |
| `left + right` | new owner, both operands remain usable |
| `== != < <= > >=` | content equality / unsigned lexicographic UTF-8 byte order |

A slice of bytes keeps its owner borrowed while used. Mutable bytes are never
exposed. `from_utf8` accepts embedded NUL and rejects overlong encodings,
surrogates, truncated sequences and scalars above U+10FFFF. It copies input, so
the original buffer can be changed or destroyed after the call.

`slice` rejects reversed/out-of-range indices and indices inside a scalar.
Returned strings and split/line elements remain valid after the input owner
is destroyed. They are independent resources with ordinary move/drop behavior.

For an empty separator, split yields individual UTF-8 scalars (not graphemes)
and no leading/trailing empty fields; splitting empty text this way is empty.
A nonempty separator on empty text yields one empty field. Lines preserve bare
CR, trim CR only immediately before LF, and return no lines for empty text.

Parsing accepts optional '+'; i64 also accepts '-'. Whitespace, separators,
Unicode digits and sign-only input are invalid. No runtime arithmetic fault
is used for ordinary parse failure. These APIs use Option, not io.Error.

The module uses functions rather than adding primitive methods outside core.
See [ADR 0042](adr/0042-string-essentials.md) for ownership, native bridges,
allocation tradeoffs and bootstrap limits. Borrowed text iterators, mutable
builders, Unicode classification and float formatting are deferred.
