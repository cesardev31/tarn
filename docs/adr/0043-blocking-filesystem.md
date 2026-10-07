# ADR 0043: owned blocking filesystem (Phase 15B)

Status: accepted.

## Decision and evidence

Phase 15A provides safe bytes/UTF-8 conversion. Examples 12 and 23 still
required an opaque filesystem contract, preventing real configuration loading
and directory inspection. Add `stdlib/fs` as a trusted layer depending on io;
its text helpers also import the ordinary string module. Core stays unchanged.
No path abstraction, process API or async filesystem is introduced.

`fs.File` is an owned, non-Copy regular-file descriptor. Moving transfers
ordinary post-drop responsibility. Destruction closes once; explicit `close`
consumes the owner and reports errors. No alive boolean, registry or user Drop.
Native capabilities explicitly grant Transfer and deny Share. Read/write/seek
and sync require `&mut self`; metadata uses `&self`. A File may be moved into a
native task and returned through its handle; sharing requires normal Mutex
serialization. Live loans prevent move/overwrite and read buffers retain
ordinary exclusive-borrow checking.

## Public API

- File.open (read-only), create (read/write, truncate), append, open_read_write
  (existing), create_new (exclusive read/write); `fs.open` is a convenience.
- File.read/write with slices and partial byte counts; zero denotes EOF for a
  nonempty read. Empty slices return zero without I/O. read_exact returns
  UnexpectedEof; write_all returns WriteZero when progress stops.
- File.seek using SeekFrom.Start(u64)/Current(i64)/End(i64); Start above
  i64::MAX returns InvalidInput before casting. File.metadata and sync_all.
- File.read_all and fs.read return owned Vec<u8>. read_text validates UTF-8;
  fs.write/write_text explicitly close and report close failures.
- metadata, exists, create_dir, remove_file, remove_dir, rename and read_dir.
- Metadata is Copy with len/is_file/is_dir. DirEntry owns its name/path;
  name/path borrow the entry, metadata inspects the current path.

Whole-file reads stream to EOF instead of trusting a preliminary stat size
(e.g. procfs has readable contents with zero st_size). They eagerly allocate
without a new size-limit framework. Allocation failure follows existing abort
policy. Text errors return InvalidData, including invalid directory names;
there is no lossy decoding. Paths are UTF-8 strings and embedded NUL is rejected
before syscalls. Linux byte-only paths are intentionally outside v0.

Directory enumeration returns Vec<DirEntry> in unspecified OS order, omitting
'.'/'..'. A private owned `_Directory` closes its DIR* on all ordinary exits,
including errors after partial enumeration. Returned entries copy native names
and do not retain libc storage. Name/path references cannot outlive the entry.

## Errors and syscall policy

Use the existing io.Error and natural try propagation. Append NotFound,
PermissionDenied, AlreadyExists, InvalidInput, InvalidData, NotDirectory,
IsDirectory, DirectoryNotEmpty and StorageFull to ErrorKind, preserving existing
variant indices. Native filesystem errno is captured in private domain 3 so
EINVAL becomes InvalidInput without changing networking InvalidAddress behavior.
Original native codes remain available. exists returns false only for NotFound
or NotDirectory; other errors propagate. Error.new creates application errors.

Retry EINTR in Tarn for open/read/write/seek/stat/sync and directory/path
operations. Linux close consumes the descriptor even when returning EINTR:
never retry, never revive the owner. Drop ignores ordinary close errors;
explicit close reports them. The generic main error reporter now handles every
ErrorKind and prints 'I/O error', with exit status 1 for normal failures.

File construction acquires a descriptor, wraps it immediately, then verifies
regular-file metadata in Tarn. A rejected resource is dropped normally. Open
uses O_CLOEXEC and O_NONBLOCK: the latter avoids waiting on a FIFO before it can
be rejected and has no effect on regular-file I/O. Pipes/devices/sockets are not
File resources in this phase. Directory enumeration uses opendir/closedir.
Files use 0666, directories 0777, respecting umask. Metadata follows symlinks;
remove_file unlinks the named entry. Rename follows Linux replacement rules.
No promise of sandboxing, path-race avoidance or transactional rollback.

write/write_text may truncate and leave partial contents on failure. Successful
close does not promise durability; use sync_all. There is no atomic-write,
recursive-delete, recursive-mkdir, permission-management or symlink API yet.

## Native boundary and verification

Tarn handles ownership, public APIs, retries, buffer accumulation, UTF-8,
error categories and whole-operation loops. C bridges individual OS/libc calls,
path termination, stat encoding and directory-name copying only. A separate
resolved filesystem intrinsic/resource catalog avoids treating files as sockets.
The frontend/post-drop verifier checks resource shape, capabilities, complete
intrinsic signatures and call operands; the backend checks native layouts.
Only verified drops call close/closedir; no backend move/borrow inference.

Private raw acquisition results temporarily contain Copy numeric handles,
wrapped exactly once before a fallible follow-up. This is the same documented
bootstrap resource boundary as sockets, not a user-accessible raw fd API.
The existing 40-byte io._Raw includes address bytes reused for a private stat
kind tag; that unrelated field name remains bootstrap debt.

## Validation and limits

Real temporary files, empty/large/binary/UTF-8 data, metadata/seek/append,
exclusive creation, directories, NUL rejection, FIFO rejection, native tasks,
all normal cleanup paths, syscall fault injection, fd traces, ABI corruption,
canonical memory safety and mutation fixtures validate this phase. No internet,
sleep-based proof or new dependency is required. Only Linux x86_64 is supported.
Path manipulation is 15C; process execution is 15D.
