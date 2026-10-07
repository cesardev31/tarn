# Filesystem (Phase 15B)

Import `fs` and `io`. Operations block the current native thread and return
`Result<..., io.Error>`. Files are owned resources closed automatically on drop.

```tarn
import "fs"
import "io"

fn main() Result<void, io.Error> {
    try fs.write_text(&"message.txt", &"hello, café\n")
    text := try fs.read_text(&"message.txt")
    print(text)
    var file = try fs.File.open(&"message.txt")
    var buffer = [8]u8{0, 0, 0, 0, 0, 0, 0, 0}
    count := try file.read(&mut buffer)
    print(count)
    try file.close()
    return Ok(())
}
```

| API | Behavior |
|---|---|
| File.open / fs.open | Existing regular file, read-only |
| File.create | Read/write, create or truncate |
| File.create_new | Exclusive read/write creation; AlreadyExists on conflict |
| File.append | Write-only, create if absent, OS append writes |
| File.open_read_write | Existing regular file, read/write |
| read / write | &mut self, borrowed slices, actual partial byte count |
| read_exact / write_all | Loop to completion, UnexpectedEof / WriteZero on no progress |
| seek(SeekFrom) | Start(u64), Current(i64), End(i64); returns new byte offset |
| metadata / sync_all | Copy metadata / explicit durability request |
| close | Consumes File; cannot close or use it again |
| read_all / fs.read | Owned Vec<u8>, streamed to EOF |
| fs.read_text | Owned validated UTF-8 string; InvalidData on malformed input |
| fs.write / write_text | Create/truncate, complete write, report explicit close errors |
| fs.metadata | Metadata with len, is_file, is_dir; follows symlinks |
| fs.exists | Result<bool>, propagates errors other than absence/non-directory |
| create_dir / remove_dir | One directory / empty directory only |
| remove_file / rename | Linux unlink / rename semantics |
| read_dir | Owned Vec<DirEntry>, unspecified order; excludes '.'/'..' |
| DirEntry.name / path / metadata | Borrowed UTF-8 strings / current path metadata |

Use `entries.as_slice()` to iterate. Entry strings remain owned after native
enumeration closes; a borrowed name cannot outlive its entry. Directory names
with invalid UTF-8 produce InvalidData instead of silently changing names.

Read zero on a nonempty buffer is EOF; empty reads/writes return zero. Seek
Start values above i64::MAX and embedded NUL in paths return InvalidInput.
Whole-file helpers do not impose a size limit and may exhaust memory on large
inputs. Empty files are valid empty strings. Embedded NUL in file contents is
valid text; NUL in paths is rejected.

Files are Transfer, not Share; move one into a native task or serialize shared
use with Mutex. A move or consuming close cannot coexist with a still-used
loan. There is no public raw descriptor, fd duplication or async file API.

File operations accept regular files only, including procfs regular files.
FIFO/device/socket resources are rejected. Paths use UTF-8, metadata follows
symlinks, creation respects umask, and descriptor creation uses close-on-exec.
Writing may leave a truncated/partial file on error. Close is not a durability
promise; sync_all is explicit. File and directory close are never retried.

New errors are NotFound, PermissionDenied, AlreadyExists, InvalidInput,
InvalidData, NotDirectory, IsDirectory, DirectoryNotEmpty and StorageFull.
Inspect with match; native_code preserves OS diagnostics. Error.new(kind) creates
an application error. See [ADR 0043](adr/0043-blocking-filesystem.md).
