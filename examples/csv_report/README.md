# CSV report

Build with `tarn build main.tarn -o csv-report`, then pipe CSV on stdin.
Header: `category,value` (ASCII case-insensitive, surrounding whitespace ignored).
Categories are trimmed and ASCII-lowercased. Values must be finite decimals.
Quoted commas, doubled quotes and multiline UTF-8 are supported. CRLF becomes LF
inside streamed multiline fields. Blank records are skipped. No BOM support.

Output is one deterministic JSON object, groups ordered by UTF-8 bytes:
`{"records":2,"groups":[{"category":"tools","count":2,"total":3.75}]}`.
No success output is written before input is validated. Errors write JSON to
stderr with kind, 1-based logical record and byte offset (CSV parse errors).
Exit status is 1 on input/output failure. Offset 0 means unspecified for
non-CSV errors. Group totals use IEEE binary64 addition, not decimal currency.

Limits: 1 MiB per logical record, 1024 fields, one million data records and
100,000 distinct categories. Input streams incrementally; grouping retains
owned keys and totals. Explicit buffered output finish is required.
