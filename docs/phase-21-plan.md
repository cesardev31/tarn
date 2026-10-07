# Phase 21: VS Code language support

Status: implemented and validated.

## Existing foundation

Language registration, TextMate grammar, Tarn icons and compiler snapshot modes
were already present. The stdio language server already reused compiler facts for
unsaved diagnostics, hover and definition. Preserve that implementation.

## Completion scope

- Align highlighting with current source syntax: numeric categories, generic
  owner methods, FFI, async and contextual keywords.
- Keep comment/string precedence, doc continuation, brackets and identifier rules.
- Add snippets for ordinary functions, tests, structs, enums, loops and Result
  matching using actual Tarn syntax.
- Expose the shared Phase 20 formatter through standard LSP document formatting.
- Make enable/disable, server-path changes and restart work without reloading.
  Register restart even when starting in highlighting-only mode; serialize server
  transitions, dispose watchers and recover cleanly after startup failures.
- Resolve configured relative binaries against the workspace and use its working
  directory, including projects outside the compiler checkout.
- Document local installation, formatting, server discovery and current limits.

## Acceptance

- Extension lifecycle tests for highlighting-only activation, configuration,
  serialized restarts and failure recovery; manifest/assets/snippets checks.
- Actual TextMate/Oniguruma tokenization using the engines shipped with VS Code.
- Real stdio LSP smoke tests for formatting, diagnostics, UTF-16 and navigation.
- Isolated real VS Code extension-host acceptance for activation, formatting and
  unsaved diagnostics, disable/enable and restart; user files/settings unchanged.

No independent parser, completion engine, rename, workspace index, debugger,
marketplace publication or Windows/macOS work is authorized by this phase.

Completed validation and known limits: [Phase 21 report](phase-21-report.md).
