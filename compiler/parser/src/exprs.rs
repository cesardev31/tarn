//! Expressions: precedence climbing (Pratt) for binary operators, then
//! prefix operators, postfix chains and primaries.
//!
//! Binding power, loosest first:
//!
//! | level | operators              | assoc            |
//! |-------|------------------------|------------------|
//! | 0     | `..` `..=`             | none (E1017)     |
//! | 1     | `\|\|`                 | left             |
//! | 2     | `&&`                   | left             |
//! | 3     | `==` `!=` `<` `<=` `>` `>=` | none (E1006) |
//! | 4     | `\|`                   | left             |
//! | 5     | `^`                    | left             |
//! | 6     | `&`                    | left             |
//! | 7     | `<<` `>>`              | left             |
//! | 8     | `+` `-`                | left             |
//! | 9     | `*` `/` `%`            | left             |
//! | 10    | prefix `-` `!` `&` `&mut` `try` | —       |
//! | 11    | postfix `.f` `(..)` `[..]` `T{..}` | —    |

use crate::Parser;
use tarn_ast::*;
use tarn_diagnostics::{Diagnostic, Span};
use tarn_lexer::TokenKind;

const CMP: u8 = 3;

fn binary_op(k: &TokenKind) -> Option<(BinaryOp, u8)> {
    use BinaryOp as B;
    use TokenKind as T;
    Some(match k {
        T::PipePipe => (B::Or, 1),
        T::AmpAmp => (B::And, 2),
        T::EqEq => (B::Eq, CMP),
        T::BangEq => (B::Ne, CMP),
        T::Lt => (B::Lt, CMP),
        T::LtEq => (B::Le, CMP),
        T::Gt => (B::Gt, CMP),
        T::GtEq => (B::Ge, CMP),
        T::Pipe => (B::BitOr, 4),
        T::Caret => (B::BitXor, 5),
        T::Amp => (B::BitAnd, 6),
        T::Shl => (B::Shl, 7),
        T::Shr => (B::Shr, 7),
        T::Plus => (B::Add, 8),
        T::Minus => (B::Sub, 8),
        T::Star => (B::Mul, 9),
        T::Slash => (B::Div, 9),
        T::Percent => (B::Rem, 9),
        _ => return None,
    })
}

/// Tokens that can start an expression (used for optional range ends).
fn starts_expr(k: &TokenKind) -> bool {
    use TokenKind as T;
    matches!(
        k,
        T::Ident(_)
            | T::Int(_)
            | T::Float(_)
            | T::Str(_)
            | T::True
            | T::False
            | T::LParen
            | T::LBracket
            | T::Minus
            | T::Bang
            | T::Amp
            | T::AmpAmp
            | T::Try
            | T::Fn
    )
}

impl Parser {
    pub(crate) fn parse_expr(&mut self) -> Expr {
        let start = self.span();
        // Prefix range: `..b` / `..`
        if let Some(inclusive) = self.range_op() {
            self.bump();
            let end = starts_expr(self.peek()).then(|| Box::new(self.parse_binary(1)));
            let e = Expr { id: self.id(), span: self.since(start), kind: ExprKind::Range { start: None, end, inclusive } };
            return self.no_chained_range(e);
        }
        let lhs = self.parse_binary(1);
        if let Some(inclusive) = self.range_op() {
            self.bump();
            let end = starts_expr(self.peek()).then(|| Box::new(self.parse_binary(1)));
            let e = Expr {
                id: self.id(),
                span: self.since(start),
                kind: ExprKind::Range { start: Some(Box::new(lhs)), end, inclusive },
            };
            return self.no_chained_range(e);
        }
        lhs
    }

    fn range_op(&mut self) -> Option<bool> {
        match self.peek() {
            TokenKind::DotDot => Some(false),
            TokenKind::DotDotEq => Some(true),
            _ => None,
        }
    }

    fn no_chained_range(&mut self, e: Expr) -> Expr {
        if self.range_op().is_some() {
            let span = self.span();
            self.error(
                Diagnostic::error("E1017", "chained_range", "range operators cannot be chained")
                    .primary(span, "second range operator")
                    .secondary(e.span, "this is already a range"),
            );
            self.bump();
            if starts_expr(self.peek()) {
                self.parse_binary(1);
            }
        }
        e
    }

    /// Precedence climbing over binary operators with level >= `min`.
    fn parse_binary(&mut self, min: u8) -> Expr {
        let start = self.span();
        let mut lhs = self.parse_unary();
        let mut last_cmp: Option<Span> = None;
        loop {
            let Some((op, prec)) = binary_op(self.peek()) else { break };
            if prec < min {
                break;
            }
            let op_tok = self.bump();
            // `x += 1` lexes as `+` `=`.
            if self.tokens[self.pos].kind == TokenKind::Eq && self.tokens[self.pos].span.start == op_tok.span.end {
                let eq = self.bump();
                self.error(
                    Diagnostic::error(
                        "E1007",
                        "compound_assignment",
                        format!("compound assignment `{}=` is not supported", op.symbol()),
                    )
                    .primary(op_tok.span.to(eq.span), "")
                    .help(format!("write `x = x {} value`", op.symbol())),
                );
                self.skip_newlines();
                self.parse_binary(prec + 1);
                return self.error_expr(self.since(start));
            }
            if prec == CMP {
                if let Some(prev) = last_cmp {
                    self.error(
                        Diagnostic::error("E1006", "chained_comparison", "comparison operators cannot be chained")
                            .primary(op_tok.span, "second comparison")
                            .secondary(prev, "first comparison")
                            .help("split it: `a < b && b < c`"),
                    );
                }
                last_cmp = Some(op_tok.span);
            }
            // An operator at the end of a line continues the expression.
            self.skip_newlines();
            let rhs = self.parse_binary(prec + 1);
            lhs = Expr {
                id: self.id(),
                span: self.since(start),
                kind: ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
            };
        }
        lhs
    }

    fn parse_unary(&mut self) -> Expr {
        let start = self.span();
        let kind = match self.peek().clone() {
            TokenKind::Minus => self.prefix(UnaryOp::Neg),
            TokenKind::Bang => self.prefix(UnaryOp::Not),
            TokenKind::Amp => {
                self.bump();
                let op = if self.eat(&TokenKind::Mut) { UnaryOp::RefMut } else { UnaryOp::Ref };
                self.skip_newlines();
                ExprKind::Unary { op, operand: Box::new(self.parse_unary()) }
            }
            TokenKind::AmpAmp => {
                // `&&x` is `&(&x)`.
                self.split_token(TokenKind::Amp);
                ExprKind::Unary { op: UnaryOp::Ref, operand: Box::new(self.parse_unary()) }
            }
            TokenKind::Try => {
                self.bump();
                self.skip_newlines();
                ExprKind::Try(Box::new(self.parse_unary()))
            }
            _ => {
                let primary = self.parse_primary();
                return self.parse_postfix(primary);
            }
        };
        Expr { id: self.id(), span: self.since(start), kind }
    }

    fn prefix(&mut self, op: UnaryOp) -> ExprKind {
        self.bump();
        self.skip_newlines();
        ExprKind::Unary { op, operand: Box::new(self.parse_unary()) }
    }

    fn parse_postfix(&mut self, mut e: Expr) -> Expr {
        let start = e.span;
        loop {
            match self.peek().clone() {
                // Method chain continued on the next line: `\n    .arg(x)`.
                TokenKind::Newline if *self.peek_past_newlines() == TokenKind::Dot => {
                    self.skip_newlines();
                }
                TokenKind::Dot => {
                    self.bump();
                    self.skip_newlines();
                    let name = self.expect_ident("field or method name");
                    e = Expr { id: self.id(), span: self.since(start), kind: ExprKind::Field { base: Box::new(e), name } };
                }
                TokenKind::LParen => {
                    self.bump();
                    let args = self.with_mode(true, |p| p.expr_list(&TokenKind::RParen));
                    e = Expr { id: self.id(), span: self.since(start), kind: ExprKind::Call { callee: Box::new(e), args } };
                }
                TokenKind::LBracket => {
                    self.bump();
                    let index = self.with_mode(true, |p| {
                        let i = p.parse_expr();
                        p.expect(&TokenKind::RBracket);
                        i
                    });
                    e = Expr {
                        id: self.id(),
                        span: self.since(start),
                        kind: ExprKind::Index { base: Box::new(e), index: Box::new(index) },
                    };
                }
                TokenKind::LBrace if !self.no_struct => {
                    let id = self.id();
                    let Some(path) = crate::expr_to_path(&e, id) else { break };
                    e = self.parse_struct_lit(path, start);
                }
                _ => break,
            }
        }
        e
    }

    /// Comma-separated expressions up to `close` (consumed). Trailing comma ok.
    fn expr_list(&mut self, close: &TokenKind) -> Vec<Expr> {
        let mut out = Vec::new();
        while !self.at(close) && !self.at(&TokenKind::Eof) {
            let before = self.pos;
            let e = self.parse_expr();
            let failed = matches!(e.kind, ExprKind::Error);
            out.push(e);
            if self.eat(&TokenKind::Comma) {
                continue;
            }
            if !self.at(close) {
                if !failed {
                    self.expect(&TokenKind::Comma);
                }
                self.skip_list_item(close);
                if self.eat(&TokenKind::Comma) {
                    continue;
                }
            }
            if self.pos == before {
                break;
            }
        }
        self.expect(close);
        out
    }

    /// Skip a malformed list element: up to `,` or `close` at depth 0, or a
    /// `}` that likely closes an enclosing block.
    fn skip_list_item(&mut self, close: &TokenKind) {
        let mut depth = 0usize;
        loop {
            let k = self.peek().clone();
            match k {
                TokenKind::Eof => return,
                TokenKind::Comma if depth == 0 => return,
                ref k if depth == 0 && crate::same(k, close) => return,
                TokenKind::RBrace if depth == 0 => return,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }
    }

    /// `Path{field: value, shorthand}` — called with the `{` current.
    pub(crate) fn parse_struct_lit(&mut self, path: Path, start: Span) -> Expr {
        self.bump();
        let fields = self.with_mode(true, |p| {
            let mut fields = Vec::new();
            while !p.at(&TokenKind::RBrace) && !p.at(&TokenKind::Eof) {
                let fstart = p.span();
                let before = p.pos;
                let name = p.expect_ident("field name");
                let value = if p.eat(&TokenKind::Colon) { Some(p.parse_expr()) } else { None };
                fields.push(FieldInit { id: p.id(), span: p.since(fstart), name, value });
                if !p.eat(&TokenKind::Comma) {
                    if !p.at(&TokenKind::RBrace) {
                        p.expect(&TokenKind::Comma);
                        p.skip_list_item(&TokenKind::RBrace);
                        p.eat(&TokenKind::Comma);
                    }
                    if p.pos == before {
                        break;
                    }
                }
            }
            p.expect(&TokenKind::RBrace);
            fields
        });
        Expr { id: self.id(), span: self.since(start), kind: ExprKind::StructLit { path, fields } }
    }

    fn parse_primary(&mut self) -> Expr {
        let start = self.span();
        let kind = match self.peek().clone() {
            TokenKind::Int(v) => {
                self.bump();
                ExprKind::Int(v)
            }
            TokenKind::Float(s) => {
                self.bump();
                ExprKind::Float(s)
            }
            TokenKind::Str(s) => {
                self.bump();
                ExprKind::Str(s)
            }
            TokenKind::True => {
                self.bump();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.bump();
                ExprKind::Bool(false)
            }
            TokenKind::Ident(n) if n == "move" && self.nth(1) == &TokenKind::Fn => {
                self.bump();
                self.parse_closure(true)
            }
            TokenKind::Ident(n) => {
                self.bump();
                ExprKind::Ident(n)
            }
            TokenKind::LParen => {
                self.bump();
                self.with_mode(true, |p| {
                    if p.eat(&TokenKind::RParen) {
                        return ExprKind::Unit;
                    }
                    let inner = p.parse_expr();
                    p.expect(&TokenKind::RParen);
                    ExprKind::Paren(Box::new(inner))
                })
            }
            TokenKind::LBracket => {
                // `[N]T{a, b}` / `[]T{a, b}`
                let ty = self.parse_type();
                if !self.at(&TokenKind::LBrace) {
                    self.expect(&TokenKind::LBrace);
                    return self.error_expr(self.since(start));
                }
                self.bump();
                let elems = self.with_mode(true, |p| p.expr_list(&TokenKind::RBrace));
                ExprKind::ArrayLit { ty, elems }
            }
            TokenKind::Fn => self.parse_closure(false),
            k => {
                let found = crate::describe(&k);
                let span = self.span();
                let mut d = Diagnostic::error("E1002", "expected_expression", format!("expected expression, found {found}"))
                    .primary(span, "expected an expression");
                if binary_op(&k).is_some() && self.tokens[self.pos.saturating_sub(1)].kind == TokenKind::Newline {
                    d = d.help("a line cannot start with a binary operator; put the operator at the end of the previous line");
                }
                self.error(d);
                return self.error_expr(Span::new(span.file, span.start, span.start));
            }
        };
        Expr { id: self.id(), span: self.since(start), kind }
    }

    /// `fn(a, b i32) R { ... }`
    fn parse_closure(&mut self, owned: bool) -> ExprKind {
        self.bump();
        let mut params = Vec::new();
        if self.expect(&TokenKind::LParen) {
            self.with_mode(true, |p| {
                while !p.at(&TokenKind::RParen) && !p.at(&TokenKind::Eof) {
                    let id = p.id();
                    let name = p.expect_ident("parameter name");
                    let ty = (!p.at(&TokenKind::Comma) && !p.at(&TokenKind::RParen)).then(|| p.parse_type());
                    params.push(ClosureParam { id, name, ty });
                    if !p.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                p.expect(&TokenKind::RParen);
            });
        }
        let ret = crate::types::starts_type(self.peek()).then(|| self.parse_type());
        let body = self.parse_block();
        ExprKind::Closure { owned, params, ret, body }
    }
}
