//! Ownership analysis on the typed IR.
//!
//! Phase 6A (this crate today): move/initialization checking — use after
//! move, possibly moved values at joins, uninitialized values, partial moves,
//! reinitialization — plus the information drop elaboration needs.
//! Phase 6B (borrow checking) builds on the same move paths.

mod moves;
mod paths;

pub use paths::{Lookup, MovePathId, MovePaths, PathElem};

use std::collections::HashMap;
use tarn_diagnostics::Diagnostic;
use tarn_ir::{BlockId, FunctionId, Program};
use tarn_resolve::Resolved;
use tarn_types::Typed;

/// What drop elaboration must do with one `Drop(place)` (ADR 0024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropDecision {
    /// Initialized on every path: drop unconditionally.
    Static,
    /// Moved or uninitialized on every path: remove the drop.
    Dead,
    /// Initialized on some paths only: needs a runtime drop flag.
    Conditional,
    /// The value exists but these fields were moved on every path: drop the
    /// remaining fields only.
    Partial(Vec<String>),
}

#[derive(Debug, Default)]
pub struct FnMoves {
    /// `(block, statement index, decision)` for every `Drop` statement.
    pub drops: Vec<(BlockId, usize, DropDecision)>,
}

#[derive(Debug, Default)]
pub struct MoveResults {
    pub functions: HashMap<FunctionId, FnMoves>,
}

impl MoveResults {
    /// Per-statement notes for `tarn_ir::print_program_annotated`.
    pub fn drop_notes(&self) -> HashMap<(FunctionId, BlockId, usize), String> {
        let mut out = HashMap::new();
        for (f, m) in &self.functions {
            for (b, i, d) in &m.drops {
                let note = match d {
                    DropDecision::Static => "drop: static".to_string(),
                    DropDecision::Dead => "drop: dead (moved)".to_string(),
                    DropDecision::Conditional => "drop: conditional (flag)".to_string(),
                    DropDecision::Partial(fs) => format!("drop: partial, skip {}", fs.join(", ")),
                };
                out.insert((*f, *b, *i), note);
            }
        }
        out
    }
}

/// Run the move/init checker on every function of the program.
pub fn check_moves(p: &Program, r: &Resolved, t: &Typed) -> (MoveResults, Vec<Diagnostic>) {
    let mut results = MoveResults::default();
    let mut diags = Vec::new();
    for f in &p.functions {
        let (m, mut d) = moves::check_function(f, r, t);
        diags.append(&mut d);
        results.functions.insert(f.id, m);
    }
    (results, diags)
}
