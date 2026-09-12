//! Object representations, lvalues, and memory loads and stores.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn synthetic_void_pointer(&self, address: u64) -> *mut c_void {
        std::ptr::without_provenance_mut::<c_void>(address as usize)
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
        if let Some(align) = self.type_align_of(target_inner)
            && byte_offset % align != 0
        {
            return Err(Diagnostic::ub(
                format!(
                    "pointer conversion yields a {} that is not correctly aligned",
                    target
                ),
                span,
                Some("6.3.2.3p7"),
            ));
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

    pub(super) fn pointer_member_byte_limit(
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

    pub(super) fn pointer_byte_offset_from_type(
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
        self.byte_region_from_pointer_with_options(pointer, size, span, objects, false, true)
    }

    /// Resolve a byte-counted library region. WG14 DR 0042 specifies the
    /// objects used by memcpy as the `n`-byte regions beginning at its pointer
    /// arguments, so a valid region may span nested array elements while
    /// remaining inside the containing member/storage object.
    pub(super) fn byte_region_from_pointer_untyped(
        &self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        let region =
            self.byte_region_from_pointer_with_options(pointer, size, span, objects, false, false)?;
        if pointer.arithmetic_domain_start.is_some()
            && let Some(object) = self.lookup_object(objects, region.0)
            && let Some(domain) =
                self.innermost_enclosing_record_member_domain(&object.ty, 0, region.1)
            && region
                .1
                .checked_add(size)
                .is_none_or(|end| end > domain.one_past_end)
        {
            return Err(Diagnostic::ub(
                format!(
                    "requested byte access of {} byte(s) exceeds the containing record member",
                    size
                ),
                span,
                Some("7.1.4"),
            ));
        }
        Ok(region)
    }

    fn innermost_enclosing_record_member_domain(
        &self,
        ty: &CType,
        base: usize,
        address: usize,
    ) -> Option<ByteDomain> {
        match ty.unqualified() {
            CType::Array(element_ty, len) if *len != 0 => {
                let element_size = self.type_size_of(element_ty)?;
                let relative = address.checked_sub(base)?;
                let index = relative / element_size;
                if index >= *len {
                    return None;
                }
                self.innermost_enclosing_record_member_domain(
                    element_ty,
                    base.checked_add(index.checked_mul(element_size)?)?,
                    address,
                )
            }
            CType::Struct(_, _) | CType::Union(_, _) => {
                let record = self.record_type(ty)?;
                // A structure has at most one containing member. Union members
                // overlap; choosing the narrowest matching member is the safe
                // interpretation when the pointer's selected member path has
                // already been rebased into an array root.
                let mut candidates = record
                    .members
                    .iter()
                    .filter(|member| member.bit_width.is_none())
                    .filter_map(|member| {
                        let start = base.checked_add(member.offset)?;
                        let one_past_end = start.checked_add(self.type_size_of(&member.ty)?)?;
                        (address >= start && address < one_past_end).then_some((
                            ByteDomain {
                                start,
                                one_past_end,
                            },
                            &member.ty,
                        ))
                    })
                    .collect::<Vec<_>>();
                candidates.sort_by_key(|(domain, _)| domain.one_past_end - domain.start);
                let (member_domain, member_ty) = candidates.into_iter().next()?;
                self.innermost_enclosing_record_member_domain(
                    member_ty,
                    member_domain.start,
                    address,
                )
                .or(Some(member_domain))
            }
            _ => None,
        }
    }

    pub(super) fn byte_region_from_pointer_allow_reserved(
        &self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(ObjectId, usize, usize), Diagnostic> {
        self.byte_region_from_pointer_with_options(pointer, size, span, objects, true, true)
    }

    pub(super) fn byte_region_from_pointer_with_options(
        &self,
        pointer: &PointerValue,
        size: usize,
        span: Span,
        objects: &ObjectFrames,
        allow_reserved_buffer: bool,
        enforce_designated_subobject: bool,
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
        if enforce_designated_subobject
            && pointer
                .object_representation_domain
                .and_then(ObjectRepresentationDomain::active)
                .is_some_and(|domain| start < domain.start || end > domain.one_past_end)
        {
            return Err(Diagnostic::ub(
                format!(
                    "requested byte access of {} byte(s) exceeds the converted object's representation",
                    size
                ),
                span,
                Some("6.3.2.3"),
            ));
        }
        if enforce_designated_subobject
            && let (Some(domain_start), Some(domain_ty)) = (
                pointer.arithmetic_domain_start,
                pointer.designated_root_ty.as_deref(),
            )
        {
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
            let end = start.checked_add(size)?;
            if lvalue
                .object_representation_domain
                .and_then(ObjectRepresentationDomain::active)
                .is_some_and(|domain| start < domain.start || end > domain.one_past_end)
            {
                return None;
            }
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
            object_representation_domain: lvalue.object_representation_domain,
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

    pub(super) fn typed_value_from_stored_view(
        &self,
        ty: &CType,
        stored: StoredValueView<'_>,
    ) -> TypedValue {
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

    pub(super) fn resolve_lvalue_array_base_mut<'b>(
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
                let slot = self.extract_union_member(stored, member, span)?;
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

    pub(super) fn apply_lvalue_offset_mut<'b>(
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
                let slot = self.extract_union_member(stored, member, span)?;
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
            && let Some(reason) = indeterminate_reason
            && object_storage_duration == StorageDuration::Automatic
        {
            return Err(Diagnostic::ub(reason, span, Some("7.13.2.1")));
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

    pub(super) fn store_value_impl(
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

    pub(super) fn default_object_value(&self, ty: &CType) -> StoredValue {
        self.indeterminate_stored_value(ty)
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
}
