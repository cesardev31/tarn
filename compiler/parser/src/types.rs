//! Types: paths with generic arguments, references, slices, arrays, fn types.

use crate::Parser;
use tarn_ast::*;
use tarn_diagnostics::Diagnostic;
use tarn_lexer::TokenKind;

pub(crate) fn starts_type(k: &TokenKind) -> bool {
    matches!(k, TokenKind::Ident(_) | TokenKind::Amp | TokenKind::AmpAmp | TokenKind::Star | TokenKind::LBracket | TokenKind::Fn | TokenKind::Mut)
}

impl Parser {
    pub(crate) fn parse_type(&mut self) -> Type {
        let start = self.span();
        let mode = if self.at(&TokenKind::Mut) && self.nth(1) == &TokenKind::Fn {
            self.bump();
            CallMode::Mutable
        } else if matches!(self.peek(), TokenKind::Ident(n) if n == "once") && self.nth(1) == &TokenKind::Fn {
            self.bump();
            CallMode::Once
        } else {
            CallMode::Shared
        };
        let kind = match self.peek().clone() {
            TokenKind::Amp => {
                self.bump();
                self.ref_type()
            }
            TokenKind::AmpAmp => {
                // `&&T` is `&(&T)`.
                self.split_token(TokenKind::Amp);
                let inner_start = self.span();
                self.bump();
                let kind = self.ref_type();
                let inner = Type { id: self.id(), span: self.since(inner_start), kind };
                TypeKind::Ref { mutable: false, inner: Box::new(inner) }
            }
            TokenKind::Star => {
                self.bump();
                let mutable = self.eat(&TokenKind::Mut);
                TypeKind::Ptr { mutable, inner: Box::new(self.parse_type()) }
            }
            TokenKind::LBracket => {
                self.bump();
                if self.eat(&TokenKind::RBracket) {
                    TypeKind::Slice(Box::new(self.parse_type()))
                } else {
                    let len = self.with_mode(true, |p| {
                        let e = p.parse_expr();
                        p.expect(&TokenKind::RBracket);
                        e
                    });
                    TypeKind::Array { len: Box::new(len), elem: Box::new(self.parse_type()) }
                }
            }
            TokenKind::Fn => {
                self.bump();
                let mut params = Vec::new();
                if self.expect(&TokenKind::LParen) {
                    self.with_mode(true, |p| {
                        while !p.at(&TokenKind::RParen) && !p.at(&TokenKind::Eof) {
                            params.push(p.parse_type());
                            if !p.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        p.expect(&TokenKind::RParen);
                    });
                }
                let ret = starts_type(self.peek()).then(|| Box::new(self.parse_type()));
                TypeKind::Fn { mode, params, ret }
            }
            TokenKind::Ident(name) if name == "any" && matches!(self.nth(1), TokenKind::Ident(_)) => {
                self.bump();
                TypeKind::Any(self.parse_path(true))
            }
            TokenKind::Ident(_) => TypeKind::Path(self.parse_path(true)),
            k => {
                let found = crate::describe(&k);
                let span = self.span();
                self.error(
                    Diagnostic::error("E1003", "expected_type", format!("expected type, found {found}"))
                        .primary(span, "expected a type such as `i32`, `&T` or `[]u8`"),
                );
                TypeKind::Error
            }
        };
        Type { id: self.id(), span: self.since(start), kind }
    }

    /// After `&`: `mut T` or `T`.
    fn ref_type(&mut self) -> TypeKind {
        let mutable = self.eat(&TokenKind::Mut);
        TypeKind::Ref { mutable, inner: Box::new(self.parse_type()) }
    }

    /// `a.b.C` with optional `<T, U>` when `generic_args` is set.
    pub(crate) fn parse_path(&mut self, generic_args: bool) -> Path {
        let start = self.span();
        let mut segments = vec![self.expect_ident("type name")];
        while self.at(&TokenKind::Dot) && matches!(self.nth(1), TokenKind::Ident(_)) {
            self.bump();
            segments.push(self.expect_ident("name"));
        }
        let mut args = Vec::new();
        if generic_args && self.at(&TokenKind::Lt) {
            self.bump();
            self.with_mode(true, |p| {
                while !p.at(&TokenKind::Gt) && !p.at(&TokenKind::Shr) && !p.at(&TokenKind::Eof) {
                    args.push(p.parse_type());
                    if !p.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                p.expect_gt();
            });
        }
        Path { id: self.id(), span: self.since(start), segments, args }
    }
}
