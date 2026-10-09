# textstats

Build: `tarn build examples/textstats/main.tarn -o /tmp/textstats`.
Run: `printf 'café tea café\n' | /tmp/textstats`.

Reads at most 8 MiB from stdin, validates UTF-8, counts exact case-sensitive
ASCII-whitespace fields, and prints count/tab/word rows. Punctuation remains
part of a word. Output sorts by descending count then UTF-8 byte order.
Invalid UTF-8 and oversized input are errors; output errors propagate.
Traversal uses offset cursors without a vector of copied fields. Keys are
owned strings; repeated occurrences still allocate a temporary lookup key.
This is a bounded batch CLI, not a Unicode tokenizer or streaming counter.
