//! Pointer domains, offsets, arithmetic, and comparisons.

use super::*;

impl<'a> Interpreter<'a> {
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
        // Library functions define their byte-addressed object from the supplied
        // pointer and byte count (DR 0042), rather than by replaying character-
        // pointer arithmetic from the caller's original conversion. The caller's
        // region has already been checked before a library-derived pointer is made.
        let mut library_pointer = pointer.clone();
        library_pointer.object_representation_domain = None;
        let mut result = self.checked_pointer_offset(
            library_pointer,
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

    pub(super) fn eval_pointer_arithmetic(
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

    pub(super) fn eval_pointer_relational(
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
                if let CType::Array(inner, len) = member_ty.unqualified()
                    && self.compatible_object_layout_types(inner, pointee_ty)
                {
                    if *len != 0 {
                        return Some(*len as isize);
                    }
                    let member_offset = self.member_path_offset(root_ty, &pointer.member_path)?;
                    let remaining = object.byte_size.saturating_sub(member_offset);
                    return Some((remaining / pointee_size?) as isize);
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
            if pointer
                .object_representation_domain
                .and_then(ObjectRepresentationDomain::active)
                .is_some_and(|domain| new_byte < domain.start || new_byte > domain.one_past_end)
            {
                return Err(Diagnostic::ub(
                    "pointer arithmetic left the bounds of the converted object's representation",
                    span,
                    Some("6.3.2.3"),
                ));
            }
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
                        object_representation_domain: pointer.object_representation_domain,
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
                object_representation_domain: pointer.object_representation_domain,
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
            let delta_bytes = delta
                .unsigned_abs()
                .checked_mul(element_size)
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
            object_representation_domain: pointer.object_representation_domain,
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
}
