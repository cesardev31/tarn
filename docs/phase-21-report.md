# Phase 21 report: VS Code language support

Status: implemented and validated.

## What was implemented

Completed the existing language-registration/TextMate/icon foundation. Highlighting
now distinguishes integer/float literals and recognizes generic owner methods and
contextual syntax. Added six source snippets and identifier configuration. The
existing compiler-backed LSP now exposes shared document formatting.

Extension lifecycle transitions are serialized. Restart remains registered in
highlighting-only mode; enabling/disabling and changing the server path work
without reload. Configured relative paths and the child working directory use
the first workspace folder, allowing external application projects. Startup
failure releases watchers and can be retried. Updated installation/usage guidance
and package exclusion rules. No production Node dependencies were added.

## Tests

Seven Node tests cover lifecycle, failure recovery, assets, numeric/method/contextual
syntax and snippet defaults parsed through the real Tarn frontend. The TextMate
acceptance script uses VS Code's bundled TextMate and Oniguruma engines to tokenize
source and compiler snapshots; no regex engine was reimplemented in production.

Real stdio tests retain diagnostics/navigation/async coverage and add unsaved
formatting, UTF-16 edits, invalid/closed-document errors, idempotence and unchanged
disk. An isolated real VS Code extension host also passes activation, applying
format edits, unsaved compiler diagnostics, disable/enable and restart. Its
`test-result.json` confirms success independently of the launcher exit status.
It uses temporary user-data/extensions directories and never saves the source.
Final workspace and rebuilt-artifact results are recorded below.

## Bugs found

The previous float regex also matched ordinary integers; generic owner methods
received incomplete declaration highlighting. Both have actual-engine regressions.
The old activation returned immediately when disabled, leaving no listener to
start the server after enabling. Server-path changes did not restart it, and its
working directory always pointed at the extension checkout. Lifecycle tests now
cover all three behaviors. Real VS Code may minimize the LSP full-document edit
into several smaller edits; acceptance verifies the applied result rather than
assuming one returned edit.

## Decisions I would defend

Keep language semantics in the existing compiler/LSP, with one shared formatter.
Test shipped grammar through the actual engines and validate an actual extension
host, rather than stopping at manifest inspection. Retain lightweight syntax-only
mode and explicit server selection without automatically building executables.

## Decisions I still question

One synchronous client/check of every open entry is simple but may need measured
scheduling changes for larger applications. Multi-root server selection is still
based on the first workspace folder; broader behavior needs actual usage evidence.

## Known limitations

Only file-backed Tarn documents, full-buffer synchronization and document format.
No completion, rename, range/on-type format, workspace indexing, debugger or
marketplace publication. Binaries are installed separately from the extension.
Extension JavaScript/grammar changes need a window reload; server-only rebuilds
need the existing restart command. No Windows/macOS support was added.

Usage and validation commands: [VS Code guide](../editors/vscode/README.md).

## Final validation

- `npm test` in `editors/vscode`: seven tests pass, including snippet defaults
  parsed by the real Tarn CLI.
- `ELECTRON_RUN_AS_NODE=1 /usr/share/code/code editors/vscode/tests/textmate.cjs`:
  real TextMate/Oniguruma source and snapshot tokenization passes.
- `python3 tools/lsp/tests/smoke.py`: all three acceptance groups pass after
  rebuilding the effective debug server, including formatting failure codes.
- The isolated VS Code extension-host acceptance writes `{ "ok": true }` after
  real activation, formatting, unsaved diagnostics, disable/enable and restart.
- Release/debug binaries were rebuilt; `tarn-lsp` and `tarn` installed in
  `~/.local/bin` match the release artifacts. External CLI use passes.
- Workspace validation and its two previously passed mutation-test exclusions
  are documented in the [Phase 20 report](phase-20-report.md).

The user's existing extension symlink already points to this checkout. Reload
that VS Code window once to load the changed JavaScript, grammar and snippets;
no user session/window was reloaded automatically by this work.
