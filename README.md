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

Status: phase 4 (type checker) done; next is the typed IR. See [docs/roadmap.md](docs/roadmap.md).

```bash
cargo build --release
./target/release/tarn check examples/02_add.tarn
cargo test
```

Docs: [language](docs/language.md) · [ownership](docs/ownership.md) ·
[errors](docs/errors.md) · [architecture](docs/architecture.md) ·
[ADRs](docs/adr/). Editor: [editors/vscode](editors/vscode).
