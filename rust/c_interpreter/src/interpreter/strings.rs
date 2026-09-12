//! String literal storage and bounded character reads.

use super::*;

impl<'a> Interpreter<'a> {
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

    pub(super) fn intern_readonly_c_bytes(&mut self, bytes: &[u8]) -> PointerValue {
        let mut c_bytes = bytes.to_vec();
        c_bytes.push(0);
        let byte_size = c_bytes.len();
        let array_ty = CType::array_of(CType::Char, byte_size);
        let id = self.allocate_object_id();
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
        let id = self.allocate_object_id();
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
        let id = self.allocate_object_id();
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
            object_representation_domain: None,
            bit_field_width: None,
            restrict_source: None,
        }))
    }
}
