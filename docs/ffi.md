# C interop (FFI)

Design: [ADR 0047](adr/0047-native-c-ffi.md). Linux x86_64, System V ABI.

## Calling C

```tarn
import "ffi"
extern "C" fn strlen(text *u8) usize

fn main() {
    match ffi.CString.new(&"héllo") {
        Some(text) => { unsafe { print(strlen(text.as_ptr())) } }   // 6
        None => {}
    }
}
```

Every `extern "C"` call requires `unsafe`. Native builds accept `bool`,
fixed-width integers, `usize`/`isize`, `f32`/`f64`, raw pointers and `void`
results. Other types (references, strings, structs, slices) remain valid in
declarations for contract checking (ADR 0031) but fail native builds. Symbols
resolve against libc and libm unless another library is granted at build time.

## Raw pointers

`*T` and `*mut T` are Copy machine addresses. They carry no loan and have
neither Transfer nor Share, so they cannot cross a native task boundary
(E3047), even inside a Copy struct. `*[]T` and `*any I` are rejected (E3072).

Creating and converting pointers is safe because nothing is accessed:

| `ffi` | |
|---|---|
| `null<T>() *mut T` | null pointer; type from context |
| `of(&T) *T`, `of_mut(&mut T) *mut T` | address of a borrowed value; the borrow ends at the call |
| `slice(&[]T) *T`, `slice_mut(&mut []T) *mut T` | address of the first element |
| `to_const(*mut T) *T` | the only mutability conversion |
| `address(*T) usize`, `from_address<T>(usize) *mut T`, `is_null(*T)` | numeric view, e.g. `(void*)-1` sentinels |

Because a pointer does not keep its pointee borrowed, the caller must keep the
pointee alive and unmoved for as long as C may use it. Reading memory through
a pointer is an `unsafe fn`:

| `ffi` unsafe | requires |
|---|---|
| `copy_bytes(*u8, count) Vec<u8>` | valid for reading `count` bytes |
| `string_from_c(*u8) Option<string>` | non-null, NUL-terminated; None if not UTF-8 |

`CString.new(&string) Option<CString>` makes an owned NUL-terminated copy
(None for interior NUL); `as_ptr()` is valid while the CString is alive and
unmoved.

## unsafe fn

`unsafe fn` declares a function whose callers must write `unsafe { ... }`
(E3071), after checking its documented requirements. Its own body is not
implicitly unsafe: unsafe operations inside still need a block.

## Linking libraries

```
tarn build app.tarn --link sqlite3
tarn run app.tarn --link :libsqlite3.so.0   # exact file when no dev symlink exists
```

Linking is a build-time grant, never something source code or an import can
request. `--link` accepts a library name (`-lname`) or `:exact-file`; option-
or path-like values are rejected. Missing symbols and libraries are reported by
name. Opaque C handles are ordinary empty structs used behind pointers:

```tarn
struct Db {}
extern "C" fn sqlite3_open(path *u8, db *mut *mut Db) i32
```

