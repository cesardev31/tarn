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
These are text viewers; live diagnostics and navigation require the planned LSP.
