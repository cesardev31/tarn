# Phase 20 report: basic canonical formatter

Status: implemented and validated.

## What was implemented

`tarn_fmt` is a shared library over the real lexer/parser with AST-derived
operator roles, comment/literal preservation, canonical indentation/spacing and
relex/reparse token/AST checks. The CLI adds recursive `tarn fmt`, `--check` and
`--stdin`, syntax preflight and atomic per-file replacements preserving permission
bits. The LSP formats unsaved buffers with UTF-16 edits using the same library.
No external dependency, new syntax or semantic compiler rule was added.

## Tests

Six formatter tests cover golden layout, CRLF/Unicode/comments, operator roles,
generics/references, continuations, invalid input, empty buffers and the valid
repository syntax corpus. All 387 accepted corpus files must format twice with an
identical result; token kinds/spelling/newline boundaries, comments and canonical
AST are also checked inside the formatter itself.

Three CLI tests cover recursive selection, check-only mode, syntax-error
preflight, permissions, ignored folders, symlinks, missing files, stdin/usage and
native execution before/after formatting. Real stdio and VS Code extension-host
tests validate unsaved formatting and unchanged disk (see Phase 21 report).
Final workspace and installed-binary results are recorded below.

## Bugs found

Naive lexical spacing would join separate `& &` or `> >` tokens and confuse
comparisons/shifts with generic delimiters. AST-derived binary roles plus explicit
non-fusion rules preserve those cases; corpus round-trip tests cover them.
Continuation indentation must survive comment-only lines and multiline generic
lists, rather than resetting at the intervening comment. Regression tests cover it.

## Decisions I would defend

Preserve existing physical line breaks for the first formatter: Tarn assigns
meaning to them. Refuse recovered syntax rather than guessing a repair. Share
one implementation between CLI/editor and check its output before exposing edits.
Preflight all source syntax before edits and replace only changed files.

## Decisions I still question

A future formatter may justify canonical wrapping and compact-block expansion;
that requires explicit parser-boundary evidence and comment placement decisions.
Four spaces are fixed for now; there is no measured need for configuration.

## Known limitations

No line-width policy, import sorting, range/on-type formatting or style knobs.
Syntax errors prevent formatting; semantic errors and missing imports do not.
Directory selection skips symlinks/hidden/build/vendor paths as documented.
Per-file atomic writes are not a multi-file transaction on I/O failure.

Design: [ADR 0050](adr/0050-conservative-source-formatting.md).
Usage: [formatting](formatting.md).

## Final validation

- `cargo test -p tarn -p tarn-lsp -p tarn_fmt`: 29 tests pass (22 CLI, one LSP
  position test, six formatter tests), including native execution equivalence.
- `cargo test --workspace --no-fail-fast -- --skip native_line_deletions_never_panic_or_emit_invalid_code --skip line_deletions_never_panic`:
  passes outside the sandbox, including socket/process/concurrency and ownership
  tests. The two long compiler mutation tests already passed during Phase 19;
  they were not repeated for these tooling-only changes. One existing native
  benchmark remains ignored.
- `cargo build -p tarn -p tarn-lsp --release` and the debug build pass without
  warnings. Both installed binaries in `~/.local/bin` match release SHA-256.
- An external temporary project passes installed `fmt`, `fmt --check`, unchanged
  file/mtime verification, `fmt --stdin`, native execution and test execution.
- `git diff --check` passes. Editor acceptance is recorded in the
  [Phase 21 report](phase-21-report.md).
