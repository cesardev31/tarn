# CSV

An ordinary Tarn module, with owned fields and no native parser.

- `parse_record(&text)` parses one comma-delimited record, including quoted
  commas, doubled quotes and embedded quoted newline characters.
- `parse_record_with_limits(&text, limits)` sets maximum record bytes/fields.
- `Reader.new(console.stdin().buffered())` reads logical records with
  `read_record(limits)`. EOF returns None; an empty physical line is one empty
  field. It does not implicitly skip a header or blank records.
- Limits.new defaults to 1 MiB/1024 fields; maximum 64 MiB/65,536 fields.
- Parse errors include kind and byte offset. Streaming errors distinguish
  I/O/UTF-8 failures from CSV syntax and poison the reader.

ASCII commas/quotes are structural. Spaces are ordinary field data; text after
closing quotes must be comma or end. No dialect guessing, BOM removal or
comment handling. Streaming quoted CRLF normalizes to LF because LineReader
strips physical terminators. Direct parse_record preserves quoted CR/LF bytes.
Record limits include CSV syntax and normalized embedded LF, excluding the
final physical terminator. Fields are owned strings, not stored references.
