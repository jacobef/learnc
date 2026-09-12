//! Statement grammar and label validation.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect(TokenKind::LBrace)?.span;
        let mut items = Vec::new();
        while !self.at(TokenKind::RBrace) {
            if self.at_keyword(Keyword::StaticAssert) {
                self.parse_static_assertion()?;
            } else if matches!(self.peek_kind(0), Some(TokenKind::Identifier(_)))
                && self.peek_kind(1) == Some(&TokenKind::Colon)
            {
                // Labels occupy a separate namespace, so a visible typedef with
                // the same spelling must not make this look like a declaration.
                items.push(BlockItem::Statement(self.parse_statement()?));
            } else if self.is_declaration_start() {
                items.extend(self.parse_block_declaration_items()?);
            } else {
                items.push(BlockItem::Statement(self.parse_statement()?));
            }
        }
        let end = self.expect(TokenKind::RBrace)?.span;
        Ok(Block {
            items,
            span: start.merge(end),
        })
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        if self.at(TokenKind::LBrace) {
            self.push_block_scope();
            let block = self.parse_block()?;
            self.pop_block_scope();
            return Ok(Statement::Block(block));
        }
        if let Some(TokenKind::Identifier(label)) = self.peek_kind(0).cloned()
            && self.peek_kind(1) == Some(&TokenKind::Colon)
        {
            let start = self.bump().span;
            self.expect(TokenKind::Colon)?;
            let statement = Box::new(self.parse_statement()?);
            let span = start.merge(statement.span());
            return Ok(Statement::UserLabeled {
                label,
                statement,
                span,
            });
        }
        if self.at_keyword(Keyword::Case) {
            let start = self.bump().span;
            let expr = self.parse_expression()?;
            self.expect(TokenKind::Colon)?;
            let statement = Box::new(self.parse_statement()?);
            let span = start.merge(statement.span());
            return Ok(Statement::Labeled {
                label: SwitchLabel::Case { expr, span: start },
                statement,
                span,
            });
        }
        if self.at_keyword(Keyword::Default) {
            let start = self.bump().span;
            self.expect(TokenKind::Colon)?;
            let statement = Box::new(self.parse_statement()?);
            let span = start.merge(statement.span());
            return Ok(Statement::Labeled {
                label: SwitchLabel::Default { span: start },
                statement,
                span,
            });
        }
        if self.at_keyword(Keyword::Break) {
            let start = self.bump().span;
            let end = self.expect(TokenKind::Semicolon)?.span;
            return Ok(Statement::Break(start.merge(end)));
        }
        if self.at_keyword(Keyword::Continue) {
            let start = self.bump().span;
            let end = self.expect(TokenKind::Semicolon)?.span;
            return Ok(Statement::Continue(start.merge(end)));
        }
        if self.at_keyword(Keyword::Goto) {
            let start = self.bump().span;
            let token = self.bump().clone();
            let TokenKind::Identifier(label) = token.kind else {
                return Err(Diagnostic::error(
                    "expected label name after goto",
                    token.span,
                ));
            };
            let end = self.expect(TokenKind::Semicolon)?.span;
            return Ok(Statement::Goto {
                label,
                span: start.merge(end),
            });
        }
        if self.at_keyword(Keyword::Do) {
            let start = self.bump().span;
            let body = Box::new(self.parse_statement()?);
            if !self.at_keyword(Keyword::While) {
                return Err(Diagnostic::error(
                    "expected while after do-body",
                    body.span(),
                ));
            }
            self.bump();
            self.expect(TokenKind::LParen)?;
            let condition = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            let end = self.expect(TokenKind::Semicolon)?.span;
            return Ok(Statement::DoWhile {
                body,
                condition,
                span: start.merge(end),
            });
        }
        if self.at_keyword(Keyword::For) {
            let start = self.bump().span;
            self.push_block_scope();
            self.expect(TokenKind::LParen)?;
            let init = if self.is_declaration_start() {
                Some(ForInit::Declarations(self.parse_declaration_list(true)?))
            } else if self.at(TokenKind::Semicolon) {
                self.bump();
                None
            } else {
                let expr = self.parse_expression()?;
                self.expect(TokenKind::Semicolon)?;
                Some(ForInit::Expression(expr))
            };
            let condition = if self.at(TokenKind::Semicolon) {
                None
            } else {
                Some(self.parse_expression()?)
            };
            self.expect(TokenKind::Semicolon)?;
            let step = if self.at(TokenKind::RParen) {
                None
            } else {
                Some(self.parse_expression()?)
            };
            self.expect(TokenKind::RParen)?;
            let body = Box::new(self.parse_statement()?);
            let span = start.merge(body.span());
            self.pop_block_scope();
            return Ok(Statement::For {
                init,
                condition,
                step,
                body,
                span,
            });
        }
        if self.at_keyword(Keyword::Return) {
            let start = self.bump().span;
            let expr = if self.at(TokenKind::Semicolon) {
                None
            } else {
                Some(self.parse_expression()?)
            };
            let end = self.expect(TokenKind::Semicolon)?.span;
            return Ok(Statement::Return(expr, start.merge(end)));
        }
        if self.at_keyword(Keyword::Switch) {
            let start = self.bump().span;
            self.push_block_scope();
            self.expect(TokenKind::LParen)?;
            let expr = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            self.push_block_scope();
            let statement = self.parse_statement()?;
            self.pop_block_scope();
            let body = match statement {
                Statement::Block(block) => block,
                statement => Block {
                    span: statement.span(),
                    items: vec![BlockItem::Statement(statement)],
                },
            };
            self.validate_switch_labels(&body)?;
            let span = start.merge(body.span);
            self.pop_block_scope();
            return Ok(Statement::Switch { expr, body, span });
        }
        if self.at_keyword(Keyword::If) {
            let start = self.bump().span;
            self.push_block_scope();
            self.expect(TokenKind::LParen)?;
            let condition = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            self.push_block_scope();
            let then_branch = Box::new(self.parse_statement()?);
            self.pop_block_scope();
            let (else_keyword_span, else_branch) = if self.at_keyword(Keyword::Else) {
                let else_span = self.bump().span;
                self.push_block_scope();
                let mut else_statement = self.parse_statement()?;
                self.pop_block_scope();
                if let Statement::If {
                    branch_keyword_span,
                    ..
                } = &mut else_statement
                {
                    *branch_keyword_span = else_span;
                }
                (Some(else_span), Some(Box::new(else_statement)))
            } else {
                (None, None)
            };
            let span = else_branch
                .as_ref()
                .map(|else_branch| start.merge(else_branch.span()))
                .unwrap_or_else(|| start.merge(then_branch.span()));
            self.pop_block_scope();
            return Ok(Statement::If {
                condition,
                then_branch,
                else_branch,
                else_keyword_span,
                branch_keyword_span: start,
                span,
            });
        }
        if self.at_keyword(Keyword::While) {
            let start = self.bump().span;
            self.expect(TokenKind::LParen)?;
            let condition = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            let body = Box::new(self.parse_statement()?);
            let span = start.merge(body.span());
            return Ok(Statement::While {
                condition,
                body,
                span,
            });
        }
        let expr = if self.at(TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        let end = self.expect(TokenKind::Semicolon)?.span;
        Ok(Statement::Expression(expr, end))
    }

    fn validate_switch_labels(&self, body: &Block) -> Result<(), Diagnostic> {
        let mut case_values = HashSet::new();
        let mut has_default = false;
        self.collect_switch_labels_in_block(body, &mut case_values, &mut has_default)
    }

    fn collect_switch_labels_in_block(
        &self,
        block: &Block,
        case_values: &mut HashSet<i128>,
        has_default: &mut bool,
    ) -> Result<(), Diagnostic> {
        for item in &block.items {
            if let BlockItem::Statement(statement) = item {
                self.collect_switch_labels(statement, case_values, has_default)?;
            }
        }
        Ok(())
    }

    fn collect_switch_labels(
        &self,
        statement: &Statement,
        case_values: &mut HashSet<i128>,
        has_default: &mut bool,
    ) -> Result<(), Diagnostic> {
        match statement {
            Statement::Block(block) => {
                self.collect_switch_labels_in_block(block, case_values, has_default)?
            }
            Statement::DoWhile { body, .. }
            | Statement::For { body, .. }
            | Statement::While { body, .. } => {
                self.collect_switch_labels(body, case_values, has_default)?
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_switch_labels(then_branch, case_values, has_default)?;
                if let Some(else_branch) = else_branch {
                    self.collect_switch_labels(else_branch, case_values, has_default)?;
                }
            }
            Statement::Labeled {
                label, statement, ..
            } => {
                match label {
                    SwitchLabel::Case { expr, span } => {
                        let value = self.eval_integer_constant_expr(expr)?;
                        if !case_values.insert(value) {
                            return Err(Diagnostic::error(
                                format!("duplicate case value {}", value),
                                *span,
                            ));
                        }
                    }
                    SwitchLabel::Default { span } => {
                        if *has_default {
                            return Err(Diagnostic::error("multiple default labels", *span));
                        }
                        *has_default = true;
                    }
                }
                self.collect_switch_labels(statement, case_values, has_default)?;
            }
            Statement::Switch { .. } => {}
            Statement::UserLabeled { statement, .. } => {
                self.collect_switch_labels(statement, case_values, has_default)?
            }
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(_, _)
            | Statement::Goto { .. }
            | Statement::Return(_, _) => {}
        }
        Ok(())
    }

    pub(super) fn validate_function_labels(&self, body: &Block) -> Result<(), Diagnostic> {
        let mut labels = HashMap::new();
        let mut gotos = Vec::new();
        self.collect_function_labels_and_gotos_in_block(body, &mut labels, &mut gotos)?;
        for (label, span) in gotos {
            if !labels.contains_key(&label) {
                return Err(Diagnostic::error(
                    format!("use of undeclared label {}", label),
                    span,
                ));
            }
        }
        Ok(())
    }

    fn collect_function_labels_and_gotos_in_block(
        &self,
        block: &Block,
        labels: &mut HashMap<String, Span>,
        gotos: &mut Vec<(String, Span)>,
    ) -> Result<(), Diagnostic> {
        for item in &block.items {
            if let BlockItem::Statement(stmt) = item {
                self.collect_function_labels_and_gotos(stmt, labels, gotos)?;
            }
        }
        Ok(())
    }

    fn collect_function_labels_and_gotos(
        &self,
        stmt: &Statement,
        labels: &mut HashMap<String, Span>,
        gotos: &mut Vec<(String, Span)>,
    ) -> Result<(), Diagnostic> {
        match stmt {
            Statement::Block(block) => {
                self.collect_function_labels_and_gotos_in_block(block, labels, gotos)?
            }
            Statement::DoWhile { body, .. } => {
                self.collect_function_labels_and_gotos(body, labels, gotos)?
            }
            Statement::For { body, .. } => {
                self.collect_function_labels_and_gotos(body, labels, gotos)?
            }
            Statement::Goto { label, span } => gotos.push((label.clone(), *span)),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_function_labels_and_gotos(then_branch, labels, gotos)?;
                if let Some(else_branch) = else_branch {
                    self.collect_function_labels_and_gotos(else_branch, labels, gotos)?;
                }
            }
            Statement::Labeled { statement, .. } => {
                self.collect_function_labels_and_gotos(statement, labels, gotos)?
            }
            Statement::Switch { body, .. } => {
                self.collect_function_labels_and_gotos_in_block(body, labels, gotos)?
            }
            Statement::UserLabeled {
                label,
                statement,
                span,
            } => {
                if labels.insert(label.clone(), *span).is_some() {
                    return Err(Diagnostic::error(
                        format!("duplicate label {}", label),
                        *span,
                    ));
                }
                self.collect_function_labels_and_gotos(statement, labels, gotos)?;
            }
            Statement::While { body, .. } => {
                self.collect_function_labels_and_gotos(body, labels, gotos)?
            }
            Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Expression(_, _)
            | Statement::Return(_, _) => {}
        }
        Ok(())
    }
}
