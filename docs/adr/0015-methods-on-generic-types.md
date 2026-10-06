# ADR 0015 — Methods on generic types: `fn Pair<A, B>.first(&self) A`

Status: accepted (2026-10-05). Must be settled before the type checker.

## Problem

`fn T.m` (ADR 0013) has no way to name the type parameters of a generic `T`,
so `fn Pair.first(&self) A` cannot say what `A` is.

## Candidates, tested on four signatures

```tarn
// A. Owner parameters at the method (chosen)
fn Pair<A, B>.first(&self) A
fn Map<K, V>.get(&self, key &K) Option<&V>
fn Result<T, E>.unwrap(self) T
fn Pair<A, B>.swap(&self) Pair<B, A>
fn Pair<A, B>.map_first<C>(self, f fn(A) C) Pair<C, B>      // method's own generics

// B. Implicit: parameter names taken from the struct declaration
fn Pair.first(&self) A
fn Map.get(&self, key &K) Option<&V>

// C. Rust-style inherent impl block
impl<A, B> Pair<A, B> {
    fn first(&self) A
    fn swap(&self) Pair<B, A>
}

// D. Go-style receiver list
fn (self &Pair<A, B>) first() A

// E. Everything on the method
fn Pair.first<A, B>(&self) A
```

| Criterion | A `Pair<A,B>.m` | B implicit | C impl block | D receiver | E on method |
|-----------|-----------------|------------|--------------|------------|-------------|
| Readability | Every name used is declared in the same line. | Shortest, but `A` appears from nowhere; reader must open the struct. | Familiar to Rust users; methods far from the params when the block is long. | Familiar to Go users; breaks `fn T.m` mirroring the call site. | Mixes the type's params with the method's own params: which `<…>` binds what? |
| Parser | After `fn IDENT <list>`, the next token (`.` vs `(`) says whose list it was: LL, no backtracking. | Trivial. | New item form with nested bodies. | New form; receiver syntax duplicated. | Trivial but ambiguous semantically. |
| Resolution | Owner params are binders declared in the method scope; arity checked against the type (E2019). | Resolver must inject names from another declaration; renaming a struct param silently changes every method. | Block scope declares the params once. | Like A. | Cannot tell type params from method params. |
| Type checker | Receiver type = `Pair<A, B>` with `A`, `B` rigid; substitution at call sites is direct. | Same, after implicit injection. | Same. | Same. | Needs a rule to split the list. |
| Formatter | Single-line signature, no nesting. | Same. | Adds an indentation level for every method. | Same as A. | Same as A. |
| LSP / agents | Hover/rename on `A` finds a declaration on the same line; an agent editing one method needs no other context. | Agent must load the struct to know what `A` is; renaming params breaks methods non-locally. | Agent must find the enclosing block. | Fine. | Error-prone. |
| Coherence with Tarn | Extends `fn T.m` and `fn f<T>` with the same `<…>` list syntax. | Fits `fn T.m` unchanged. | Contradicts ADR 0013 (no inherent impl blocks). | Contradicts `fn T.m`. | Fits superficially. |

## Decision: A

- `fn Name<P1, …, Pn>.method<M…>(…)`: the owner's list are fresh **binders**
  for the type's parameters, in declaration order. Arity must match the type
  (E2019). Names are free (they need not match the struct's names), like
  function parameter names.
- Binders are plain names: no bounds and no concrete types on the owner in v0
  (`fn Pair<i32, B>.x` and `fn Map<K: Hash, V>.x` are E2021). Bounds belong to
  the type declaration. Conditional methods (only when `A: Display`) are not
  in v0.
- Non-generic types keep `fn User.new()`. Writing `fn Pair.first` for a
  generic `Pair` is E2019 with a fix-it showing the binders.
- `fn Result<T, E>.unwrap` is only legal in the module that defines `Result`
  (`core`), per ADR 0013 rule 5. User code cannot add methods to std types.

## Risks / debt

- Repeating `<A, B>` on every method of a generic type is verbose for types
  with many methods. Accepted: locality beats brevity for readers and agents.
- No conditional methods. Evidence to revisit: stdlib code (collections) that
  needs `fn List<T>.sort` only when `T: Ordered`; then allow bounds on owner
  binders (an extension of this syntax, not a new one).
