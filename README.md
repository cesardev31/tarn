<p align="center"><img src="docs/assets/logo.svg" alt="Tarn" width="360"></p>

# Tarn

A memory-safe systems language without a mandatory GC: ownership and borrowing
checked at compile time, inferred lifetimes, small regular syntax, official
tooling. Linux x86_64 only for now.

```tarn
fn add(a i32, b i32) i32 {
    return a + b
}

fn main() {
    value := add(20, 22)
    print(value)
}
```

Status: ownership and executable drops are implemented; the initial Cranelift Linux x86_64 backend supports `build` and `run` (ADR 0027). See [docs/roadmap.md](docs/roadmap.md).

```bash
cargo build --release
./target/release/tarn check examples/02_add.tarn
cargo test
```

Docs: [language](docs/language.md) · [ownership](docs/ownership.md) ·
[errors](docs/errors.md) · [architecture](docs/architecture.md) ·
[ADRs](docs/adr/) · [dependency security (planned)](docs/dependency-security.md). Editor: [editors/vscode](editors/vscode).

## Native execution (Linux x86_64)

Requires system `cc`, C11/libc headers and libm. From a compiler checkout:

```bash
cargo run -p tarn -- run tests/native/pass/milestone.tarn
# 42
cargo run -p tarn -- build tests/native/pass/milestone.tarn -o /tmp/tarn-milestone
/tmp/tarn-milestone
```

`tarn build file.tarn` writes `file` beside the source; `tarn run` uses a temporary
executable and removes it. Scalars, concrete structs/enums/arrays, thin references,
direct calls and executable drops are supported. Reachable generic instances, borrowed
slices, borrowed nonescaping closures and escaping owned `move fn` closures also execute. Stored callables support shared, mutable and consuming invocation (ADR 0032). Borrowed dynamic interface calls also execute. Owned `spawn move fn` tasks execute on pthreads with consuming join and join on handle destruction (phase 11A). Scoped borrowing and synchronization remain pending. Unmodeled external ABI
execution remains unsupported; unknown std APIs reject semantically with E3040.
See [ADR 0027](docs/adr/0027-native-backend.md) for exact ABI, layout and restrictions.

Native feature completion after phase 7 adds reachable generic specialization,
concrete generic ADTs, borrowed fat slices, nonescaping borrowed closures and
checked shifts/float-to-int casts. Unknown bootstrap stdlib APIs now fail with
E3040. See [ADR 0028](docs/adr/0028-native-feature-completeness.md) and
[ADR 0029](docs/adr/0029-checked-shifts-and-float-casts.md). Callable locals retain current consuming semantics; optimization remains deferred.
Borrowed dynamic interfaces and declaration-derived semantic contracts are covered
by [ADR 0030](docs/adr/0030-borrowed-dynamic-interfaces.md),
[ADR 0031](docs/adr/0031-declaration-semantic-contracts.md) and the
[phase report](docs/dynamic-interface-report.md).
