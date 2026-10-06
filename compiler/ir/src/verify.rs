//! Structural invariants of the IR. A failure is a compiler bug, never a
//! user error; tests run the verifier on every lowered program.

use crate::*;

pub fn verify(p: &Program) -> Vec<String> {
    let mut errs = Vec::new();
    for (i, f) in p.functions.iter().enumerate() {
        if f.id.0 as usize != i {
            errs.push(format!("{}: id {} at index {i}", f.name, f.id.0));
        }
        if f.blocks.is_empty() {
            if matches!(f.kind, FnKind::Body | FnKind::Closure { .. }) {
                errs.push(format!("{}: function with a body has no blocks", f.name));
            }
            continue;
        }
        let mut e = |m: String| errs.push(format!("{}: {m}", f.name));
        if f.locals.first().map(|l| l.kind) != Some(LocalKind::Return) {
            e("local _0 is not the return place".into());
        }
        if f.param_count as usize >= f.locals.len() {
            e("param_count exceeds locals".into());
        }
        let nl = f.locals.len() as u32;
        let nb = f.blocks.len() as u32;
        let place_ok = |pl: &Place| pl.local.0 < nl && pl.proj.iter().all(|pr| !matches!(pr, Proj::Index(l) if l.0 >= nl));
        let op_ok = |o: &Operand| match o {
            Operand::Copy(pl) | Operand::Move(pl) => place_ok(pl),
            Operand::Const(Const::Fn(id, _)) => (id.0 as usize) < p.functions.len(),
            Operand::Const(_) => true,
        };
        for (bi, b) in f.blocks.iter().enumerate() {
            for s in &b.stmts {
                let ok = match &s.kind {
                    StatementKind::Assign(pl, rv) => {
                        place_ok(pl)
                            && match rv {
                                Rvalue::Use(o) | Rvalue::Unary(_, o) | Rvalue::Cast(o, _) | Rvalue::Coerce(_, o, _) => op_ok(o),
                                Rvalue::Ref(_, pl) | Rvalue::Discriminant(pl) | Rvalue::Len(pl) => place_ok(pl),
                                Rvalue::SliceRef { base, start, end, .. } => place_ok(base) && start.iter().chain(end.iter()).all(&op_ok),
                                Rvalue::Binary(_, a, b) => op_ok(a) && op_ok(b),
                                Rvalue::Aggregate(_, os) => os.iter().all(&op_ok),
                            }
                    }
                    StatementKind::StorageLive(l) | StatementKind::StorageDead(l) => l.0 < nl,
                    StatementKind::Drop(pl) => place_ok(pl),
                };
                if !ok {
                    e(format!("bb{bi}: invalid statement {:?}", s.kind));
                }
            }
            for succ in b.term.successors() {
                if succ.0 >= nb {
                    e(format!("bb{bi}: jump to missing bb{}", succ.0));
                }
            }
            if let Terminator::Call { args, dest, callee, .. } = &b.term {
                let callee_ok = match callee {
                    Callee::Fn(id, _) => (id.0 as usize) < p.functions.len(),
                    Callee::Value(o) => op_ok(o),
                    _ => true,
                };
                if !place_ok(dest) || !args.iter().all(&op_ok) || !callee_ok {
                    e(format!("bb{bi}: invalid call"));
                }
            }
            if let Terminator::Switch { discr, .. } = &b.term
                && !op_ok(discr)
            {
                e(format!("bb{bi}: invalid switch operand"));
            }
        }
        // Every block reachable from bb0 (lowering prunes the rest).
        let mut seen = vec![false; f.blocks.len()];
        let mut stack = vec![0usize];
        while let Some(b) = stack.pop() {
            if std::mem::replace(&mut seen[b], true) {
                continue;
            }
            stack.extend(f.blocks[b].term.successors().into_iter().map(|s| s.0 as usize).filter(|&s| s < f.blocks.len()));
        }
        if let Some(b) = seen.iter().position(|s| !s) {
            e(format!("bb{b} is unreachable"));
        }
    }
    errs
}
