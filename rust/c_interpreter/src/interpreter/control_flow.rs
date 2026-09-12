//! Blocks, loops, switch dispatch, goto, and longjmp resumption.

use super::*;

impl<'a> Interpreter<'a> {
    pub(super) fn exec_block(
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
            if decl.storage_class == Some(StorageClass::Static)
                && let Some(initializer) = decl.init.as_ref()
            {
                self.validate_static_initializer(initializer, frame, objects)?;
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

    pub(super) fn resume_function_after_longjmp(
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
}
