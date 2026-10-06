# ADR 0008 — Newlines are tokens; the parser decides statement ends

Status: accepted (2026-10-05). Supersedes ADR 0003.

Tarn still has no semicolons. What changed is *who* decides.

## Lexer

The lexer preserves every line break as a `Newline` token, with two purely
lexical simplifications: a run of blank lines (and comment-only lines)
collapses into one `Newline`, and no `Newline` is emitted before the first
token of the file. The lexer has no notion of "can this token end a statement".

## Parser

The parser treats `Newline` as significant or insignificant depending on
syntactic context:

1. **Inside `( )`, `[ ]`, `< >` type arguments and struct/array literal
   `{ }`**, newlines are ignored entirely. An argument list, an index or a
   literal can span lines freely.
2. **Inside blocks `{ }`** (function bodies, `if`/`for`/`match` bodies,
   closures) newlines are significant. A statement ends at a `Newline`, at the
   closing `}` or at end of file. Anything else after a complete statement is
   E1005.
3. **Within an expression in a block**, a newline is skipped whenever the
   expression is *incomplete*: after a binary or prefix operator, `:=`, `=`,
   `=>`, `,`, `.`, or an opening delimiter. So `a +⏎ b` is one expression.
4. **Leading-dot continuation**: in the postfix position of an expression, if
   the next significant token after one or more newlines is `.`, the newlines
   are skipped and the member access continues (method chains). `..` does not
   continue a line.
5. A binary operator at the *start* of a line does not continue the previous
   line: `a⏎+ b` is two statements (the second one, `+ b`, is an error). The
   formatter always puts the operator at the end of the broken line.
6. `else` must be on the same line as the `}` that closes the `if` block
   (E1016). One layout, and `}⏎else` would otherwise be ambiguous with a new
   statement.

## Why

- The lexer stays context-free and reusable (formatter, highlighting, LSP
  tokens) and keeps all line information.
- Decisions that depend on grammar (am I inside a call? is the expression
  complete?) are made where the grammar is known, with precise diagnostics.
- The rules above are local (one token of lookahead past newlines), so the
  parser stays a simple recursive descent.

Hard to reverse: it defines how every multi-line construct is written.
