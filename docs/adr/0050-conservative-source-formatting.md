# ADR 0050: conservative source formatting

Status: accepted (Phase 20).

## Context

Tarn newlines participate in parsing (ADR 0008). Printing an AST alone would
lose comments and literal spelling; rewriting whitespace by regular expressions
would confuse strings, unary operators, generic lists and statement boundaries.

## Decision

`tools/fmt` is a dependency-free library over the existing lexer, parser and AST.
It formats a UTF-8 buffer without loading imports or running semantic analysis.
Parser diagnostics prevent edits. AST spans identify binary operators, so
comparisons and shifts receive spaces while generic delimiters and prefixes stay
tight. The AST remains neutral; formatting state lives outside it.

Canonical layout uses four spaces, LF separators, one terminal newline for
nonempty sources, at most one blank line, tight calls/indexing/member access,
spaces around binary/assignment operators and after commas/colons, and two spaces
before trailing comments. Comment text, doc markers, numeric spelling and string
escapes are retained. Existing physical line breaks remain; single-line blocks
stay single-line. Continuations and multiline delimiters receive indentation.
No wrapping, import sorting, token insertion or syntax repair occurs.

Before returning output, relex and reparse it. Require equal token kinds,
non-newline token spelling, lexical newline boundaries (apart from a terminal
newline), comment text/order and the canonical syntax dump. A disagreement is an
internal formatter refusal, never an edit. Tests require repeated formatting to
produce exactly the same result across the repository's valid syntax corpus.

The CLI and LSP call the same library. Formatting requests operate on open,
unsaved buffers and return UTF-16 full-document edits without changing server
state or disk. Editor tab settings do not override the canonical four-space style.

`tarn fmt [file.tarn | directory] [--check]` defaults to the current directory;
directories are visited recursively and deterministically. Skip symlinks, hidden
subdirectories, `target`, `node_modules` and `graphify-out`; an explicit symlink
is refused. Never load or modify sources outside the selected tree via imports.
`tarn fmt --stdin` emits formatted source to stdout.

Read and format all selected sources before any writes: a syntax error prevents
all edits in that invocation. Changed files are replaced atomically through
same-directory, exclusively created temporary files, retaining permission bits
and checking for intervening source changes. Unchanged files are not rewritten.
An I/O failure during replacement may leave earlier files formatted: this is
per-file atomicity, not a filesystem transaction.

## Limits

The basic formatter preserves line layout rather than prescribing line width or
expanding compact blocks. It refuses invalid syntax and invariant violations.
Range/on-type formatting and style configuration are outside this phase. No
compiler semantic rules or editor-local parser are introduced.
