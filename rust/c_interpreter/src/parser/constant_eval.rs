//! Compile-time integer expressions and their type rules.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn eval_integer_constant_expr(&self, expr: &Expr) -> Result<i128, Diagnostic> {
        Ok(self.eval_typed_integer_constant_expr(expr)?.value)
    }

    pub(super) fn eval_typed_integer_constant_expr(
        &self,
        expr: &Expr,
    ) -> Result<ConstantInteger, Diagnostic> {
        match expr {
            Expr::Number(literal, span) => match literal.value {
                NumberValue::Integer(value) => Ok(ConstantInteger {
                    ty: literal.ty.clone(),
                    value,
                }),
                NumberValue::Floating(_) => Err(Diagnostic::error(
                    "an integer constant expression cannot use a floating-point value",
                    *span,
                )),
            },
            Expr::CharLiteral(value, _) => Ok(ConstantInteger {
                ty: CType::Int,
                value: *value as i128,
            }),
            Expr::WideCharLiteral(value, _) => Ok(ConstantInteger {
                ty: CType::Int,
                value: *value as i128,
            }),
            Expr::Utf16CharLiteral(value, _) => Ok(ConstantInteger {
                ty: CType::UnsignedShort,
                value: *value as i128,
            }),
            Expr::Utf32CharLiteral(value, _) => Ok(ConstantInteger {
                ty: CType::UnsignedInt,
                value: *value as i128,
            }),
            Expr::Variable(name, span) => self
                .lookup_enum_constant_value(name)
                .map(|value| ConstantInteger {
                    ty: CType::Int,
                    value,
                })
                .ok_or_else(|| {
                    Diagnostic::error(
                        format!("identifier {} is not an integer constant expression", name),
                        *span,
                    )
                }),
            Expr::Unary { op, expr, span } => {
                let value = self.eval_typed_integer_constant_expr(expr)?;
                match op {
                    UnaryOp::Plus => self.promote_constant_integer(value),
                    UnaryOp::Minus => {
                        let value = self.promote_constant_integer(value)?;
                        self.constant_integer_negate(value, *span)
                    }
                    UnaryOp::LogicalNot => Ok(ConstantInteger {
                        ty: CType::Int,
                        value: (value.value == 0) as i128,
                    }),
                    UnaryOp::BitNot => {
                        let value = self.promote_constant_integer(value)?;
                        Ok(ConstantInteger {
                            value: value
                                .ty
                                .normalize_integer_value(!value.value)
                                .expect("promoted integer has an integer representation"),
                            ty: value.ty,
                        })
                    }
                    _ => Err(Diagnostic::error(
                        "unsupported operator in integer constant expression",
                        *span,
                    )),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.eval_typed_integer_constant_expr(lhs)?;
                if *op == BinaryOp::LogicalAnd && lhs.value == 0 {
                    return Ok(ConstantInteger {
                        ty: CType::Int,
                        value: 0,
                    });
                }
                if *op == BinaryOp::LogicalOr && lhs.value != 0 {
                    return Ok(ConstantInteger {
                        ty: CType::Int,
                        value: 1,
                    });
                }
                let rhs = self.eval_typed_integer_constant_expr(rhs)?;
                self.eval_constant_integer_binary(*op, lhs, rhs, *span)
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                span,
            } => {
                let condition = self.eval_typed_integer_constant_expr(condition)?;
                let (selected, unselected) = if condition.value != 0 {
                    (then_expr.as_ref(), else_expr.as_ref())
                } else {
                    (else_expr.as_ref(), then_expr.as_ref())
                };
                self.validate_unevaluated_integer_constant_expr(unselected)?;
                let then_ty = self.parser_sizeof_expr_type(then_expr)?;
                let else_ty = self.parser_sizeof_expr_type(else_expr)?;
                if !then_ty.is_integer() || !else_ty.is_integer() {
                    return Err(Diagnostic::error(
                        "integer constant expression must have integer type",
                        *span,
                    ));
                }
                let result_ty = self.usual_constant_integer_type(
                    &self
                        .promote_constant_integer(ConstantInteger {
                            ty: then_ty,
                            value: 0,
                        })?
                        .ty,
                    &self
                        .promote_constant_integer(ConstantInteger {
                            ty: else_ty,
                            value: 0,
                        })?
                        .ty,
                );
                let selected = self.eval_typed_integer_constant_expr(selected)?;
                Ok(self.convert_constant_integer(selected, &result_ty))
            }
            Expr::Cast { ty, expr, span, .. } => {
                if !ty.is_integer() {
                    return Err(Diagnostic::error(
                        "integer constant expression must have integer type",
                        *span,
                    ));
                }
                let value = match expr.as_ref() {
                    Expr::Number(literal, _) => match literal.value {
                        NumberValue::Floating(value) => {
                            self.convert_floating_constant_to_integer(value, ty, *span)?
                        }
                        NumberValue::Integer(_) => self.eval_typed_integer_constant_expr(expr)?,
                    },
                    _ => self.eval_typed_integer_constant_expr(expr)?,
                };
                Ok(self.convert_constant_integer(value, ty))
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => {
                let selected = self.select_parser_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                )?;
                self.eval_typed_integer_constant_expr(selected)
            }
            Expr::SizeofType {
                ty,
                vla_bounds,
                span,
            } => {
                if vla_bounds.iter().any(Option::is_some) {
                    return Err(Diagnostic::error(
                        "sizeof a variably modified type is not an integer constant expression",
                        *span,
                    ));
                }
                Ok(ConstantInteger {
                    ty: CType::UnsignedLong,
                    value: i128::try_from(self.type_size_of(ty)?)
                        .map_err(|_| Diagnostic::error("sizeof result is too large", *span))?,
                })
            }
            Expr::SizeofExpr { expr, span } => {
                let ty = self.parser_sizeof_expr_type(expr)?;
                Ok(ConstantInteger {
                    ty: CType::UnsignedLong,
                    value: i128::try_from(self.type_size_of(&ty)?)
                        .map_err(|_| Diagnostic::error("sizeof result is too large", *span))?,
                })
            }
            Expr::OffsetOf {
                ty,
                designators,
                span,
            } => Ok(ConstantInteger {
                ty: CType::UnsignedLong,
                value: self.eval_offsetof_constant_expr(ty, designators, *span)?,
            }),
            _ => Err(Diagnostic::error(
                "expression is not a supported integer constant expression",
                expr.span(),
            )),
        }
    }

    fn validate_unevaluated_integer_constant_expr(&self, expr: &Expr) -> Result<(), Diagnostic> {
        match expr {
            Expr::Assign { .. }
            | Expr::CompoundAssign { .. }
            | Expr::Postfix { .. }
            | Expr::Call { .. }
            | Expr::Binary {
                op: BinaryOp::Comma,
                ..
            }
            | Expr::Unary {
                op: UnaryOp::PreIncrement | UnaryOp::PreDecrement,
                ..
            } => Ok(()),
            Expr::Number(literal, span) => match literal.value {
                NumberValue::Integer(_) => Ok(()),
                NumberValue::Floating(_) => Err(Diagnostic::error(
                    "an integer constant expression cannot use a floating-point value",
                    *span,
                )),
            },
            Expr::CharLiteral(..)
            | Expr::WideCharLiteral(..)
            | Expr::Utf16CharLiteral(..)
            | Expr::Utf32CharLiteral(..)
            | Expr::OffsetOf { .. } => Ok(()),
            Expr::Variable(name, span) => {
                if self.lookup_enum_constant_value(name).is_some() {
                    Ok(())
                } else {
                    Err(Diagnostic::error(
                        format!("identifier {name} is not an integer constant expression"),
                        *span,
                    ))
                }
            }
            Expr::Unary { op, expr, span } => match op {
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::LogicalNot | UnaryOp::BitNot => {
                    self.validate_unevaluated_integer_constant_expr(expr)
                }
                _ => Err(Diagnostic::error(
                    "unsupported operator in integer constant expression",
                    *span,
                )),
            },
            Expr::Binary { lhs, rhs, .. } => {
                self.validate_unevaluated_integer_constant_expr(lhs)?;
                self.validate_unevaluated_integer_constant_expr(rhs)
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.validate_unevaluated_integer_constant_expr(condition)?;
                self.validate_unevaluated_integer_constant_expr(then_expr)?;
                self.validate_unevaluated_integer_constant_expr(else_expr)
            }
            Expr::Cast { ty, expr, span, .. } => {
                if !ty.is_integer() {
                    return Err(Diagnostic::error(
                        "integer constant expression must have integer type",
                        *span,
                    ));
                }
                if matches!(
                    expr.as_ref(),
                    Expr::Number(literal, _)
                        if matches!(literal.value, NumberValue::Floating(_))
                ) {
                    Ok(())
                } else {
                    self.validate_unevaluated_integer_constant_expr(expr)
                }
            }
            Expr::SizeofType {
                vla_bounds, span, ..
            } => {
                if vla_bounds.iter().any(Option::is_some) {
                    Err(Diagnostic::error(
                        "sizeof a variably modified type is not an integer constant expression",
                        *span,
                    ))
                } else {
                    Ok(())
                }
            }
            Expr::SizeofExpr { .. } => Ok(()),
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => self.validate_unevaluated_integer_constant_expr(
                self.select_parser_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                )?,
            ),
            Expr::StringLiteral(..)
            | Expr::WideStringLiteral(..)
            | Expr::Utf16StringLiteral(..)
            | Expr::Utf32StringLiteral(..)
            | Expr::Subscript { .. }
            | Expr::Member { .. }
            | Expr::CompoundLiteral { .. }
            | Expr::VaArg { .. } => Err(Diagnostic::error(
                "expression is not a supported integer constant expression",
                expr.span(),
            )),
        }
    }

    fn promote_constant_integer(
        &self,
        value: ConstantInteger,
    ) -> Result<ConstantInteger, Diagnostic> {
        let promoted = match value.ty.unqualified() {
            CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(_, _) => CType::Int,
            _ if value.ty.is_integer() => value.ty.clone(),
            _ => {
                return Err(Diagnostic::error(
                    "integer constant expression requires integer operands",
                    Span::new(crate::source::FileId(0), 0, 0),
                ));
            }
        };
        Ok(self.convert_constant_integer(value, &promoted))
    }

    fn convert_floating_constant_to_integer(
        &self,
        value: f64,
        target: &CType,
        span: Span,
    ) -> Result<ConstantInteger, Diagnostic> {
        if matches!(target.unqualified(), CType::Bool) {
            return Ok(ConstantInteger {
                ty: target.clone(),
                value: (value != 0.0) as i128,
            });
        }
        let bits = target.integer_bits().ok_or_else(|| {
            Diagnostic::error("integer constant expression must have integer type", span)
        })?;
        let truncated = value.trunc();
        let exponent = i32::try_from(bits).expect("supported integer widths fit in i32");
        let (lower, upper_exclusive) = if target.is_signed_integer() {
            let limit = 2f64.powi(exponent - 1);
            (-limit, limit)
        } else {
            (0.0, 2f64.powi(exponent))
        };
        if truncated < lower || truncated >= upper_exclusive {
            return Err(Diagnostic::error(
                "floating constant is outside the range of the integer cast type",
                span,
            ));
        }
        Ok(ConstantInteger {
            ty: target.clone(),
            value: truncated as i128,
        })
    }

    fn parser_sizeof_expr_type(&self, expr: &Expr) -> Result<CType, Diagnostic> {
        match expr {
            Expr::Number(literal, _) => Ok(literal.ty.clone()),
            Expr::CharLiteral(..) => Ok(CType::Int),
            Expr::WideCharLiteral(..) => Ok(CType::Int),
            Expr::Utf16CharLiteral(..) => Ok(CType::UnsignedShort),
            Expr::Utf32CharLiteral(..) => Ok(CType::UnsignedInt),
            Expr::StringLiteral(text, _) => Ok(CType::array_of(CType::Char, text.narrow_len() + 1)),
            Expr::WideStringLiteral(text, _) => {
                Ok(CType::array_of(CType::Int, text.utf32_units().len() + 1))
            }
            Expr::Utf16StringLiteral(text, _) => Ok(CType::array_of(
                CType::UnsignedShort,
                text.utf16_units().len() + 1,
            )),
            Expr::Utf32StringLiteral(text, _) => Ok(CType::array_of(
                CType::UnsignedInt,
                text.utf32_units().len() + 1,
            )),
            Expr::Variable(name, span) => self
                .visible_scope_entry(name)
                .and_then(|entry| {
                    entry
                        .ordinary_ty
                        .clone()
                        .or_else(|| entry.enum_constant.map(|_| CType::Int))
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!("cannot determine the type of {name}"), *span)
                }),
            Expr::Unary { op, expr, span } => {
                let ty = self.parser_sizeof_expr_type(expr)?;
                match op {
                    UnaryOp::AddressOf => Ok(CType::pointer_to(ty)),
                    UnaryOp::Dereference => ty.element_type().cloned().ok_or_else(|| {
                        Diagnostic::error("cannot dereference a non-pointer expression", *span)
                    }),
                    UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => {
                        if ty.is_integer() {
                            Ok(self
                                .promote_constant_integer(ConstantInteger { ty, value: 0 })?
                                .ty)
                        } else if ty.is_arithmetic() && *op != UnaryOp::BitNot {
                            Ok(ty)
                        } else {
                            Err(Diagnostic::error(
                                "invalid unary operand while determining expression type",
                                *span,
                            ))
                        }
                    }
                    UnaryOp::LogicalNot => Ok(CType::Int),
                    UnaryOp::PreIncrement | UnaryOp::PreDecrement => Ok(ty),
                }
            }
            Expr::Postfix { expr, .. } => self.parser_sizeof_expr_type(expr),
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs_ty = self.parser_sizeof_expr_type(lhs)?;
                let rhs_ty = self.parser_sizeof_expr_type(rhs)?;
                match op {
                    BinaryOp::Comma => Ok(rhs_ty),
                    BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr
                    | BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => Ok(CType::Int),
                    BinaryOp::ShiftLeft | BinaryOp::ShiftRight => Ok(self
                        .promote_constant_integer(ConstantInteger {
                            ty: lhs_ty,
                            value: 0,
                        })?
                        .ty),
                    BinaryOp::Add | BinaryOp::Sub if lhs_ty.is_pointer() && rhs_ty.is_integer() => {
                        Ok(lhs_ty)
                    }
                    BinaryOp::Add if lhs_ty.is_integer() && rhs_ty.is_pointer() => Ok(rhs_ty),
                    BinaryOp::Sub if lhs_ty.is_pointer() && rhs_ty.is_pointer() => Ok(CType::Long),
                    _ if lhs_ty.is_integer() && rhs_ty.is_integer() => {
                        let lhs_ty = self
                            .promote_constant_integer(ConstantInteger {
                                ty: lhs_ty,
                                value: 0,
                            })?
                            .ty;
                        let rhs_ty = self
                            .promote_constant_integer(ConstantInteger {
                                ty: rhs_ty,
                                value: 0,
                            })?
                            .ty;
                        Ok(self.usual_constant_integer_type(&lhs_ty, &rhs_ty))
                    }
                    _ => Err(Diagnostic::error(
                        "cannot determine the type of this binary expression",
                        *span,
                    )),
                }
            }
            Expr::Subscript { base, index, span } => {
                let base_ty = self.parser_sizeof_expr_type(base)?;
                let index_ty = self.parser_sizeof_expr_type(index)?;
                let pointer_ty =
                    if base_ty.is_pointer() || matches!(base_ty.unqualified(), CType::Array(..)) {
                        base_ty
                    } else if index_ty.is_pointer()
                        || matches!(index_ty.unqualified(), CType::Array(..))
                    {
                        index_ty
                    } else {
                        return Err(Diagnostic::error(
                            "cannot determine the element type of this subscript",
                            *span,
                        ));
                    };
                pointer_ty.element_type().cloned().ok_or_else(|| {
                    Diagnostic::error("subscript requires an array or pointer", *span)
                })
            }
            Expr::Assign { lhs, .. } | Expr::CompoundAssign { lhs, .. } => {
                self.parser_sizeof_expr_type(lhs)
            }
            Expr::Cast { ty, .. } => Ok(ty.clone()),
            Expr::SizeofType { .. } | Expr::SizeofExpr { .. } | Expr::OffsetOf { .. } => {
                Ok(CType::UnsignedLong)
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => self.parser_sizeof_expr_type(self.select_parser_generic_association(
                control,
                associations,
                default.as_deref(),
                *span,
            )?),
            Expr::CompoundLiteral { ty, .. } | Expr::VaArg { ty, .. } => Ok(ty.clone()),
            Expr::Conditional {
                then_expr,
                else_expr,
                span,
                ..
            } => {
                let then_ty = self.parser_sizeof_expr_type(then_expr)?;
                let else_ty = self.parser_sizeof_expr_type(else_expr)?;
                if then_ty.is_integer() && else_ty.is_integer() {
                    let then_ty = self
                        .promote_constant_integer(ConstantInteger {
                            ty: then_ty,
                            value: 0,
                        })?
                        .ty;
                    let else_ty = self
                        .promote_constant_integer(ConstantInteger {
                            ty: else_ty,
                            value: 0,
                        })?
                        .ty;
                    Ok(self.usual_constant_integer_type(&then_ty, &else_ty))
                } else if then_ty == else_ty {
                    Ok(then_ty)
                } else {
                    Err(Diagnostic::error(
                        "cannot determine the conditional expression type",
                        *span,
                    ))
                }
            }
            Expr::Call { callee, span, .. } => {
                let callee_ty = self.parser_sizeof_expr_type(callee)?;
                match callee_ty.unqualified() {
                    CType::Function(return_ty, _, _) => Ok((**return_ty).clone()),
                    CType::Pointer(inner) => match inner.unqualified() {
                        CType::Function(return_ty, _, _) => Ok((**return_ty).clone()),
                        _ => Err(Diagnostic::error("call target is not a function", *span)),
                    },
                    _ => Err(Diagnostic::error("call target is not a function", *span)),
                }
            }
            Expr::Member { base, member, span } => {
                let base_ty = self.parser_sizeof_expr_type(base)?;
                self.resolve_visible_member_chain(&base_ty, member)
                    .and_then(|members| members.last().map(|member| member.ty.clone()))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", base_ty, member),
                            *span,
                        )
                    })
            }
        }
    }

    fn select_parser_generic_association<'b>(
        &self,
        control: &Expr,
        associations: &'b [GenericAssociation],
        default: Option<&'b Expr>,
        span: Span,
    ) -> Result<&'b Expr, Diagnostic> {
        let raw_controlling_ty = self.parser_sizeof_expr_type(control)?;
        let controlling_ty = match raw_controlling_ty.unqualified() {
            CType::Array(inner, _) => CType::pointer_to((**inner).clone()),
            CType::Function(..) => CType::pointer_to(raw_controlling_ty.unqualified().clone()),
            ty => ty.clone(),
        };
        associations
            .iter()
            .find(|association| generic_types_compatible(&association.ty, &controlling_ty))
            .map(|association| &association.expr)
            .or(default)
            .ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "_Generic has no association compatible with controlling type {controlling_ty}"
                    ),
                    span,
                )
            })
    }

    fn convert_constant_integer(&self, value: ConstantInteger, target: &CType) -> ConstantInteger {
        ConstantInteger {
            value: target
                .normalize_integer_value(value.value)
                .expect("integer target has an integer representation"),
            ty: target.clone(),
        }
    }

    fn usual_constant_integer_type(&self, lhs: &CType, rhs: &CType) -> CType {
        if lhs == rhs {
            return lhs.clone();
        }
        if lhs.is_signed_integer() == rhs.is_signed_integer() {
            return if lhs.integer_rank() >= rhs.integer_rank() {
                lhs.clone()
            } else {
                rhs.clone()
            };
        }
        let (signed_ty, unsigned_ty) = if lhs.is_signed_integer() {
            (lhs, rhs)
        } else {
            (rhs, lhs)
        };
        if signed_ty.integer_rank() > unsigned_ty.integer_rank() {
            let (_, signed_max) = signed_ty.integer_bounds().unwrap();
            let (_, unsigned_max) = unsigned_ty.integer_bounds().unwrap();
            if signed_max >= unsigned_max {
                signed_ty.clone()
            } else {
                signed_ty.unsigned_variant().unwrap()
            }
        } else {
            unsigned_ty.clone()
        }
    }

    fn constant_integer_negate(
        &self,
        value: ConstantInteger,
        span: Span,
    ) -> Result<ConstantInteger, Diagnostic> {
        if value.ty.is_unsigned_integer() {
            return Ok(ConstantInteger {
                value: value
                    .ty
                    .normalize_integer_value(-value.value)
                    .expect("unsigned integer has an integer representation"),
                ty: value.ty,
            });
        }
        let negated = value
            .value
            .checked_neg()
            .filter(|result| {
                value
                    .ty
                    .integer_bounds()
                    .is_some_and(|(min, max)| (min..=max).contains(result))
            })
            .ok_or_else(|| Diagnostic::error("constant expression overflow", span))?;
        Ok(ConstantInteger {
            ty: value.ty,
            value: negated,
        })
    }

    fn eval_constant_integer_binary(
        &self,
        op: BinaryOp,
        lhs: ConstantInteger,
        rhs: ConstantInteger,
        span: Span,
    ) -> Result<ConstantInteger, Diagnostic> {
        if op == BinaryOp::Comma {
            return Err(Diagnostic::error(
                "comma operator is not allowed in an evaluated integer constant expression",
                span,
            ));
        }
        if matches!(op, BinaryOp::LogicalAnd | BinaryOp::LogicalOr) {
            let value = match op {
                BinaryOp::LogicalAnd => lhs.value != 0 && rhs.value != 0,
                BinaryOp::LogicalOr => lhs.value != 0 || rhs.value != 0,
                _ => unreachable!(),
            };
            return Ok(ConstantInteger {
                ty: CType::Int,
                value: value as i128,
            });
        }
        if matches!(op, BinaryOp::ShiftLeft | BinaryOp::ShiftRight) {
            let lhs = self.promote_constant_integer(lhs)?;
            let rhs = self.promote_constant_integer(rhs)?;
            let bits = lhs.ty.integer_bits().unwrap() as i128;
            if !(0..bits).contains(&rhs.value) {
                return Err(Diagnostic::error(
                    "shift count is negative or too large in constant expression",
                    span,
                ));
            }
            let shift = rhs.value as u32;
            let shifted = match op {
                BinaryOp::ShiftLeft if lhs.ty.is_unsigned_integer() => lhs
                    .ty
                    .normalize_integer_value(lhs.value << shift)
                    .expect("unsigned integer has an integer representation"),
                BinaryOp::ShiftLeft => {
                    if lhs.value < 0 {
                        return Err(Diagnostic::error(
                            "left shift of a negative value in constant expression",
                            span,
                        ));
                    }
                    let result = lhs.value << shift;
                    if !lhs
                        .ty
                        .integer_bounds()
                        .is_some_and(|(min, max)| (min..=max).contains(&result))
                    {
                        return Err(Diagnostic::error("constant expression overflow", span));
                    }
                    result
                }
                BinaryOp::ShiftRight => lhs.value >> shift,
                _ => unreachable!(),
            };
            return Ok(ConstantInteger {
                ty: lhs.ty,
                value: shifted,
            });
        }

        let lhs = self.promote_constant_integer(lhs)?;
        let rhs = self.promote_constant_integer(rhs)?;
        let common_ty = self.usual_constant_integer_type(&lhs.ty, &rhs.ty);
        let lhs = self.convert_constant_integer(lhs, &common_ty).value;
        let rhs = self.convert_constant_integer(rhs, &common_ty).value;
        let comparison = |value: bool| ConstantInteger {
            ty: CType::Int,
            value: value as i128,
        };
        if matches!(
            op,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        ) {
            return Ok(comparison(match op {
                BinaryOp::Equal => lhs == rhs,
                BinaryOp::NotEqual => lhs != rhs,
                BinaryOp::Less => lhs < rhs,
                BinaryOp::LessEqual => lhs <= rhs,
                BinaryOp::Greater => lhs > rhs,
                BinaryOp::GreaterEqual => lhs >= rhs,
                _ => unreachable!(),
            }));
        }
        if matches!(op, BinaryOp::Div | BinaryOp::Rem) && rhs == 0 {
            return Err(Diagnostic::error(
                "division by zero in constant expression",
                span,
            ));
        }
        if common_ty.is_unsigned_integer()
            && matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
        {
            let bits = common_ty.integer_bits().unwrap();
            let mask = (1u128 << bits) - 1;
            let lhs = lhs as u128;
            let rhs = rhs as u128;
            let value = match op {
                BinaryOp::Add => lhs.wrapping_add(rhs),
                BinaryOp::Sub => lhs.wrapping_sub(rhs),
                BinaryOp::Mul => lhs.wrapping_mul(rhs),
                _ => unreachable!(),
            } & mask;
            return Ok(ConstantInteger {
                ty: common_ty,
                value: value as i128,
            });
        }
        let raw = match op {
            BinaryOp::Add => lhs + rhs,
            BinaryOp::Sub => lhs - rhs,
            BinaryOp::Mul => lhs * rhs,
            BinaryOp::Div => {
                if common_ty.is_signed_integer()
                    && common_ty
                        .integer_bounds()
                        .is_some_and(|(min, _)| lhs == min && rhs == -1)
                {
                    return Err(Diagnostic::error("constant expression overflow", span));
                }
                lhs / rhs
            }
            BinaryOp::Rem => {
                if common_ty.is_signed_integer()
                    && common_ty
                        .integer_bounds()
                        .is_some_and(|(min, _)| lhs == min && rhs == -1)
                {
                    return Err(Diagnostic::error("constant expression overflow", span));
                }
                lhs % rhs
            }
            BinaryOp::BitAnd => lhs & rhs,
            BinaryOp::BitXor => lhs ^ rhs,
            BinaryOp::BitOr => lhs | rhs,
            _ => unreachable!(),
        };
        if common_ty.is_signed_integer()
            && !common_ty
                .integer_bounds()
                .is_some_and(|(min, max)| (min..=max).contains(&raw))
        {
            return Err(Diagnostic::error("constant expression overflow", span));
        }
        Ok(ConstantInteger {
            value: common_ty
                .normalize_integer_value(raw)
                .expect("integer result has an integer representation"),
            ty: common_ty,
        })
    }

    fn eval_offsetof_constant_expr(
        &self,
        ty: &CType,
        designators: &[Designator],
        span: Span,
    ) -> Result<i128, Diagnostic> {
        let mut current = ty.clone();
        let mut offset = 0usize;
        for designator in designators {
            match designator {
                Designator::Member(name, designator_span) => {
                    let chain = self
                        .resolve_visible_member_chain(&current, name)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                format!("{} has no member named {}", current, name),
                                *designator_span,
                            )
                        })?;
                    for member in chain {
                        offset = offset
                            .checked_add(member.offset)
                            .ok_or_else(|| Diagnostic::error("offsetof overflow", span))?;
                        current = member.ty.clone();
                    }
                }
                Designator::Index(index, designator_span) => match current.unqualified() {
                    CType::Array(inner, len) => {
                        if *index >= *len {
                            return Err(Diagnostic::error(
                                "offsetof array designator is outside the bounds of the array",
                                *designator_span,
                            ));
                        }
                        let stride = self.type_size_of(inner)?;
                        offset = offset
                            .checked_add(index.checked_mul(stride).ok_or_else(|| {
                                Diagnostic::error("offsetof overflow", *designator_span)
                            })?)
                            .ok_or_else(|| {
                                Diagnostic::error("offsetof overflow", *designator_span)
                            })?;
                        current = (**inner).clone();
                    }
                    _ => {
                        return Err(Diagnostic::error(
                            "offsetof array designator requires an array type",
                            *designator_span,
                        ));
                    }
                },
            }
        }
        i128::try_from(offset).map_err(|_| Diagnostic::error("offsetof overflow", span))
    }
}
