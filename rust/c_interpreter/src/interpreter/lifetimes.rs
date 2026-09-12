//! Object allocation, scope retirement, and setjmp/longjmp environments.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn allocate_object_id(&mut self) -> ObjectId {
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
                .unwrap_or_else(|| self.allocate_object_id())
        } else {
            self.allocate_object_id()
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

    pub(super) fn lookup_active_object<'b>(
        &self,
        objects: &'b ObjectFrames,
        object_id: ObjectId,
    ) -> Option<&'b ObjectState> {
        objects.get(object_id)
    }

    pub(super) fn lookup_active_object_mut<'b>(
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
            if !snapshot_ids.contains(&id)
                && let Some(mut object) = objects.remove_from_last(id)
            {
                self.end_live_non_dynamic_bytes(&object);
                object.alive = false;
                object.value = StoredValue::Indeterminate;
                self.retired_objects.insert(id, object);
                retired_now.push(id);
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
}
