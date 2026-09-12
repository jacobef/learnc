//! Remap translation-unit tag IDs without rebuilding the syntax tree.

use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{
    Block, BlockItem, Declaration, Expr, ExternalDeclaration, ForInit, FunctionDecl, Initializer,
    Parameter, Statement, SwitchLabel,
};
use crate::types::CType;

pub(super) struct TagRemapper {
    pub records: HashMap<usize, usize>,
    pub enums: HashMap<usize, usize>,
}

impl TagRemapper {
    pub fn ty(&self, ty: &mut CType) {
        match ty {
            CType::Struct(id, _) | CType::Union(id, _) => {
                *id = self.records.get(id).copied().unwrap_or(*id);
            }
            CType::Enum(id, _) => *id = self.enums.get(id).copied().unwrap_or(*id),
            CType::Function(result, params, _) => {
                self.ty(Arc::make_mut(result));
                for param in Arc::make_mut(params) {
                    self.ty(param);
                }
            }
            CType::Qualified(inner, _) | CType::Pointer(inner) | CType::Array(inner, _) => {
                self.ty(Arc::make_mut(inner));
            }
            _ => {}
        }
    }

    pub fn external(&self, external: &mut ExternalDeclaration) {
        match external {
            ExternalDeclaration::Function(function) => {
                self.ty(&mut function.return_type);
                self.parameters(&mut function.params);
                self.block(&mut function.body);
            }
            ExternalDeclaration::FunctionDeclaration(decl) => self.function_decl(decl),
            ExternalDeclaration::ObjectDeclaration(decl) => self.declaration(decl),
        }
    }

    fn function_decl(&self, decl: &mut FunctionDecl) {
        self.ty(&mut decl.return_type);
        self.parameters(&mut decl.params);
    }

    fn parameters(&self, params: &mut [Parameter]) {
        for param in params {
            self.ty(&mut param.ty);
            self.bounds(&mut param.vla_bounds);
            if let Some(bound) = &mut param.static_array_bound {
                self.expr(bound);
            }
        }
    }

    fn bounds(&self, bounds: &mut [Option<Expr>]) {
        for bound in bounds.iter_mut().flatten() {
            self.expr(bound);
        }
    }

    fn declaration(&self, decl: &mut Declaration) {
        self.ty(&mut decl.ty);
        self.bounds(&mut decl.vla_bounds);
        if let Some(init) = &mut decl.init {
            self.initializer(init);
        }
    }

    fn initializer(&self, init: &mut Initializer) {
        match init {
            Initializer::Expr(expr) => self.expr(expr),
            Initializer::List { items, .. } => {
                for item in items {
                    self.initializer(&mut item.initializer);
                }
            }
        }
    }

    fn block(&self, block: &mut Block) {
        for item in &mut block.items {
            match item {
                BlockItem::Declaration(decl) => self.declaration(decl),
                BlockItem::FunctionDeclaration(decl) => self.function_decl(decl),
                BlockItem::Statement(stmt) => self.statement(stmt),
            }
        }
    }

    fn statement(&self, stmt: &mut Statement) {
        match stmt {
            Statement::Block(block) => self.block(block),
            Statement::Break(_) | Statement::Continue(_) | Statement::Goto { .. } => {}
            Statement::DoWhile {
                body, condition, ..
            }
            | Statement::While {
                body, condition, ..
            } => {
                self.statement(body);
                self.expr(condition);
            }
            Statement::Expression(expr, _) | Statement::Return(expr, _) => {
                if let Some(expr) = expr {
                    self.expr(expr);
                }
            }
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
                            self.declaration(decl);
                        }
                    }
                    Some(ForInit::Expression(expr)) => self.expr(expr),
                    None => {}
                }
                if let Some(expr) = condition {
                    self.expr(expr);
                }
                if let Some(expr) = step {
                    self.expr(expr);
                }
                self.statement(body);
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expr(condition);
                self.statement(then_branch);
                if let Some(branch) = else_branch {
                    self.statement(branch);
                }
            }
            Statement::Labeled {
                label, statement, ..
            } => {
                if let SwitchLabel::Case { expr, .. } = label {
                    self.expr(expr);
                }
                self.statement(statement);
            }
            Statement::Switch { expr, body, .. } => {
                self.expr(expr);
                self.block(body);
            }
            Statement::UserLabeled { statement, .. } => self.statement(statement),
        }
    }

    fn expr(&self, expr: &mut Expr) {
        match expr {
            Expr::Number(..)
            | Expr::CharLiteral(..)
            | Expr::WideCharLiteral(..)
            | Expr::Utf16CharLiteral(..)
            | Expr::Utf32CharLiteral(..)
            | Expr::StringLiteral(..)
            | Expr::WideStringLiteral(..)
            | Expr::Utf16StringLiteral(..)
            | Expr::Utf32StringLiteral(..)
            | Expr::Variable(..) => {}
            Expr::Unary { expr, .. }
            | Expr::Postfix { expr, .. }
            | Expr::SizeofExpr { expr, .. }
            | Expr::Member { base: expr, .. } => self.expr(expr),
            Expr::Binary { lhs, rhs, .. }
            | Expr::Assign { lhs, rhs, .. }
            | Expr::CompoundAssign { lhs, rhs, .. }
            | Expr::Subscript {
                base: lhs,
                index: rhs,
                ..
            } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::SizeofType { ty, vla_bounds, .. } => {
                self.ty(ty);
                self.bounds(vla_bounds);
            }
            Expr::OffsetOf { ty, .. } => self.ty(ty),
            Expr::Cast {
                ty,
                vla_bounds,
                expr,
                ..
            } => {
                self.ty(ty);
                self.bounds(vla_bounds);
                self.expr(expr);
            }
            Expr::CompoundLiteral {
                ty,
                vla_bounds,
                initializer,
                ..
            } => {
                self.ty(ty);
                self.bounds(vla_bounds);
                self.initializer(initializer);
            }
            Expr::GenericSelection {
                control,
                associations,
                default,
                ..
            } => {
                self.expr(control);
                for association in associations {
                    self.ty(&mut association.ty);
                    self.expr(&mut association.expr);
                }
                if let Some(expr) = default {
                    self.expr(expr);
                }
            }
            Expr::VaArg { ap, ty, .. } => {
                self.expr(ap);
                self.ty(ty);
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.expr(condition);
                self.expr(then_expr);
                self.expr(else_expr);
            }
            Expr::Call {
                callee,
                args,
                declared_callee_type,
                ..
            } => {
                self.expr(callee);
                for arg in args {
                    self.expr(arg);
                }
                if let Some(ty) = declared_callee_type {
                    self.ty(ty);
                }
            }
        }
    }
}
