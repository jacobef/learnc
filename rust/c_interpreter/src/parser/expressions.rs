//! Expression grammar and binary operator precedence.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_comma()
    }

    fn parse_comma(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_assignment()?;
        while self.eat(TokenKind::Comma) {
            let rhs = self.parse_assignment()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::Comma,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    pub(super) fn parse_assignment(&mut self) -> Result<Expr, Diagnostic> {
        let lhs = self.parse_conditional()?;
        if self.eat(TokenKind::Equal) {
            let rhs = self.parse_assignment()?;
            let span = lhs.span().merge(rhs.span());
            return Ok(Expr::Assign {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            });
        }
        let op = match self.peek_kind(0) {
            Some(TokenKind::PlusEqual) => BinaryOp::Add,
            Some(TokenKind::MinusEqual) => BinaryOp::Sub,
            Some(TokenKind::StarEqual) => BinaryOp::Mul,
            Some(TokenKind::SlashEqual) => BinaryOp::Div,
            Some(TokenKind::PercentEqual) => BinaryOp::Rem,
            Some(TokenKind::LeftShiftEqual) => BinaryOp::ShiftLeft,
            Some(TokenKind::RightShiftEqual) => BinaryOp::ShiftRight,
            Some(TokenKind::AmpEqual) => BinaryOp::BitAnd,
            Some(TokenKind::CaretEqual) => BinaryOp::BitXor,
            Some(TokenKind::PipeEqual) => BinaryOp::BitOr,
            _ => return Ok(lhs),
        };
        self.bump();
        let rhs = self.parse_assignment()?;
        let span = lhs.span().merge(rhs.span());
        Ok(Expr::CompoundAssign {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        })
    }

    pub(super) fn parse_conditional(&mut self) -> Result<Expr, Diagnostic> {
        let condition = self.parse_binary(1)?;
        if self.eat(TokenKind::Question) {
            let then_expr = self.parse_expression()?;
            self.expect(TokenKind::Colon)?;
            let else_expr = self.parse_conditional()?;
            let span = condition.span().merge(else_expr.span());
            Ok(Expr::Conditional {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            })
        } else {
            Ok(condition)
        }
    }

    // Binary operators are left associative; recursive RHS parsing handles only
    // tighter operators. Assignment and ?: retain their separate C grammar.
    fn parse_binary(&mut self, minimum_precedence: u8) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_unary()?;
        while let Some((op, precedence)) = self.peek_kind(0).and_then(binary_operator) {
            if precedence < minimum_precedence {
                break;
            }
            self.bump();
            let rhs = self.parse_binary(precedence + 1)?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.at_keyword(Keyword::Alignof) {
            let start = self.bump().span;
            self.expect(TokenKind::LParen)?;
            let parsed = self.parse_type_name()?;
            let end = self.expect(TokenKind::RParen)?.span;
            let alignment = self.type_align_of(&parsed.ty).map_err(|_| {
                Diagnostic::error("_Alignof requires a complete object type", start.merge(end))
            })?;
            let span = start.merge(end);
            return parse_number_literal(format!("{alignment}UL"), span)
                .map(|literal| Expr::Number(literal, span));
        }
        if self.at_keyword(Keyword::Sizeof) {
            let start = self.bump().span;
            if self.at(TokenKind::LParen) && self.is_type_name_start() {
                self.bump();
                let ty = self.parse_type_name()?;
                let end = self.expect(TokenKind::RParen)?.span;
                if self.at(TokenKind::LBrace) {
                    let initializer = self.parse_initializer()?;
                    let literal_span = end.merge(initializer.span());
                    let expr = self.parse_postfix_suffix(Expr::CompoundLiteral {
                        ty: ty.ty,
                        vla_bounds: ty.vla_bounds,
                        initializer: Box::new(initializer),
                        span: literal_span,
                    })?;
                    let span = start.merge(expr.span());
                    return Ok(Expr::SizeofExpr {
                        expr: Box::new(expr),
                        span,
                    });
                }
                return Ok(Expr::SizeofType {
                    ty: ty.ty,
                    vla_bounds: ty.vla_bounds,
                    span: start.merge(end),
                });
            }
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::SizeofExpr {
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::LParen) && self.is_type_name_start() {
            let start = self.bump().span;
            let ty = self.parse_type_name()?;
            let end = self.expect(TokenKind::RParen)?.span;
            if self.at(TokenKind::LBrace) {
                let initializer = self.parse_initializer()?;
                let span = start.merge(initializer.span());
                let expr = Expr::CompoundLiteral {
                    ty: ty.ty,
                    vla_bounds: ty.vla_bounds,
                    initializer: Box::new(initializer),
                    span,
                };
                return self.parse_postfix_suffix(expr);
            }
            let expr = self.parse_unary()?;
            let span = start.merge(end).merge(expr.span());
            return Ok(Expr::Cast {
                ty: ty.ty,
                vla_bounds: ty.vla_bounds,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::DoublePlus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::PreIncrement,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::DoubleMinus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::PreDecrement,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Amp) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::AddressOf,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Star) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Dereference,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Plus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Minus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Bang) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::LogicalNot,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Tilde) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_primary()?;
        self.parse_postfix_suffix(expr)
    }

    fn parse_postfix_suffix(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self.eat(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.at(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_assignment()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end = self.expect(TokenKind::RParen)?.span;
                let span = expr.span().merge(end);
                let declared_callee_type = match &expr {
                    Expr::Variable(name, _) => self
                        .visible_scope_entry(name)
                        .and_then(|entry| entry.ordinary_ty.clone()),
                    _ => None,
                };
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    declared_callee_type,
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::LBracket) {
                let index = self.parse_expression()?;
                let end = self.expect(TokenKind::RBracket)?.span;
                let span = expr.span().merge(end);
                expr = Expr::Subscript {
                    base: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::DoublePlus) {
                let span = expr.span().merge(self.prev_span());
                expr = Expr::Postfix {
                    op: PostfixOp::PostIncrement,
                    expr: Box::new(expr),
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::DoubleMinus) {
                let span = expr.span().merge(self.prev_span());
                expr = Expr::Postfix {
                    op: PostfixOp::PostDecrement,
                    expr: Box::new(expr),
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::Dot) {
                let member = match self.bump().clone().kind {
                    TokenKind::Identifier(name) => name,
                    _ => {
                        return Err(Diagnostic::error(
                            "expected member name after .",
                            self.prev_span(),
                        ));
                    }
                };
                let span = expr.span().merge(self.prev_span());
                expr = Expr::Member {
                    base: Box::new(expr),
                    member,
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::Arrow) {
                let member_token = self.bump().clone();
                let member = match member_token.kind {
                    TokenKind::Identifier(name) => name,
                    _ => {
                        return Err(Diagnostic::error(
                            "expected member name after ->",
                            member_token.span,
                        ));
                    }
                };
                let expr_span = expr.span();
                let deref = Expr::Unary {
                    op: UnaryOp::Dereference,
                    expr: Box::new(expr),
                    span: expr_span.merge(member_token.span),
                };
                let deref_span = deref.span();
                expr = Expr::Member {
                    base: Box::new(deref),
                    member,
                    span: deref_span.merge(member_token.span),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Number(text) => parse_number_literal(text, token.span)
                .map(|literal| Expr::Number(literal, token.span)),
            TokenKind::CharLiteral(value) => Ok(Expr::CharLiteral(value, token.span)),
            TokenKind::WideCharLiteral(value) => Ok(Expr::WideCharLiteral(value, token.span)),
            TokenKind::Utf16CharLiteral(value) => Ok(Expr::Utf16CharLiteral(value, token.span)),
            TokenKind::Utf32CharLiteral(value) => Ok(Expr::Utf32CharLiteral(value, token.span)),
            TokenKind::StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Narrow, token.span)
            }
            TokenKind::Utf8StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Utf8, token.span)
            }
            TokenKind::WideStringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Wide, token.span)
            }
            TokenKind::Utf16StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Utf16, token.span)
            }
            TokenKind::Utf32StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Utf32, token.span)
            }
            TokenKind::Keyword(Keyword::Generic) => self.parse_generic_selection(token.span),
            TokenKind::Identifier(name) => {
                if name == "va_arg" && self.at(TokenKind::LParen) {
                    self.expect(TokenKind::LParen)?;
                    let ap = self.parse_assignment()?;
                    self.expect(TokenKind::Comma)?;
                    let ty = self.parse_type_name()?;
                    if ty.vla_bounds.iter().any(|bound| bound.is_some()) {
                        return Err(Diagnostic::error(
                            "va_arg does not support variable length array type names",
                            token.span,
                        ));
                    }
                    let end = self.expect(TokenKind::RParen)?.span;
                    return Ok(Expr::VaArg {
                        ap: Box::new(ap),
                        ty: ty.ty,
                        span: token.span.merge(end),
                    });
                }
                if name == "__builtin_offsetof" && self.at(TokenKind::LParen) {
                    self.expect(TokenKind::LParen)?;
                    let ty = self.parse_type_name()?;
                    if ty.vla_bounds.iter().any(|bound| bound.is_some()) {
                        return Err(Diagnostic::error(
                            "__builtin_offsetof does not support variable length array type names",
                            token.span,
                        ));
                    }
                    if !matches!(ty.ty.unqualified(), CType::Struct(..) | CType::Union(..)) {
                        return Err(Diagnostic::error(
                            "__builtin_offsetof requires a struct or union type",
                            ty.span,
                        ));
                    }
                    self.expect(TokenKind::Comma)?;
                    let designators = self.parse_offsetof_designators()?;
                    let end = self.expect(TokenKind::RParen)?.span;
                    return Ok(Expr::OffsetOf {
                        ty: ty.ty,
                        designators,
                        span: token.span.merge(end),
                    });
                }
                if let Some(value) = self.lookup_enum_constant_value(&name) {
                    return Ok(Self::enum_constant_expr(value, token.span));
                }
                if !self.allow_undeclared_identifiers
                    && !self.identifier_is_visible(&name)
                    && !self.is_builtin_identifier(&name)
                {
                    return Err(Diagnostic::error(
                        format!("use of undeclared identifier {}", name),
                        token.span,
                    ));
                }
                Ok(Expr::Variable(name, token.span))
            }
            TokenKind::LParen => {
                let mut expr = self.parse_expression()?;
                let end = self.expect(TokenKind::RParen)?.span;
                expr.set_span(token.span.merge(end));
                Ok(expr)
            }
            _ => Err(Diagnostic::error("expected expression", token.span)),
        }
    }

    fn enum_constant_expr(value: i128, span: Span) -> Expr {
        if value >= 0 {
            return Self::generated_number_expr(value.to_string(), span);
        }
        let magnitude_before_last = value
            .checked_add(1)
            .and_then(i128::checked_neg)
            .expect("enumerator values are constrained to the range of int");
        Expr::Binary {
            op: BinaryOp::Sub,
            lhs: Box::new(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(Self::generated_number_expr(
                    magnitude_before_last.to_string(),
                    span,
                )),
                span,
            }),
            rhs: Box::new(Self::generated_number_expr("1".to_owned(), span)),
            span,
        }
    }

    fn generated_number_expr(text: String, span: Span) -> Expr {
        Expr::Number(
            parse_number_literal(text, span).expect("generated number literal must be valid"),
            span,
        )
    }

    fn parse_string_literal_primary(
        &mut self,
        text: StringLiteralValue,
        mut encoding: StringEncoding,
        span: Span,
    ) -> Result<Expr, Diagnostic> {
        let mut combined = text;
        let mut combined_span = span;
        loop {
            let (text, next_encoding) = match self.peek_kind(0).cloned() {
                Some(TokenKind::StringLiteral(text)) => (text, StringEncoding::Narrow),
                Some(TokenKind::Utf8StringLiteral(text)) => (text, StringEncoding::Utf8),
                Some(TokenKind::WideStringLiteral(text)) => (text, StringEncoding::Wide),
                Some(TokenKind::Utf16StringLiteral(text)) => (text, StringEncoding::Utf16),
                Some(TokenKind::Utf32StringLiteral(text)) => (text, StringEncoding::Utf32),
                _ => break,
            };
            let encoding_is_wide =
                !matches!(encoding, StringEncoding::Narrow | StringEncoding::Utf8);
            let next_is_wide =
                !matches!(next_encoding, StringEncoding::Narrow | StringEncoding::Utf8);
            if (encoding == StringEncoding::Utf8 && next_is_wide)
                || (next_encoding == StringEncoding::Utf8 && encoding_is_wide)
            {
                return Err(Diagnostic::error(
                    "adjacent string literals cannot mix wide and UTF-8 prefixes",
                    combined_span.merge(self.tokens[self.index].span),
                ));
            }
            if encoding != StringEncoding::Narrow
                && encoding != StringEncoding::Utf8
                && next_encoding != StringEncoding::Narrow
                && next_encoding != StringEncoding::Utf8
                && encoding != next_encoding
            {
                return Err(Diagnostic::error(
                    "adjacent string literals have incompatible encoding prefixes",
                    combined_span.merge(self.tokens[self.index].span),
                ));
            }
            if encoding == StringEncoding::Narrow
                || (encoding == StringEncoding::Utf8 && next_encoding != StringEncoding::Narrow)
            {
                encoding = next_encoding;
            }
            {
                let next_span = self.tokens[self.index].span;
                self.bump();
                combined.append(text);
                combined_span = combined_span.merge(next_span);
            }
        }
        Ok(match encoding {
            StringEncoding::Narrow => Expr::StringLiteral(combined, combined_span),
            StringEncoding::Utf8 => Expr::StringLiteral(combined, combined_span),
            StringEncoding::Wide => Expr::WideStringLiteral(combined, combined_span),
            StringEncoding::Utf16 => Expr::Utf16StringLiteral(combined, combined_span),
            StringEncoding::Utf32 => Expr::Utf32StringLiteral(combined, combined_span),
        })
    }

    fn parse_generic_selection(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::LParen)?;
        let control = self.parse_assignment()?;
        self.expect(TokenKind::Comma)?;
        let mut associations = Vec::new();
        let mut default_expr = None;
        loop {
            if self.at_keyword(Keyword::Default) {
                let default_span = self.bump().span;
                if default_expr.is_some() {
                    return Err(Diagnostic::error(
                        "_Generic may specify default at most once",
                        default_span,
                    ));
                }
                self.expect(TokenKind::Colon)?;
                default_expr = Some(Box::new(self.parse_assignment()?));
            } else {
                let ty = self.parse_type_name()?;
                if ty.vla_bounds.iter().any(|bound| bound.is_some()) {
                    return Err(Diagnostic::error(
                        "_Generic does not support variably modified association types",
                        start,
                    ));
                }
                if !self.type_is_complete(&ty.ty) {
                    return Err(Diagnostic::error(
                        "_Generic association type must be a complete object type",
                        ty.span,
                    ));
                }
                if associations.iter().any(|association: &GenericAssociation| {
                    generic_types_compatible(&association.ty, &ty.ty)
                }) {
                    return Err(Diagnostic::error(
                        "_Generic associations may not specify compatible types more than once",
                        ty.span,
                    ));
                }
                self.expect(TokenKind::Colon)?;
                let expr = self.parse_assignment()?;
                associations.push(GenericAssociation {
                    ty: ty.ty,
                    span: expr.span(),
                    expr,
                });
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(TokenKind::RParen)?.span;
        if associations.is_empty() && default_expr.is_none() {
            return Err(Diagnostic::error(
                "_Generic requires at least one association",
                start.merge(end),
            ));
        }
        Ok(Expr::GenericSelection {
            control: Box::new(control),
            associations,
            default: default_expr,
            span: start.merge(end),
        })
    }

    fn parse_offsetof_designators(&mut self) -> Result<Vec<Designator>, Diagnostic> {
        let mut designators = Vec::new();
        let first = self.bump().clone();
        let TokenKind::Identifier(name) = first.kind else {
            return Err(Diagnostic::error(
                "offsetof requires a member designator after the type name",
                first.span,
            ));
        };
        designators.push(Designator::Member(name, first.span));
        loop {
            if self.eat(TokenKind::Dot) {
                let token = self.bump().clone();
                let TokenKind::Identifier(name) = token.kind else {
                    return Err(Diagnostic::error(
                        "expected member name after . in offsetof designator",
                        token.span,
                    ));
                };
                designators.push(Designator::Member(name, token.span));
                continue;
            }
            if self.eat(TokenKind::LBracket) {
                let expr = self.parse_assignment()?;
                let value = self.eval_integer_constant_expr(&expr)?;
                if value < 0 {
                    return Err(Diagnostic::error(
                        "offsetof array designator index must be non-negative",
                        expr.span(),
                    ));
                }
                let end = self.expect(TokenKind::RBracket)?.span;
                designators.push(Designator::Index(
                    usize::try_from(value).map_err(|_| {
                        Diagnostic::error(
                            "offsetof array designator index is out of range",
                            expr.span(),
                        )
                    })?,
                    expr.span().merge(end),
                ));
                continue;
            }
            break;
        }
        Ok(designators)
    }
}

fn binary_operator(kind: &TokenKind) -> Option<(BinaryOp, u8)> {
    Some(match kind {
        TokenKind::DoublePipe => (BinaryOp::LogicalOr, 1),
        TokenKind::DoubleAmp => (BinaryOp::LogicalAnd, 2),
        TokenKind::Pipe => (BinaryOp::BitOr, 3),
        TokenKind::Caret => (BinaryOp::BitXor, 4),
        TokenKind::Amp => (BinaryOp::BitAnd, 5),
        TokenKind::DoubleEqual => (BinaryOp::Equal, 6),
        TokenKind::BangEqual => (BinaryOp::NotEqual, 6),
        TokenKind::Less => (BinaryOp::Less, 7),
        TokenKind::LessEqual => (BinaryOp::LessEqual, 7),
        TokenKind::Greater => (BinaryOp::Greater, 7),
        TokenKind::GreaterEqual => (BinaryOp::GreaterEqual, 7),
        TokenKind::LeftShift => (BinaryOp::ShiftLeft, 8),
        TokenKind::RightShift => (BinaryOp::ShiftRight, 8),
        TokenKind::Plus => (BinaryOp::Add, 9),
        TokenKind::Minus => (BinaryOp::Sub, 9),
        TokenKind::Star => (BinaryOp::Mul, 10),
        TokenKind::Slash => (BinaryOp::Div, 10),
        TokenKind::Percent => (BinaryOp::Rem, 10),
        _ => return None,
    })
}
