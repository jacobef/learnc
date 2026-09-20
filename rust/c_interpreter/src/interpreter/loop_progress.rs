//! Exact progress checks for a single integer counter updated by a constant stride.

use super::*;

pub(super) enum LoopProgress {
    Integer(IntegerLoopProgress),
    All(Box<LoopProgress>, Box<LoopProgress>),
    Any(Box<LoopProgress>, Box<LoopProgress>),
    Floating(super::loop_float_progress::FloatLoopProgress),
}

impl LoopProgress {
    pub(super) fn exit_is_unreachable(
        &self,
        objects: &ObjectFrames,
        interpreter: &Interpreter<'_>,
    ) -> bool {
        match self {
            Self::All(a, b) => {
                a.exit_is_unreachable(objects, interpreter)
                    && b.exit_is_unreachable(objects, interpreter)
            }
            Self::Any(a, b) => {
                a.exit_is_unreachable(objects, interpreter)
                    || b.exit_is_unreachable(objects, interpreter)
            }
            Self::Integer(progress) => progress.exit_is_unreachable(objects, interpreter),
            Self::Floating(progress) => progress.exit_is_unreachable(objects, interpreter),
        }
    }
}

pub(super) struct IntegerLoopProgress {
    object: ObjectId,
    op: BinaryOp,
    bound: i128,
    stride: i128,
    offset: i128,
    ty: CType,
}

impl IntegerLoopProgress {
    pub(super) fn exit_is_unreachable(
        &self,
        objects: &ObjectFrames,
        interpreter: &Interpreter<'_>,
    ) -> bool {
        let Some(object) = interpreter.lookup_object(objects, self.object) else {
            return false;
        };
        let StoredValue::Scalar(value) = &object.value else {
            return false;
        };
        if value.indeterminate {
            return false;
        }
        let Ok(current) = value.to_int() else {
            return false;
        };
        let (min, max) = self.ty.integer_bounds().unwrap();
        let Some(current) = current.checked_add(self.offset) else {
            return false;
        };
        let current = if self.ty.is_unsigned_integer() {
            current.rem_euclid(max + 1)
        } else if (min..=max).contains(&current) {
            current
        } else {
            return false;
        };
        let condition = |value| match self.op {
            BinaryOp::Less => value < self.bound,
            BinaryOp::LessEqual => value <= self.bound,
            BinaryOp::Greater => value > self.bound,
            BinaryOp::GreaterEqual => value >= self.bound,
            BinaryOp::NotEqual => value != self.bound,
            _ => false,
        };
        if !condition(current) {
            return false;
        }
        if self.op != BinaryOp::NotEqual && condition(min) && condition(max) {
            return true;
        }
        if self.stride == 0 {
            return true;
        }
        if self.ty.is_unsigned_integer() {
            // Unsigned wraparound can turn a backwards-looking loop into a finite
            // one. For !=, solve reachability in the actual modular integer domain.
            if self.op != BinaryOp::NotEqual {
                return false;
            }
            let modulus = max + 1;
            let mut a = self.stride.rem_euclid(modulus);
            let mut b = modulus;
            while b != 0 {
                (a, b) = (b, a % b);
            }
            return (self.bound - current).rem_euclid(a) != 0;
        }
        match self.op {
            BinaryOp::Less | BinaryOp::LessEqual => self.stride < 0,
            BinaryOp::Greater | BinaryOp::GreaterEqual => self.stride > 0,
            BinaryOp::NotEqual => {
                let distance = self.bound - current;
                distance.signum() != self.stride.signum() || distance % self.stride != 0
            }
            _ => false,
        }
    }
}

struct CounterOperand<'e> {
    name: &'e str,
    condition_stride: i128,
    post_shift: i128,
    offset_expr: Option<(BinaryOp, &'e Expr)>,
}

fn counter_operand(expr: &Expr) -> Option<CounterOperand<'_>> {
    let plain = |name| CounterOperand {
        name,
        condition_stride: 0,
        post_shift: 0,
        offset_expr: None,
    };
    match expr {
        Expr::Variable(name, _) => Some(plain(name)),
        Expr::Unary { op, expr, .. } => {
            let Expr::Variable(name, _) = expr.as_ref() else {
                return None;
            };
            let condition_stride = match op {
                UnaryOp::PreIncrement => 1,
                UnaryOp::PreDecrement => -1,
                _ => return None,
            };
            Some(CounterOperand {
                condition_stride,
                ..plain(name)
            })
        }
        Expr::Postfix { op, expr, .. } => {
            let Expr::Variable(name, _) = expr.as_ref() else {
                return None;
            };
            let delta = if *op == PostfixOp::PostIncrement {
                1
            } else {
                -1
            };
            Some(CounterOperand {
                condition_stride: delta,
                post_shift: delta,
                ..plain(name)
            })
        }
        Expr::Binary {
            op: op @ (BinaryOp::Add | BinaryOp::Sub),
            lhs,
            rhs,
            ..
        } => {
            let Expr::Variable(name, _) = lhs.as_ref() else {
                return None;
            };
            Some(CounterOperand {
                offset_expr: Some((*op, rhs)),
                ..plain(name)
            })
        }
        _ => None,
    }
}

impl<'a> Interpreter<'a> {
    pub(super) fn loop_trailing_exit_progress(
        &self,
        condition: Option<&Expr>,
        step: Option<&Expr>,
        body: &Statement,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<LoopProgress> {
        // Handle a trailing conditional break or assignment to a loop-control flag.
        // Sampling occurs at the next iteration, immediately after this guard.
        // A for-step after the guard would require translating its comparison.
        if step.is_some() {
            return None;
        }
        let Statement::Block(block) = body else {
            return None;
        };
        let (last, preceding) = block.items.split_last()?;
        let BlockItem::Statement(Statement::If {
            condition: exit,
            then_branch,
            else_branch: None,
            ..
        }) = last
        else {
            return None;
        };
        let updates = Statement::Block(Block {
            items: preceding.to_vec(),
            span: block.span,
        });
        let action = match then_branch.as_ref() {
            Statement::Block(block) => match block.items.as_slice() {
                [BlockItem::Statement(statement)] => statement,
                _ => return None,
            },
            statement => statement,
        };
        match action {
            Statement::Break(_) => {
                if condition.is_some_and(|expr| {
                    !self
                        .eval_loop_constant(expr, frame, objects)
                        .and_then(|value| self.scalar_truthy(&value, expr.span()).ok())
                        .unwrap_or(false)
                }) {
                    return None;
                }
            }
            Statement::Expression(Some(Expr::Assign { lhs, rhs, .. }), _) => {
                let Expr::Variable(flag, _) = lhs.as_ref() else {
                    return None;
                };
                let id = frame
                    .bindings
                    .get(flag)
                    .copied()
                    .or_else(|| self.lookup_global_binding(flag, lhs.span().file))?;
                let ty = &self.lookup_object(objects, id)?.ty;
                let assigned = self
                    .convert_value(
                        self.eval_loop_constant(rhs, frame, objects)?,
                        ty,
                        rhs.span(),
                    )
                    .ok()?;
                if self.loop_flag_condition(condition?, flag, &assigned, frame, objects)? {
                    return None;
                }
                let mut updates_analysis = super::loop_detection::LoopAnalysis::default();
                updates_analysis.statement(&updates)?;
                if updates_analysis.modified.contains(flag) || updates_analysis.indirect_writes {
                    return None;
                }
            }
            _ => return None,
        }
        let mut analysis = super::loop_detection::LoopAnalysis::default();
        analysis.expression(exit)?;
        if !analysis.modified.is_empty() || analysis.indirect_writes || !analysis.calls.is_empty() {
            return None;
        }
        let guard = invert_loop_comparison(exit)?;
        self.loop_progress(&guard, None, &updates, frame, objects, modified)
    }
    fn loop_flag_condition(
        &self,
        expr: &Expr,
        flag: &str,
        assigned: &TypedValue,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Option<bool> {
        match expr {
            Expr::Variable(name, _) if name == flag => {
                self.scalar_truthy(assigned, expr.span()).ok()
            }
            Expr::Unary {
                op: UnaryOp::LogicalNot,
                expr,
                ..
            } => Some(!self.loop_flag_condition(expr, flag, assigned, frame, objects)?),
            Expr::Binary { op, lhs, rhs, span }
                if matches!(
                    op,
                    BinaryOp::Equal
                        | BinaryOp::NotEqual
                        | BinaryOp::Less
                        | BinaryOp::LessEqual
                        | BinaryOp::Greater
                        | BinaryOp::GreaterEqual
                ) =>
            {
                let value = |operand: &Expr| match operand {
                    Expr::Variable(name, _) if name == flag => Some(assigned.clone()),
                    _ => self.eval_loop_constant(operand, frame, objects),
                };
                let lhs_value = value(lhs)?;
                let rhs_value = value(rhs)?;
                let result = if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) {
                    let ValueCategory::RValue(value) = self
                        .eval_equality(*op, lhs_value, rhs_value, lhs, rhs, *span, frame, objects)
                        .ok()?
                    else {
                        return None;
                    };
                    value
                } else {
                    self.compute_binary_value(*op, lhs_value, rhs_value, *span, objects)
                        .ok()?
                };
                self.scalar_truthy(&result, *span).ok()
            }
            _ => None,
        }
    }

    pub(super) fn loop_progress(
        &self,
        condition: &Expr,
        step: Option<&Expr>,
        body: &Statement,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<LoopProgress> {
        if let Expr::Unary {
            op: UnaryOp::LogicalNot,
            expr,
            ..
        } = condition
        {
            let inverse = invert_loop_comparison(expr)?;
            return self.loop_progress(&inverse, step, body, frame, objects, modified);
        }
        if let Expr::Binary {
            op: op @ (BinaryOp::LogicalAnd | BinaryOp::LogicalOr),
            lhs,
            rhs,
            ..
        } = condition
        {
            // Each proof assumes the guard does not itself change the state. In
            // particular, short-circuit increments must not be counted as unconditional.
            let mut analysis = super::loop_detection::LoopAnalysis::default();
            analysis.expression(condition)?;
            if !analysis.modified.is_empty()
                || analysis.indirect_writes
                || !analysis.calls.is_empty()
            {
                return None;
            }
            let a = self.loop_progress(lhs, step, body, frame, objects, modified);
            let b = self.loop_progress(rhs, step, body, frame, objects, modified);
            return match (op, a, b) {
                (BinaryOp::LogicalAnd, Some(a), Some(b)) => {
                    Some(LoopProgress::All(Box::new(a), Box::new(b)))
                }
                (BinaryOp::LogicalOr, Some(a), Some(b)) => {
                    Some(LoopProgress::Any(Box::new(a), Box::new(b)))
                }
                (BinaryOp::LogicalOr, Some(a), None) | (BinaryOp::LogicalOr, None, Some(a)) => {
                    Some(a)
                }
                _ => None,
            };
        }
        let (counter, bound_expr, op, comparison) = if let Expr::Binary { op, lhs, rhs, .. } =
            condition
            && matches!(
                op,
                BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
                    | BinaryOp::NotEqual
            ) {
            let left = counter_operand(lhs);
            let right = counter_operand(rhs);
            let prefer_right = right
                .as_ref()
                .is_some_and(|candidate| modified.contains(candidate.name))
                && left
                    .as_ref()
                    .is_none_or(|candidate| !modified.contains(candidate.name));
            if let Some(counter) = left.filter(|_| !prefer_right) {
                (
                    counter,
                    Some(rhs.as_ref()),
                    *op,
                    Some((lhs.as_ref(), rhs.as_ref())),
                )
            } else if let Some(counter) = right {
                let op = match op {
                    BinaryOp::Less => BinaryOp::Greater,
                    BinaryOp::LessEqual => BinaryOp::GreaterEqual,
                    BinaryOp::Greater => BinaryOp::Less,
                    BinaryOp::GreaterEqual => BinaryOp::LessEqual,
                    BinaryOp::NotEqual => BinaryOp::NotEqual,
                    _ => return None,
                };
                (
                    counter,
                    Some(lhs.as_ref()),
                    op,
                    Some((lhs.as_ref(), rhs.as_ref())),
                )
            } else {
                return None;
            }
        } else {
            (counter_operand(condition)?, None, BinaryOp::NotEqual, None)
        };
        let CounterOperand {
            name,
            condition_stride,
            post_shift,
            offset_expr,
        } = counter;
        if !matches!(
            op,
            BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
                | BinaryOp::NotEqual
        ) {
            return None;
        }
        let object = frame
            .bindings
            .get(name)
            .copied()
            .or_else(|| self.lookup_global_binding(name, condition.span().file))?;
        let ty = self
            .lookup_object(objects, object)?
            .ty
            .unqualified()
            .clone();
        if ty.is_floating() {
            if condition_stride != 0 || offset_expr.is_some() {
                return None;
            }
            let bound = if let Some(expr) = bound_expr {
                let value = self.loop_invariant_value(expr, frame, objects, modified)?;
                let common = self
                    .usual_arithmetic_type(&ty, &value.ty, expr.span())
                    .ok()?;
                self.convert_value(value, &common, expr.span())
                    .ok()?
                    .to_float()
                    .ok()?
            } else {
                0.0
            };
            if !bound.is_finite() {
                return None;
            }
            let mut direction =
                self.loop_float_body_direction(body, name, frame, objects, modified)?;
            if let Some(step) = step {
                direction = super::loop_float_progress::combine_directions(
                    direction,
                    self.loop_float_expression_direction(step, name, frame, objects, modified)?,
                )?;
            }
            return Some(LoopProgress::Floating(
                super::loop_float_progress::FloatLoopProgress {
                    object,
                    op,
                    bound,
                    direction,
                },
            ));
        }
        if !matches!(
            ty,
            CType::Int
                | CType::UnsignedInt
                | CType::Long
                | CType::UnsignedLong
                | CType::LongLong
                | CType::UnsignedLongLong
        ) {
            return None;
        }
        // Mixed signedness and wider arithmetic need a different proof. In particular,
        // narrowing an intermediate result may wrap even when the counter is signed.
        if let Some((lhs, rhs)) = comparison {
            let common = self
                .usual_arithmetic_expr_type(lhs, rhs, condition.span(), frame, objects)
                .ok()?;
            if common != ty {
                return None;
            }
        }
        let offset = if let Some((op, expr)) = offset_expr {
            let value = self.loop_invariant_value(expr, frame, objects, modified)?;
            if self
                .usual_arithmetic_type(&ty, &value.ty, expr.span())
                .ok()?
                != ty
            {
                return None;
            }
            let offset = value.to_int().ok()?;
            if op == BinaryOp::Sub {
                offset.checked_neg()?
            } else {
                offset
            }
        } else {
            0
        };
        let bound = if let Some(bound_expr) = bound_expr {
            let bound = self.loop_invariant_value(bound_expr, frame, objects, modified)?;
            self.convert_value(bound, &ty, condition.span())
                .ok()?
                .to_int()
                .ok()?
        } else {
            0
        };
        let bound = bound.checked_add(post_shift)?;
        let (min, max) = ty.integer_bounds()?;
        // We sample after evaluating a while/for condition. A postfix test compares
        // the previous value, so translate its target into the sampled value's domain.
        let bound = if ty.is_unsigned_integer() {
            bound.rem_euclid(max + 1)
        } else if (min..=max).contains(&bound) {
            bound
        } else {
            return None;
        };
        // Relational comparisons across the unsigned wrap boundary cannot be shifted
        // this way, unlike equality. Keep those on cycle detection/the step limit.
        if ty.is_unsigned_integer() && post_shift != 0 && op != BinaryOp::NotEqual {
            return None;
        }
        let mut stride = self
            .loop_body_stride(body, name, &ty, frame, objects, modified)?
            .checked_add(condition_stride)?;
        if let Some(step) = step {
            stride = stride.checked_add(
                self.loop_expression_stride(step, name, &ty, frame, objects, modified)?,
            )?;
        }
        Some(LoopProgress::Integer(IntegerLoopProgress {
            object,
            op,
            bound,
            stride,
            offset,
            ty,
        }))
    }

    fn loop_body_stride(
        &self,
        body: &Statement,
        name: &str,
        ty: &CType,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<i128> {
        match body {
            Statement::Expression(None, _) => Some(0),
            Statement::Expression(Some(expr), _) => {
                self.loop_expression_stride(expr, name, ty, frame, objects, modified)
            }
            Statement::Block(block) => {
                let mut stride = 0i128;
                for item in &block.items {
                    let BlockItem::Statement(stmt) = item else {
                        return None;
                    };
                    stride = stride.checked_add(
                        self.loop_body_stride(stmt, name, ty, frame, objects, modified)?,
                    )?;
                }
                Some(stride)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } if self.loop_expression_preserves_counter(condition, name) => {
                let then_stride =
                    self.loop_body_stride(then_branch, name, ty, frame, objects, modified)?;
                let else_stride = match else_branch {
                    Some(branch) => {
                        self.loop_body_stride(branch, name, ty, frame, objects, modified)?
                    }
                    None => 0,
                };
                (then_stride == else_stride).then_some(then_stride)
            }
            // A break, return, unequal branches, or nested loop can change the induction step.
            _ => None,
        }
    }

    fn loop_expression_stride(
        &self,
        expr: &Expr,
        name: &str,
        ty: &CType,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<i128> {
        let is_counter =
            |expr: &Expr| matches!(expr, Expr::Variable(candidate, _) if candidate == name);
        let constant_stride = |op, rhs: &Expr| {
            if !matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                return None;
            }
            let value = self.loop_invariant_value(rhs, frame, objects, modified)?;
            let common = self.usual_arithmetic_type(ty, &value.ty, rhs.span()).ok()?;
            if common != *ty {
                return None;
            }
            let value = value.to_int().ok()?;
            if op == BinaryOp::Add {
                Some(value)
            } else {
                value.checked_neg()
            }
        };
        match expr {
            Expr::Binary {
                op: BinaryOp::Comma,
                lhs,
                rhs,
                ..
            } => self
                .loop_expression_stride(lhs, name, ty, frame, objects, modified)?
                .checked_add(self.loop_expression_stride(rhs, name, ty, frame, objects, modified)?),

            Expr::Unary {
                op: UnaryOp::PreIncrement,
                expr,
                ..
            }
            | Expr::Postfix {
                op: PostfixOp::PostIncrement,
                expr,
                ..
            } if is_counter(expr) => Some(1),
            Expr::Unary {
                op: UnaryOp::PreDecrement,
                expr,
                ..
            }
            | Expr::Postfix {
                op: PostfixOp::PostDecrement,
                expr,
                ..
            } if is_counter(expr) => Some(-1),
            Expr::CompoundAssign { op, lhs, rhs, .. } if is_counter(lhs) => {
                constant_stride(*op, rhs)
            }
            Expr::Assign { lhs, rhs, .. } if is_counter(lhs) => {
                let Expr::Binary { op, lhs, rhs, .. } = rhs.as_ref() else {
                    return None;
                };
                if is_counter(lhs) {
                    constant_stride(*op, rhs)
                } else if *op == BinaryOp::Add && is_counter(rhs) {
                    constant_stride(*op, lhs)
                } else {
                    None
                }
            }
            _ => self
                .loop_expression_preserves_counter(expr, name)
                .then_some(0),
        }
    }
    pub(super) fn loop_expression_preserves_counter(&self, expr: &Expr, name: &str) -> bool {
        let mut analysis = super::loop_detection::LoopAnalysis::default();
        analysis.discarded_expression(expr).is_some()
            && !analysis.indirect_writes
            && !analysis.modified.contains(name)
            && analysis.calls.iter().all(|call| {
                call.output_only && self.lookup_function(&call.name, call.span.file).is_none()
            })
    }

    pub(super) fn loop_invariant_value(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<TypedValue> {
        if let Some(value) = self.eval_loop_constant(expr, frame, objects) {
            return Some(value);
        }
        match expr {
            Expr::Variable(name, _) => {
                if modified.contains(name) {
                    return None;
                }
                let id = frame
                    .bindings
                    .get(name)
                    .copied()
                    .or_else(|| self.lookup_global_binding(name, expr.span().file))?;
                let StoredValue::Scalar(value) = &self.lookup_object(objects, id)?.value else {
                    return None;
                };
                (!value.indeterminate).then(|| value.clone())
            }
            Expr::Binary { op, lhs, rhs, span }
                if matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) =>
            {
                self.compute_binary_value(
                    *op,
                    self.loop_invariant_value(lhs, frame, objects, modified)?,
                    self.loop_invariant_value(rhs, frame, objects, modified)?,
                    *span,
                    objects,
                )
                .ok()
            }
            Expr::Unary { op, expr, span } if matches!(op, UnaryOp::Plus | UnaryOp::Minus) => self
                .compute_binary_value(
                    if *op == UnaryOp::Plus {
                        BinaryOp::Add
                    } else {
                        BinaryOp::Sub
                    },
                    TypedValue::int(0),
                    self.loop_invariant_value(expr, frame, objects, modified)?,
                    *span,
                    objects,
                )
                .ok(),
            _ => None,
        }
    }

    pub(super) fn eval_loop_constant(
        &self,
        expr: &Expr,
        frame: &Frame,
        objects: &ObjectFrames,
    ) -> Option<TypedValue> {
        if let Ok(value) = self.eval_typed_integer_constant_expr(expr, frame, objects) {
            return Some(value);
        }
        match expr {
            Expr::Number(number, _) => match number.value {
                NumberValue::Floating(value) => {
                    Some(TypedValue::floating(number.ty.clone(), value))
                }
                _ => None,
            },
            Expr::Unary { op, expr, .. } if matches!(op, UnaryOp::Plus | UnaryOp::Minus) => {
                let value = self.eval_loop_constant(expr, frame, objects)?;
                if !value.ty.is_floating() {
                    return None;
                }
                let number = value.to_float().ok()?;
                Some(TypedValue::floating(
                    value.ty,
                    if *op == UnaryOp::Minus {
                        -number
                    } else {
                        number
                    },
                ))
            }
            Expr::Binary { op, lhs, rhs, span }
                if matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
                ) =>
            {
                self.compute_binary_value(
                    *op,
                    self.eval_loop_constant(lhs, frame, objects)?,
                    self.eval_loop_constant(rhs, frame, objects)?,
                    *span,
                    objects,
                )
                .ok()
            }
            Expr::Cast {
                ty,
                expr,
                vla_bounds,
                span,
            } if vla_bounds.iter().all(Option::is_none) => self
                .convert_value(self.eval_loop_constant(expr, frame, objects)?, ty, *span)
                .ok(),
            _ => None,
        }
    }
}

fn invert_loop_comparison(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Unary {
            op: UnaryOp::LogicalNot,
            expr,
            ..
        } => Some(expr.as_ref().clone()),
        Expr::Binary { op, lhs, rhs, span } => {
            let inverse = match op {
                BinaryOp::Equal => BinaryOp::NotEqual,
                BinaryOp::NotEqual => BinaryOp::Equal,
                BinaryOp::Less => BinaryOp::GreaterEqual,
                BinaryOp::LessEqual => BinaryOp::Greater,
                BinaryOp::Greater => BinaryOp::LessEqual,
                BinaryOp::GreaterEqual => BinaryOp::Less,
                _ => return None,
            };
            Some(Expr::Binary {
                op: inverse,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
                span: *span,
            })
        }
        _ => None,
    }
}
