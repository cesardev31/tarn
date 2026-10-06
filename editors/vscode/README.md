# Tarn for VS Code

Language registration (`.tarn`) with its own file icon, syntax highlighting, comments, brackets,
auto-closing and indentation. Formatter and diagnostics integration come with
`tarn fmt` / `tarn check`.

Install locally while developing:

    ln -s "$PWD" ~/.vscode/extensions/tarn-lang.tarn-0.0.1

then reload VS Code.

Icons are generated from one geometry by `icons/make_icons.py` (SVG + PNG via Pillow);
edit that script, not the generated files.

Compiler snapshots have dedicated highlighting and icons: `.diag` (diagnostics),
`.ast` (syntax trees), and `.res` (name resolution). Resolution snapshots are
recognized by their `== ` header; workspace settings associate `tests/**/*.res`
with Tarn Resolution without taking over ReScript files globally.
Snapshot modes are text viewers; compiler diagnostics and navigation apply to `.tarn` files.

## Language server

Build from the repository root with `cargo build -p tarn-lsp`, then run
**Developer: Reload Window** once to activate the extension's language client.
The extension searches local `target/debug` and `target/release` directories,
then `PATH`. Set `tarn.serverPath` to use a different binary. After rebuilding,
run **Tarn: Restart Language Server**.

The initial server reports compiler errors and warnings on open/change/save,
including unsaved imported buffers, with UTF-16 positions. It supports hover
(local/parameter types and symbol information) and go to definition for
resolved names. Only file-backed `.tarn` documents are supported. Each open
file is checked as an entry; its directory remains the driver's module root.
Checks are synchronous and recheck all open entries; workspace-wide indexing,
completion, rename, formatting and incremental analysis are future work.
Run `python3 tools/lsp/tests/smoke.py` after building to verify stdio integration.
