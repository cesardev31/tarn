# Tarn for VS Code

Language registration (`.tarn`) with its own file icon, syntax highlighting, comments, brackets,
auto-closing and indentation. Formatter and diagnostics integration come with
`tarn fmt` / `tarn check`.

Install locally while developing:

    ln -s "$PWD" ~/.vscode/extensions/tarn-lang.tarn-0.0.1

then reload VS Code.

Icons are generated from one geometry by `icons/make_icons.py` (SVG + PNG via Pillow);
edit that script, not the generated files.
