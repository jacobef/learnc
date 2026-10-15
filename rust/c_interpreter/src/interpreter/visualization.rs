//! Browser state snapshots, traces, and expression inspection.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn stdin_position(&self) -> usize {
        self.host_capture_streams
            .get(&CaptureStream::Stdin)
            .and_then(|id| self.host_streams.get(id))
            .map_or(0, |stream| {
                stream.position.saturating_sub(stream.pushback.len())
            })
    }

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

    pub(super) fn trace_cboxes_loop_event(
        &mut self,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.trace_cboxes_main_event_with_kind(span, frame, objects, "loop")
    }

    pub(super) fn trace_cboxes_main_event(
        &mut self,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.trace_cboxes_main_event_with_details(span, frame, objects, "statement", None)
    }

    pub(super) fn trace_cboxes_main_event_with_kind(
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
        if function.name != "main" && !self.run_options.trace_function_calls {
            return Ok(());
        }
        let function_name = function.name.clone();
        // A braced for-loop's update executes at the closing brace in the
        // stepper, including calls made by the update expression.
        let display_span = self
            .active_block_scopes
            .iter()
            .rev()
            .find_map(|scope| {
                let (step, body) = scope.for_step_trace?;
                (span.file == step.file && span.start >= step.start && span.end <= step.end)
                    .then(|| Span::new(body.file, body.end.saturating_sub(1), body.end))
            })
            .unwrap_or(span);
        let (path, start_line, end_line) = self.sources.span_location(display_span);
        let trace_index = self.cboxes_trace.len();
        self.evaluate_cboxes_expression_at(trace_index, frame, objects)?;
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
            call_depth: self.current_frame_ids.len().saturating_sub(1),
            frame_id: frame.id,
            function: function_name,
            stdout_length: self.capture_stream_bytes(CaptureStream::Stdout).len(),
            stderr_length: self.capture_stream_bytes(CaptureStream::Stderr).len(),
            stdin_position: self.stdin_position(),
            file: path.display().to_string(),
            start_line: start_line.saturating_sub(1),
            end_line: end_line.saturating_sub(1),
            kind: kind.to_owned(),
            state: self.cboxes_frame_state(frame, objects, span)?,
            skipped_range,
        });
        if self
            .run_options
            .execution_trace_limit
            .is_some_and(|limit| self.cboxes_trace.len() >= limit)
        {
            return Err(Diagnostic::execution_trace_limit(span));
        }
        Ok(())
    }

    pub(super) fn evaluate_cboxes_expression_at(
        &mut self,
        event_index: usize,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if self.cboxes_expression_result.is_none()
            && self
                .cboxes_expression_request
                .as_ref()
                .is_some_and(|request| request.event_index == event_index)
        {
            let request = self
                .cboxes_expression_request
                .as_ref()
                .expect("expression request was checked")
                .clone();
            self.cboxes_expression_result =
                Some(self.evaluate_cboxes_expression(&request, frame, objects)?);
            return Err(Diagnostic::expression_complete(request.expr.span()));
        }
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
        let saved_next_heap_pointer = self.next_heap_pointer;
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
                    } else if self.dynamic_allocations.contains(&lvalue.object) {
                        // Allocated memory is untyped bytes, so read it the way the
                        // program would. An element without a value yet shows as empty.
                        match self.load_lvalue(lvalue.clone(), expr.span(), &eval_objects) {
                            Ok(loaded) if !loaded.indeterminate => {
                                self.cboxes_typed_value_string(&loaded, &ty, expr.span())?
                            }
                            Ok(_) => String::new(),
                            Err(_) => String::new(),
                        }
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
        self.next_heap_pointer = saved_next_heap_pointer;
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
        // Byte-backed objects such as string literals must be decoded before
        // their elements can be addressed, as in cboxes_push_object_state.
        let decoded_value;
        let object_value = if let StoredValue::ObjectRepresentation(bytes) = &object.value {
            decoded_value = self.deserialize_stored_value(&object.ty, bytes, span)?;
            &decoded_value
        } else {
            &object.value
        };
        let (base_stored, base_ty, tail_offset) =
            self.resolve_lvalue_array_base(object_value, &object.ty, lvalue, span)?;
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
        flow: &Flow,
    ) -> Result<(), Diagnostic> {
        let Some(function) = self.current_functions.last() else {
            return Ok(());
        };
        if (!self.run_options.trace_function_calls && function.name != "main")
            || function.body.span == block.span
        {
            return Ok(());
        }
        // A block left by a jump (break, continue, goto, or return) never reaches
        // its closing brace.
        if !matches!(flow, Flow::Continue) {
            return Ok(());
        }
        if matches!(flow, Flow::Continue | Flow::LoopContinue)
            && self.active_block_scopes.iter().any(|scope| {
                scope.frame_id == frame.id
                    && scope
                        .for_step_trace
                        .is_some_and(|(_, body)| body == block.span)
            })
        {
            // The update snapshot also records the body locals leaving scope.
            return Ok(());
        }
        self.trace_cboxes_main_scope_close(block.span, frame, objects)
    }

    pub(super) fn trace_cboxes_main_scope_close(
        &mut self,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let close = span.end.saturating_sub(1).max(span.start);
        self.trace_cboxes_main_event_with_kind(
            Span::new(span.file, close, close.saturating_add(1)),
            frame,
            objects,
            "block-close",
        )
    }

    pub(super) fn cboxes_frame_state(
        &self,
        frame: &Frame,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<Vec<ProgramStateBox>, Diagnostic> {
        let mut visible: HashMap<ObjectId, &str> = self
            .cboxes_visible_global_bindings(frame)
            .into_iter()
            .collect();
        visible.extend(
            frame
                .bindings
                .iter()
                .map(|(name, object)| (*object, name.as_str())),
        );
        let mut bindings: BTreeMap<usize, (String, String)> = BTreeMap::new();
        for definition in &self.program.global_definitions {
            if let Some(object) = self.lookup_global_binding(&definition.name, definition.span.file)
            {
                bindings.insert(object.0, (definition.name.clone(), "global".to_owned()));
            }
        }
        // Walk current storage frames, not the historical object-ID slots left
        // behind by earlier calls and loop iterations.
        for id in objects.frames.iter().flatten() {
            if let Some((name, scope)) = objects
                .get(*id)
                .and_then(|object| object.display_binding.as_ref())
            {
                bindings.insert(id.0, (name.clone(), scope.clone()));
            }
        }
        let mut state = Vec::new();
        self.cboxes_push_named_objects(&mut state, bindings, &visible, objects, span)?;
        self.cboxes_push_string_literals(&mut state, objects, span)?;
        self.cboxes_push_main_arguments(&mut state, objects, span)?;
        self.cboxes_push_dynamic_allocations(&mut state, objects, span)?;
        self.cboxes_add_aliases(&mut state);
        Ok(state)
    }

    /// main's argv array and the argument strings it points to have no names either.
    fn cboxes_push_main_arguments(
        &self,
        state: &mut Vec<ProgramStateBox>,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        for (index, &object_id) in self.cboxes_argument_objects.iter().enumerate() {
            let Some(object) = self
                .lookup_object(objects, object_id)
                .filter(|object| object.alive)
            else {
                continue;
            };
            let start = state.len();
            let base_address = self.object_base_addresses.get(&object_id).copied();
            self.cboxes_push_object_state(state, "", object, base_address, span)?;
            let kind = if index == 0 {
                "command-line arguments"
            } else {
                "command-line argument"
            };
            for item in &mut state[start..] {
                item.object_id = object_id.0;
                item.anonymous = Some(kind.to_owned());
            }
        }
        Ok(())
    }

    /// Memory from `malloc` and friends has no name and no declared type, so each
    /// block is shown as the type it's being used as: the type of a pointer that
    /// points into it, or else the type of the values stored in it. A block nothing
    /// has given a type to yet is shown as just its size.
    fn cboxes_push_dynamic_allocations(
        &self,
        state: &mut Vec<ProgramStateBox>,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut blocks = self
            .dynamic_allocations
            .iter()
            .copied()
            .filter(|id| objects.get(*id).is_some_and(|object| object.alive))
            .collect::<Vec<_>>();
        if blocks.is_empty() {
            return Ok(());
        }
        blocks.sort_unstable_by_key(|id| id.0);
        let mut pointee_hints: HashMap<ObjectId, CType> = HashMap::default();
        let mut sources = objects.frames.iter().flatten().copied().collect::<Vec<_>>();
        for definition in &self.program.global_definitions {
            if let Some(id) = self.lookup_global_binding(&definition.name, definition.span.file) {
                sources.push(id);
            }
        }
        for id in sources {
            if let Some(object) = objects.get(id).filter(|object| object.alive) {
                self.cboxes_collect_heap_pointer_hints(&object.value, &mut pointee_hints);
            }
        }
        // Blocks can point to each other, as in a linked list, so a block's type can
        // depend on another block's contents.
        let mut views: HashMap<ObjectId, Option<(CType, StoredValue)>> = HashMap::default();
        for _ in 0..=blocks.len() {
            let mut changed = false;
            for &id in &blocks {
                if views.get(&id).is_some_and(Option::is_some) {
                    continue;
                }
                let object = objects.get(id).expect("live block");
                let view = self.cboxes_dynamic_view_type(object, pointee_hints.get(&id));
                let view = match view {
                    Some(ty) => {
                        let value = match &object.value {
                            StoredValue::ObjectRepresentation(bytes) => {
                                self.deserialize_stored_value(&ty, bytes, span)?
                            }
                            StoredValue::Indeterminate => self.deserialize_stored_value(
                                &ty,
                                &vec![ByteCell::Indeterminate; object.byte_size],
                                span,
                            )?,
                            other => other.clone(),
                        };
                        self.cboxes_collect_heap_pointer_hints(&value, &mut pointee_hints);
                        changed = true;
                        Some((ty, value))
                    }
                    None => None,
                };
                views.insert(id, view);
            }
            if !changed {
                break;
            }
        }
        for id in blocks {
            let object = objects.get(id).expect("live block");
            let base_address = self.object_base_addresses.get(&id).copied();
            let start = state.len();
            match views.remove(&id).flatten() {
                Some((ty, value)) => {
                    self.cboxes_push_subobject_state(
                        state,
                        "",
                        &ty,
                        &value,
                        base_address,
                        None,
                        &[],
                        span,
                    )?;
                }
                None => state.push(ProgramStateBox {
                    object_id: 0,
                    name_accessible: true,
                    scope: String::new(),
                    name: String::new(),
                    ty: format!(
                        "{} byte{}, no type yet",
                        object.byte_size,
                        if object.byte_size == 1 { "" } else { "s" }
                    ),
                    value: String::new(),
                    display_value: String::new(),
                    exact_value: String::new(),
                    address: base_address,
                    array_root: None,
                    array_shape: Vec::new(),
                    array_indices: Vec::new(),
                    aggregate_root: None,
                    aggregate_path: Vec::new(),
                    aggregate_kind: None,
                    anonymous: None,
                    aliases: Vec::new(),
                    type_info: self.cboxes_type_info(&CType::Void),
                }),
            }
            for item in &mut state[start..] {
                item.object_id = id.0;
                item.anonymous = Some("allocated block".to_owned());
            }
        }
        Ok(())
    }

    /// The type a block of allocated memory is being used as, if any: a whole number
    /// of whatever a pointer into it points to, or of the type stored in it.
    fn cboxes_dynamic_view_type(
        &self,
        object: &ObjectState,
        hint: Option<&CType>,
    ) -> Option<CType> {
        let size = object.byte_size;
        let fits = |ty: &CType| {
            self.type_size_of(ty)
                .filter(|element| *element > 0 && size % *element == 0)
                .map(|element| (ty.clone(), size / element))
        };
        let from_pointer = hint.and_then(|ty| fits(ty.unqualified()));
        let from_stores = || {
            let mut types = object.effective_types.iter().map(|region| {
                region
                    .coalesced_element_type
                    .clone()
                    .unwrap_or_else(|| region.ty.clone())
            });
            let first = types.next()?;
            let mut element = first.unqualified().clone();
            while let CType::Array(inner, _) = element {
                element = inner.unqualified().clone();
            }
            let uniform = object.effective_types.iter().all(|region| {
                let mut ty = region
                    .coalesced_element_type
                    .clone()
                    .unwrap_or_else(|| region.ty.clone())
                    .unqualified()
                    .clone();
                while let CType::Array(inner, _) = ty {
                    ty = inner.unqualified().clone();
                }
                ty == element
            });
            if uniform { fits(&element) } else { None }
        };
        let (element, count) = from_pointer.or_else(from_stores)?;
        Some(if count == 1 {
            element
        } else {
            CType::array_of(element, count)
        })
    }

    fn cboxes_collect_heap_pointer_hints(
        &self,
        value: &StoredValue,
        hints: &mut HashMap<ObjectId, CType>,
    ) {
        match value {
            StoredValue::Scalar(typed) => {
                if let (ValueData::Pointer(pointer), CType::Pointer(pointee)) =
                    (&typed.data, typed.ty.unqualified())
                {
                    if let Some(object) = pointer
                        .object
                        .filter(|id| self.dynamic_allocations.contains(id))
                    {
                        if !matches!(pointee.unqualified(), CType::Void) {
                            hints
                                .entry(object)
                                .or_insert_with(|| pointee.as_ref().clone());
                        }
                    }
                }
            }
            StoredValue::Array(items) => {
                for item in items {
                    self.cboxes_collect_heap_pointer_hints(item, hints);
                }
            }
            StoredValue::Record(members) | StoredValue::Union { members, .. } => {
                for (_, member) in members {
                    self.cboxes_collect_heap_pointer_hints(member, hints);
                }
            }
            StoredValue::ObjectRepresentation(_) | StoredValue::Indeterminate => {}
        }
    }

    fn cboxes_push_named_objects(
        &self,
        state: &mut Vec<ProgramStateBox>,
        bindings: BTreeMap<usize, (String, String)>,
        visible: &HashMap<ObjectId, &str>,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        for (id, (name, scope)) in bindings {
            if name.is_empty() || name == "__func__" || name.starts_with("__cboxes_") {
                continue;
            }
            let object_id = ObjectId(id);
            let Some(object) = self
                .lookup_object(objects, object_id)
                .filter(|object| object.alive)
            else {
                continue;
            };
            let start = state.len();
            let base_address = self.object_base_addresses.get(&object_id).copied();
            self.cboxes_push_object_state(state, &name, object, base_address, span)?;
            for item in &mut state[start..] {
                item.object_id = id;
                item.name_accessible = visible
                    .get(&object_id)
                    .is_some_and(|visible_name| **visible_name == name);
                item.scope = scope.clone();
            }
        }
        Ok(())
    }

    /// String literal arrays have no name, so they are listed by object, in
    /// allocation order. Literal objects exist before main starts.
    fn cboxes_push_string_literals(
        &self,
        state: &mut Vec<ProgramStateBox>,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut literals = self
            .literal_bindings
            .iter()
            .flat_map(|bindings| bindings.values().copied())
            .collect::<Vec<_>>();
        literals.sort_unstable_by_key(|id| id.0);
        for object_id in literals {
            let Some(object) = self
                .lookup_object(objects, object_id)
                .filter(|object| object.alive)
            else {
                continue;
            };
            let start = state.len();
            let base_address = self.object_base_addresses.get(&object_id).copied();
            self.cboxes_push_object_state(state, "", object, base_address, span)?;
            for item in &mut state[start..] {
                item.object_id = object_id.0;
                item.anonymous = Some("string literal".to_owned());
            }
        }
        Ok(())
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
                    && !definition.name.starts_with("__cboxes_")
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
                object_id: 0,
                name_accessible: true,
                scope: String::new(),
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
                anonymous: None,
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
            object_id: 0,
            name_accessible: true,
            scope: String::new(),
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
            anonymous: None,
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
            object_id: 0,
            name_accessible: true,
            scope: String::new(),
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
            anonymous: None,
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
                object_id: 0,
                name_accessible: true,
                scope: String::new(),
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
                anonymous: None,
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

    pub(super) fn cboxes_type_info(&self, ty: &CType) -> Arc<ProgramTypeInfo> {
        if let Some(info) = self.type_info_cache.borrow().get(ty) {
            return info.clone();
        }
        let info = Arc::new(ProgramTypeInfo {
            kind: cboxes_type_kind(ty).to_owned(),
            help: cboxes_type_help(ty),
            help_type_names: cboxes_type_help_type_names(ty),
            help_tree: cboxes_type_help_tree(ty),
            pointer_depth: cboxes_pointer_depth(ty),
            array_shape: cboxes_array_shape(ty),
            pointee_array_shape: cboxes_pointee_array_shape(ty),
            size: self.type_size_of(ty),
            align: self.type_align_of(ty),
        });
        let mut cache = self.type_info_cache.borrow_mut();
        if cache.len() < 4096 {
            cache.insert(ty.clone(), info.clone());
        }
        info
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
        let mut addresses = HashMap::default();
        for (index, item) in state.iter().enumerate() {
            if let Some(address) = item.address {
                addresses.entry(address).or_insert(index);
            }
        }
        let pointers = state
            .iter()
            .filter(|item| {
                item.name_accessible
                    && item.array_root.is_none()
                    && item.type_info.pointer_depth > 0
            })
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
                let Some(&index) = address
                    .parse::<u64>()
                    .ok()
                    .and_then(|key| addresses.get(&key))
                else {
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
