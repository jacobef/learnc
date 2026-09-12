//! Expression evaluation, value categories, and lvalue operations.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn reject_indeterminate_pointer_use(
        &self,
        value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if value.indeterminate && value.ty.is_pointer() {
            Err(Diagnostic::ub(
                "use of an indeterminate pointer value",
                span,
                Some("6.2.4"),
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn eval_rvalue(
        &mut self,
        expr: &Expr,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let category = self.eval(expr, frame, objects)?;
        self.value_category_to_rvalue(category, expr, objects)
    }

    fn value_category_to_rvalue(
        &mut self,
        category: ValueCategory,
        expr: &Expr,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        match category {
            ValueCategory::RValue(value) => {
                if value.ty.is_function() {
                    Ok(TypedValue {
                        ty: CType::pointer_to(value.ty.clone()),
                        data: value.data,
                        restrict_source: None,
                        indeterminate: value.indeterminate,
                        missing_return: value.missing_return,
                    })
                } else {
                    Ok(value)
                }
            }
            ValueCategory::LValue(mut lvalue) => {
                if let CType::Array(inner, _) = lvalue.ty.unqualified() {
                    if self
                        .lookup_object(objects, lvalue.object)
                        .is_some_and(|object| object.register_object)
                    {
                        return Err(Diagnostic::ub(
                            "array-to-pointer conversion of an array declared with register storage class",
                            expr.span(),
                            Some("6.3.2.1p3"),
                        ));
                    }
                    let inner_ty = (**inner).clone();
                    self.normalize_lvalue_base_offset(&mut lvalue, objects);
                    self.rebase_array_lvalue(&mut lvalue, objects, expr.span())?;
                    if let Some(object) = self.lookup_object_mut(objects, lvalue.object) {
                        object.address_taken = true;
                    }
                    let designated_root_ty = lvalue.designated_root_ty.clone().or_else(|| {
                        lvalue
                            .member_path
                            .is_empty()
                            .then(|| Rc::new(lvalue.ty.clone()))
                    });
                    Ok(TypedValue {
                        ty: CType::pointer_to(inner_ty),
                        data: ValueData::Pointer(Rc::new(PointerValue {
                            object: Some(lvalue.object),
                            base_offset: lvalue.base_offset,
                            offset: lvalue.offset,
                            member_path: lvalue.member_path,
                            designated_root_ty,
                            byte_offset_override: lvalue.byte_offset_override,
                            arithmetic_domain_start: lvalue.arithmetic_domain_start,
                            object_representation_domain: lvalue.object_representation_domain,
                        })),
                        restrict_source: lvalue.restrict_source,
                        indeterminate: false,
                        missing_return: false,
                    })
                } else {
                    let bit_field_width = lvalue.bit_field_width;
                    let value = self.load_lvalue(lvalue, expr.span(), objects)?;
                    if let Some(width) = bit_field_width {
                        let promoted = self.promoted_bit_field_type(&value.ty, width);
                        return self.convert_value(value, &promoted, expr.span());
                    }
                    Ok(value)
                }
            }
        }
    }

    pub(super) fn eval(
        &mut self,
        expr: &Expr,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let expr_key = expr as *const Expr as usize;
        match expr {
            Expr::Number(literal, _) => {
                Ok(ValueCategory::RValue(self.eval_number_literal(literal)))
            }
            Expr::CharLiteral(value, _) => Ok(ValueCategory::RValue(TypedValue::integer(
                CType::Int,
                *value as i128,
            ))),
            Expr::WideCharLiteral(value, _) => Ok(ValueCategory::RValue(TypedValue::integer(
                self.wchar_type(),
                *value as i128,
            ))),
            Expr::Utf16CharLiteral(value, _) => Ok(ValueCategory::RValue(TypedValue::integer(
                CType::UnsignedShort,
                *value as i128,
            ))),
            Expr::Utf32CharLiteral(value, _) => Ok(ValueCategory::RValue(TypedValue::integer(
                CType::UnsignedInt,
                *value as i128,
            ))),
            Expr::StringLiteral(text, _) => {
                let pointer = self.intern_string_literal(text);
                let object = pointer.object.expect("string literal has an object");
                let ty = self.lookup_object(objects, object).unwrap().ty.clone();
                Ok(ValueCategory::LValue(LValue {
                    object,
                    ty,
                    base_offset: pointer.base_offset,
                    offset: pointer.offset,
                    member_path: pointer.member_path,
                    designated_root_ty: pointer.designated_root_ty,
                    byte_offset_override: pointer.byte_offset_override,
                    arithmetic_domain_start: pointer.arithmetic_domain_start,
                    object_representation_domain: pointer.object_representation_domain,
                    bit_field_width: None,
                    restrict_source: None,
                }))
            }
            Expr::WideStringLiteral(text, span) => {
                let pointer = self.intern_wide_string_literal(text, *span)?;
                let object = pointer.object.expect("wide string literal has an object");
                let ty = self.lookup_object(objects, object).unwrap().ty.clone();
                Ok(ValueCategory::LValue(LValue {
                    object,
                    ty,
                    base_offset: pointer.base_offset,
                    offset: pointer.offset,
                    member_path: pointer.member_path,
                    designated_root_ty: pointer.designated_root_ty,
                    byte_offset_override: pointer.byte_offset_override,
                    arithmetic_domain_start: pointer.arithmetic_domain_start,
                    object_representation_domain: pointer.object_representation_domain,
                    bit_field_width: None,
                    restrict_source: None,
                }))
            }
            Expr::Utf16StringLiteral(text, span) => {
                self.eval_unicode_string_literal(text, true, *span, objects)
            }
            Expr::Utf32StringLiteral(text, span) => {
                self.eval_unicode_string_literal(text, false, *span, objects)
            }
            Expr::Variable(name, span) => self.eval_variable(name, *span, expr_key, frame, objects),
            Expr::Unary { op, expr, span } => self.eval_unary(*op, expr, *span, frame, objects),
            Expr::Postfix { op, expr, span } => self.eval_postfix(*op, expr, *span, frame, objects),
            Expr::Binary { op, lhs, rhs, span } => {
                self.eval_binary(*op, lhs, rhs, *span, frame, objects)
            }
            Expr::Subscript { base, index, span } => {
                self.eval_subscript(base, index, *span, frame, objects)
            }
            Expr::Assign { lhs, rhs, span } => {
                let lvalue = match self.eval(lhs, frame, objects)? {
                    ValueCategory::LValue(lvalue) => lvalue,
                    ValueCategory::RValue(_) => {
                        return Err(Diagnostic::error(
                            "the left side of = is not a stored object that can be changed",
                            *span,
                        ));
                    }
                };
                if matches!(lvalue.ty.unqualified(), CType::Array(..)) {
                    return Err(Diagnostic::error(
                        "arrays cannot be assigned; assign their elements individually",
                        *span,
                    ));
                }
                if self.type_has_const_subobject(&lvalue.ty) {
                    return Err(Diagnostic::error(
                        "the left side of = is const and cannot be changed",
                        *span,
                    ));
                }
                if matches!(lvalue.ty.unqualified(), CType::Void) {
                    return Err(Diagnostic::error(
                        "the left side of = has type void and cannot store a value",
                        *span,
                    ));
                }
                let assignment_target = self.lvalue_access_region(&lvalue, objects, *span)?;
                self.push_assignment_target(assignment_target, AssignmentTargetKind::Simple);
                let rhs_category = self.eval(rhs, frame, objects)?;
                if let ValueCategory::LValue(source) = &rhs_category
                    && !matches!(
                        source.ty.unqualified(),
                        CType::Array(..) | CType::Function(..)
                    )
                {
                    self.validate_simple_assignment_overlap(&lvalue, source, objects, *span)?;
                }
                let rhs_value = self.value_category_to_rvalue(rhs_category, rhs, objects)?;
                self.pop_assignment_target();
                self.validate_restricted_pointer_assignment(&lvalue, &rhs_value, *span)?;
                let converted = self.convert_value_in_context(
                    rhs,
                    rhs_value,
                    &lvalue.ty,
                    rhs.span(),
                    frame,
                    objects,
                )?;
                self.store_lvalue(objects, &lvalue, converted.clone(), *span)?;
                Ok(ValueCategory::RValue(converted))
            }
            Expr::CompoundAssign { op, lhs, rhs, span } => {
                let lvalue = match self.eval(lhs, frame, objects)? {
                    ValueCategory::LValue(lvalue) => lvalue,
                    ValueCategory::RValue(_) => {
                        return Err(Diagnostic::error(
                            "the left side of this assignment is not a stored object that can be changed",
                            *span,
                        ));
                    }
                };
                if matches!(lvalue.ty.unqualified(), CType::Array(..)) {
                    return Err(Diagnostic::error(
                        "arrays cannot be assigned; assign their elements individually",
                        *span,
                    ));
                }
                if lvalue.ty.is_const_qualified() {
                    return Err(Diagnostic::error(
                        "the left side of this assignment is const and cannot be changed",
                        *span,
                    ));
                }
                if matches!(lvalue.ty.unqualified(), CType::Void) {
                    return Err(Diagnostic::error(
                        "the left side of this assignment has type void and cannot store a value",
                        *span,
                    ));
                }
                let assignment_target = self.lvalue_access_region(&lvalue, objects, *span)?;
                self.push_assignment_target(assignment_target, AssignmentTargetKind::Compound);
                let old_value = self.load_lvalue(lvalue.clone(), lhs.span(), objects)?;
                let rhs_value = self.eval_rvalue(rhs, frame, objects)?;
                self.pop_assignment_target();
                let computed =
                    self.compute_binary_value(*op, old_value, rhs_value, *span, objects)?;
                let binary = Expr::Binary {
                    op: *op,
                    lhs: lhs.clone(),
                    rhs: rhs.clone(),
                    span: *span,
                };
                let stored = self.convert_value_in_context(
                    &binary, computed, &lvalue.ty, *span, frame, objects,
                )?;
                self.store_lvalue(objects, &lvalue, stored.clone(), *span)?;
                Ok(ValueCategory::RValue(stored))
            }
            Expr::SizeofType {
                ty,
                vla_bounds,
                span,
            } => Ok(ValueCategory::RValue(
                self.eval_sizeof_type_name(ty, vla_bounds, *span, frame, objects)?,
            )),
            Expr::SizeofExpr { expr, span } => {
                if self.expr_designates_bit_field(expr, frame, objects)? {
                    return Err(Diagnostic::error(
                        "sizeof cannot be applied to a bit-field",
                        *span,
                    ));
                }
                let ty = self.expr_type(expr, frame, objects)?;
                if matches!(ty.unqualified(), CType::Array(..))
                    && self.expr_refers_to_vla(expr, frame, objects)
                {
                    // Unlike every other sizeof expression, a VLA-typed operand is
                    // evaluated.  Keep its value category intact: evaluation is
                    // required for side effects, but an array lvalue must not be
                    // converted to an rvalue here.
                    let _ = self.eval(expr, frame, objects)?;
                }
                Ok(ValueCategory::RValue(self.eval_sizeof_type(&ty, *span)?))
            }
            Expr::OffsetOf {
                ty,
                designators,
                span,
            } => Ok(ValueCategory::RValue(self.eval_offsetof_type(
                ty,
                designators,
                *span,
            )?)),
            Expr::Cast {
                ty,
                vla_bounds,
                expr,
                span,
            } => {
                let resolved_ty = self.resolve_decl_type(ty, vla_bounds, frame, objects, *span)?;
                let value = self.eval_rvalue(expr, frame, objects)?;
                if matches!(resolved_ty.unqualified(), CType::Void) {
                    self.reject_missing_return_value(&value, *span)?;
                    self.reject_indeterminate_pointer_use(&value, *span)?;
                    return Ok(ValueCategory::RValue(TypedValue::void()));
                }
                self.check_pointer_cast_ub(&value, &resolved_ty, *span, objects)?;
                let converted = self.convert_value(value, &resolved_ty, *span)?;
                Ok(ValueCategory::RValue(
                    self.specialize_dynamic_raw_storage_pointer(converted, &resolved_ty, objects),
                ))
            }
            Expr::CompoundLiteral {
                ty,
                vla_bounds,
                initializer,
                span,
            } => self.eval_compound_literal(ty, vla_bounds, initializer, *span, frame, objects),
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
                self.eval(selected, frame, objects)
            }
            Expr::VaArg { ap, ty, span } => Ok(ValueCategory::RValue(
                self.eval_va_arg(ap, ty, *span, frame, objects)?,
            )),
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                span,
            } => {
                let sequencing = self.sequencing_snapshot();
                let cond = self.eval_rvalue(condition, frame, objects)?;
                let truthy = self.scalar_truthy(&cond, condition.span())?;
                let condition_footprint = self.finish_sequenced_operand(sequencing);
                let result_ty =
                    self.conditional_result_type(then_expr, else_expr, *span, frame, objects)?;
                let result = if truthy {
                    let value = self.eval_rvalue(then_expr, frame, objects)?;
                    ValueCategory::RValue(self.convert_value_in_context(
                        then_expr,
                        value,
                        &result_ty,
                        then_expr.span(),
                        frame,
                        objects,
                    )?)
                } else {
                    let value = self.eval_rvalue(else_expr, frame, objects)?;
                    ValueCategory::RValue(self.convert_value_in_context(
                        else_expr,
                        value,
                        &result_ty,
                        else_expr.span(),
                        frame,
                        objects,
                    )?)
                };
                self.merge_sequenced_footprint(condition_footprint);
                Ok(result)
            }
            Expr::Call {
                callee,
                args,
                declared_callee_type,
                span,
            } => self.eval_call(
                callee,
                args,
                declared_callee_type.as_ref(),
                *span,
                frame,
                objects,
            ),
            Expr::Member { base, member, span } => {
                self.eval_member(base, member, *span, frame, objects)
            }
        }
    }

    pub(super) fn eval_variable(
        &mut self,
        name: &str,
        span: Span,
        expr_key: usize,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        if let Some(cached) = self.variable_cache.get(&expr_key) {
            let valid = match cached.frame_id {
                None => true,
                Some(frame_id) if frame_id == frame.id => match &cached.value {
                    ValueCategory::LValue(lvalue) => self
                        .lookup_object(objects, lvalue.object)
                        .is_some_and(|object| object.alive),
                    ValueCategory::RValue(_) => false,
                },
                Some(_) => false,
            };
            if valid {
                return Ok(cached.value.clone());
            }
        }

        let (value, frame_local) = if let Some(object) = frame.bindings.get(name).copied() {
            let ty = self.lookup_object(objects, object).unwrap().ty.clone();
            (
                ValueCategory::LValue(LValue {
                    object,
                    ty,
                    base_offset: 0,
                    offset: 0,
                    member_path: Rc::new(Vec::new()),
                    designated_root_ty: None,
                    byte_offset_override: None,
                    arithmetic_domain_start: None,
                    object_representation_domain: None,
                    bit_field_width: None,
                    restrict_source: None,
                }),
                true,
            )
        } else if let Some(decl) = frame.object_decls.get(name) {
            let Some(object) = self.lookup_global_binding(name, span.file) else {
                return Err(self.missing_definition_use_diag(name, span));
            };
            (
                ValueCategory::LValue(LValue {
                    object,
                    ty: decl.ty.clone(),
                    base_offset: 0,
                    offset: 0,
                    member_path: Rc::new(Vec::new()),
                    designated_root_ty: None,
                    byte_offset_override: None,
                    arithmetic_domain_start: None,
                    object_representation_domain: None,
                    bit_field_width: None,
                    restrict_source: None,
                }),
                false,
            )
        } else if let Some(function_decl) = frame.function_decls.get(name) {
            let data = if let Some(function) = self.lookup_function(name, span.file) {
                ValueData::Function(function_symbol(function).into())
            } else if Self::is_host_library_function(name) {
                ValueData::Function(name.into())
            } else {
                return Err(self.missing_definition_use_diag(name, span));
            };
            (
                ValueCategory::RValue(TypedValue::from_data(
                    self.function_declaration_type(function_decl),
                    data,
                )),
                false,
            )
        } else if let Some(object) = self.lookup_global_binding(name, span.file) {
            let ty = self.lookup_object(objects, object).unwrap().ty.clone();
            (
                ValueCategory::LValue(LValue {
                    object,
                    ty,
                    base_offset: 0,
                    offset: 0,
                    member_path: Rc::new(Vec::new()),
                    designated_root_ty: None,
                    byte_offset_override: None,
                    arithmetic_domain_start: None,
                    object_representation_domain: None,
                    bit_field_width: None,
                    restrict_source: None,
                }),
                false,
            )
        } else if let Some(function) = self.lookup_function(name, span.file) {
            (
                ValueCategory::RValue(self.function_designator_value(function)),
                false,
            )
        } else if let Some(function_decl) = self.lookup_function_declaration(name, span.file) {
            if !Self::is_host_library_function(name) {
                return Err(self.missing_definition_use_diag(name, span));
            }
            (
                ValueCategory::RValue(TypedValue::function(
                    self.function_declaration_type(function_decl),
                    name,
                )),
                false,
            )
        } else if self.lookup_global_declaration(name, span.file).is_some() {
            return Err(self.missing_definition_use_diag(name, span));
        } else if let Some(ty) = self.builtin_function_type(name) {
            (ValueCategory::RValue(TypedValue::function(ty, name)), false)
        } else if let Some(value) = self
            .program
            .enum_constants
            .get(&(span.file, name.to_owned()))
        {
            (ValueCategory::RValue(TypedValue::int(*value)), false)
        } else {
            return Err(Diagnostic::error(
                format!("use of undeclared identifier {name}"),
                span,
            ));
        };

        self.variable_cache.insert(
            expr_key,
            VariableCacheEntry {
                frame_id: frame_local.then_some(frame.id),
                value: value.clone(),
            },
        );
        Ok(value)
    }

    pub(super) fn eval_unary(
        &mut self,
        op: UnaryOp,
        expr: &Expr,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        match op {
            UnaryOp::AddressOf => {
                if let Expr::Subscript { base, index, .. } = expr {
                    return self.eval_subscript_address(base, index, span, frame, objects);
                }
                if let Expr::Unary {
                    op: UnaryOp::Dereference,
                    expr: pointer_expr,
                    ..
                } = expr
                {
                    let value = self.eval_rvalue(pointer_expr, frame, objects)?;
                    self.reject_missing_return_value(&value, span)?;
                    self.reject_indeterminate_pointer_use(&value, span)?;
                    if value.ty.element_type().is_none() {
                        return Err(Diagnostic::error(
                            format!(
                                "the * operator requires a pointer, but this expression has type {}",
                                value.ty
                            ),
                            span,
                        ));
                    }
                    return Ok(ValueCategory::RValue(value));
                }
                match self.eval(expr, frame, objects)? {
                    ValueCategory::RValue(value) if value.ty.is_function() => {
                        Ok(ValueCategory::RValue(TypedValue {
                            ty: CType::pointer_to(value.ty),
                            data: value.data,
                            restrict_source: None,
                            indeterminate: value.indeterminate,
                            missing_return: value.missing_return,
                        }))
                    }
                    ValueCategory::LValue(lvalue) => {
                        let mut lvalue = lvalue;
                        self.normalize_lvalue_base_offset(&mut lvalue, objects);
                        if lvalue.bit_field_width.is_some() {
                            return Err(Diagnostic::error(
                                "cannot take the address of a bit-field",
                                span,
                            ));
                        }
                        if self
                            .lookup_object(objects, lvalue.object)
                            .is_some_and(|object| object.register_object)
                        {
                            return Err(Diagnostic::error(
                                "cannot take the address of a register object",
                                span,
                            ));
                        }
                        if let Some(object) = self.lookup_object_mut(objects, lvalue.object) {
                            object.address_taken = true;
                        }
                        Ok(ValueCategory::RValue(TypedValue {
                            ty: CType::pointer_to(lvalue.ty),
                            data: ValueData::Pointer(Rc::new(PointerValue {
                                object: Some(lvalue.object),
                                base_offset: lvalue.base_offset,
                                offset: lvalue.offset,
                                member_path: lvalue.member_path.clone(),
                                designated_root_ty: lvalue.designated_root_ty.clone(),
                                byte_offset_override: lvalue.byte_offset_override,
                                arithmetic_domain_start: lvalue.arithmetic_domain_start,
                                object_representation_domain: lvalue.object_representation_domain,
                            })),
                            restrict_source: lvalue.restrict_source.clone(),
                            indeterminate: false,
                            missing_return: false,
                        }))
                    }
                    ValueCategory::RValue(_) => Err(Diagnostic::error(
                        "the & operator requires an object or function; a temporary value does not have an address",
                        span,
                    )),
                }
            }
            UnaryOp::Dereference => {
                let value = self.eval_rvalue(expr, frame, objects)?;
                self.dereference_value(value, span, objects)
            }
            UnaryOp::Plus => {
                let value = self.eval_rvalue(expr, frame, objects)?;
                self.reject_missing_return_value(&value, span)?;
                if value.ty.is_floating() || value.ty.is_complex() {
                    Ok(ValueCategory::RValue(value))
                } else {
                    let value = self.promote_integer_expr_value(expr, value, frame, objects)?;
                    Ok(ValueCategory::RValue(value))
                }
            }
            UnaryOp::Minus => {
                let value = self.eval_rvalue(expr, frame, objects)?;
                self.reject_missing_return_value(&value, span)?;
                if value.ty.is_complex() {
                    let complex = value.to_complex()?;
                    Ok(ValueCategory::RValue(TypedValue::complex(
                        value.ty.clone(),
                        -complex.real,
                        -complex.imag,
                    )))
                } else if value.ty.is_floating() {
                    Ok(ValueCategory::RValue(TypedValue::floating(
                        value.ty.clone(),
                        -value.to_float()?,
                    )))
                } else {
                    let value = self.promote_integer_expr_value(expr, value, frame, objects)?;
                    let int = value.to_int()?;
                    let ty = value.ty.clone();
                    let negated = self.integer_sub_for_type(0, int, &ty, span, "6.5.3.3")?;
                    Ok(ValueCategory::RValue(TypedValue::integer(ty, negated)))
                }
            }
            UnaryOp::LogicalNot => {
                let value = self.eval_rvalue(expr, frame, objects)?;
                Ok(ValueCategory::RValue(TypedValue::int(
                    (!self.scalar_truthy(&value, span)?) as i128,
                )))
            }
            UnaryOp::BitNot => {
                let value = self.eval_rvalue(expr, frame, objects)?;
                self.reject_missing_return_value(&value, span)?;
                let value = self.promote_integer_expr_value(expr, value, frame, objects)?;
                let ty = value.ty.clone();
                let int = value.to_int()?;
                let result = self.integer_bitwise_for_type(int, 0, &ty, |operand, _| !operand)?;
                Ok(ValueCategory::RValue(TypedValue::integer(ty, result)))
            }
            UnaryOp::PreIncrement => self.eval_increment(expr, span, frame, objects, 1, true),
            UnaryOp::PreDecrement => self.eval_increment(expr, span, frame, objects, -1, true),
        }
    }

    fn eval_postfix(
        &mut self,
        op: PostfixOp,
        expr: &Expr,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        match op {
            PostfixOp::PostIncrement => self.eval_increment(expr, span, frame, objects, 1, false),
            PostfixOp::PostDecrement => self.eval_increment(expr, span, frame, objects, -1, false),
        }
    }

    fn dereference_value(
        &self,
        value: TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        self.reject_missing_return_value(&value, span)?;
        let ty = value
            .ty
            .element_type()
            .cloned()
            .filter(|_| value.ty.is_pointer())
            .ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "the * operator requires a pointer, but this expression has type {}",
                        value.ty
                    ),
                    span,
                )
            })?;
        if ty.is_function() {
            return match value.data {
                ValueData::Function(name) => Ok(ValueCategory::RValue(TypedValue {
                    ty,
                    data: ValueData::Function(name),
                    restrict_source: None,
                    indeterminate: value.indeterminate,
                    missing_return: value.missing_return,
                })),
                ValueData::Pointer(pointer) if pointer.is_null() => Err(Diagnostic::ub(
                    "dereference of a null pointer",
                    span,
                    Some("6.5.3.2"),
                )),
                _ => Err(Diagnostic::error(
                    "function pointer does not name a supported function",
                    span,
                )),
            };
        }
        let pointer = value.as_pointer(span)?;
        let object = pointer.object.ok_or_else(|| {
            Diagnostic::ub("dereference of a null pointer", span, Some("6.5.3.2"))
        })?;
        let object_state = self.lookup_object(objects, object).ok_or_else(|| {
            Diagnostic::ub(
                "dereference of a pointer to an object whose lifetime has ended",
                span,
                Some("6.5.3.2"),
            )
        })?;
        if !object_state.alive {
            return Err(Diagnostic::ub(
                "dereference of a pointer to an object whose lifetime has ended",
                span,
                Some("6.5.3.2"),
            ));
        }
        let target_is_record = matches!(ty.unqualified(), CType::Struct(_, _) | CType::Union(_, _));
        let root_matches_target = self
            .pointer_root_type(&pointer, objects)
            .is_some_and(|root| match root.unqualified() {
                CType::Array(inner, _) => self.cross_unit_tagged_type_compatible(inner, &ty),
                _ => self.cross_unit_tagged_type_compatible(root, &ty),
            });
        let leaf_matches_target = self
            .pointer_root_type(&pointer, objects)
            .and_then(|root| self.storage_path_type(root, &pointer.member_path))
            .is_some_and(|leaf| self.cross_unit_tagged_type_compatible(&leaf, &ty));
        let rebase_to_containing_record = target_is_record
            && !root_matches_target
            && !leaf_matches_target
            && self.pointer_targets_initial_member_chain(&pointer, &ty, objects);
        let byte_offset_override = if rebase_to_containing_record {
            Some(
                self.pointer_byte_offset(&pointer, &ty, objects)
                    .ok_or_else(|| {
                        Diagnostic::ub(
                            "pointer does not point into a live supported object",
                            span,
                            Some("6.5.3.2"),
                        )
                    })?,
            )
        } else if object_state.storage_duration == StorageDuration::Dynamic {
            let start = self
                .pointer_byte_offset(&pointer, &ty, objects)
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "pointer does not point into a live supported object",
                        span,
                        Some("6.5.3.2"),
                    )
                })?;
            if start >= object_state.byte_size {
                return Err(Diagnostic::ub(
                    "pointer is not valid to dereference",
                    span,
                    Some("6.5.3.2"),
                ));
            }
            self.validate_designated_subobject_dereference(&pointer, &ty, start, span)?;
            // Dereferencing a pointer only produces an lvalue; it does not read or
            // write the complete pointed-to type. Raw allocated storage can therefore
            // designate a member that fits even when another member makes the enclosing
            // union larger than the allocation. The eventual lvalue access performs the
            // full byte-range check.
            Some(start)
        } else if let Some(start) = pointer.byte_offset_override {
            self.validate_designated_subobject_dereference(&pointer, &ty, start, span)?;
            let size = self.type_size_of(&ty).ok_or_else(|| {
                Diagnostic::ub(
                    "pointer does not point into a live supported object",
                    span,
                    Some("6.5.3.2"),
                )
            })?;
            let end = start.checked_add(size).ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
            })?;
            if end > object_state.byte_size {
                return Err(Diagnostic::ub(
                    "pointer is not valid to dereference",
                    span,
                    Some("6.5.3.2"),
                ));
            }
            Some(start)
        } else {
            let max = self
                .object_pointer_limit(objects, &pointer, &ty)
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "pointer does not point into a live supported object",
                        span,
                        Some("6.5.3.2"),
                    )
                })?;
            if pointer.offset < 0 || pointer.offset >= max {
                return Err(Diagnostic::ub(
                    "pointer is not valid to dereference",
                    span,
                    Some("6.5.3.2"),
                ));
            }
            None
        };
        let member_path = if target_is_record
            && (root_matches_target || rebase_to_containing_record)
            && self.pointer_targets_initial_member_chain(&pointer, &ty, objects)
        {
            Rc::new(Vec::new())
        } else {
            pointer.member_path.clone()
        };
        let designated_root_ty = if rebase_to_containing_record {
            Some(Rc::new(ty.clone()))
        } else if object_state.storage_duration == StorageDuration::Dynamic {
            pointer
                .designated_root_ty
                .clone()
                .or_else(|| Some(Rc::new(ty.clone())))
        } else {
            pointer.designated_root_ty.clone()
        };
        Ok(ValueCategory::LValue(LValue {
            object,
            ty,
            base_offset: if rebase_to_containing_record {
                0
            } else {
                pointer.base_offset
            },
            offset: if rebase_to_containing_record {
                0
            } else {
                pointer.offset
            },
            member_path,
            designated_root_ty,
            byte_offset_override,
            arithmetic_domain_start: if rebase_to_containing_record {
                byte_offset_override
            } else {
                pointer.arithmetic_domain_start
            },
            object_representation_domain: pointer.object_representation_domain,
            bit_field_width: None,
            restrict_source: value.restrict_source,
        }))
    }

    fn validate_designated_subobject_dereference(
        &self,
        pointer: &PointerValue,
        target_ty: &CType,
        start: usize,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let target_size = self.type_size_of(target_ty).ok_or_else(|| {
            Diagnostic::ub(
                "pointer does not point into a live supported object",
                span,
                Some("6.5.3.2"),
            )
        })?;
        if pointer
            .object_representation_domain
            .and_then(ObjectRepresentationDomain::active)
            .is_some_and(|domain| {
                start < domain.start
                    || start
                        .checked_add(target_size)
                        .is_none_or(|end| end > domain.one_past_end)
            })
        {
            return Err(Diagnostic::ub(
                "pointer is not valid to dereference outside the converted object's representation",
                span,
                Some("6.3.2.3"),
            ));
        }
        let (Some(domain_start), Some(domain_ty)) = (
            pointer.arithmetic_domain_start,
            pointer.designated_root_ty.as_deref(),
        ) else {
            return Ok(());
        };
        if !matches!(domain_ty.unqualified(), CType::Array(_, len) if *len != 0) {
            return Ok(());
        }
        let domain_size = self.type_size_of(domain_ty).ok_or_else(|| {
            Diagnostic::ub(
                "pointer does not point into a live supported object",
                span,
                Some("6.5.3.2"),
            )
        })?;
        let domain_end = domain_start.checked_add(domain_size).ok_or_else(|| {
            Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
        })?;
        if start < domain_start
            || start
                .checked_add(target_size)
                .is_none_or(|end| end > domain_end)
        {
            return Err(Diagnostic::ub(
                "pointer is not valid to dereference",
                span,
                Some("6.5.3.2"),
            ));
        }
        Ok(())
    }

    fn eval_subscript(
        &mut self,
        base: &Expr,
        index: &Expr,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let base_ty = self.value_expr_type(base, frame, objects)?;
        let index_ty = self.value_expr_type(index, frame, objects)?;
        if base_ty.is_integer() && index_ty.is_pointer() {
            return self.eval_subscript(index, base, span, frame, objects);
        }
        let base_span = base.span();
        let base = self.eval(base, frame, objects)?;
        let index_value = self.eval_rvalue(index, frame, objects)?;
        if !index_value.ty.is_integer() {
            return Err(Diagnostic::error(
                "array subscript must have integer type",
                index.span(),
            ));
        }
        let index = self
            .integer_promotion(index_value, index.span())?
            .to_int()?;

        match base {
            ValueCategory::LValue(mut lvalue)
                if matches!(lvalue.ty.unqualified(), CType::Array(_, _)) =>
            {
                if self
                    .lookup_object(objects, lvalue.object)
                    .is_some_and(|object| object.register_object)
                {
                    return Err(Diagnostic::ub(
                        "array-to-pointer conversion of an array declared with register storage class",
                        base_span,
                        Some("6.3.2.1p3"),
                    ));
                }
                let CType::Array(inner, _) = lvalue.ty.unqualified() else {
                    unreachable!();
                };
                let element_ty = (**inner).clone();
                self.normalize_lvalue_base_offset(&mut lvalue, objects);
                self.rebase_array_lvalue(&mut lvalue, objects, span)?;
                let pointer = PointerValue {
                    object: Some(lvalue.object),
                    base_offset: lvalue.base_offset,
                    offset: lvalue.offset,
                    member_path: lvalue.member_path,
                    designated_root_ty: lvalue.designated_root_ty,
                    byte_offset_override: lvalue.byte_offset_override,
                    arithmetic_domain_start: lvalue.arithmetic_domain_start,
                    object_representation_domain: lvalue.object_representation_domain,
                };
                let pointer =
                    self.checked_pointer_offset(pointer, index, &element_ty, objects, span)?;
                self.dereference_value(
                    TypedValue {
                        ty: CType::pointer_to(element_ty),
                        data: ValueData::Pointer(Rc::new(pointer)),
                        restrict_source: lvalue.restrict_source,
                        indeterminate: false,
                        missing_return: false,
                    },
                    span,
                    objects,
                )
            }
            ValueCategory::LValue(lvalue) => {
                let value = self.load_lvalue(lvalue, base_span, objects)?;
                self.subscript_pointer_value(value, index, span, objects)
            }
            ValueCategory::RValue(value) => {
                self.subscript_pointer_value(value, index, span, objects)
            }
        }
    }

    fn eval_subscript_address(
        &mut self,
        base: &Expr,
        index: &Expr,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let base_ty = self.value_expr_type(base, frame, objects)?;
        let index_ty = self.value_expr_type(index, frame, objects)?;
        if base_ty.is_integer() && index_ty.is_pointer() {
            return self.eval_subscript_address(index, base, span, frame, objects);
        }
        let base_span = base.span();
        let base = self.eval(base, frame, objects)?;
        let index_value = self.eval_rvalue(index, frame, objects)?;
        if !index_value.ty.is_integer() {
            return Err(Diagnostic::error(
                "array subscript must have integer type",
                index.span(),
            ));
        }
        let index = self
            .integer_promotion(index_value, index.span())?
            .to_int()?;
        let (pointer, element_ty, restrict_source) = match base {
            ValueCategory::LValue(mut lvalue)
                if matches!(lvalue.ty.unqualified(), CType::Array(_, _)) =>
            {
                if self
                    .lookup_object(objects, lvalue.object)
                    .is_some_and(|object| object.register_object)
                {
                    return Err(Diagnostic::ub(
                        "array-to-pointer conversion of an array declared with register storage class",
                        base_span,
                        Some("6.3.2.1p3"),
                    ));
                }
                let CType::Array(inner, _) = lvalue.ty.unqualified() else {
                    unreachable!();
                };
                let element_ty = (**inner).clone();
                self.normalize_lvalue_base_offset(&mut lvalue, objects);
                self.rebase_array_lvalue(&mut lvalue, objects, span)?;
                if let Some(object) = self.lookup_object_mut(objects, lvalue.object) {
                    object.address_taken = true;
                }
                let pointer = self.checked_pointer_offset(
                    PointerValue {
                        object: Some(lvalue.object),
                        base_offset: lvalue.base_offset,
                        offset: lvalue.offset,
                        member_path: lvalue.member_path,
                        designated_root_ty: lvalue.designated_root_ty,
                        byte_offset_override: lvalue.byte_offset_override,
                        arithmetic_domain_start: lvalue.arithmetic_domain_start,
                        object_representation_domain: lvalue.object_representation_domain,
                    },
                    index,
                    &element_ty,
                    objects,
                    span,
                )?;
                (pointer, element_ty, lvalue.restrict_source)
            }
            ValueCategory::LValue(lvalue) => {
                let value = self.load_lvalue(lvalue, base_span, objects)?;
                let element_ty = value
                    .ty
                    .element_type()
                    .cloned()
                    .filter(|_| value.ty.is_pointer())
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!(
                                "subscripted expression has type {}, not array or pointer",
                                value.ty
                            ),
                            span,
                        )
                    })?;
                let pointer = self.checked_pointer_offset(
                    value.as_pointer(span)?,
                    index,
                    &element_ty,
                    objects,
                    span,
                )?;
                (pointer, element_ty, value.restrict_source)
            }
            ValueCategory::RValue(value) if value.ty.is_pointer() => {
                let element_ty = value.ty.element_type().cloned().unwrap();
                let pointer = self.checked_pointer_offset(
                    value.as_pointer(span)?,
                    index,
                    &element_ty,
                    objects,
                    span,
                )?;
                (pointer, element_ty, value.restrict_source)
            }
            ValueCategory::RValue(value) => {
                return Err(Diagnostic::error(
                    format!(
                        "cannot take the address of a subobject of {} rvalue",
                        value.ty
                    ),
                    span,
                ));
            }
        };
        Ok(ValueCategory::RValue(TypedValue {
            ty: CType::pointer_to(element_ty),
            data: ValueData::Pointer(Rc::new(pointer)),
            restrict_source,
            indeterminate: false,
            missing_return: false,
        }))
    }

    fn subscript_pointer_value(
        &self,
        value: TypedValue,
        index: i128,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let element_ty = value
            .ty
            .element_type()
            .cloned()
            .filter(|_| value.ty.is_pointer())
            .ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "subscripted expression has type {}, not array or pointer",
                        value.ty
                    ),
                    span,
                )
            })?;
        let pointer = self.checked_pointer_offset(
            value.as_pointer(span)?,
            index,
            &element_ty,
            objects,
            span,
        )?;
        self.dereference_value(
            TypedValue {
                ty: value.ty,
                data: ValueData::Pointer(Rc::new(pointer)),
                restrict_source: value.restrict_source,
                indeterminate: value.indeterminate,
                missing_return: value.missing_return,
            },
            span,
            objects,
        )
    }

    pub(super) fn eval_binary(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        match op {
            BinaryOp::Comma => {
                let sequencing = self.sequencing_snapshot();
                let _ = self.eval_rvalue(lhs, frame, objects)?;
                let lhs_footprint = self.finish_sequenced_operand(sequencing);
                let result = self.eval_rvalue(rhs, frame, objects)?;
                self.merge_sequenced_footprint(lhs_footprint);
                return Ok(ValueCategory::RValue(result));
            }
            BinaryOp::LogicalAnd => {
                let sequencing = self.sequencing_snapshot();
                let lhs_value = self.eval_rvalue(lhs, frame, objects)?;
                let truthy = self.scalar_truthy(&lhs_value, lhs.span())?;
                let lhs_footprint = self.finish_sequenced_operand(sequencing);
                if !truthy {
                    self.merge_sequenced_footprint(lhs_footprint);
                    return Ok(ValueCategory::RValue(TypedValue::int(0)));
                }
                let rhs_value = self.eval_rvalue(rhs, frame, objects)?;
                let result = TypedValue::int(self.scalar_truthy(&rhs_value, rhs.span())? as i128);
                self.merge_sequenced_footprint(lhs_footprint);
                return Ok(ValueCategory::RValue(result));
            }
            BinaryOp::LogicalOr => {
                let sequencing = self.sequencing_snapshot();
                let lhs_value = self.eval_rvalue(lhs, frame, objects)?;
                let truthy = self.scalar_truthy(&lhs_value, lhs.span())?;
                let lhs_footprint = self.finish_sequenced_operand(sequencing);
                if truthy {
                    self.merge_sequenced_footprint(lhs_footprint);
                    return Ok(ValueCategory::RValue(TypedValue::int(1)));
                }
                let rhs_value = self.eval_rvalue(rhs, frame, objects)?;
                let result = TypedValue::int(self.scalar_truthy(&rhs_value, rhs.span())? as i128);
                self.merge_sequenced_footprint(lhs_footprint);
                return Ok(ValueCategory::RValue(result));
            }
            _ => {}
        }
        let lhs_value = self.eval_rvalue(lhs, frame, objects)?;
        let rhs_value = self.eval_rvalue(rhs, frame, objects)?;
        let lhs_value = self.promote_integer_expr_value(lhs, lhs_value, frame, objects)?;
        let rhs_value = self.promote_integer_expr_value(rhs, rhs_value, frame, objects)?;
        if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
            return self.eval_equality(op, lhs_value, rhs_value, lhs, rhs, span, frame, objects);
        }
        Ok(ValueCategory::RValue(self.compute_binary_value(
            op, lhs_value, rhs_value, span, objects,
        )?))
    }

    fn eval_equality(
        &self,
        op: BinaryOp,
        mut lhs: TypedValue,
        mut rhs: TypedValue,
        lhs_expr: &Expr,
        rhs_expr: &Expr,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        if lhs.ty.is_pointer() || rhs.ty.is_pointer() {
            if lhs.ty.is_pointer() && rhs.ty.is_pointer() {
                let common_ty = if self.is_null_pointer_constant(lhs_expr, frame, objects)? {
                    rhs.ty.clone()
                } else if self.is_null_pointer_constant(rhs_expr, frame, objects)? {
                    lhs.ty.clone()
                } else {
                    let CType::Pointer(lhs_inner) = lhs.ty.unqualified() else {
                        unreachable!();
                    };
                    let CType::Pointer(rhs_inner) = rhs.ty.unqualified() else {
                        unreachable!();
                    };
                    self.composite_pointer_target_type(lhs_inner, rhs_inner)
                        .map(CType::pointer_to)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "equality comparison requires compatible pointer operand types",
                                span,
                            )
                        })?
                };
                lhs = self.convert_value(lhs, &common_ty, span)?;
                rhs = self.convert_value(rhs, &common_ty, span)?;
            }
            let lhs = self.pointer_equality_operand(lhs_expr, lhs, span, frame, objects)?;
            let rhs = self.pointer_equality_operand(rhs_expr, rhs, span, frame, objects)?;
            let equal = lhs == rhs;
            let value = match op {
                BinaryOp::Equal => equal,
                BinaryOp::NotEqual => !equal,
                _ => unreachable!(),
            };
            return Ok(ValueCategory::RValue(TypedValue::int(value as i128)));
        }
        let (lhs, rhs, common_ty) = self.usual_arithmetic_operands(lhs, rhs, span)?;
        let equal = if common_ty.is_complex() {
            let lhs = lhs.to_complex()?;
            let rhs = rhs.to_complex()?;
            lhs.real == rhs.real && lhs.imag == rhs.imag
        } else if common_ty.is_floating() {
            lhs.to_float()? == rhs.to_float()?
        } else {
            self.integer_compare_for_type(lhs.to_int()?, rhs.to_int()?, &common_ty, |a, b| a == b)?
        };
        let value = match op {
            BinaryOp::Equal => equal,
            BinaryOp::NotEqual => !equal,
            _ => unreachable!(),
        };
        Ok(ValueCategory::RValue(TypedValue::int(value as i128)))
    }

    fn eval_increment(
        &mut self,
        expr: &Expr,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        delta: i64,
        is_prefix: bool,
    ) -> Result<ValueCategory, Diagnostic> {
        let lvalue = match self.eval(expr, frame, objects)? {
            ValueCategory::LValue(lvalue) => lvalue,
            ValueCategory::RValue(_) => {
                return Err(Diagnostic::error(
                    "++ and -- require a changeable arithmetic variable or pointer",
                    span,
                ));
            }
        };
        if !(lvalue.ty.is_integer() || lvalue.ty.is_floating() || lvalue.ty.is_pointer()) {
            return Err(Diagnostic::error(
                "++ and -- require a numeric variable or pointer",
                span,
            ));
        }
        if lvalue.ty.is_const_qualified() {
            return Err(Diagnostic::error(
                "++ and -- require a changeable arithmetic variable or pointer",
                span,
            ));
        }
        let assignment_target = self.lvalue_access_region(&lvalue, objects, span)?;
        self.push_assignment_target(assignment_target, AssignmentTargetKind::Increment);
        let old_value = self.load_lvalue(lvalue.clone(), expr.span(), objects)?;
        self.pop_assignment_target();
        let new_value = if lvalue.ty.is_pointer() {
            let pointer = old_value.as_pointer(span)?;
            TypedValue {
                ty: lvalue.ty.clone(),
                data: ValueData::Pointer(Rc::new(
                    self.checked_pointer_offset(
                        pointer,
                        delta.into(),
                        lvalue
                            .ty
                            .element_type()
                            .expect("pointer lvalue has element type"),
                        objects,
                        span,
                    )?,
                )),
                restrict_source: old_value.restrict_source.clone(),
                indeterminate: false,
                missing_return: false,
            }
        } else {
            self.compute_binary_value(
                if delta >= 0 {
                    BinaryOp::Add
                } else {
                    BinaryOp::Sub
                },
                old_value.clone(),
                TypedValue::int(i128::from(delta.unsigned_abs())),
                span,
                objects,
            )?
        };
        let stored = self.convert_value(new_value, &lvalue.ty, span)?;
        self.store_lvalue(objects, &lvalue, stored.clone(), span)?;
        Ok(ValueCategory::RValue(if is_prefix {
            stored
        } else {
            old_value
        }))
    }

    fn eval_member(
        &mut self,
        base: &Expr,
        member: &str,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        match self.eval(base, frame, objects)? {
            ValueCategory::LValue(mut lvalue) => {
                let ResolvedMemberAccess {
                    path: member_path,
                    ty: member_ty,
                    bit_field_width,
                } = self.resolve_member_access(&lvalue.ty, member, span)?;
                let member_byte_offset = if lvalue.byte_offset_override.is_some() {
                    self.member_path_offset(&lvalue.ty, &member_path)
                } else {
                    None
                };
                lvalue.ty = member_ty;
                if lvalue.member_path.is_empty() {
                    lvalue.member_path = member_path;
                } else {
                    Rc::make_mut(&mut lvalue.member_path).extend(member_path.iter().cloned());
                }
                if let (Some(current), Some(extra)) =
                    (lvalue.byte_offset_override, member_byte_offset)
                {
                    lvalue.byte_offset_override = current.checked_add(extra);
                    lvalue.base_offset = 0;
                    lvalue.offset = 0;
                }
                lvalue.bit_field_width = bit_field_width;
                self.normalize_lvalue_base_offset(&mut lvalue, objects);
                Ok(ValueCategory::LValue(lvalue))
            }
            ValueCategory::RValue(value) => {
                let ResolvedMemberAccess {
                    path: member_path,
                    ty: member_ty,
                    ..
                } = self.resolve_member_access(&value.ty, member, span)?;
                let ValueData::Aggregate(stored) = value.data else {
                    return Err(Diagnostic::error(
                        "member access requires a structure or union operand",
                        span,
                    ));
                };
                let member_value =
                    self.load_member_from_aggregate(&stored, &value.ty, &member_path, span)?;
                if matches!(member_ty.unqualified(), CType::Array(..)) {
                    let stored =
                        self.stored_value_from_typed_value(member_value, &member_ty, span)?;
                    return self
                        .materialize_full_expression_temporary(member_ty, stored, span, objects);
                }
                Ok(ValueCategory::RValue(self.convert_value(
                    member_value,
                    &member_ty,
                    span,
                )?))
            }
        }
    }

    fn materialize_full_expression_temporary(
        &mut self,
        ty: CType,
        stored: StoredValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let byte_size = self
            .type_size_of(&ty)
            .ok_or_else(|| Diagnostic::error("temporary object has incomplete type", span))?;
        self.ensure_object_storage_limit(
            objects,
            StorageDuration::Automatic,
            byte_size,
            None,
            span,
        )?;
        let object = self.allocate_object_id();
        self.assign_object_base_address(object, &ty);
        let initialized = self.stored_value_is_determinate(&stored);
        self.insert_live_retired_object(
            object,
            ObjectState {
                ty: ty.clone(),
                storage_duration: StorageDuration::Automatic,
                alive: true,
                readonly: false,
                const_object: ty.is_const_qualified(),
                has_const_subobject: self.type_has_const_subobject(&ty),
                has_volatile_subobject: self.type_has_volatile_subobject(&ty),
                register_object: false,
                address_taken: false,
                initialized,
                indeterminate_reason: None,
                byte_size,
                value: stored,
                declaration_span: span,
                modification_count: 1,
                raw_indeterminate_bytes: None,
                variably_modified: false,
                effective_types: Vec::new(),
                pointer_slots: BTreeMap::default(),
            },
        );
        self.object_type_registry.insert(object, ty.clone());
        self.full_expression_temporaries.push(object);
        Ok(ValueCategory::LValue(LValue {
            object,
            ty,
            base_offset: 0,
            offset: 0,
            member_path: Rc::new(Vec::new()),
            designated_root_ty: None,
            byte_offset_override: None,
            arithmetic_domain_start: None,
            object_representation_domain: None,
            bit_field_width: None,
            restrict_source: None,
        }))
    }

    fn eval_compound_literal(
        &mut self,
        ty: &CType,
        vla_bounds: &[Option<Expr>],
        initializer: &Initializer,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let ty = self.resolve_decl_type(ty, vla_bounds, frame, objects, span)?;
        let storage = if objects.len() == 1 {
            StorageDuration::Static
        } else {
            StorageDuration::Automatic
        };
        let (actual_ty, stored) =
            self.build_initialized_stored_value(&ty, initializer, frame, objects)?;
        let cache_key = (frame.id, span);
        let object = if !vla_bounds.iter().any(Option::is_some) {
            if let Some(object) = self
                .compound_literal_bindings
                .get(&cache_key)
                .copied()
                .filter(|object| {
                    self.lookup_object(objects, *object)
                        .is_some_and(|state| state.alive)
                })
            {
                object
            } else {
                let object =
                    self.allocate_object(objects, actual_ty.clone(), storage, span, false, false)?;
                self.compound_literal_bindings.insert(cache_key, object);
                object
            }
        } else {
            self.allocate_object(objects, actual_ty.clone(), storage, span, false, true)?
        };
        let initialized = self.stored_value_is_determinate(&stored);
        let actual_size = self.type_size_of(&actual_ty).unwrap_or(0);
        let previous_size = self
            .lookup_object(objects, object)
            .filter(|state| state.alive && state.storage_duration != StorageDuration::Dynamic)
            .map_or(0, |state| state.byte_size);
        self.ensure_object_storage_limit(objects, storage, actual_size, Some(object), span)?;
        if let Some(state) = self.lookup_object_mut(objects, object) {
            state.ty = actual_ty.clone();
            state.initialized = initialized;
            state.indeterminate_reason = None;
            state.byte_size = actual_size;
            state.value = stored;
            state.modification_count = state.modification_count.saturating_add(1);
        }
        if storage != StorageDuration::Dynamic {
            self.replace_live_non_dynamic_bytes(previous_size, actual_size);
        }
        self.object_type_registry.insert(object, actual_ty.clone());
        Ok(ValueCategory::LValue(LValue {
            object,
            ty: actual_ty.clone(),
            base_offset: 0,
            offset: 0,
            member_path: Rc::new(Vec::new()),
            designated_root_ty: Some(Rc::new(actual_ty.clone())),
            byte_offset_override: None,
            arithmetic_domain_start: None,
            object_representation_domain: None,
            bit_field_width: None,
            restrict_source: None,
        }))
    }
}
