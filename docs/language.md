# The Tarn language (draft 0)

Tarn is a systems language with compile-time memory safety (ownership and
borrowing, no mandatory GC), inferred lifetimes, a small regular syntax and
official tooling. This document is the normative description of the syntax and
semantics *as currently designed*. Sections marked **(provisional)** are not
implemented yet and may change; every change must update `examples/`.

Guiding rule for every feature: **one obvious way to write it**. If two forms
express the same thing, one of them is removed.

## 1. Lexical structure

- Source files are UTF-8, extension `.tarn`.
- Comments: `// line` and `/// doc comment` (attached to the next item).
  There are no block comments (one form only; also easier for line-based tools).
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. ASCII only in draft 0.
- There are no semicolons. Statements end at a line break, decided by the
  parser (ADR 0008):
  - inside `( )`, `[ ]` and struct/array literals, line breaks are ignored;
  - an incomplete expression continues on the next line: a line ending in a
    binary operator, `,`, `:=`, `=`, `=>`, `.` or an opening delimiter;
  - a line whose first token is `.` continues the previous expression
    (method chains);
  - a line starting with a binary operator does **not** continue the previous
    one — break lines *after* the operator;
  - `else` goes on the same line as the closing `}`.
- Integer literals: `42`, `1_000`, `0xff`, `0b1010`, `0o755`.
- Float literals: `1.5`, `2.0e-3`, `1e9`. A float needs a digit on both sides
  of the `.` (`1.` and `.5` are errors) so `0..10` is never ambiguous.
- String literals: `"text"` with escapes `\n \t \r \\ \" \0 \u{1F600}`.
  Strings may not contain raw newlines in draft 0.

Keywords:

```
fn return if else for in break continue match
struct enum interface impl copy
var pub import
true false
try spawn unsafe extern mut
```

`self`, `any` and `scope` are contextual: identifiers with a special meaning in
one position (receiver, dynamic interface type, structured-concurrency block).

## 2. Modules and imports

A file is a module. A directory of files is a package-less module tree; the
module path is the file path relative to the project root, without extension.
There is no `package` line: the file system already says it (one less thing to
keep in sync). See ADR 0004.

```tarn
import "fs"
import "process"
import "app/config"     // local module app/config.tarn

fn main() {
    cfg := config.load()
}
```

Items are private to their module unless marked `pub`. Module-level items can
be used before their declaration; locals cannot. Types, functions and values
share one namespace per scope. Imports are private to the importing module
and may form cycles. Full rules: `docs/resolution.md`.

## 3. Bindings

```tarn
count := 10            // immutable, type inferred (i64 for a bare integer literal)
name := "hello"        // string
limit: u64 := 100     // immutable, explicit type
var total: i64 = 0     // mutable, explicit type
var index = 0          // mutable, inferred
```

- `name := value` / `name: Type := value` declares an **immutable** binding.
- In statement position a type annotation is introduced by `:`; in
  declaration lists (parameters, fields) the type follows the name directly:
  `fn f(a i32)`, `struct S { a i32 }` (ADR 0011).
- `var name = value` / `var name: Type = value` declares a **mutable** binding. Type annotation optional; initializer
  required (no uninitialized variables in draft 0).
- `=` assigns to an existing mutable place.
- **Shadowing** (ADR 0009):
  - redeclaring a name in the **same scope** is an error (E2003);
  - a **nested** scope may shadow a name from an enclosing scope;
  - the compiler warns (W2001) when the shadowed local is used again after
    the inner scope ends, or when it is a `var`; shadowing an import or a
    module-level item is W2002.

```tarn
fn parse(line string) {
    count := 0
    // count := 1             // error[E2003]: already declared in this scope
    for part in line.split(",") {
        line := part.trim()   // ok: shadows the parameter inside the loop
        print(line)
    }
}
```

## 4. Types

Primitive: `bool`, `i8 i16 i32 i64`, `u8 u16 u32 u64`, `isize usize`,
`f32 f64`, `string`, `void`, `never`.

- `string` is an owned, immutable-by-default UTF-8 string (heap). A string
  literal has type `string` in draft 0; a borrowed view is `&string`.
- `void` is the unit type (functions without a return type return `void`).
  Its only value is written `()`, e.g. `return Ok(())`.
- `never` is the type of expressions that do not return (`panic(...)`, `return`).

Compound:

| Form | Meaning |
|------|---------|
| `[N]T` | fixed-size array, a value |
| `[]T` | slice (unsized; only usable behind a reference: `&[]T`, `&mut []T`) |
| `&T` | shared reference |
| `&mut T` | mutable (exclusive) reference |
| `fn(A, B) R` | function type |
| `Name<T>` | generic instantiation (type position only) |

There is no `null`. Absence is `Option<T>`.

Numeric conversions are always explicit; conversions are written `u64(x)` (checked: panics if the value does not fit,
in all build profiles) or `u64.wrap(x)` (wrapping). Integer arithmetic overflow panics in
both debug and release (same safety in every profile).

## 5. Functions

```tarn
fn add(a i32, b i32) i32 {
    return a + b
}

fn log(message string) {        // returns void
    print(message)
}
```

- Parameter syntax is `name Type`; return type follows the parameter list.
- `return` is required to return a value; the last expression of a block is
  **not** an implicit return (one way to return).
- Functions may be declared in any order within a module.

## 6. Control flow

```tarn
if x > 0 {
    print("positive")
} else if x < 0 {
    print("negative")
} else {
    print("zero")
}

for i in 0..10 { }       // range, exclusive end; 0..=10 inclusive
for item in list { }     // iteration (provisional: iteration protocol)
for running { }          // while-style
for { }                  // infinite
```

- `for` is the only loop keyword.
- Braces are mandatory; parentheses around conditions are not used.
- `if`, `for` and `match` are statements; they do not produce values. A
  value-dependent result is written with a mutable binding or a `return`
  from each branch (provisional; see open questions in the roadmap).
- Struct literals are not allowed directly in `if`/`for`/`match` heads; wrap
  them in parentheses (avoids the `if x == P{...} {` ambiguity).

## 7. Structs and methods

```tarn
struct User {
    name string
    age  u32
}

copy struct Point {        // opt-in Copy: all fields must be copy
    x f64
    y f64
}

fn User.new(name string) User {          // associated function
    return User{name: name, age: 0}
}

fn User.greet(&self) {                   // shared borrow of the receiver
    print(self.name)
}

fn User.birthday(&mut self) {            // exclusive borrow
    self.age = self.age + 1
}

fn User.into_name(self) string {         // consumes the receiver
    return self.name
}
```

Enums follow the same rule: `copy enum Light { Red Green }` is copied on
assignment; a plain `enum` moves. Copy types must contain only copy types.

The declaration mirrors the call site: `User.new("ana")`, `u.greet()`.
There is no `impl` block for inherent methods.

Methods of a **generic type** name the type's parameters right after the type
(ADR 0015); the names are binders, one per parameter, in order:

```tarn
struct Pair<A, B> {
    first  A
    second B
}

fn Pair<A, B>.swap(self) Pair<B, A> {
    return Pair{first: self.second, second: self.first}
}

fn Pair<A, B>.map_first<C>(self, f fn(A) C) Pair<C, B> {   // own generics after the name
    return Pair{first: f(self.first), second: self.second}
}
```

Methods can only be declared in the module that declares the type.

Struct literal: `User{name: "ana", age: 30}`. All fields must be given.
Field shorthand `User{name, age}` is allowed when a local of the same name exists.

## 8. Enums and pattern matching

```tarn
enum Shape {
    Circle(f64)
    Rect(f64, f64)
    Empty
}

fn area(s &Shape) f64 {
    match s {
        Circle(r) => return 3.14159 * r * r
        Rect(w, h) => return w * h
        Empty => return 0.0
    }
}
```

- In a pattern, variants of the scrutinee's enum are written unqualified (the
  type is known). In expressions, variants are qualified: `Shape.Circle(2.0)`.
- Exception: the prelude variants `Some`, `None`, `Ok`, `Err` are always
  available unqualified.
- `match` must be exhaustive. `_` is the wildcard.
- Arms are `pattern => statement` or `pattern => { block }`, one per line.
- In patterns, a **capitalized** name (`Empty`, `Circle(r)`) is a variant and
  a lowercase name (`n`) is a new binding (ADR 0014). Variant names must
  start with an uppercase letter.
- Patterns: literals, `_`, bindings, variants, struct patterns
  `User{name, age: 0}`, ranges `0..=9`. Guards: `n if n > 0 =>` (provisional).

## 9. Errors

`Result<T, E>` and `Option<T>` are ordinary prelude enums.

```tarn
fn read_config(path &string) Result<Config, Error> {
    text := try fs.read_text(path)
    return Ok(parse(&text))
}
```

`try expr` evaluates `expr`; on `Err(e)` (or `None`) it returns early from the
enclosing function, converting the error with `Error.from` when the types
differ (provisional). `try` is a prefix keyword rather than a postfix `?`
because it is visible at the start of the line, reads the same to people and
models, and makes control flow greppable. See ADR 0005.

`panic("msg")` aborts the process. There are no exceptions.

## 10. Ownership and borrowing

Summary (full rules in `ownership.md`):

- Every value has one owner. Assignment, passing by value and returning **move**
  non-copy values. Using a moved value is error E4001.
- `&x` borrows shared, `&mut x` borrows exclusively. At any point: many `&` or
  one `&mut`, never both.
- Lifetimes are **inferred**; there is no lifetime syntax. A function that
  returns a reference has an inferred *provenance* (which reference parameters
  the result may borrow from), stored in its interface (ADR 0010). When the
  compiler cannot determine or prove it, the program is rejected.

## 11. Generics (provisional, phase 13)

```tarn
fn identity<T>(value T) T {
    return value
}

fn largest<T: Ordered>(items &[]T) &T { ... }
```

Generic arguments are written only in type positions. Inside expressions they
are inferred; when they can't be, annotate the binding: `x: Option<i32> := None`.
This is a **v0 restriction, not a permanent promise** (ADR 0012): a syntax
for explicit generic arguments may be added once real code shows the need.
No explicit turbofish; this keeps `<` unambiguous for the parser.

## 12. Interfaces (provisional, phase 14)

```tarn
interface Writer {
    fn write(&mut self, data &[]u8) Result<usize, Error>
}

impl Writer for File {
    fn write(&mut self, data &[]u8) Result<usize, Error> { ... }
}

fn save<W: Writer>(w &mut W) { ... }     // static dispatch (monomorphized or shared)
fn save_any(w &mut any Writer) { ... }    // dynamic dispatch, explicit with `any`
```

Implementations are explicit (`impl I for T`), not structural: intent is
visible and greppable, and adding a method never silently changes which
interfaces a type satisfies.

Coherence (ADR 0016): an `impl I for T` must be written in the module that
declares `I` or the one that declares `T`; `T` is a struct or enum; its type
arguments are binders (`impl I for Pair<A, B>`); at most one impl per pair.

## 13. Concurrency (provisional, phase 25)

```tarn
ch: Channel<Result<Bytes, Error>> := channel(8)
scope {
    spawn download(url, ch.sender())
    spawn download(mirror, ch.sender())
}                      // scope waits for every task spawned inside it
```

Values sent across tasks are moved. Shared mutable state requires an explicit
synchronization type (`Mutex<T>`). There is no `async`/`await` in the plan.

Channels are created with `channel(capacity)` and sent to through a
`Sender<T>` (`ch.sender()`); the element type comes from the
binding: `ch: Channel<Result<usize, Error>> := channel(8)` (no explicit generic
arguments in expressions in v0, ADR 0012).

## 13b. Closures (provisional)

`fn(x) T { ... }` is an anonymous function; parameter types may be omitted when
inferred from context. Captures are borrows unless the closure is passed to
`spawn` (then they are moves).

## 14. Unsafe and FFI (provisional)

```tarn
extern "C" fn getpid() i32

fn pid() i32 {
    unsafe {
        return getpid()
    }
}
```

`unsafe { ... }` is a block statement (blocks never produce values).

`unsafe` enables a fixed list of operations (call extern functions, deref raw
pointers `*T`, access mutable statics). Ownership and type checks still apply
inside `unsafe`.

## 15. Builtins (draft 0)

- `print(value)` — prints a primitive or string followed by a newline.
- `panic(message)`.

These move into `core` once the stdlib exists.

## Appendix: notes for implementers

- `>>` is lexed as one token; the parser splits it when closing nested generic
  arguments in type position (`Option<Option<i32>>`). Same for `&&` in
  prefix/type position (`&&x` is a reference to a reference).
- Compound assignment (`+=`) does not exist: `x = x + 1` is the one form.
- The grammar actually implemented by the parser is in `docs/grammar.md`.
