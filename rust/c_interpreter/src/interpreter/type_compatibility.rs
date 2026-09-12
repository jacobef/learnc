//! Conditional conversions and composite pointer and tagged types.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn conditional_result_type(
        &self,
        then_expr: &Expr,
        else_expr: &Expr,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<CType, Diagnostic> {
        let then_ty = self.value_expr_type(then_expr, frame, objects)?;
        let else_ty = self.value_expr_type(else_expr, frame, objects)?;
        if then_ty.is_arithmetic() && else_ty.is_arithmetic() {
            self.usual_arithmetic_expr_type(then_expr, else_expr, span, frame, objects)
        } else if then_ty == else_ty {
            Ok(then_ty)
        } else if matches!(
            (then_ty.unqualified(), else_ty.unqualified()),
            (CType::Struct(..), CType::Struct(..)) | (CType::Union(..), CType::Union(..))
        ) && self
            .cross_unit_tagged_type_compatible(then_ty.unqualified(), else_ty.unqualified())
        {
            Ok(then_ty.unqualified().clone())
        } else if then_ty.is_pointer()
            && self.is_null_pointer_constant(else_expr, frame, objects)?
        {
            Ok(then_ty)
        } else if else_ty.is_pointer()
            && self.is_null_pointer_constant(then_expr, frame, objects)?
        {
            Ok(else_ty)
        } else if then_ty.is_pointer() && else_ty.is_pointer() {
            let CType::Pointer(then_inner) = then_ty.unqualified() else {
                unreachable!();
            };
            let CType::Pointer(else_inner) = else_ty.unqualified() else {
                unreachable!();
            };
            self.composite_pointer_target_type(then_inner, else_inner)
                .map(CType::pointer_to)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "conditional operator requires compatible pointer operand types",
                        span,
                    )
                })
        } else {
            Err(Diagnostic::error(
                "unsupported conditional operand types",
                span,
            ))
        }
    }

    pub(super) fn is_null_pointer_constant(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<bool, Diagnostic> {
        match expr {
            Expr::Cast { ty, expr, .. } if *ty == CType::pointer_to(CType::Void) => Ok(self
                .eval_integer_constant_expr(expr, frame, objects)
                .is_ok_and(|value| value == 0)),
            _ => Ok(self
                .eval_integer_constant_expr(expr, frame, objects)
                .is_ok_and(|value| value == 0)),
        }
    }

    pub(super) fn convert_value_in_context(
        &self,
        expr: &Expr,
        value: TypedValue,
        target: &CType,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if target.is_integer()
            && !matches!(target.unqualified(), CType::Bool)
            && value.ty.is_pointer()
        {
            return Err(Diagnostic::error(
                format!(
                    "cannot implicitly convert pointer type {} to integer type {}",
                    value.ty, target
                ),
                span,
            ));
        }
        if target.is_pointer()
            && value.ty.is_integer()
            && !self.is_null_pointer_constant(expr, frame, objects)?
        {
            let message = if matches!(
                expr,
                Expr::CharLiteral(..)
                    | Expr::WideCharLiteral(..)
                    | Expr::Utf16CharLiteral(..)
                    | Expr::Utf32CharLiteral(..)
            ) {
                format!("cannot convert a character constant to pointer type {target}")
            } else {
                format!(
                    "cannot implicitly convert integer expression of type {} to pointer type {target}; the expression is not a null pointer constant",
                    value.ty
                )
            };
            return Err(Diagnostic::error(message, span));
        }
        if target.is_pointer()
            && value.ty.is_pointer()
            && !self.is_null_pointer_constant(expr, frame, objects)?
            && !self.pointer_assignment_compatible(target, &value.ty)
        {
            return Err(Diagnostic::error(
                format!("cannot convert {} to {}", value.ty, target),
                span,
            ));
        }
        let converted = self.convert_value(value, target, span)?;
        Ok(self.specialize_dynamic_raw_storage_pointer(converted, target, objects))
    }

    pub(super) fn specialize_dynamic_raw_storage_pointer(
        &self,
        mut value: TypedValue,
        target: &CType,
        objects: &ObjectFrames,
    ) -> TypedValue {
        let CType::Pointer(target_inner) = target.unqualified() else {
            return value;
        };
        if !matches!(
            target_inner.unqualified(),
            CType::Struct(_, _) | CType::Union(_, _)
        ) {
            return value;
        }
        let ValueData::Pointer(pointer) = &mut value.data else {
            return value;
        };
        let pointer = Rc::make_mut(pointer);
        if pointer.is_null() {
            return value;
        }
        let Some(object_id) = pointer.object else {
            return value;
        };
        let Some(object) = self.lookup_object(objects, object_id) else {
            return value;
        };
        if object.storage_duration != StorageDuration::Dynamic {
            return value;
        }
        if !matches!(
            object.ty.unqualified(),
            CType::Array(inner, _) if inner.is_character()
        ) {
            return value;
        }
        if let Some(root) = pointer.designated_root_ty.as_ref() {
            if self.cross_unit_tagged_type_compatible(root, target_inner) {
                if !pointer.member_path.is_empty()
                    && self.path_is_initial_member_chain(root, &pointer.member_path)
                    && let (Some(byte), Some(domain_start), Some(element_size)) = (
                        pointer.byte_offset_override,
                        pointer.arithmetic_domain_start,
                        self.type_size_of(target_inner),
                    )
                    && let Some(relative) = byte.checked_sub(domain_start)
                    && relative % element_size == 0
                {
                    pointer.base_offset = 0;
                    pointer.offset = (relative / element_size) as isize;
                    Rc::make_mut(&mut pointer.member_path).clear();
                }
                return value;
            }
            if self
                .storage_path_type(root, &pointer.member_path)
                .is_some_and(|path_ty| {
                    self.cross_unit_tagged_type_compatible(&path_ty, target_inner)
                        || matches!(path_ty.unqualified(), CType::Array(inner, 0)
                                if self.cross_unit_tagged_type_compatible(inner, target_inner))
                })
            {
                return value;
            }
        }
        let byte_offset = pointer
            .byte_offset_override
            .or_else(|| {
                let root = pointer.designated_root_ty.as_ref()?;
                self.pointer_byte_offset_from_root_type(pointer, root)
            })
            .or_else(|| self.pointer_byte_offset(pointer, target_inner, objects));
        pointer.base_offset = 0;
        pointer.offset = 0;
        Rc::make_mut(&mut pointer.member_path).clear();
        pointer.designated_root_ty = Some(Rc::new((**target_inner).clone()));
        pointer.byte_offset_override = byte_offset;
        pointer.arithmetic_domain_start = byte_offset;
        value
    }

    pub(super) fn pointer_assignment_compatible(&self, target: &CType, source: &CType) -> bool {
        let CType::Pointer(target_inner) = target.unqualified() else {
            return false;
        };
        let CType::Pointer(source_inner) = source.unqualified() else {
            return false;
        };
        self.pointer_target_compatible(target_inner, source_inner)
    }

    pub(super) fn array_flatten_domain(&self, source: &CType, target: &CType) -> Option<CType> {
        let mut current = source;
        loop {
            let CType::Array(inner, _) = current.unqualified() else {
                return None;
            };
            if self.compatible_object_layout_types(inner, target) {
                return Some(current.clone());
            }
            current = inner;
        }
    }

    fn function_parameter_types_compatible(&self, lhs: &[CType], rhs: &[CType]) -> bool {
        if lhs.is_empty() {
            return self.function_prototype_compatible_with_unspecified_parameters(rhs);
        }
        if rhs.is_empty() {
            return self.function_prototype_compatible_with_unspecified_parameters(lhs);
        }
        lhs.len() == rhs.len()
            && lhs.iter().zip(rhs).all(|(lhs, rhs)| {
                self.cross_unit_tagged_type_compatible(lhs.unqualified(), rhs.unqualified())
            })
    }

    fn composite_function_parameter_types(&self, lhs: &[CType], rhs: &[CType]) -> Vec<CType> {
        let params = if lhs.is_empty() { rhs } else { lhs };
        params
            .iter()
            .map(|param| param.unqualified().clone())
            .collect()
    }

    pub(super) fn function_prototype_compatible_with_unspecified_parameters(
        &self,
        params: &[CType],
    ) -> bool {
        params == [CType::Void]
            || params.iter().all(|param| {
                !matches!(
                    param.unqualified(),
                    CType::Bool
                        | CType::Char
                        | CType::SignedChar
                        | CType::UnsignedChar
                        | CType::Short
                        | CType::UnsignedShort
                        | CType::Enum(..)
                        | CType::Float
                )
            })
    }

    pub(super) fn cross_unit_tagged_type_compatible(&self, lhs: &CType, rhs: &CType) -> bool {
        let mut seen_records = HashSet::default();
        self.cross_unit_tagged_type_compatible_inner(lhs, rhs, &mut seen_records)
    }

    pub(super) fn cross_unit_tagged_type_compatible_inner(
        &self,
        lhs: &CType,
        rhs: &CType,
        seen_records: &mut HashSet<(usize, usize)>,
    ) -> bool {
        match (lhs, rhs) {
            (
                CType::Qualified(lhs_inner, lhs_qualifiers),
                CType::Qualified(rhs_inner, rhs_qualifiers),
            ) => {
                lhs_qualifiers == rhs_qualifiers
                    && self.cross_unit_tagged_type_compatible_inner(
                        lhs_inner,
                        rhs_inner,
                        seen_records,
                    )
            }
            (CType::Qualified(_, _), _) | (_, CType::Qualified(_, _)) => false,
            (CType::Pointer(lhs_inner), CType::Pointer(rhs_inner)) => {
                self.cross_unit_tagged_type_compatible_inner(lhs_inner, rhs_inner, seen_records)
            }
            (CType::Array(lhs_inner, lhs_len), CType::Array(rhs_inner, rhs_len)) => {
                (lhs_len == rhs_len || *lhs_len == 0 || *rhs_len == 0)
                    && self.cross_unit_tagged_type_compatible_inner(
                        lhs_inner,
                        rhs_inner,
                        seen_records,
                    )
            }
            (
                CType::Function(lhs_return, lhs_params, lhs_variadic),
                CType::Function(rhs_return, rhs_params, rhs_variadic),
            ) => {
                lhs_variadic == rhs_variadic
                    && self.cross_unit_tagged_type_compatible_inner(
                        lhs_return,
                        rhs_return,
                        seen_records,
                    )
                    && if lhs_params.is_empty() {
                        self.function_prototype_compatible_with_unspecified_parameters(rhs_params)
                    } else if rhs_params.is_empty() {
                        self.function_prototype_compatible_with_unspecified_parameters(lhs_params)
                    } else {
                        lhs_params.len() == rhs_params.len()
                            && lhs_params.iter().zip(rhs_params.iter()).all(
                                |(lhs_param, rhs_param)| {
                                    self.cross_unit_tagged_type_compatible_inner(
                                        lhs_param.unqualified(),
                                        rhs_param.unqualified(),
                                        seen_records,
                                    )
                                },
                            )
                    }
            }
            (CType::Struct(lhs_id, lhs_tag), CType::Struct(rhs_id, rhs_tag)) => self
                .cross_unit_record_compatible(
                    *lhs_id,
                    lhs_tag.as_deref().map(String::as_str),
                    *rhs_id,
                    rhs_tag.as_deref().map(String::as_str),
                    seen_records,
                ),
            (CType::Union(lhs_id, lhs_tag), CType::Union(rhs_id, rhs_tag)) => self
                .cross_unit_record_compatible(
                    *lhs_id,
                    lhs_tag.as_deref().map(String::as_str),
                    *rhs_id,
                    rhs_tag.as_deref().map(String::as_str),
                    seen_records,
                ),
            (CType::Enum(lhs_id, lhs_tag), CType::Enum(rhs_id, rhs_tag)) => {
                if lhs_id == rhs_id {
                    true
                } else if lhs_tag != rhs_tag {
                    false
                } else {
                    let lhs = self.program.enums.get(lhs_id);
                    let rhs = self.program.enums.get(rhs_id);
                    lhs.zip(rhs)
                        .is_some_and(|(lhs, rhs)| lhs.complete == rhs.complete || lhs_tag.is_some())
                }
            }
            _ => lhs == rhs,
        }
    }

    fn cross_unit_record_compatible(
        &self,
        lhs_id: usize,
        lhs_tag: Option<&str>,
        rhs_id: usize,
        rhs_tag: Option<&str>,
        seen_records: &mut HashSet<(usize, usize)>,
    ) -> bool {
        if lhs_id == rhs_id {
            return true;
        }
        let Some(lhs) = self.program.records.get(&lhs_id) else {
            return false;
        };
        let Some(rhs) = self.program.records.get(&rhs_id) else {
            return false;
        };
        if lhs.kind != rhs.kind || lhs_tag != rhs_tag {
            return false;
        }
        let key = if lhs_id < rhs_id {
            (lhs_id, rhs_id)
        } else {
            (rhs_id, lhs_id)
        };
        if !seen_records.insert(key) {
            return true;
        }
        if !lhs.complete || !rhs.complete {
            return true;
        }
        lhs.members.len() == rhs.members.len()
            && lhs
                .members
                .iter()
                .zip(&rhs.members)
                .all(|(lhs_member, rhs_member)| {
                    lhs_member.name == rhs_member.name
                        && lhs_member.storage_name == rhs_member.storage_name
                        && lhs_member.offset == rhs_member.offset
                        && lhs_member.bit_width == rhs_member.bit_width
                        && lhs_member.bit_offset == rhs_member.bit_offset
                        && lhs_member.bit_storage_size == rhs_member.bit_storage_size
                        && self.cross_unit_tagged_type_compatible_inner(
                            &lhs_member.ty,
                            &rhs_member.ty,
                            seen_records,
                        )
                })
    }

    fn pointer_target_compatible(&self, target: &CType, source: &CType) -> bool {
        self.pointer_target_compatible_inner(target, source, false)
    }

    pub(super) fn pointer_targets_are_compatible_object_types(
        &self,
        lhs: &CType,
        rhs: &CType,
        require_complete: bool,
    ) -> bool {
        if matches!(lhs.unqualified(), CType::Void | CType::Function(..))
            || matches!(rhs.unqualified(), CType::Void | CType::Function(..))
            || self
                .composite_pointer_target_type_inner(lhs, rhs, false)
                .is_none()
        {
            return false;
        }
        !require_complete || (self.type_is_complete(lhs) && self.type_is_complete(rhs))
    }

    fn pointer_target_compatible_inner(
        &self,
        target: &CType,
        source: &CType,
        allow_array_flatten: bool,
    ) -> bool {
        let target_qualifiers = target.top_level_qualifiers();
        let source_qualifiers = source.top_level_qualifiers();
        if !target_qualifiers.contains(source_qualifiers) {
            return false;
        }
        if target.unqualified() == source.unqualified() {
            return true;
        }
        let directly_compatible = match (target.unqualified(), source.unqualified()) {
            (CType::Void, other) | (other, CType::Void) => !matches!(other, CType::Function(..)),
            (CType::Bool, CType::Bool) => true,
            (CType::Char, CType::Char)
            | (CType::SignedChar, CType::SignedChar)
            | (CType::UnsignedChar, CType::UnsignedChar)
            | (CType::Short, CType::Short)
            | (CType::UnsignedShort, CType::UnsignedShort)
            | (CType::Int, CType::Int)
            | (CType::UnsignedInt, CType::UnsignedInt)
            | (CType::Long, CType::Long)
            | (CType::UnsignedLong, CType::UnsignedLong)
            | (CType::LongLong, CType::LongLong)
            | (CType::UnsignedLongLong, CType::UnsignedLongLong)
            | (CType::Float, CType::Float)
            | (CType::Double, CType::Double)
            | (CType::LongDouble, CType::LongDouble) => true,
            (CType::Struct(lhs_id, _), CType::Struct(rhs_id, _))
            | (CType::Union(lhs_id, _), CType::Union(rhs_id, _))
            | (CType::Enum(lhs_id, _), CType::Enum(rhs_id, _)) => {
                lhs_id == rhs_id
                    || self.cross_unit_tagged_type_compatible(
                        target.unqualified(),
                        source.unqualified(),
                    )
            }
            (
                CType::Function(target_return, target_params, target_variadic),
                CType::Function(source_return, source_params, source_variadic),
            ) => {
                self.cross_unit_tagged_type_compatible(target_return, source_return)
                    && self.function_parameter_types_compatible(target_params, source_params)
                    && target_variadic == source_variadic
            }
            (CType::Pointer(target_inner), CType::Pointer(source_inner)) => {
                self.cross_unit_tagged_type_compatible(target_inner, source_inner)
            }
            (CType::Array(target_inner, target_len), CType::Array(source_inner, source_len))
                if target_len == source_len || *target_len == 0 || *source_len == 0 =>
            {
                self.pointer_target_compatible_inner(target_inner, source_inner, false)
            }
            _ => false,
        };
        if directly_compatible {
            return true;
        }
        allow_array_flatten
            && matches!(source.unqualified(), CType::Array(_, _))
            && if let CType::Array(source_inner, _) = source.unqualified() {
                self.pointer_target_compatible_inner(target, source_inner, true)
            } else {
                false
            }
    }

    pub(super) fn compatible_object_layout_types(&self, lhs: &CType, rhs: &CType) -> bool {
        match (lhs.unqualified(), rhs.unqualified()) {
            (CType::Array(lhs_inner, lhs_len), CType::Array(rhs_inner, rhs_len)) => {
                (*lhs_len == 0 || *rhs_len == 0 || lhs_len == rhs_len)
                    && self.compatible_object_layout_types(lhs_inner, rhs_inner)
            }
            _ => {
                lhs.unqualified() == rhs.unqualified()
                    || self.cross_unit_tagged_type_compatible(lhs.unqualified(), rhs.unqualified())
            }
        }
    }

    pub(super) fn corresponding_signed_unsigned_types(lhs: &CType, rhs: &CType) -> bool {
        matches!(
            (lhs.unqualified(), rhs.unqualified()),
            (CType::SignedChar, CType::UnsignedChar)
                | (CType::UnsignedChar, CType::SignedChar)
                | (CType::Short, CType::UnsignedShort)
                | (CType::UnsignedShort, CType::Short)
                | (CType::Int, CType::UnsignedInt)
                | (CType::UnsignedInt, CType::Int)
                | (CType::Long, CType::UnsignedLong)
                | (CType::UnsignedLong, CType::Long)
                | (CType::LongLong, CType::UnsignedLongLong)
                | (CType::UnsignedLongLong, CType::LongLong)
        )
    }

    pub(super) fn composite_pointer_target_type(&self, lhs: &CType, rhs: &CType) -> Option<CType> {
        self.composite_pointer_target_type_inner(lhs, rhs, false)
    }

    pub(super) fn composite_pointer_target_type_inner(
        &self,
        lhs: &CType,
        rhs: &CType,
        allow_array_flatten: bool,
    ) -> Option<CType> {
        let qualifiers = lhs.top_level_qualifiers().union(rhs.top_level_qualifiers());
        let merged = match (lhs.unqualified(), rhs.unqualified()) {
            (CType::Void, other) | (other, CType::Void)
                if !matches!(other, CType::Function(..)) =>
            {
                CType::Void
            }
            (CType::Bool, CType::Bool) => CType::Bool,
            (CType::Char, CType::Char) => CType::Char,
            (CType::SignedChar, CType::SignedChar) => CType::SignedChar,
            (CType::UnsignedChar, CType::UnsignedChar) => CType::UnsignedChar,
            (CType::Short, CType::Short) => CType::Short,
            (CType::UnsignedShort, CType::UnsignedShort) => CType::UnsignedShort,
            (CType::Int, CType::Int) => CType::Int,
            (CType::UnsignedInt, CType::UnsignedInt) => CType::UnsignedInt,
            (CType::Long, CType::Long) => CType::Long,
            (CType::UnsignedLong, CType::UnsignedLong) => CType::UnsignedLong,
            (CType::LongLong, CType::LongLong) => CType::LongLong,
            (CType::UnsignedLongLong, CType::UnsignedLongLong) => CType::UnsignedLongLong,
            (CType::Float, CType::Float) => CType::Float,
            (CType::Double, CType::Double) => CType::Double,
            (CType::LongDouble, CType::LongDouble) => CType::LongDouble,
            (CType::Struct(lhs, tag), CType::Struct(rhs, _)) if lhs == rhs => {
                CType::Struct(*lhs, tag.clone())
            }
            (CType::Union(lhs, tag), CType::Union(rhs, _)) if lhs == rhs => {
                CType::Union(*lhs, tag.clone())
            }
            (CType::Enum(lhs, tag), CType::Enum(rhs, _)) if lhs == rhs => {
                CType::Enum(*lhs, tag.clone())
            }
            (
                CType::Function(lhs_return, lhs_params, lhs_variadic),
                CType::Function(rhs_return, rhs_params, rhs_variadic),
            ) if lhs_return == rhs_return
                && self.function_parameter_types_compatible(lhs_params, rhs_params)
                && lhs_variadic == rhs_variadic =>
            {
                let params = self.composite_function_parameter_types(lhs_params, rhs_params);
                if *lhs_variadic {
                    CType::variadic_function((**lhs_return).clone(), params)
                } else {
                    CType::function((**lhs_return).clone(), params)
                }
            }
            (CType::Pointer(lhs_inner), CType::Pointer(rhs_inner)) => self
                .composite_pointer_target_type_inner(lhs_inner, rhs_inner, false)
                .map(CType::pointer_to)?,
            (CType::Array(lhs_inner, lhs_len), CType::Array(rhs_inner, rhs_len))
                if (lhs_len == rhs_len || *lhs_len == 0 || *rhs_len == 0)
                    && self
                        .composite_pointer_target_type_inner(lhs_inner, rhs_inner, false)
                        .is_some() =>
            {
                CType::array_of(
                    self.composite_pointer_target_type_inner(lhs_inner, rhs_inner, false)
                        .expect("guard verified compatible array element types"),
                    if *lhs_len == 0 { *rhs_len } else { *lhs_len },
                )
            }
            _ => {
                if allow_array_flatten {
                    let flattened = match (lhs.unqualified(), rhs.unqualified()) {
                        (CType::Array(lhs_inner, _), _) => {
                            self.composite_pointer_target_type_inner(lhs_inner, rhs, true)
                        }
                        (_, CType::Array(rhs_inner, _)) => {
                            self.composite_pointer_target_type_inner(lhs, rhs_inner, true)
                        }
                        _ => None,
                    };
                    if let Some(flattened) = flattened {
                        return Some(CType::qualified(
                            flattened.unqualified().clone(),
                            flattened.top_level_qualifiers().union(qualifiers),
                        ));
                    }
                }
                return None;
            }
        };
        Some(CType::qualified(merged, qualifiers))
    }
}
