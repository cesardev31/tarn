//! Patterns for `match` arms.

use crate::Parser;
use tarn_ast::*;
use tarn_diagnostics::Diagnostic;
use tarn_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_pattern(&mut self) -> Pattern {
        let start = self.span();
        let kind = match self.peek().clone() {
            TokenKind::Ident(n) if n == "_" => {
                self.bump();
                PatternKind::Wildcard
            }
            TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::Str(_) | TokenKind::True | TokenKind::False | TokenKind::Minus => {
                let lit = self.literal_pattern();
                let inclusive = match self.peek() {
                    TokenKind::DotDot => Some(false),
                    TokenKind::DotDotEq => Some(true),
                    _ => None,
                };
                match inclusive {
                    Some(inclusive) => {
                        self.bump();
                        let end = self.literal_pattern();
                        PatternKind::Range { start: lit, end, inclusive }
                    }
                    None => PatternKind::Literal(lit),
                }
            }
            TokenKind::Ident(_) => {
                let path = self.parse_path(false);
                if self.eat(&TokenKind::LParen) {
                    let args = self.with_mode(true, |p| {
                        let mut args = Vec::new();
                        while !p.at(&TokenKind::RParen) && !p.at(&TokenKind::Eof) {
                            args.push(p.parse_pattern());
                            if !p.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        p.expect(&TokenKind::RParen);
                        args
                    });
                    PatternKind::Variant { path, args }
                } else if self.at(&TokenKind::LBrace) {
                    self.bump();
                    let fields = self.with_mode(true, |p| {
                        let mut fields = Vec::new();
                        while !p.at(&TokenKind::RBrace) && !p.at(&TokenKind::Eof) {
                            let fstart = p.span();
                            let name = p.expect_ident("field name");
                            let pattern = p.eat(&TokenKind::Colon).then(|| p.parse_pattern());
                            fields.push(FieldPattern { id: p.id(), span: p.since(fstart), name, pattern });
                            if !p.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        p.expect(&TokenKind::RBrace);
                        fields
                    });
                    PatternKind::Struct { path, fields }
                } else if path.segments.len() == 1 {
                    PatternKind::Ident(path.segments[0].name.clone())
                } else {
                    PatternKind::Variant { path, args: Vec::new() }
                }
            }
            k => {
                let found = crate::describe(&k);
                let span = self.span();
                self.error(
                    Diagnostic::error("E1004", "expected_pattern", format!("expected pattern, found {found}"))
                        .primary(span, "expected a pattern such as `_`, `x`, `0`, `Some(x)`"),
                );
                PatternKind::Error
            }
        };
        Pattern { id: self.id(), span: self.since(start), kind }
    }

    /// Literal, optionally negated: `1`, `-1`, `"s"`, `true`.
    fn literal_pattern(&mut self) -> Expr {
        let start = self.span();
        let neg = self.eat(&TokenKind::Minus);
        let kind = match self.peek().clone() {
            TokenKind::Int(v) => ExprKind::Int(v),
            TokenKind::Float(s) => ExprKind::Float(s),
            TokenKind::Str(s) if !neg => ExprKind::Str(s),
            TokenKind::True if !neg => ExprKind::Bool(true),
            TokenKind::False if !neg => ExprKind::Bool(false),
            k => {
                let found = crate::describe(&k);
                let span = self.span();
                self.error(
                    Diagnostic::error("E1004", "expected_pattern", format!("expected literal, found {found}"))
                        .primary(span, "expected a literal"),
                );
                return self.error_expr(self.since(start));
            }
        };
        let tok = self.bump();
        let lit = Expr { id: self.id(), span: tok.span, kind };
        if neg {
            return Expr {
                id: self.id(),
                span: self.since(start),
                kind: ExprKind::Unary { op: UnaryOp::Neg, operand: Box::new(lit) },
            };
        }
        lit
    }
}
