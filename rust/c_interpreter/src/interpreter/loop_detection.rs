//! Cycle detection over the bounded memory that can influence a loop's exits.
//!
//! Tracking only the condition is unsound: assignments, helper functions, input,
//! and branch conditions can make it change later. Close those dependencies before
//! comparing exact states, declining operations whose effects are not modeled.

use super::loop_state::{LoopPathPart, LoopReadPaths, LoopSnapshot, LoopValue, loop_read_path};
use super::*;
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct LoopAnalysis {
    controls: HashSet<String>,
    read_paths: HashMap<String, LoopReadPaths>,
    assignments: HashMap<String, HashSet<String>>,
    locals: HashSet<String>,
    initialized_locals: HashSet<String>,
    static_locals: HashSet<Span>,
    pub(super) calls: Vec<LoopCall>,
    reads: HashMap<String, Span>,
    pub(super) modified: HashSet<String>,
    pub(super) indirect_writes: bool,
}

impl LoopAnalysis {
    pub(super) fn expression(&mut self, expr: &Expr) -> Option<HashSet<String>> {
        let mut reads = HashSet::default();
        match expr {
            Expr::Variable(name, _) => {
                reads.insert(name.clone());
                self.reads.insert(name.clone(), expr.span());
                self.read_paths.insert(name.clone(), None);
            }
            Expr::Number(..)
            | Expr::CharLiteral(..)
            | Expr::WideCharLiteral(..)
            | Expr::Utf16CharLiteral(..)
            | Expr::Utf32CharLiteral(..)
            | Expr::StringLiteral(..) => {}
            Expr::Unary { op, expr, .. } => {
                reads = self.expression(expr)?;
                if matches!(op, UnaryOp::PreIncrement | UnaryOp::PreDecrement) {
                    self.modification(expr, &reads);
                }
            }
            Expr::Postfix { expr, .. } => {
                reads = self.expression(expr)?;
                self.modification(expr, &reads);
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                reads = self.expression(lhs)?;
                if matches!(op, BinaryOp::LogicalAnd | BinaryOp::LogicalOr) {
                    self.controls.extend(reads.iter().cloned());
                }
                reads.extend(self.expression(rhs)?);
            }
            Expr::Subscript { .. } | Expr::Member { .. } if loop_read_path(expr).is_some() => {
                let (name, path) = loop_read_path(expr)?;
                reads.insert(name.to_owned());
                self.reads.insert(name.to_owned(), expr.span());
                if let Some(paths) = self
                    .read_paths
                    .entry(name.to_owned())
                    .or_insert_with(|| Some(HashSet::default()))
                {
                    paths.insert(path);
                }
            }
            Expr::Subscript { base, index, .. } => {
                reads = self.expression(base)?;
                reads.extend(self.expression(index)?);
            }
            Expr::Member { base, .. } | Expr::SizeofExpr { expr: base, .. } => {
                reads = self.expression(base)?;
            }
            Expr::Assign { lhs, rhs, .. } | Expr::CompoundAssign { lhs, rhs, .. } => {
                reads = self.expression(rhs)?;
                if let Expr::Variable(name, _) = lhs.as_ref() {
                    self.modified.insert(name.clone());
                    self.read_paths.insert(name.clone(), None);
                    self.assignments
                        .entry(name.clone())
                        .or_default()
                        .extend(reads.iter().cloned());
                    reads.insert(name.clone());
                } else {
                    reads.extend(self.expression(lhs)?);
                    self.modification(lhs, &reads);
                }
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let condition_reads = self.expression(condition)?;
                self.controls.extend(condition_reads.iter().cloned());
                reads = condition_reads;
                reads.extend(self.expression(then_expr)?);
                reads.extend(self.expression(else_expr)?);
            }
            Expr::Call {
                callee, args, span, ..
            } => {
                reads = self.call(callee, args, *span, true)?;
            }
            Expr::Cast {
                expr, vla_bounds, ..
            } if vla_bounds.iter().all(Option::is_none) => {
                reads = self.expression(expr)?;
            }
            Expr::SizeofType { vla_bounds, .. } if vla_bounds.iter().all(Option::is_none) => {}
            _ => return None,
        }
        Some(reads)
    }

    fn modification(&mut self, target: &Expr, reads: &HashSet<String>) {
        if let Expr::Variable(name, _) = target {
            self.modified.insert(name.clone());
        } else {
            // An indirect store may alias any exit dependency. Include its address
            // and value inputs so a delayed update cannot look like a stable loop.
            self.indirect_writes = true;
            self.controls.extend(reads.iter().cloned());
        }
    }

    fn control(&mut self, expr: &Expr) -> Option<()> {
        let reads = self.expression(expr)?;
        self.controls.extend(reads);
        Some(())
    }

    fn call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        span: Span,
        result_used: bool,
    ) -> Option<HashSet<String>> {
        let Expr::Variable(name, _) = callee else {
            return None;
        };
        let output_only = match name.as_str() {
            "printf" => {
                let Some(Expr::StringLiteral(format, _)) = args.first() else {
                    return None;
                };
                let mut conversion = false;
                for byte in format
                    .narrow_bytes()
                    .into_iter()
                    .take_while(|byte| *byte != 0)
                {
                    if !conversion {
                        conversion = byte == b'%';
                    } else if b"diouxXfFeEgGaAcspn%".contains(&byte) {
                        if byte == b'n' {
                            return None;
                        }
                        conversion = false;
                    }
                }
                if conversion {
                    return None;
                }
                true
            }
            "puts" | "putchar" => true,
            _ => false,
        };
        let stream_argument = match name.as_str() {
            "feof" | "ferror" | "fgetc" | "getc" | "fscanf" => Some(0),
            "fgets" => Some(2),
            "fread" => Some(3),
            _ => None,
        };
        let uses_stdin_argument = stream_argument.is_some_and(|index| {
            matches!(args.get(index), Some(Expr::Variable(name, _)) if name == "__cboxes_stdin")
        });
        let mut reads = HashSet::default();
        let mut argument_reads = HashSet::default();
        let first_output = match name.as_str() {
            "scanf" => Some(1),
            "sscanf" | "fscanf" => Some(2),
            _ => None,
        };
        for (index, arg) in args.iter().enumerate() {
            if first_output.is_some_and(|first| index >= first)
                && let Expr::Unary {
                    op: UnaryOp::AddressOf,
                    expr,
                    ..
                } = arg
                && let Expr::Variable(target, target_span) = expr.as_ref()
            {
                // A scanf destination's previous value is not an input. In particular,
                // a failed conversion can repeat forever with an uninitialized destination.
                self.modified.insert(target.clone());
                self.reads.insert(target.clone(), *target_span);
                argument_reads.insert(target.clone());
            } else if ((first_output.is_some_and(|first| index >= first))
                || (index == 0 && matches!(name.as_str(), "fgets" | "fread")))
                && let Expr::Variable(target, target_span) = arg
            {
                // Preserve the destination address; other expressions that read
                // this buffer independently add their own required value paths.
                self.reads.insert(target.clone(), *target_span);
                self.read_paths
                    .entry(target.clone())
                    .or_insert_with(|| Some(HashSet::default()));
                reads.insert(target.clone());
                argument_reads.insert(target.clone());
            } else {
                let output_variable = match arg {
                    Expr::Variable(name, _) if output_only && !result_used => Some(name),
                    _ => None,
                };
                let previous_paths =
                    output_variable.and_then(|name| self.read_paths.get(name).cloned());
                let argument = self.expression(arg)?;
                if let Some(name) = output_variable {
                    // Merely printing a buffer cannot affect this loop's exits.
                    // A user-defined function with this name restores full argument
                    // dependencies when the call is resolved below.
                    if let Some(paths) = previous_paths {
                        self.read_paths.insert(name.clone(), paths);
                    } else {
                        self.read_paths.remove(name);
                    }
                }
                argument_reads.extend(argument.iter().cloned());
                reads.extend(argument);
            }
        }
        self.calls.push(LoopCall {
            name: name.clone(),
            span,
            output_only,
            argument_reads,
            uses_stdin_argument,
            reads_stdin: uses_stdin_argument || matches!(name.as_str(), "scanf" | "getchar"),
            modeled_library: uses_stdin_argument
                || matches!(
                    name.as_str(),
                    "strlen"
                        | "strcmp"
                        | "strncmp"
                        | "strchr"
                        | "strrchr"
                        | "strstr"
                        | "memcmp"
                        | "memchr"
                        | "abs"
                        | "labs"
                        | "llabs"
                        | "sscanf"
                        | "scanf"
                        | "getchar"
                ),
        });
        if result_used || !output_only {
            self.controls.extend(reads.iter().cloned());
        }
        Some(reads)
    }

    pub(super) fn discarded_expression(&mut self, expr: &Expr) -> Option<()> {
        if let Expr::Call {
            callee, args, span, ..
        } = expr
        {
            self.call(callee, args, *span, false)?;
        } else {
            self.expression(expr)?;
        }
        Some(())
    }

    fn declaration(&mut self, decl: &Declaration) -> Option<()> {
        if matches!(decl.storage_class, Some(StorageClass::Extern))
            || decl.vla_bounds.iter().any(Option::is_some)
            || decl.ty.is_volatile_qualified()
            || decl.ty.top_level_qualifiers().is_atomic
        {
            return None;
        }
        if !self.locals.insert(decl.name.clone()) {
            return None;
        }
        if decl.storage_class == Some(StorageClass::Static) {
            self.static_locals.insert(decl.span);
        }
        if let Some(init) = &decl.init {
            self.initialized_locals.insert(decl.name.clone());
            let Initializer::Expr(expr) = init else {
                return None;
            };
            let reads = self.expression(expr)?;
            self.assignments
                .entry(decl.name.clone())
                .or_default()
                .extend(reads);
        }
        Some(())
    }

    fn block(&mut self, block: &Block) -> Option<()> {
        for item in &block.items {
            match item {
                BlockItem::Statement(statement) => self.statement(statement)?,
                BlockItem::Declaration(decl) => self.declaration(decl)?,
                BlockItem::FunctionDeclaration(_) => return None,
            }
        }
        Some(())
    }

    pub(super) fn statement(&mut self, statement: &Statement) -> Option<()> {
        match statement {
            Statement::Block(block) => self.block(block)?,
            Statement::Expression(Some(expr), _) => self.discarded_expression(expr)?,
            Statement::Expression(None, _) | Statement::Break(_) | Statement::Continue(_) => {}
            Statement::Return(expr, _) => {
                if let Some(expr) = expr {
                    self.expression(expr)?;
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.control(condition)?;
                self.statement(then_branch)?;
                if let Some(branch) = else_branch {
                    self.statement(branch)?;
                }
            }
            Statement::While {
                condition, body, ..
            }
            | Statement::DoWhile {
                condition, body, ..
            } => {
                self.control(condition)?;
                self.statement(body)?;
            }
            Statement::Switch { expr, body, .. } => {
                self.control(expr)?;
                self.block(body)?;
            }
            Statement::Labeled { statement, .. } => self.statement(statement)?,
            Statement::For {
                init,
                condition,
                step,
                body,
                ..
            } => {
                match init {
                    Some(ForInit::Declarations(decls)) => {
                        for decl in decls {
                            self.declaration(decl)?;
                        }
                    }
                    Some(ForInit::Expression(expr)) => {
                        self.expression(expr)?;
                    }
                    None => {}
                }
                if let Some(condition) = condition {
                    self.control(condition)?;
                }
                if let Some(step) = step {
                    self.expression(step)?;
                }
                self.statement(body)?;
            }
            // Arbitrary jumps need a control-flow analysis rather than this AST walk.
            _ => return None,
        }
        Some(())
    }
}

pub(super) struct LoopCall {
    pub name: String,
    pub span: Span,
    pub output_only: bool,
    pub modeled_library: bool,
    argument_reads: HashSet<String>,
    uses_stdin_argument: bool,
    reads_stdin: bool,
}

enum LoopVariable {
    Object(ObjectId, Option<Vec<Vec<LoopPathPart>>>),
    Static(Span),
}

pub(super) struct LoopDetector {
    variables: Vec<LoopVariable>,
    seen: VecDeque<Vec<LoopValue>>,
    checkpoint: Option<Vec<LoopValue>>,
    checkpoint_period: usize,
    checkpoint_distance: usize,
    trace_start: usize,
    detected_at_step: Option<usize>,
    progress: Option<super::loop_progress::LoopProgress>,
    reason: &'static str,
    iterations: usize,
    reads_stdin: bool,
}

pub(super) struct DetectedLoop {
    pub trace_position: usize,
    pub reason: &'static str,
}

impl<'a> Interpreter<'a> {
    pub(super) fn loop_detector(
        &self,
        condition: Option<&Expr>,
        step: Option<&Expr>,
        body: &Statement,
        span: Span,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Option<LoopDetector> {
        self.execution_steps_remaining?;
        let mut analysis = LoopAnalysis::default();
        if let Some(condition) = condition {
            analysis.control(condition)?;
        }
        if let Some(step) = step {
            analysis.expression(step)?;
        }
        analysis.statement(body)?;
        let mut external_objects = HashSet::default();
        let mut static_objects = analysis.static_locals.clone();
        let mut reads_stdin = false;
        for call in &analysis.calls {
            if self.lookup_function(&call.name, call.span.file).is_some() {
                analysis
                    .controls
                    .extend(call.argument_reads.iter().cloned());
                for name in &call.argument_reads {
                    analysis.read_paths.insert(name.clone(), None);
                }
            }
            if frame.bindings.contains_key(&call.name)
                || (call.uses_stdin_argument && frame.bindings.contains_key("__cboxes_stdin"))
            {
                return None;
            }
            self.loop_call_objects(
                call,
                &mut external_objects,
                &mut static_objects,
                &mut reads_stdin,
                &mut HashSet::default(),
                0,
            )?;
        }
        loop {
            let before = analysis.controls.len();
            for name in analysis.controls.clone() {
                if let Some(reads) = analysis.assignments.get(&name) {
                    analysis.controls.extend(reads.iter().cloned());
                }
            }
            if analysis.controls.len() == before {
                break;
            }
        }
        if analysis.controls.len() > 32 {
            return None;
        }
        // A freshly initialized automatic local is a function of its initializer's
        // dependencies, already closed above. It need not exist at the loop boundary.
        // Reject shadowing: the same spelling could otherwise denote two objects.
        for name in &analysis.locals {
            if frame.bindings.contains_key(name)
                || self.lookup_global_binding(name, span.file).is_some()
                || (analysis.controls.contains(name) && !analysis.initialized_locals.contains(name))
            {
                return None;
            }
        }
        analysis
            .controls
            .retain(|name| !analysis.locals.contains(name));
        let mut variables = external_objects
            .into_iter()
            .map(|id| (id, None))
            .collect::<HashMap<ObjectId, LoopReadPaths>>();
        for name in analysis.controls {
            let Some(id) = frame
                .bindings
                .get(&name)
                .copied()
                .or_else(|| self.lookup_global_binding(&name, span.file))
            else {
                if self.program.enum_constants.contains_key(&(span.file, name)) {
                    continue;
                }
                return None;
            };
            let object = self.lookup_object(objects, id)?;
            if object.ty.is_volatile_qualified() || object.ty.top_level_qualifiers().is_atomic {
                return None;
            }
            let paths = analysis.read_paths.remove(&name).flatten();
            variables
                .entry(id)
                .and_modify(|existing| match (existing.as_mut(), paths.as_ref()) {
                    (Some(existing), Some(paths)) => existing.extend(paths.iter().cloned()),
                    _ => *existing = None,
                })
                .or_insert(paths);
        }
        for id in variables.keys() {
            let object = self.lookup_object(objects, *id)?;
            if object.ty.is_volatile_qualified() || object.ty.top_level_qualifiers().is_atomic {
                return None;
            }
        }
        let mut variables = variables
            .into_iter()
            .map(|(id, paths)| {
                let paths = paths.map(|paths| {
                    let mut paths = paths.into_iter().collect::<Vec<_>>();
                    paths.sort();
                    paths
                });
                LoopVariable::Object(id, paths)
            })
            .collect::<Vec<_>>();
        variables.sort_by_key(|variable| match variable {
            LoopVariable::Object(id, _) => id.0,
            _ => unreachable!(),
        });
        variables.extend(static_objects.into_iter().map(LoopVariable::Static));
        if variables.len() > 32 {
            return None;
        }
        Some(LoopDetector {
            variables,
            seen: VecDeque::new(),
            checkpoint: None,
            checkpoint_period: 1,
            checkpoint_distance: 0,
            trace_start: self.cboxes_trace.len(),
            detected_at_step: None,
            progress: condition
                .and_then(|condition| {
                    self.loop_progress(condition, step, body, frame, objects, &analysis.modified)
                })
                .or_else(|| {
                    self.loop_trailing_exit_progress(
                        condition,
                        step,
                        body,
                        frame,
                        objects,
                        &analysis.modified,
                    )
                }),
            reason: "repeating-loop",
            iterations: 0,
            reads_stdin,
        })
    }

    fn loop_call_objects(
        &self,
        call: &LoopCall,
        objects: &mut HashSet<ObjectId>,
        statics: &mut HashSet<Span>,
        reads_stdin: &mut bool,
        visiting: &mut HashSet<(FileId, String)>,
        depth: usize,
    ) -> Option<()> {
        if depth > 8
            || self
                .lookup_global_binding(&call.name, call.span.file)
                .is_some()
        {
            return None;
        }
        let Some(function) = self.lookup_function(&call.name, call.span.file) else {
            if call.output_only && self.run_options.native_stream_io.is_some() {
                return None;
            }
            if call.reads_stdin {
                if self.run_options.native_stream_io.is_some() {
                    return None;
                }
                *reads_stdin = true;
            }
            return (call.output_only || call.modeled_library).then_some(());
        };
        let key = (function.span.file, function.name.clone());
        if !visiting.insert(key.clone()) {
            return None;
        }
        let mut analysis = LoopAnalysis::default();
        analysis.block(&function.body)?;
        statics.extend(analysis.static_locals.iter().copied());
        let parameters = function
            .params
            .iter()
            .filter_map(|param| param.name.clone())
            .collect::<HashSet<_>>();
        // Reject ambiguous block shadowing rather than confuse a global use before
        // a nested declaration with the local use after it.
        if analysis.locals.iter().any(|name| {
            parameters.contains(name)
                || self
                    .lookup_global_binding(name, function.span.file)
                    .is_some()
        }) {
            return None;
        }
        for (name, span) in &analysis.reads {
            if parameters.contains(name) || analysis.locals.contains(name) {
                continue;
            }
            if let Some(id) = self.lookup_global_binding(name, span.file) {
                objects.insert(id);
            } else if !self
                .program
                .enum_constants
                .contains_key(&(span.file, name.clone()))
            {
                return None;
            }
        }
        for name in &analysis.modified {
            if parameters.contains(name) || analysis.locals.contains(name) {
                continue;
            }
            objects.insert(self.lookup_global_binding(name, function.span.file)?);
        }
        for nested in &analysis.calls {
            if parameters.contains(&nested.name)
                || analysis.locals.contains(&nested.name)
                || (nested.uses_stdin_argument
                    && (parameters.contains("__cboxes_stdin")
                        || analysis.locals.contains("__cboxes_stdin")))
            {
                return None;
            }
            self.loop_call_objects(nested, objects, statics, reads_stdin, visiting, depth + 1)?;
        }
        visiting.remove(&key);
        Some(())
    }

    pub(super) fn check_loop_cycle(
        &mut self,
        detector: &mut Option<LoopDetector>,
        span: Span,
        objects: &ObjectFrames,
    ) -> Result<(), Diagnostic> {
        let Some(active) = detector else {
            return Ok(());
        };
        active.iterations += 1;
        let mut snapshot = LoopSnapshot::default();
        let complete = active.variables.iter().try_for_each(|variable| {
            let id = match variable {
                LoopVariable::Object(id, Some(paths)) => {
                    return self.snapshot_loop_paths(*id, paths, objects, &mut snapshot);
                }
                LoopVariable::Object(id, None) => *id,
                LoopVariable::Static(span) => {
                    let Some(id) = self.local_static_bindings.get(span) else {
                        snapshot.values.push(LoopValue::UncreatedStatic);
                        return Some(());
                    };
                    *id
                }
            };
            self.snapshot_loop_object(id, objects, &mut snapshot)
        });
        if complete.is_none() {
            // A buffer may become initialized by the first input operation. Never
            // compare across an incomplete snapshot, but allow later complete states.
            active.seen.clear();
            active.checkpoint = None;
            active.checkpoint_period = 1;
            active.checkpoint_distance = 0;
            active.detected_at_step = None;
            active.reason = "repeating-loop";
            return Ok(());
        }
        if active.reads_stdin {
            let Some(input) = self.snapshot_loop_stdin() else {
                *detector = None;
                return Ok(());
            };
            snapshot.values.push(input);
        }
        let snapshot = snapshot.values;
        // Recent states catch short cycles immediately. Brent's expanding checkpoint
        // also catches arbitrarily long periods without keeping every past state.
        active.checkpoint_distance += 1;
        if active.seen.contains(&snapshot) || active.checkpoint.as_ref() == Some(&snapshot) {
            active
                .detected_at_step
                .get_or_insert(self.execution_steps_completed);
        } else {
            if active.checkpoint.is_none() || active.checkpoint_distance >= active.checkpoint_period
            {
                active.checkpoint = Some(snapshot.clone());
                active.checkpoint_distance = 0;
                active.checkpoint_period = active.checkpoint_period.saturating_mul(2);
            }
            if active.seen.len() == 64 {
                active.seen.pop_front();
            }
            active.seen.push_back(snapshot);
        }
        if active.iterations > 1
            && active.detected_at_step.is_none()
            && active
                .progress
                .as_ref()
                .is_some_and(|progress| progress.exit_is_unreachable(objects, self))
        {
            active.detected_at_step = Some(self.execution_steps_completed);
            active.reason = "unreachable-loop-exit";
        }
        if let Some(detected_at) = active.detected_at_step {
            // Keep enough iterations for the browser's Continue loop preview, while
            // bounding extra work in helper functions that produce no main trace.
            let preview = self.run_options.execution_trace_following_limit;
            if !self.run_options.capture_visualization
                || self.cboxes_trace.len().saturating_sub(active.trace_start) >= preview
                || self.execution_steps_completed.saturating_sub(detected_at)
                    >= preview.saturating_mul(4)
            {
                self.detected_loop = Some(DetectedLoop {
                    trace_position: active.trace_start,
                    reason: active.reason,
                });
                return Err(Diagnostic::execution_step_limit(span));
            }
        }
        Ok(())
    }
}
