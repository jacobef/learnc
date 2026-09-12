//! Scalar conversions, promotions, arithmetic, and overflow checks.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn normalize_bit_field_value(&self, value: i128, ty: &CType, width: u8) -> i128 {
        if matches!(ty.unqualified(), CType::Bool) {
            return (value != 0) as i128;
        }
        if width == 0 {
            return 0;
        }
        let mask = self.integer_mask(width as u32);
        let wrapped = (value as u128) & mask;
        if ty.is_unsigned_integer() {
            wrapped as i128
        } else {
            let sign_bit = 1u128 << (width - 1);
            if wrapped & sign_bit != 0 {
                (wrapped as i128) - ((mask + 1) as i128)
            } else {
                wrapped as i128
            }
        }
    }

    pub(super) fn convert_value(
        &self,
        value: TypedValue,
        target: &CType,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        self.reject_missing_return_value(&value, span)?;
        self.reject_indeterminate_pointer_use(&value, span)?;
        if &value.ty == target {
            return Ok(value);
        }
        let source_ty = value.ty.clone();
        match () {
            _ if matches!(source_ty.unqualified(), CType::Array(_, _))
                && matches!(target.unqualified(), CType::Array(_, _))
                && self.array_value_types_compatible(target, &source_ty, false) =>
            {
                Ok(TypedValue {
                    ty: target.clone(),
                    data: value.data,
                    restrict_source: None,
                    indeterminate: value.indeterminate,
                    missing_return: false,
                })
            }
            _ if matches!(
                source_ty.unqualified(),
                CType::Struct(_, _) | CType::Union(_, _)
            ) && self.cross_unit_tagged_type_compatible(
                source_ty.unqualified(),
                target.unqualified(),
            ) =>
            {
                Ok(TypedValue {
                    ty: target.clone(),
                    data: value.data,
                    restrict_source: None,
                    indeterminate: value.indeterminate,
                    missing_return: false,
                })
            }
            _ if matches!(target.unqualified(), CType::Bool)
                && (source_ty.is_arithmetic()
                    || source_ty.is_pointer()
                    || source_ty.is_function()) =>
            {
                Ok(TypedValue::integer(
                    target.clone(),
                    self.scalar_truthy(&value, span)? as i128,
                ))
            }
            _ if source_ty.is_integer() && target.is_integer() => {
                let converted = target
                    .normalize_integer_value(value.to_int()?)
                    .expect("integer target has a representable object model");
                Ok(TypedValue {
                    ty: target.clone(),
                    data: ValueData::Int(CIntValue::new(converted)),
                    restrict_source: None,
                    indeterminate: value.indeterminate,
                    missing_return: false,
                })
            }
            _ if source_ty.is_integer() && target.is_floating() => Ok(TypedValue::floating(
                target.clone(),
                self.convert_int_to_float(value.to_int()?, target, span)?,
            )),
            _ if source_ty.is_integer() && target.is_complex() => {
                let component_ty = target
                    .complex_component_type()
                    .expect("complex target has component type");
                Ok(TypedValue::complex(
                    target.clone(),
                    self.convert_int_to_float(value.to_int()?, component_ty, span)?,
                    0.0,
                ))
            }
            _ if source_ty.is_floating() && target.is_integer() => Ok(TypedValue::integer(
                target.clone(),
                self.convert_float_to_integer(value.to_float()?, target, span)?,
            )),
            _ if source_ty.is_floating() && target.is_complex() => {
                let component_ty = target
                    .complex_component_type()
                    .expect("complex target has component type");
                Ok(TypedValue::complex(
                    target.clone(),
                    self.convert_float_to_float(value.to_float()?, component_ty, span)?,
                    0.0,
                ))
            }
            _ if source_ty.is_floating() && target.is_floating() => Ok(TypedValue::floating(
                target.clone(),
                self.convert_float_to_float(value.to_float()?, target, span)?,
            )),
            _ if source_ty.is_complex() && target.is_complex() => {
                let source = value.to_complex()?;
                let component_ty = target
                    .complex_component_type()
                    .expect("complex target has component type");
                Ok(TypedValue::complex(
                    target.clone(),
                    self.convert_float_to_float(source.real, component_ty, span)?,
                    self.convert_float_to_float(source.imag, component_ty, span)?,
                ))
            }
            _ if source_ty.is_complex() && target.is_floating() => Ok(TypedValue::floating(
                target.clone(),
                self.convert_float_to_float(value.to_complex()?.real, target, span)?,
            )),
            _ if source_ty.is_complex() && target.is_integer() => Ok(TypedValue::integer(
                target.clone(),
                self.convert_float_to_integer(value.to_complex()?.real, target, span)?,
            )),
            _ if source_ty.is_pointer() && target.is_integer() => Ok(TypedValue::integer(
                target.clone(),
                self.ensure_integer_range(
                    i128::from(self.pointer_numeric_address(&value, &source_ty, span)?),
                    target,
                    span,
                    "pointer value is outside the supported range of the destination integer type",
                )?,
            )),
            _ if source_ty.is_integer() && target.is_pointer() => {
                let address = value.to_int()? as u64;
                Ok(self.pointer_value_from_integer(address, target, value.indeterminate))
            }
            _ if source_ty.is_pointer() && target.is_pointer() => {
                let mut data = value.data;
                if let (CType::Pointer(target_inner), CType::Pointer(source_inner)) =
                    (target.unqualified(), source_ty.unqualified())
                    && let ValueData::Pointer(pointer) = &mut data
                {
                    let pointer = Rc::make_mut(pointer);
                    let target_exposes_object_representation = target_inner.is_character()
                        || matches!(target_inner.unqualified(), CType::Void);
                    let source_exposes_object_representation = source_inner.is_character()
                        || matches!(source_inner.unqualified(), CType::Void);
                    if ((target_exposes_object_representation
                        && !self.compatible_object_layout_types(target_inner, source_inner))
                        || Self::corresponding_signed_unsigned_types(target_inner, source_inner))
                        && !pointer.is_null()
                        && Self::opaque_integer_pointer_address(pointer).is_none()
                    {
                        let byte_offset = self
                            .pointer_byte_offset_from_root_type(pointer, source_inner)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "cannot preserve the address while converting to a character or void pointer",
                                    span,
                                )
                            })?;
                        pointer.byte_offset_override = Some(byte_offset);
                        if target_exposes_object_representation
                            && !source_exposes_object_representation
                        {
                            pointer.object_representation_domain = self
                                .type_size_of(source_inner)
                                .and_then(|size| byte_offset.checked_add(size))
                                .map(|one_past_end| {
                                    let domain = ByteDomain {
                                        start: byte_offset,
                                        one_past_end,
                                    };
                                    if target_inner.is_character() {
                                        ObjectRepresentationDomain::Active(domain)
                                    } else {
                                        ObjectRepresentationDomain::Latent(domain)
                                    }
                                });
                        }
                    } else if matches!(source_inner.unqualified(), CType::Union(_, _))
                        && self.record_type(source_inner).is_some_and(|record| {
                            record.members.iter().any(|member| {
                                member.bit_width.is_none()
                                    && self.compatible_object_layout_types(&member.ty, target_inner)
                            })
                        })
                        && !pointer.is_null()
                        && Self::opaque_integer_pointer_address(pointer).is_none()
                    {
                        let byte_offset = self
                            .pointer_byte_offset_from_root_type(pointer, source_inner)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "cannot preserve the address while converting a union pointer to a member pointer",
                                    span,
                                )
                            })?;
                        Rc::make_mut(&mut pointer.member_path).clear();
                        pointer.byte_offset_override = Some(byte_offset);
                    } else if let Some(domain) =
                        self.array_flatten_domain(source_inner, target_inner)
                    {
                        let byte_offset = self
                            .pointer_byte_offset_from_root_type(pointer, source_inner)
                            .or_else(|| {
                                self.pointer_byte_offset_from_root_type(pointer, target_inner)
                            })
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "cannot preserve the address while converting pointer-to-array to pointer-to-element",
                                    span,
                                )
                            })?;
                        pointer.base_offset = 0;
                        pointer.offset = 0;
                        Rc::make_mut(&mut pointer.member_path).clear();
                        pointer.designated_root_ty = Some(Rc::new(domain));
                        pointer.byte_offset_override = Some(byte_offset);
                        pointer.arithmetic_domain_start = Some(byte_offset);
                    }
                    if target_inner.is_character() {
                        pointer.object_representation_domain = pointer
                            .object_representation_domain
                            .map(|domain| ObjectRepresentationDomain::Active(domain.byte_domain()));
                    } else if matches!(target_inner.unqualified(), CType::Void) {
                        pointer.object_representation_domain = pointer
                            .object_representation_domain
                            .map(|domain| ObjectRepresentationDomain::Latent(domain.byte_domain()));
                    } else {
                        // A round trip through void* or a character pointer recovers the
                        // original typed pointer. Its ordinary array provenance remains
                        // in designated_root_ty/arithmetic_domain_start, so the temporary
                        // object-representation bound must no longer constrain it.
                        pointer.object_representation_domain = None;
                    }
                }
                Ok(TypedValue {
                    ty: target.clone(),
                    data,
                    restrict_source: value.restrict_source,
                    indeterminate: value.indeterminate,
                    missing_return: false,
                })
            }
            _ => Err(Diagnostic::error(
                format!("cannot convert {} to {}", source_ty, target),
                span,
            )),
        }
    }

    pub(super) fn array_value_types_compatible(
        &self,
        target: &CType,
        source: &CType,
        allow_incomplete_target: bool,
    ) -> bool {
        let (CType::Array(target_inner, target_len), CType::Array(source_inner, source_len)) =
            (target.unqualified(), source.unqualified())
        else {
            return false;
        };
        (*target_len == *source_len || (allow_incomplete_target && *target_len == 0))
            && self.cross_unit_tagged_type_compatible(
                &Self::unqualified_value_type(target_inner),
                &Self::unqualified_value_type(source_inner),
            )
    }

    pub(super) fn integer_promotion(
        &self,
        value: TypedValue,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        if !value.ty.is_integer() {
            return Err(Diagnostic::error(
                format!("expected integer type, got {}", value.ty),
                span,
            ));
        }
        let promoted = match value.ty.unqualified() {
            CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(_, _) => CType::Int,
            _ => value.ty.unqualified().clone(),
        };
        self.convert_value(value, &promoted, span)
    }

    pub(super) fn scalar_truthy(&self, value: &TypedValue, span: Span) -> Result<bool, Diagnostic> {
        self.reject_missing_return_value(value, span)?;
        self.reject_indeterminate_pointer_use(value, span)?;
        match &value.data {
            ValueData::Int(int) => Ok(int.get() != 0),
            ValueData::Float(float) => Ok(*float != 0.0),
            ValueData::Complex(complex) => Ok(complex.real != 0.0 || complex.imag != 0.0),
            ValueData::Pointer(pointer) => Ok(!pointer.is_null()),
            ValueData::Function(_) => Ok(true),
            ValueData::Aggregate(_) => Err(Diagnostic::error(
                "aggregate expression is not scalar",
                span,
            )),
            ValueData::ObjectRepresentation(_) => Err(Diagnostic::ub(
                "read of a scalar with an invalid object representation",
                span,
                Some("6.2.6.1p5-6"),
            )),
            ValueData::Void => Err(Diagnostic::error("void expression is not scalar", span)),
        }
    }

    pub(super) fn compute_binary_value(
        &self,
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if lhs.ty == rhs.ty
            && Self::is_promoted_integer_type(&lhs.ty)
            && matches!(
                op,
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem
                    | BinaryOp::BitAnd
                    | BinaryOp::BitXor
                    | BinaryOp::BitOr
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
            )
        {
            self.reject_missing_return_value(&lhs, span)?;
            self.reject_missing_return_value(&rhs, span)?;
            return self.compute_same_type_integer_binary_value(op, lhs, rhs, span);
        }
        if matches!(op, BinaryOp::Add | BinaryOp::Sub)
            && (lhs.ty.is_pointer() || rhs.ty.is_pointer())
        {
            return self.eval_pointer_arithmetic(op, lhs, rhs, span, objects);
        }
        if matches!(
            op,
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual
        ) && (lhs.ty.is_pointer() || rhs.ty.is_pointer())
        {
            return self.eval_pointer_relational(op, lhs, rhs, span, objects);
        }
        if matches!(
            op,
            BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitXor | BinaryOp::BitOr
        ) && (!lhs.ty.is_integer() || !rhs.ty.is_integer())
        {
            return Err(Diagnostic::error(
                format!(
                    "this operator requires integer operands, but the operands have types {} and {}",
                    lhs.ty, rhs.ty
                ),
                span,
            ));
        }
        match op {
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                self.compute_shift_value(op, lhs, rhs, span)
            }
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::BitAnd
            | BinaryOp::BitXor
            | BinaryOp::BitOr
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual => self.compute_arithmetic_binary_value(op, lhs, rhs, span),
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::Comma => unreachable!(),
        }
    }

    fn is_promoted_integer_type(ty: &CType) -> bool {
        matches!(
            ty.unqualified(),
            CType::Int
                | CType::UnsignedInt
                | CType::Long
                | CType::UnsignedLong
                | CType::LongLong
                | CType::UnsignedLongLong
        )
    }

    fn compute_same_type_integer_binary_value(
        &self,
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let ty = lhs.ty.clone();
        let lhs = lhs.to_int()?;
        let rhs = rhs.to_int()?;
        match op {
            BinaryOp::Add => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_add_for_type(lhs, rhs, &ty, span, "6.5.6")?,
            )),
            BinaryOp::Sub => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_sub_for_type(lhs, rhs, &ty, span, "6.5.6")?,
            )),
            BinaryOp::Mul => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_mul_for_type(lhs, rhs, &ty, span, "6.5.5")?,
            )),
            BinaryOp::Div => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_div_for_type(lhs, rhs, &ty, span)?,
            )),
            BinaryOp::Rem => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_rem_for_type(lhs, rhs, &ty, span)?,
            )),
            BinaryOp::BitAnd => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_bitwise_for_type(lhs, rhs, &ty, |a, b| a & b)?,
            )),
            BinaryOp::BitXor => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_bitwise_for_type(lhs, rhs, &ty, |a, b| a ^ b)?,
            )),
            BinaryOp::BitOr => Ok(TypedValue::integer(
                ty.clone(),
                self.integer_bitwise_for_type(lhs, rhs, &ty, |a, b| a | b)?,
            )),
            BinaryOp::Less => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(lhs, rhs, &ty, |a, b| a < b)? as i128,
                ))
            }
            BinaryOp::LessEqual => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(lhs, rhs, &ty, |a, b| a <= b)? as i128,
                ))
            }
            BinaryOp::Greater => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(lhs, rhs, &ty, |a, b| a > b)? as i128,
                ))
            }
            BinaryOp::GreaterEqual => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(lhs, rhs, &ty, |a, b| a >= b)? as i128,
                ))
            }
            _ => unreachable!(),
        }
    }

    fn compute_arithmetic_binary_value(
        &self,
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let (lhs, rhs, common_ty) = self.usual_arithmetic_operands(lhs, rhs, span)?;
        if common_ty.is_complex() {
            let l = lhs.to_complex()?;
            let r = rhs.to_complex()?;
            let component_ty = common_ty
                .complex_component_type()
                .expect("complex arithmetic type has component type");
            let result = |real: f64, imag: f64| -> Result<TypedValue, Diagnostic> {
                let finite_operands = l.real.is_finite()
                    && l.imag.is_finite()
                    && r.real.is_finite()
                    && r.imag.is_finite();
                if finite_operands && (!real.is_finite() || !imag.is_finite()) {
                    return Err(Diagnostic::ub(
                        "complex arithmetic result is outside the range of its type",
                        span,
                        Some("6.5p5"),
                    ));
                }
                Ok(TypedValue::complex(
                    common_ty.clone(),
                    self.convert_float_to_float(real, component_ty, span)?,
                    self.convert_float_to_float(imag, component_ty, span)?,
                ))
            };
            return match op {
                BinaryOp::Add => result(l.real + r.real, l.imag + r.imag),
                BinaryOp::Sub => result(l.real - r.real, l.imag - r.imag),
                BinaryOp::Mul => {
                    let product = l.mul(r);
                    result(product.real, product.imag)
                }
                BinaryOp::Div => {
                    if r.real == 0.0 && r.imag == 0.0 {
                        return Err(Diagnostic::ub("division by zero", span, Some("6.5.5")));
                    }
                    let quotient = l.div(r);
                    result(quotient.real, quotient.imag)
                }
                BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => Err(Diagnostic::error(
                    "relational operators require real operands",
                    span,
                )),
                BinaryOp::Rem
                | BinaryOp::BitAnd
                | BinaryOp::BitXor
                | BinaryOp::BitOr
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::LogicalAnd
                | BinaryOp::LogicalOr
                | BinaryOp::Comma => unreachable!(),
            };
        }
        if common_ty.is_floating() {
            let l = lhs.to_float()?;
            let r = rhs.to_float()?;
            let result = |value: f64| -> Result<TypedValue, Diagnostic> {
                if value.is_infinite() && l.is_finite() && r.is_finite() {
                    return Err(Diagnostic::ub(
                        "floating arithmetic result is outside the range of its type",
                        span,
                        Some("6.5p5"),
                    ));
                }
                if value.is_nan() && !l.is_nan() && !r.is_nan() {
                    return Err(Diagnostic::ub(
                        "floating arithmetic result is not mathematically defined",
                        span,
                        Some("6.5p5"),
                    ));
                }
                Ok(TypedValue::floating(
                    common_ty.clone(),
                    self.convert_float_to_float(value, &common_ty, span)?,
                ))
            };
            return match op {
                BinaryOp::Add => result(l + r),
                BinaryOp::Sub => result(l - r),
                BinaryOp::Mul => result(l * r),
                BinaryOp::Div => {
                    if r == 0.0 {
                        return Err(Diagnostic::ub("division by zero", span, Some("6.5.5")));
                    }
                    result(l / r)
                }
                BinaryOp::Less => Ok(TypedValue::int((l < r) as i128)),
                BinaryOp::LessEqual => Ok(TypedValue::int((l <= r) as i128)),
                BinaryOp::Greater => Ok(TypedValue::int((l > r) as i128)),
                BinaryOp::GreaterEqual => Ok(TypedValue::int((l >= r) as i128)),
                BinaryOp::Rem
                | BinaryOp::BitAnd
                | BinaryOp::BitXor
                | BinaryOp::BitOr
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::LogicalAnd
                | BinaryOp::LogicalOr
                | BinaryOp::Comma => unreachable!(),
            };
        }
        let l = lhs.to_int()?;
        let r = rhs.to_int()?;
        match op {
            BinaryOp::Add => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_add_for_type(l, r, &common_ty, span, "6.5.6")?,
            )),
            BinaryOp::Sub => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_sub_for_type(l, r, &common_ty, span, "6.5.6")?,
            )),
            BinaryOp::Mul => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_mul_for_type(l, r, &common_ty, span, "6.5.5")?,
            )),
            BinaryOp::Div => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_div_for_type(l, r, &common_ty, span)?,
            )),
            BinaryOp::Rem => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_rem_for_type(l, r, &common_ty, span)?,
            )),
            BinaryOp::BitAnd => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_bitwise_for_type(l, r, &common_ty, |a, b| a & b)?,
            )),
            BinaryOp::BitXor => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_bitwise_for_type(l, r, &common_ty, |a, b| a ^ b)?,
            )),
            BinaryOp::BitOr => Ok(TypedValue::integer(
                common_ty.clone(),
                self.integer_bitwise_for_type(l, r, &common_ty, |a, b| a | b)?,
            )),
            BinaryOp::Less => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(l, r, &common_ty, |a, b| a < b)? as i128,
                ))
            }
            BinaryOp::LessEqual => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(l, r, &common_ty, |a, b| a <= b)? as i128,
                ))
            }
            BinaryOp::Greater => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(l, r, &common_ty, |a, b| a > b)? as i128,
                ))
            }
            BinaryOp::GreaterEqual => {
                Ok(TypedValue::int(
                    self.integer_compare_for_type(l, r, &common_ty, |a, b| a >= b)? as i128,
                ))
            }
            BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::Comma => unreachable!(),
        }
    }

    fn compute_shift_value(
        &self,
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        let lhs = self.integer_promotion(lhs, span)?;
        let rhs = self.integer_promotion(rhs, span)?;
        let lhs_ty = lhs.ty.clone();
        let lhs_value = lhs.to_int()?;
        let rhs_value = rhs.to_int()?;
        let bits = lhs_ty.integer_bits().unwrap() as i128;
        if rhs_value < 0 || rhs_value >= bits {
            return Err(Diagnostic::ub(
                match op {
                    BinaryOp::ShiftLeft => "left shift count is negative or too large",
                    BinaryOp::ShiftRight => "right shift count is negative or too large",
                    _ => unreachable!(),
                },
                span,
                Some("6.5.7"),
            ));
        }
        let shift = rhs_value as u32;
        let value = match op {
            BinaryOp::ShiftLeft => self.integer_shift_left(lhs_value, shift, &lhs_ty, span)?,
            BinaryOp::ShiftRight => self.integer_shift_right(lhs_value, shift, &lhs_ty),
            _ => unreachable!(),
        };
        Ok(TypedValue::integer(lhs_ty, value))
    }

    pub(super) fn usual_arithmetic_operands(
        &self,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
    ) -> Result<(TypedValue, TypedValue, CType), Diagnostic> {
        let lhs = if lhs.ty.is_integer() {
            self.integer_promotion(lhs, span)?
        } else {
            lhs
        };
        let rhs = if rhs.ty.is_integer() {
            self.integer_promotion(rhs, span)?
        } else {
            rhs
        };
        let common_ty = self.usual_arithmetic_type(&lhs.ty, &rhs.ty, span)?;
        let lhs = self.convert_value(lhs, &common_ty, span)?;
        let rhs = self.convert_value(rhs, &common_ty, span)?;
        Ok((lhs, rhs, common_ty))
    }

    fn common_real_arithmetic_type(&self, lhs: &CType, rhs: &CType) -> CType {
        if matches!(lhs.unqualified(), CType::LongDouble)
            || matches!(rhs.unqualified(), CType::LongDouble)
        {
            CType::LongDouble
        } else if matches!(lhs.unqualified(), CType::Double)
            || matches!(rhs.unqualified(), CType::Double)
        {
            CType::Double
        } else {
            CType::Float
        }
    }

    pub(super) fn usual_arithmetic_type(
        &self,
        lhs: &CType,
        rhs: &CType,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        if !lhs.is_arithmetic() || !rhs.is_arithmetic() {
            return Err(Diagnostic::error(
                format!(
                    "this arithmetic operator requires numeric operands, but the operands have types {} and {}",
                    lhs, rhs
                ),
                span,
            ));
        }
        let lhs = if lhs.is_integer() {
            self.promoted_integer_type(lhs, span)?
        } else {
            lhs.clone()
        };
        let rhs = if rhs.is_integer() {
            self.promoted_integer_type(rhs, span)?
        } else {
            rhs.clone()
        };
        let (lhs, rhs) = (&lhs, &rhs);
        if lhs.is_complex() || rhs.is_complex() {
            let lhs_real = lhs.complex_component_type().unwrap_or(lhs);
            let rhs_real = rhs.complex_component_type().unwrap_or(rhs);
            return Ok(CType::complex_of(
                self.common_real_arithmetic_type(lhs_real, rhs_real),
            ));
        }
        if lhs.is_floating() || rhs.is_floating() {
            return Ok(self.common_real_arithmetic_type(lhs, rhs));
        }
        if lhs == rhs {
            return Ok(lhs.clone());
        }
        if lhs.is_signed_integer() == rhs.is_signed_integer() {
            return Ok(if lhs.integer_rank() >= rhs.integer_rank() {
                lhs.clone()
            } else {
                rhs.clone()
            });
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
                Ok(signed_ty.clone())
            } else {
                Ok(signed_ty.unsigned_variant().unwrap())
            }
        } else {
            Ok(unsigned_ty.clone())
        }
    }

    fn float_max_finite(&self, ty: &CType) -> Option<f64> {
        match ty.unqualified() {
            CType::Float => Some(f32::MAX as f64),
            CType::Double | CType::LongDouble => Some(f64::MAX),
            _ => None,
        }
    }

    fn convert_int_to_float(
        &self,
        value: i128,
        target: &CType,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        let max = self
            .float_max_finite(target)
            .ok_or_else(|| Diagnostic::error("destination type is not floating", span))?;
        let abs = if value == i128::MIN {
            (i128::MAX as f64) + 1.0
        } else {
            value.abs() as f64
        };
        if abs > max {
            return Err(Diagnostic::ub(
                "integer to floating conversion is outside the range of the destination type",
                span,
                Some("6.3.1.4"),
            ));
        }
        Ok(match target.unqualified() {
            CType::Float => (value as f32) as f64,
            CType::Double | CType::LongDouble => value as f64,
            _ => unreachable!(),
        })
    }

    fn convert_float_to_integer(
        &self,
        value: f64,
        target: &CType,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        if matches!(target.unqualified(), CType::Bool) {
            return Ok((value != 0.0) as i128);
        }
        if !value.is_finite() {
            return Err(Diagnostic::ub(
                "floating to integer conversion requires a finite value",
                span,
                Some("6.3.1.4"),
            ));
        }
        let truncated = value.trunc();
        let Some(bits) = target.integer_bits() else {
            return Err(Diagnostic::error("destination type is not integer", span));
        };
        let (lower_bound, upper_bound) = if target.is_signed_integer() {
            let magnitude = 2f64.powi((bits - 1) as i32);
            (-magnitude, magnitude)
        } else {
            (0.0, 2f64.powi(bits as i32))
        };
        if truncated < lower_bound || truncated >= upper_bound {
            return Err(Diagnostic::ub(
                "floating to integer conversion is outside the range of the destination type",
                span,
                Some("6.3.1.4"),
            ));
        }
        Ok(target.normalize_integer_value(truncated as i128).unwrap())
    }

    pub(super) fn convert_float_to_float(
        &self,
        value: f64,
        target: &CType,
        span: Span,
    ) -> Result<f64, Diagnostic> {
        match target.unqualified() {
            CType::Float => {
                let narrowed = value as f32;
                if value.is_finite() && !narrowed.is_finite() {
                    return Err(Diagnostic::ub(
                        "floating conversion is outside the range of the destination type",
                        span,
                        Some("6.3.1.5"),
                    ));
                }
                Ok(narrowed as f64)
            }
            CType::Double | CType::LongDouble => Ok(value),
            _ => Err(Diagnostic::error("destination type is not floating", span)),
        }
    }

    fn integer_add_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        span: Span,
        standard: &'static str,
    ) -> Result<i128, Diagnostic> {
        if ty.is_unsigned_integer() {
            Ok(self.wrap_unsigned_binary(lhs, rhs, ty, |a, b| a.wrapping_add(b)))
        } else {
            self.ensure_integer_ub_range(lhs + rhs, ty, span, standard)
        }
    }

    pub(super) fn integer_sub_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        span: Span,
        standard: &'static str,
    ) -> Result<i128, Diagnostic> {
        if ty.is_unsigned_integer() {
            Ok(self.wrap_unsigned_binary(lhs, rhs, ty, |a, b| a.wrapping_sub(b)))
        } else {
            self.ensure_integer_ub_range(lhs - rhs, ty, span, standard)
        }
    }

    fn integer_mul_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        span: Span,
        standard: &'static str,
    ) -> Result<i128, Diagnostic> {
        if ty.is_unsigned_integer() {
            Ok(self.wrap_unsigned_binary(lhs, rhs, ty, |a, b| a.wrapping_mul(b)))
        } else {
            self.ensure_integer_ub_range(lhs * rhs, ty, span, standard)
        }
    }

    pub(super) fn integer_div_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        if rhs == 0 {
            return Err(Diagnostic::ub("division by zero", span, Some("6.5.5")));
        }
        if ty.is_unsigned_integer() {
            Ok((self.integer_to_unsigned(lhs, ty)? / self.integer_to_unsigned(rhs, ty)?) as i128)
        } else {
            let (min, _) = ty.integer_bounds().unwrap();
            if lhs == min && rhs == -1 {
                return Err(Diagnostic::ub(
                    "signed integer overflow",
                    span,
                    Some("6.5.5"),
                ));
            }
            self.ensure_integer_ub_range(lhs / rhs, ty, span, "6.5.5")
        }
    }

    pub(super) fn integer_rem_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        if rhs == 0 {
            return Err(Diagnostic::ub(
                "remainder with a zero divisor",
                span,
                Some("6.5.5"),
            ));
        }
        if ty.is_unsigned_integer() {
            Ok((self.integer_to_unsigned(lhs, ty)? % self.integer_to_unsigned(rhs, ty)?) as i128)
        } else {
            let (min, _) = ty.integer_bounds().unwrap();
            if lhs == min && rhs == -1 {
                return Err(Diagnostic::ub(
                    "signed integer overflow",
                    span,
                    Some("6.5.5"),
                ));
            }
            self.ensure_integer_ub_range(lhs % rhs, ty, span, "6.5.5")
        }
    }

    pub(super) fn integer_bitwise_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        op: fn(u128, u128) -> u128,
    ) -> Result<i128, Diagnostic> {
        let bits = ty.integer_bits().unwrap();
        let mask = self.integer_mask(bits);
        let lhs = self.integer_to_unsigned(lhs, ty)?;
        let rhs = self.integer_to_unsigned(rhs, ty)?;
        Ok(ty
            .normalize_integer_value((op(lhs, rhs) & mask) as i128)
            .unwrap())
    }

    pub(super) fn integer_compare_for_type(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        op: fn(i128, i128) -> bool,
    ) -> Result<bool, Diagnostic> {
        if ty.is_unsigned_integer() {
            let lhs = self.integer_to_unsigned(lhs, ty)? as i128;
            let rhs = self.integer_to_unsigned(rhs, ty)? as i128;
            Ok(op(lhs, rhs))
        } else {
            Ok(op(lhs, rhs))
        }
    }

    fn integer_shift_left(
        &self,
        lhs: i128,
        shift: u32,
        ty: &CType,
        span: Span,
    ) -> Result<i128, Diagnostic> {
        if ty.is_unsigned_integer() {
            let bits = ty.integer_bits().unwrap();
            let mask = self.integer_mask(bits);
            let lhs = self.integer_to_unsigned(lhs, ty)?;
            return Ok(ty
                .normalize_integer_value(((lhs << shift) & mask) as i128)
                .unwrap());
        }
        if lhs < 0 {
            return Err(Diagnostic::ub(
                "left shift of a negative value is undefined",
                span,
                Some("6.5.7"),
            ));
        }
        self.ensure_integer_ub_range(lhs << shift, ty, span, "6.5.7")
    }

    fn integer_shift_right(&self, lhs: i128, shift: u32, ty: &CType) -> i128 {
        if ty.is_unsigned_integer() {
            (self.integer_to_unsigned(lhs, ty).unwrap() >> shift) as i128
        } else {
            lhs >> shift
        }
    }

    fn integer_to_unsigned(&self, value: i128, ty: &CType) -> Result<u128, Diagnostic> {
        let normalized = ty.normalize_integer_value(value).ok_or_else(|| {
            Diagnostic::error(
                "expected integer type",
                Span::new(crate::source::FileId(0), 0, 0),
            )
        })?;
        Ok(normalized as u128)
    }

    pub(super) fn integer_mask(&self, bits: u32) -> u128 {
        if bits == 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        }
    }

    fn wrap_unsigned_binary(
        &self,
        lhs: i128,
        rhs: i128,
        ty: &CType,
        op: fn(u128, u128) -> u128,
    ) -> i128 {
        let bits = ty.integer_bits().unwrap();
        let mask = self.integer_mask(bits);
        ty.normalize_integer_value(
            (op(
                self.integer_to_unsigned(lhs, ty).unwrap(),
                self.integer_to_unsigned(rhs, ty).unwrap(),
            ) & mask) as i128,
        )
        .unwrap()
    }

    fn ensure_integer_range(
        &self,
        value: i128,
        ty: &CType,
        span: Span,
        message: &'static str,
    ) -> Result<i128, Diagnostic> {
        let normalized = ty
            .normalize_integer_value(value)
            .ok_or_else(|| Diagnostic::error(message, span))?;
        if ty.is_unsigned_integer()
            || ty
                .integer_bounds()
                .is_some_and(|(min, max)| (min..=max).contains(&value))
        {
            Ok(normalized)
        } else {
            Err(Diagnostic::error(message, span))
        }
    }

    pub(super) fn ensure_integer_ub_range(
        &self,
        value: i128,
        ty: &CType,
        span: Span,
        standard: &'static str,
    ) -> Result<i128, Diagnostic> {
        if ty.is_unsigned_integer() {
            Ok(ty.normalize_integer_value(value).unwrap())
        } else if ty
            .integer_bounds()
            .is_some_and(|(min, max)| (min..=max).contains(&value))
        {
            Ok(value)
        } else {
            Err(Diagnostic::ub(
                "signed integer overflow",
                span,
                Some(standard),
            ))
        }
    }

    pub(super) fn ensure_int_range(
        &self,
        value: i128,
        span: Span,
        message: &'static str,
    ) -> Result<i128, Diagnostic> {
        if (INT_MIN..=INT_MAX).contains(&value) {
            Ok(value)
        } else {
            Err(Diagnostic::error(message, span))
        }
    }
}
