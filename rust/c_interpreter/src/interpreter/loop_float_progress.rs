//! Monotonic floating counter checks, without assuming exact repeated addition.

use super::*;

pub(super) struct FloatLoopProgress {
    pub object: ObjectId,
    pub op: BinaryOp,
    pub bound: f64,
    pub direction: i8,
}

impl FloatLoopProgress {
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
        let Ok(current) = value.to_float() else {
            return false;
        };
        if value.indeterminate || !current.is_finite() {
            return false;
        }
        match self.op {
            BinaryOp::Less => current < self.bound && self.direction <= 0,
            BinaryOp::LessEqual => current <= self.bound && self.direction <= 0,
            BinaryOp::Greater => current > self.bound && self.direction >= 0,
            BinaryOp::GreaterEqual => current >= self.bound && self.direction >= 0,
            BinaryOp::NotEqual => {
                (current < self.bound && self.direction <= 0)
                    || (current > self.bound && self.direction >= 0)
            }
            _ => false,
        }
    }
}

pub(super) fn combine_directions(a: i8, b: i8) -> Option<i8> {
    if a == 0 {
        Some(b)
    } else if b == 0 || a == b {
        Some(a)
    } else {
        None
    }
}

impl<'a> Interpreter<'a> {
    pub(super) fn loop_float_body_direction(
        &self,
        body: &Statement,
        name: &str,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<i8> {
        match body {
            Statement::Expression(None, _) => Some(0),
            Statement::Expression(Some(expr), _) => {
                self.loop_float_expression_direction(expr, name, frame, objects, modified)
            }
            Statement::Block(block) => {
                let mut direction = 0;
                for item in &block.items {
                    let BlockItem::Statement(statement) = item else {
                        return None;
                    };
                    direction = combine_directions(
                        direction,
                        self.loop_float_body_direction(statement, name, frame, objects, modified)?,
                    )?;
                }
                Some(direction)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } if self.loop_expression_preserves_counter(condition, name) => {
                let then_direction =
                    self.loop_float_body_direction(then_branch, name, frame, objects, modified)?;
                let else_direction = match else_branch {
                    Some(branch) => {
                        self.loop_float_body_direction(branch, name, frame, objects, modified)?
                    }
                    None => 0,
                };
                combine_directions(then_direction, else_direction)
            }
            _ => None,
        }
    }

    pub(super) fn loop_float_expression_direction(
        &self,
        expr: &Expr,
        name: &str,
        frame: &Frame,
        objects: &ObjectFrames,
        modified: &HashSet<String>,
    ) -> Option<i8> {
        let is_counter =
            |expr: &Expr| matches!(expr, Expr::Variable(candidate, _) if candidate == name);
        let constant_direction = |op, rhs: &Expr| {
            if !matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                return None;
            }
            let value = self.loop_invariant_value(rhs, frame, objects, modified)?;
            let delta = self
                .convert_value(value, &CType::Double, rhs.span())
                .ok()?
                .to_float()
                .ok()?;
            if !delta.is_finite() {
                return None;
            }
            let direction = if delta > 0.0 {
                1
            } else if delta < 0.0 {
                -1
            } else {
                0
            };
            Some(if op == BinaryOp::Sub {
                -direction
            } else {
                direction
            })
        };
        match expr {
            Expr::Binary {
                op: BinaryOp::Comma,
                lhs,
                rhs,
                ..
            } => combine_directions(
                self.loop_float_expression_direction(lhs, name, frame, objects, modified)?,
                self.loop_float_expression_direction(rhs, name, frame, objects, modified)?,
            ),

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
                constant_direction(*op, rhs)
            }
            Expr::Assign { lhs, rhs, .. } if is_counter(lhs) => {
                let Expr::Binary { op, lhs, rhs, .. } = rhs.as_ref() else {
                    return None;
                };
                if is_counter(lhs) {
                    constant_direction(*op, rhs)
                } else if *op == BinaryOp::Add && is_counter(rhs) {
                    constant_direction(*op, lhs)
                } else {
                    None
                }
            }
            _ => self
                .loop_expression_preserves_counter(expr, name)
                .then_some(0),
        }
    }
}
