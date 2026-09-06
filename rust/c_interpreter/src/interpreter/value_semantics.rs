use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn reject_indeterminate_library_value(
        &self,
        value: &TypedValue,
        span: Span,
        function_name: &str,
    ) -> Result<(), Diagnostic> {
        if value.indeterminate {
            return Err(Diagnostic::ub(
                format!("{function_name} argument has an indeterminate value"),
                span,
                Some("7.1.4, 7.19.6.1, WG14 DR 451"),
            )
            .with_note(
                "library functions exhibit undefined behavior when used on indeterminate values",
            ));
        }
        Ok(())
    }

    pub(super) fn check_library_argument_value(
        &self,
        value: &TypedValue,
        span: Span,
        function_name: &str,
    ) -> Result<(), Diagnostic> {
        self.reject_missing_return_value(value, span)?;
        self.reject_indeterminate_library_value(value, span, function_name)
    }

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

    fn parse_printf_count_core<F>(
        &self,
        function_name: &str,
        what: &str,
        span: Span,
        index: &mut usize,
        char_at: &mut F,
    ) -> Result<Option<PrintfCount>, Diagnostic>
    where
        F: FnMut(usize) -> Option<char>,
    {
        match char_at(*index) {
            Some('*') => {
                *index += 1;
                Ok(Some(PrintfCount::FromArg))
            }
            Some(ch) if ch.is_ascii_digit() => {
                let start = *index;
                let mut value = 0i32;
                while let Some(ch) = char_at(*index) {
                    if !ch.is_ascii_digit() {
                        break;
                    }
                    value = value
                        .checked_mul(10)
                        .and_then(|current| current.checked_add((ch as i32) - ('0' as i32)))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                format!("{function_name} {what} is out of supported range"),
                                span,
                            )
                        })?;
                    *index += 1;
                }
                debug_assert!(*index > start);
                Ok(Some(PrintfCount::Literal(value)))
            }
            _ => Ok(None),
        }
    }

    fn parse_printf_conversion_core<F>(
        &self,
        function_name: &str,
        span: Span,
        index: &mut usize,
        mut char_at: F,
    ) -> Result<PrintfConversion, Diagnostic>
    where
        F: FnMut(usize) -> Option<char>,
    {
        let mut flags = String::new();
        while let Some(ch) = char_at(*index) {
            if matches!(ch, '-' | '+' | ' ' | '#' | '0') {
                flags.push(ch);
                *index += 1;
            } else {
                break;
            }
        }
        let width =
            self.parse_printf_count_core(function_name, "field width", span, index, &mut char_at)?;
        let precision = if char_at(*index) == Some('.') {
            *index += 1;
            self.parse_printf_count_core(function_name, "precision", span, index, &mut char_at)?
                .or(Some(PrintfCount::Literal(0)))
        } else {
            None
        };
        let length = if char_at(*index) == Some('h') && char_at(*index + 1) == Some('h') {
            *index += 2;
            PrintfLength::Hh
        } else if char_at(*index) == Some('h') {
            *index += 1;
            PrintfLength::H
        } else if char_at(*index) == Some('l') && char_at(*index + 1) == Some('l') {
            *index += 2;
            PrintfLength::Ll
        } else if char_at(*index) == Some('l') {
            *index += 1;
            PrintfLength::L
        } else if char_at(*index) == Some('j') {
            *index += 1;
            PrintfLength::J
        } else if char_at(*index) == Some('z') {
            *index += 1;
            PrintfLength::Z
        } else if char_at(*index) == Some('t') {
            *index += 1;
            PrintfLength::T
        } else if char_at(*index) == Some('L') {
            *index += 1;
            PrintfLength::BigL
        } else {
            PrintfLength::None
        };
        let spec = char_at(*index).ok_or_else(|| {
            Diagnostic::error(format!("incomplete {function_name} format specifier"), span)
        })?;
        *index += 1;
        Ok(PrintfConversion {
            flags,
            width,
            precision,
            length,
            spec,
        })
    }

    pub(super) fn parse_printf_conversion_bytes(
        &self,
        function_name: &str,
        format: &[u8],
        index: &mut usize,
        span: Span,
    ) -> Result<PrintfConversion, Diagnostic> {
        self.parse_printf_conversion_core(function_name, span, index, |offset| {
            format.get(offset).copied().map(char::from)
        })
    }

    pub(super) fn parse_printf_conversion_wide(
        &self,
        function_name: &str,
        format: &[libc::wchar_t],
        index: &mut usize,
        span: Span,
    ) -> Result<PrintfConversion, Diagnostic> {
        self.parse_printf_conversion_core(function_name, span, index, |offset| {
            format
                .get(offset)
                .and_then(|unit| char::from_u32(*unit as u32))
        })
    }

    fn printf_length_text(&self, length: PrintfLength) -> &'static str {
        match length {
            PrintfLength::None => "",
            PrintfLength::Hh => "hh",
            PrintfLength::H => "h",
            PrintfLength::L => "l",
            PrintfLength::Ll => "ll",
            PrintfLength::J => "j",
            PrintfLength::Z => "z",
            PrintfLength::T => "t",
            PrintfLength::BigL => "L",
        }
    }

    pub(super) fn printf_conversion_text(&self, conv: &PrintfConversion) -> String {
        let mut text = String::from("%");
        text.push_str(&conv.flags);
        match conv.width {
            Some(PrintfCount::Literal(value)) => text.push_str(&value.to_string()),
            Some(PrintfCount::FromArg) => text.push('*'),
            None => {}
        }
        if let Some(precision) = conv.precision {
            text.push('.');
            match precision {
                PrintfCount::Literal(value) => text.push_str(&value.to_string()),
                PrintfCount::FromArg => text.push('*'),
            }
        }
        text.push_str(self.printf_length_text(conv.length));
        text.push(conv.spec);
        text
    }

    pub(super) fn printf_integer_type(
        &self,
        length: PrintfLength,
        signed: bool,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let ty = match (length, signed) {
            (PrintfLength::None, true) => CType::Int,
            (PrintfLength::None, false) => CType::UnsignedInt,
            (PrintfLength::H | PrintfLength::Hh, _) => CType::Int,
            (PrintfLength::L | PrintfLength::J | PrintfLength::T, true) => CType::Long,
            (PrintfLength::L | PrintfLength::J | PrintfLength::Z | PrintfLength::T, false) => {
                CType::UnsignedLong
            }
            (PrintfLength::Z, true) => CType::Long,
            (PrintfLength::Ll, true) => CType::LongLong,
            (PrintfLength::Ll, false) => CType::UnsignedLongLong,
            _ => {
                return Err(Diagnostic::error(
                    "unsupported printf length modifier",
                    span,
                ));
            }
        };
        Ok(ty)
    }

    pub(super) fn snprintf_bytes_with_call<F>(&self, call: F) -> Result<Vec<u8>, Diagnostic>
    where
        F: Fn(*mut c_char, usize) -> c_int,
    {
        let mut buffer = vec![0u8; 256];
        loop {
            let written = call(buffer.as_mut_ptr().cast::<c_char>(), buffer.len());
            if written < 0 {
                return Err(Diagnostic::error(
                    "host snprintf failed",
                    Span::new(FileId(0), 0, 0),
                ));
            }
            let written = written as usize;
            if written < buffer.len() {
                buffer.truncate(written);
                return Ok(buffer);
            }
            buffer.resize(written + 1, 0);
        }
    }

    pub(super) fn synthetic_void_pointer(&self, address: u64) -> *mut c_void {
        std::ptr::without_provenance_mut::<c_void>(address as usize)
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

    fn unqualified_value_type(ty: &CType) -> CType {
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
                let (_, _, bit_field_width) =
                    self.resolve_member_access(&base_ty, member, *span)?;
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

    pub(super) fn pointer_object_bytes(
        &mut self,
        pointer: &PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, Vec<ByteCell>), Diagnostic> {
        let Some(object_id) = pointer.object else {
            return Err(Diagnostic::ub(
                "null pointer passed where a string or byte region is required",
                span,
                Some("7.1.4"),
            ));
        };
        let object = self
            .lookup_object(objects, object_id)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
        if !object.alive {
            return Err(Diagnostic::ub(
                "access through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            ));
        }
        let start = self
            .pointer_byte_offset(pointer, &CType::UnsignedChar, objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "pointer does not designate a valid byte-addressable region of the object",
                    span,
                    Some("7.1.4"),
                )
            })?;
        let mut all_bytes = self.serialize_stored_value(&object.ty, &object.value, span)?;
        if start > all_bytes.len() {
            return Err(Diagnostic::ub(
                "pointer does not designate a valid byte-addressable region of the object",
                span,
                Some("7.1.4"),
            ));
        }
        if !pointer.member_path.is_empty() {
            let available = self
                .pointer_member_byte_limit(pointer, start, object.byte_size, objects)
                .unwrap_or(0);
            all_bytes.truncate(start.saturating_add(available).min(all_bytes.len()));
        }
        Ok((object_id, start, all_bytes))
    }

    pub(super) fn read_c_string_bytes(
        &mut self,
        pointer: PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<u8>, Diagnostic> {
        let (_, start, all_bytes) = self.pointer_object_bytes(&pointer, span, objects)?;
        let mut bytes = Vec::new();
        for byte in all_bytes.into_iter().skip(start) {
            match byte {
                ByteCell::Known(0) => {
                    let _ =
                        self.library_array_region(&pointer, bytes.len() + 1, 1, span, objects)?;
                    return Ok(bytes);
                }
                ByteCell::Known(byte) => bytes.push(byte),
                ByteCell::Indeterminate => {
                    return Err(Diagnostic::ub(
                        "library string argument contains an indeterminate byte",
                        span,
                        Some("7.1.4, WG14 DR 451"),
                    )
                    .with_note(
                        "library functions exhibit undefined behavior when used on indeterminate values",
                    ));
                }
            }
        }
        Err(Diagnostic::ub(
            "string is not terminated within the bounds of its object",
            span,
            Some("7.1.4"),
        ))
    }

    pub(super) fn byte_region_from_pointer_bounded_string(
        &mut self,
        pointer: &PointerValue,
        limit: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        let (object_id, start, all_bytes) = self.pointer_object_bytes(pointer, span, objects)?;
        let available = all_bytes.len().saturating_sub(start);
        let prefix_len = available.min(limit);
        for (index, byte) in all_bytes.iter().skip(start).take(prefix_len).enumerate() {
            match byte {
                ByteCell::Known(0) => {
                    let _ = self.library_array_region(pointer, index + 1, 1, span, objects)?;
                    return Ok((object_id, start, available));
                }
                ByteCell::Known(_) => {}
                ByteCell::Indeterminate => {
                    return Err(Diagnostic::ub(
                        "library string argument contains an indeterminate byte",
                        span,
                        Some("7.1.4, WG14 DR 451"),
                    )
                    .with_note(
                        "library functions exhibit undefined behavior when used on indeterminate values",
                    ));
                }
            }
        }
        if available < limit {
            return Err(Diagnostic::ub(
                format!(
                    "requested byte access of {} byte(s) starting at offset {} exceeds the {}-byte object",
                    limit,
                    start,
                    all_bytes.len()
                ),
                span,
                Some("7.1.4"),
            ));
        }
        let _ = self.library_array_region(pointer, limit, 1, span, objects)?;
        Ok((object_id, start, available))
    }

    pub(super) fn read_bounded_c_string_source(
        &mut self,
        pointer: PointerValue,
        limit: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<Vec<u8>, Diagnostic> {
        if limit == 0 {
            let _ = self.library_array_region(&pointer, 0, 1, span, objects)?;
            return Ok(Vec::new());
        }
        let (_, start, all_bytes) = self.pointer_object_bytes(&pointer, span, objects)?;
        let mut result = vec![0u8; limit];
        let available = all_bytes.len().saturating_sub(start);
        let prefix_len = available.min(limit);
        for index in 0..prefix_len {
            match all_bytes[start + index] {
                ByteCell::Known(byte) => {
                    result[index] = byte;
                    if byte == 0 {
                        let _ = self.library_array_region(&pointer, index + 1, 1, span, objects)?;
                        return Ok(result);
                    }
                }
                ByteCell::Indeterminate => {
                    return Err(Diagnostic::ub(
                        "library string argument contains an indeterminate byte",
                        span,
                        Some("7.1.4, WG14 DR 451"),
                    )
                    .with_note(
                        "library functions exhibit undefined behavior when used on indeterminate values",
                    ));
                }
            }
        }
        if available < limit {
            return Err(Diagnostic::ub(
                format!(
                    "requested byte access of {} byte(s) starting at offset {} exceeds the {}-byte object",
                    limit,
                    start,
                    all_bytes.len()
                ),
                span,
                Some("7.1.4"),
            ));
        }
        let _ = self.library_array_region(&pointer, limit, 1, span, objects)?;
        Ok(result)
    }

    pub(super) fn read_c_string(
        &mut self,
        pointer: PointerValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<String, Diagnostic> {
        Ok(
            String::from_utf8_lossy(&self.read_c_string_bytes(pointer, span, objects)?)
                .into_owned(),
        )
    }

    pub(super) fn allocate_object_id(&mut self, _frame_index: Option<usize>) -> ObjectId {
        let id = ObjectId(self.next_object);
        self.next_object += 1;
        id
    }

    pub(super) fn insert_live_retired_object(&mut self, id: ObjectId, object: ObjectState) {
        debug_assert!(object.alive);
        debug_assert_ne!(object.storage_duration, StorageDuration::Dynamic);
        if self.run_options.allocation_limit_bytes.is_some() {
            self.live_non_dynamic_bytes = self
                .live_non_dynamic_bytes
                .checked_add(object.byte_size)
                .expect("live non-dynamic object byte total overflowed");
        }
        let previous = self.retired_objects.insert(id, object);
        debug_assert!(previous.is_none());
    }

    pub(super) fn end_live_retired_object_lifetime(&mut self, id: ObjectId) {
        let Some(object) = self.retired_objects.get_mut(&id) else {
            return;
        };
        if !object.alive || object.storage_duration == StorageDuration::Dynamic {
            return;
        }
        if self.run_options.allocation_limit_bytes.is_some() {
            self.live_non_dynamic_bytes = self
                .live_non_dynamic_bytes
                .checked_sub(object.byte_size)
                .expect("retired live object was included in the non-dynamic byte total");
        }
        object.alive = false;
    }

    pub(super) fn intern_readonly_c_bytes(&mut self, bytes: &[u8]) -> PointerValue {
        let mut c_bytes = bytes.to_vec();
        c_bytes.push(0);
        let byte_size = c_bytes.len();
        let array_ty = CType::array_of(CType::Char, byte_size);
        let id = self.allocate_object_id(None);
        self.assign_object_base_address(id, &array_ty);
        self.insert_live_retired_object(
            id,
            ObjectState {
                ty: array_ty.clone(),
                storage_duration: StorageDuration::Static,
                alive: true,
                readonly: true,
                const_object: false,
                has_const_subobject: false,
                has_volatile_subobject: false,
                register_object: false,
                address_taken: true,
                initialized: true,
                indeterminate_reason: None,
                byte_size: c_bytes.len(),
                value: StoredValue::Array(
                    c_bytes
                        .into_iter()
                        .map(|byte| {
                            StoredValue::Scalar(TypedValue::from_data(
                                CType::Char,
                                ValueData::Int(CIntValue::new(byte as i8 as i128)),
                            ))
                        })
                        .collect(),
                ),
                declaration_span: Span::new(crate::source::FileId(0), 0, 0),
                modification_count: 1,
                raw_indeterminate_bytes: None,
                variably_modified: false,
                effective_types: Vec::new(),
                pointer_slots: BTreeMap::default(),
            },
        );
        self.object_type_registry.insert(id, array_ty);
        PointerValue::at_object(id)
    }

    pub(super) fn intern_time_c_bytes(
        &mut self,
        bytes: &[u8],
        span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<PointerValue, Diagnostic> {
        let mut c_bytes = bytes.to_vec();
        c_bytes.push(0);
        let byte_size = c_bytes.len();
        let array_ty = CType::array_of(CType::Char, byte_size);
        let id = if let Some(id) = self.time_text_binding {
            id
        } else {
            let id = self.allocate_object(
                objects,
                array_ty.clone(),
                StorageDuration::Static,
                span,
                false,
                false,
            )?;
            self.time_text_binding = Some(id);
            id
        };
        self.ensure_object_storage_limit(
            objects,
            StorageDuration::Static,
            byte_size,
            Some(id),
            span,
        )?;
        let previous_size = self
            .lookup_object(objects, id)
            .filter(|object| object.alive && object.storage_duration != StorageDuration::Dynamic)
            .map_or(0, |object| object.byte_size);
        let object = self
            .lookup_object_mut(objects, id)
            .ok_or_else(|| Diagnostic::error("time text buffer is unavailable", span))?;
        object.ty = array_ty.clone();
        object.alive = true;
        object.readonly = false;
        object.initialized = true;
        object.byte_size = byte_size;
        object.value = StoredValue::Array(
            c_bytes
                .into_iter()
                .map(|byte| {
                    StoredValue::Scalar(TypedValue::integer(CType::Char, byte as i8 as i128))
                })
                .collect(),
        );
        object.modification_count = object.modification_count.saturating_add(1);
        self.replace_live_non_dynamic_bytes(previous_size, byte_size);
        self.object_type_registry.insert(id, array_ty);
        Ok(PointerValue::at_object(id))
    }

    pub(super) fn intern_string_literal(&mut self, text: &StringLiteralValue) -> PointerValue {
        self.intern_readonly_c_bytes(&text.narrow_bytes())
    }

    pub(super) fn intern_wide_string_literal(
        &mut self,
        text: &StringLiteralValue,
        span: Span,
    ) -> Result<PointerValue, Diagnostic> {
        let mut units = self
            .wide_literal_units(text)
            .map_err(|_| Diagnostic::error("invalid wide string literal", span))?;
        units.push(0);
        let bytes = self.wide_units_to_byte_cells(&units, span)?;
        let array_ty = CType::array_of(self.wchar_type(), units.len());
        let id = self.allocate_object_id(None);
        self.assign_object_base_address(id, &array_ty);
        let value = self.deserialize_stored_value(&array_ty, &bytes, span)?;
        self.insert_live_retired_object(
            id,
            ObjectState {
                ty: array_ty.clone(),
                storage_duration: StorageDuration::Static,
                alive: true,
                readonly: true,
                const_object: false,
                has_const_subobject: false,
                has_volatile_subobject: false,
                register_object: false,
                address_taken: true,
                initialized: true,
                indeterminate_reason: None,
                byte_size: bytes.len(),
                value,
                declaration_span: Span::new(crate::source::FileId(0), 0, 0),
                modification_count: 1,
                raw_indeterminate_bytes: None,
                variably_modified: false,
                effective_types: Vec::new(),
                pointer_slots: BTreeMap::default(),
            },
        );
        self.object_type_registry.insert(id, array_ty);
        Ok(PointerValue::at_object(id))
    }

    pub(super) fn eval_unicode_string_literal(
        &mut self,
        text: &StringLiteralValue,
        utf16: bool,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        let element_ty = if utf16 {
            CType::UnsignedShort
        } else {
            CType::UnsignedInt
        };
        let mut units: Vec<u32> = if utf16 {
            text.utf16_units().into_iter().map(u32::from).collect()
        } else {
            text.utf32_units()
        };
        units.push(0);
        let array_ty = CType::array_of(element_ty.clone(), units.len());
        let id = self.allocate_object_id(None);
        self.assign_object_base_address(id, &array_ty);
        self.insert_live_retired_object(
            id,
            ObjectState {
                ty: array_ty.clone(),
                storage_duration: StorageDuration::Static,
                alive: true,
                readonly: true,
                const_object: false,
                has_const_subobject: false,
                has_volatile_subobject: false,
                register_object: false,
                address_taken: true,
                initialized: true,
                indeterminate_reason: None,
                byte_size: self.type_size_of(&array_ty).unwrap_or(0),
                value: StoredValue::Array(
                    units
                        .into_iter()
                        .map(|unit| {
                            StoredValue::Scalar(TypedValue::integer(
                                element_ty.clone(),
                                unit as i128,
                            ))
                        })
                        .collect(),
                ),
                declaration_span: span,
                modification_count: 1,
                raw_indeterminate_bytes: None,
                variably_modified: false,
                effective_types: Vec::new(),
                pointer_slots: BTreeMap::default(),
            },
        );
        self.object_type_registry.insert(id, array_ty.clone());
        let _ = objects;
        Ok(ValueCategory::LValue(LValue {
            object: id,
            ty: array_ty,
            base_offset: 0,
            offset: 0,
            member_path: Rc::new(Vec::new()),
            designated_root_ty: None,
            byte_offset_override: None,
            arithmetic_domain_start: None,
            bit_field_width: None,
            restrict_source: None,
        }))
    }

    fn cache_pointer_address(
        &mut self,
        pointer: &PointerValue,
        byte_offset: usize,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(object_id) = pointer.object else {
            return Ok(());
        };
        let base = self
            .object_base_addresses
            .get(&object_id)
            .copied()
            .ok_or_else(|| Diagnostic::error("object does not have an assigned address", span))?;
        let address = base
            .checked_add(byte_offset as u64)
            .ok_or_else(|| Diagnostic::ub("pointer address overflow", span, Some("6.2.6.1p5-6")))?;
        self.encoded_object_pointers
            .insert(pointer.clone(), address);
        let candidate = EncodedPointer::Object {
            pointer: pointer.clone(),
            pointee_ty: None,
        };
        let candidates = self.decoded_pointers.entry(address).or_default();
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
        Ok(())
    }

    pub(super) fn pointer_with_byte_offset(
        &mut self,
        pointer: &PointerValue,
        delta: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<PointerValue, Diagnostic> {
        let mut result = self.checked_pointer_offset(
            pointer.clone(),
            delta as i128,
            &CType::Char,
            objects,
            span,
        )?;
        let byte_offset = self
            .pointer_byte_offset(&result, &CType::Char, objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "pointer does not designate a valid byte-addressable region of the object",
                    span,
                    Some("7.1.4"),
                )
            })?;
        result.byte_offset_override = Some(byte_offset);
        self.cache_pointer_address(&result, byte_offset, span)?;
        Ok(result)
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

    fn type_contains_union(&mut self, ty: &CType) -> bool {
        if self.run_options.optimizing_precomputations
            && let Some(&cached) = self.type_contains_union_cache.get(ty)
        {
            return cached;
        }
        fn visit(interpreter: &Interpreter<'_>, ty: &CType, seen: &mut HashSet<usize>) -> bool {
            match ty.unqualified() {
                CType::Union(_, _) => true,
                CType::Array(inner, _) => visit(interpreter, inner, seen),
                CType::Struct(id, _) => {
                    if !seen.insert(*id) {
                        return false;
                    }
                    let contains_union =
                        interpreter.program.records.get(id).is_some_and(|record| {
                            record
                                .members
                                .iter()
                                .any(|member| visit(interpreter, &member.ty, seen))
                        });
                    seen.remove(id);
                    contains_union
                }
                _ => false,
            }
        }

        let result = visit(self, ty, &mut HashSet::default());
        if self.run_options.optimizing_precomputations {
            self.type_contains_union_cache.insert(ty.clone(), result);
        }
        result
    }

    fn effective_type_alias_allowed(&self, access_ty: &CType, effective_ty: &CType) -> bool {
        fn visit(
            interpreter: &Interpreter<'_>,
            access_ty: &CType,
            effective_ty: &CType,
            seen: &mut HashSet<usize>,
        ) -> bool {
            if access_ty.is_character()
                || interpreter.cross_unit_tagged_type_compatible(
                    access_ty.unqualified(),
                    effective_ty.unqualified(),
                )
                || Interpreter::corresponding_signed_unsigned_types(access_ty, effective_ty)
            {
                return true;
            }
            let (CType::Struct(id, _) | CType::Union(id, _)) = access_ty.unqualified() else {
                return false;
            };
            if !seen.insert(*id) {
                return false;
            }
            let contains = interpreter.record_type(access_ty).is_some_and(|record| {
                record.members.iter().any(|member| {
                    member.bit_width != Some(0)
                        && visit(interpreter, &member.ty, effective_ty, seen)
                })
            });
            seen.remove(id);
            contains
        }

        visit(self, access_ty, effective_ty, &mut HashSet::default())
    }

    fn type_region_allows_effective_access(
        &self,
        parent_ty: &CType,
        parent_start: usize,
        access_start: usize,
        access_size: usize,
        access_ty: &CType,
    ) -> bool {
        let Some(parent_size) = self.type_size_of(parent_ty) else {
            return false;
        };
        let Some(parent_end) = parent_start.checked_add(parent_size) else {
            return false;
        };
        let Some(access_end) = access_start.checked_add(access_size) else {
            return false;
        };
        if access_start < parent_start || access_end > parent_end {
            return false;
        }
        if self.type_region_is_padding(parent_ty, parent_start, access_start, access_size) {
            return true;
        }
        if access_start == parent_start
            && access_size == parent_size
            && self.effective_type_alias_allowed(access_ty, parent_ty)
        {
            return true;
        }
        match parent_ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return false;
                };
                if stride == 0 {
                    return false;
                }
                let first = (access_start - parent_start) / stride;
                if first >= *len {
                    return false;
                }
                let child_start = parent_start + first * stride;
                self.type_region_allows_effective_access(
                    inner,
                    child_start,
                    access_start,
                    access_size,
                    access_ty,
                )
            }
            CType::Struct(_, _) => self.record_type(parent_ty).is_some_and(|record| {
                record.members.iter().any(|member| {
                    member.bit_width != Some(0)
                        && self.type_region_allows_effective_access(
                            &member.ty,
                            parent_start.saturating_add(member.offset),
                            access_start,
                            access_size,
                            access_ty,
                        )
                })
            }),
            CType::Union(_, _) => self.record_type(parent_ty).is_some_and(|record| {
                record.members.iter().any(|member| {
                    member.bit_width != Some(0)
                        && self.type_region_allows_effective_access(
                            &member.ty,
                            parent_start,
                            access_start,
                            access_size,
                            access_ty,
                        )
                })
            }),
            _ => false,
        }
    }

    fn type_region_is_padding(&self, ty: &CType, base: usize, start: usize, size: usize) -> bool {
        if size == 0 {
            return true;
        }
        let Some(end) = start.checked_add(size) else {
            return false;
        };
        let Some(ty_size) = self.type_size_of(ty) else {
            return false;
        };
        let Some(ty_end) = base.checked_add(ty_size) else {
            return false;
        };
        if start < base || end > ty_end {
            return false;
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return false;
                };
                if stride == 0 {
                    return false;
                }
                (0..*len).all(|index| {
                    let child_start = base + index * stride;
                    let child_end = child_start + stride;
                    if start >= child_end || child_start >= end {
                        true
                    } else {
                        self.type_region_is_padding(
                            inner,
                            child_start,
                            start.max(child_start),
                            end.min(child_end) - start.max(child_start),
                        )
                    }
                })
            }
            CType::Struct(_, _) | CType::Union(_, _) => {
                self.record_type(ty).is_some_and(|record| {
                    record.members.iter().all(|member| {
                        if member.bit_width == Some(0) {
                            return true;
                        }
                        let Some(member_size) = self.type_size_of(&member.ty) else {
                            return true;
                        };
                        let member_start = if record.kind == crate::types::RecordKind::Union {
                            base
                        } else {
                            base.saturating_add(member.offset)
                        };
                        let member_end = member_start.saturating_add(member_size);
                        if start >= member_end || member_start >= end {
                            return true;
                        }
                        if member.bit_width.is_some() {
                            return false;
                        }
                        let overlap_start = start.max(member_start);
                        let overlap_end = end.min(member_end);
                        self.type_region_is_padding(
                            &member.ty,
                            member_start,
                            overlap_start,
                            overlap_end - overlap_start,
                        )
                    })
                })
            }
            _ => false,
        }
    }

    fn effective_region_allows_access(
        &self,
        region: &EffectiveTypeRegion,
        access_start: usize,
        access_size: usize,
        access_ty: &CType,
    ) -> bool {
        let Some(region_end) = region.start.checked_add(region.size) else {
            return false;
        };
        let Some(access_end) = access_start.checked_add(access_size) else {
            return false;
        };
        if access_start >= region.start && access_end <= region_end {
            return self.type_region_allows_effective_access(
                &region.ty,
                region.start,
                access_start,
                access_size,
                access_ty,
            );
        }
        if region.start >= access_start && region_end <= access_end {
            if let Some(element_ty) = region.coalesced_element_type.as_ref()
                && let Some(element_size) = self.type_size_of(element_ty)
                && element_size != 0
                && region.size % element_size == 0
                && (0..region.size / element_size).all(|index| {
                    self.type_region_allows_effective_access(
                        access_ty,
                        access_start,
                        region.start + index * element_size,
                        element_size,
                        element_ty,
                    )
                })
            {
                return true;
            }
            return self.type_region_allows_effective_access(
                access_ty,
                access_start,
                region.start,
                region.size,
                &region.ty,
            );
        }
        if let Some(element_ty) = region.coalesced_element_type.as_ref()
            && let Some(element_size) = self.type_size_of(element_ty)
            && element_size != 0
            && region.size % element_size == 0
        {
            return (0..region.size / element_size)
                .filter_map(|index| {
                    let element_start = region.start + index * element_size;
                    let element_end = element_start + element_size;
                    (access_start < element_end && element_start < access_end)
                        .then_some((element_start, element_end))
                })
                .all(|(element_start, element_end)| {
                    if element_start >= access_start && element_end <= access_end {
                        self.type_region_allows_effective_access(
                            access_ty,
                            access_start,
                            element_start,
                            element_size,
                            element_ty,
                        )
                    } else if access_start >= element_start && access_end <= element_end {
                        self.type_region_allows_effective_access(
                            element_ty,
                            element_start,
                            access_start,
                            access_size,
                            access_ty,
                        )
                    } else {
                        false
                    }
                });
        }
        false
    }

    fn check_dynamic_effective_type_read(
        &self,
        object: &ObjectState,
        start: usize,
        size: usize,
        access_ty: &CType,
        access_root_ty: &CType,
        member_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if object.storage_duration != StorageDuration::Dynamic
            || access_ty.is_character()
            || size == 0
        {
            return Ok(());
        }
        let end = start.saturating_add(size);
        let first = object
            .effective_types
            .partition_point(|region| region.start.saturating_add(region.size) <= start);
        for region in object.effective_types[first..]
            .iter()
            .take_while(|region| region.start < end)
        {
            if !self.effective_region_allows_access(region, start, size, access_ty)
                && !self.union_member_path_allows_effective_access(
                    access_root_ty,
                    member_path,
                    region.coalesced_element_type.as_ref().unwrap_or(&region.ty),
                    access_ty,
                )
            {
                return Err(Diagnostic::ub(
                    format!(
                        "access through an lvalue of type {} is incompatible with the allocated object's effective type {}",
                        access_ty, region.ty
                    ),
                    span,
                    Some("6.5p6-7"),
                ));
            }
        }
        Ok(())
    }

    fn union_member_path_allows_effective_access(
        &self,
        root_ty: &CType,
        member_path: &[String],
        stored_ty: &CType,
        access_ty: &CType,
    ) -> bool {
        let mut current_ty = root_ty;
        for (path_index, member_name) in member_path.iter().enumerate() {
            while let CType::Array(inner, _) = current_ty.unqualified() {
                current_ty = inner;
            }
            let Some(record) = self.record_type(current_ty) else {
                return false;
            };
            let Some(selected) = record
                .members
                .iter()
                .find(|member| member.storage_name == *member_name)
            else {
                return false;
            };
            if record.kind == crate::types::RecordKind::Union {
                let selected_access_ty = self
                    .storage_path_type(&selected.ty, &member_path[path_index + 1..])
                    .unwrap_or_else(|| selected.ty.clone());
                if self.compatible_object_layout_types(&selected_access_ty, access_ty)
                    && record
                        .members
                        .iter()
                        .any(|member| self.effective_type_alias_allowed(&member.ty, stored_ty))
                {
                    return true;
                }
            }
            current_ty = &selected.ty;
        }
        false
    }

    fn split_effective_region_around_write(
        &self,
        region: &EffectiveTypeRegion,
        write_start: usize,
        write_size: usize,
        out: &mut Vec<EffectiveTypeRegion>,
    ) {
        let region_end = region.start.saturating_add(region.size);
        let write_end = write_start.saturating_add(write_size);
        if write_start >= region_end || region.start >= write_end {
            out.push(region.clone());
            return;
        }
        match region.ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return;
                };
                if stride == 0 || *len == 0 {
                    return;
                }
                let overlap_start = write_start.max(region.start) - region.start;
                let overlap_end = write_end.min(region_end) - region.start;
                let first = (overlap_start / stride).min(*len);
                let last = overlap_end
                    .saturating_add(stride - 1)
                    .checked_div(stride)
                    .unwrap_or(*len)
                    .min(*len);
                if first != 0 {
                    out.push(EffectiveTypeRegion {
                        start: region.start,
                        size: first * stride,
                        ty: CType::array_of((**inner).clone(), first),
                        coalesced_element_type: region.coalesced_element_type.clone(),
                    });
                }
                for index in first..last {
                    let child = EffectiveTypeRegion {
                        start: region.start + index * stride,
                        size: stride,
                        ty: (**inner).clone(),
                        coalesced_element_type: None,
                    };
                    let child_end = child.start + child.size;
                    if !(write_start <= child.start && write_end >= child_end) {
                        self.split_effective_region_around_write(
                            &child,
                            write_start,
                            write_size,
                            out,
                        );
                    }
                }
                if last < *len {
                    out.push(EffectiveTypeRegion {
                        start: region.start + last * stride,
                        size: (*len - last) * stride,
                        ty: CType::array_of((**inner).clone(), *len - last),
                        coalesced_element_type: region.coalesced_element_type.clone(),
                    });
                }
            }
            CType::Struct(_, _) => {
                if let Some(record) = self.record_type(&region.ty) {
                    for member in &record.members {
                        if member.bit_width.is_some() {
                            continue;
                        }
                        let Some(member_size) = self.type_size_of(&member.ty) else {
                            continue;
                        };
                        self.split_effective_region_around_write(
                            &EffectiveTypeRegion {
                                start: region.start.saturating_add(member.offset),
                                size: member_size,
                                ty: member.ty.clone(),
                                coalesced_element_type: None,
                            },
                            write_start,
                            write_size,
                            out,
                        );
                    }
                }
            }
            CType::Union(_, _) => {}
            _ => {}
        }
    }

    fn coalesce_effective_type_regions(&self, regions: &mut Vec<EffectiveTypeRegion>) {
        if !regions.is_sorted_by_key(|region| region.start) {
            regions.sort_by_key(|region| region.start);
        }
        let mut coalesced: Vec<EffectiveTypeRegion> = Vec::with_capacity(regions.len());
        for region in regions.drain(..) {
            let Some(previous) = coalesced.last_mut() else {
                coalesced.push(region);
                continue;
            };
            if previous.start == region.start
                && previous.size == region.size
                && previous.ty == region.ty
            {
                continue;
            }
            let Some(previous_end) = previous.start.checked_add(previous.size) else {
                coalesced.push(region);
                continue;
            };
            if previous_end != region.start {
                coalesced.push(region);
                continue;
            }
            let previous_element = previous
                .coalesced_element_type
                .as_ref()
                .unwrap_or(&previous.ty);
            let region_element = region.coalesced_element_type.as_ref().unwrap_or(&region.ty);
            if previous_element != region_element {
                coalesced.push(region);
                continue;
            }
            let Some(element_size) = self.type_size_of(previous_element) else {
                coalesced.push(region);
                continue;
            };
            if element_size == 0
                || previous.size % element_size != 0
                || region.size % element_size != 0
            {
                coalesced.push(region);
                continue;
            }
            let Some(size) = previous.size.checked_add(region.size) else {
                coalesced.push(region);
                continue;
            };
            let count = size / element_size;
            let previous_element = previous_element.clone();
            previous.size = size;
            previous.ty = CType::array_of(previous_element.clone(), count);
            previous.coalesced_element_type = Some(previous_element);
        }
        *regions = coalesced;
    }

    fn merge_effective_type_regions_at(
        &self,
        regions: &mut Vec<EffectiveTypeRegion>,
        index: usize,
    ) -> bool {
        if index + 1 >= regions.len() {
            return false;
        }
        let merge = {
            let (left, right) = regions.split_at_mut(index + 1);
            let previous = &mut left[index];
            let region = &right[0];
            if previous.start == region.start
                && previous.size == region.size
                && previous.ty == region.ty
            {
                true
            } else if previous.start.checked_add(previous.size) == Some(region.start) {
                let previous_element = previous
                    .coalesced_element_type
                    .as_ref()
                    .unwrap_or(&previous.ty);
                let region_element = region.coalesced_element_type.as_ref().unwrap_or(&region.ty);
                if previous_element != region_element {
                    false
                } else if let Some(element_size) = self.type_size_of(previous_element) {
                    if element_size == 0
                        || previous.size % element_size != 0
                        || region.size % element_size != 0
                    {
                        false
                    } else if let Some(size) = previous.size.checked_add(region.size) {
                        let element = previous_element.clone();
                        previous.size = size;
                        previous.ty = CType::array_of(element.clone(), size / element_size);
                        previous.coalesced_element_type = Some(element);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };
        if merge {
            regions.remove(index + 1);
        }
        merge
    }

    fn update_dynamic_effective_types_after_store(
        &self,
        regions: &mut Vec<EffectiveTypeRegion>,
        start: usize,
        size: usize,
        access_ty: &CType,
    ) {
        if size == 0 {
            return;
        }
        let end = start.saturating_add(size);
        let first =
            regions.partition_point(|region| region.start.saturating_add(region.size) <= start);
        let last = first + regions[first..].partition_point(|region| region.start < end);
        let replaced = regions.drain(first..last).collect::<Vec<_>>();
        let mut replacement = Vec::with_capacity(replaced.len().saturating_add(1));
        for region in &replaced {
            self.split_effective_region_around_write(region, start, size, &mut replacement);
        }
        if !access_ty.is_character() {
            replacement.push(EffectiveTypeRegion {
                start,
                size,
                ty: access_ty.unqualified().clone(),
                coalesced_element_type: None,
            });
        }
        self.coalesce_effective_type_regions(&mut replacement);
        let inserted = replacement.len();
        regions.splice(first..first, replacement);

        let mut index = first.saturating_sub(1);
        let mut pairs_to_check = inserted.saturating_add(2);
        while index + 1 < regions.len() && pairs_to_check != 0 {
            if !self.merge_effective_type_regions_at(regions, index) {
                index += 1;
            }
            pairs_to_check -= 1;
        }
    }

    pub(super) fn update_object_effective_types_after_store(
        &mut self,
        objects: &mut ObjectFrames,
        object_id: ObjectId,
        start: usize,
        size: usize,
        access_ty: &CType,
    ) {
        let Some(object) = self.lookup_active_object_mut(objects, object_id) else {
            return;
        };
        if object.storage_duration != StorageDuration::Dynamic || size == 0 {
            return;
        }
        let mut regions = std::mem::take(&mut object.effective_types);
        self.update_dynamic_effective_types_after_store(&mut regions, start, size, access_ty);
        if let Some(object) = self.lookup_active_object_mut(objects, object_id) {
            object.effective_types = regions;
        }
    }

    fn collect_exact_subobject_types(
        &self,
        ty: &CType,
        base: usize,
        start: usize,
        size: usize,
        out: &mut Vec<CType>,
    ) {
        let Some(ty_size) = self.type_size_of(ty) else {
            return;
        };
        let Some(end) = start.checked_add(size) else {
            return;
        };
        let Some(ty_end) = base.checked_add(ty_size) else {
            return;
        };
        if start < base || end > ty_end {
            return;
        }
        if start == base && size == ty_size {
            out.push(ty.clone());
        }
        match ty.unqualified() {
            CType::Array(inner, len) => {
                let Some(stride) = self.type_size_of(inner) else {
                    return;
                };
                if stride == 0 {
                    return;
                }
                let index = (start - base) / stride;
                if index < *len {
                    self.collect_exact_subobject_types(
                        inner,
                        base + index * stride,
                        start,
                        size,
                        out,
                    );
                }
            }
            CType::Struct(_, _) => {
                if let Some(record) = self.record_type(ty) {
                    for member in &record.members {
                        if member.bit_width.is_none() {
                            self.collect_exact_subobject_types(
                                &member.ty,
                                base.saturating_add(member.offset),
                                start,
                                size,
                                out,
                            );
                        }
                    }
                }
            }
            CType::Union(_, _) => {
                if let Some(record) = self.record_type(ty) {
                    for member in &record.members {
                        if member.bit_width.is_none() {
                            self.collect_exact_subobject_types(&member.ty, base, start, size, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn choose_copied_effective_type(
        &self,
        ty: &CType,
        base: usize,
        start: usize,
        size: usize,
        source_pointee: Option<&CType>,
    ) -> Option<CType> {
        let mut candidates = Vec::new();
        self.collect_exact_subobject_types(ty, base, start, size, &mut candidates);
        if let Some(source_pointee) = source_pointee {
            if source_pointee.is_character() {
                return None;
            }
            if !matches!(source_pointee.unqualified(), CType::Void) {
                if let Some(candidate) = candidates.iter().find(|candidate| {
                    self.cross_unit_tagged_type_compatible(
                        candidate.unqualified(),
                        source_pointee.unqualified(),
                    )
                }) {
                    return Some(candidate.unqualified().clone());
                }
            }
        }
        candidates
            .into_iter()
            .next()
            .map(|candidate| candidate.unqualified().clone())
    }

    pub(super) fn copied_effective_type_regions(
        &self,
        source: &ObjectState,
        start: usize,
        size: usize,
        source_pointee: Option<&CType>,
    ) -> Vec<EffectiveTypeRegion> {
        if size == 0 || source_pointee.is_some_and(CType::is_character) {
            return Vec::new();
        }
        if source.storage_duration != StorageDuration::Dynamic {
            return self
                .choose_copied_effective_type(&source.ty, 0, start, size, source_pointee)
                .map(|ty| {
                    vec![EffectiveTypeRegion {
                        start,
                        size,
                        ty,
                        coalesced_element_type: None,
                    }]
                })
                .unwrap_or_default();
        }

        let end = start.saturating_add(size);
        let mut copied = source
            .effective_types
            .iter()
            .filter(|region| {
                region.start >= start && region.start.saturating_add(region.size) <= end
            })
            .cloned()
            .collect::<Vec<_>>();
        if copied.is_empty() {
            if let Some(region) = source.effective_types.iter().find(|region| {
                start >= region.start
                    && end <= region.start.saturating_add(region.size)
                    && self
                        .choose_copied_effective_type(
                            &region.ty,
                            region.start,
                            start,
                            size,
                            source_pointee,
                        )
                        .is_some()
            }) {
                if let Some(ty) = self.choose_copied_effective_type(
                    &region.ty,
                    region.start,
                    start,
                    size,
                    source_pointee,
                ) {
                    copied.push(EffectiveTypeRegion {
                        start,
                        size,
                        ty,
                        coalesced_element_type: None,
                    });
                }
            }
        }
        copied
    }

    pub(super) fn dynamic_effective_types_after_copy(
        &self,
        object: &ObjectState,
        dest_start: usize,
        size: usize,
        source_start: usize,
        source_regions: Vec<EffectiveTypeRegion>,
    ) -> Option<Vec<EffectiveTypeRegion>> {
        if object.storage_duration != StorageDuration::Dynamic || size == 0 {
            return None;
        }
        let mut updated = Vec::with_capacity(
            object
                .effective_types
                .len()
                .saturating_add(source_regions.len()),
        );
        for region in &object.effective_types {
            self.split_effective_region_around_write(region, dest_start, size, &mut updated);
        }
        for region in source_regions.into_iter().filter_map(|region| {
            let relative = region.start.checked_sub(source_start)?;
            Some(EffectiveTypeRegion {
                start: dest_start.checked_add(relative)?,
                size: region.size,
                ty: region.ty,
                coalesced_element_type: region.coalesced_element_type,
            })
        }) {
            let insertion = updated.partition_point(|existing| existing.start <= region.start);
            updated.insert(insertion, region);
        }
        self.coalesce_effective_type_regions(&mut updated);
        Some(updated)
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
            .map(|(_, ty, _)| ty)
    }

    pub(super) fn resolve_member_access(
        &self,
        base_ty: &CType,
        member: &str,
        span: Span,
    ) -> Result<(Rc<Vec<String>>, CType, Option<u8>), Diagnostic> {
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
            return Ok((
                cached.path.clone(),
                cached.ty.clone(),
                cached.bit_field_width,
            ));
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
        let result = (
            resolved.path.clone(),
            resolved.ty.clone(),
            resolved.bit_field_width,
        );
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
            {
                if let Some(mut tail) = self.resolve_visible_member_chain(&candidate.ty, member) {
                    let mut chain = vec![candidate];
                    chain.append(&mut tail);
                    return Some(chain);
                }
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

    pub(super) fn check_pointer_cast_ub(
        &self,
        value: &TypedValue,
        target: &CType,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let CType::Pointer(target_inner) = target.unqualified() else {
            return Ok(());
        };
        let CType::Pointer(source_inner) = value.ty.unqualified() else {
            return Ok(());
        };
        if matches!(source_inner.unqualified(), CType::Function(..))
            && matches!(target_inner.unqualified(), CType::Function(..))
        {
            return Ok(());
        }
        let pointer = value.as_pointer(span)?;
        if pointer.is_null() {
            return Ok(());
        }
        let Some(byte_offset) = self.pointer_byte_offset(&pointer, source_inner, objects) else {
            return Ok(());
        };
        if let Some(align) = self.type_align_of(target_inner) {
            if byte_offset % align != 0 {
                return Err(Diagnostic::ub(
                    format!(
                        "pointer conversion yields a {} that is not correctly aligned",
                        target
                    ),
                    span,
                    Some("6.3.2.3p7"),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn pointer_targets_initial_member_chain(
        &self,
        pointer: &PointerValue,
        target: &CType,
        objects: &ObjectFrames,
    ) -> bool {
        let Some(root_ty) = self.pointer_root_type(pointer, objects) else {
            return false;
        };
        match root_ty.unqualified() {
            CType::Array(inner, _) => {
                self.path_reaches_initial_container(inner, &pointer.member_path, target)
            }
            _ => self.path_reaches_initial_container(root_ty, &pointer.member_path, target),
        }
    }

    fn path_reaches_initial_container(
        &self,
        root: &CType,
        path: &[String],
        target: &CType,
    ) -> bool {
        let mut current = root.clone();
        for index in 0..=path.len() {
            if self.cross_unit_tagged_type_compatible(&current, target)
                && self.path_is_initial_member_chain(&current, &path[index..])
            {
                return true;
            }
            let Some(name) = path.get(index) else {
                break;
            };
            let Some(record) = self.record_type(&current) else {
                return false;
            };
            let Some(member_index) = record
                .members
                .iter()
                .position(|member| member.storage_name == *name)
            else {
                return false;
            };
            current = self.qualified_member_type(&current, &record.members[member_index]);
        }
        false
    }

    pub(super) fn path_is_initial_member_chain(&self, ty: &CType, path: &[String]) -> bool {
        if path.is_empty() {
            return true;
        }
        let member_name = &path[0];
        let Some(record) = self.record_type(ty) else {
            return false;
        };
        let Some(member_index) = record
            .members
            .iter()
            .position(|member| member.storage_name == *member_name)
        else {
            return false;
        };
        let member_ty = &record.members[member_index].ty;
        match record.kind {
            crate::types::RecordKind::Struct if member_index != 0 => false,
            crate::types::RecordKind::Struct | crate::types::RecordKind::Union => {
                self.path_is_initial_member_chain(member_ty, &path[1..])
            }
        }
    }

    pub(super) fn pointer_byte_offset(
        &self,
        pointer: &PointerValue,
        pointee_ty: &CType,
        objects: &ObjectFrames,
    ) -> Option<usize> {
        let root_ty = self.pointer_root_type(pointer, objects)?;
        self.pointer_byte_offset_from_type(pointer, pointee_ty, root_ty)
    }

    pub(super) fn pointer_byte_offset_from_root_type(
        &self,
        pointer: &PointerValue,
        pointee_ty: &CType,
    ) -> Option<usize> {
        let object_id = pointer.object?;
        let root_ty = pointer
            .designated_root_ty
            .as_deref()
            .or_else(|| self.object_type_registry.get(&object_id))?;
        self.pointer_byte_offset_from_type(pointer, pointee_ty, root_ty)
    }

    fn pointer_member_byte_limit(
        &self,
        pointer: &PointerValue,
        current_start: usize,
        object_size: usize,
        objects: &ObjectFrames,
    ) -> Option<usize> {
        if pointer.member_path.is_empty() {
            return None;
        }
        let root_ty = self.pointer_root_type(pointer, objects)?;
        let member_ty = self.storage_path_type(root_ty, &pointer.member_path)?;
        let object_remaining = object_size.checked_sub(current_start)?;
        if matches!(member_ty.unqualified(), CType::Array(_, 0)) {
            // A flexible array member behaves as the longest array that fits in the
            // containing allocated object (C11 6.7.2.1p18).
            return Some(object_remaining);
        }
        let member_size = self.type_size_of(&member_ty)?;
        let element_ty = match member_ty.unqualified() {
            CType::Array(inner, _) => &**inner,
            _ => &member_ty,
        };
        let element_size = self.type_size_of(element_ty)?;
        // A byte offset override already identifies the selected array element. Applying the
        // logical element offset again would incorrectly consume bytes from its member.
        let offset = if pointer.byte_offset_override.is_some() {
            0
        } else {
            usize::try_from(pointer.offset).ok()?
        };
        let consumed = offset.checked_mul(element_size)?;
        let member_remaining = member_size.checked_sub(consumed)?;
        Some(member_remaining.min(object_remaining))
    }

    fn pointer_byte_offset_from_type(
        &self,
        pointer: &PointerValue,
        pointee_ty: &CType,
        root_ty: &CType,
    ) -> Option<usize> {
        if let Some(byte_offset) = pointer.byte_offset_override {
            return Some(byte_offset);
        }
        let element_size = self.type_size_of(pointee_ty).or_else(|| {
            self.pointer_element_type(root_ty, pointer)
                .and_then(|ty| self.type_size_of(&ty))
        })?;
        if pointer.member_path.is_empty() {
            let base_offset = usize::try_from(pointer.base_offset).ok()?;
            let element_offset = usize::try_from(pointer.offset)
                .ok()?
                .checked_mul(element_size)?;
            let base_stride = match root_ty.unqualified() {
                CType::Array(inner, _) => self.type_size_of(inner)?,
                _ => element_size,
            };
            return base_offset
                .checked_mul(base_stride)?
                .checked_add(element_offset);
        }
        let member_offset = self.member_path_offset(root_ty, &pointer.member_path)?;
        let element_offset = usize::try_from(pointer.offset)
            .ok()?
            .checked_mul(element_size)?;
        let outer_offset = match root_ty.unqualified() {
            CType::Array(inner, _) => {
                let index = usize::try_from(pointer.base_offset).ok()?;
                index.checked_mul(self.type_size_of(inner)?)?
            }
            _ if pointer.base_offset == 0 => 0,
            _ => {
                let index = usize::try_from(pointer.base_offset).ok()?;
                index.checked_mul(self.type_size_of(root_ty)?)?
            }
        };
        outer_offset
            .checked_add(member_offset)?
            .checked_add(element_offset)
    }

    fn pointer_element_type(&self, root_ty: &CType, pointer: &PointerValue) -> Option<CType> {
        if pointer.member_path.is_empty() {
            match root_ty.unqualified() {
                CType::Array(inner, _) => Some((**inner).clone()),
                _ => Some(root_ty.clone()),
            }
        } else {
            self.storage_path_type(root_ty, &pointer.member_path)
        }
    }

    pub(super) fn member_path_offset(&self, ty: &CType, path: &[String]) -> Option<usize> {
        if path.is_empty() {
            return Some(0);
        }
        match ty.unqualified() {
            CType::Array(inner, _) => self.member_path_offset(inner, path),
            CType::Struct(_, _) | CType::Union(_, _) => {
                let record = self.record_type(ty)?;
                let member = record
                    .members
                    .iter()
                    .find(|member| member.storage_name == path[0])?;
                let inner = self.member_path_offset(&member.ty, &path[1..])?;
                member.offset.checked_add(inner)
            }
            _ => None,
        }
    }

    pub(super) fn pointer_root_type<'b>(
        &'b self,
        pointer: &'b PointerValue,
        objects: &'b ObjectFrames,
    ) -> Option<&'b CType> {
        if let Some(root_ty) = pointer.designated_root_ty.as_ref() {
            Some(root_ty)
        } else {
            let object_id = pointer.object?;
            self.lookup_object(objects, object_id)
                .map(|object| &object.ty)
        }
    }

    pub(super) fn byte_region_from_pointer(
        &self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        self.byte_region_from_pointer_with_options(pointer, size, span, objects, false)
    }

    pub(super) fn byte_region_from_pointer_allow_reserved(
        &self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        self.byte_region_from_pointer_with_options(pointer, size, span, objects, true)
    }

    fn byte_region_from_pointer_with_options(
        &self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
        allow_reserved_buffer: bool,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        let Some(object_id) = pointer.object else {
            return Err(Diagnostic::ub(
                "null pointer passed where an object region is required",
                span,
                Some("7.1.4"),
            ));
        };
        let object = self.lookup_object(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "access through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        if !object.alive {
            return Err(Diagnostic::ub(
                "access through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            ));
        }
        let start = self
            .pointer_byte_offset(pointer, &CType::UnsignedChar, objects)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "pointer does not designate a valid byte-addressable region of the object",
                    span,
                    Some("7.1.4"),
                )
            })?;
        let end = start.checked_add(size).ok_or_else(|| {
            Diagnostic::ub(
                "requested byte access is out of supported range",
                span,
                Some("7.1.4"),
            )
        })?;
        if end > object.byte_size {
            return Err(Diagnostic::ub(
                format!(
                    "requested byte access of {} byte(s) starting at offset {} exceeds the {}-byte object",
                    size, start, object.byte_size
                ),
                span,
                Some("7.1.4"),
            ));
        }
        if let (Some(domain_start), Some(domain_ty)) = (
            pointer.arithmetic_domain_start,
            pointer.designated_root_ty.as_deref(),
        ) {
            let domain_end = self
                .type_size_of(domain_ty)
                .and_then(|size| domain_start.checked_add(size))
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "pointer does not designate a valid byte-addressable subobject",
                        span,
                        Some("7.1.4"),
                    )
                })?;
            if start < domain_start || end > domain_end {
                return Err(Diagnostic::ub(
                    format!(
                        "requested byte access of {} byte(s) exceeds the designated subobject",
                        size
                    ),
                    span,
                    Some("7.1.4"),
                ));
            }
        }
        if !allow_reserved_buffer
            && self.stream_buffer_region_overlaps(object_id, start, size, objects)
        {
            return Err(self.stream_buffer_use_diag(span));
        }
        if !pointer.member_path.is_empty() {
            let available = self
                .pointer_member_byte_limit(pointer, start, object.byte_size, objects)
                .unwrap_or(0);
            if size > available {
                return Err(Diagnostic::ub(
                    format!(
                        "requested byte access of {} byte(s) exceeds the {}-byte designated subobject",
                        size, available
                    ),
                    span,
                    Some("7.1.4"),
                ));
            }
        }
        Ok((object_id, start, object.byte_size))
    }

    pub(super) fn lvalue_byte_range(
        &self,
        lvalue: &LValue,
        effective_ty: &CType,
        objects: &ObjectFrames,
    ) -> Option<(ObjectId, usize, usize)> {
        if let Some(start) = lvalue.byte_offset_override {
            let size = self.type_size_of(effective_ty)?;
            return Some((lvalue.object, start, size));
        }
        if lvalue.base_offset == 0
            && lvalue.offset == 0
            && lvalue.member_path.is_empty()
            && lvalue.designated_root_ty.is_none()
        {
            let object = self.lookup_object(objects, lvalue.object)?;
            if self.compatible_object_layout_types(&object.ty, effective_ty) {
                return Some((lvalue.object, 0, self.type_size_of(effective_ty)?));
            }
        }
        let default_root = self
            .lookup_object(objects, lvalue.object)
            .map(|object| &object.ty)?;
        let root_ty = lvalue.designated_root_ty.as_deref().unwrap_or(default_root);
        let pointer = PointerValue {
            object: Some(lvalue.object),
            base_offset: lvalue.base_offset,
            offset: lvalue.offset,
            member_path: lvalue.member_path.clone(),
            designated_root_ty: lvalue.designated_root_ty.clone(),
            byte_offset_override: lvalue.byte_offset_override,
            arithmetic_domain_start: lvalue.arithmetic_domain_start,
        };
        let start = self.pointer_byte_offset_from_type(&pointer, effective_ty, root_ty)?;
        let size = self.type_size_of(effective_ty)?;
        Some((lvalue.object, start, size))
    }

    pub(super) fn load_member_from_aggregate(
        &self,
        stored: &StoredValue,
        ty: &CType,
        path: &[String],
        span: Span,
    ) -> Result<TypedValue, Diagnostic> {
        self.reject_simple_inactive_union_member_read(stored, ty, path, span)?;
        let (stored, member_ty) = self.extract_stored_subobject_view(stored, ty, path, span)?;
        Ok(self.typed_value_from_stored_view(&member_ty, stored))
    }

    pub(super) fn indeterminate_stored_value(&self, ty: &CType) -> StoredValue {
        match ty.unqualified() {
            CType::Array(inner, len) => StoredValue::Array(
                (0..*len)
                    .map(|_| self.indeterminate_stored_value(inner))
                    .collect(),
            ),
            CType::Struct(_, _) => {
                let members = self
                    .record_type(ty)
                    .map(|record| {
                        record
                            .members
                            .iter()
                            .map(|member| {
                                (
                                    member.storage_name.as_str().into(),
                                    self.indeterminate_stored_value(&member.ty),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                StoredValue::Record(members)
            }
            CType::Union(_, _) => {
                let members = self
                    .record_type(ty)
                    .map(|record| {
                        record
                            .members
                            .iter()
                            .map(|member| {
                                (
                                    member.storage_name.as_str().into(),
                                    self.indeterminate_stored_value(&member.ty),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let byte_len = self.type_size_of(ty).unwrap_or(0);
                StoredValue::Union {
                    active_member: None,
                    members,
                    bytes: vec![ByteCell::Indeterminate; byte_len],
                }
            }
            _ => StoredValue::Indeterminate,
        }
    }

    pub(super) fn zero_stored_value(&self, ty: &CType) -> StoredValue {
        match ty.unqualified() {
            CType::Array(inner, len) => {
                StoredValue::Array((0..*len).map(|_| self.zero_stored_value(inner)).collect())
            }
            CType::Struct(_, _) => StoredValue::Record(
                self.record_type(ty)
                    .map(|record| {
                        record
                            .members
                            .iter()
                            .map(|member| {
                                (
                                    member.storage_name.as_str().into(),
                                    self.zero_stored_value(&member.ty),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            CType::Union(_, _) => {
                let members = self
                    .record_type(ty)
                    .map(|record| {
                        record
                            .members
                            .iter()
                            .map(|member| {
                                (
                                    member.storage_name.as_str().into(),
                                    self.zero_stored_value(&member.ty),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let active_member = self.record_type(ty).and_then(|record| {
                    record
                        .members
                        .iter()
                        .find(|member| member.bit_width != Some(0))
                        .map(|member| member.storage_name.as_str().into())
                });
                let byte_len = self.type_size_of(ty).unwrap_or(0);
                StoredValue::Union {
                    active_member,
                    members,
                    bytes: vec![ByteCell::Known(0); byte_len],
                }
            }
            _ => StoredValue::Scalar(self.zero_value(ty)),
        }
    }

    pub(super) fn stored_value_is_determinate(&self, value: &StoredValue) -> bool {
        match value {
            StoredValue::Scalar(value) => !value.indeterminate,
            StoredValue::Array(values) => values
                .iter()
                .all(|value| self.stored_value_is_determinate(value)),
            StoredValue::ObjectRepresentation(bytes) => {
                bytes.iter().all(|byte| matches!(byte, ByteCell::Known(_)))
            }
            StoredValue::Record(values) => values
                .iter()
                .all(|(_, value)| self.stored_value_is_determinate(value)),
            StoredValue::Union { bytes, .. } => {
                bytes.iter().all(|byte| matches!(byte, ByteCell::Known(_)))
            }
            StoredValue::Indeterminate => false,
        }
    }

    pub(super) fn typed_value_from_stored(&self, ty: &CType, stored: &StoredValue) -> TypedValue {
        let value_ty = Self::unqualified_value_type(ty);
        match stored {
            StoredValue::Scalar(value) => {
                let mut value = value.clone();
                value.ty = value_ty;
                value
            }
            StoredValue::Indeterminate => TypedValue::indeterminate_for(value_ty),
            StoredValue::Array(_)
            | StoredValue::ObjectRepresentation(_)
            | StoredValue::Record(_)
            | StoredValue::Union { .. } => TypedValue {
                ty: value_ty,
                data: ValueData::Aggregate(Box::new(stored.clone())),
                restrict_source: None,
                indeterminate: !self.stored_value_is_determinate(stored),
                missing_return: false,
            },
        }
    }

    fn typed_value_from_stored_view(&self, ty: &CType, stored: StoredValueView<'_>) -> TypedValue {
        let value_ty = Self::unqualified_value_type(ty);
        match stored {
            StoredValueView::Borrowed(StoredValue::Scalar(value)) => {
                let mut value = value.clone();
                value.ty = value_ty;
                value
            }
            StoredValueView::Owned(StoredValue::Scalar(mut value)) => {
                value.ty = value_ty;
                value
            }
            StoredValueView::Borrowed(StoredValue::Indeterminate)
            | StoredValueView::Owned(StoredValue::Indeterminate) => {
                TypedValue::indeterminate_for(value_ty)
            }
            StoredValueView::Borrowed(stored) => TypedValue {
                ty: value_ty,
                data: ValueData::Aggregate(Box::new(stored.clone())),
                restrict_source: None,
                indeterminate: !self.stored_value_is_determinate(stored),
                missing_return: false,
            },
            StoredValueView::Owned(stored) => {
                let indeterminate = !self.stored_value_is_determinate(&stored);
                TypedValue {
                    ty: value_ty,
                    data: ValueData::Aggregate(Box::new(stored)),
                    restrict_source: None,
                    indeterminate,
                    missing_return: false,
                }
            }
        }
    }

    pub(super) fn stored_value_from_typed_value(
        &self,
        value: TypedValue,
        target: &CType,
        span: Span,
    ) -> Result<StoredValue, Diagnostic> {
        if matches!(
            target.unqualified(),
            CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _)
        ) {
            let ValueData::Aggregate(stored) = value.data else {
                return Err(Diagnostic::error(
                    format!("cannot convert {} to {}", value.ty, target),
                    span,
                ));
            };
            return Ok(*stored);
        }
        Ok(StoredValue::Scalar(value))
    }

    pub(super) fn resolve_lvalue_array_base<'b>(
        &self,
        stored: &'b StoredValue,
        ty: &CType,
        lvalue: &LValue,
        span: Span,
    ) -> Result<(&'b StoredValue, CType, isize), Diagnostic> {
        if let CType::Array(inner, _) = ty.unqualified() {
            if self.compatible_object_layout_types(inner, &lvalue.ty) {
                let offset = lvalue
                    .base_offset
                    .checked_add(lvalue.offset)
                    .ok_or_else(|| {
                        Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                    })?;
                let (stored, element_ty) = self.apply_lvalue_offset(stored, ty, offset, span)?;
                return Ok((stored, element_ty, 0));
            }
            if lvalue.base_offset != 0
                || lvalue.offset != 0
                || !matches!(lvalue.ty.unqualified(), CType::Array(_, _))
            {
                let (stored, element_ty) =
                    self.apply_lvalue_offset(stored, ty, lvalue.base_offset, span)?;
                return Ok((stored, element_ty, lvalue.offset));
            }
        }
        if lvalue.base_offset != 0 {
            let (stored, element_ty) =
                self.apply_lvalue_offset(stored, ty, lvalue.base_offset, span)?;
            return Ok((stored, element_ty, lvalue.offset));
        }
        Ok((stored, ty.clone(), lvalue.offset))
    }

    fn resolve_lvalue_array_base_mut<'b>(
        &self,
        stored: &'b mut StoredValue,
        ty: &CType,
        lvalue: &LValue,
        span: Span,
    ) -> Result<(&'b mut StoredValue, CType, isize), Diagnostic> {
        if let CType::Array(inner, _) = ty.unqualified() {
            if self.compatible_object_layout_types(inner, &lvalue.ty) {
                let offset = lvalue
                    .base_offset
                    .checked_add(lvalue.offset)
                    .ok_or_else(|| {
                        Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                    })?;
                let stored = self.apply_lvalue_offset_mut(stored, ty, offset, span)?;
                return Ok((stored, (**inner).clone(), 0));
            }
            if lvalue.base_offset != 0
                || lvalue.offset != 0
                || !matches!(lvalue.ty.unqualified(), CType::Array(_, _))
            {
                let stored = self.apply_lvalue_offset_mut(stored, ty, lvalue.base_offset, span)?;
                return Ok((stored, (**inner).clone(), lvalue.offset));
            }
        }
        if lvalue.base_offset != 0 {
            let stored = self.apply_lvalue_offset_mut(stored, ty, lvalue.base_offset, span)?;
            let element_ty = ty
                .element_type()
                .cloned()
                .ok_or_else(|| Diagnostic::error("array type has no element type", span))?;
            return Ok((stored, element_ty, lvalue.offset));
        }
        Ok((stored, ty.clone(), lvalue.offset))
    }

    pub(super) fn resolve_lvalue_storage_mut<'b>(
        &self,
        stored: &'b mut StoredValue,
        ty: &CType,
        lvalue: &LValue,
        span: Span,
    ) -> Result<&'b mut StoredValue, Diagnostic> {
        let (stored, base_ty, tail_offset) =
            self.resolve_lvalue_array_base_mut(stored, ty, lvalue, span)?;
        let (stored, base_ty, tail_offset) = if !lvalue.member_path.is_empty()
            && matches!(base_ty.unqualified(), CType::Array(_, _))
        {
            let stored = self.apply_lvalue_offset_mut(stored, &base_ty, tail_offset, span)?;
            let element_ty = base_ty
                .element_type()
                .cloned()
                .ok_or_else(|| Diagnostic::error("array type has no element type", span))?;
            (stored, element_ty, 0)
        } else {
            (stored, base_ty, tail_offset)
        };
        let stored =
            self.resolve_member_storage_mut(stored, &base_ty, &lvalue.member_path, span)?;
        let subobject_ty = self
            .storage_path_type(&base_ty, &lvalue.member_path)
            .unwrap_or(base_ty);
        if tail_offset == 0 && self.compatible_object_layout_types(&subobject_ty, &lvalue.ty) {
            return Ok(stored);
        }
        self.apply_lvalue_offset_mut(stored, &subobject_ty, tail_offset, span)
    }

    pub(super) fn extract_stored_subobject_view<'s>(
        &'s self,
        stored: &'s StoredValue,
        ty: &CType,
        path: &[String],
        span: Span,
    ) -> Result<(StoredValueView<'s>, CType), Diagnostic> {
        if path.is_empty() {
            return Ok((StoredValueView::Borrowed(stored), ty.clone()));
        }
        let member_name = &path[0];
        match (stored, ty.unqualified()) {
            (StoredValue::Array(_), CType::Array(inner, _)) => {
                self.extract_stored_subobject_view(stored, inner, path, span)
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let slot = self.record_slot(values, member_name, span)?;
                self.extract_stored_subobject_view(slot, &member_ty, &path[1..], span)
            }
            (StoredValue::Union { .. }, CType::Union(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let slot = self.extract_union_member(stored, ty, member, span)?;
                if path.len() == 1 {
                    Ok((StoredValueView::Owned(slot), member_ty))
                } else {
                    let (slot, final_ty) =
                        self.extract_stored_subobject(&slot, &member_ty, &path[1..], span)?;
                    Ok((StoredValueView::Owned(slot), final_ty))
                }
            }
            _ => Err(Diagnostic::ub(
                "pointer or lvalue does not actually designate a structure or union object of the required type",
                span,
                Some("6.5.2.3"),
            )),
        }
    }

    pub(super) fn apply_lvalue_offset_view<'s>(
        &self,
        stored: StoredValueView<'s>,
        ty: &CType,
        offset: isize,
        span: Span,
    ) -> Result<(StoredValueView<'s>, CType), Diagnostic> {
        match stored {
            StoredValueView::Borrowed(stored) => {
                let (stored, ty) = self.apply_lvalue_offset(stored, ty, offset, span)?;
                Ok((StoredValueView::Borrowed(stored), ty))
            }
            StoredValueView::Owned(stored) => {
                let (stored, ty) = self.apply_owned_lvalue_offset(stored, ty, offset, span)?;
                Ok((StoredValueView::Owned(stored), ty))
            }
        }
    }

    pub(super) fn apply_lvalue_offset<'b>(
        &self,
        stored: &'b StoredValue,
        ty: &CType,
        offset: isize,
        span: Span,
    ) -> Result<(&'b StoredValue, CType), Diagnostic> {
        match (stored, ty.unqualified()) {
            (StoredValue::Array(values), CType::Array(inner, _)) => {
                let index = usize::try_from(offset).map_err(|_| {
                    Diagnostic::ub(
                        "array subscript is outside the bounds of the object",
                        span,
                        Some("6.5.6"),
                    )
                })?;
                let stored = values.get(index).ok_or_else(|| {
                    Diagnostic::ub(
                        "array subscript is outside the bounds of the object",
                        span,
                        Some("6.5.6"),
                    )
                })?;
                Ok((stored, (**inner).clone()))
            }
            (_, _) if offset == 0 => Ok((stored, ty.clone())),
            _ => Err(Diagnostic::ub(
                "pointer arithmetic produced an invalid scalar object offset",
                span,
                Some("6.5.6"),
            )),
        }
    }

    fn apply_lvalue_offset_mut<'b>(
        &self,
        stored: &'b mut StoredValue,
        ty: &CType,
        offset: isize,
        span: Span,
    ) -> Result<&'b mut StoredValue, Diagnostic> {
        match ty.unqualified() {
            CType::Array(_, _) => match stored {
                StoredValue::Array(values) => {
                    let index = usize::try_from(offset).map_err(|_| {
                        Diagnostic::ub(
                            "array subscript is outside the bounds of the object",
                            span,
                            Some("6.5.6"),
                        )
                    })?;
                    values.get_mut(index).ok_or_else(|| {
                        Diagnostic::ub(
                            "array subscript is outside the bounds of the object",
                            span,
                            Some("6.5.6"),
                        )
                    })
                }
                _ => Err(Diagnostic::error(
                    "array object does not have array storage",
                    span,
                )),
            },
            _ if offset == 0 => Ok(stored),
            _ => Err(Diagnostic::ub(
                "pointer arithmetic produced an invalid scalar object offset",
                span,
                Some("6.5.6"),
            )),
        }
    }

    fn resolve_member_storage_mut<'b>(
        &self,
        stored: &'b mut StoredValue,
        ty: &CType,
        path: &[String],
        span: Span,
    ) -> Result<&'b mut StoredValue, Diagnostic> {
        if path.is_empty() {
            return Ok(stored);
        }
        if let CType::Array(inner, _) = ty.unqualified() {
            return self.resolve_member_storage_mut(stored, inner, path, span);
        }
        let member_name = &path[0];
        match (stored, ty.unqualified()) {
            (StoredValue::Record(members), CType::Struct(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let index = members
                    .iter()
                    .position(|(name, _)| name.as_ref() == member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let stored = &mut members[index].1;
                self.resolve_member_storage_mut(stored, &member_ty, &path[1..], span)
            }
            (
                StoredValue::Union {
                    active_member,
                    members,
                    bytes,
                },
                CType::Union(_, _),
            ) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let index = members
                    .iter()
                    .position(|(name, _)| name.as_ref() == member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                if active_member
                    .as_ref()
                    .is_none_or(|active| active.as_ref() != member_name)
                {
                    let member_size = if member.bit_width.is_some() {
                        member.bit_storage_size
                    } else {
                        self.type_size_of(&member.ty).unwrap()
                    };
                    let end = member.offset + member_size;
                    let current = if member.bit_width.is_some() {
                        self.deserialize_bit_field(member, &bytes[member.offset..end], span)?
                    } else {
                        self.deserialize_stored_value(&member.ty, &bytes[member.offset..end], span)?
                    };
                    members[index].1 = current;
                }
                *active_member = Some(member_name.as_str().into());
                let stored = &mut members[index].1;
                self.resolve_member_storage_mut(stored, &member_ty, &path[1..], span)
            }
            _ => Err(Diagnostic::ub(
                "pointer or lvalue does not actually designate a structure or union object of the required type",
                span,
                Some("6.5.2.3"),
            )),
        }
    }

    pub(super) fn simple_ub_diagnostic(
        &self,
        message: impl Into<String>,
        span: Span,
        standard_mode_note: impl Into<String>,
    ) -> Diagnostic {
        debug_assert_eq!(self.run_options.ub_detection_mode, UbDetectionMode::Simple);
        Diagnostic::ub(message, span, None)
            .with_note("this is an intentional conservative rejection by simple UB mode")
            .with_note(standard_mode_note)
    }

    fn reject_potentially_invalid_object_representation(
        &self,
        value: &TypedValue,
        ty: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if matches!(value.data, ValueData::ObjectRepresentation(_)) {
            let message = if ty.is_pointer() {
                "read of a pointer value with an invalid object representation".to_owned()
            } else {
                format!(
                    "read of indeterminate {} with a potentially invalid object representation",
                    ty
                )
            };
            return Err(Diagnostic::ub(message, span, Some("6.2.6.1p5-6")));
        }
        if value.indeterminate
            && !ty.is_character()
            && !matches!(ty.unqualified(), CType::Struct(_, _) | CType::Union(_, _))
            && !all_bit_patterns_valid(ty)
        {
            return Err(Diagnostic::ub(
                format!(
                    "read of indeterminate {} with a potentially invalid object representation",
                    ty
                ),
                span,
                Some("6.2.6.1p5-6"),
            ));
        }
        Ok(())
    }

    fn inactive_union_read_is_common_initial_sequence(
        &self,
        union_ty: &CType,
        active_member_name: &str,
        selected_member: &RecordMember,
        selected_path: &[String],
    ) -> bool {
        let Some(selected_subobject_name) = selected_path.first() else {
            return false;
        };
        let Some(active_member) = self.direct_member_by_storage_name(union_ty, active_member_name)
        else {
            return false;
        };
        if !matches!(active_member.ty.unqualified(), CType::Struct(_, _))
            || !matches!(selected_member.ty.unqualified(), CType::Struct(_, _))
        {
            return false;
        }
        let (Some(active_record), Some(selected_record)) = (
            self.record_type(&active_member.ty),
            self.record_type(&selected_member.ty),
        ) else {
            return false;
        };
        let Some(selected_index) = selected_record
            .members
            .iter()
            .position(|member| member.storage_name == *selected_subobject_name)
        else {
            return false;
        };
        if active_record.members.len() <= selected_index {
            return false;
        }
        active_record
            .members
            .iter()
            .zip(&selected_record.members)
            .take(selected_index + 1)
            .all(|(active, selected)| {
                self.compatible_object_layout_types(&active.ty, &selected.ty)
                    && active.ty.top_level_qualifiers() == selected.ty.top_level_qualifiers()
                    && self.type_align_of(&active.ty) == self.type_align_of(&selected.ty)
                    && active.bit_width == selected.bit_width
                    && active.offset == selected.offset
            })
    }

    fn reject_simple_inactive_union_member_read(
        &self,
        stored: &StoredValue,
        ty: &CType,
        path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if self.run_options.ub_detection_mode != UbDetectionMode::Simple || path.is_empty() {
            return Ok(());
        }
        let member_name = &path[0];
        match (stored, ty.unqualified()) {
            (StoredValue::Array(_), CType::Array(inner, _)) => {
                self.reject_simple_inactive_union_member_read(stored, inner, path, span)
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                let member_ty = self.qualified_member_type(ty, member);
                let slot = self.record_slot(values, member_name, span)?;
                self.reject_simple_inactive_union_member_read(slot, &member_ty, &path[1..], span)
            }
            (StoredValue::Union { active_member, .. }, CType::Union(_, _)) => {
                let member = self
                    .direct_member_by_storage_name(ty, member_name)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            format!("{} has no member named {}", ty, member_name),
                            span,
                        )
                    })?;
                // Decode first so a genuinely invalid representation keeps the strict
                // standard-mode diagnostic instead of being mislabeled as a simple-mode false
                // positive.
                let member_ty = self.qualified_member_type(ty, member);
                let slot = self.extract_union_member(stored, ty, member, span)?;
                let (read_slot, read_ty) = if path.len() == 1 {
                    (slot.clone(), member_ty.clone())
                } else {
                    self.extract_stored_subobject(&slot, &member_ty, &path[1..], span)?
                };
                let read_value = self.typed_value_from_stored(&read_ty, &read_slot);
                self.reject_potentially_invalid_object_representation(&read_value, &read_ty, span)?;
                if let Some(active) = active_member
                    && active.as_ref() != member_name
                    && !self.inactive_union_read_is_common_initial_sequence(
                        ty,
                        active,
                        member,
                        &path[1..],
                    )
                {
                    return Err(self.simple_ub_diagnostic(
                        format!(
                            "read of inactive union member {} while member {} is active",
                            member_name, active
                        ),
                        span,
                        "ISO C permits some reads of a union member other than the last-stored member; simple mode treats that representation-level technique as too error-prone",
                    ));
                }
                self.reject_simple_inactive_union_member_read(&slot, &member_ty, &path[1..], span)
            }
            _ => Ok(()),
        }
    }

    pub(super) fn load_lvalue(
        &mut self,
        lvalue: LValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        self.validate_atomic_member_access(&lvalue, objects, span)?;
        // Ordinary scalar variables dominate interpreted code.  Keep that case out of the
        // aggregate/member/raw-storage machinery below while retaining every check that can
        // apply to a live, non-volatile object with declared scalar type.
        let fast_scalar = (!self.restrict_tracking_required
            && self.configured_stream_buffers == 0
            && lvalue.base_offset == 0
            && lvalue.offset == 0
            && lvalue.member_path.is_empty()
            && lvalue.designated_root_ty.is_none()
            && lvalue.byte_offset_override.is_none()
            && lvalue.bit_field_width.is_none())
        .then(|| self.lookup_object(objects, lvalue.object))
        .flatten()
        .and_then(|object| match &object.value {
            StoredValue::Scalar(stored)
                if object.alive
                    && object.storage_duration != StorageDuration::Dynamic
                    && !object.ty.is_volatile_qualified()
                    && object.ty == lvalue.ty
                    && !stored.indeterminate
                    && !stored.missing_return
                    && matches!(
                        stored.data,
                        ValueData::Int(_)
                            | ValueData::Float(_)
                            | ValueData::Complex(_)
                            | ValueData::Pointer(_)
                    ) =>
            {
                Some((
                    stored.data.clone(),
                    stored.restrict_source.clone(),
                    object.byte_size,
                ))
            }
            _ => None,
        });
        if let Some((data, restrict_source, byte_size)) = fast_scalar {
            if self.expr_state.track_unsequenced_accesses {
                self.record_read(
                    AccessRegion {
                        object: lvalue.object,
                        bit_start: 0,
                        bit_size: byte_size.saturating_mul(8),
                    },
                    span,
                )?;
            }
            let ty = match lvalue.ty {
                CType::Qualified(inner, _) => inner.unqualified().clone(),
                ty => ty,
            };
            let value = TypedValue {
                ty,
                data,
                restrict_source,
                indeterminate: false,
                missing_return: false,
            };
            if matches!(value.data, ValueData::Pointer(_))
                && self.pointer_value_has_ended_lifetime(&value, objects)
            {
                return Err(Diagnostic::ub(
                    "read of a pointer value whose referent's lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
                .with_note(
                    "the pointer value became indeterminate when the pointed-to object's lifetime ended",
                ));
            }
            return Ok(value);
        }
        let (_, access_start, access_size) = self
            .lvalue_byte_range(&lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
            Diagnostic::ub("pointer is not valid to access", span, Some("6.5.3.2"))
        })?;
        if self.expr_state.track_unsequenced_accesses {
            let access_region = self.lvalue_access_region_from_range(
                &lvalue,
                access_start,
                access_size,
                objects,
                span,
            )?;
            self.record_read(access_region, span)?;
        }
        let (
            mut element_value,
            effective_ty,
            object_storage_duration,
            object_address_taken,
            declaration_span,
            indeterminate_reason,
        ) = {
            let object = self.lookup_object(objects, lvalue.object).ok_or_else(|| {
                Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
            if !object.alive {
                return Err(Diagnostic::ub(
                    "access through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                ));
            }
            let declared_lvalue_is_volatile =
                self.lvalue_declared_type_is_volatile(&lvalue, &object.ty);
            if object.has_volatile_subobject
                && self.qualified_subobject_overlaps(
                    &object.ty,
                    0,
                    access_start,
                    access_size,
                    TrackedQualifier::Volatile,
                )
                && !lvalue.ty.is_volatile_qualified()
                && (declared_lvalue_is_volatile
                    || !matches!(
                        lvalue.ty.unqualified(),
                        CType::Struct(..) | CType::Union(..)
                    ))
            {
                return Err(Diagnostic::ub(
                    "attempt to refer to a volatile-qualified object or subobject through a non-volatile lvalue",
                    span,
                    Some("6.7.3"),
                ));
            }
            if self.stream_buffer_region_overlaps(lvalue.object, access_start, access_size, objects)
            {
                return Err(self.stream_buffer_use_diag(span));
            }
            self.check_dynamic_effective_type_read(
                object,
                access_start,
                access_size,
                &lvalue.ty,
                lvalue.designated_root_ty.as_deref().unwrap_or(&object.ty),
                &lvalue.member_path,
                span,
            )?;
            let byte_backed_storage = matches!(object.value, StoredValue::ObjectRepresentation(_))
                || (object.byte_size >= COMPACT_OBJECT_REPRESENTATION_THRESHOLD
                    && matches!(object.value, StoredValue::Indeterminate));
            if self.object_uses_raw_character_storage(object, &lvalue)
                || byte_backed_storage
                || (lvalue.byte_offset_override.is_some() && lvalue.bit_field_width.is_none())
            {
                let effective_ty = lvalue.ty.clone();
                self.record_restrict_access(&lvalue, &effective_ty, false, span, objects)?;
                let mut loaded = self.load_raw_byte_lvalue(&lvalue, span, objects)?;
                if let Some(width) = lvalue.bit_field_width {
                    let bit_offset = self.lvalue_bit_field_offset(&lvalue, objects).unwrap_or(0);
                    let mask = self.integer_mask(width as u32);
                    let bits = ((loaded.to_int()? as u128) >> bit_offset) & mask;
                    let value = if matches!(lvalue.ty.unqualified(), CType::Bool)
                        || lvalue.ty.is_unsigned_integer()
                    {
                        bits as i128
                    } else {
                        let sign_bit = 1u128 << (width - 1);
                        if bits & sign_bit != 0 {
                            (bits as i128) - ((mask + 1) as i128)
                        } else {
                            bits as i128
                        }
                    };
                    loaded = TypedValue::integer(lvalue.ty.clone(), value);
                }
                self.reject_potentially_invalid_object_representation(
                    &loaded,
                    &effective_ty,
                    span,
                )?;
                return Ok(loaded);
            }
            let (base_stored, base_ty, tail_offset) =
                self.resolve_lvalue_array_base(&object.value, &object.ty, &lvalue, span)?;
            let (base_stored, base_ty, tail_offset) = if !lvalue.member_path.is_empty()
                && matches!(base_ty.unqualified(), CType::Array(_, _))
            {
                let (stored, element_ty) =
                    self.apply_lvalue_offset(base_stored, &base_ty, tail_offset, span)?;
                (stored, element_ty, 0)
            } else {
                (base_stored, base_ty, tail_offset)
            };
            self.reject_simple_inactive_union_member_read(
                base_stored,
                &base_ty,
                &lvalue.member_path,
                span,
            )?;
            let (stored, ty) = self.extract_stored_subobject_view(
                base_stored,
                &base_ty,
                &lvalue.member_path,
                span,
            )?;
            let (stored, effective_ty) =
                if tail_offset == 0 && self.compatible_object_layout_types(&ty, &lvalue.ty) {
                    let effective_ty = if ty.unqualified() != lvalue.ty.unqualified()
                        && self.cross_unit_tagged_type_compatible(
                            ty.unqualified(),
                            lvalue.ty.unqualified(),
                        ) {
                        lvalue.ty.clone()
                    } else {
                        ty
                    };
                    (stored, effective_ty)
                } else {
                    self.apply_lvalue_offset_view(stored, &ty, tail_offset, span)?
                };
            let element_value = self.typed_value_from_stored_view(&effective_ty, stored);
            (
                element_value,
                effective_ty,
                object.storage_duration,
                object.address_taken,
                object.declaration_span,
                object.indeterminate_reason,
            )
        };
        self.record_restrict_access(&lvalue, &effective_ty, false, span, objects)?;
        if effective_ty.is_pointer() && effective_ty.top_level_qualifiers().is_restrict {
            let pointed_to_const = effective_ty
                .element_type()
                .is_some_and(CType::is_const_qualified);
            element_value.restrict_source = Some(Rc::new(RestrictSource::from_lvalue(
                &lvalue,
                pointed_to_const,
            )));
        }
        if matches!(effective_ty.unqualified(), CType::Pointer(_))
            && self.pointer_value_has_ended_lifetime(&element_value, objects)
        {
            return Err(Diagnostic::ub(
                "read of a pointer value whose referent's lifetime has ended",
                span,
                Some("6.2.4"),
            )
            .with_note("the pointer value became indeterminate when the pointed-to object's lifetime ended"));
        }
        if let ValueData::Aggregate(stored) = &element_value.data
            && self.stored_value_has_ended_pointer(stored, &effective_ty, objects)
        {
            return Err(Diagnostic::ub(
                "read of an aggregate value containing a pointer whose referent's lifetime has ended",
                span,
                Some("6.2.4"),
            )
            .with_note(
                "the pointer subobject became indeterminate when the pointed-to object's lifetime ended",
            ));
        }
        if element_value.indeterminate
            && indeterminate_reason.is_some()
            && object_storage_duration == StorageDuration::Automatic
        {
            return Err(Diagnostic::ub(
                indeterminate_reason.unwrap(),
                span,
                Some("7.13.2.1"),
            ));
        }
        if element_value.indeterminate
            && object_storage_duration == StorageDuration::Automatic
            && !object_address_taken
        {
            let declaration = self.sources.snippet(declaration_span);
            return Err(Diagnostic::ub(
                format!(
                    "read of uninitialized automatic object {} whose address was never taken",
                    effective_ty
                ),
                span,
                Some("6.3.2.1p2"),
            )
            .with_note(
                "this object could have been declared with register storage class, so the lvalue-to-rvalue conversion is undefined",
            )
            .with_note(format!(
                "object declared at {}:{}:{}",
                declaration.path.display(),
                declaration.line_number,
                declaration.column
            )));
        }
        if element_value.indeterminate
            && object_storage_duration == StorageDuration::Automatic
            && object_address_taken
            && self.run_options.ub_detection_mode == UbDetectionMode::Simple
            && (effective_ty.is_character() || all_bit_patterns_valid(&effective_ty))
        {
            let declaration = self.sources.snippet(declaration_span);
            return Err(self
                .simple_ub_diagnostic(
                    format!("read of uninitialized automatic object {}", effective_ty),
                    span,
                    "the object's address was taken, so the strict C11 6.3.2.1p2 rule for never-addressed automatic objects does not apply; simple mode rejects the indeterminate read anyway",
                )
                .with_note(format!(
                    "object declared at {}:{}:{}",
                    declaration.path.display(),
                    declaration.line_number,
                    declaration.column
                )));
        }
        self.reject_potentially_invalid_object_representation(&element_value, &effective_ty, span)?;
        Ok(element_value)
    }

    pub(super) fn pointer_value_has_ended_lifetime(
        &self,
        value: &TypedValue,
        objects: &ObjectFrames,
    ) -> bool {
        let ValueData::Pointer(pointer) = &value.data else {
            return false;
        };
        let Some(object_id) = pointer.object else {
            return false;
        };
        self.lookup_object(objects, object_id)
            .is_none_or(|object| !object.alive)
    }

    fn stored_value_has_ended_pointer(
        &self,
        stored: &StoredValue,
        ty: &CType,
        objects: &ObjectFrames,
    ) -> bool {
        match (stored, ty.unqualified()) {
            (StoredValue::Scalar(value), CType::Pointer(_)) => {
                self.pointer_value_has_ended_lifetime(value, objects)
            }
            (StoredValue::Array(values), CType::Array(inner, _)) => values
                .iter()
                .any(|value| self.stored_value_has_ended_pointer(value, inner, objects)),
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                self.record_type(ty).is_some_and(|record| {
                    record.members.iter().any(|member| {
                        values
                            .iter()
                            .find(|(name, _)| name.as_ref() == member.storage_name)
                            .is_some_and(|(_, value)| {
                                self.stored_value_has_ended_pointer(value, &member.ty, objects)
                            })
                    })
                })
            }
            (
                StoredValue::Union {
                    active_member,
                    members,
                    ..
                },
                CType::Union(_, _),
            ) => active_member.as_ref().is_some_and(|active| {
                self.record_type(ty).is_some_and(|record| {
                    record
                        .members
                        .iter()
                        .find(|member| member.storage_name == active.as_ref())
                        .and_then(|member| {
                            members
                                .iter()
                                .find(|(name, _)| name.as_ref() == active.as_ref())
                                .map(|(_, value)| (member, value))
                        })
                        .is_some_and(|(member, value)| {
                            self.stored_value_has_ended_pointer(value, &member.ty, objects)
                        })
                })
            }),
            _ => false,
        }
    }

    pub(super) fn store_value(
        &mut self,
        objects: &mut ObjectFrames,
        object_id: ObjectId,
        value: TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.store_value_impl(objects, object_id, value, span, false)
    }

    pub(super) fn initialize_object_value(
        &mut self,
        objects: &mut ObjectFrames,
        object_id: ObjectId,
        value: TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.store_value_impl(objects, object_id, value, span, true)
    }

    fn store_value_impl(
        &mut self,
        objects: &mut ObjectFrames,
        object_id: ObjectId,
        value: TypedValue,
        span: Span,
        is_initialization: bool,
    ) -> Result<(), Diagnostic> {
        let (ty, byte_size) = self
            .lookup_object(objects, object_id)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "write through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })
            .map(|object| (object.ty.clone(), object.byte_size))?;
        self.record_write(
            AccessRegion {
                object: object_id,
                bit_start: 0,
                bit_size: byte_size.saturating_mul(8),
            },
            span,
        )?;
        let converted = self.convert_value(value, &ty, span)?;
        {
            let object = self.lookup_object_mut(objects, object_id).ok_or_else(|| {
                Diagnostic::ub(
                    "write through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
            if !object.alive {
                return Err(Diagnostic::ub(
                    "write through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                ));
            }
            if object.readonly {
                return Err(Diagnostic::ub(
                    "attempt to modify a string literal or other read-only object",
                    span,
                    Some("6.4.5"),
                ));
            }
            if self.type_has_const_subobject(&ty) && !is_initialization {
                return Err(Diagnostic::ub(
                    "attempt to modify an object defined with a const-qualified type",
                    span,
                    Some("6.7.3p6"),
                ));
            }
        }
        let stored = self.stored_value_from_typed_value(converted, &ty, span)?;
        let initialized = self.stored_value_is_determinate(&stored);
        let object = self.lookup_object_mut(objects, object_id).ok_or_else(|| {
            Diagnostic::ub(
                "write through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        object.initialized = initialized;
        object.value = stored;
        object.indeterminate_reason = None;
        object.modification_count = object.modification_count.saturating_add(1);
        Ok(())
    }

    pub(super) fn store_lvalue(
        &mut self,
        objects: &mut ObjectFrames,
        lvalue: &LValue,
        value: TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.validate_atomic_member_access(lvalue, objects, span)?;
        let (
            object_ty,
            readonly,
            target_const,
            target_volatile,
            alive,
            raw_character_storage,
            byte_backed_storage,
        ) = {
            let object_state = self.lookup_object(objects, lvalue.object).ok_or_else(|| {
                Diagnostic::ub(
                    "write through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
            let root_ty = lvalue
                .designated_root_ty
                .as_deref()
                .unwrap_or(&object_state.ty);
            let target_ty = self
                .storage_path_type(root_ty, &lvalue.member_path)
                .unwrap_or_else(|| lvalue.ty.clone());
            (
                object_state.ty.clone(),
                object_state.readonly,
                self.type_has_const_subobject(&target_ty),
                target_ty.is_volatile_qualified(),
                object_state.alive,
                self.object_uses_raw_character_storage(object_state, lvalue),
                matches!(object_state.value, StoredValue::ObjectRepresentation(_))
                    || (object_state.byte_size >= COMPACT_OBJECT_REPRESENTATION_THRESHOLD
                        && matches!(object_state.value, StoredValue::Indeterminate)),
            )
        };
        let (_, access_start, access_size) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to access", span, Some("6.5.3.2"))
            })?;
        if self.stream_buffer_region_overlaps(lvalue.object, access_start, access_size, objects) {
            return Err(self.stream_buffer_use_diag(span));
        }
        if raw_character_storage
            || byte_backed_storage
            || (lvalue.byte_offset_override.is_some() && lvalue.bit_field_width.is_none())
        {
            return self.store_raw_byte_lvalue(objects, lvalue, value, span);
        }
        if !alive {
            return Err(Diagnostic::ub(
                "write through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            ));
        }
        if lvalue.base_offset == 0
            && lvalue.offset == 0
            && lvalue.member_path.is_empty()
            && self.compatible_object_layout_types(&object_ty, &lvalue.ty)
        {
            self.record_restrict_access(lvalue, &lvalue.ty, true, span, objects)?;
            return self.store_value(objects, lvalue.object, value, span);
        }
        self.record_restrict_access(lvalue, &lvalue.ty, true, span, objects)?;
        let access_region =
            self.lvalue_access_region_from_range(lvalue, access_start, access_size, objects, span)?;
        self.record_write(access_region, span)?;
        let converted = self.convert_value(value, &lvalue.ty, span)?;
        if readonly {
            return Err(Diagnostic::ub(
                "attempt to modify a string literal or other read-only object",
                span,
                Some("6.4.5"),
            ));
        }
        if target_const {
            return Err(Diagnostic::ub(
                "attempt to modify an object defined with a const-qualified type",
                span,
                Some("6.7.3p6"),
            ));
        }
        if target_volatile && !lvalue.ty.is_volatile_qualified() {
            return Err(Diagnostic::ub(
                "attempt to refer to an object defined with a volatile-qualified type through a non-volatile lvalue",
                span,
                Some("6.7.3"),
            ));
        }
        let converted = if let Some(width) = lvalue.bit_field_width {
            TypedValue::integer(
                lvalue.ty.clone(),
                self.normalize_bit_field_value(converted.to_int()?, &lvalue.ty, width),
            )
        } else {
            converted
        };
        let stored = self.stored_value_from_typed_value(converted, &lvalue.ty, span)?;
        {
            let object = self
                .lookup_active_object_mut(objects, lvalue.object)
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "write through a pointer to an object whose lifetime has ended",
                        span,
                        Some("6.2.4"),
                    )
                })?;
            let slot =
                self.resolve_lvalue_storage_mut(&mut object.value, &object_ty, lvalue, span)?;
            *slot = stored;
        }
        let object = self
            .lookup_active_object_mut(objects, lvalue.object)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "write through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
        if self.type_contains_union(&object_ty) {
            self.refresh_all_union_bytes(&mut object.value, &object_ty, span)?;
        }
        object.initialized = self.stored_value_is_determinate(&object.value);
        object.indeterminate_reason = None;
        object.modification_count = object.modification_count.saturating_add(1);
        Ok(())
    }

    fn object_uses_raw_character_storage(&self, object: &ObjectState, lvalue: &LValue) -> bool {
        if matches!(
            object.ty.unqualified(),
            CType::Array(inner, _) if inner.is_character()
        ) && object.storage_duration == StorageDuration::Dynamic
        {
            return true;
        }
        let root_ty = lvalue.designated_root_ty.as_deref().unwrap_or(&object.ty);
        matches!(
            self.storage_path_type(root_ty, &lvalue.member_path)
                .as_ref()
                .map(|ty| ty.unqualified()),
            Some(CType::Array(_, 0))
        )
    }

    fn load_raw_byte_lvalue(
        &mut self,
        lvalue: &LValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let (_, start, size) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
            })?;
        let end = start.checked_add(size).ok_or_else(|| {
            Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
        })?;
        let object = self.lookup_object(objects, lvalue.object).ok_or_else(|| {
            Diagnostic::ub(
                "access through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        if end > object.byte_size {
            return Err(Diagnostic::ub(
                "pointer is not valid to dereference",
                span,
                Some("6.5.3.2"),
            ));
        }
        if matches!(lvalue.ty.unqualified(), CType::Pointer(_))
            && let Some(stored_pointer) = object.pointer_slots.get(&(start, size))
        {
            let data = match stored_pointer {
                StoredPointerValue::Object(pointer) => ValueData::Pointer(Rc::new(pointer.clone())),
                StoredPointerValue::Function(name) => ValueData::Function(name.clone().into()),
            };
            return Ok(TypedValue::from_data(
                Self::unqualified_value_type(&lvalue.ty),
                data,
            ));
        }
        if matches!(object.value, StoredValue::Indeterminate) {
            return Ok(TypedValue::indeterminate_for(Self::unqualified_value_type(
                &lvalue.ty,
            )));
        }
        if let StoredValue::ObjectRepresentation(bytes) = &object.value {
            let stored = self.deserialize_stored_value(&lvalue.ty, &bytes[start..end], span)?;
            let value = self.typed_value_from_stored(&lvalue.ty, &stored);
            return Ok(self.specialize_dynamic_raw_storage_pointer(value, &lvalue.ty, objects));
        }
        let object_ty = object.ty.clone();
        let object_value = object.value.clone();
        let all_bytes = self.serialize_stored_value(&object_ty, &object_value, span)?;
        let stored = self.deserialize_stored_value(&lvalue.ty, &all_bytes[start..end], span)?;
        let value = self.typed_value_from_stored(&lvalue.ty, &stored);
        Ok(self.specialize_dynamic_raw_storage_pointer(value, &lvalue.ty, objects))
    }

    fn store_raw_byte_lvalue(
        &mut self,
        objects: &mut ObjectFrames,
        lvalue: &LValue,
        value: TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.record_restrict_access(lvalue, &lvalue.ty, true, span, objects)?;
        let (_, access_start, access_size) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to access", span, Some("6.5.3.2"))
            })?;
        let access_region =
            self.lvalue_access_region_from_range(lvalue, access_start, access_size, objects, span)?;
        self.record_write(access_region, span)?;
        let converted = self.convert_value(value, &lvalue.ty, span)?;
        let stored = if lvalue.bit_field_width.is_none() {
            Some(self.stored_value_from_typed_value(converted.clone(), &lvalue.ty, span)?)
        } else {
            None
        };
        let mut replacement_pointer_slots = Vec::new();
        if let Some(stored) = stored.as_ref() {
            self.collect_stored_pointer_slots(
                stored,
                &lvalue.ty,
                access_start,
                &mut replacement_pointer_slots,
            );
        }
        self.ensure_writable_region(lvalue.object, access_start, access_size, span, objects)?;
        let (object_ty, volatile_object, declared_lvalue_is_volatile) = {
            let object = self.lookup_object(objects, lvalue.object).ok_or_else(|| {
                Diagnostic::ub(
                    "write through a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.2.4"),
                )
            })?;
            (
                object.ty.clone(),
                object.has_volatile_subobject
                    && self.qualified_subobject_overlaps(
                        &object.ty,
                        0,
                        access_start,
                        access_size,
                        TrackedQualifier::Volatile,
                    ),
                self.lvalue_declared_type_is_volatile(lvalue, &object.ty),
            )
        };
        if volatile_object
            && !lvalue.ty.is_volatile_qualified()
            && (declared_lvalue_is_volatile
                || !matches!(
                    lvalue.ty.unqualified(),
                    CType::Struct(..) | CType::Union(..)
                ))
        {
            return Err(Diagnostic::ub(
                "attempt to refer to an object defined with a volatile-qualified type through a non-volatile lvalue",
                span,
                Some("6.7.3"),
            ));
        }
        let new_bytes = if let Some(width) = lvalue.bit_field_width {
            let bit_offset = self.lvalue_bit_field_offset(lvalue, objects).unwrap_or(0);
            let normalized =
                self.normalize_bit_field_value(converted.to_int()?, &lvalue.ty, width) as u128;
            let encoded = normalized << bit_offset;
            let field_mask = self.integer_mask(width as u32) << bit_offset;
            let (object_ty, object_size, object_value) = self
                .lookup_object(objects, lvalue.object)
                .map(|object| (object.ty.clone(), object.byte_size, object.value.clone()))
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "write through a pointer to an object whose lifetime has ended",
                        span,
                        Some("6.2.4"),
                    )
                })?;
            let all_bytes = match &object_value {
                StoredValue::Indeterminate => vec![ByteCell::Indeterminate; object_size],
                StoredValue::ObjectRepresentation(bytes) => bytes.clone(),
                stored => self.serialize_stored_value(&object_ty, stored, span)?,
            };
            let end = access_start.checked_add(access_size).ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
            })?;
            let mut bytes = all_bytes
                .get(access_start..end)
                .ok_or_else(|| {
                    Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
                })?
                .to_vec();
            for (index, byte) in bytes.iter_mut().enumerate() {
                let mask = ((field_mask >> (index * 8)) & 0xff) as u8;
                let bits = ((encoded >> (index * 8)) & 0xff) as u8;
                *byte = match *byte {
                    ByteCell::Known(existing) => {
                        ByteCell::Known((existing & !mask) | (bits & mask))
                    }
                    ByteCell::Indeterminate if mask == 0xff => ByteCell::Known(bits),
                    ByteCell::Indeterminate => ByteCell::Indeterminate,
                };
            }
            bytes
        } else {
            self.serialize_stored_value(&lvalue.ty, stored.as_ref().unwrap(), span)?
        };
        let wrote_dynamic_raw_storage = {
            let object = self
                .lookup_active_object_mut(objects, lvalue.object)
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "write through a pointer to an object whose lifetime has ended",
                        span,
                        Some("6.2.4"),
                    )
                })?;
            if object.storage_duration == StorageDuration::Dynamic
                && matches!(
                    &object.value,
                    StoredValue::Indeterminate | StoredValue::ObjectRepresentation(_)
                )
            {
                if matches!(&object.value, StoredValue::Indeterminate) {
                    object.value = StoredValue::ObjectRepresentation(vec![
                        ByteCell::Indeterminate;
                        object.byte_size
                    ]);
                }
                let current_version = object.modification_count;
                let mut indeterminate_bytes = object
                    .raw_indeterminate_bytes
                    .filter(|(version, _)| *version == current_version)
                    .map(|(_, count)| count)
                    .unwrap_or_else(|| match &object.value {
                        StoredValue::ObjectRepresentation(bytes) => bytes
                            .iter()
                            .filter(|byte| matches!(byte, ByteCell::Indeterminate))
                            .count(),
                        _ => unreachable!(),
                    });
                let StoredValue::ObjectRepresentation(all_bytes) = &mut object.value else {
                    unreachable!();
                };
                let end = access_start.checked_add(access_size).ok_or_else(|| {
                    Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
                })?;
                let destination = all_bytes.get_mut(access_start..end).ok_or_else(|| {
                    Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
                })?;
                if destination.len() != new_bytes.len() {
                    return Err(Diagnostic::ub(
                        "pointer is not valid to dereference",
                        span,
                        Some("6.5.3.2"),
                    ));
                }
                for (destination, source) in destination.iter_mut().zip(&new_bytes) {
                    match (*destination, *source) {
                        (ByteCell::Indeterminate, ByteCell::Known(_)) => {
                            indeterminate_bytes = indeterminate_bytes.saturating_sub(1);
                        }
                        (ByteCell::Known(_), ByteCell::Indeterminate) => {
                            indeterminate_bytes = indeterminate_bytes.saturating_add(1);
                        }
                        _ => {}
                    }
                    *destination = *source;
                }
                object.initialized = indeterminate_bytes == 0;
                object.indeterminate_reason = None;
                object.modification_count = object.modification_count.saturating_add(1);
                object.raw_indeterminate_bytes =
                    Some((object.modification_count, indeterminate_bytes));
                Self::replace_pointer_slots_for_write(object, access_start, access_size, None);
                object
                    .pointer_slots
                    .extend(replacement_pointer_slots.iter().cloned());
                true
            } else {
                false
            }
        };
        if wrote_dynamic_raw_storage {
            self.update_object_effective_types_after_store(
                objects,
                lvalue.object,
                access_start,
                access_size,
                &lvalue.ty,
            );
            return Ok(());
        }
        let mut all_bytes = {
            let object = self
                .lookup_active_object(objects, lvalue.object)
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "write through a pointer to an object whose lifetime has ended",
                        span,
                        Some("6.2.4"),
                    )
                })?;
            self.serialize_stored_value(&object_ty, &object.value, span)?
        };
        let (_, start, size) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
            })?;
        let end = start.checked_add(size).ok_or_else(|| {
            Diagnostic::ub("pointer is not valid to dereference", span, Some("6.5.3.2"))
        })?;
        if end > all_bytes.len() || new_bytes.len() != size {
            return Err(Diagnostic::ub(
                "pointer is not valid to dereference",
                span,
                Some("6.5.3.2"),
            ));
        }
        self.overlay_bytes(&mut all_bytes, start, &new_bytes);
        let initialized = all_bytes
            .iter()
            .all(|byte| matches!(byte, ByteCell::Known(_)));
        let new_value = StoredValue::ObjectRepresentation(all_bytes);
        {
            let object = self
                .lookup_active_object_mut(objects, lvalue.object)
                .ok_or_else(|| {
                    Diagnostic::ub(
                        "write through a pointer to an object whose lifetime has ended",
                        span,
                        Some("6.2.4"),
                    )
                })?;
            object.value = new_value;
            object.initialized = initialized;
            object.indeterminate_reason = None;
            object.modification_count = object.modification_count.saturating_add(1);
            Self::replace_pointer_slots_for_write(object, access_start, access_size, None);
            object.pointer_slots.extend(replacement_pointer_slots);
        }
        self.update_object_effective_types_after_store(
            objects,
            lvalue.object,
            access_start,
            access_size,
            &lvalue.ty,
        );
        Ok(())
    }

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
                    if (((target_inner.is_character()
                        || matches!(target_inner.unqualified(), CType::Void))
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

    pub(super) fn allocate_object(
        &mut self,
        objects: &mut ObjectFrames,
        ty: CType,
        storage_duration: StorageDuration,
        declaration_span: Span,
        register_object: bool,
        variably_modified: bool,
    ) -> Result<ObjectId, Diagnostic> {
        let byte_size = match self.type_size_of(&ty) {
            Some(size) => size,
            None if matches!(ty.unqualified(), CType::Array(_, 0)) => 0,
            None => {
                return Err(Diagnostic::error(
                    format!(
                        "cannot allocate an object of incomplete or oversized type {}",
                        ty
                    ),
                    declaration_span,
                ));
            }
        };
        self.ensure_object_storage_limit(
            objects,
            storage_duration,
            byte_size,
            None,
            declaration_span,
        )?;
        let target_frame_index = if matches!(
            storage_duration,
            StorageDuration::Static | StorageDuration::Dynamic
        ) {
            0
        } else {
            objects.len() - 1
        };
        let id = if storage_duration == StorageDuration::Automatic {
            self.reusable_automatic_object_ids
                .pop()
                .unwrap_or_else(|| self.allocate_object_id(Some(target_frame_index)))
        } else {
            self.allocate_object_id(Some(target_frame_index))
        };
        self.object_type_registry.insert(id, ty.clone());
        self.assign_object_base_address(id, &ty);
        let initial_value = if storage_duration == StorageDuration::Dynamic
            || byte_size >= COMPACT_OBJECT_REPRESENTATION_THRESHOLD
        {
            StoredValue::Indeterminate
        } else {
            self.default_object_value(&ty)
        };
        let has_const_subobject = self.type_has_const_subobject(&ty);
        let has_volatile_subobject = self.type_has_volatile_subobject(&ty);
        objects.insert(
            target_frame_index,
            id,
            ObjectState {
                ty: ty.clone(),
                storage_duration,
                alive: true,
                readonly: false,
                const_object: ty.is_const_qualified(),
                has_const_subobject,
                has_volatile_subobject,
                register_object,
                address_taken: false,
                initialized: false,
                indeterminate_reason: None,
                byte_size,
                value: initial_value,
                declaration_span,
                modification_count: 0,
                raw_indeterminate_bytes: None,
                variably_modified,
                effective_types: Vec::new(),
                pointer_slots: BTreeMap::default(),
            },
        );
        if self.run_options.allocation_limit_bytes.is_some()
            && storage_duration != StorageDuration::Dynamic
        {
            self.live_non_dynamic_bytes = self
                .live_non_dynamic_bytes
                .checked_add(byte_size)
                .expect("live non-dynamic object byte total overflowed");
        }
        Ok(id)
    }

    fn end_live_non_dynamic_bytes(&mut self, object: &ObjectState) {
        if self.run_options.allocation_limit_bytes.is_some()
            && object.alive
            && object.storage_duration != StorageDuration::Dynamic
        {
            self.live_non_dynamic_bytes = self
                .live_non_dynamic_bytes
                .checked_sub(object.byte_size)
                .expect("ended object was included in the non-dynamic byte total");
        }
    }

    pub(super) fn replace_live_non_dynamic_bytes(&mut self, previous_size: usize, new_size: usize) {
        if self.run_options.allocation_limit_bytes.is_none() {
            return;
        }
        self.live_non_dynamic_bytes = self
            .live_non_dynamic_bytes
            .checked_sub(previous_size)
            .and_then(|total| total.checked_add(new_size))
            .expect("replacement object byte total overflowed");
    }

    pub(super) fn ensure_object_storage_limit(
        &self,
        objects: &ObjectFrames,
        storage_duration: StorageDuration,
        byte_size: usize,
        replacing: Option<ObjectId>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(allocation_limit) = self.run_options.allocation_limit_bytes else {
            return Ok(());
        };
        if storage_duration == StorageDuration::Dynamic {
            return Ok(());
        }
        let replacing_bytes = replacing
            .and_then(|id| self.lookup_object(objects, id))
            .filter(|object| object.alive && object.storage_duration != StorageDuration::Dynamic)
            .map_or(0, |object| object.byte_size);
        let active_bytes = self.live_non_dynamic_bytes.checked_sub(replacing_bytes);
        if byte_size > allocation_limit
            || active_bytes
                .and_then(|total| total.checked_add(byte_size))
                .is_none_or(|total| total > allocation_limit)
        {
            return Err(Diagnostic::error(
                format!(
                    "object storage exceeds the {} interpreter limit",
                    byte_limit_description(allocation_limit)
                ),
                span,
            ));
        }
        Ok(())
    }

    pub(super) fn assign_object_base_address(&mut self, object: ObjectId, ty: &CType) {
        if self.object_base_addresses.contains_key(&object) {
            return;
        }
        let size = self.type_size_of(ty).unwrap_or(1).max(1) as u64;
        let align = self.type_align_of(ty).unwrap_or(1).max(1) as u64;
        let base = align_address(self.next_encoded_pointer, align);
        let stride = object_address_stride(size, align);
        self.next_encoded_pointer = base.saturating_add(stride);
        self.object_base_addresses.insert(object, base);
    }

    pub(super) fn ensure_object_alignment(
        &mut self,
        object: ObjectId,
        ty: &CType,
        requested_alignment: Option<usize>,
    ) {
        let Some(requested_alignment) = requested_alignment else {
            return;
        };
        let requested_alignment = requested_alignment.max(1) as u64;
        if self
            .object_base_addresses
            .get(&object)
            .is_some_and(|address| address % requested_alignment == 0)
        {
            return;
        }
        let size = self.type_size_of(ty).unwrap_or(1).max(1) as u64;
        let base = align_address(self.next_encoded_pointer, requested_alignment);
        self.next_encoded_pointer =
            base.saturating_add(object_address_stride(size, requested_alignment));
        self.object_base_addresses.insert(object, base);
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

    fn eval_pointer_arithmetic(
        &self,
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        self.reject_missing_return_value(&lhs, span)?;
        self.reject_missing_return_value(&rhs, span)?;
        match (lhs.ty.unqualified(), rhs.ty.unqualified(), op) {
            (CType::Pointer(_), _, BinaryOp::Add) if rhs.ty.is_integer() => {
                let pointer = lhs.as_pointer(span)?;
                let delta = self.integer_promotion(rhs, span)?.to_int()?;
                let offset = self.checked_pointer_offset(
                    pointer,
                    delta,
                    lhs.ty.element_type().expect("pointer has element type"),
                    objects,
                    span,
                )?;
                Ok(TypedValue {
                    ty: lhs.ty.clone(),
                    data: ValueData::Pointer(Rc::new(offset)),
                    restrict_source: lhs.restrict_source.clone(),
                    indeterminate: false,
                    missing_return: false,
                })
            }
            (_, CType::Pointer(_), BinaryOp::Add) if lhs.ty.is_integer() => {
                let pointer = rhs.as_pointer(span)?;
                let delta = self.integer_promotion(lhs, span)?.to_int()?;
                let offset = self.checked_pointer_offset(
                    pointer,
                    delta,
                    rhs.ty.element_type().expect("pointer has element type"),
                    objects,
                    span,
                )?;
                Ok(TypedValue {
                    ty: rhs.ty.clone(),
                    data: ValueData::Pointer(Rc::new(offset)),
                    restrict_source: rhs.restrict_source.clone(),
                    indeterminate: false,
                    missing_return: false,
                })
            }
            (CType::Pointer(_), _, BinaryOp::Sub) if rhs.ty.is_integer() => {
                let pointer = lhs.as_pointer(span)?;
                let delta = self.integer_promotion(rhs, span)?.to_int()?;
                let offset = self.checked_pointer_offset(
                    pointer,
                    -delta,
                    lhs.ty.element_type().expect("pointer has element type"),
                    objects,
                    span,
                )?;
                Ok(TypedValue {
                    ty: lhs.ty.clone(),
                    data: ValueData::Pointer(Rc::new(offset)),
                    restrict_source: lhs.restrict_source.clone(),
                    indeterminate: false,
                    missing_return: false,
                })
            }
            (CType::Pointer(_), CType::Pointer(_), BinaryOp::Sub) => {
                let CType::Pointer(lhs_inner) = lhs.ty.unqualified() else {
                    unreachable!();
                };
                let CType::Pointer(rhs_inner) = rhs.ty.unqualified() else {
                    unreachable!();
                };
                if !self.pointer_targets_are_compatible_object_types(lhs_inner, rhs_inner, true) {
                    return Err(Diagnostic::error(
                        "pointer subtraction requires pointers to compatible complete object types",
                        span,
                    ));
                }
                let common_inner = self
                    .composite_pointer_target_type_inner(lhs_inner, rhs_inner, false)
                    .filter(|ty| self.type_is_complete(ty))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "pointer subtraction requires pointers to compatible complete object types",
                            span,
                        )
                    })?;
                let common_ty = CType::pointer_to(common_inner.clone());
                let lhs = self.convert_value(lhs, &common_ty, span)?;
                let rhs = self.convert_value(rhs, &common_ty, span)?;
                let element_size = self.type_size_of(&common_inner).ok_or_else(|| {
                    Diagnostic::error(
                        "pointer subtraction requires pointers to compatible complete object types",
                        span,
                    )
                })?;
                let lhs_pointer = lhs.as_pointer(span)?;
                let rhs_pointer = rhs.as_pointer(span)?;
                if !self.pointers_share_arithmetic_domain(
                    &lhs_pointer,
                    &rhs_pointer,
                    &common_inner,
                    objects,
                ) {
                    return Err(Diagnostic::ub(
                        "pointer subtraction requires pointers into the same array object or one past it",
                        span,
                        Some("6.5.6"),
                    ));
                }
                let lhs_offset =
                    self.pointer_byte_offset(&lhs_pointer, &common_inner, objects).ok_or_else(|| {
                        Diagnostic::ub(
                            "pointer subtraction requires pointers into the same array object or one past it",
                            span,
                            Some("6.5.6"),
                        )
                    })?;
                let rhs_offset =
                    self.pointer_byte_offset(&rhs_pointer, &common_inner, objects).ok_or_else(|| {
                        Diagnostic::ub(
                            "pointer subtraction requires pointers into the same array object or one past it",
                            span,
                            Some("6.5.6"),
                        )
                    })?;
                let diff_bytes = lhs_offset as i128 - rhs_offset as i128;
                if diff_bytes % element_size as i128 != 0 {
                    return Err(Diagnostic::ub(
                        "pointer subtraction requires pointers into the same array object or one past it",
                        span,
                        Some("6.5.6"),
                    ));
                }
                Ok(TypedValue::integer(
                    CType::Long,
                    diff_bytes / element_size as i128,
                ))
            }
            _ => Err(Diagnostic::error(
                "unsupported operands for pointer arithmetic",
                span,
            )),
        }
    }

    fn eval_pointer_relational(
        &self,
        op: BinaryOp,
        lhs: TypedValue,
        rhs: TypedValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        self.reject_missing_return_value(&lhs, span)?;
        self.reject_missing_return_value(&rhs, span)?;
        self.reject_indeterminate_pointer_use(&lhs, span)?;
        self.reject_indeterminate_pointer_use(&rhs, span)?;
        let (CType::Pointer(lhs_inner), CType::Pointer(rhs_inner)) =
            (lhs.ty.unqualified(), rhs.ty.unqualified())
        else {
            return Err(Diagnostic::error(
                "relational pointer comparison requires two pointer operands",
                span,
            ));
        };
        if !self.pointer_targets_are_compatible_object_types(lhs_inner, rhs_inner, false) {
            return Err(Diagnostic::error(
                "relational comparison requires pointers to compatible object types",
                span,
            ));
        }
        let common_inner = self
            .composite_pointer_target_type(lhs_inner, rhs_inner)
            .ok_or_else(|| {
                Diagnostic::error(
                    "relational comparison requires pointers to compatible object types",
                    span,
                )
            })?;
        let common_ty = CType::pointer_to(common_inner.clone());
        let lhs = self.convert_value(lhs, &common_ty, span)?;
        let rhs = self.convert_value(rhs, &common_ty, span)?;
        let lhs_pointer = lhs.as_pointer(span)?;
        let rhs_pointer = rhs.as_pointer(span)?;
        if !self.pointers_share_arithmetic_domain(
            &lhs_pointer,
            &rhs_pointer,
            &common_inner,
            objects,
        ) {
            return Err(Diagnostic::ub(
                "relational pointer comparison requires pointers into the same array object or one past it",
                span,
                Some("6.5.8"),
            ));
        }
        let lhs_address = self.pointer_numeric_address(&lhs, &common_ty, span)?;
        let rhs_address = self.pointer_numeric_address(&rhs, &common_ty, span)?;
        let result = match op {
            BinaryOp::Less => lhs_address < rhs_address,
            BinaryOp::LessEqual => lhs_address <= rhs_address,
            BinaryOp::Greater => lhs_address > rhs_address,
            BinaryOp::GreaterEqual => lhs_address >= rhs_address,
            _ => unreachable!(),
        };
        Ok(TypedValue::int(result as i128))
    }

    fn pointers_share_arithmetic_domain(
        &self,
        lhs: &PointerValue,
        rhs: &PointerValue,
        pointee_ty: &CType,
        objects: &ObjectFrames,
    ) -> bool {
        if lhs.object.is_none() || lhs.object != rhs.object {
            return false;
        }
        if pointee_ty.is_character()
            && lhs
                .object
                .and_then(|object_id| self.lookup_object(objects, object_id))
                .is_some_and(|object| {
                    object.storage_duration == StorageDuration::Dynamic
                        && matches!(
                            object.ty.unqualified(),
                            CType::Array(inner, _) if inner.is_character()
                        )
                        && self
                            .pointer_byte_offset(lhs, pointee_ty, objects)
                            .is_some_and(|offset| offset <= object.byte_size)
                        && self
                            .pointer_byte_offset(rhs, pointee_ty, objects)
                            .is_some_and(|offset| offset <= object.byte_size)
                })
        {
            return true;
        }
        let (Some(lhs_root), Some(rhs_root)) = (
            self.pointer_root_type(lhs, objects),
            self.pointer_root_type(rhs, objects),
        ) else {
            return false;
        };
        if !self.compatible_object_layout_types(lhs_root, rhs_root) {
            return false;
        }
        if let (Some(lhs_byte), Some(rhs_byte)) =
            (lhs.byte_offset_override, rhs.byte_offset_override)
        {
            if let (Some(lhs_start), Some(rhs_start)) =
                (lhs.arithmetic_domain_start, rhs.arithmetic_domain_start)
            {
                return lhs_start == rhs_start;
            }
            let Some(element_size) = self.type_size_of(pointee_ty) else {
                return false;
            };
            let domain_start = |pointer: &PointerValue, byte: usize| {
                usize::try_from(pointer.offset)
                    .ok()
                    .and_then(|offset| offset.checked_mul(element_size))
                    .and_then(|offset| byte.checked_sub(offset))
            };
            return matches!(
                (domain_start(lhs, lhs_byte), domain_start(rhs, rhs_byte)),
                (Some(lhs_start), Some(rhs_start)) if lhs_start == rhs_start
            );
        }
        if lhs.member_path.is_empty()
            && rhs.member_path.is_empty()
            && matches!(
                lhs_root.unqualified(),
                CType::Array(inner, _)
                    if self.compatible_object_layout_types(inner, pointee_ty)
            )
        {
            return true;
        }
        if lhs.base_offset == rhs.base_offset
            && self.member_pointer_paths_have_defined_order(
                lhs_root,
                &lhs.member_path,
                &rhs.member_path,
            )
        {
            return true;
        }
        lhs.base_offset == rhs.base_offset && lhs.member_path == rhs.member_path
    }

    fn member_pointer_paths_have_defined_order(
        &self,
        root_ty: &CType,
        lhs_path: &[String],
        rhs_path: &[String],
    ) -> bool {
        let common = lhs_path
            .iter()
            .zip(rhs_path)
            .take_while(|(lhs, rhs)| lhs == rhs)
            .count();
        let Some(parent_ty) = self.storage_path_type(root_ty, &lhs_path[..common]) else {
            return false;
        };
        if !matches!(
            parent_ty.unqualified(),
            CType::Struct(_, _) | CType::Union(_, _)
        ) {
            return false;
        }
        self.path_designates_effective_direct_member(&parent_ty, &lhs_path[common..])
            && self.path_designates_effective_direct_member(&parent_ty, &rhs_path[common..])
    }

    fn path_designates_effective_direct_member(&self, parent_ty: &CType, path: &[String]) -> bool {
        let Some((first, rest)) = path.split_first() else {
            return false;
        };
        let Some(mut member) = self.direct_member_by_storage_name(parent_ty, first) else {
            return false;
        };
        for storage_name in rest {
            // Only anonymous aggregate membership is promoted into the
            // containing aggregate. Ordinary nested members remain members of
            // their immediate nested object.
            if member.name.is_some() {
                return false;
            }
            let Some(next) = self.direct_member_by_storage_name(&member.ty, storage_name) else {
                return false;
            };
            member = next;
        }
        true
    }

    pub(super) fn zero_value(&self, ty: &CType) -> TypedValue {
        match ty.unqualified() {
            CType::Void => TypedValue::void(),
            CType::Pointer(_) => TypedValue::pointer(ty.clone(), Self::null_pointer()),
            CType::Complex(_) => TypedValue::complex(ty.clone(), 0.0, 0.0),
            CType::Float | CType::Double | CType::LongDouble => {
                TypedValue::floating(ty.clone(), 0.0)
            }
            CType::VaList => TypedValue::from_data(ty.clone(), ValueData::Int(CIntValue::new(0))),
            CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _) => {
                TypedValue::aggregate(ty.clone(), self.zero_stored_value(ty))
            }
            _ => TypedValue::integer(ty.clone(), 0),
        }
    }

    fn default_object_value(&self, ty: &CType) -> StoredValue {
        self.indeterminate_stored_value(ty)
    }

    pub(super) fn object_pointer_limit(
        &self,
        objects: &ObjectFrames,
        pointer: &PointerValue,
        pointee_ty: &CType,
    ) -> Option<isize> {
        let object_id = pointer.object?;
        let object = self.lookup_object(objects, object_id)?;
        let root_ty = self
            .pointer_root_type(pointer, objects)
            .unwrap_or(&object.ty);
        let pointee_size = self.type_size_of(pointee_ty);
        if !pointer.member_path.is_empty() {
            if let Some(member_ty) = self.storage_path_type(root_ty, &pointer.member_path) {
                if let CType::Array(inner, len) = member_ty.unqualified() {
                    if self.compatible_object_layout_types(inner, pointee_ty) {
                        if *len != 0 {
                            return Some(*len as isize);
                        }
                        let member_offset =
                            self.member_path_offset(root_ty, &pointer.member_path)?;
                        let remaining = object.byte_size.saturating_sub(member_offset);
                        return Some((remaining / pointee_size?) as isize);
                    }
                }
                if let Some(limit) = self.subobject_pointer_limit_with_base(
                    &member_ty,
                    pointer.base_offset,
                    pointee_ty,
                ) {
                    return Some(limit);
                }
            }
            if matches!(
                pointee_ty.unqualified(),
                CType::Struct(_, _) | CType::Union(_, _)
            ) && self.pointer_targets_initial_member_chain(pointer, pointee_ty, objects)
            {
                return Some(match root_ty.unqualified() {
                    CType::Array(inner, len)
                        if self.compatible_object_layout_types(inner, pointee_ty) =>
                    {
                        (*len as isize) - pointer.base_offset
                    }
                    _ => 1,
                });
            }
            return Some(1);
        }
        Some(match (root_ty.unqualified(), pointee_ty.unqualified()) {
            _ if pointee_ty.is_character() => {
                (object.byte_size / pointee_size?) as isize - pointer.base_offset
            }
            _ if object.storage_duration == StorageDuration::Dynamic => {
                (object.byte_size / pointee_size?) as isize - pointer.base_offset
            }
            _ if self.compatible_object_layout_types(root_ty, pointee_ty) => {
                1 - pointer.base_offset
            }
            (CType::Array(inner, len), _)
                if self.compatible_object_layout_types(inner, pointee_ty) =>
            {
                if pointer.designated_root_ty.is_some() {
                    *len as isize
                } else {
                    (*len as isize) - pointer.base_offset
                }
            }
            (CType::Array(_, _), _) => {
                self.subobject_pointer_limit_with_base(root_ty, pointer.base_offset, pointee_ty)?
            }
            _ => return None,
        })
    }

    fn subobject_pointer_limit_with_base(
        &self,
        base_ty: &CType,
        base_offset: isize,
        pointee_ty: &CType,
    ) -> Option<isize> {
        match base_ty.unqualified() {
            CType::Array(inner, len) if self.compatible_object_layout_types(inner, pointee_ty) => {
                Some((*len as isize) - base_offset)
            }
            CType::Array(inner, len) => {
                let index = usize::try_from(base_offset).ok()?;
                if index >= *len {
                    return None;
                }
                self.subobject_pointer_limit(inner, pointee_ty)
            }
            _ if base_offset == 0 => self.subobject_pointer_limit(base_ty, pointee_ty),
            _ => None,
        }
    }

    fn subobject_pointer_limit(&self, base_ty: &CType, pointee_ty: &CType) -> Option<isize> {
        if pointee_ty.is_character() {
            let pointee_size = self.type_size_of(pointee_ty)?;
            return Some((self.type_size_of(base_ty)? / pointee_size) as isize);
        }
        match base_ty.unqualified() {
            CType::Array(inner, len) if self.compatible_object_layout_types(inner, pointee_ty) => {
                Some(*len as isize)
            }
            CType::Array(inner, _) => self.subobject_pointer_limit(inner, pointee_ty),
            _ if self.compatible_object_layout_types(base_ty, pointee_ty) => Some(1),
            _ => None,
        }
    }

    pub(super) fn storage_path_type(&self, ty: &CType, path: &[String]) -> Option<CType> {
        if path.is_empty() {
            return Some(ty.clone());
        }
        match ty.unqualified() {
            CType::Array(inner, _) => self.storage_path_type(inner, path),
            CType::Struct(_, _) | CType::Union(_, _) => {
                let member = self.direct_member_by_storage_name(ty, &path[0])?;
                let member_ty = self.qualified_member_type(ty, member);
                self.storage_path_type(&member_ty, &path[1..])
            }
            _ => None,
        }
    }

    fn lvalue_declared_type_is_volatile(&self, lvalue: &LValue, object_ty: &CType) -> bool {
        let root_ty = lvalue.designated_root_ty.as_deref().unwrap_or(object_ty);
        let mut declared_ty = self
            .storage_path_type(root_ty, &lvalue.member_path)
            .unwrap_or_else(|| root_ty.clone());
        while let CType::Array(inner, _) = declared_ty.unqualified() {
            if self.cross_unit_tagged_type_compatible(&declared_ty, &lvalue.ty) {
                break;
            }
            declared_ty = (**inner).clone();
        }
        declared_ty.is_volatile_qualified()
    }

    pub(super) fn checked_pointer_offset(
        &self,
        pointer: PointerValue,
        delta: i128,
        pointee_ty: &CType,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<PointerValue, Diagnostic> {
        let object = pointer.object.ok_or_else(|| {
            Diagnostic::ub("pointer arithmetic on a null pointer", span, Some("6.5.6"))
        })?;
        if self.lookup_object(objects, object).is_none() {
            return Err(Diagnostic::ub(
                "pointer arithmetic on a pointer to an object whose lifetime has ended",
                span,
                Some("6.5.6"),
            ));
        }
        if let Some(current_byte) = pointer.byte_offset_override {
            let object_state = self.lookup_object(objects, object).ok_or_else(|| {
                Diagnostic::ub(
                    "pointer arithmetic on a pointer to an object whose lifetime has ended",
                    span,
                    Some("6.5.6"),
                )
            })?;
            let root_ty = self
                .pointer_root_type(&pointer, objects)
                .unwrap_or(&object_state.ty);
            let element_size = self.type_size_of(pointee_ty).ok_or_else(|| {
                Diagnostic::ub(
                    format!(
                        "pointer arithmetic cannot be performed because pointed-to type {pointee_ty} is incomplete"
                    ),
                    span,
                    Some("6.5.6"),
                )
            })?;
            let delta_bytes = usize::try_from(delta.unsigned_abs())
                .ok()
                .and_then(|magnitude| magnitude.checked_mul(element_size))
                .ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?;
            let new_byte = if delta >= 0 {
                current_byte.checked_add(delta_bytes).ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?
            } else {
                current_byte.checked_sub(delta_bytes).ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?
            };
            if let (Some(domain_start), Some(domain_ty)) = (
                pointer.arithmetic_domain_start,
                pointer.designated_root_ty.as_deref(),
            ) && matches!(domain_ty.unqualified(), CType::Array(_, len) if *len != 0)
            {
                let domain_size = self.type_size_of(domain_ty).ok_or_else(|| {
                    Diagnostic::ub(
                        "pointer arithmetic cannot be performed within an incomplete object",
                        span,
                        Some("6.5.6"),
                    )
                })?;
                let domain_end = domain_start.checked_add(domain_size).ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?;
                if new_byte < domain_start || new_byte > domain_end {
                    return Err(Diagnostic::ub(
                        "pointer arithmetic produced a pointer outside the bounds of the designated subobject",
                        span,
                        Some("6.5.6"),
                    ));
                }
            }
            if let Some(limit) = self.object_pointer_limit(objects, &pointer, pointee_ty) {
                let delta = isize::try_from(delta).map_err(|_| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?;
                let new_offset = pointer.offset.checked_add(delta).ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?;
                if (0..=limit).contains(&new_offset) {
                    return Ok(PointerValue {
                        object: Some(object),
                        base_offset: pointer.base_offset,
                        offset: new_offset,
                        member_path: pointer.member_path,
                        designated_root_ty: pointer.designated_root_ty,
                        byte_offset_override: Some(new_byte),
                        arithmetic_domain_start: pointer.arithmetic_domain_start,
                    });
                }
            }
            let mut base_offset = pointer.base_offset;
            let mut offset = pointer.offset;
            let valid = match root_ty.unqualified() {
                CType::Array(inner, len)
                    if self.compatible_object_layout_types(inner, pointee_ty) =>
                {
                    if new_byte % element_size != 0 {
                        false
                    } else {
                        let index = new_byte / element_size;
                        if index <= *len {
                            base_offset = index as isize;
                            offset = 0;
                            true
                        } else {
                            false
                        }
                    }
                }
                _ if pointee_ty.is_character() => {
                    if new_byte <= object_state.byte_size {
                        base_offset = new_byte as isize;
                        offset = 0;
                        true
                    } else {
                        false
                    }
                }
                _ if object_state.storage_duration == StorageDuration::Dynamic => {
                    if new_byte <= object_state.byte_size {
                        // The allocated object may be a suballocation within raw dynamic
                        // storage, so its origin need not be an absolute multiple of its size.
                        // The checked delta is already an integral number of elements.
                        base_offset = 0;
                        offset = 0;
                        true
                    } else {
                        false
                    }
                }
                _ if self.compatible_object_layout_types(root_ty, pointee_ty) => {
                    if new_byte == 0 {
                        base_offset = 0;
                        offset = 0;
                        true
                    } else if new_byte == element_size {
                        base_offset = 0;
                        offset = 1;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if !valid {
                return Err(Diagnostic::ub(
                    "pointer arithmetic produced a pointer outside the bounds of the object",
                    span,
                    Some("6.5.6"),
                ));
            }
            return Ok(PointerValue {
                object: Some(object),
                base_offset,
                offset,
                member_path: pointer.member_path,
                designated_root_ty: pointer.designated_root_ty,
                byte_offset_override: Some(new_byte),
                arithmetic_domain_start: pointer.arithmetic_domain_start,
            });
        }
        let limit = self
            .object_pointer_limit(objects, &pointer, pointee_ty)
            .ok_or_else(|| {
                Diagnostic::ub(
                    "pointer arithmetic requires a pointer to an array of the pointed-to type, or a character pointer into an object representation",
                    span,
                    Some("6.5.6"),
                )
            })?;
        let delta = isize::try_from(delta)
            .map_err(|_| Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6")))?;
        let new_offset = pointer
            .offset
            .checked_add(delta)
            .ok_or_else(|| Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6")))?;
        if !(0..=limit).contains(&new_offset) {
            return Err(Diagnostic::ub(
                "pointer arithmetic produced a pointer outside the bounds of the object",
                span,
                Some("6.5.6"),
            ));
        }
        let byte_offset_override = if let Some(current) = pointer.byte_offset_override {
            let element_size = self.type_size_of(pointee_ty).ok_or_else(|| {
                Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
            })?;
            let delta_bytes = usize::try_from(delta.unsigned_abs())
                .ok()
                .and_then(|magnitude| magnitude.checked_mul(element_size))
                .ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?;
            Some(if delta >= 0 {
                current.checked_add(delta_bytes).ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?
            } else {
                current.checked_sub(delta_bytes).ok_or_else(|| {
                    Diagnostic::ub("pointer arithmetic overflow", span, Some("6.5.6"))
                })?
            })
        } else {
            None
        };
        Ok(PointerValue {
            object: Some(object),
            base_offset: pointer.base_offset,
            offset: new_offset,
            member_path: pointer.member_path,
            designated_root_ty: pointer.designated_root_ty,
            byte_offset_override,
            arithmetic_domain_start: pointer.arithmetic_domain_start,
        })
    }

    pub(super) fn pointer_equality_operand(
        &self,
        expr: &Expr,
        value: TypedValue,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<ComparablePointer, Diagnostic> {
        self.reject_missing_return_value(&value, span)?;
        self.reject_indeterminate_pointer_use(&value, span)?;
        match &value.data {
            ValueData::Pointer(pointer) if pointer.is_null() => Ok(ComparablePointer::Null),
            ValueData::Pointer(_) => Ok(ComparablePointer::Object(
                self.pointer_numeric_address(&value, &value.ty, span)?,
            )),
            ValueData::Function(name) => Ok(ComparablePointer::Function(name.to_string())),
            ValueData::ObjectRepresentation(_) if value.ty.is_pointer() => Err(Diagnostic::ub(
                "read of a pointer value with an invalid object representation",
                span,
                Some("6.2.6.1p5-6"),
            )),
            ValueData::Int(_) if self.is_null_pointer_constant(expr, frame, objects)? => {
                Ok(ComparablePointer::Null)
            }
            _ => Err(Diagnostic::error(
                "invalid operands for pointer comparison",
                span,
            )),
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

    pub(super) fn lookup_object<'b>(
        &'b self,
        objects: &'b ObjectFrames,
        object_id: ObjectId,
    ) -> Option<&'b ObjectState> {
        objects
            .get(object_id)
            .or_else(|| self.retired_objects.get(&object_id))
    }

    pub(super) fn lookup_object_mut<'b>(
        &'b mut self,
        objects: &'b mut ObjectFrames,
        object_id: ObjectId,
    ) -> Option<&'b mut ObjectState> {
        if let Some(object) = objects.get_mut(object_id) {
            return Some(object);
        }
        self.retired_objects.get_mut(&object_id)
    }

    fn lookup_active_object<'b>(
        &self,
        objects: &'b ObjectFrames,
        object_id: ObjectId,
    ) -> Option<&'b ObjectState> {
        objects.get(object_id)
    }

    fn lookup_active_object_mut<'b>(
        &self,
        objects: &'b mut ObjectFrames,
        object_id: ObjectId,
    ) -> Option<&'b mut ObjectState> {
        objects.get_mut(object_id)
    }

    fn retire_current_frame_objects(&mut self, objects: &mut ObjectFrames) {
        let Some(frame_objects) = objects.pop_frame() else {
            return;
        };
        let retired_ids = frame_objects.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        self.note_stream_buffer_lifetime_end(&retired_ids);
        for (id, object) in frame_objects {
            let reusable = !object.address_taken;
            self.retire_automatic_object(id, object);
            if reusable {
                self.reusable_automatic_object_ids.push(id);
            }
        }
        self.forget_retired_restrict_sources(&retired_ids);
    }

    pub(super) fn retire_automatic_object(&mut self, id: ObjectId, mut object: ObjectState) {
        debug_assert_eq!(object.storage_duration, StorageDuration::Automatic);
        self.end_live_non_dynamic_bytes(&object);
        if !object.address_taken {
            self.object_type_registry.remove(&id);
            self.object_base_addresses.remove(&id);
            return;
        }
        object.alive = false;
        object.value = StoredValue::Indeterminate;
        object.raw_indeterminate_bytes = None;
        object.effective_types.clear();
        object.pointer_slots.clear();
        self.retired_objects.insert(id, object);
    }

    fn clear_block_scopes_for_frame(&mut self, frame_id: usize) {
        self.active_block_scopes
            .retain(|scope| scope.frame_id != frame_id);
    }

    pub(super) fn push_scope_info(
        &mut self,
        frame: &Frame,
        span: Span,
        saves_bindings: bool,
        saves_function_decls: bool,
        track_new_objects: bool,
        objects: &ObjectFrames,
    ) {
        let saves_ordinary_bindings = saves_bindings || saves_function_decls;
        let existing_objects = track_new_objects.then(|| {
            objects
                .last_ids()
                .map(<[ObjectId]>::to_vec)
                .unwrap_or_default()
        });
        self.active_block_scopes.push(ActiveBlockScope {
            frame_id: frame.id,
            block_span: span,
            saved_bindings: saves_ordinary_bindings.then(|| frame.bindings.clone()),
            saved_object_decls: saves_ordinary_bindings.then(|| frame.object_decls.clone()),
            saved_function_decls: saves_ordinary_bindings.then(|| frame.function_decls.clone()),
            existing_objects,
        });
    }

    pub(super) fn push_block_scope(
        &mut self,
        frame: &Frame,
        block: &Block,
        objects: &ObjectFrames,
    ) {
        let mut saves_bindings = false;
        let mut saves_function_decls = false;
        for item in &block.items {
            match item {
                BlockItem::Declaration(_) => {
                    saves_bindings = true;
                }
                BlockItem::FunctionDeclaration(_) => {
                    saves_function_decls = true;
                }
                BlockItem::Statement(_) => {}
            }
            if saves_bindings && saves_function_decls {
                break;
            }
        }
        self.push_scope_info(
            frame,
            block.span,
            saves_bindings,
            saves_function_decls,
            true,
            objects,
        );
    }

    pub(super) fn active_block_scope(
        &self,
        frame_id: usize,
        block_span: Span,
    ) -> Option<&ActiveBlockScope> {
        self.active_block_scopes
            .iter()
            .rev()
            .find(|scope| scope.frame_id == frame_id && scope.block_span == block_span)
    }

    pub(super) fn pop_active_block_scope(
        &mut self,
        frame_id: usize,
        block_span: Span,
    ) -> Option<ActiveBlockScope> {
        match self.active_block_scopes.last() {
            Some(scope) if scope.frame_id == frame_id && scope.block_span == block_span => {
                self.active_block_scopes.pop()
            }
            _ => None,
        }
    }

    pub(super) fn retire_block_objects(
        &mut self,
        objects: &mut ObjectFrames,
        existing_objects: &[ObjectId],
    ) {
        let Some(frame_ids) = objects.last_ids() else {
            return;
        };
        let new_ids = if frame_ids.starts_with(existing_objects) {
            objects
                .frames
                .last_mut()
                .expect("current object frame exists")
                .split_off(existing_objects.len())
        } else {
            let existing: HashSet<ObjectId> = existing_objects.iter().copied().collect();
            frame_ids
                .iter()
                .copied()
                .filter(|id| !existing.contains(id))
                .collect::<Vec<_>>()
        };
        self.note_stream_buffer_lifetime_end(&new_ids);
        for &id in &new_ids {
            let object = if objects.contains_in_last(id) {
                objects.remove_from_last(id)
            } else {
                objects.states.get_mut(id.0).and_then(Option::take)
            };
            if let Some(object) = object {
                self.retire_automatic_object(id, object);
            }
        }
        self.forget_retired_restrict_sources(&new_ids);
    }

    pub(super) fn restore_block_scope(
        &mut self,
        scope: ActiveBlockScope,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) {
        if let Some(existing_objects) = scope.existing_objects.as_deref() {
            self.retire_block_objects(objects, existing_objects);
        }
        if let Some(saved_bindings) = scope.saved_bindings {
            frame.bindings = saved_bindings;
        }
        if let Some(saved_object_decls) = scope.saved_object_decls {
            frame.object_decls = saved_object_decls;
        }
        if let Some(saved_function_decls) = scope.saved_function_decls {
            frame.function_decls = saved_function_decls;
        }
    }

    pub(super) fn begin_full_expression(&mut self) {
        self.expr_state.accesses.clear();
        self.expr_state.track_unsequenced_accesses = true;
        self.assignment_targets.clear();
    }

    pub(super) fn begin_full_expression_for(&mut self, expr: &Expr) {
        self.expr_state.accesses.clear();
        self.expr_state.track_unsequenced_accesses = !cfg!(feature = "pure-expression-precompute")
            || !self.run_options.optimizing_precomputations
            || self.expression_has_side_effects(expr);
        self.assignment_targets.clear();
    }

    pub(super) fn end_full_expression(&mut self) {
        self.expr_state.accesses.clear();
        self.expr_state.track_unsequenced_accesses = true;
        self.assignment_targets.clear();
        for object in self.full_expression_temporaries.drain(..) {
            if let Some(state) = self.retired_objects.get_mut(&object)
                && state.alive
            {
                state.alive = false;
                self.live_non_dynamic_bytes =
                    self.live_non_dynamic_bytes.saturating_sub(state.byte_size);
            }
        }
    }

    fn clear_setjmp_contexts_for_frame(&mut self, frame_id: usize) {
        self.active_setjmp_contexts
            .retain(|context| context.frame_id != frame_id);
    }

    pub(super) fn cleanup_call_frame(&mut self, frame_id: usize, objects: &mut ObjectFrames) {
        self.pending_longjmp_return = None;
        self.clear_setjmp_contexts_for_frame(frame_id);
        self.clear_block_scopes_for_frame(frame_id);
        self.live_setjmp_frames.remove(&frame_id);
        let _ = self.current_frame_ids.pop();
        let _ = self.current_functions.pop();
        let _ = self.current_variadic_args.pop();
        let _ = self.restrict_trackers.pop();
        self.va_lists
            .retain(|_, cursor| cursor.owner_frame_id != frame_id);
        self.automatic_object_bindings
            .retain(|(owner_frame, _), _| *owner_frame != frame_id);
        self.compound_literal_bindings
            .retain(|(owner_frame, _), _| *owner_frame != frame_id);
        self.retire_current_frame_objects(objects);
    }

    pub(super) fn current_frame_id(&self) -> Option<usize> {
        self.current_frame_ids.last().copied()
    }

    pub(super) fn capture_setjmp_environment(
        &self,
        frame: &Frame,
        objects: &ObjectFrames,
        context: &ActiveSetjmpContext,
    ) -> Result<SetjmpEnvironment, Diagnostic> {
        let Some(frame_ids) = objects.last_ids() else {
            return Err(Diagnostic::error(
                "internal error: setjmp requires an active function frame",
                context.stmt_span,
            ));
        };
        let mut object_snapshots = HashMap::default();
        let mut object_versions = HashMap::default();
        let mut variably_modified_object_ids = Vec::new();
        for &id in frame_ids {
            let state = objects
                .get(id)
                .expect("current frame object ID has live state");
            object_snapshots.insert(id, state.clone());
            object_versions.insert(id, state.modification_count);
            if state.variably_modified {
                variably_modified_object_ids.push(id);
            }
        }
        Ok(SetjmpEnvironment {
            frame_id: frame.id,
            site: SetjmpSite {
                stmt_span: context.stmt_span,
                kind: context.kind,
            },
            bindings: frame.bindings.clone(),
            object_decls: frame.object_decls.clone(),
            function_decls: frame.function_decls.clone(),
            object_snapshots,
            object_versions,
            variably_modified_object_ids,
            block_scopes: self
                .active_block_scopes
                .iter()
                .filter(|scope| scope.frame_id == frame.id)
                .cloned()
                .collect(),
        })
    }

    pub(super) fn restore_setjmp_environment(
        &mut self,
        env: &SetjmpEnvironment,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        for &object_id in &env.variably_modified_object_ids {
            let still_active = objects.contains_in_last(object_id);
            if !still_active {
                return Err(Diagnostic::ub(
                    "longjmp was called after execution left the scope of a variably modified object that was in scope at setjmp",
                    span,
                    Some("7.13.2.1"),
                ));
            }
        }

        frame.bindings = env.bindings.clone();
        frame.object_decls = env.object_decls.clone();
        frame.function_decls = env.function_decls.clone();

        let Some(frame_ids) = objects.last_ids() else {
            return Err(Diagnostic::error(
                "internal error: longjmp target frame is missing",
                span,
            ));
        };
        let snapshot_ids = env.object_snapshots.keys().copied().collect::<HashSet<_>>();
        let current_ids = frame_ids.to_vec();
        let mut retired_now = Vec::new();
        for id in current_ids {
            if !snapshot_ids.contains(&id) {
                if let Some(mut object) = objects.remove_from_last(id) {
                    self.end_live_non_dynamic_bytes(&object);
                    object.alive = false;
                    object.value = StoredValue::Indeterminate;
                    self.retired_objects.insert(id, object);
                    retired_now.push(id);
                }
            }
        }
        self.note_stream_buffer_lifetime_end(&retired_now);
        self.forget_retired_restrict_sources(&retired_now);

        for (&id, snapshot) in &env.object_snapshots {
            let current_active = objects.get(id).cloned();
            let current_active_missing = current_active.is_none();
            let current_retired = self.retired_objects.get(&id).cloned();
            let changed = current_active
                .as_ref()
                .map(|state| state.modification_count)
                .or_else(|| {
                    current_retired
                        .as_ref()
                        .map(|state| state.modification_count)
                })
                .is_some_and(|version| {
                    env.object_versions
                        .get(&id)
                        .is_some_and(|saved| version != *saved)
                });
            let changed_bytes_only_volatile = current_active
                .as_ref()
                .or(current_retired.as_ref())
                .is_some_and(|current| {
                    self.changed_bytes_are_all_volatile(snapshot, current, span)
                        .unwrap_or(false)
                });

            let previously_counted_size = current_active
                .as_ref()
                .or(current_retired.as_ref())
                .filter(|state| state.alive && state.storage_duration != StorageDuration::Dynamic)
                .map_or(0, |state| state.byte_size);
            let mut restored = if let Some(current) = current_active {
                let mut current = current;
                current.alive = true;
                current
            } else if let Some(mut retired) = self.retired_objects.remove(&id) {
                retired.alive = true;
                retired
            } else {
                snapshot.clone()
            };

            if current_active_missing {
                restored.value = snapshot.value.clone();
                restored.initialized = snapshot.initialized;
                restored.address_taken |= snapshot.address_taken;
            }

            if restored.storage_duration == StorageDuration::Automatic
                && !restored.ty.is_volatile_qualified()
                && changed
                && !changed_bytes_only_volatile
            {
                restored.value = self.indeterminate_stored_value(&restored.ty);
                restored.initialized = false;
                restored.indeterminate_reason = Some(
                    "read of automatic object whose value became indeterminate after longjmp because it was changed between setjmp and longjmp",
                );
            }

            let restored_size =
                if restored.alive && restored.storage_duration != StorageDuration::Dynamic {
                    restored.byte_size
                } else {
                    0
                };
            self.replace_live_non_dynamic_bytes(previously_counted_size, restored_size);
            if objects.get(id).is_some() {
                objects.states[id.0] = Some(restored);
            } else {
                objects.insert_last(id, restored);
            }
        }

        self.clear_setjmp_contexts_for_frame(env.frame_id);
        self.clear_block_scopes_for_frame(env.frame_id);
        self.active_block_scopes.extend(env.block_scopes.clone());
        self.begin_full_expression();
        Ok(())
    }

    fn changed_bytes_are_all_volatile(
        &mut self,
        snapshot: &ObjectState,
        current: &ObjectState,
        span: Span,
    ) -> Result<bool, Diagnostic> {
        let before = self.serialize_stored_value(&snapshot.ty, &snapshot.value, span)?;
        let after = self.serialize_stored_value(&current.ty, &current.value, span)?;
        if before.len() != after.len() {
            return Ok(false);
        }
        let changed = before
            .iter()
            .zip(&after)
            .enumerate()
            .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
            .collect::<Vec<_>>();
        Ok(!changed.is_empty()
            && changed.iter().all(|&offset| {
                self.qualified_subobject_overlaps(
                    &snapshot.ty,
                    0,
                    offset,
                    1,
                    TrackedQualifier::Volatile,
                )
            }))
    }

    pub(super) fn eval_in_setjmp_context(
        &mut self,
        expr: &Expr,
        kind: SetjmpContextKind,
        stmt_span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        self.begin_full_expression_for(expr);
        if !self.expr_requires_setjmp_context(expr) {
            let value = self.eval_rvalue(expr, frame, objects);
            self.end_full_expression();
            return value;
        }
        self.active_setjmp_contexts.push(ActiveSetjmpContext {
            frame_id: frame.id,
            root_expr: expr.clone(),
            stmt_span,
            kind,
        });
        let value = self.eval_rvalue(expr, frame, objects);
        let _ = self.active_setjmp_contexts.pop();
        self.end_full_expression();
        value
    }

    fn expr_requires_setjmp_context(&mut self, expr: &Expr) -> bool {
        let key = expr as *const Expr as usize;
        if let Some(&cached) = self.expr_setjmp_cache.get(&key) {
            return cached;
        }
        let contains = expr_contains_setjmp(expr);
        self.expr_setjmp_cache.insert(key, contains);
        contains
    }

    fn expression_has_side_effects(&mut self, expr: &Expr) -> bool {
        let key = expr as *const Expr as usize;
        if let Some(&cached) = self.expr_side_effect_cache.get(&key) {
            return cached;
        }
        let has_side_effects = cboxes_expression_has_side_effects(expr);
        self.expr_side_effect_cache.insert(key, has_side_effects);
        has_side_effects
    }

    fn setjmp_call_matches(&self, expr: &Expr, call_span: Span) -> bool {
        matches!(
            expr,
            Expr::Call { callee, span, .. }
                if *span == call_span
                    && matches!(
                        callee.as_ref(),
                        Expr::Variable(name, _) if name == "__codex_setjmp"
                    )
        )
    }

    fn expr_is_void_cast_of_setjmp(&self, expr: &Expr, call_span: Span) -> bool {
        if self.setjmp_call_matches(expr, call_span) {
            return true;
        }
        matches!(
            expr,
            Expr::Cast { ty, expr, .. }
                if *ty == CType::Void && self.expr_is_void_cast_of_setjmp(expr, call_span)
        )
    }

    pub(super) fn setjmp_call_allowed_in_context(
        &self,
        context: &ActiveSetjmpContext,
        call_span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<bool, Diagnostic> {
        let root = &context.root_expr;
        Ok(match context.kind {
            SetjmpContextKind::ExpressionStatement => {
                self.expr_is_void_cast_of_setjmp(root, call_span)
            }
            SetjmpContextKind::IfCondition
            | SetjmpContextKind::WhileCondition
            | SetjmpContextKind::DoWhileCondition
            | SetjmpContextKind::ForCondition
            | SetjmpContextKind::SwitchExpression => {
                self.setjmp_call_matches(root, call_span)
                    || matches!(
                        root,
                        Expr::Unary {
                            op: UnaryOp::LogicalNot,
                            expr,
                            ..
                        } if self.setjmp_call_matches(expr, call_span)
                    )
                    || matches!(
                        root,
                        Expr::Binary { op, lhs, rhs, .. }
                            if matches!(
                                op,
                                BinaryOp::Less
                                    | BinaryOp::LessEqual
                                    | BinaryOp::Greater
                                    | BinaryOp::GreaterEqual
                                    | BinaryOp::Equal
                                    | BinaryOp::NotEqual
                            ) && (
                                (self.setjmp_call_matches(lhs, call_span)
                                    && self.is_null_pointer_constant(rhs, frame, objects)?)
                                || (self.setjmp_call_matches(rhs, call_span)
                                    && self.is_null_pointer_constant(lhs, frame, objects)?)
                                || (self.setjmp_call_matches(lhs, call_span)
                                    && self.eval_integer_constant_expr(rhs, frame, objects).is_ok())
                                || (self.setjmp_call_matches(rhs, call_span)
                                    && self.eval_integer_constant_expr(lhs, frame, objects).is_ok())
                            )
                    )
            }
        })
    }

    pub(super) fn sequence_point(&mut self) {
        self.expr_state.accesses.clear();
    }

    pub(super) fn sequencing_snapshot(&self) -> SequencingSnapshot {
        SequencingSnapshot {
            accesses: self.expr_state.accesses.clone(),
            assignment_targets: self.assignment_targets.clone(),
        }
    }

    pub(super) fn finish_sequenced_operand(
        &mut self,
        snapshot: SequencingSnapshot,
    ) -> HashMap<AccessRegion, ObjectAccess> {
        let mut footprint = HashMap::default();
        for (&region, access) in &self.expr_state.accesses {
            let baseline = snapshot.accesses.get(&region).copied().unwrap_or_default();
            let delta = ObjectAccess {
                self_read: (access.self_read != baseline.self_read)
                    .then_some(access.self_read)
                    .flatten(),
                other_read: (access.other_read != baseline.other_read)
                    .then_some(access.other_read)
                    .flatten(),
                write: (access.write != baseline.write)
                    .then_some(access.write)
                    .flatten(),
            };
            if delta != ObjectAccess::default() {
                footprint.insert(region, delta);
            }
        }
        self.expr_state.accesses = snapshot.accesses;
        self.assignment_targets = snapshot.assignment_targets;
        footprint
    }

    pub(super) fn merge_sequenced_footprint(
        &mut self,
        footprint: HashMap<AccessRegion, ObjectAccess>,
    ) {
        for (region, mut access) in footprint {
            if self.assignment_targets.iter().any(|target| {
                Self::access_region_contains(target.region, region)
                    && target.kind == AssignmentTargetKind::Simple
            }) {
                access.write = None;
            }
            let current = self.expr_state.accesses.entry(region).or_default();
            current.self_read = current.self_read.or(access.self_read);
            current.other_read = current.other_read.or(access.other_read);
            current.write = current.write.or(access.write);
        }
    }

    pub(super) fn accumulate_initializer_footprint(
        &mut self,
        footprint: HashMap<AccessRegion, ObjectAccess>,
    ) {
        let Some(sequence) = self.initializer_sequencing.as_mut() else {
            return;
        };
        for (region, access) in footprint {
            let accumulated = sequence.footprint.entry(region).or_default();
            accumulated.self_read = accumulated.self_read.or(access.self_read);
            accumulated.other_read = accumulated.other_read.or(access.other_read);
            accumulated.write = accumulated.write.or(access.write);
        }
    }

    fn in_host_library_runtime(&self) -> bool {
        self.host_library_runtime_depth != 0
    }

    pub(super) fn push_assignment_target(
        &mut self,
        region: AccessRegion,
        kind: AssignmentTargetKind,
    ) {
        self.assignment_targets
            .push(AssignmentTarget { region, kind });
    }

    pub(super) fn pop_assignment_target(&mut self) {
        let _ = self.assignment_targets.pop();
    }

    pub(super) fn validate_restricted_pointer_assignment(
        &self,
        target: &LValue,
        source_value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !target.ty.is_pointer() || !target.ty.top_level_qualifiers().is_restrict {
            return Ok(());
        }
        let Some(source) = source_value.restrict_source.as_ref() else {
            return Ok(());
        };
        let same_restricted_pointer = source.object == target.object
            && source.base_offset == target.base_offset
            && source.offset == target.offset
            && source.member_path == target.member_path;
        if same_restricted_pointer {
            return Ok(());
        }
        let current_frame = self.current_frame_ids.last().copied();
        let block_rank = |object: ObjectId| {
            self.active_block_scopes
                .iter()
                .filter(|scope| Some(scope.frame_id) == current_frame)
                .enumerate()
                .filter_map(|(rank, scope)| {
                    scope
                        .existing_objects
                        .as_ref()
                        .is_some_and(|existing| !existing.contains(&object))
                        .then_some(rank)
                })
                .last()
                .unwrap_or(0)
        };
        if block_rank(source.object) >= block_rank(target.object) {
            return Err(Diagnostic::ub(
                "assignment between restrict-qualified pointer objects whose associated blocks do not have the required ordering",
                span,
                Some("6.7.3.1"),
            ));
        }
        Ok(())
    }

    pub(super) fn validate_simple_assignment_overlap(
        &self,
        target: &LValue,
        source: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let target_region = self.lvalue_access_region(target, objects, span)?;
        let source_region = self.lvalue_access_region(source, objects, span)?;
        if !Self::access_regions_overlap(target_region, source_region) {
            return Ok(());
        }
        let exact_overlap = target_region == source_region;
        let compatible_types = self
            .cross_unit_tagged_type_compatible(target.ty.unqualified(), source.ty.unqualified());
        if !exact_overlap || !compatible_types {
            return Err(Diagnostic::ub(
                "assignment reads its stored value from an overlapping object without exact overlap and compatible type",
                span,
                Some("6.5.16.1"),
            ));
        }
        Ok(())
    }

    fn validate_atomic_member_access(
        &self,
        lvalue: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let Some(object) = self.lookup_object(objects, lvalue.object) else {
            return Ok(());
        };
        let mut ty = lvalue
            .designated_root_ty
            .as_deref()
            .unwrap_or(&object.ty)
            .clone();
        for member in lvalue.member_path.iter() {
            while let CType::Array(inner, _) = ty.unqualified() {
                ty = (**inner).clone();
            }
            if ty.top_level_qualifiers().is_atomic {
                return Err(Diagnostic::ub(
                    "access to a member of an atomic structure or union object",
                    span,
                    Some("6.5.2.3p5"),
                ));
            }
            let Some(next) = self.direct_member_by_storage_name(&ty, member) else {
                break;
            };
            ty = next.ty.clone();
        }
        Ok(())
    }

    fn is_assignment_target(&self, region: AccessRegion) -> bool {
        self.assignment_targets
            .iter()
            .any(|target| Self::access_regions_overlap(target.region, region))
    }

    fn access_regions_overlap(lhs: AccessRegion, rhs: AccessRegion) -> bool {
        lhs.object == rhs.object
            && lhs.bit_size != 0
            && rhs.bit_size != 0
            && lhs.bit_start < rhs.bit_start.saturating_add(rhs.bit_size)
            && rhs.bit_start < lhs.bit_start.saturating_add(lhs.bit_size)
    }

    fn access_region_contains(outer: AccessRegion, inner: AccessRegion) -> bool {
        outer.object == inner.object
            && inner.bit_start >= outer.bit_start
            && inner.bit_start.saturating_add(inner.bit_size)
                <= outer.bit_start.saturating_add(outer.bit_size)
    }

    fn lvalue_bit_field_offset(&self, lvalue: &LValue, objects: &ObjectFrames) -> Option<usize> {
        lvalue.bit_field_width?;
        let object_ty = &self.lookup_object(objects, lvalue.object)?.ty;
        let mut ty = lvalue.designated_root_ty.as_deref().unwrap_or(object_ty);
        let mut bit_offset = None;
        for storage_name in lvalue.member_path.iter() {
            while let CType::Array(inner, _) = ty.unqualified() {
                ty = inner;
            }
            let member = self.direct_member_by_storage_name(ty, storage_name)?;
            bit_offset = Some(member.bit_offset as usize);
            ty = &member.ty;
        }
        bit_offset
    }

    pub(super) fn lvalue_access_region(
        &self,
        lvalue: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<AccessRegion, Diagnostic> {
        let (_, start, size) = self
            .lvalue_byte_range(lvalue, &lvalue.ty, objects)
            .ok_or_else(|| {
                Diagnostic::ub("pointer is not valid to access", span, Some("6.5.3.2"))
            })?;
        self.lvalue_access_region_from_range(lvalue, start, size, objects, span)
    }

    fn lvalue_access_region_from_range(
        &self,
        lvalue: &LValue,
        start: usize,
        size: usize,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<AccessRegion, Diagnostic> {
        let mut bit_start = start.checked_mul(8).ok_or_else(|| {
            Diagnostic::ub("object access is out of supported range", span, Some("6.5"))
        })?;
        let bit_size = if let Some(width) = lvalue.bit_field_width {
            bit_start = bit_start
                .checked_add(self.lvalue_bit_field_offset(lvalue, objects).unwrap_or(0))
                .ok_or_else(|| {
                    Diagnostic::ub("object access is out of supported range", span, Some("6.5"))
                })?;
            width as usize
        } else {
            size.checked_mul(8).ok_or_else(|| {
                Diagnostic::ub("object access is out of supported range", span, Some("6.5"))
            })?
        };
        Ok(AccessRegion {
            object: lvalue.object,
            bit_start,
            bit_size,
        })
    }

    fn record_read(&mut self, region: AccessRegion, span: Span) -> Result<(), Diagnostic> {
        if !self.expr_state.track_unsequenced_accesses || self.in_host_library_runtime() {
            return Ok(());
        }
        let is_target = self.is_assignment_target(region);
        if let Some(write_span) = self
            .expr_state
            .accesses
            .iter()
            .filter(|(existing, _)| Self::access_regions_overlap(**existing, region))
            .find_map(|(_, access)| access.write)
        {
            return Err(self.unsequenced_access_diag(
                "unsequenced read of an object after it was modified",
                span,
                write_span,
            ));
        }
        let access = self.expr_state.accesses.entry(region).or_default();
        if is_target {
            access.self_read.get_or_insert(span);
        } else {
            access.other_read.get_or_insert(span);
        }
        Ok(())
    }

    fn record_write(&mut self, region: AccessRegion, span: Span) -> Result<(), Diagnostic> {
        if !self.expr_state.track_unsequenced_accesses || self.in_host_library_runtime() {
            return Ok(());
        }
        if let Some(write_span) = self
            .expr_state
            .accesses
            .iter()
            .filter(|(existing, _)| Self::access_regions_overlap(**existing, region))
            .find_map(|(_, access)| access.write)
        {
            return Err(self.unsequenced_access_diag(
                "multiple unsequenced modifications of the same object",
                span,
                write_span,
            ));
        }
        if let Some(read_span) = self
            .expr_state
            .accesses
            .iter()
            .filter(|(existing, _)| Self::access_regions_overlap(**existing, region))
            .find_map(|(_, access)| access.other_read)
        {
            return Err(self.unsequenced_access_diag(
                "modification of an object is unsequenced relative to another access of the same object",
                span,
                read_span,
            ));
        }
        let access = self.expr_state.accesses.entry(region).or_default();
        access.write = Some(span);
        Ok(())
    }

    fn record_restrict_access(
        &mut self,
        lvalue: &LValue,
        effective_ty: &CType,
        is_write: bool,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if !self.restrict_tracking_required || self.in_host_library_runtime() {
            return Ok(());
        }
        let Some((object, start, size)) = self.lvalue_byte_range(lvalue, effective_ty, objects)
        else {
            return Ok(());
        };
        let Some(tracker) = self.restrict_trackers.last_mut() else {
            return Ok(());
        };
        let current_source = lvalue.restrict_source.clone();
        if is_write
            && current_source
                .as_ref()
                .is_some_and(|source| source.pointed_to_const)
        {
            return Err(Diagnostic::ub(
                "modification through a pointer based on a restrict-qualified pointer to const-qualified type",
                span,
                Some("6.7.3.1"),
            ));
        }
        if current_source.is_none() && tracker.source_objects.is_empty() {
            let entries = tracker.unrestricted_accesses.entry(object).or_default();
            if let Some(entry) = entries.get_mut(&(start, size)) {
                entry.saw_write |= is_write;
            } else {
                entries.insert(
                    (start, size),
                    RestrictAccess {
                        source: None,
                        start,
                        size,
                        saw_write: is_write,
                        span,
                    },
                );
            }
            return Ok(());
        }
        if let Some(source) = current_source.as_ref() {
            tracker.source_objects.insert(source.object);
        }
        let entries = tracker.accesses.entry(object).or_default();
        if let Some(unrestricted) = tracker.unrestricted_accesses.remove(&object) {
            entries.extend(unrestricted.into_values());
        }
        for entry in entries.iter() {
            let overlaps = start < entry.start.saturating_add(entry.size)
                && entry.start < start.saturating_add(size);
            let different_source = entry.source != current_source;
            let restrict_involved = entry.source.is_some() || current_source.is_some();
            if overlaps && different_source && restrict_involved && (is_write || entry.saw_write) {
                let previous = self.sources.snippet(entry.span);
                return Err(Diagnostic::ub(
                    "access to overlapping object through different restrict-qualified pointer bases",
                    span,
                    Some("6.7.3.1"),
                )
                .with_note(format!(
                    "conflicting prior access through a different base at {}:{}:{}",
                    previous.path.display(),
                    previous.line_number,
                    previous.column
                )));
            }
        }
        if let Some(entry) = entries.iter_mut().find(|entry| {
            entry.source == current_source && entry.start == start && entry.size == size
        }) {
            entry.saw_write |= is_write;
        } else {
            entries.push(RestrictAccess {
                source: current_source,
                start,
                size,
                saw_write: is_write,
                span,
            });
        }
        Ok(())
    }

    pub(super) fn forget_retired_restrict_sources(&mut self, retired_ids: &[ObjectId]) {
        if !self.restrict_tracking_required || retired_ids.is_empty() {
            return;
        }
        let retired_set =
            (retired_ids.len() > 8).then(|| retired_ids.iter().copied().collect::<HashSet<_>>());
        let contains_retired = |object: ObjectId| match retired_ids {
            [only] => *only == object,
            [first, second] => *first == object || *second == object,
            ids if ids.len() <= 8 => ids.contains(&object),
            _ => retired_set
                .as_ref()
                .is_some_and(|retired| retired.contains(&object)),
        };
        for tracker in &mut self.restrict_trackers {
            for &retired in retired_ids {
                tracker.accesses.remove(&retired);
                tracker.unrestricted_accesses.remove(&retired);
            }
            let retired_source = retired_ids
                .iter()
                .copied()
                .any(|retired| tracker.source_objects.remove(&retired));
            if !retired_source {
                continue;
            }
            tracker.accesses.retain(|_, entries| {
                entries.retain(|entry| {
                    entry
                        .source
                        .as_ref()
                        .is_none_or(|source| !contains_retired(source.object))
                });
                !entries.is_empty()
            });
        }
    }

    fn unsequenced_access_diag(
        &self,
        message: &'static str,
        current_span: Span,
        previous_span: Span,
    ) -> Diagnostic {
        let previous = self.sources.snippet(previous_span);
        Diagnostic::ub(message, current_span, Some("6.5p2")).with_note(format!(
            "previous unsequenced access at {}:{}:{}",
            previous.path.display(),
            previous.line_number,
            previous.column
        ))
    }
}
