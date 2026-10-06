//! Tarn IR: a typed control-flow graph per function (ADR 0023).
//!
//! The IR makes every semantic decision of the type checker explicit, so that
//! ownership/borrow analysis and backends never re-derive them:
//!
//! - reads of places are `Copy` or `Move` operands (decided with
//!   `Decls::is_copy`), borrows are `Ref` rvalues, auto-deref is a `Deref`
//!   projection, coercions are `Coerce` rvalues;
//! - calls name a resolved `FunctionId` (with type arguments), an interface
//!   method (`Virtual`), an intrinsic, a builtin or a value;
//! - `Drop(place)` statements mark where owned values go out of scope, with
//!   *drop-if-initialized* semantics: ownership analysis decides which drops
//!   actually run (drop elaboration).
//!
//! Not SSA: locals are mutable slots assigned many times; places (`x.f`,
//! `(*r)[i]`) are first-class because borrow checking reasons about them.

mod lower;
mod pretty;
mod verify;

pub use lower::lower_program;
pub use pretty::{print_function, print_program};
pub use verify::verify;

use std::collections::HashMap;
use tarn_diagnostics::Span;
use tarn_resolve::SymbolId;
use tarn_types::{FloatTy, IntTy, ParamId, Ty};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

/// Local 0 is always the return place.
pub const RETURN: LocalId = LocalId(0);

#[derive(Debug, Default)]
pub struct Program {
    pub functions: Vec<Function>,
    pub by_symbol: HashMap<SymbolId, FunctionId>,
}

impl Program {
    pub fn function(&self, id: FunctionId) -> &Function {
        &self.functions[id.0 as usize]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FnKind {
    /// Has a Tarn body.
    Body,
    /// `extern "C"`: declared, implemented elsewhere.
    Extern,
    /// `extern "intrinsic"` in `core`: implemented by the compiler/backend.
    Intrinsic,
    /// A closure; its captures are its first `captures` parameters.
    Closure { parent: FunctionId, captures: u32 },
}

#[derive(Clone, Debug)]
pub struct Function {
    pub id: FunctionId,
    /// Qualified name for printing: `Pair.swap`, `main`, `main::closure#0`.
    pub name: String,
    pub symbol: Option<SymbolId>,
    pub kind: FnKind,
    /// Generic parameters (owner binders + own); the IR stays generic.
    pub generics: Vec<ParamId>,
    /// Parameters are locals `1..=param_count`.
    pub param_count: u32,
    pub ret: Ty,
    pub locals: Vec<LocalDecl>,
    pub blocks: Vec<BasicBlock>,
    pub span: Span,
}

impl Function {
    pub fn local(&self, l: LocalId) -> &LocalDecl {
        &self.locals[l.0 as usize]
    }

    pub fn params(&self) -> impl Iterator<Item = LocalId> {
        (1..=self.param_count).map(LocalId)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Return,
    Param,
    /// A named binding from the source (`x := …`, pattern, `for` variable).
    User,
    /// Compiler temporary holding an intermediate value.
    Temp,
}

#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub ty: Ty,
    pub kind: LocalKind,
    pub name: Option<String>,
    pub symbol: Option<SymbolId>,
    /// Declared `var` (only user locals can be mutable; temps are assigned
    /// once by construction, the return place by `return`).
    pub mutable: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub stmts: Vec<Statement>,
    pub term: Terminator,
    pub term_span: Span,
}

#[derive(Clone, Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    /// The local's storage becomes valid (it is not yet initialized).
    StorageLive(LocalId),
    /// The local's storage ends; any borrow of it must be dead by now.
    StorageDead(LocalId),
    /// Drop the value in `place` if it is initialized (drop elaboration
    /// after ownership analysis makes this precise).
    Drop(Place),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: LocalId,
    pub proj: Vec<Proj>,
}

impl Place {
    pub fn local(l: LocalId) -> Place {
        Place { local: l, proj: Vec::new() }
    }

    pub fn project(&self, p: Proj) -> Place {
        let mut x = self.clone();
        x.proj.push(p);
        x
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Proj {
    /// Through a `&` / `&mut`.
    Deref,
    /// Struct field (declaration order) or, after `Downcast`, variant field.
    Field(u32),
    /// Array/slice element; the index is a local holding a `usize`.
    Index(LocalId),
    /// View an enum place as one variant (index in declaration order).
    Downcast(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    /// Read a copy-type place; the place stays initialized.
    Copy(Place),
    /// Read a place by moving out of it.
    Move(Place),
    Const(Const),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Const {
    Int(i128, IntTy),
    Float(String, FloatTy),
    Bool(bool),
    Str(String),
    Unit,
    /// A function used as a value.
    Fn(FunctionId, Vec<Ty>),
    /// Value of an opaque standard-library expression (not compilable yet).
    Opaque,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Aggregate {
    Struct(SymbolId, Vec<Ty>),
    /// Enum value: enum symbol, variant index, type arguments.
    Variant(SymbolId, u32, Vec<Ty>),
    Array(Ty),
    /// A closure value; operands are its captures.
    Closure(FunctionId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoerceKind {
    /// `&mut T → &T`.
    MutToShared,
    /// `&[N]T → &[]T`.
    Unsize,
    /// `&T → &any I`.
    ToDyn(SymbolId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rvalue {
    Use(Operand),
    /// `&place` / `&mut place`.
    Ref(bool, Place),
    /// `&place[start..end]` / `&mut …`: a slice reference into an array or slice place.
    SliceRef { mutable: bool, base: Place, start: Option<Operand>, end: Option<Operand> },
    /// Integer arithmetic is checked: overflow panics in every build profile.
    Binary(BinOp, Operand, Operand),
    Unary(UnOp, Operand),
    Aggregate(Aggregate, Vec<Operand>),
    /// Checked numeric conversion `T(x)`.
    Cast(Operand, Ty),
    Coerce(CoerceKind, Operand, Ty),
    /// Variant index of an enum place.
    Discriminant(Place),
    /// Length of an array or slice place.
    Len(Place),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Print,
    Panic,
    Channel,
    /// End of a `scope { }`: waits for every task spawned inside it.
    JoinScope,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Callee {
    /// Statically resolved function or method, with type arguments.
    Fn(FunctionId, Vec<Ty>),
    /// Interface method on a type parameter or `any I`: chosen at
    /// monomorphization (static dispatch) or at run time (`any`).
    Virtual { method: SymbolId, type_args: Vec<Ty> },
    /// `extern "intrinsic"` from `core`, or a compiler-known array/slice method.
    Intrinsic(String),
    Builtin(Builtin),
    /// Call through a function/closure value.
    Value(Operand),
    /// Member of an opaque standard module (not compilable yet).
    Opaque(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminator {
    Goto(BlockId),
    /// Jump to the block of the matching value, else `otherwise`. Booleans
    /// switch on 0 (`false`) / 1 (`true`); enums on `Discriminant`.
    Switch { discr: Operand, cases: Vec<(i128, BlockId)>, otherwise: BlockId },
    /// `next` is `None` when the callee never returns (`panic`).
    /// `spawn` runs the call as a concurrent task inside the current scope.
    Call { callee: Callee, args: Vec<Operand>, dest: Place, next: Option<BlockId>, spawn: bool },
    Return,
    /// Proven unreachable (e.g. after an exhaustive `match`).
    Unreachable,
}

impl Terminator {
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Goto(b) => vec![*b],
            Terminator::Switch { cases, otherwise, .. } => cases.iter().map(|(_, b)| *b).chain([*otherwise]).collect(),
            Terminator::Call { next, .. } => next.iter().copied().collect(),
            Terminator::Return | Terminator::Unreachable => Vec::new(),
        }
    }
}
