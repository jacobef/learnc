//! Expression types, generic selection, layout, and member lookup.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn default_argument_promotion_for_expr(
        &self,
        value: TypedValue,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if matches!(value.ty.unqualified(), CType::Float) {
            self.convert_value(value, &CType::Double, expr.span())
        } else if value.ty.is_integer() {
            let promoted = self.promoted_integer_expr_type(expr, frame, objects)?;
            self.convert_value(value, &promoted, expr.span())
        } else {
            Ok(value)
        }
    }

    pub(super) fn eval_sizeof_type(
        &self,
        ty: &CType,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let size = self.type_size_of(ty).ok_or_else(|| {
            Diagnostic::error(
                format!("sizeof cannot determine the size of type {ty}"),
                span,
            )
        })?;
        let size = self.ensure_int_range(
            size as i128,
            span,
            "sizeof result is outside the supported int range",
        )?;
        Ok(TypedValue::integer(CType::UnsignedLong, size))
    }

    pub(super) fn eval_offsetof_type(
        &self,
        ty: &CType,
        designators: &[Designator],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
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
                        if *len == 0 || *index >= *len {
                            return Err(Diagnostic::error(
                                "offsetof array designator is outside the bounds of the array",
                                *designator_span,
                            ));
                        }
                        let stride = self.type_size_of(inner).ok_or_else(|| {
                            Diagnostic::error(
                                "offsetof array designator requires a complete element type",
                                *designator_span,
                            )
                        })?;
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
        let offset = self.ensure_int_range(
            offset as i128,
            span,
            "offsetof result is outside the supported int range",
        )?;
        Ok(TypedValue::integer(CType::UnsignedLong, offset))
    }

    pub(super) fn eval_sizeof_type_name(
        &mut self,
        ty: &CType,
        vla_bounds: &[Option<Expr>],
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let ty = self.resolve_decl_type(ty, vla_bounds, frame, objects, span)?;
        self.eval_sizeof_type(&ty, span)
    }

    pub(super) fn eval_number_literal(&self, literal: &NumberLiteral) -> TypedValue {
        match literal.value {
            NumberValue::Integer(value) => TypedValue::integer(literal.ty.clone(), value),
            NumberValue::Floating(value) => TypedValue::floating(literal.ty.clone(), value),
        }
    }

    pub(super) fn expr_type(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        match expr {
            Expr::Number(literal, _) => Ok(literal.ty.clone()),
            Expr::CharLiteral(_, _) => Ok(CType::Int),
            Expr::WideCharLiteral(_, _) => Ok(self.wchar_type()),
            Expr::Utf16CharLiteral(_, _) => Ok(CType::UnsignedShort),
            Expr::Utf32CharLiteral(_, _) => Ok(CType::UnsignedInt),
            Expr::StringLiteral(text, _) => Ok(CType::array_of(CType::Char, text.narrow_len() + 1)),
            Expr::WideStringLiteral(text, _) => Ok(CType::array_of(
                self.wchar_type(),
                text.utf32_units().len() + 1,
            )),
            Expr::Utf16StringLiteral(text, _) => Ok(CType::array_of(
                CType::UnsignedShort,
                text.utf16_units().len() + 1,
            )),
            Expr::Utf32StringLiteral(text, _) => Ok(CType::array_of(
                CType::UnsignedInt,
                text.utf32_units().len() + 1,
            )),
            Expr::Variable(name, span) => {
                if let Some(object) = frame.bindings.get(name).copied() {
                    self.lookup_object(objects, object)
                        .map(|state| state.ty.clone())
                        .ok_or_else(|| {
                            Diagnostic::error(
                                format!("use of undeclared identifier {}", name),
                                *span,
                            )
                        })
                } else if let Some(decl) = frame.object_decls.get(name) {
                    Ok(decl.ty.clone())
                } else if let Some(function_decl) = frame.function_decls.get(name) {
                    Ok(self.function_declaration_type(function_decl))
                } else if let Some(object) = self.lookup_global_binding(name, span.file) {
                    self.lookup_object(objects, object)
                        .map(|state| state.ty.clone())
                        .ok_or_else(|| {
                            Diagnostic::error(
                                format!("use of undeclared identifier {}", name),
                                *span,
                            )
                        })
                } else if let Some(decl) = self.lookup_global_declaration(name, span.file) {
                    Ok(decl.ty.clone())
                } else if let Some(function_decl) =
                    self.lookup_function_declaration(name, span.file)
                {
                    Ok(self.function_declaration_type(function_decl))
                } else if let Some(function) = self.lookup_function(name, span.file) {
                    Ok(self.function_type(function))
                } else if let Some(ty) = self.builtin_function_type(name) {
                    Ok(ty)
                } else if self
                    .program
                    .enum_constants
                    .contains_key(&(span.file, name.clone()))
                {
                    Ok(CType::Int)
                } else {
                    Err(Diagnostic::error(
                        format!("use of undeclared identifier {}", name),
                        *span,
                    ))
                }
            }
            Expr::Unary { op, expr, span } => match op {
                UnaryOp::AddressOf => {
                    if let Expr::Unary {
                        op: UnaryOp::Dereference,
                        expr: pointer_expr,
                        ..
                    } = expr.as_ref()
                    {
                        let pointer_ty = self.value_expr_type(pointer_expr, frame, objects)?;
                        if pointer_ty.is_pointer() {
                            return Ok(pointer_ty);
                        }
                    }
                    Ok(CType::pointer_to(self.expr_type(expr, frame, objects)?))
                }
                UnaryOp::Dereference => {
                    let ty = self.value_expr_type(expr, frame, objects)?;
                    if matches!(ty.unqualified(), CType::Pointer(inner) if matches!(inner.unqualified(), CType::Void))
                    {
                        return Err(Diagnostic::error(
                            "the * operator cannot dereference a pointer to incomplete type void",
                            *span,
                        ));
                    }
                    match ty.element_type() {
                        Some(inner)
                            if ty.is_pointer() && !matches!(inner.unqualified(), CType::Void) =>
                        {
                            Ok(inner.clone())
                        }
                        _ => Err(Diagnostic::error(
                            format!(
                                "the * operator requires a pointer, but this expression has type {}",
                                ty
                            ),
                            *span,
                        )),
                    }
                }
                UnaryOp::Plus | UnaryOp::Minus => {
                    let ty = self.value_expr_type(expr, frame, objects)?;
                    if ty.is_floating() || ty.is_complex() {
                        return Ok(ty);
                    }
                    if !ty.is_integer() {
                        return Err(Diagnostic::error(
                            format!("expected arithmetic type, got {}", ty),
                            *span,
                        ));
                    }
                    self.promoted_integer_expr_type(expr, frame, objects)
                }
                UnaryOp::BitNot => {
                    let ty = self.value_expr_type(expr, frame, objects)?;
                    if !ty.is_integer() {
                        return Err(Diagnostic::error(
                            format!("expected integer type, got {}", ty),
                            *span,
                        ));
                    }
                    self.promoted_integer_expr_type(expr, frame, objects)
                }
                UnaryOp::LogicalNot => Ok(CType::Int),
                UnaryOp::PreIncrement | UnaryOp::PreDecrement => {
                    self.expr_type(expr, frame, objects)
                }
            },
            Expr::Postfix { expr, .. } => self.expr_type(expr, frame, objects),
            Expr::Binary { op, lhs, rhs, .. } => match op {
                BinaryOp::Comma => {
                    let _ = self.value_expr_type(lhs, frame, objects)?;
                    self.value_expr_type(rhs, frame, objects)
                }
                BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    if (lhs_ty.is_arithmetic() || lhs_ty.is_pointer())
                        && (rhs_ty.is_arithmetic() || rhs_ty.is_pointer())
                    {
                        Ok(CType::Int)
                    } else {
                        Err(Diagnostic::error(
                            "logical operators require scalar operands",
                            lhs.span().merge(rhs.span()),
                        ))
                    }
                }
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    let compatible = (lhs_ty.is_arithmetic() && rhs_ty.is_arithmetic())
                        || (lhs_ty.is_pointer()
                            && rhs_ty.is_pointer()
                            && match (lhs_ty.unqualified(), rhs_ty.unqualified()) {
                                (CType::Pointer(lhs), CType::Pointer(rhs)) => {
                                    self.composite_pointer_target_type(lhs, rhs).is_some()
                                }
                                _ => false,
                            })
                        || (lhs_ty.is_pointer()
                            && self.is_null_pointer_constant(rhs, frame, objects)?)
                        || (rhs_ty.is_pointer()
                            && self.is_null_pointer_constant(lhs, frame, objects)?);
                    if compatible {
                        Ok(CType::Int)
                    } else {
                        Err(Diagnostic::error(
                            "invalid operands: equality comparison requires arithmetic operands, compatible pointer operand types, or a null pointer constant",
                            lhs.span().merge(rhs.span()),
                        ))
                    }
                }
                BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    if lhs_ty.is_arithmetic()
                        && rhs_ty.is_arithmetic()
                        && !lhs_ty.is_complex()
                        && !rhs_ty.is_complex()
                    {
                        return Ok(CType::Int);
                    }
                    if let (CType::Pointer(lhs_inner), CType::Pointer(rhs_inner)) =
                        (lhs_ty.unqualified(), rhs_ty.unqualified())
                        && self.pointer_targets_are_compatible_object_types(
                            lhs_inner, rhs_inner, false,
                        )
                    {
                        return Ok(CType::Int);
                    }
                    Err(Diagnostic::error(
                        format!(
                            "relational operators require real operands or pointers to compatible object types, but operands have types {lhs_ty} and {rhs_ty}"
                        ),
                        lhs.span().merge(rhs.span()),
                    ))
                }
                BinaryOp::Add => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    if lhs_ty.is_pointer() && rhs_ty.is_integer() {
                        self.require_complete_pointer_arithmetic_type(
                            &lhs_ty,
                            lhs.span().merge(rhs.span()),
                        )?;
                        Ok(lhs_ty)
                    } else if lhs_ty.is_integer() && rhs_ty.is_pointer() {
                        self.require_complete_pointer_arithmetic_type(
                            &rhs_ty,
                            lhs.span().merge(rhs.span()),
                        )?;
                        Ok(rhs_ty)
                    } else {
                        self.usual_arithmetic_expr_type(
                            lhs,
                            rhs,
                            lhs.span().merge(rhs.span()),
                            frame,
                            objects,
                        )
                    }
                }
                BinaryOp::Sub => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    if lhs_ty.is_pointer() && rhs_ty.is_integer() {
                        self.require_complete_pointer_arithmetic_type(
                            &lhs_ty,
                            lhs.span().merge(rhs.span()),
                        )?;
                        Ok(lhs_ty)
                    } else if let (CType::Pointer(lhs_inner), CType::Pointer(rhs_inner)) =
                        (lhs_ty.unqualified(), rhs_ty.unqualified())
                    {
                        if !self
                            .pointer_targets_are_compatible_object_types(lhs_inner, rhs_inner, true)
                        {
                            return Err(Diagnostic::error(
                                "pointer subtraction requires pointers to compatible complete object types",
                                lhs.span().merge(rhs.span()),
                            ));
                        }
                        Ok(CType::Long)
                    } else {
                        self.usual_arithmetic_expr_type(
                            lhs,
                            rhs,
                            lhs.span().merge(rhs.span()),
                            frame,
                            objects,
                        )
                    }
                }
                BinaryOp::Mul | BinaryOp::Div => self.usual_arithmetic_expr_type(
                    lhs,
                    rhs,
                    lhs.span().merge(rhs.span()),
                    frame,
                    objects,
                ),
                BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitXor | BinaryOp::BitOr => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    if !lhs_ty.is_integer() || !rhs_ty.is_integer() {
                        return Err(Diagnostic::error(
                            format!(
                                "this operator requires integer operands, but the operands have types {} and {}",
                                lhs_ty, rhs_ty
                            ),
                            lhs.span().merge(rhs.span()),
                        ));
                    }
                    self.usual_arithmetic_expr_type(
                        lhs,
                        rhs,
                        lhs.span().merge(rhs.span()),
                        frame,
                        objects,
                    )
                }
                BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                    let lhs_ty = self.value_expr_type(lhs, frame, objects)?;
                    let rhs_ty = self.value_expr_type(rhs, frame, objects)?;
                    if !lhs_ty.is_integer() || !rhs_ty.is_integer() {
                        return Err(Diagnostic::error(
                            "shift operators require integer operands",
                            lhs.span().merge(rhs.span()),
                        ));
                    }
                    self.promoted_integer_type(&lhs_ty, lhs.span())
                }
            },
            Expr::Subscript { base, index, span } => {
                let base_ty = self.value_expr_type(base, frame, objects)?;
                let index_ty = self.value_expr_type(index, frame, objects)?;
                let pointer_ty = if base_ty.is_pointer() && index_ty.is_integer() {
                    base_ty
                } else if base_ty.is_integer() && index_ty.is_pointer() {
                    index_ty
                } else {
                    return Err(Diagnostic::error(
                        format!(
                            "array subscripting requires an array or pointer and an integer index, but the operands have types {} and {}",
                            base_ty, index_ty
                        ),
                        *span,
                    ));
                };
                self.require_complete_pointer_arithmetic_type(&pointer_ty, *span)?;
                Ok(pointer_ty
                    .element_type()
                    .expect("pointer type was checked")
                    .clone())
            }
            Expr::Assign { lhs, .. } | Expr::CompoundAssign { lhs, .. } => {
                self.expr_type(lhs, frame, objects)
            }
            Expr::SizeofType { .. } | Expr::SizeofExpr { .. } | Expr::OffsetOf { .. } => {
                Ok(CType::UnsignedLong)
            }
            Expr::Cast { ty, vla_bounds, .. } => {
                Ok(Self::resolve_vla_type_for_constraints(ty, vla_bounds).0)
            }
            Expr::CompoundLiteral {
                ty, initializer, ..
            } => self.complete_compound_literal_type(ty, initializer, frame, objects),
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => {
                let selected = self.select_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                    frame,
                    objects,
                )?;
                self.expr_type(selected, frame, objects)
            }
            Expr::VaArg { ty, .. } => Ok(ty.clone()),
            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => self.conditional_result_type(then_expr, else_expr, expr.span(), frame, objects),
            Expr::Call {
                callee,
                declared_callee_type,
                span,
                ..
            } => {
                if matches!(callee.as_ref(), Expr::Variable(name, _) if name == "printf") {
                    return Ok(CType::Int);
                }
                let callee_ty = declared_callee_type
                    .clone()
                    .unwrap_or(self.value_expr_type(callee, frame, objects)?);
                match callee_ty.unqualified() {
                    CType::Pointer(inner) => match inner.unqualified() {
                        CType::Function(return_type, _, _) => Ok((**return_type).clone()),
                        _ => Err(Diagnostic::error("unsupported call target", *span)),
                    },
                    CType::Function(return_type, _, _) => Ok((**return_type).clone()),
                    _ => Err(Diagnostic::error("unsupported call target", *span)),
                }
            }
            Expr::Member { base, member, span } => {
                let base_ty = self.expr_type(base, frame, objects)?;
                self.member_access_type(&base_ty, member, *span)
            }
        }
    }

    pub(super) fn require_complete_pointer_arithmetic_type(
        &self,
        pointer_ty: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let CType::Pointer(inner) = pointer_ty.unqualified() else {
            return Err(Diagnostic::error(
                "pointer arithmetic requires a pointer operand",
                span,
            ));
        };
        let message = match inner.unqualified() {
            CType::Void => Some("pointer arithmetic cannot be performed on void*".to_owned()),
            CType::Function(..) => {
                Some("pointer arithmetic cannot be performed on a function pointer".to_owned())
            }
            _ if !self.type_is_complete(inner) => Some(format!(
                "pointer arithmetic cannot be performed because pointed-to type {inner} is incomplete"
            )),
            _ => None,
        };
        if let Some(message) = message {
            return Err(Diagnostic::error(message, span));
        }
        Ok(())
    }

    pub(super) fn value_expr_type(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        let ty = self.expr_type(expr, frame, objects)?;
        Ok(match ty.unqualified() {
            CType::Function(..) => CType::pointer_to(ty),
            CType::Array(inner, _) => CType::pointer_to((**inner).clone()),
            _ if self.expr_is_lvalue(expr, frame, objects)? => ty.unqualified().clone(),
            _ => ty,
        })
    }

    pub(super) fn unqualified_value_type(ty: &CType) -> CType {
        match ty.unqualified() {
            CType::Array(inner, len) => CType::array_of(Self::unqualified_value_type(inner), *len),
            _ => ty.unqualified().clone(),
        }
    }

    fn generic_controlling_type(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        Ok(self
            .value_expr_type(expr, frame, objects)?
            .unqualified()
            .clone())
    }

    fn generic_type_compatible(&self, lhs: &CType, rhs: &CType) -> bool {
        self.cross_unit_tagged_type_compatible(lhs, rhs)
    }

    pub(super) fn select_generic_association<'b>(
        &self,
        control: &Expr,
        associations: &'b [GenericAssociation],
        default: Option<&'b Expr>,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<&'b Expr, Diagnostic> {
        let controlling_ty = self.generic_controlling_type(control, frame, objects)?;
        let mut matched = None;
        for association in associations {
            if self.generic_type_compatible(&association.ty, &controlling_ty) {
                if matched.is_some() {
                    return Err(Diagnostic::error(
                        "_Generic has multiple associations compatible with the controlling expression type",
                        association.span,
                    ));
                }
                matched = Some(&association.expr);
            }
        }
        matched.or(default).ok_or_else(|| {
            Diagnostic::error(
                format!(
                    "_Generic has no association compatible with controlling type {}",
                    controlling_ty
                ),
                span,
            )
        })
    }

    pub(super) fn expr_designates_bit_field(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<bool, Diagnostic> {
        Ok(self.expr_bit_field_width(expr, frame, objects)?.is_some())
    }

    pub(super) fn expr_bit_field_width(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<Option<u8>, Diagnostic> {
        match expr {
            Expr::Member { base, member, span } => {
                let base_ty = self.expr_type(base, frame, objects)?;
                let ResolvedMemberAccess {
                    bit_field_width, ..
                } = self.resolve_member_access(&base_ty, member, *span)?;
                Ok(bit_field_width)
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => {
                let selected = self.select_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                    frame,
                    objects,
                )?;
                self.expr_bit_field_width(selected, frame, objects)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn promoted_integer_expr_type(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        let ty = self.value_expr_type(expr, frame, objects)?;
        if !ty.is_integer() {
            return Ok(ty);
        }
        if let Some(width) = self.expr_bit_field_width(expr, frame, objects)? {
            return Ok(self.promoted_bit_field_type(&ty, width));
        }
        self.promoted_integer_type(&ty, expr.span())
    }

    pub(super) fn promoted_bit_field_type(&self, ty: &CType, width: u8) -> CType {
        let width = u32::from(width);
        let int_bits = CType::Int.integer_bits().unwrap();
        let unsigned = ty.is_unsigned_integer();
        if (!unsigned && width <= int_bits) || (unsigned && width < int_bits) {
            CType::Int
        } else if width <= CType::UnsignedInt.integer_bits().unwrap() {
            CType::UnsignedInt
        } else {
            ty.unqualified().clone()
        }
    }

    pub(super) fn usual_arithmetic_expr_type(
        &self,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        let lhs_ty = self.promoted_integer_expr_type(lhs, frame, objects)?;
        let rhs_ty = self.promoted_integer_expr_type(rhs, frame, objects)?;
        self.usual_arithmetic_type(&lhs_ty, &rhs_ty, span)
    }

    pub(super) fn promote_integer_expr_value(
        &self,
        expr: &Expr,
        value: TypedValue,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if !value.ty.is_integer() {
            return Ok(value);
        }
        let promoted = self.promoted_integer_expr_type(expr, frame, objects)?;
        self.convert_value(value, &promoted, expr.span())
    }

    pub(super) fn promoted_integer_type(
        &self,
        ty: &CType,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        if !ty.is_integer() {
            return Err(Diagnostic::error(
                format!("expected integer type, got {}", ty),
                span,
            ));
        }
        Ok(match ty.unqualified() {
            CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(_, _) => CType::Int,
            _ => ty.unqualified().clone(),
        })
    }

    pub(super) fn eval_integer_constant_expr(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<i128, Diagnostic> {
        self.eval_typed_integer_constant_expr(expr, frame, objects)?
            .to_int()
    }

    pub(super) fn eval_typed_integer_constant_expr(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        match expr {
            Expr::Number(literal, span) => match literal.value {
                NumberValue::Integer(value) => Ok(TypedValue::integer(literal.ty.clone(), value)),
                NumberValue::Floating(_) => Err(Diagnostic::error(
                    "an integer constant expression cannot use a floating-point value",
                    *span,
                )),
            },
            Expr::CharLiteral(value, _) => Ok(TypedValue::int(*value as i128)),
            Expr::WideCharLiteral(value, _) => {
                Ok(TypedValue::integer(self.wchar_type(), *value as i128))
            }
            Expr::Utf16CharLiteral(value, _) => {
                Ok(TypedValue::integer(CType::UnsignedShort, *value as i128))
            }
            Expr::Utf32CharLiteral(value, _) => {
                Ok(TypedValue::integer(CType::UnsignedInt, *value as i128))
            }
            Expr::Variable(name, span) => self
                .program
                .enum_constants
                .get(&(span.file, name.clone()))
                .copied()
                .map(TypedValue::int)
                .ok_or_else(|| {
                    Diagnostic::error(
                        format!("identifier {} is not an integer constant expression", name),
                        *span,
                    )
                }),
            Expr::Unary { op, expr, span } => {
                let value = self.eval_typed_integer_constant_expr(expr, frame, objects)?;
                match op {
                    UnaryOp::Plus => self.integer_promotion(value, *span),
                    UnaryOp::Minus => {
                        let value = self.integer_promotion(value, *span)?;
                        let ty = value.ty.clone();
                        Ok(TypedValue::integer(
                            ty.clone(),
                            self.integer_sub_for_type(0, value.to_int()?, &ty, *span, "6.6")?,
                        ))
                    }
                    UnaryOp::LogicalNot => Ok(TypedValue::int((value.to_int()? == 0) as i128)),
                    UnaryOp::BitNot => {
                        let value = self.integer_promotion(value, *span)?;
                        let ty = value.ty.clone();
                        Ok(TypedValue::integer(
                            ty.clone(),
                            self.integer_bitwise_for_type(
                                value.to_int()?,
                                0,
                                &ty,
                                |operand, _| !operand,
                            )?,
                        ))
                    }
                    UnaryOp::AddressOf
                    | UnaryOp::Dereference
                    | UnaryOp::PreIncrement
                    | UnaryOp::PreDecrement => Err(Diagnostic::error(
                        "case label must be an integer constant expression",
                        *span,
                    )),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.eval_typed_integer_constant_expr(lhs, frame, objects)?;
                match op {
                    BinaryOp::LogicalAnd => {
                        if lhs.to_int()? == 0 {
                            return Ok(TypedValue::int(0));
                        }
                        let rhs = self.eval_typed_integer_constant_expr(rhs, frame, objects)?;
                        Ok(TypedValue::int((rhs.to_int()? != 0) as i128))
                    }
                    BinaryOp::LogicalOr => {
                        if lhs.to_int()? != 0 {
                            return Ok(TypedValue::int(1));
                        }
                        let rhs = self.eval_typed_integer_constant_expr(rhs, frame, objects)?;
                        Ok(TypedValue::int((rhs.to_int()? != 0) as i128))
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => {
                        let rhs = self.eval_typed_integer_constant_expr(rhs, frame, objects)?;
                        let (lhs, rhs, common_ty) =
                            self.usual_arithmetic_operands(lhs, rhs, *span)?;
                        let equal = self.integer_compare_for_type(
                            lhs.to_int()?,
                            rhs.to_int()?,
                            &common_ty,
                            |a, b| a == b,
                        )?;
                        Ok(TypedValue::int(
                            (if *op == BinaryOp::Equal {
                                equal
                            } else {
                                !equal
                            }) as i128,
                        ))
                    }
                    BinaryOp::Comma => Err(Diagnostic::error(
                        "case label must be an integer constant expression",
                        *span,
                    )),
                    _ => {
                        let rhs = self.eval_typed_integer_constant_expr(rhs, frame, objects)?;
                        let empty_objects = ObjectFrames::new();
                        self.compute_binary_value(*op, lhs, rhs, *span, &empty_objects)
                    }
                }
            }
            Expr::Cast { ty, expr, span, .. } => {
                if !ty.is_integer() {
                    return Err(Diagnostic::error(
                        "integer constant expression must have integer type",
                        *span,
                    ));
                }
                let value = match expr.as_ref() {
                    // 6.6 permits a floating constant only when it is the
                    // immediate operand of a cast in an integer constant
                    // expression.
                    Expr::Number(
                        literal @ NumberLiteral {
                            value: NumberValue::Floating(_),
                            ..
                        },
                        _,
                    ) => self.eval_number_literal(literal),
                    _ => self.eval_typed_integer_constant_expr(expr, frame, objects)?,
                };
                self.convert_value(value, ty, *span)
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                span,
            } => {
                let selected = self.select_generic_association(
                    control,
                    associations,
                    default.as_deref(),
                    *span,
                    frame,
                    objects,
                )?;
                self.eval_typed_integer_constant_expr(selected, frame, objects)
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                span,
            } => {
                let condition = self.eval_typed_integer_constant_expr(condition, frame, objects)?;
                let result_ty = self.usual_arithmetic_type(
                    &self.expr_type(then_expr, frame, objects)?,
                    &self.expr_type(else_expr, frame, objects)?,
                    *span,
                )?;
                let selected = if condition.to_int()? != 0 {
                    self.eval_typed_integer_constant_expr(then_expr, frame, objects)
                } else {
                    self.eval_typed_integer_constant_expr(else_expr, frame, objects)
                }?;
                self.convert_value(selected, &result_ty, *span)
            }
            Expr::SizeofType {
                ty,
                vla_bounds,
                span,
            } => {
                if vla_bounds.iter().any(|bound| bound.is_some()) {
                    return Err(Diagnostic::error(
                        "case label must be an integer constant expression",
                        *span,
                    ));
                }
                self.eval_sizeof_type(ty, *span)
            }
            Expr::SizeofExpr { expr, span } => {
                let ty = self.expr_type(expr, frame, objects)?;
                if matches!(ty.unqualified(), CType::Array(..))
                    && self.expr_refers_to_vla(expr, frame, objects)
                {
                    return Err(Diagnostic::error(
                        "sizeof a variable length array is not an integer constant expression",
                        *span,
                    ));
                }
                self.eval_sizeof_type(&ty, *span)
            }
            Expr::OffsetOf {
                ty,
                designators,
                span,
            } => self.eval_offsetof_type(ty, designators, *span),
            _ => Err(Diagnostic::error(
                "case label must be an integer constant expression",
                expr.span(),
            )),
        }
    }

    pub(super) fn type_is_complete(&self, ty: &CType) -> bool {
        match ty.unqualified() {
            CType::Void | CType::Function(..) => false,
            CType::Array(inner, len) => *len != 0 && self.type_is_complete(inner),
            CType::Struct(id, _) | CType::Union(id, _) => self
                .program
                .records
                .get(id)
                .is_some_and(|record| record.complete),
            CType::Enum(id, _) => self
                .program
                .enums
                .get(id)
                .is_some_and(|enum_ty| enum_ty.complete),
            _ => true,
        }
    }

    pub(super) fn type_has_const_subobject(&self, ty: &CType) -> bool {
        self.type_has_qualified_subobject(ty, TrackedQualifier::Const)
    }

    pub(super) fn type_has_volatile_subobject(&self, ty: &CType) -> bool {
        self.type_has_qualified_subobject(ty, TrackedQualifier::Volatile)
    }

    fn type_has_qualified_subobject(&self, ty: &CType, qualifier: TrackedQualifier) -> bool {
        fn visit(
            interpreter: &Interpreter<'_>,
            ty: &CType,
            qualifier: TrackedQualifier,
            seen: &mut HashSet<usize>,
        ) -> bool {
            let qualifiers = ty.top_level_qualifiers();
            if match qualifier {
                TrackedQualifier::Const => qualifiers.is_const,
                TrackedQualifier::Volatile => qualifiers.is_volatile,
            } {
                return true;
            }
            match ty.unqualified() {
                CType::Array(inner, _) => visit(interpreter, inner, qualifier, seen),
                CType::Struct(id, _) | CType::Union(id, _) => {
                    if !seen.insert(*id) {
                        return false;
                    }
                    let has_qualifier = interpreter.program.records.get(id).is_some_and(|record| {
                        record
                            .members
                            .iter()
                            .any(|member| visit(interpreter, &member.ty, qualifier, seen))
                    });
                    seen.remove(id);
                    has_qualifier
                }
                _ => false,
            }
        }

        visit(self, ty, qualifier, &mut HashSet::default())
    }

    pub(super) fn type_size_of(&self, ty: &CType) -> Option<usize> {
        match ty.unqualified() {
            CType::Struct(id, _) | CType::Union(id, _) => self
                .program
                .records
                .get(id)
                .filter(|record| record.complete)
                .map(|record| record.size),
            CType::Array(inner, len) => {
                if *len == 0 {
                    None
                } else {
                    self.type_size_of(inner)
                        .and_then(|size| size.checked_mul(*len))
                }
            }
            _ => ty.size_of(),
        }
    }

    pub(super) fn type_align_of(&self, ty: &CType) -> Option<usize> {
        match ty.unqualified() {
            CType::Bool => Some(1),
            CType::Char | CType::SignedChar | CType::UnsignedChar => Some(1),
            CType::Float => Some(4),
            CType::Short | CType::UnsignedShort => Some(2),
            CType::Int | CType::UnsignedInt | CType::Enum(_, _) => Some(4),
            CType::Complex(inner) => self.type_align_of(inner),
            CType::Double => Some(8),
            CType::Long
            | CType::UnsignedLong
            | CType::LongLong
            | CType::UnsignedLongLong
            | CType::Pointer(_)
            | CType::VaList => Some(8),
            CType::LongDouble => Some(HOST_LONG_DOUBLE_ALIGN),
            CType::Struct(id, _) | CType::Union(id, _) => self
                .program
                .records
                .get(id)
                .filter(|record| record.complete)
                .map(|record| record.align),
            CType::Array(inner, _) | CType::Qualified(inner, _) => self.type_align_of(inner),
            CType::Void | CType::Function(..) => None,
        }
    }

    pub(super) fn record_type<'b>(&'b self, ty: &CType) -> Option<&'b RecordType> {
        match ty.unqualified() {
            CType::Struct(id, _) | CType::Union(id, _) => self.program.records.get(id),
            _ => None,
        }
    }

    fn member_access_type(
        &self,
        base_ty: &CType,
        member: &str,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        self.resolve_member_access(base_ty, member, span)
            .map(|resolved| resolved.ty)
    }

    pub(super) fn resolve_member_access(
        &self,
        base_ty: &CType,
        member: &str,
        span: Span,
    ) -> Result<ResolvedMemberAccess, Diagnostic> {
        let record_id = match base_ty.unqualified() {
            CType::Struct(id, _) | CType::Union(id, _) => *id,
            _ => usize::MAX,
        };
        let cache_key = (record_id, base_ty.top_level_qualifiers());
        let cache = self.member_access_cache.borrow();
        if let Some(cached) = cache
            .get(&cache_key)
            .and_then(|members| members.get(member))
        {
            return Ok(cached.clone());
        }
        drop(cache);
        let chain = self
            .resolve_visible_member_chain(base_ty, member)
            .ok_or_else(|| {
                Diagnostic::error(format!("{} has no member named {}", base_ty, member), span)
            })?;
        let mut qualifiers = base_ty.top_level_qualifiers();
        for entry in &chain {
            qualifiers = qualifiers.union(entry.ty.top_level_qualifiers());
        }
        let last = chain.last().expect("resolved chain is non-empty");
        let resolved = ResolvedMemberAccess {
            path: Rc::new(
                chain
                    .iter()
                    .map(|entry| entry.storage_name.clone())
                    .collect(),
            ),
            ty: Self::qualify_member_result_type(&last.ty, qualifiers),
            bit_field_width: last.bit_width,
        };
        let result = resolved.clone();
        self.member_access_cache
            .borrow_mut()
            .entry(cache_key)
            .or_default()
            .insert(member.to_owned(), resolved);
        Ok(result)
    }

    pub(super) fn resolve_visible_member_chain<'b>(
        &'b self,
        ty: &CType,
        member: &str,
    ) -> Option<Vec<&'b RecordMember>> {
        let record = self.record_type(ty)?;
        for candidate in &record.members {
            if candidate.name.as_deref() == Some(member) {
                return Some(vec![candidate]);
            }
        }
        for candidate in &record.members {
            if candidate.name.is_none()
                && candidate.bit_width.is_none()
                && matches!(
                    candidate.ty.unqualified(),
                    CType::Struct(_, _) | CType::Union(_, _)
                )
                && let Some(mut tail) = self.resolve_visible_member_chain(&candidate.ty, member)
            {
                let mut chain = vec![candidate];
                chain.append(&mut tail);
                return Some(chain);
            }
        }
        None
    }

    pub(super) fn direct_member_by_storage_name<'b>(
        &'b self,
        ty: &CType,
        storage_name: &str,
    ) -> Option<&'b RecordMember> {
        self.record_type(ty)?
            .members
            .iter()
            .find(|member| member.storage_name == storage_name)
    }

    pub(super) fn qualified_member_type(&self, base_ty: &CType, member: &RecordMember) -> CType {
        let qualifiers = base_ty
            .top_level_qualifiers()
            .union(member.ty.top_level_qualifiers());
        Self::qualify_member_result_type(&member.ty, qualifiers)
    }

    fn qualify_member_result_type(ty: &CType, qualifiers: TypeQualifiers) -> CType {
        match ty.unqualified() {
            // C11 associates an array's qualifiers with its element type.
            CType::Array(inner, len) => {
                CType::array_of(CType::qualified((**inner).clone(), qualifiers), *len)
            }
            _ => CType::qualified(ty.unqualified().clone(), qualifiers),
        }
    }
}
