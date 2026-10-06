# ADR 0004 — Files are modules, no `package` declaration

Status: accepted

The module path is the file path relative to the project root. There is no
`package` line: it duplicates information the file system already has and is
one more thing for humans and agents to keep in sync. `import "fs"` names a
stdlib module, `import "app/config"` a local one; the last path segment is the
qualifier (`config.load()`). Imports are deterministic (no search paths beyond
project root and stdlib) so modules can be cached independently.
