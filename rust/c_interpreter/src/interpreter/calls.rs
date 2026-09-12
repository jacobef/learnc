//! Function calls, parameter binding, and variadic argument lifetimes.

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
                    // Array-to-pointer adjustment removes only the outer array
                    // dimension. Evaluate that bound for its side effects, then
                    // retain the remaining dimensions in the parameter object.
                    let (_, retained) =
                        Self::resolve_vla_type_for_constraints(&param.ty, &param.vla_bounds);
                    let discarded = param.vla_bounds.len() - retained;
                    for bound in param.vla_bounds[..discarded].iter().flatten() {
                        let _ = self.evaluate_vla_bound(bound, &mut frame, objects)?;
                    }
                    let mut resolved_param = param.clone();
                    resolved_param.ty = self.resolve_decl_type(
                        &param.ty,
                        &param.vla_bounds[discarded..],
                        &mut frame,
                        objects,
                        param.span,
                    )?;
                    self.check_static_array_parameter(&resolved_param, &arg, &mut frame, objects)?;
                    let arg = self.convert_value(arg, &resolved_param.ty, call_span)?;
                    let object = self.allocate_object(
                        objects,
                        resolved_param.ty,
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

    pub(super) fn resolve_decl_type_impl(
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

    pub(super) fn function_designator_value(&self, function: &FunctionDef) -> TypedValue {
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
            ) || self.corresponding_signedness_value_exception(&comparison_ty, arg)
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

    pub(super) fn corresponding_signedness_value_exception(
        &self,
        expected_ty: &CType,
        actual: &TypedValue,
    ) -> bool {
        if !Self::corresponding_signed_unsigned_types(expected_ty, &actual.ty) {
            return false;
        }
        let Ok(value) = actual.to_int() else {
            return false;
        };
        expected_ty
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

    pub(super) fn eval_va_arg(
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
