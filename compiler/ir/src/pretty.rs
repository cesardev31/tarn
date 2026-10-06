//! Textual form of the IR, for `tarn ir` and snapshot tests.
//!
//! ```text
//! fn add(_1: i32, _2: i32) -> i32 {
//!     _0: i32            // return
//!     _3: i32            // temp
//!   bb0:
//!     _3 = add(copy _1, copy _2)
//!     _0 = move _3
//!     return
//! }
//! ```

use crate::*;
use std::fmt::Write;
use tarn_resolve::Resolved;
use tarn_types::Typed;

pub fn print_program(p: &Program, r: &Resolved, t: &Typed) -> String {
    let mut out = String::new();
    for f in &p.functions {
        // `core` declarations are not printed (they have no body).
        if f.blocks.is_empty() {
            continue;
        }
        out.push_str(&print_function(f, p, r, t));
        out.push('\n');
    }
    out
}

struct P<'a> {
    p: &'a Program,
    r: &'a Resolved,
    t: &'a Typed,
}

impl P<'_> {
    fn ty(&self, ty: &Ty) -> String {
        self.t.display(ty, self.r)
    }

    fn place(&self, pl: &Place) -> String {
        let mut s = format!("_{}", pl.local.0);
        for pr in &pl.proj {
            s = match pr {
                Proj::Deref => format!("(*{s})"),
                Proj::Field(i) => format!("{s}.{i}"),
                Proj::Index(l) => format!("{s}[_{}]", l.0),
                Proj::Downcast(v) => format!("({s} as #{v})"),
            };
        }
        s
    }

    fn operand(&self, o: &Operand) -> String {
        match o {
            Operand::Copy(p) => format!("copy {}", self.place(p)),
            Operand::Move(p) => format!("move {}", self.place(p)),
            Operand::Const(c) => self.konst(c),
        }
    }

    fn konst(&self, c: &Const) -> String {
        match c {
            Const::Int(v, it) => format!("{v}_{}", it.name()),
            Const::Float(s, _) => s.clone(),
            Const::Bool(b) => b.to_string(),
            Const::Str(s) => format!("{s:?}"),
            Const::Unit => "()".into(),
            Const::Fn(id, targs) => format!("fn {}{}", self.p.function(*id).name, self.targs(targs)),
            Const::Opaque => "<std>".into(),
        }
    }

    fn targs(&self, ts: &[Ty]) -> String {
        if ts.is_empty() {
            String::new()
        } else {
            format!("<{}>", ts.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", "))
        }
    }

    fn ops(&self, os: &[Operand]) -> String {
        os.iter().map(|o| self.operand(o)).collect::<Vec<_>>().join(", ")
    }

    fn rvalue(&self, rv: &Rvalue) -> String {
        match rv {
            Rvalue::Use(o) => self.operand(o),
            Rvalue::Ref(m, p) => format!("&{}{}", if *m { "mut " } else { "" }, self.place(p)),
            Rvalue::SliceRef { mutable, base, start, end } => format!(
                "&{}{}[{}..{}]",
                if *mutable { "mut " } else { "" },
                self.place(base),
                start.as_ref().map(|o| self.operand(o)).unwrap_or_default(),
                end.as_ref().map(|o| self.operand(o)).unwrap_or_default()
            ),
            Rvalue::Binary(op, a, b) => format!("{}({}, {})", crate::lower::binop_name(*op), self.operand(a), self.operand(b)),
            Rvalue::Unary(UnOp::Neg, o) => format!("neg({})", self.operand(o)),
            Rvalue::Unary(UnOp::Not, o) => format!("not({})", self.operand(o)),
            Rvalue::Aggregate(a, os) => match a {
                Aggregate::Struct(s, targs) => format!("{}{}{{{}}}", self.r.symbol(*s).name, self.targs(targs), self.ops(os)),
                Aggregate::Variant(e, v, targs) => {
                    let name = self.t.decls.enums.get(e).and_then(|d| d.variants.get(*v as usize)).map(|x| x.name.clone()).unwrap_or_default();
                    format!("{}{}.{name}({})", self.r.symbol(*e).name, self.targs(targs), self.ops(os))
                }
                Aggregate::Array(_) => format!("[{}]", self.ops(os)),
                Aggregate::Closure(id) => format!("closure {}[{}]", self.p.function(*id).name, self.ops(os)),
            },
            Rvalue::Cast(o, t) => format!("cast({}) as {}", self.operand(o), self.ty(t)),
            Rvalue::Coerce(k, o, t) => {
                let k = match k {
                    CoerceKind::MutToShared => "mut_to_shared",
                    CoerceKind::Unsize => "unsize",
                    CoerceKind::ToDyn(_) => "to_dyn",
                };
                format!("coerce.{k}({}) as {}", self.operand(o), self.ty(t))
            }
            Rvalue::Discriminant(p) => format!("discriminant({})", self.place(p)),
            Rvalue::Len(p) => format!("len({})", self.place(p)),
        }
    }

    fn callee(&self, c: &Callee) -> String {
        match c {
            Callee::Fn(id, targs) => format!("{}{}", self.p.function(*id).name, self.targs(targs)),
            Callee::Virtual { method, type_args } => format!("virtual {}{}", self.r.symbol(*method).name, self.targs(type_args)),
            Callee::Intrinsic(n) => format!("intrinsic {n}"),
            Callee::Builtin(b) => format!("builtin {b:?}").to_lowercase(),
            Callee::Value(o) => format!("({})", self.operand(o)),
            Callee::Opaque(n) => format!("opaque {n}"),
        }
    }
}

pub fn print_function(f: &Function, p: &Program, r: &Resolved, t: &Typed) -> String {
    let pp = P { p, r, t };
    let mut out = String::new();
    let params: Vec<String> = f.params().map(|l| format!("_{}: {}", l.0, pp.ty(&f.local(l).ty))).collect();
    let _ = writeln!(out, "fn {}({}) -> {} {{", f.name, params.join(", "), pp.ty(&f.ret));
    for (i, l) in f.locals.iter().enumerate() {
        let what = match l.kind {
            LocalKind::Return => "return".to_string(),
            LocalKind::Param => format!("param {}", l.name.clone().unwrap_or_default()),
            LocalKind::User => format!("{}{}", if l.mutable { "var " } else { "let " }, l.name.clone().unwrap_or_default()),
            LocalKind::Temp => "temp".to_string(),
        };
        let _ = writeln!(out, "    _{i}: {}    // {what}", pp.ty(&l.ty));
    }
    for (i, b) in f.blocks.iter().enumerate() {
        let _ = writeln!(out, "  bb{i}:");
        for s in &b.stmts {
            let line = match &s.kind {
                StatementKind::Assign(pl, rv) => format!("{} = {}", pp.place(pl), pp.rvalue(rv)),
                StatementKind::StorageLive(l) => format!("live _{}", l.0),
                StatementKind::StorageDead(l) => format!("dead _{}", l.0),
                StatementKind::Drop(pl) => format!("drop {}", pp.place(pl)),
            };
            let _ = writeln!(out, "    {line}");
        }
        let term = match &b.term {
            Terminator::Goto(t) => format!("goto bb{}", t.0),
            Terminator::Switch { discr, cases, otherwise } => {
                let cs: Vec<String> = cases.iter().map(|(v, b)| format!("{v} => bb{}", b.0)).collect();
                format!("switch {} [{}, _ => bb{}]", pp.operand(discr), cs.join(", "), otherwise.0)
            }
            Terminator::Call { callee, args, dest, next, spawn } => format!(
                "{}{} = call {}({}){}",
                if *spawn { "spawn " } else { "" },
                pp.place(dest),
                pp.callee(callee),
                pp.ops(args),
                match next {
                    Some(n) => format!(" -> bb{}", n.0),
                    None => " -> !".into(),
                }
            ),
            Terminator::Return => "return".into(),
            Terminator::Unreachable => "unreachable".into(),
        };
        let _ = writeln!(out, "    {term}");
    }
    out.push_str("}\n");
    out
}
