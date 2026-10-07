# Source formatting

```sh
tarn fmt                    # recursively format .tarn files below the current directory
tarn fmt main.tarn
tarn fmt src --check        # list files needing formatting; never writes
tarn fmt --stdin            # format a UTF-8 buffer from stdin to stdout
```

Formatting uses four spaces and LF, trims blank-line runs to one and adds a final
newline to nonempty sources. It retains comment text, literal spelling and
existing line breaks, including compact blocks and method chains. It never sorts
imports or repairs syntax. Parser and token/AST checks prevent changes to program
structure. Formatting does not require imports to exist or types to be valid.

Directories are recursive; symlinks, hidden subdirectories, `target`,
`node_modules` and `graphify-out` are skipped. Explicit symbolic-link inputs are
refused. Only `.tarn` files are selected; imported files outside the selected tree
are untouched. Empty directories succeed without edits.

All selected sources are checked before writing. Invalid syntax prevents all
edits in that invocation. Individual replacements are atomic and preserve file
permission bits. Unchanged files retain their modification time. I/O errors may
leave earlier successful replacements in place; there is no multi-file transaction.
Compiler test directories containing deliberately invalid syntax should be
formatted selectively, rather than selecting the whole compiler checkout.

Exit codes: 0 success/already formatted, 1 invalid syntax, I/O failure or files
needing formatting in `--check`, 2 invalid command usage. Normal formatting prints
only changed paths; `--stdin` prints only formatted source. Errors go to stderr.

In VS Code, select **Format Document** or configure:

```json
"[tarn]": {
  "editor.defaultFormatter": "tarn-lang.tarn",
  "editor.formatOnSave": true
}
```

Enable the Tarn language server to format documents. It uses unsaved buffers and
does not write disk directly. Canonical four-space indentation applies regardless
of editor tab preferences. Range/on-type formatting and configurable style are
outside the basic phase. See [ADR 0050](adr/0050-conservative-source-formatting.md).
