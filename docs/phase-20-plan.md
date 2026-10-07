# Phase 20: basic canonical formatter

Status: implemented; final validation in progress.
Design: [ADR 0050](adr/0050-conservative-source-formatting.md).

## Scope

- Shared `tarn_fmt` library using real lexer/parser trivia and neutral AST spans.
- Canonical whitespace, comments and literal preservation; existing physical
  line breaks remain, preserving Tarn statement and continuation rules.
- `tarn fmt`, recursive directory selection, `--check`, and `--stdin`.
- Syntax preflight, invariant checks, unchanged-file detection and atomic writes.
- LSP formatting of unsaved documents using the same library.

## Acceptance

- Golden layout for comments, Unicode, CRLF, operators, generics, references,
  compact blocks, argument lists and continuations.
- Valid repository sources round-trip and formatting is idempotent.
- Invalid syntax cannot produce edits; project preflight prevents partial edits
  on syntax failure, permissions survive, symlinks and ignored directories are safe.
- `--check` reports changed paths and returns 1 without writing; clean trees
  return 0. Usage errors return 2. Standard input produces source only.
- Real LSP requests format unsaved buffers with UTF-16 edits, retain comments,
  refuse invalid/closed documents and leave disk unchanged.

No wrapping policy, range formatting, import sorting or syntax changes.
