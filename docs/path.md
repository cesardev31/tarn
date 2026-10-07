# Lexical paths

```tarn
import "path"
import "fs"
import "io"
fn main() Result<void, io.Error> {
    root := path.Path.new("/opt/xlinux")
    config := root.join(&"config.txt")
    text := try fs.read_text(config.as_string())
    print(text)
    return Ok(())
}
```

`Path.new(string)` consumes its input; `from_text(&string)` copies it.
`as_string()` borrows the path, `into_string()` consumes it, and `clone()` copies.
Path is non-Copy, Transfer and Share through ordinary owned-string rules.
A borrowed text view prevents moving/overwriting its owner while the loan is live.

Methods: `is_empty`, `is_absolute`, `is_relative`, `is_valid`, `join`,
`normalize`, `components`, `parent`, `file_name`, `file_stem`, `extension`,
`with_extension`, `starts_with`, `strip_prefix`. Lexical free functions accept
borrowed strings and return strings rather than Paths. Components return
`Vec<string>`; optional names/extensions are `Option<string>`, and optional
parent/replacement/prefix-removal methods return `Option<Path>`.

Operations are lexical and never access the disk. Slash is the only separator;
Unicode names are preserved. Components omit empty segments and `.` but retain
`..`. Prefixes match whole components: `/app` does not prefix `/apple`.
Join never normalizes. Only explicit `normalize` cancels `..`; it cannot resolve
symlinks and must not be used as a filesystem sandbox check.

Empty/root/dot-only paths have no file name. Final `..` also has none. A lone
relative name has an empty parent. `.hidden` has no extension; `name.` has an
empty extension. Empty `with_extension` removes the extension; slash/NUL suffixes
return None. Replacement and prefix removal reconstruct component spelling.

Construction preserves text verbatim. `is_valid` checks nonempty/NUL-free input,
not permissions or existence. `fs` still validates paths at its syscall boundary.
Multiple leading slashes normalize to one POSIX root; Windows and raw non-UTF-8
filesystem paths are unsupported. See [ADR 0044](adr/0044-lexical-owned-paths.md).
