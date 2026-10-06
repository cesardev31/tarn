# ADR 0003 — Newline-terminated statements, no semicolons

Status: **superseded by ADR 0008** (2026-10-05). Kept for history.

The lexer emits a `Newline` token at a line break only when the previous token
can end a statement (identifier, literal, `)`, `]`, `}`, `return`, `break`,
`continue`, `true`, `false`). Consecutive newlines collapse into one. A line
whose first token is `.` (but not `..`) continues the previous one, so method
chains can start lines with `.arg(...)`.

Why: no semicolons (less noise, fewer forms), but the rule is decided entirely
in the lexer with one token of lookbehind and lookahead, keeping the parser
simple. Unlike Go's semicolon insertion, the leading-dot rule allows the chain
style used by the process API.

Hard to reverse: changing it later rewrites every source file (the formatter
could automate it, but it is still a language change).
