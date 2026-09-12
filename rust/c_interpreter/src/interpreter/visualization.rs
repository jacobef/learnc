//! Browser state snapshots, traces, and expression inspection.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn capture_cboxes_main_state_if_needed(
        &mut self,
        block: &Block,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if !self.run_options.capture_visualization {
            return Ok(());
        }
        let Some(function) = self.current_functions.last() else {
            return Ok(());
        };
        if function.name != "main" || function.body.span != block.span {
            return Ok(());
        }
        self.cboxes_main_state = self.cboxes_frame_state(frame, objects, block.span)?;
        Ok(())
    }

    pub(super) fn trace_cboxes_main_event(
        &mut self,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.trace_cboxes_main_event_with_details(span, frame, objects, "statement", None)
    }

    fn trace_cboxes_main_event_with_kind(
        &mut self,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
        kind: &str,
    ) -> Result<(), Diagnostic> {
        self.trace_cboxes_main_event_with_details(span, frame, objects, kind, None)
    }

    pub(super) fn trace_cboxes_main_branch_event(
        &mut self,
        span: Span,
        skipped_span: Option<Span>,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.trace_cboxes_main_event_with_details(span, frame, objects, "branch", skipped_span)
    }

    fn trace_cboxes_main_event_with_details(
        &mut self,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
        kind: &str,
        skipped_span: Option<Span>,
    ) -> Result<(), Diagnostic> {
        if !self.run_options.capture_visualization {
            return Ok(());
        }
        let Some(function) = self.current_functions.last() else {
            return Ok(());
        };
        if function.name != "main" {
            return Ok(());
        }
        let (path, start_line, end_line) = self.sources.span_location(span);
        let trace_index = self.cboxes_trace.len();
        if self.cboxes_expression_result.is_none()
            && self
                .cboxes_expression_request
                .as_ref()
                .is_some_and(|request| request.event_index == trace_index)
        {
            let request = self
                .cboxes_expression_request
                .as_ref()
                .expect("expression request was checked")
                .clone();
            self.cboxes_expression_result =
                Some(self.evaluate_cboxes_expression(&request, frame, objects)?);
        }
        let skipped_range = skipped_span.map(|skipped_span| {
            let (file, start_line, start_column, end_line, end_column) =
                self.sources.span_display_range(skipped_span);
            ProgramSourceRange {
                file: file.display().to_string(),
                start_line,
                start_column,
                end_line,
                end_column,
            }
        });
        self.cboxes_trace.push(ProgramTraceEvent {
            file: path.display().to_string(),
            start_line: start_line.saturating_sub(1),
            end_line: end_line.saturating_sub(1),
            kind: kind.to_owned(),
            state: self.cboxes_frame_state(frame, objects, span)?,
            skipped_range,
        });
        Ok(())
    }

    fn evaluate_cboxes_expression(
        &mut self,
        request: &ProgramExpressionEvalRequest,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<ProgramExpressionResult, Diagnostic> {
        let expr = &request.expr;
        if cboxes_expression_has_side_effects(expr) {
            return Err(Diagnostic::error(
                "the expression box can only evaluate expressions that do not change program state",
                expr.span(),
            ));
        }
        let saved_expr_state = self.expr_state.clone();
        let saved_assignment_targets = self.assignment_targets.clone();
        let saved_restrict_trackers = self.restrict_trackers.clone();
        let saved_compound_literal_bindings = self.compound_literal_bindings.clone();
        let saved_next_object = self.next_object;
        let saved_reusable_automatic_object_ids = self.reusable_automatic_object_ids.clone();
        let saved_next_encoded_pointer = self.next_encoded_pointer;
        let saved_retired_objects = self.retired_objects.clone();
        let saved_live_non_dynamic_bytes = self.live_non_dynamic_bytes;
        let saved_object_type_registry = self.object_type_registry.clone();
        let saved_object_base_addresses = self.object_base_addresses.clone();
        let saved_encoded_object_pointers = self.encoded_object_pointers.clone();
        let saved_encoded_function_pointers = self.encoded_function_pointers.clone();
        let saved_decoded_pointers = self.decoded_pointers.clone();
        let saved_opaque_integer_pointer_addresses =
            self.opaque_integer_pointer_addresses.borrow().clone();

        let mut eval_frame = frame.clone();
        let mut eval_objects = objects.clone();
        let result = (|| {
            let value = self.eval(expr, &mut eval_frame, &mut eval_objects)?;
            let name = self.sources.span_text(expr.span()).trim().to_owned();
            let value_literal = request
                .value_literal_text
                .as_deref()
                .map(|text| self.cboxes_value_literal(text, expr.span()))
                .transpose()?;
            match value {
                ValueCategory::LValue(lvalue) => {
                    let ty = lvalue.ty.clone();
                    let address = self.cboxes_lvalue_address(&lvalue, &eval_objects);
                    let value = if matches!(ty.unqualified(), CType::Array(_, _)) {
                        address
                            .map(|address| address.to_string())
                            .unwrap_or_default()
                    } else {
                        self.cboxes_lvalue_value_string(&lvalue, expr.span(), &eval_objects)?
                    };
                    let (display_value, exact_value) =
                        self.cboxes_value_display_strings(&value, &ty);
                    Ok(ProgramExpressionResult {
                        kind: "lvalue".to_owned(),
                        ty: cboxes_type_string(&ty),
                        value,
                        address,
                        name,
                        value_literal,
                        display_value,
                        exact_value,
                        type_info: self.cboxes_type_info(&ty),
                    })
                }
                ValueCategory::RValue(value) => {
                    let ty = value.ty.clone();
                    let text = self.cboxes_typed_value_string(&value, &ty, expr.span())?;
                    let (display_value, exact_value) =
                        self.cboxes_value_display_strings(&text, &ty);
                    Ok(ProgramExpressionResult {
                        kind: "rvalue".to_owned(),
                        ty: cboxes_type_string(&ty),
                        value: text,
                        address: None,
                        name: String::new(),
                        value_literal,
                        display_value,
                        exact_value,
                        type_info: self.cboxes_type_info(&ty),
                    })
                }
            }
        })();

        self.expr_state = saved_expr_state;
        self.assignment_targets = saved_assignment_targets;
        self.restrict_trackers = saved_restrict_trackers;
        self.compound_literal_bindings = saved_compound_literal_bindings;
        self.next_object = saved_next_object;
        self.reusable_automatic_object_ids = saved_reusable_automatic_object_ids;
        self.next_encoded_pointer = saved_next_encoded_pointer;
        self.retired_objects = saved_retired_objects;
        self.live_non_dynamic_bytes = saved_live_non_dynamic_bytes;
        self.object_type_registry = saved_object_type_registry;
        self.object_base_addresses = saved_object_base_addresses;
        self.encoded_object_pointers = saved_encoded_object_pointers;
        self.encoded_function_pointers = saved_encoded_function_pointers;
        self.decoded_pointers = saved_decoded_pointers;
        *self.opaque_integer_pointer_addresses.borrow_mut() =
            saved_opaque_integer_pointer_addresses;

        result
    }

    fn cboxes_lvalue_value_string(
        &self,
        lvalue: &LValue,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<String, Diagnostic> {
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
        let (base_stored, base_ty, tail_offset) =
            self.resolve_lvalue_array_base(&object.value, &object.ty, lvalue, span)?;
        let (base_stored, base_ty, tail_offset) = if !lvalue.member_path.is_empty()
            && matches!(base_ty.unqualified(), CType::Array(_, _))
        {
            let (stored, element_ty) =
                self.apply_lvalue_offset(base_stored, &base_ty, tail_offset, span)?;
            (stored, element_ty, 0)
        } else {
            (base_stored, base_ty, tail_offset)
        };
        let (stored, ty) =
            self.extract_stored_subobject_view(base_stored, &base_ty, &lvalue.member_path, span)?;
        let (stored, effective_ty) =
            self.apply_lvalue_offset_view(stored, &ty, tail_offset, span)?;
        match stored {
            StoredValueView::Borrowed(stored) => {
                self.cboxes_stored_value_string(&effective_ty, stored, span)
            }
            StoredValueView::Owned(stored) => {
                self.cboxes_stored_value_string(&effective_ty, &stored, span)
            }
        }
    }

    pub(super) fn cboxes_value_literal(
        &self,
        text: &str,
        span: Span,
    ) -> Result<ProgramValueLiteral, Diagnostic> {
        let literal = parse_number_literal(text.to_owned(), span)?;
        Ok(ProgramValueLiteral {
            kind: match literal.value {
                NumberValue::Integer(_) => "integer",
                NumberValue::Floating(_) => "floating",
            }
            .to_owned(),
            has_suffix: literal.has_suffix,
        })
    }

    fn cboxes_lvalue_address(&self, lvalue: &LValue, objects: &ObjectFrames) -> Option<u64> {
        let base = self.object_base_addresses.get(&lvalue.object).copied()?;
        let (_, start, _) = self.lvalue_byte_range(lvalue, &lvalue.ty, objects)?;
        base.checked_add(start as u64)
    }

    pub(super) fn trace_cboxes_main_block_close(
        &mut self,
        block: &Block,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let Some(function) = self.current_functions.last() else {
            return Ok(());
        };
        if function.name != "main" || function.body.span == block.span {
            return Ok(());
        }
        let close = block.span.end.saturating_sub(1).max(block.span.start);
        self.trace_cboxes_main_event_with_kind(
            Span::new(block.span.file, close, close.saturating_add(1)),
            frame,
            objects,
            "block-close",
        )
    }

    fn cboxes_frame_state(
        &self,
        frame: &Frame,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<Vec<ProgramStateBox>, Diagnostic> {
        let mut bindings = self.cboxes_visible_global_bindings(frame);
        bindings.extend(
            frame
                .bindings
                .iter()
                .filter(|(name, _)| {
                    !name.is_empty() && name.as_str() != "__func__" && !name.starts_with("__codex_")
                })
                .map(|(name, object)| (*object, name.as_str())),
        );
        bindings.sort_by_key(|(object, _)| object.0);

        let mut state = Vec::new();
        for (object_id, name) in bindings {
            let Some(object) = self.lookup_object(objects, object_id) else {
                continue;
            };
            if !object.alive {
                continue;
            }
            let base_address = self.object_base_addresses.get(&object_id).copied();
            self.cboxes_push_object_state(&mut state, name, object, base_address, span)?;
        }
        self.cboxes_add_aliases(&mut state);
        Ok(state)
    }

    fn cboxes_visible_global_bindings<'b>(&'b self, frame: &Frame) -> Vec<(ObjectId, &'b str)> {
        let Some(function) = self.current_functions.last() else {
            return Vec::new();
        };
        let Some(visible) = self
            .function_visible_global_declarations
            .get(&function.name)
        else {
            return Vec::new();
        };

        self.program
            .global_definitions
            .iter()
            .filter(|definition| {
                !definition.name.is_empty()
                    && !definition.name.starts_with("__codex_")
                    && !frame.bindings.contains_key(&definition.name)
                    && visible.get(&definition.name).is_some_and(|declaration| {
                        if definition.linkage == Some(Linkage::Internal) {
                            declaration.linkage == Some(Linkage::Internal)
                                && declaration.span.file == definition.span.file
                        } else {
                            declaration.linkage != Some(Linkage::Internal)
                        }
                    })
            })
            .filter_map(|definition| {
                let object = if definition.linkage == Some(Linkage::Internal) {
                    self.internal_global_bindings
                        .get(&definition.span.file)
                        .and_then(|bindings| bindings.get(&definition.name))
                        .copied()
                } else {
                    self.global_bindings.get(&definition.name).copied()
                }?;
                Some((object, definition.name.as_str()))
            })
            .collect()
    }

    fn cboxes_push_object_state(
        &self,
        state: &mut Vec<ProgramStateBox>,
        name: &str,
        object: &ObjectState,
        base_address: Option<u64>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let decoded_value;
        let value = if let StoredValue::ObjectRepresentation(bytes) = &object.value {
            decoded_value = self.deserialize_stored_value(&object.ty, bytes, span)?;
            &decoded_value
        } else {
            &object.value
        };
        self.cboxes_push_subobject_state(
            state,
            name,
            &object.ty,
            value,
            base_address,
            None,
            &[],
            span,
        )
    }

    fn cboxes_push_subobject_state(
        &self,
        state: &mut Vec<ProgramStateBox>,
        name: &str,
        ty: &CType,
        value: &StoredValue,
        address: Option<u64>,
        aggregate_root: Option<&str>,
        aggregate_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if let StoredValue::ObjectRepresentation(bytes) = value {
            let decoded = self.deserialize_stored_value(ty, bytes, span)?;
            return self.cboxes_push_subobject_state(
                state,
                name,
                ty,
                &decoded,
                address,
                aggregate_root,
                aggregate_path,
                span,
            );
        }

        if matches!(ty.unqualified(), CType::Array(_, _)) {
            let shape = cboxes_array_shape(ty);
            state.push(ProgramStateBox {
                name: name.to_owned(),
                ty: cboxes_type_string(ty),
                value: String::new(),
                display_value: String::new(),
                exact_value: String::new(),
                address,
                array_root: None,
                array_shape: shape.clone(),
                array_indices: Vec::new(),
                aggregate_root: aggregate_root.map(str::to_owned),
                aggregate_path: aggregate_path.to_vec(),
                aggregate_kind: None,
                aliases: Vec::new(),
                type_info: self.cboxes_type_info(ty),
            });
            if shape
                .iter()
                .try_fold(1usize, |count, length| count.checked_mul(*length))
                .is_none_or(|count| count > CBOXES_MAX_ARRAY_ELEMENTS)
            {
                return Ok(());
            }
            self.cboxes_push_array_elements(
                state,
                name,
                ty,
                value,
                address,
                &shape,
                &mut Vec::new(),
                aggregate_root,
                aggregate_path,
                span,
            )?;
            return Ok(());
        }

        if matches!(ty.unqualified(), CType::Struct(_, _) | CType::Union(_, _)) {
            return self.cboxes_push_aggregate_state(
                state,
                name,
                ty,
                value,
                address,
                aggregate_root,
                aggregate_path,
                span,
            );
        }

        let value = self.cboxes_stored_value_string(ty, value, span)?;
        let (display_value, exact_value) = self.cboxes_value_display_strings(&value, ty);
        state.push(ProgramStateBox {
            name: name.to_owned(),
            ty: cboxes_type_string(ty),
            value,
            display_value,
            exact_value,
            address,
            array_root: None,
            array_shape: Vec::new(),
            array_indices: Vec::new(),
            aggregate_root: aggregate_root.map(str::to_owned),
            aggregate_path: aggregate_path.to_vec(),
            aggregate_kind: None,
            aliases: Vec::new(),
            type_info: self.cboxes_type_info(ty),
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn cboxes_push_aggregate_state(
        &self,
        state: &mut Vec<ProgramStateBox>,
        name: &str,
        ty: &CType,
        value: &StoredValue,
        address: Option<u64>,
        aggregate_root: Option<&str>,
        aggregate_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let kind = match ty.unqualified() {
            CType::Struct(_, _) => "struct",
            CType::Union(_, _) => "union",
            _ => return Ok(()),
        };
        let unknown_union_member = matches!(
            value,
            StoredValue::Union {
                active_member: None,
                ..
            }
        );
        let status = if unknown_union_member {
            "active member unknown"
        } else {
            ""
        };
        state.push(ProgramStateBox {
            name: name.to_owned(),
            ty: cboxes_type_string(ty),
            value: status.to_owned(),
            display_value: status.to_owned(),
            exact_value: status.to_owned(),
            address,
            array_root: None,
            array_shape: Vec::new(),
            array_indices: Vec::new(),
            aggregate_root: aggregate_root.map(str::to_owned),
            aggregate_path: aggregate_path.to_vec(),
            aggregate_kind: Some(kind.to_owned()),
            aliases: Vec::new(),
            type_info: self.cboxes_type_info(ty),
        });

        let root_name = aggregate_root.unwrap_or(name);
        self.cboxes_push_aggregate_members(
            state,
            root_name,
            name,
            ty,
            value,
            address,
            aggregate_path,
            span,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn cboxes_push_aggregate_members(
        &self,
        state: &mut Vec<ProgramStateBox>,
        root_name: &str,
        parent_name: &str,
        ty: &CType,
        value: &StoredValue,
        address: Option<u64>,
        aggregate_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        match (ty.unqualified(), value) {
            (CType::Struct(_, _), StoredValue::Record(values)) => {
                let Some(record) = self.record_type(ty) else {
                    return Ok(());
                };
                for member in &record.members {
                    let Some((_, member_value)) = values
                        .iter()
                        .find(|(name, _)| name.as_ref() == member.storage_name)
                    else {
                        continue;
                    };
                    self.cboxes_push_aggregate_member(
                        state,
                        root_name,
                        parent_name,
                        ty,
                        member,
                        member_value,
                        address,
                        aggregate_path,
                        span,
                    )?;
                }
            }
            (
                CType::Union(_, _),
                StoredValue::Union {
                    active_member: Some(active),
                    ..
                },
            ) => {
                let Some(member) = self.direct_member_by_storage_name(ty, active) else {
                    return Ok(());
                };
                let member_value = self.extract_union_member(value, member, span)?;
                self.cboxes_push_aggregate_member(
                    state,
                    root_name,
                    parent_name,
                    ty,
                    member,
                    &member_value,
                    address,
                    aggregate_path,
                    span,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn cboxes_push_aggregate_member(
        &self,
        state: &mut Vec<ProgramStateBox>,
        root_name: &str,
        parent_name: &str,
        parent_ty: &CType,
        member: &RecordMember,
        member_value: &StoredValue,
        address: Option<u64>,
        aggregate_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if member.bit_width == Some(0) || (member.name.is_none() && member.bit_width.is_some()) {
            return Ok(());
        }
        let member_ty = self.qualified_member_type(parent_ty, member);
        let member_address = address.and_then(|base| base.checked_add(member.offset as u64));
        let Some(member_name) = &member.name else {
            if matches!(
                member_ty.unqualified(),
                CType::Struct(_, _) | CType::Union(_, _)
            ) {
                self.cboxes_push_aggregate_members(
                    state,
                    root_name,
                    parent_name,
                    &member_ty,
                    member_value,
                    member_address,
                    aggregate_path,
                    span,
                )?;
            }
            return Ok(());
        };
        let name = format!("{parent_name}.{member_name}");
        let mut path = aggregate_path.to_vec();
        path.push(member_name.clone());
        self.cboxes_push_subobject_state(
            state,
            &name,
            &member_ty,
            member_value,
            member_address,
            Some(root_name),
            &path,
            span,
        )
    }

    fn cboxes_push_array_elements(
        &self,
        state: &mut Vec<ProgramStateBox>,
        root_name: &str,
        ty: &CType,
        value: &StoredValue,
        address: Option<u64>,
        shape: &[usize],
        indices: &mut Vec<usize>,
        aggregate_root: Option<&str>,
        aggregate_path: &[String],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let CType::Array(inner, _) = ty.unqualified() else {
            let value = self.cboxes_stored_value_string(ty, value, span)?;
            let (display_value, exact_value) = self.cboxes_value_display_strings(&value, ty);
            state.push(ProgramStateBox {
                name: cboxes_array_element_name(root_name, indices),
                ty: cboxes_type_string(ty),
                value,
                display_value,
                exact_value,
                address,
                array_root: Some(root_name.to_owned()),
                array_shape: shape.to_vec(),
                array_indices: indices.clone(),
                aggregate_root: aggregate_root.map(str::to_owned),
                aggregate_path: aggregate_path.to_vec(),
                aggregate_kind: None,
                aliases: Vec::new(),
                type_info: self.cboxes_type_info(ty),
            });
            return Ok(());
        };
        let element_size = self.type_size_of(inner).unwrap_or(0) as u64;
        let elements = match value {
            StoredValue::Array(elements) => elements.as_slice(),
            _ => &[],
        };
        for (index, element) in elements.iter().enumerate() {
            indices.push(index);
            let element_address = address
                .and_then(|base| base.checked_add((index as u64).saturating_mul(element_size)));
            self.cboxes_push_array_elements(
                state,
                root_name,
                inner,
                element,
                element_address,
                shape,
                indices,
                aggregate_root,
                aggregate_path,
                span,
            )?;
            indices.pop();
        }
        Ok(())
    }

    pub(super) fn cboxes_type_info(&self, ty: &CType) -> ProgramTypeInfo {
        ProgramTypeInfo {
            kind: cboxes_type_kind(ty).to_owned(),
            help: cboxes_type_help(ty),
            help_type_names: cboxes_type_help_type_names(ty),
            help_tree: cboxes_type_help_tree(ty),
            pointer_depth: cboxes_pointer_depth(ty),
            array_shape: cboxes_array_shape(ty),
            pointee_array_shape: cboxes_pointee_array_shape(ty),
            size: self.type_size_of(ty),
            align: self.type_align_of(ty),
        }
    }

    fn cboxes_value_display_strings(&self, value: &str, ty: &CType) -> (String, String) {
        if !matches!(
            ty.unqualified(),
            CType::Float | CType::Double | CType::LongDouble
        ) {
            return (value.to_owned(), value.to_owned());
        }
        let Ok(value) = value.parse::<f64>() else {
            return (value.to_owned(), value.to_owned());
        };
        let key = value.to_bits();
        if let Some(cached) = self.float_display_cache.borrow().get(&key) {
            return cached.clone();
        }
        let rendered = (
            cboxes_float_default_string(value),
            cboxes_float_exact_string(value),
        );
        let mut cache = self.float_display_cache.borrow_mut();
        if cache.len() < CBOXES_FLOAT_DISPLAY_CACHE_LIMIT {
            cache.insert(key, rendered.clone());
        }
        rendered
    }

    fn cboxes_add_aliases(&self, state: &mut [ProgramStateBox]) {
        let pointers = state
            .iter()
            .filter(|item| item.array_root.is_none() && item.type_info.pointer_depth > 0)
            .map(|item| {
                (
                    item.name.clone(),
                    item.value.clone(),
                    item.type_info.pointer_depth,
                )
            })
            .collect::<Vec<_>>();
        for (name, mut address, depth) in pointers {
            for level in 1..=depth {
                let Some(index) = state.iter().position(|item| {
                    item.address
                        .is_some_and(|candidate| candidate.to_string() == address)
                }) else {
                    break;
                };
                let alias = format!("{}{}", "*".repeat(level), name);
                if !state[index].aliases.contains(&alias) {
                    state[index].aliases.push(alias);
                }
                address = state[index].value.clone();
                if address.is_empty() {
                    break;
                }
            }
        }
        for item in state {
            item.aliases.sort_by(|left, right| {
                left.chars()
                    .take_while(|ch| *ch == '*')
                    .count()
                    .cmp(&right.chars().take_while(|ch| *ch == '*').count())
                    .then_with(|| left.cmp(right))
            });
        }
    }

    fn cboxes_stored_value_string(
        &self,
        ty: &CType,
        value: &StoredValue,
        span: Span,
    ) -> Result<String, Diagnostic> {
        match value {
            StoredValue::Scalar(value) if value.indeterminate => Ok(String::new()),
            StoredValue::Scalar(value) => self.cboxes_typed_value_string(value, ty, span),
            StoredValue::Indeterminate => Ok(String::new()),
            StoredValue::ObjectRepresentation(bytes) => {
                let decoded = self.deserialize_stored_value(ty, bytes, span)?;
                if matches!(
                    decoded,
                    StoredValue::Scalar(TypedValue {
                        data: ValueData::ObjectRepresentation(_),
                        ..
                    })
                ) {
                    return Ok(String::new());
                }
                self.cboxes_stored_value_string(ty, &decoded, span)
            }
            StoredValue::Array(elements) => {
                let CType::Array(element_ty, _) = ty.unqualified() else {
                    return Ok(String::new());
                };
                let values = elements
                    .iter()
                    .map(|element| {
                        self.cboxes_aggregate_component_string(element_ty, element, span)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("{{ {} }}", values.join(", ")))
            }
            StoredValue::Record(values) => {
                let Some(record) = self.record_type(ty) else {
                    return Ok(String::new());
                };
                let mut members = Vec::new();
                for member in &record.members {
                    if member.bit_width == Some(0)
                        || (member.name.is_none() && member.bit_width.is_some())
                    {
                        continue;
                    }
                    let Some((_, value)) = values
                        .iter()
                        .find(|(name, _)| name.as_ref() == member.storage_name)
                    else {
                        continue;
                    };
                    let value = self.cboxes_aggregate_component_string(&member.ty, value, span)?;
                    if let Some(name) = &member.name {
                        members.push(format!(".{name} = {value}"));
                    } else {
                        members.push(value);
                    }
                }
                Ok(format!("{{ {} }}", members.join(", ")))
            }
            StoredValue::Union { active_member, .. } => {
                let Some(active_member) = active_member else {
                    return Ok("{ active member unknown }".to_owned());
                };
                let Some(member) = self.direct_member_by_storage_name(ty, active_member) else {
                    return Ok("{ active member unknown }".to_owned());
                };
                let value = self.extract_union_member(value, member, span)?;
                let value = self.cboxes_aggregate_component_string(&member.ty, &value, span)?;
                Ok(match &member.name {
                    Some(name) => format!("{{ .{name} = {value} }}"),
                    None => format!("{{ {value} }}"),
                })
            }
        }
    }

    fn cboxes_aggregate_component_string(
        &self,
        ty: &CType,
        value: &StoredValue,
        span: Span,
    ) -> Result<String, Diagnostic> {
        let value = self.cboxes_stored_value_string(ty, value, span)?;
        if value.is_empty() {
            Ok("?".to_owned())
        } else {
            Ok(self.cboxes_value_display_strings(&value, ty).0)
        }
    }

    fn cboxes_typed_value_string(
        &self,
        value: &TypedValue,
        ty: &CType,
        span: Span,
    ) -> Result<String, Diagnostic> {
        match &value.data {
            ValueData::Void => Ok(String::new()),
            ValueData::Int(value) => Ok(value.get().to_string()),
            ValueData::Float(value) => Ok(cboxes_float_string(*value)),
            ValueData::Aggregate(value) => self.cboxes_stored_value_string(ty, value, span),
            ValueData::ObjectRepresentation(_) => Ok(String::new()),
            ValueData::Complex(_) => Ok(String::new()),
            ValueData::Function(_) => Ok(String::new()),
            ValueData::Pointer(pointer) => {
                if pointer.is_null() {
                    return Ok("0".to_owned());
                }
                let pointer_ty = if matches!(ty.unqualified(), CType::Pointer(_)) {
                    ty.clone()
                } else {
                    value.ty.clone()
                };
                match self.pointer_numeric_address(value, &pointer_ty, span) {
                    Ok(address) => Ok(address.to_string()),
                    Err(_) => Ok(String::new()),
                }
            }
        }
    }
}
