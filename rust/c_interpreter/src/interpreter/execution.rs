use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn call_function(
        &mut self,
        function: &Rc<FunctionDef>,
        function_may_setjmp: bool,
        mut args: Vec<TypedValue>,
        call_span: Span,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if self.current_frame_ids.len() >= MAX_FUNCTION_CALL_DEPTH {
            return Err(Diagnostic::error(
                format!(
                    "function call depth exceeded the interpreter limit of {MAX_FUNCTION_CALL_DEPTH}; check for recursion that does not reach its base case"
                ),
                call_span,
            ));
        }
        let fixed_param_count =
            if function.params.len() == 1 && function.params[0].ty == CType::Void {
                0
            } else {
                function.params.len()
            };
        if function.params.len() == 1
            && function.params[0].ty == CType::Void
            && !function.is_variadic
        {
            if !args.is_empty() {
                return Err(Diagnostic::error(
                    format!("function {} takes no arguments", function.name),
                    call_span,
                ));
            }
        } else if function.params.is_empty() && !args.is_empty() {
            return Err(Diagnostic::ub(
                format!(
                    "call supplies {} argument(s), but the definition of {} has no parameters",
                    args.len(),
                    function.name
                ),
                call_span,
                Some("6.5.2.2p6"),
            ));
        } else if (!function.is_variadic && fixed_param_count != args.len())
            || (function.is_variadic && args.len() < fixed_param_count)
        {
            return Err(Diagnostic::error(
                format!(
                    "function {} expected {}{}argument(s), got {}",
                    function.name,
                    fixed_param_count,
                    if function.is_variadic { "+" } else { " " },
                    args.len()
                ),
                call_span,
            ));
        }
        let variadic_args = if function.is_variadic {
            args.split_off(fixed_param_count)
        } else {
            Vec::new()
        };

        let frame_id = self.next_frame_id;
        self.next_frame_id += 1;
        let mut frame = Frame {
            id: frame_id,
            bindings: HashMap::default(),
            object_decls: HashMap::default(),
            function_decls: HashMap::default(),
        };
        objects.push_frame();
        self.restrict_trackers.push(RestrictTracker::default());
        self.current_variadic_args.push(variadic_args);
        self.current_functions.push(function.clone());

        self.current_frame_ids.push(frame_id);

        let func_key = Rc::as_ptr(function) as usize;
        let func_object = if let Some(object) = self.func_name_bindings.get(&func_key).copied() {
            object
        } else {
            let pointer = self.intern_readonly_c_bytes(function.name.as_bytes());
            let object = pointer.object.expect("__func__ has static storage");
            let func_ty = CType::array_of(
                CType::qualified(
                    CType::Char,
                    crate::types::TypeQualifiers {
                        is_const: true,
                        ..crate::types::TypeQualifiers::default()
                    },
                ),
                function.name.len() + 1,
            );
            if let Some(state) = self.retired_objects.get_mut(&object) {
                state.ty = func_ty.clone();
                state.const_object = true;
            }
            self.object_type_registry.insert(object, func_ty);
            self.func_name_bindings.insert(func_key, object);
            object
        };
        frame.bindings.insert("__func__".to_owned(), func_object);

        let result = {
            let _cleanup = CallFrameCleanup::new(self, objects, frame_id);
            let execution = (|| -> Result<TypedValue, Diagnostic> {
                for (param, arg) in function.params.iter().zip(args) {
                    if param.ty == CType::Void {
                        continue;
                    }
                    for bound in param.vla_bounds.iter().flatten() {
                        let _ = self.evaluate_vla_bound(bound, &mut frame, objects)?;
                    }
                    self.check_static_array_parameter(param, &arg, &mut frame, objects)?;
                    let arg = self.convert_value(arg, &param.ty, call_span)?;
                    let object = self.allocate_object(
                        objects,
                        param.ty.clone(),
                        StorageDuration::Automatic,
                        param.span,
                        param.storage_class == Some(StorageClass::Register),
                        param.vla_bounds.iter().any(|bound| bound.is_some()),
                    )?;
                    self.initialize_object_value(objects, object, arg, param.span)?;
                    frame
                        .bindings
                        .insert(param.name.clone().unwrap_or_default(), object);
                }

                let needs_longjmp_catch =
                    function_may_setjmp || !self.live_setjmp_frames.is_empty();
                let flow = if needs_longjmp_catch {
                    let mut execution = catch_unwind(AssertUnwindSafe(|| {
                        self.exec_block(&function.body, &mut frame, objects)
                    }));
                    loop {
                        match execution {
                            Ok(flow) => break flow?,
                            Err(payload) => {
                                let Some(signal) = payload.downcast_ref::<LongjmpSignal>().copied()
                                else {
                                    resume_unwind(payload);
                                };
                                if signal.target_frame_id != frame_id {
                                    resume_unwind(payload);
                                }
                                let env = self
                                    .setjmp_envs
                                    .get(&signal.handle)
                                    .cloned()
                                    .ok_or_else(|| {
                                        Diagnostic::ub(
                                            "longjmp was called with an invalid or uninitialized jmp_buf",
                                            call_span,
                                            Some("7.13.2.1"),
                                        )
                                    })?;
                                self.restore_setjmp_environment(
                                    &env, &mut frame, objects, call_span,
                                )?;
                                self.pending_longjmp_return = Some(PendingLongjmpReturn {
                                    frame_id,
                                    value: signal.value,
                                });
                                execution = catch_unwind(AssertUnwindSafe(|| {
                                    self.resume_function_after_longjmp(
                                        function, env.site, &mut frame, objects,
                                    )
                                }));
                            }
                        }
                    }
                } else {
                    self.exec_block(&function.body, &mut frame, objects)?
                };

                if function.is_noreturn && matches!(flow, Flow::Continue | Flow::Return(_)) {
                    return Err(Diagnostic::ub(
                        format!(
                            "_Noreturn function {} returned to its caller",
                            function.name
                        ),
                        function.span,
                        Some("6.7.4p8"),
                    ));
                }
                match flow {
                        Flow::Continue if function.name == "main" => {
                            let value = self.convert_value(
                                TypedValue::int(0),
                                &function.return_type,
                                function.span,
                            )?;
                            Ok(self.specialize_dynamic_raw_storage_pointer(
                                value,
                                &function.return_type,
                                objects,
                            ))
                        }
                        Flow::Continue if function.return_type == CType::Void => {
                            Ok(TypedValue::void())
                        }
                        Flow::Continue
                            if self.run_options.ub_detection_mode == UbDetectionMode::Simple =>
                        {
                            Err(self.simple_ub_diagnostic(
                                format!(
                                    "control reached the end of non-void function {}",
                                    function.name
                                ),
                                function.body.span,
                                "ISO C only makes this undefined when the caller uses the missing return value; simple mode rejects the fallthrough itself",
                            ))
                        }
                        Flow::Continue => {
                            Ok(TypedValue::missing_return(function.return_type.clone()))
                        }
                        Flow::Return(value) => {
                            self.reject_missing_return_value(&value, function.span)?;
                            let value =
                                self.convert_value(value, &function.return_type, function.span)?;
                            Ok(self.specialize_dynamic_raw_storage_pointer(
                                value,
                                &function.return_type,
                                objects,
                            ))
                        }
                        Flow::Goto(label, span) => Err(Diagnostic::error(
                            format!("use of undeclared label {}", label),
                            span,
                        )),
                        Flow::LoopBreak => Err(Diagnostic::error(
                            "break statement is not within a loop",
                            function.span,
                        )),
                        Flow::LoopContinue => Err(Diagnostic::error(
                            "continue statement is not within a loop",
                            function.span,
                        )),
                }
            })();
            execution.and_then(|value| {
                self.reject_unended_va_lists(frame_id, function.body.span)?;
                Ok(value)
            })
        };
        result.and_then(|mut value| {
            self.take_pending_stream_buffer_lifetime_ub()?;
            if value.ty.is_pointer() && self.pointer_value_has_ended_lifetime(&value, objects) {
                value.indeterminate = true;
            }
            if let ValueData::Aggregate(stored) = &mut value.data {
                self.mark_ended_pointer_subobjects_indeterminate(stored, &value.ty, objects);
                self.refresh_all_union_bytes(stored, &value.ty, call_span)?;
                value.indeterminate = !self.stored_value_is_determinate(stored);
            }
            Ok(value)
        })
    }

    fn mark_ended_pointer_subobjects_indeterminate(
        &self,
        stored: &mut StoredValue,
        ty: &CType,
        objects: &ObjectFrames,
    ) {
        match (stored, ty.unqualified()) {
            (StoredValue::Scalar(value), CType::Pointer(_)) => {
                if self.pointer_value_has_ended_lifetime(value, objects) {
                    value.indeterminate = true;
                }
            }
            (StoredValue::Array(values), CType::Array(inner, _)) => {
                for value in values {
                    self.mark_ended_pointer_subobjects_indeterminate(value, inner, objects);
                }
            }
            (StoredValue::Record(values), CType::Struct(_, _)) => {
                let Some(record) = self.record_type(ty) else {
                    return;
                };
                for member in &record.members {
                    if let Some((_, value)) = values
                        .iter_mut()
                        .find(|(name, _)| name.as_ref() == member.storage_name)
                    {
                        self.mark_ended_pointer_subobjects_indeterminate(
                            value, &member.ty, objects,
                        );
                    }
                }
            }
            (StoredValue::Union { members, .. }, CType::Union(_, _)) => {
                let Some(record) = self.record_type(ty) else {
                    return;
                };
                for member in &record.members {
                    if let Some((_, value)) = members
                        .iter_mut()
                        .find(|(name, _)| name.as_ref() == member.storage_name)
                    {
                        self.mark_ended_pointer_subobjects_indeterminate(
                            value, &member.ty, objects,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn resolve_decl_type(
        &mut self,
        ty: &CType,
        vla_bounds: &[Option<Expr>],
        frame: &mut Frame,
        objects: &mut ObjectFrames,
        span: Span,
    ) -> Result<CType, Diagnostic> {
        let (resolved, used) = self.resolve_decl_type_impl(ty, vla_bounds, frame, objects)?;
        if used != vla_bounds.len() {
            return Err(Diagnostic::error(
                "internal error: variable length array bounds did not match the declared type",
                span,
            ));
        }
        Ok(resolved)
    }

    fn resolve_decl_type_impl(
        &mut self,
        ty: &CType,
        vla_bounds: &[Option<Expr>],
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(CType, usize), Diagnostic> {
        match ty {
            CType::Array(inner, len) => {
                let (resolved_len, used_here) = if *len == 0 {
                    match vla_bounds.first() {
                        Some(Some(expr)) => (self.evaluate_vla_bound(expr, frame, objects)?, 1),
                        Some(None) => (0, 1),
                        None => (0, 0),
                    }
                } else {
                    (*len, 0)
                };
                let (resolved_inner, used_inner) =
                    self.resolve_decl_type_impl(inner, &vla_bounds[used_here..], frame, objects)?;
                Ok((
                    CType::array_of(resolved_inner, resolved_len),
                    used_here + used_inner,
                ))
            }
            CType::Pointer(inner) => {
                let (resolved_inner, used) =
                    self.resolve_decl_type_impl(inner, vla_bounds, frame, objects)?;
                Ok((CType::pointer_to(resolved_inner), used))
            }
            CType::Qualified(inner, qualifiers) => {
                let (resolved_inner, used) =
                    self.resolve_decl_type_impl(inner, vla_bounds, frame, objects)?;
                Ok((CType::qualified(resolved_inner, *qualifiers), used))
            }
            CType::Function(ret, params, is_variadic) => {
                let (resolved_ret, used) =
                    self.resolve_decl_type_impl(ret, vla_bounds, frame, objects)?;
                let resolved = if *is_variadic {
                    CType::variadic_function(resolved_ret, params.to_vec())
                } else {
                    CType::function(resolved_ret, params.to_vec())
                };
                Ok((resolved, used))
            }
            _ => Ok((ty.clone(), 0)),
        }
    }

    fn check_static_array_parameter(
        &mut self,
        param: &Parameter,
        arg: &TypedValue,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let Some(bound_expr) = &param.static_array_bound else {
            return Ok(());
        };
        let required = self.evaluate_vla_bound(bound_expr, frame, objects)?;
        let pointer = arg.as_pointer(param.span)?;
        if pointer.is_null() {
            return Err(Diagnostic::ub(
                format!(
                    "argument for parameter {} declared with [static {}] is a null pointer",
                    param.name.as_deref().unwrap_or("<unnamed>"),
                    required
                ),
                param.span,
                Some("6.7.6.3p7"),
            ));
        }
        let pointee_ty = param.ty.element_type().ok_or_else(|| {
            Diagnostic::error(
                "static array parameter does not have adjusted pointer type",
                param.span,
            )
        })?;
        let limit = self.object_pointer_limit(objects, &pointer, pointee_ty).ok_or_else(|| {
            Diagnostic::ub(
                "pointer argument for a [static] array parameter does not point into a live object",
                param.span,
                Some("6.7.6.3p7"),
            )
        })?;
        let remaining = limit.saturating_sub(pointer.offset);
        if remaining < required as isize {
            return Err(Diagnostic::ub(
                format!(
                    "argument for parameter {} declared with [static {}] does not provide enough elements",
                    param.name.as_deref().unwrap_or("<unnamed>"),
                    required
                ),
                param.span,
                Some("6.7.6.3p7"),
            ));
        }
        Ok(())
    }

    fn evaluate_vla_bound(
        &mut self,
        expr: &Expr,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<usize, Diagnostic> {
        self.begin_full_expression_for(expr);
        let value = self.eval_rvalue(expr, frame, objects)?;
        self.end_full_expression();
        if !value.ty.is_integer() {
            return Err(Diagnostic::error(
                "variable length array bound must have integer type",
                expr.span(),
            ));
        }
        let value = self.integer_promotion(value, expr.span())?.to_int()?;
        if value <= 0 {
            return Err(Diagnostic::ub(
                "variable length array bound evaluated to a non-positive value",
                expr.span(),
                Some("6.7.5.2"),
            ));
        }
        usize::try_from(value).map_err(|_| {
            Diagnostic::error(
                "variable length array bound is out of supported range",
                expr.span(),
            )
        })
    }

    pub(super) fn function_type(&self, function: &FunctionDef) -> CType {
        if !function.has_prototype {
            return CType::function(function.return_type.clone(), Vec::new());
        }
        let params = function
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect();
        if function.is_variadic {
            CType::variadic_function(function.return_type.clone(), params)
        } else {
            CType::function(function.return_type.clone(), params)
        }
    }

    pub(super) fn function_declaration_type(&self, function: &FunctionDecl) -> CType {
        if !function.has_prototype {
            return CType::function(function.return_type.clone(), Vec::new());
        }
        let params = function
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect();
        if function.is_variadic {
            CType::variadic_function(function.return_type.clone(), params)
        } else {
            CType::function(function.return_type.clone(), params)
        }
    }

    fn function_designator_value(&self, function: &FunctionDef) -> TypedValue {
        TypedValue::function(self.function_type(function), function_symbol(function))
    }

    pub(super) fn call_target_signature<'b>(
        &self,
        ty: &'b CType,
        span: Span,
    ) -> Result<(&'b CType, &'b [CType], bool), Diagnostic> {
        match ty.unqualified() {
            CType::Function(ret, params, is_variadic) => Ok((ret, params, *is_variadic)),
            CType::Pointer(inner) => match inner.unqualified() {
                CType::Function(ret, params, is_variadic) => Ok((ret, params, *is_variadic)),
                _ => Err(Diagnostic::error("unsupported call target", span)),
            },
            _ => Err(Diagnostic::error("unsupported call target", span)),
        }
    }

    pub(super) fn reject_missing_return_value(
        &self,
        value: &TypedValue,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if value.missing_return {
            Err(Diagnostic::ub(
                "value of function call that reached the end of a non-void function is used",
                span,
                Some("6.9.1p12"),
            ))
        } else {
            Ok(())
        }
    }

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

    fn cross_unit_tagged_type_compatible_inner(
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

    fn exec_block(
        &mut self,
        block: &Block,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        self.push_block_scope(frame, block, objects);
        for item in &block.items {
            match item {
                BlockItem::Declaration(decl) => {
                    self.exec_declaration(decl, frame, objects)?;
                    self.trace_cboxes_main_event(decl.span, frame, objects)?;
                }
                BlockItem::FunctionDeclaration(decl) => self.exec_function_declaration(decl, frame),
                BlockItem::Statement(stmt) => match self.exec_statement(stmt, frame, objects)? {
                    Flow::Continue => {}
                    Flow::Goto(label, span) => {
                        if let Some(flow) =
                            self.enter_block_at_label(block, &label, frame, objects)?
                        {
                            self.capture_cboxes_main_state_if_needed(block, frame, objects)?;
                            let scope = self
                                .pop_active_block_scope(frame.id, block.span)
                                .expect("active block scope must exist");
                            self.restore_block_scope(scope, frame, objects);
                            self.trace_cboxes_main_block_close(block, frame, objects)?;
                            return Ok(flow);
                        } else {
                            self.capture_cboxes_main_state_if_needed(block, frame, objects)?;
                            let scope = self
                                .pop_active_block_scope(frame.id, block.span)
                                .expect("active block scope must exist");
                            self.restore_block_scope(scope, frame, objects);
                            self.trace_cboxes_main_block_close(block, frame, objects)?;
                            return Ok(Flow::Goto(label, span));
                        }
                    }
                    flow => {
                        self.capture_cboxes_main_state_if_needed(block, frame, objects)?;
                        let scope = self
                            .pop_active_block_scope(frame.id, block.span)
                            .expect("active block scope must exist");
                        self.restore_block_scope(scope, frame, objects);
                        self.trace_cboxes_main_block_close(block, frame, objects)?;
                        return Ok(flow);
                    }
                },
            }
        }
        self.capture_cboxes_main_state_if_needed(block, frame, objects)?;
        let scope = self
            .pop_active_block_scope(frame.id, block.span)
            .expect("active block scope must exist");
        self.restore_block_scope(scope, frame, objects);
        self.trace_cboxes_main_block_close(block, frame, objects)?;
        Ok(Flow::Continue)
    }

    fn exec_function_declaration(&mut self, decl: &FunctionDecl, frame: &mut Frame) {
        frame.bindings.remove(&decl.name);
        frame.object_decls.remove(&decl.name);
        frame.function_decls.insert(decl.name.clone(), decl.clone());
    }

    fn capture_cboxes_main_state_if_needed(
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

    fn trace_cboxes_main_event(
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

    fn trace_cboxes_main_branch_event(
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

    fn trace_cboxes_main_block_close(
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
                let member_value = self.extract_union_member(value, ty, member, span)?;
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
                let value = self.extract_union_member(value, ty, member, span)?;
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

    fn exec_declaration(
        &mut self,
        decl: &Declaration,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        if decl.storage_class == Some(StorageClass::Extern) {
            frame.bindings.remove(&decl.name);
            frame.function_decls.remove(&decl.name);
            frame.object_decls.insert(decl.name.clone(), decl.clone());
            return Ok(());
        }
        let resolved_ty =
            self.resolve_decl_type(&decl.ty, &decl.vla_bounds, frame, objects, decl.span)?;
        if matches!(resolved_ty.unqualified(), CType::Void | CType::Function(..)) {
            return Err(Diagnostic::error(
                format!("an object cannot have type {resolved_ty}"),
                decl.span,
            ));
        }
        let has_variably_modified_type = decl.vla_bounds.iter().any(|bound| bound.is_some());
        let has_vla_object_type =
            has_variably_modified_type && matches!(resolved_ty.unqualified(), CType::Array(_, _));
        if has_vla_object_type && decl.storage_class == Some(StorageClass::Static) {
            return Err(Diagnostic::error(
                "variable length array objects cannot have static storage duration",
                decl.span,
            ));
        }
        if !self.type_is_complete(&resolved_ty)
            && !matches!(resolved_ty.unqualified(), CType::Array(_, 0) if decl.init.is_some())
        {
            return Err(Diagnostic::error(
                format!("object definition has incomplete type {resolved_ty}"),
                decl.span,
            ));
        }
        let (object, static_zero) = if decl.storage_class == Some(StorageClass::Static) {
            if let Some(object) = self.local_static_bindings.get(&decl.span).copied() {
                (object, false)
            } else {
                let object = self.allocate_object(
                    objects,
                    resolved_ty.clone(),
                    StorageDuration::Static,
                    decl.span,
                    false,
                    false,
                )?;
                self.local_static_bindings.insert(decl.span, object);
                (object, true)
            }
        } else if !has_variably_modified_type {
            let cache_key = (frame.id, decl.span);
            if let Some(object) = self
                .automatic_object_bindings
                .get(&cache_key)
                .copied()
                .filter(|object| {
                    self.lookup_object(objects, *object)
                        .is_some_and(|state| state.alive)
                })
            {
                (object, false)
            } else {
                let object = self.allocate_object(
                    objects,
                    resolved_ty.clone(),
                    StorageDuration::Automatic,
                    decl.span,
                    decl.storage_class == Some(StorageClass::Register),
                    false,
                )?;
                self.automatic_object_bindings.insert(cache_key, object);
                (object, false)
            }
        } else {
            (
                self.allocate_object(
                    objects,
                    resolved_ty.clone(),
                    StorageDuration::Automatic,
                    decl.span,
                    decl.storage_class == Some(StorageClass::Register),
                    true,
                )?,
                false,
            )
        };
        self.ensure_object_alignment(object, &resolved_ty, decl.alignment);
        frame.object_decls.remove(&decl.name);
        frame.function_decls.remove(&decl.name);
        frame.bindings.insert(decl.name.clone(), object);
        if decl.storage_class != Some(StorageClass::Static)
            || !self
                .lookup_object(objects, object)
                .is_some_and(|state| state.initialized)
        {
            if decl.storage_class == Some(StorageClass::Static) {
                if let Some(initializer) = decl.init.as_ref() {
                    self.validate_static_initializer(initializer, frame, objects)?;
                }
            }
            self.initialize_declared_object(
                object,
                &resolved_ty,
                decl.init.as_ref(),
                frame,
                objects,
                static_zero,
                decl.span,
            )?;
        }
        Ok(())
    }

    fn exec_statement(
        &mut self,
        stmt: &Statement,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        self.consume_execution_step(statement_span(stmt))?;
        let implicit_scope_span = matches!(
            stmt,
            Statement::DoWhile { .. }
                | Statement::If { .. }
                | Statement::Switch { .. }
                | Statement::While { .. }
        )
        .then(|| statement_span(stmt));
        if let Some(span) = implicit_scope_span {
            self.push_scope_info(frame, span, false, false, true, objects);
        }
        let result = (|| -> Result<Flow, Diagnostic> {
            match stmt {
                Statement::Block(block) => self.exec_block(block, frame, objects),
                Statement::Break(_) => Ok(Flow::LoopBreak),
                Statement::Continue(_) => Ok(Flow::LoopContinue),
                Statement::DoWhile {
                    body,
                    condition,
                    span,
                } => loop {
                    match self.exec_controlled_substatement(body, frame, objects)? {
                        Flow::Continue => {}
                        Flow::LoopContinue => {}
                        Flow::LoopBreak => return Ok(Flow::Continue),
                        flow => return Ok(flow),
                    }
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::DoWhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&cond, condition.span())? {
                        return Ok(Flow::Continue);
                    }
                },
                Statement::Expression(expr, span) => {
                    if let Some(expr) = expr {
                        let _ = self.eval_in_setjmp_context(
                            expr,
                            SetjmpContextKind::ExpressionStatement,
                            *span,
                            frame,
                            objects,
                        )?;
                    }
                    self.trace_cboxes_main_event(*span, frame, objects)?;
                    Ok(Flow::Continue)
                }
                Statement::For {
                    init,
                    condition,
                    step,
                    body,
                    span,
                } => self.exec_for(
                    init.as_ref(),
                    condition.as_ref(),
                    step.as_ref(),
                    body,
                    *span,
                    frame,
                    objects,
                ),
                Statement::Goto { label, span } => Ok(Flow::Goto(label.clone(), *span)),
                Statement::Labeled {
                    statement, span, ..
                } => {
                    if self.active_switch_dispatch_depth == 0 {
                        Err(Diagnostic::error(
                            "case/default label is not within an active switch dispatch",
                            *span,
                        ))
                    } else {
                        self.exec_statement(statement, frame, objects)
                    }
                }
                Statement::Return(expr, span) => {
                    let value = if let Some(expr) = expr {
                        self.begin_full_expression_for(expr);
                        self.eval_rvalue(expr, frame, objects)?
                    } else {
                        TypedValue::void()
                    };
                    self.end_full_expression();
                    self.trace_cboxes_main_event(*span, frame, objects)?;
                    Ok(Flow::Return(value.with_span(*span)))
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                    else_keyword_span,
                    branch_keyword_span,
                    span,
                } => {
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::IfCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    let took_then_branch = self.scalar_truthy(&cond, condition.span())?;
                    let skipped_span = if took_then_branch {
                        else_keyword_span.zip(else_branch.as_ref()).map(
                            |(else_keyword_span, else_branch)| {
                                else_keyword_span.merge(else_branch.span())
                            },
                        )
                    } else {
                        Some(Span::new(
                            span.file,
                            branch_keyword_span.start,
                            then_branch.span().end,
                        ))
                    };
                    self.trace_cboxes_main_branch_event(
                        condition.span(),
                        skipped_span,
                        frame,
                        objects,
                    )?;
                    if took_then_branch {
                        self.exec_controlled_substatement(then_branch, frame, objects)
                    } else if let Some(else_branch) = else_branch {
                        self.exec_controlled_substatement(else_branch, frame, objects)
                    } else {
                        Ok(Flow::Continue)
                    }
                }
                Statement::Switch {
                    expr, body, span, ..
                } => self.exec_switch(expr, body, *span, frame, objects),
                Statement::UserLabeled { statement, .. } => {
                    self.exec_statement(statement, frame, objects)
                }
                Statement::While {
                    condition,
                    body,
                    span,
                    ..
                } => loop {
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::WhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    self.trace_cboxes_main_event(condition.span(), frame, objects)?;
                    let condition_is_true = self.scalar_truthy(&cond, condition.span())?;
                    if !condition_is_true {
                        return Ok(Flow::Continue);
                    }
                    if self.execution_steps_remaining.is_some()
                        && Self::expression_is_constant_nonzero(condition)
                        && Self::statement_has_no_runtime_effect(body)
                    {
                        return Err(Diagnostic::execution_step_limit(condition.span()));
                    }
                    match self.exec_controlled_substatement(body, frame, objects)? {
                        Flow::Continue => {}
                        Flow::LoopContinue => continue,
                        Flow::LoopBreak => return Ok(Flow::Continue),
                        flow => return Ok(flow),
                    }
                },
            }
        })();
        if let Some(span) = implicit_scope_span
            && let Some(scope) = self.pop_active_block_scope(frame.id, span)
        {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn statement_has_own_block_scope(statement: &Statement) -> bool {
        matches!(
            statement,
            Statement::Block(_)
                | Statement::DoWhile { .. }
                | Statement::For { .. }
                | Statement::If { .. }
                | Statement::Switch { .. }
                | Statement::While { .. }
        )
    }

    fn exec_controlled_substatement(
        &mut self,
        statement: &Statement,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        if Self::statement_has_own_block_scope(statement) {
            return self.exec_statement(statement, frame, objects);
        }
        let span = statement_span(statement);
        self.push_scope_info(frame, span, false, false, true, objects);
        let result = self.exec_statement(statement, frame, objects);
        if let Some(scope) = self.pop_active_block_scope(frame.id, span) {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn expression_is_constant_nonzero(expr: &Expr) -> bool {
        match expr {
            Expr::Number(number, _) => match number.value {
                NumberValue::Integer(value) => value != 0,
                NumberValue::Floating(value) => value != 0.0,
            },
            Expr::CharLiteral(value, _) => *value != 0,
            Expr::WideCharLiteral(value, _) => *value != 0,
            Expr::Utf16CharLiteral(value, _) => *value != 0,
            Expr::Utf32CharLiteral(value, _) => *value != 0,
            _ => false,
        }
    }

    fn statement_has_no_runtime_effect(statement: &Statement) -> bool {
        match statement {
            Statement::Expression(None, _) => true,
            Statement::Block(block) => block.items.iter().all(|item| {
                matches!(
                    item,
                    BlockItem::Statement(statement)
                        if Self::statement_has_no_runtime_effect(statement)
                )
            }),
            _ => false,
        }
    }

    fn enter_block_at_label(
        &mut self,
        block: &Block,
        label: &str,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let Some(mut index) = block.items.iter().position(|item| match item {
            BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
            BlockItem::Statement(stmt) => self.statement_contains_label(stmt, label),
        }) else {
            return Ok(None);
        };
        self.push_scope_info(frame, block.span, true, true, true, objects);
        let result = (|| -> Result<Option<Flow>, Diagnostic> {
            self.prepare_block_control_entry(block, index, "goto", frame, objects)?;
            let mut pending_label = Some(label.to_owned());

            loop {
                if index >= block.items.len() {
                    break Ok(Some(Flow::Continue));
                }
                let flow = match &block.items[index] {
                    BlockItem::Declaration(decl) => {
                        if pending_label.is_none() {
                            self.exec_declaration(decl, frame, objects)?;
                        }
                        Flow::Continue
                    }
                    BlockItem::FunctionDeclaration(decl) => {
                        if pending_label.is_none() {
                            self.exec_function_declaration(decl, frame);
                        }
                        Flow::Continue
                    }
                    BlockItem::Statement(stmt) => {
                        if let Some(target) = pending_label.take() {
                            self.enter_statement_at_label(stmt, &target, frame, objects)?
                                .unwrap_or(Flow::Goto(target, statement_span(stmt)))
                        } else {
                            self.exec_statement(stmt, frame, objects)?
                        }
                    }
                };

                match flow {
                    Flow::Continue => index += 1,
                    Flow::Goto(target, span) => {
                        if let Some(found) = block.items.iter().position(|item| match item {
                            BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
                            BlockItem::Statement(stmt) => {
                                self.statement_contains_label(stmt, &target)
                            }
                        }) {
                            pending_label = Some(target);
                            index = found;
                        } else {
                            break Ok(Some(Flow::Goto(target, span)));
                        }
                    }
                    flow => break Ok(Some(flow)),
                }
            }
        })();
        if let Some(scope) = self.pop_active_block_scope(frame.id, block.span) {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn prepare_block_control_entry(
        &mut self,
        block: &Block,
        target_index: usize,
        entry_kind: &str,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let declarations_after_target = block
            .items
            .iter()
            .skip(target_index.saturating_add(1))
            .filter_map(|item| match item {
                BlockItem::Declaration(decl)
                    if decl.vla_bounds.iter().any(|bound| bound.is_some()) =>
                {
                    Some(decl.span)
                }
                _ => None,
            })
            .collect::<HashSet<_>>();
        let mut retired = Vec::new();
        if let Some(frame_ids) = objects.last_ids() {
            let ids = frame_ids
                .iter()
                .copied()
                .filter(|id| {
                    objects.get(*id).is_some_and(|object| {
                        object.variably_modified
                            && declarations_after_target.contains(&object.declaration_span)
                    })
                })
                .collect::<Vec<_>>();
            for id in ids {
                if let Some(object) = objects.remove_from_last(id) {
                    self.retire_automatic_object(id, object);
                    retired.push(id);
                }
            }
        }
        if !retired.is_empty() {
            self.note_stream_buffer_lifetime_end(&retired);
            frame.bindings.retain(|_, object| !retired.contains(object));
            self.forget_retired_restrict_sources(&retired);
        }

        for item in block.items.iter().take(target_index) {
            match item {
                BlockItem::Declaration(decl) => {
                    let already_active = frame.bindings.get(&decl.name).is_some_and(|object| {
                        self.lookup_object(objects, *object)
                            .is_some_and(|state| state.alive && state.declaration_span == decl.span)
                    });
                    if already_active {
                        continue;
                    }
                    if decl.vla_bounds.iter().any(|bound| bound.is_some()) {
                        return Err(Diagnostic::error(
                            format!(
                                "{entry_kind} enters the scope of an object with variably modified type"
                            ),
                            decl.span,
                        ));
                    }
                    if decl.storage_class == Some(StorageClass::Static) {
                        self.exec_declaration(decl, frame, objects)?;
                    } else {
                        let mut skipped = decl.clone();
                        skipped.init = None;
                        self.exec_declaration(&skipped, frame, objects)?;
                    }
                }
                BlockItem::FunctionDeclaration(decl) => {
                    self.exec_function_declaration(decl, frame);
                }
                BlockItem::Statement(_) => {}
            }
        }
        Ok(())
    }

    fn enter_statement_at_label(
        &mut self,
        stmt: &Statement,
        label: &str,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let implicit_scope_span = (matches!(
            stmt,
            Statement::DoWhile { .. }
                | Statement::If { .. }
                | Statement::Switch { .. }
                | Statement::While { .. }
        ) && self.statement_contains_label(stmt, label)
            && self
                .active_block_scope(frame.id, statement_span(stmt))
                .is_none())
        .then(|| statement_span(stmt));
        if let Some(span) = implicit_scope_span {
            self.push_scope_info(frame, span, false, false, true, objects);
        }
        let result = self.enter_statement_at_label_in_scope(stmt, label, frame, objects);
        if let Some(span) = implicit_scope_span
            && let Some(scope) = self.pop_active_block_scope(frame.id, span)
        {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn enter_statement_at_label_in_scope(
        &mut self,
        stmt: &Statement,
        label: &str,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let flow = match stmt {
            Statement::Block(block) => {
                return self.enter_block_at_label(block, label, frame, objects);
            }
            Statement::DoWhile {
                body,
                condition,
                span,
            } if self.statement_contains_label(body, label) => {
                let mut flow = self
                    .enter_controlled_substatement_at_label(body, label, frame, objects)?
                    .unwrap_or(Flow::Continue);
                loop {
                    match flow {
                        Flow::Continue | Flow::LoopContinue => {}
                        Flow::LoopBreak => break Flow::Continue,
                        other => break other,
                    }
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::DoWhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&cond, condition.span())? {
                        break Flow::Continue;
                    }
                    flow = self.exec_controlled_substatement(body, frame, objects)?;
                }
            }
            Statement::For {
                init,
                condition,
                step,
                body,
                span,
            } if self.statement_contains_label(body, label) => {
                self.push_scope_info(frame, *span, true, true, true, objects);
                let result = (|| -> Result<Flow, Diagnostic> {
                    if let Some(ForInit::Declarations(declarations)) = init {
                        for declaration in declarations {
                            if declaration.vla_bounds.iter().any(Option::is_some) {
                                return Err(Diagnostic::error(
                                    "goto enters the scope of an object with variably modified type",
                                    declaration.span,
                                ));
                            }
                            let mut skipped = declaration.clone();
                            skipped.init = None;
                            self.exec_declaration(&skipped, frame, objects)?;
                        }
                    }
                    let mut flow = self
                        .enter_controlled_substatement_at_label(body, label, frame, objects)?
                        .unwrap_or(Flow::Continue);
                    loop {
                        match flow {
                            Flow::Continue | Flow::LoopContinue => {}
                            Flow::LoopBreak => break Ok(Flow::Continue),
                            other => break Ok(other),
                        }
                        if let Some(step) = step {
                            self.begin_full_expression_for(step);
                            let value = self.eval_rvalue(step, frame, objects);
                            self.end_full_expression();
                            let _ = value?;
                        }
                        if let Some(condition) = condition {
                            let cond = self.eval_in_setjmp_context(
                                condition,
                                SetjmpContextKind::ForCondition,
                                *span,
                                frame,
                                objects,
                            )?;
                            if !self.scalar_truthy(&cond, condition.span())? {
                                break Ok(Flow::Continue);
                            }
                        }
                        flow = self.exec_controlled_substatement(body, frame, objects)?;
                    }
                })();
                if let Some(scope) = self.pop_active_block_scope(frame.id, *span) {
                    self.restore_block_scope(scope, frame, objects);
                }
                result?
            }
            Statement::If { then_branch, .. }
                if self.statement_contains_label(then_branch, label) =>
            {
                self.enter_controlled_substatement_at_label(then_branch, label, frame, objects)?
                    .unwrap_or(Flow::Continue)
            }
            Statement::If {
                else_branch: Some(else_branch),
                ..
            } if self.statement_contains_label(else_branch, label) => self
                .enter_controlled_substatement_at_label(else_branch, label, frame, objects)?
                .unwrap_or(Flow::Continue),
            Statement::Labeled { statement, .. } => {
                return self.enter_statement_at_label(statement, label, frame, objects);
            }
            Statement::Switch { body, .. } if self.block_contains_label(body, label) => {
                let _switch_depth = DepthGuard::enter(&mut self.active_switch_dispatch_depth);
                let result = self.enter_block_at_label(body, label, frame, objects);
                match result? {
                    Some(Flow::LoopBreak) | Some(Flow::Continue) | None => Flow::Continue,
                    Some(flow) => flow,
                }
            }
            Statement::UserLabeled {
                label: stmt_label,
                statement,
                ..
            } if stmt_label == label => self.exec_statement(statement, frame, objects)?,
            Statement::UserLabeled { statement, .. } => {
                return self.enter_statement_at_label(statement, label, frame, objects);
            }
            Statement::While {
                condition,
                body,
                span,
                ..
            } if self.statement_contains_label(body, label) => {
                let mut flow = self
                    .enter_controlled_substatement_at_label(body, label, frame, objects)?
                    .unwrap_or(Flow::Continue);
                loop {
                    match flow {
                        Flow::Continue | Flow::LoopContinue => {}
                        Flow::LoopBreak => break Flow::Continue,
                        other => break other,
                    }
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::WhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&cond, condition.span())? {
                        break Flow::Continue;
                    }
                    flow = self.exec_controlled_substatement(body, frame, objects)?;
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(flow))
    }

    fn enter_controlled_substatement_at_label(
        &mut self,
        statement: &Statement,
        label: &str,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        if Self::statement_has_own_block_scope(statement) {
            return self.enter_statement_at_label(statement, label, frame, objects);
        }
        let span = statement_span(statement);
        let scope_was_pushed = self.active_block_scope(frame.id, span).is_none();
        if scope_was_pushed {
            self.push_scope_info(frame, span, false, false, true, objects);
        }
        let result = self.enter_statement_at_label(statement, label, frame, objects);
        if scope_was_pushed && let Some(scope) = self.pop_active_block_scope(frame.id, span) {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn block_contains_label(&self, block: &Block, label: &str) -> bool {
        block.items.iter().any(|item| match item {
            BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
            BlockItem::Statement(stmt) => self.statement_contains_label(stmt, label),
        })
    }

    fn statement_contains_label(&self, stmt: &Statement, label: &str) -> bool {
        match stmt {
            Statement::Block(block) => self.block_contains_label(block, label),
            Statement::DoWhile { body, .. } => self.statement_contains_label(body, label),
            Statement::For { body, .. } => self.statement_contains_label(body, label),
            Statement::Goto { .. } => false,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.statement_contains_label(then_branch, label)
                    || else_branch
                        .as_ref()
                        .is_some_and(|stmt| self.statement_contains_label(stmt, label))
            }
            Statement::Labeled { statement, .. } => self.statement_contains_label(statement, label),
            Statement::Switch { body, .. } => self.block_contains_label(body, label),
            Statement::UserLabeled {
                label: stmt_label,
                statement,
                ..
            } => stmt_label == label || self.statement_contains_label(statement, label),
            Statement::While { body, .. } => self.statement_contains_label(body, label),
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(_, _)
            | Statement::Return(_, _) => false,
        }
    }

    fn block_contains_span(&self, block: &Block, target: Span) -> bool {
        block.items.iter().any(|item| match item {
            BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
            BlockItem::Statement(stmt) => self.statement_contains_span(stmt, target),
        })
    }

    fn statement_contains_span(&self, stmt: &Statement, target: Span) -> bool {
        if statement_span(stmt) == target {
            return true;
        }
        match stmt {
            Statement::Block(block) => self.block_contains_span(block, target),
            Statement::DoWhile { body, .. } => self.statement_contains_span(body, target),
            Statement::For { body, .. } => self.statement_contains_span(body, target),
            Statement::Goto { .. } => false,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.statement_contains_span(then_branch, target)
                    || else_branch
                        .as_ref()
                        .is_some_and(|stmt| self.statement_contains_span(stmt, target))
            }
            Statement::Labeled { statement, .. } => self.statement_contains_span(statement, target),
            Statement::Switch { body, .. } => self.block_contains_span(body, target),
            Statement::UserLabeled { statement, .. } => {
                self.statement_contains_span(statement, target)
            }
            Statement::While { body, .. } => self.statement_contains_span(body, target),
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(_, _)
            | Statement::Return(_, _) => false,
        }
    }

    fn resume_function_after_longjmp(
        &mut self,
        function: &FunctionDef,
        site: SetjmpSite,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        self.resume_block_after_longjmp(&function.body, site, frame, objects)?
            .ok_or_else(|| {
                Diagnostic::error(
                    "internal error: longjmp target site no longer exists",
                    site.stmt_span,
                )
            })
    }

    fn resume_block_after_longjmp(
        &mut self,
        block: &Block,
        site: SetjmpSite,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let Some(mut index) = block.items.iter().position(|item| match item {
            BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
            BlockItem::Statement(stmt) => self.statement_contains_span(stmt, site.stmt_span),
        }) else {
            return Ok(None);
        };

        let has_active_scope = self.active_block_scope(frame.id, block.span).is_some();
        let mut pending_site = Some(site);

        loop {
            if index >= block.items.len() {
                if has_active_scope {
                    let scope = self
                        .pop_active_block_scope(frame.id, block.span)
                        .expect("active block scope must exist");
                    self.restore_block_scope(scope, frame, objects);
                }
                return Ok(Some(Flow::Continue));
            }

            let flow = match &block.items[index] {
                BlockItem::Declaration(decl) => {
                    if pending_site.is_some() {
                        return Err(Diagnostic::error(
                            "internal error: longjmp target resolved to a declaration",
                            site.stmt_span,
                        ));
                    }
                    self.exec_declaration(decl, frame, objects)?;
                    Flow::Continue
                }
                BlockItem::FunctionDeclaration(decl) => {
                    if pending_site.is_some() {
                        return Err(Diagnostic::error(
                            "internal error: longjmp target resolved to a declaration",
                            site.stmt_span,
                        ));
                    }
                    self.exec_function_declaration(decl, frame);
                    Flow::Continue
                }
                BlockItem::Statement(stmt) => {
                    if let Some(site) = pending_site.take() {
                        self.resume_statement_after_longjmp(stmt, site, frame, objects)?
                    } else {
                        self.exec_statement(stmt, frame, objects)?
                    }
                }
            };

            match flow {
                Flow::Continue => index += 1,
                flow => {
                    if has_active_scope {
                        let scope = self
                            .pop_active_block_scope(frame.id, block.span)
                            .expect("active block scope must exist");
                        self.restore_block_scope(scope, frame, objects);
                    }
                    return Ok(Some(flow));
                }
            }
        }
    }

    fn resume_statement_after_longjmp(
        &mut self,
        stmt: &Statement,
        site: SetjmpSite,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        let implicit_scope_span = matches!(
            stmt,
            Statement::DoWhile { .. }
                | Statement::If { .. }
                | Statement::Switch { .. }
                | Statement::While { .. }
        )
        .then(|| statement_span(stmt))
        .filter(|span| self.active_block_scope(frame.id, *span).is_some());
        let result = self.resume_statement_after_longjmp_in_scope(stmt, site, frame, objects);
        if let Some(span) = implicit_scope_span
            && let Some(scope) = self.pop_active_block_scope(frame.id, span)
        {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn resume_statement_after_longjmp_in_scope(
        &mut self,
        stmt: &Statement,
        site: SetjmpSite,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        if statement_span(stmt) == site.stmt_span {
            return self.resume_exact_statement_after_longjmp(stmt, site, frame, objects);
        }

        match stmt {
            Statement::Block(block) => self
                .resume_block_after_longjmp(block, site, frame, objects)?
                .ok_or_else(|| {
                    Diagnostic::error(
                        "internal error: longjmp block target disappeared",
                        site.stmt_span,
                    )
                }),
            Statement::DoWhile {
                body,
                condition,
                span,
            } if self.statement_contains_span(body, site.stmt_span) => {
                let mut flow =
                    self.resume_controlled_substatement_after_longjmp(body, site, frame, objects)?;
                loop {
                    match flow {
                        Flow::Continue | Flow::LoopContinue => {}
                        Flow::LoopBreak => return Ok(Flow::Continue),
                        other => return Ok(other),
                    }
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::DoWhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&cond, condition.span())? {
                        return Ok(Flow::Continue);
                    }
                    flow = self.exec_controlled_substatement(body, frame, objects)?;
                }
            }
            Statement::For {
                condition,
                step,
                body,
                span,
                ..
            } if self.statement_contains_span(body, site.stmt_span) => {
                let result = (|| -> Result<Flow, Diagnostic> {
                    let mut flow = self
                        .resume_controlled_substatement_after_longjmp(body, site, frame, objects)?;
                    loop {
                        match flow {
                            Flow::Continue | Flow::LoopContinue => {}
                            Flow::LoopBreak => break Ok(Flow::Continue),
                            other => break Ok(other),
                        }
                        if let Some(step) = step {
                            self.begin_full_expression_for(step);
                            let value = self.eval_rvalue(step, frame, objects);
                            self.end_full_expression();
                            let _ = value?;
                        }
                        if let Some(condition) = condition {
                            let cond = self.eval_in_setjmp_context(
                                condition,
                                SetjmpContextKind::ForCondition,
                                *span,
                                frame,
                                objects,
                            )?;
                            if !self.scalar_truthy(&cond, condition.span())? {
                                break Ok(Flow::Continue);
                            }
                        }
                        flow = self.exec_controlled_substatement(body, frame, objects)?;
                    }
                })();
                if let Some(scope) = self.pop_active_block_scope(frame.id, *span) {
                    self.restore_block_scope(scope, frame, objects);
                }
                result
            }
            Statement::If { then_branch, .. }
                if self.statement_contains_span(then_branch, site.stmt_span) =>
            {
                self.resume_controlled_substatement_after_longjmp(then_branch, site, frame, objects)
            }
            Statement::If {
                else_branch: Some(else_branch),
                ..
            } if self.statement_contains_span(else_branch, site.stmt_span) => {
                self.resume_controlled_substatement_after_longjmp(else_branch, site, frame, objects)
            }
            Statement::Labeled { statement, .. } => {
                self.resume_statement_after_longjmp(statement, site, frame, objects)
            }
            Statement::Switch { body, .. } if self.block_contains_span(body, site.stmt_span) => {
                let _switch_depth = DepthGuard::enter(&mut self.active_switch_dispatch_depth);
                let result = self.resume_block_after_longjmp(body, site, frame, objects);
                match result? {
                    Some(Flow::LoopBreak) | Some(Flow::Continue) | None => Ok(Flow::Continue),
                    Some(flow) => Ok(flow),
                }
            }
            Statement::UserLabeled { statement, .. } => {
                self.resume_statement_after_longjmp(statement, site, frame, objects)
            }
            Statement::While {
                condition,
                body,
                span,
                ..
            } if self.statement_contains_span(body, site.stmt_span) => {
                let mut flow =
                    self.resume_controlled_substatement_after_longjmp(body, site, frame, objects)?;
                loop {
                    match flow {
                        Flow::Continue | Flow::LoopContinue => {}
                        Flow::LoopBreak => return Ok(Flow::Continue),
                        other => return Ok(other),
                    }
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::WhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&cond, condition.span())? {
                        return Ok(Flow::Continue);
                    }
                    flow = self.exec_controlled_substatement(body, frame, objects)?;
                }
            }
            _ => Err(Diagnostic::error(
                "internal error: longjmp target statement could not be resumed",
                site.stmt_span,
            )),
        }
    }

    fn resume_controlled_substatement_after_longjmp(
        &mut self,
        statement: &Statement,
        site: SetjmpSite,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        let implicit_scope_span = (!Self::statement_has_own_block_scope(statement))
            .then(|| statement_span(statement))
            .filter(|span| self.active_block_scope(frame.id, *span).is_some());
        let result = self.resume_statement_after_longjmp(statement, site, frame, objects);
        if let Some(span) = implicit_scope_span
            && let Some(scope) = self.pop_active_block_scope(frame.id, span)
        {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn resume_exact_statement_after_longjmp(
        &mut self,
        stmt: &Statement,
        site: SetjmpSite,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        match (stmt, site.kind) {
            (Statement::Expression(expr, span), SetjmpContextKind::ExpressionStatement) => {
                if let Some(expr) = expr {
                    let _ = self.eval_in_setjmp_context(
                        expr,
                        SetjmpContextKind::ExpressionStatement,
                        *span,
                        frame,
                        objects,
                    )?;
                }
                Ok(Flow::Continue)
            }
            (
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                    span,
                    ..
                },
                SetjmpContextKind::IfCondition,
            ) => {
                let cond = self.eval_in_setjmp_context(
                    condition,
                    SetjmpContextKind::IfCondition,
                    *span,
                    frame,
                    objects,
                )?;
                if self.scalar_truthy(&cond, condition.span())? {
                    self.exec_controlled_substatement(then_branch, frame, objects)
                } else if let Some(else_branch) = else_branch {
                    self.exec_controlled_substatement(else_branch, frame, objects)
                } else {
                    Ok(Flow::Continue)
                }
            }
            (
                Statement::While {
                    condition,
                    body,
                    span,
                    ..
                },
                SetjmpContextKind::WhileCondition,
            ) => loop {
                let cond = self.eval_in_setjmp_context(
                    condition,
                    SetjmpContextKind::WhileCondition,
                    *span,
                    frame,
                    objects,
                )?;
                if !self.scalar_truthy(&cond, condition.span())? {
                    return Ok(Flow::Continue);
                }
                match self.exec_controlled_substatement(body, frame, objects)? {
                    Flow::Continue => {}
                    Flow::LoopContinue => continue,
                    Flow::LoopBreak => return Ok(Flow::Continue),
                    flow => return Ok(flow),
                }
            },
            (
                Statement::DoWhile {
                    body,
                    condition,
                    span,
                },
                SetjmpContextKind::DoWhileCondition,
            ) => loop {
                let cond = self.eval_in_setjmp_context(
                    condition,
                    SetjmpContextKind::DoWhileCondition,
                    *span,
                    frame,
                    objects,
                )?;
                if !self.scalar_truthy(&cond, condition.span())? {
                    return Ok(Flow::Continue);
                }
                match self.exec_controlled_substatement(body, frame, objects)? {
                    Flow::Continue => {}
                    Flow::LoopContinue => continue,
                    Flow::LoopBreak => return Ok(Flow::Continue),
                    flow => return Ok(flow),
                }
            },
            (
                Statement::For {
                    condition,
                    step,
                    body,
                    span,
                    ..
                },
                SetjmpContextKind::ForCondition,
            ) => {
                let Some(condition) = condition else {
                    return Err(Diagnostic::error(
                        "internal error: longjmp resumed a for statement without a condition",
                        site.stmt_span,
                    ));
                };
                let result = (|| -> Result<Flow, Diagnostic> {
                    loop {
                        let cond = self.eval_in_setjmp_context(
                            condition,
                            SetjmpContextKind::ForCondition,
                            *span,
                            frame,
                            objects,
                        )?;
                        if !self.scalar_truthy(&cond, condition.span())? {
                            break Ok(Flow::Continue);
                        }
                        match self.exec_controlled_substatement(body, frame, objects)? {
                            Flow::Continue | Flow::LoopContinue => {}
                            Flow::LoopBreak => break Ok(Flow::Continue),
                            flow => break Ok(flow),
                        }
                        if let Some(step) = step {
                            self.begin_full_expression_for(step);
                            let value = self.eval_rvalue(step, frame, objects);
                            self.end_full_expression();
                            let _ = value?;
                        }
                    }
                })();
                if let Some(scope) = self.pop_active_block_scope(frame.id, *span) {
                    self.restore_block_scope(scope, frame, objects);
                }
                result
            }
            (
                Statement::Switch {
                    expr, body, span, ..
                },
                SetjmpContextKind::SwitchExpression,
            ) => {
                let control = self.eval_in_setjmp_context(
                    expr,
                    SetjmpContextKind::SwitchExpression,
                    *span,
                    frame,
                    objects,
                )?;
                let control = self.integer_promotion(control, expr.span())?;
                self.exec_switch_with_control(control, body, frame, objects)
            }
            _ => Err(Diagnostic::error(
                "internal error: longjmp target site did not match the resumed statement",
                site.stmt_span,
            )),
        }
    }

    fn exec_switch(
        &mut self,
        expr: &Expr,
        body: &Block,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        let control = self.eval_in_setjmp_context(
            expr,
            SetjmpContextKind::SwitchExpression,
            span,
            frame,
            objects,
        )?;
        let control = self.integer_promotion(control, expr.span())?;
        self.exec_switch_with_control(control, body, frame, objects)
    }

    fn exec_switch_with_control(
        &mut self,
        control: TypedValue,
        body: &Block,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        let control_value = control.to_int()?;
        let target = if self.run_options.optimizing_precomputations {
            let cache_key = (body as *const Block as usize, control.ty.clone());
            if !self.switch_dispatch_cache.contains_key(&cache_key) {
                let dispatch = self.build_switch_dispatch(body, &control.ty, frame, objects)?;
                self.switch_dispatch_cache
                    .insert(cache_key.clone(), dispatch);
            }
            Self::switch_dispatch_target(
                self.switch_dispatch_cache
                    .get(&cache_key)
                    .expect("switch dispatch was cached"),
                control_value,
            )
        } else {
            let dispatch = self.build_switch_dispatch(body, &control.ty, frame, objects)?;
            Self::switch_dispatch_target(&dispatch, control_value)
        };
        let Some(target) = target else {
            return Ok(Flow::Continue);
        };
        let _switch_depth = DepthGuard::enter(&mut self.active_switch_dispatch_depth);
        let result = self.enter_block_at_switch_label(body, target, frame, objects);
        match result? {
            Some(Flow::LoopBreak) | Some(Flow::Continue) | None => Ok(Flow::Continue),
            Some(flow) => Ok(flow),
        }
    }

    fn build_switch_dispatch(
        &self,
        body: &Block,
        control_ty: &CType,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<SwitchDispatch, Diagnostic> {
        let mut default = None;
        let mut case_values = HashSet::default();
        let mut cases = Vec::new();
        let mut labels = Vec::new();
        Self::collect_switch_labels_from_block(body, &mut labels);
        for label in labels {
            match label {
                SwitchLabel::Case { expr, span } => {
                    let case_value = self.eval_typed_integer_constant_expr(expr, frame, objects)?;
                    let case_value = self
                        .convert_value(case_value, control_ty, *span)?
                        .to_int()?;
                    if !case_values.insert(case_value) {
                        return Err(Diagnostic::error(
                            format!(
                                "duplicate case value {} after conversion to {}",
                                case_value, control_ty
                            ),
                            *span,
                        ));
                    }
                    cases.push((case_value, *span));
                }
                SwitchLabel::Default { span } => {
                    if default.replace(*span).is_some() {
                        return Err(Diagnostic::error(
                            "multiple default labels in one switch",
                            *span,
                        ));
                    }
                }
            }
        }
        Ok(SwitchDispatch { cases, default })
    }

    pub(super) fn validate_switch_dispatch(
        &self,
        body: &Block,
        control_ty: &CType,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<(), Diagnostic> {
        self.build_switch_dispatch(body, control_ty, frame, objects)
            .map(|_| ())
    }

    fn switch_dispatch_target(dispatch: &SwitchDispatch, control: i128) -> Option<Span> {
        dispatch
            .cases
            .iter()
            .find_map(|(value, span)| (*value == control).then_some(*span))
            .or(dispatch.default)
    }

    fn collect_switch_labels_from_block<'b>(block: &'b Block, labels: &mut Vec<&'b SwitchLabel>) {
        for item in &block.items {
            if let BlockItem::Statement(statement) = item {
                Self::collect_switch_labels_from_statement(statement, labels);
            }
        }
    }

    fn collect_switch_labels_from_statement<'b>(
        statement: &'b Statement,
        labels: &mut Vec<&'b SwitchLabel>,
    ) {
        match statement {
            Statement::Block(block) => Self::collect_switch_labels_from_block(block, labels),
            Statement::DoWhile { body, .. }
            | Statement::For { body, .. }
            | Statement::While { body, .. }
            | Statement::UserLabeled {
                statement: body, ..
            } => Self::collect_switch_labels_from_statement(body, labels),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                Self::collect_switch_labels_from_statement(then_branch, labels);
                if let Some(else_branch) = else_branch {
                    Self::collect_switch_labels_from_statement(else_branch, labels);
                }
            }
            Statement::Labeled {
                label, statement, ..
            } => {
                labels.push(label);
                Self::collect_switch_labels_from_statement(statement, labels);
            }
            Statement::Switch { .. }
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(..)
            | Statement::Goto { .. }
            | Statement::Return(..) => {}
        }
    }

    fn switch_label_span(label: &SwitchLabel) -> Span {
        match label {
            SwitchLabel::Case { span, .. } | SwitchLabel::Default { span } => *span,
        }
    }

    fn block_contains_switch_label(block: &Block, target: Span) -> bool {
        block.items.iter().any(|item| match item {
            BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
            BlockItem::Statement(statement) => {
                Self::statement_contains_switch_label(statement, target)
            }
        })
    }

    fn statement_contains_switch_label(statement: &Statement, target: Span) -> bool {
        match statement {
            Statement::Block(block) => Self::block_contains_switch_label(block, target),
            Statement::DoWhile { body, .. }
            | Statement::For { body, .. }
            | Statement::While { body, .. }
            | Statement::UserLabeled {
                statement: body, ..
            } => Self::statement_contains_switch_label(body, target),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                Self::statement_contains_switch_label(then_branch, target)
                    || else_branch.as_ref().is_some_and(|else_branch| {
                        Self::statement_contains_switch_label(else_branch, target)
                    })
            }
            Statement::Labeled {
                label, statement, ..
            } => {
                Self::switch_label_span(label) == target
                    || Self::statement_contains_switch_label(statement, target)
            }
            Statement::Switch { .. }
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(..)
            | Statement::Goto { .. }
            | Statement::Return(..) => false,
        }
    }

    fn enter_block_at_switch_label(
        &mut self,
        block: &Block,
        target: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let cache_key = (block as *const Block as usize, target);
        let index = if let Some(index) = self.switch_entry_index_cache.get(&cache_key) {
            *index
        } else {
            let index = block.items.iter().position(|item| match item {
                BlockItem::Declaration(_) | BlockItem::FunctionDeclaration(_) => false,
                BlockItem::Statement(statement) => {
                    Self::statement_contains_switch_label(statement, target)
                }
            });
            self.switch_entry_index_cache.insert(cache_key, index);
            index
        };
        let Some(mut index) = index else {
            return Ok(None);
        };
        self.push_scope_info(frame, block.span, true, true, true, objects);
        let result = (|| -> Result<Option<Flow>, Diagnostic> {
            self.prepare_block_control_entry(block, index, "switch", frame, objects)?;
            let mut pending_target = Some(target);
            loop {
                if index >= block.items.len() {
                    break Ok(Some(Flow::Continue));
                }
                let flow = match &block.items[index] {
                    BlockItem::Declaration(declaration) => {
                        self.exec_declaration(declaration, frame, objects)?;
                        Flow::Continue
                    }
                    BlockItem::FunctionDeclaration(declaration) => {
                        self.exec_function_declaration(declaration, frame);
                        Flow::Continue
                    }
                    BlockItem::Statement(statement) => {
                        if let Some(target) = pending_target.take() {
                            self.enter_statement_at_switch_label(statement, target, frame, objects)?
                                .unwrap_or(Flow::Continue)
                        } else {
                            self.exec_statement(statement, frame, objects)?
                        }
                    }
                };
                match flow {
                    Flow::Continue => index += 1,
                    flow => break Ok(Some(flow)),
                }
            }
        })();
        if let Some(scope) = self.pop_active_block_scope(frame.id, block.span) {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn enter_statement_at_switch_label(
        &mut self,
        statement: &Statement,
        target: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let implicit_scope_span = (matches!(
            statement,
            Statement::DoWhile { .. } | Statement::If { .. } | Statement::While { .. }
        ) && Self::statement_contains_switch_label(statement, target)
            && self
                .active_block_scope(frame.id, statement_span(statement))
                .is_none())
        .then(|| statement_span(statement));
        if let Some(span) = implicit_scope_span {
            self.push_scope_info(frame, span, false, false, true, objects);
        }
        let result =
            self.enter_statement_at_switch_label_in_scope(statement, target, frame, objects);
        if let Some(span) = implicit_scope_span
            && let Some(scope) = self.pop_active_block_scope(frame.id, span)
        {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn enter_statement_at_switch_label_in_scope(
        &mut self,
        statement: &Statement,
        target: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        let flow = match statement {
            Statement::Block(block) => {
                return self.enter_block_at_switch_label(block, target, frame, objects);
            }
            Statement::DoWhile {
                body,
                condition,
                span,
            } if Self::statement_contains_switch_label(body, target) => {
                let mut flow = self
                    .enter_controlled_substatement_at_switch_label(body, target, frame, objects)?
                    .unwrap_or(Flow::Continue);
                loop {
                    match flow {
                        Flow::Continue | Flow::LoopContinue => {}
                        Flow::LoopBreak => break Flow::Continue,
                        other => break other,
                    }
                    let condition_value = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::DoWhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&condition_value, condition.span())? {
                        break Flow::Continue;
                    }
                    flow = self.exec_controlled_substatement(body, frame, objects)?;
                }
            }
            Statement::For {
                init,
                condition,
                step,
                body,
                span,
            } if Self::statement_contains_switch_label(body, target) => {
                self.push_scope_info(frame, *span, true, true, true, objects);
                let result = (|| -> Result<Flow, Diagnostic> {
                    if let Some(ForInit::Declarations(declarations)) = init {
                        for declaration in declarations {
                            if declaration.vla_bounds.iter().any(Option::is_some) {
                                return Err(Diagnostic::error(
                                    "switch enters the scope of an object with variably modified type",
                                    declaration.span,
                                ));
                            }
                            if declaration.storage_class == Some(StorageClass::Static) {
                                self.exec_declaration(declaration, frame, objects)?;
                            } else {
                                let mut skipped = declaration.clone();
                                skipped.init = None;
                                self.exec_declaration(&skipped, frame, objects)?;
                            }
                        }
                    }
                    let mut flow = self
                        .enter_controlled_substatement_at_switch_label(
                            body, target, frame, objects,
                        )?
                        .unwrap_or(Flow::Continue);
                    loop {
                        match flow {
                            Flow::Continue | Flow::LoopContinue => {}
                            Flow::LoopBreak => break Ok(Flow::Continue),
                            other => break Ok(other),
                        }
                        if let Some(step) = step {
                            self.begin_full_expression_for(step);
                            let _ = self.eval_rvalue(step, frame, objects)?;
                            self.end_full_expression();
                        }
                        if let Some(condition) = condition {
                            let condition_value = self.eval_in_setjmp_context(
                                condition,
                                SetjmpContextKind::ForCondition,
                                *span,
                                frame,
                                objects,
                            )?;
                            if !self.scalar_truthy(&condition_value, condition.span())? {
                                break Ok(Flow::Continue);
                            }
                        }
                        flow = self.exec_controlled_substatement(body, frame, objects)?;
                    }
                })();
                if let Some(scope) = self.pop_active_block_scope(frame.id, *span) {
                    self.restore_block_scope(scope, frame, objects);
                }
                result?
            }
            Statement::If { then_branch, .. }
                if Self::statement_contains_switch_label(then_branch, target) =>
            {
                self.enter_controlled_substatement_at_switch_label(
                    then_branch,
                    target,
                    frame,
                    objects,
                )?
                .unwrap_or(Flow::Continue)
            }
            Statement::If {
                else_branch: Some(else_branch),
                ..
            } if Self::statement_contains_switch_label(else_branch, target) => self
                .enter_controlled_substatement_at_switch_label(else_branch, target, frame, objects)?
                .unwrap_or(Flow::Continue),
            Statement::Labeled {
                label, statement, ..
            } if Self::switch_label_span(label) == target => {
                self.exec_statement(statement, frame, objects)?
            }
            Statement::Labeled { statement, .. } | Statement::UserLabeled { statement, .. } => {
                return self.enter_statement_at_switch_label(statement, target, frame, objects);
            }
            Statement::While {
                condition,
                body,
                span,
            } if Self::statement_contains_switch_label(body, target) => {
                let mut flow = self
                    .enter_controlled_substatement_at_switch_label(body, target, frame, objects)?
                    .unwrap_or(Flow::Continue);
                loop {
                    match flow {
                        Flow::Continue | Flow::LoopContinue => {}
                        Flow::LoopBreak => break Flow::Continue,
                        other => break other,
                    }
                    let condition_value = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::WhileCondition,
                        *span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&condition_value, condition.span())? {
                        break Flow::Continue;
                    }
                    flow = self.exec_controlled_substatement(body, frame, objects)?;
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(flow))
    }

    fn enter_controlled_substatement_at_switch_label(
        &mut self,
        statement: &Statement,
        target: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Option<Flow>, Diagnostic> {
        if Self::statement_has_own_block_scope(statement) {
            return self.enter_statement_at_switch_label(statement, target, frame, objects);
        }
        let span = statement_span(statement);
        let scope_was_pushed = self.active_block_scope(frame.id, span).is_none();
        if scope_was_pushed {
            self.push_scope_info(frame, span, false, false, true, objects);
        }
        let result = self.enter_statement_at_switch_label(statement, target, frame, objects);
        if scope_was_pushed && let Some(scope) = self.pop_active_block_scope(frame.id, span) {
            self.restore_block_scope(scope, frame, objects);
        }
        result
    }

    fn exec_for(
        &mut self,
        init: Option<&ForInit>,
        condition: Option<&Expr>,
        step: Option<&Expr>,
        body: &Statement,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<Flow, Diagnostic> {
        self.push_scope_info(frame, span, true, true, true, objects);
        let result = (|| -> Result<Flow, Diagnostic> {
            if let Some(init) = init {
                match init {
                    ForInit::Declarations(decls) => {
                        for decl in decls {
                            self.exec_declaration(decl, frame, objects)?;
                        }
                    }
                    ForInit::Expression(expr) => {
                        self.begin_full_expression_for(expr);
                        let value = self.eval_rvalue(expr, frame, objects);
                        self.end_full_expression();
                        let _ = value?;
                    }
                }
            }
            loop {
                if let Some(condition) = condition {
                    let cond = self.eval_in_setjmp_context(
                        condition,
                        SetjmpContextKind::ForCondition,
                        span,
                        frame,
                        objects,
                    )?;
                    if !self.scalar_truthy(&cond, condition.span())? {
                        break Ok(Flow::Continue);
                    }
                }
                let body_flow = match self.exec_controlled_substatement(body, frame, objects)? {
                    Flow::Goto(label, goto_span) if self.statement_contains_label(body, &label) => {
                        self.enter_statement_at_label(body, &label, frame, objects)?
                            .unwrap_or(Flow::Goto(label, goto_span))
                    }
                    flow => flow,
                };
                match body_flow {
                    Flow::Continue | Flow::LoopContinue => {}
                    Flow::LoopBreak => break Ok(Flow::Continue),
                    flow => break Ok(flow),
                }
                if let Some(step) = step {
                    self.begin_full_expression_for(step);
                    let value = self.eval_rvalue(step, frame, objects);
                    self.end_full_expression();
                    let _ = value?;
                }
            }
        })();
        if let Some(scope) = self.pop_active_block_scope(frame.id, span) {
            self.restore_block_scope(scope, frame, objects);
        }
        result
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

    fn eval_variable(
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
            // Dereferencing a pointer only produces an lvalue; it does not read or
            // write the complete pointed-to type. Raw allocated storage can therefore
            // designate a member that fits even when another member makes the enclosing
            // union larger than the allocation. The eventual lvalue access performs the
            // full byte-range check.
            Some(start)
        } else if let Some(start) = pointer.byte_offset_override {
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
            bit_field_width: None,
            restrict_source: value.restrict_source,
        }))
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
                let (member_path, member_ty, bit_field_width) =
                    self.resolve_member_access(&lvalue.ty, member, span)?;
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
                let (member_path, member_ty, _) =
                    self.resolve_member_access(&value.ty, member, span)?;
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
        let object = self.allocate_object_id(None);
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
            bit_field_width: None,
            restrict_source: None,
        }))
    }

    pub(super) fn eval_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        declared_callee_type: Option<&CType>,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<ValueCategory, Diagnostic> {
        if let Expr::Variable(name, _) = callee {
            let value = match name.as_str() {
                "va_start" => Some(self.eval_va_start(args, span, frame, objects)?),
                "va_end" => Some(self.eval_va_end(args, span, frame, objects)?),
                "va_copy" => Some(self.eval_va_copy(args, span, frame, objects)?),
                _ => None,
            };
            if let Some(value) = value {
                return Ok(ValueCategory::RValue(value));
            }
        }
        let call_snapshot = self.sequencing_snapshot();
        let mut callee_value = self.eval_rvalue(callee, frame, objects)?;
        if let Some(declared_callee_type) = declared_callee_type {
            callee_value.ty = match declared_callee_type.unqualified() {
                CType::Function(..) => CType::pointer_to(declared_callee_type.clone()),
                _ => declared_callee_type.clone(),
            };
        }
        self.reject_missing_return_value(&callee_value, span)?;
        let function_name = match callee_value.data {
            ValueData::Function(name) => name,
            ValueData::Pointer(pointer) if pointer.is_null() => {
                return Err(Diagnostic::ub(
                    "call through a null function pointer",
                    span,
                    Some("6.5.2.2"),
                ));
            }
            ValueData::Pointer(_) => {
                return Err(Diagnostic::ub(
                    "call through an opaque implementation-defined function pointer",
                    span,
                    Some("6.3.2.3p5"),
                ));
            }
            _ => return Err(Diagnostic::error("unsupported call target", span)),
        };
        let (_, params, is_variadic) = self.call_target_signature(&callee_value.ty, span)?;
        let has_prototype = !params.is_empty();
        let mut evaluated = Vec::new();
        for arg in args {
            evaluated.push(self.eval_rvalue(arg, frame, objects)?);
        }
        let operand_footprint = self.finish_sequenced_operand(call_snapshot.clone());
        self.sequence_point();
        let fixed_param_count = if params.len() == 1 && params[0] == CType::Void {
            0
        } else {
            params.len()
        };
        if has_prototype && !is_variadic && fixed_param_count != args.len() {
            return Err(Diagnostic::error(
                format!(
                    "function {} expected {} argument(s), got {}",
                    function_name,
                    fixed_param_count,
                    args.len()
                ),
                span,
            ));
        }
        if has_prototype && is_variadic && args.len() < fixed_param_count {
            return Err(Diagnostic::error(
                format!(
                    "function {} expected {}+ argument(s), got {}",
                    function_name,
                    fixed_param_count,
                    args.len()
                ),
                span,
            ));
        }
        if has_prototype && args.len() >= fixed_param_count {
            for ((param_ty, expr), value) in
                params.iter().zip(args.iter()).zip(evaluated.iter_mut())
            {
                if self.array_value_types_compatible(param_ty, &value.ty, true)
                    && matches!(param_ty.unqualified(), CType::Array(_, 0))
                {
                    continue;
                }
                *value = self.convert_value_in_context(
                    expr,
                    value.clone(),
                    param_ty,
                    expr.span(),
                    frame,
                    objects,
                )?;
            }
            if is_variadic {
                for (index, value) in evaluated.iter_mut().enumerate().skip(fixed_param_count) {
                    *value = self.default_argument_promotion_for_expr(
                        value.clone(),
                        &args[index],
                        frame,
                        objects,
                    )?;
                }
            }
        } else if !has_prototype {
            for (index, value) in evaluated.iter_mut().enumerate() {
                *value = self.default_argument_promotion_for_expr(
                    value.clone(),
                    &args[index],
                    frame,
                    objects,
                )?;
            }
        }

        if let Some(function) = self.lookup_function_symbol(&function_name).cloned() {
            let actual_params = function
                .params
                .iter()
                .map(|param| {
                    if function.has_prototype {
                        Ok(param.ty.clone())
                    } else if matches!(param.ty.unqualified(), CType::Float) {
                        Ok(CType::Double)
                    } else if param.ty.is_integer() {
                        self.promoted_integer_type(&param.ty, param.span)
                    } else {
                        Ok(param.ty.clone())
                    }
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let actual_type = if function.is_variadic {
                CType::variadic_function(function.return_type.clone(), actual_params)
            } else {
                CType::function(function.return_type.clone(), actual_params)
            };
            let called_type = callee_value.ty.element_type().unwrap_or(&callee_value.ty);
            if has_prototype && !self.cross_unit_tagged_type_compatible(called_type, &actual_type) {
                return Err(Diagnostic::ub(
                    format!(
                        "call through function pointer of type {} is incompatible with definition of {} with type {}",
                        called_type, function_name, actual_type
                    ),
                    span,
                    Some("6.5.2.2p9"),
                ));
            }
            if !has_prototype {
                self.validate_unprototyped_call_against_definition(
                    function.as_ref(),
                    &evaluated,
                    span,
                )?;
            }
            let caller_assignment_targets = self.assignment_targets.clone();
            let call_result = self.call_function(
                &function,
                self.function_may_setjmp_symbol(&function_name),
                evaluated,
                span,
                objects,
            );
            self.assignment_targets = caller_assignment_targets;
            let _ = self.finish_sequenced_operand(call_snapshot);
            self.merge_sequenced_footprint(operand_footprint);
            let value = call_result?;
            return Ok(ValueCategory::RValue(value));
        }

        if Self::is_host_library_function(&function_name)
            && self
                .lookup_function_declaration(&function_name, span.file)
                .is_some()
        {
            self.check_host_library_call(&function_name, args, &evaluated, span, frame, objects)?;
            let value =
                self.eval_host_library_call(&function_name, args, &evaluated, span, frame, objects);
            let _ = self.finish_sequenced_operand(call_snapshot);
            self.merge_sequenced_footprint(operand_footprint);
            let value = value?;
            return Ok(ValueCategory::RValue(value));
        }

        Err(Diagnostic::error(
            format!("call to undefined function {}", function_name),
            span,
        ))
    }

    fn validate_unprototyped_call_against_definition(
        &self,
        function: &FunctionDef,
        args: &[TypedValue],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let params = if function.params.len() == 1 && function.params[0].ty == CType::Void {
            &[][..]
        } else {
            function.params.as_slice()
        };
        if function.is_variadic || params.len() != args.len() {
            return Err(Diagnostic::ub(
                format!(
                    "call without a prototype supplies {} argument(s), but the definition of {} has {} parameter(s)",
                    args.len(),
                    function.name,
                    params.len()
                ),
                span,
                Some("6.5.2.2p6"),
            ));
        }
        for (param, arg) in params.iter().zip(args) {
            let comparison_ty = if !function.has_prototype {
                if matches!(param.ty.unqualified(), CType::Float) {
                    CType::Double
                } else if param.ty.is_integer() {
                    self.promoted_integer_type(&param.ty, param.span)?
                } else {
                    param.ty.clone()
                }
            } else {
                param.ty.clone()
            };
            if self.cross_unit_tagged_type_compatible(
                comparison_ty.unqualified(),
                arg.ty.unqualified(),
            ) || self.old_style_signedness_exception(&comparison_ty, arg)
                || Self::old_style_character_void_pointer_exception(&comparison_ty, &arg.ty)
            {
                continue;
            }
            return Err(Diagnostic::ub(
                format!(
                    "argument of promoted type {} is incompatible with parameter type {} in the definition of {}",
                    arg.ty, param.ty, function.name
                ),
                span,
                Some("6.5.2.2p6"),
            ));
        }
        Ok(())
    }

    fn old_style_signedness_exception(&self, param_ty: &CType, arg: &TypedValue) -> bool {
        if !Self::corresponding_signed_unsigned_types(param_ty, &arg.ty) {
            return false;
        }
        let Ok(value) = arg.to_int() else {
            return false;
        };
        param_ty
            .integer_bounds()
            .is_some_and(|(min, max)| (min..=max).contains(&value))
    }

    fn old_style_character_void_pointer_exception(lhs: &CType, rhs: &CType) -> bool {
        let (CType::Pointer(lhs), CType::Pointer(rhs)) = (lhs.unqualified(), rhs.unqualified())
        else {
            return false;
        };
        let is_character_or_void = |ty: &CType| {
            let ty = ty.unqualified();
            ty.is_character() || matches!(ty, CType::Void)
        };
        is_character_or_void(lhs) && is_character_or_void(rhs)
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
            bit_field_width: None,
            restrict_source: None,
        }))
    }

    pub(super) fn va_list_value(&self, handle: u64) -> TypedValue {
        TypedValue::from_data(
            CType::VaList,
            ValueData::Int(CIntValue::new(handle as i128)),
        )
    }

    fn eval_va_start(
        &mut self,
        args: &[Expr],
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::error(
                "va_start requires exactly two arguments",
                span,
            ));
        }
        let current_function = self
            .current_functions
            .last()
            .ok_or_else(|| Diagnostic::error("va_start is only valid inside a function", span))?;
        if !current_function.is_variadic {
            return Err(Diagnostic::error(
                "va_start requires a variadic function",
                span,
            ));
        }
        let last_named = current_function
            .params
            .last()
            .ok_or_else(|| Diagnostic::error("va_start requires a named last parameter", span))?;
        if !matches!(&args[1], Expr::Variable(name, _) if Some(name) == last_named.name.as_ref()) {
            return Err(Diagnostic::error(
                "second argument to va_start must be the last named parameter",
                args[1].span(),
            ));
        }
        let last_parameter = last_named;
        if last_parameter.storage_class == Some(StorageClass::Register) {
            return Err(Diagnostic::ub(
                "va_start used with a register-qualified last named parameter",
                args[1].span(),
                Some("7.16.1.4"),
            ));
        }
        if last_parameter.adjusted_from_array_or_function {
            return Err(Diagnostic::ub(
                "va_start used with a last named parameter declared with array or function type",
                args[1].span(),
                Some("7.16.1.4"),
            ));
        }
        if matches!(
            last_parameter.ty.unqualified(),
            CType::Bool
                | CType::Char
                | CType::SignedChar
                | CType::UnsignedChar
                | CType::Short
                | CType::UnsignedShort
                | CType::Float
        ) {
            return Err(Diagnostic::ub(
                "va_start used with a last named parameter whose type changes under the default argument promotions",
                args[1].span(),
                Some("7.16.1.4"),
            ));
        }
        let ap = match self.eval(&args[0], frame, objects)? {
            ValueCategory::LValue(lvalue) => lvalue,
            ValueCategory::RValue(_) => {
                return Err(Diagnostic::error(
                    "first argument to va_start must be a va_list lvalue",
                    args[0].span(),
                ));
            }
        };
        if ap.ty != CType::VaList {
            return Err(Diagnostic::error(
                "first argument to va_start must have type va_list",
                args[0].span(),
            ));
        }
        if self
            .va_list_lvalue_handle(&ap, objects, args[0].span())?
            .is_some_and(|handle| self.va_lists.contains_key(&handle))
        {
            return Err(Diagnostic::ub(
                "va_start used to reinitialize an active va_list without an intervening va_end",
                args[0].span(),
                Some("7.16.1.4"),
            ));
        }
        let handle = self.next_va_list_handle;
        self.next_va_list_handle += 1;
        self.va_lists.insert(
            handle,
            VaListCursor {
                args: self
                    .current_variadic_args
                    .last()
                    .cloned()
                    .unwrap_or_default(),
                index: 0,
                owner_frame_id: self.current_frame_id().unwrap_or(0),
            },
        );
        self.sequence_point();
        self.store_lvalue(objects, &ap, self.va_list_value(handle), args[0].span())?;
        self.sequence_point();
        Ok(TypedValue::void())
    }

    fn va_list_lvalue_handle(
        &self,
        lvalue: &LValue,
        objects: &ObjectFrames,
        span: Span,
    ) -> Result<Option<u64>, Diagnostic> {
        let object = self.lookup_object(objects, lvalue.object).ok_or_else(|| {
            Diagnostic::ub(
                "access through a pointer to an object whose lifetime has ended",
                span,
                Some("6.2.4"),
            )
        })?;
        let mut stored = object.value.clone();
        let slot = self.resolve_lvalue_storage_mut(&mut stored, &object.ty, lvalue, span)?;
        let StoredValue::Scalar(value) = slot else {
            return Ok(None);
        };
        if value.indeterminate {
            return Ok(None);
        }
        let ValueData::Int(handle) = value.data else {
            return Ok(None);
        };
        Ok(u64::try_from(handle.get())
            .ok()
            .filter(|handle| *handle != 0))
    }

    fn reject_unended_va_lists(&self, frame_id: usize, span: Span) -> Result<(), Diagnostic> {
        if self
            .va_lists
            .values()
            .any(|cursor| cursor.owner_frame_id == frame_id)
        {
            return Err(Diagnostic::ub(
                "function returned without a matching va_end for every va_start or va_copy",
                span,
                Some("7.16.1"),
            ));
        }
        Ok(())
    }

    fn eval_va_end(
        &mut self,
        args: &[Expr],
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::error(
                "va_end requires exactly one argument",
                span,
            ));
        }
        let ap = match self.eval(&args[0], frame, objects)? {
            ValueCategory::LValue(lvalue) => lvalue,
            ValueCategory::RValue(_) => {
                return Err(Diagnostic::error(
                    "argument to va_end must be a va_list lvalue",
                    args[0].span(),
                ));
            }
        };
        if ap.ty != CType::VaList {
            return Err(Diagnostic::error(
                "argument to va_end must have type va_list",
                args[0].span(),
            ));
        }
        let value = self.load_lvalue(ap.clone(), args[0].span(), objects)?;
        let handle = u64::try_from(value.to_int()?).unwrap_or(0);
        self.sequence_point();
        let current_frame = self.current_frame_id().unwrap_or(0);
        if handle == 0 || !self.va_lists.contains_key(&handle) {
            return Err(Diagnostic::ub(
                "va_end used with a va_list that is not active",
                args[0].span(),
                Some("7.16.1.3"),
            ));
        }
        if self
            .va_lists
            .get(&handle)
            .is_some_and(|cursor| cursor.owner_frame_id != current_frame)
        {
            return Err(Diagnostic::ub(
                "va_end must be invoked in the same function as the matching va_start or va_copy",
                args[0].span(),
                Some("7.16.1.3"),
            ));
        }
        self.va_lists.remove(&handle);
        self.store_lvalue(objects, &ap, self.va_list_value(0), args[0].span())?;
        self.sequence_point();
        Ok(TypedValue::void())
    }

    fn eval_va_copy(
        &mut self,
        args: &[Expr],
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::error(
                "va_copy requires exactly two arguments",
                span,
            ));
        }
        let dest = match self.eval(&args[0], frame, objects)? {
            ValueCategory::LValue(lvalue) => lvalue,
            ValueCategory::RValue(_) => {
                return Err(Diagnostic::error(
                    "first argument to va_copy must be a va_list lvalue",
                    args[0].span(),
                ));
            }
        };
        if dest.ty != CType::VaList {
            return Err(Diagnostic::error(
                "first argument to va_copy must have type va_list",
                args[0].span(),
            ));
        }
        if self
            .va_list_lvalue_handle(&dest, objects, args[0].span())?
            .is_some_and(|handle| self.va_lists.contains_key(&handle))
        {
            return Err(Diagnostic::ub(
                "va_copy used to overwrite an active destination va_list without an intervening va_end",
                args[0].span(),
                Some("7.16.1.2"),
            ));
        }
        let src = self.eval_rvalue(&args[1], frame, objects)?;
        if src.ty != CType::VaList {
            return Err(Diagnostic::error(
                "second argument to va_copy must have type va_list",
                args[1].span(),
            ));
        }
        let src_handle = u64::try_from(src.to_int()?).unwrap_or(0);
        let new_handle = if src_handle == 0 {
            return Err(Diagnostic::ub(
                "va_copy used with an uninitialized or ended source va_list",
                args[1].span(),
                Some("7.16.1.2"),
            ));
        } else {
            let state = self.va_lists.get(&src_handle).cloned().ok_or_else(|| {
                Diagnostic::ub(
                    "va_copy used with an invalid va_list",
                    args[1].span(),
                    Some("7.15.1.2"),
                )
            })?;
            let handle = self.next_va_list_handle;
            self.next_va_list_handle += 1;
            self.va_lists.insert(
                handle,
                VaListCursor {
                    owner_frame_id: self.current_frame_id().unwrap_or(0),
                    ..state
                },
            );
            handle
        };
        self.sequence_point();
        self.store_lvalue(
            objects,
            &dest,
            self.va_list_value(new_handle),
            args[0].span(),
        )?;
        self.sequence_point();
        Ok(TypedValue::void())
    }

    fn eval_va_arg(
        &mut self,
        ap: &Expr,
        ty: &CType,
        span: Span,
        frame: &mut Frame,
        objects: &mut ObjectFrames,
    ) -> Result<TypedValue, Diagnostic> {
        let ap_lvalue = match self.eval(ap, frame, objects)? {
            ValueCategory::LValue(lvalue) => lvalue,
            ValueCategory::RValue(_) => {
                return Err(Diagnostic::error(
                    "va_arg requires a va_list lvalue",
                    ap.span(),
                ));
            }
        };
        if ap_lvalue.ty != CType::VaList {
            return Err(Diagnostic::error(
                "va_arg requires a va_list lvalue",
                ap.span(),
            ));
        }
        if matches!(ty.unqualified(), CType::Void | CType::Function(..))
            || !self.type_is_complete(ty)
        {
            return Err(Diagnostic::error(
                "va_arg requires a complete object type",
                span,
            ));
        }
        let list_value = self.load_lvalue(ap_lvalue.clone(), ap.span(), objects)?;
        let handle = u64::try_from(list_value.to_int()?).unwrap_or(0);
        if handle == 0 {
            return Err(Diagnostic::ub(
                "va_arg used with an uninitialized or ended va_list",
                span,
                Some("7.15.1.1"),
            ));
        }
        let cursor = self.va_lists.get_mut(&handle).ok_or_else(|| {
            Diagnostic::ub(
                "va_arg used with an invalid va_list",
                span,
                Some("7.15.1.1"),
            )
        })?;
        if cursor.index >= cursor.args.len() {
            return Err(Diagnostic::ub(
                "va_arg advanced past the end of the variadic arguments",
                span,
                Some("7.15.1.1"),
            ));
        }
        let value = cursor.args[cursor.index].clone();
        cursor.index += 1;
        if !self.va_arg_type_compatible(&value, ty) {
            return Err(Diagnostic::ub(
                format!(
                    "va_arg requested {} but the next variadic argument has type {}",
                    ty, value.ty
                ),
                span,
                Some("7.15.1.1"),
            ));
        }
        self.convert_value(value, ty, span)
    }

    fn va_arg_type_compatible(&self, value: &TypedValue, requested: &CType) -> bool {
        let actual = &value.ty;
        if self.cross_unit_tagged_type_compatible(actual, requested) {
            return true;
        }
        if actual.top_level_qualifiers() != requested.top_level_qualifiers() {
            return false;
        }
        let actual = actual.unqualified();
        let requested = requested.unqualified();
        if actual.is_integer()
            && requested.is_integer()
            && actual.integer_rank() == requested.integer_rank()
            && actual.is_signed_integer() != requested.is_signed_integer()
        {
            let Ok(value) = value.to_int() else {
                return false;
            };
            return actual
                .integer_bounds()
                .zip(requested.integer_bounds())
                .is_some_and(
                    |((actual_min, actual_max), (requested_min, requested_max))| {
                        (actual_min..=actual_max).contains(&value)
                            && (requested_min..=requested_max).contains(&value)
                    },
                );
        }
        match (actual, requested) {
            (CType::Pointer(actual), CType::Pointer(requested)) => {
                (matches!(actual.unqualified(), CType::Void)
                    && requested.unqualified().is_character())
                    || (actual.unqualified().is_character()
                        && matches!(requested.unqualified(), CType::Void))
            }
            _ => false,
        }
    }
}
