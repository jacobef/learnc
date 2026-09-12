//! External and block declarations, parameter lists, and initializers.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_external_declaration(
        &mut self,
        externals: &mut Vec<ExternalDeclaration>,
    ) -> Result<(), Diagnostic> {
        self.skip_gnu_attributes()?;
        let declaration_start = self.current_span();
        let specs = self.parse_declaration_specifiers(DeclContext::FileScope)?;
        let return_type_span = declaration_start.merge(self.prev_span());
        if self.eat(TokenKind::Semicolon) {
            if specs.storage_class.is_some()
                || specs.is_inline
                || specs.is_noreturn
                || specs.alignment.is_some()
            {
                return Err(Diagnostic::error(
                    "storage class specifiers and inline require a declarator",
                    self.prev_span(),
                ));
            }
            if !specs.declares_tag_or_enumerators {
                return Err(Diagnostic::error(
                    "declaration does not declare a declarator, tag, or enumeration constant",
                    self.prev_span(),
                ));
            }
            return Ok(());
        }
        let mut declarator =
            self.parse_declarator(specs.base_type.clone(), DeclContext::FileScope)?;
        declarator.vla_bounds.extend(specs.vla_bounds.clone());
        if specs.is_noreturn && !declarator.ty.is_function() {
            return Err(Diagnostic::error(
                "_Noreturn is only valid on functions",
                declarator.span,
            ));
        }
        if specs.is_noreturn && specs.storage_class == Some(ParsedStorageClass::Typedef) {
            return Err(Diagnostic::error(
                "_Noreturn is not valid in a typedef declaration",
                declarator.span,
            ));
        }
        if specs.is_noreturn && declarator.name == "main" && declarator.ty.is_function() {
            return Err(Diagnostic::error(
                "function specifiers are not valid in a declaration of main",
                declarator.span,
            ));
        }
        if specs.alignment.is_some() && declarator.ty.is_function() {
            return Err(Diagnostic::error(
                "_Alignas is not valid on functions",
                declarator.span,
            ));
        }
        if declarator.vla_bounds.iter().any(|bound| bound.is_some()) {
            return Err(Diagnostic::error(
                "file-scope declarations cannot have variable length array type",
                declarator.span,
            ));
        }
        if declarator.function_params.is_none()
            || self.at(TokenKind::Equal)
            || self.at(TokenKind::Comma)
            || self.at(TokenKind::Semicolon)
        {
            externals.extend(self.finish_external_declaration_list(declarator, &specs)?);
            return Ok(());
        }

        let ResolvedDeclarator {
            name,
            ty,
            vla_bounds,
            span: base_span,
            function_params,
            ..
        } = declarator;
        if specs.storage_class == Some(ParsedStorageClass::Typedef) {
            externals.extend(
                self.finish_declarator_list(
                    name,
                    ty,
                    specs.base_type.clone(),
                    vla_bounds,
                    base_span,
                    specs.storage_class,
                    specs.is_inline,
                    specs.alignment,
                    DeclContext::FileScope,
                )?
                .into_iter()
                .map(ExternalDeclaration::ObjectDeclaration),
            );
            return Ok(());
        }
        let ParsedFunctionInfo {
            mut params,
            is_variadic,
            parameter_tags,
            parameter_enum_constants,
            old_style,
        } = function_params.expect("non-function declarators were completed above");
        self.validate_function_decl_specifiers(&specs, base_span)?;
        let linkage = self.declare_function_symbol(
            &name,
            &ty,
            specs.storage_class,
            base_span,
            DeclContext::FileScope,
        )?;
        let return_type = match ty.unqualified() {
            CType::Function(return_type, _, _) => (**return_type).clone(),
            _ => unreachable!("outermost function declarator must produce a function type"),
        };
        match return_type.unqualified() {
            CType::Array(_, _) => {
                return Err(Diagnostic::error(
                    "a function cannot return an array type",
                    base_span,
                ));
            }
            CType::Function(..) => {
                return Err(Diagnostic::error(
                    "a function cannot return a function type",
                    base_span,
                ));
            }
            _ => {}
        }
        if self.at(TokenKind::Eof) {
            return Err(Diagnostic::error(
                format!("function header for {name} is missing a body or semicolon"),
                base_span,
            ));
        }
        if let Some(param) = params.iter().find(|param| param.prototype_vla_star) {
            return Err(Diagnostic::error(
                "[*] is only valid in a function declaration with prototype scope",
                param.span,
            ));
        }
        if let Some(param) = params
            .iter()
            .find(|param| param.ty != CType::Void && param.name.is_none())
        {
            return Err(Diagnostic::error(
                "function definition parameters must have names",
                param.span,
            ));
        }
        if !matches!(return_type.unqualified(), CType::Void) && !self.type_is_complete(&return_type)
        {
            return Err(Diagnostic::error(
                "a function definition must return void or a complete object type",
                base_span,
            ));
        }
        self.push_block_scope();
        self.current_tag_scope_mut().extend(parameter_tags);
        for (name, value) in parameter_enum_constants {
            let entry = self.current_scope_mut().entry(name).or_default();
            entry.ordinary = Some(SymbolKind::EnumConstant);
            entry.enum_constant = Some(value);
        }
        if old_style {
            self.parse_old_style_parameter_declarations(&mut params, &name, base_span)?;
        }
        for param in &params {
            if let Some(name) = &param.name {
                if old_style && self.current_scope_entry(name).is_some() {
                    continue;
                }
                self.declare_object_symbol(
                    name,
                    &param.ty,
                    None,
                    param.span,
                    DeclContext::Parameter,
                )?;
            }
        }
        let func_name_ty = CType::array_of(
            CType::qualified(
                CType::Char,
                TypeQualifiers {
                    is_const: true,
                    ..TypeQualifiers::default()
                },
            ),
            name.len() + 1,
        );
        self.declare_object_symbol(
            "__func__",
            &func_name_ty,
            Some(ParsedStorageClass::Static),
            base_span,
            DeclContext::BlockScope,
        )?;
        let body = self.parse_block()?;
        self.pop_block_scope();
        self.validate_function_labels(&body)?;
        let span = base_span.merge(body.span);
        externals.push(ExternalDeclaration::Function(FunctionDef {
            name,
            return_type,
            return_type_span,
            params,
            is_variadic,
            storage_class: specs.storage_class.and_then(ast_storage_class),
            linkage,
            is_inline: specs.is_inline,
            is_noreturn: specs.is_noreturn,
            has_prototype: !old_style,
            body,
            span,
        }));
        Ok(())
    }

    fn finish_external_declaration_list(
        &mut self,
        first: ResolvedDeclarator,
        specs: &DeclarationSpecifiers,
    ) -> Result<Vec<ExternalDeclaration>, Diagnostic> {
        let declaration_base = specs.base_type.clone();
        let mut declarations = Vec::new();
        self.finish_single_external_declarator(&mut declarations, first, specs)?;
        while self.eat(TokenKind::Comma) {
            let mut declarator =
                self.parse_declarator(declaration_base.clone(), DeclContext::FileScope)?;
            declarator.vla_bounds.extend(specs.vla_bounds.clone());
            self.finish_single_external_declarator(&mut declarations, declarator, specs)?;
        }
        let end = self.expect(TokenKind::Semicolon)?.span;
        for declaration in &mut declarations {
            match declaration {
                ExternalDeclaration::FunctionDeclaration(declaration) => {
                    declaration.span = declaration.span.merge(end);
                }
                ExternalDeclaration::ObjectDeclaration(declaration) => {
                    declaration.span = declaration.span.merge(end);
                }
                ExternalDeclaration::Function(_) => unreachable!(),
            }
        }
        Ok(declarations)
    }

    fn finish_single_external_declarator(
        &mut self,
        declarations: &mut Vec<ExternalDeclaration>,
        declarator: ResolvedDeclarator,
        specs: &DeclarationSpecifiers,
    ) -> Result<(), Diagnostic> {
        let ResolvedDeclarator {
            name,
            ty,
            vla_bounds,
            function_params,
            span,
            ..
        } = declarator;
        if specs.storage_class == Some(ParsedStorageClass::Typedef) {
            let mut ignored = Vec::new();
            let init = self
                .eat(TokenKind::Equal)
                .then(|| self.parse_initializer())
                .transpose()?;
            self.finish_single_declarator(
                &mut ignored,
                name,
                ty,
                vla_bounds,
                specs.storage_class,
                specs.alignment,
                init,
                false,
                span,
                DeclContext::FileScope,
            )?;
            return Ok(());
        }
        if ty.is_function() {
            if specs.alignment.is_some() {
                return Err(Diagnostic::error(
                    "_Alignas is not valid on functions",
                    span,
                ));
            }
            if self.at(TokenKind::Equal) {
                return Err(Diagnostic::error(
                    "function declaration cannot have an initializer",
                    span,
                ));
            }
            self.validate_function_decl_specifiers(specs, span)?;
            let linkage = self.declare_function_symbol(
                &name,
                &ty,
                specs.storage_class,
                span,
                DeclContext::FileScope,
            )?;
            let declaration = if let Some(ParsedFunctionInfo {
                params,
                is_variadic,
                old_style,
                ..
            }) = function_params
            {
                if old_style && !params.is_empty() {
                    return Err(Diagnostic::error(
                        "an identifier-list function declarator is only valid in a definition",
                        span,
                    ));
                }
                let return_type = match ty.unqualified() {
                    CType::Function(return_type, _, _) => (**return_type).clone(),
                    _ => unreachable!(),
                };
                match return_type.unqualified() {
                    CType::Array(..) => {
                        return Err(Diagnostic::error(
                            "a function cannot return an array type",
                            span,
                        ));
                    }
                    CType::Function(..) => {
                        return Err(Diagnostic::error(
                            "a function cannot return a function type",
                            span,
                        ));
                    }
                    _ => {}
                }
                FunctionDecl {
                    name,
                    return_type,
                    params,
                    is_variadic,
                    storage_class: specs.storage_class.and_then(ast_storage_class),
                    linkage,
                    is_inline: specs.is_inline,
                    is_noreturn: specs.is_noreturn,
                    has_prototype: !old_style,
                    span,
                }
            } else {
                self.build_function_declaration(name, ty, span, &specs, linkage)?
            };
            declarations.push(ExternalDeclaration::FunctionDeclaration(declaration));
            return Ok(());
        }
        if specs.is_inline || specs.is_noreturn {
            return Err(Diagnostic::error(
                "function specifiers are only valid on function declarators",
                span,
            ));
        }
        let (init, predeclared) = if self.eat(TokenKind::Equal) {
            self.declare_object_symbol(
                &name,
                &ty,
                specs.storage_class,
                span,
                DeclContext::FileScope,
            )?;
            (Some(self.parse_initializer()?), true)
        } else {
            (None, false)
        };
        let mut objects = Vec::new();
        self.finish_single_declarator(
            &mut objects,
            name,
            ty,
            vla_bounds,
            specs.storage_class,
            specs.alignment,
            init,
            predeclared,
            span,
            DeclContext::FileScope,
        )?;
        declarations.extend(
            objects
                .into_iter()
                .map(ExternalDeclaration::ObjectDeclaration),
        );
        Ok(())
    }

    fn build_function_declaration(
        &self,
        name: String,
        ty: CType,
        span: Span,
        specs: &DeclarationSpecifiers,
        linkage: Linkage,
    ) -> Result<FunctionDecl, Diagnostic> {
        let DeclarationSpecifiers {
            storage_class,
            is_inline,
            is_noreturn,
            ..
        } = *specs;
        let (return_type, params, is_variadic) = match ty.unqualified() {
            CType::Function(return_type, params, is_variadic) => {
                ((**return_type).clone(), params.to_vec(), *is_variadic)
            }
            _ => return Err(Diagnostic::error("expected function declarator", span)),
        };
        match return_type.unqualified() {
            CType::Array(_, _) => {
                return Err(Diagnostic::error(
                    "a function cannot return an array type",
                    span,
                ));
            }
            CType::Function(..) => {
                return Err(Diagnostic::error(
                    "a function cannot return a function type",
                    span,
                ));
            }
            _ => {}
        }
        let params = params
            .into_iter()
            .map(|ty| Parameter {
                name: None,
                ty,
                vla_bounds: Vec::new(),
                static_array_bound: None,
                prototype_vla_star: false,
                adjusted_from_array_or_function: false,
                storage_class: None,
                span,
            })
            .collect();
        Ok(FunctionDecl {
            name,
            return_type,
            params,
            is_variadic,
            storage_class: storage_class.and_then(ast_storage_class),
            linkage,
            is_inline,
            is_noreturn,
            has_prototype: true,
            span,
        })
    }

    fn parse_parameter_list(&mut self) -> Result<ParsedFunctionInfo, Diagnostic> {
        self.expect(TokenKind::LParen)?;
        self.push_block_scope();
        let result = (|| {
            if self.at(TokenKind::RParen) {
                return Ok((Vec::new(), false));
            }
            if self.at_keyword(Keyword::Void) && self.peek_kind(1) == Some(&TokenKind::RParen) {
                let void_tok = self.bump().clone();
                return Ok((
                    vec![Parameter {
                        name: None,
                        ty: CType::Void,
                        vla_bounds: Vec::new(),
                        static_array_bound: None,
                        prototype_vla_star: false,
                        adjusted_from_array_or_function: false,
                        storage_class: None,
                        span: void_tok.span,
                    }],
                    false,
                ));
            }

            let mut params = Vec::new();
            let mut is_variadic = false;
            loop {
                self.skip_gnu_attributes()?;
                if self.eat(TokenKind::Ellipsis) {
                    if params.is_empty() {
                        return Err(Diagnostic::error(
                            "a variadic function requires at least one fixed parameter before ...",
                            self.prev_span(),
                        ));
                    }
                    is_variadic = true;
                    break;
                }
                let start = self.current_span();
                let specs = self.parse_declaration_specifiers(DeclContext::Parameter)?;
                if self.at(TokenKind::LBrace) || self.at(TokenKind::Eof) {
                    return Err(Diagnostic::error(
                        "function parameter list is missing a closing parenthesis",
                        self.current_span(),
                    ));
                }
                if self.at(TokenKind::Comma) || self.at(TokenKind::RParen) {
                    self.validate_parameter_decl_specifiers(&specs, start)?;
                    let ty = self.adjust_parameter_type(specs.base_type);
                    self.validate_parameter_type(&ty, start)?;
                    params.push(Parameter {
                        name: None,
                        ty,
                        vla_bounds: specs.vla_bounds,
                        static_array_bound: None,
                        prototype_vla_star: false,
                        adjusted_from_array_or_function: false,
                        storage_class: specs.storage_class.and_then(ast_storage_class),
                        span: start,
                    });
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                    continue;
                }
                let declarator = self.parse_declarator_tree_inner(DeclContext::Parameter, true)?;
                let prototype_vla_star = Self::parsed_declarator_has_vla_star(&declarator);
                let ResolvedDeclarator {
                    name,
                    ty,
                    mut vla_bounds,
                    static_array_bound,
                    span,
                    ..
                } = self.apply_parsed_declarator(declarator, specs.base_type.clone())?;
                vla_bounds.extend(specs.vla_bounds.clone());
                let parameter_span = start.merge(span);
                self.validate_parameter_decl_specifiers(&specs, parameter_span)?;
                if !name.is_empty() {
                    self.declare_object_symbol(
                        &name,
                        &ty,
                        specs.storage_class,
                        parameter_span,
                        DeclContext::Parameter,
                    )?;
                }
                let adjusted_ty = self.adjust_parameter_type(ty.clone());
                self.validate_parameter_type(&adjusted_ty, parameter_span)?;
                params.push(Parameter {
                    name: (!name.is_empty()).then_some(name),
                    adjusted_from_array_or_function: matches!(
                        ty.unqualified(),
                        CType::Array(_, _) | CType::Function(..)
                    ),
                    ty: adjusted_ty,
                    vla_bounds,
                    static_array_bound,
                    prototype_vla_star,
                    storage_class: specs.storage_class.and_then(ast_storage_class),
                    span: parameter_span,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            Ok((params, is_variadic))
        })();
        let parameter_tags = self.block_tag_scopes.last().cloned().unwrap_or_default();
        let parameter_enum_constants = self
            .block_scopes
            .last()
            .into_iter()
            .flat_map(|scope| scope.iter())
            .filter_map(|(name, entry)| entry.enum_constant.map(|value| (name.clone(), value)))
            .collect();
        self.pop_block_scope();
        result.map(|(params, is_variadic)| ParsedFunctionInfo {
            params,
            is_variadic,
            parameter_tags,
            parameter_enum_constants,
            old_style: false,
        })
    }

    pub(super) fn parse_function_parameter_clause(
        &mut self,
    ) -> Result<ParsedFunctionInfo, Diagnostic> {
        if self.peek_kind(0) == Some(&TokenKind::LParen)
            && self.peek_kind(1) == Some(&TokenKind::RParen)
        {
            self.bump();
            return Ok(ParsedFunctionInfo {
                old_style: true,
                ..ParsedFunctionInfo::default()
            });
        }
        let identifier_list = matches!(self.peek_kind(1), Some(TokenKind::Identifier(name))
            if self.lookup_typedef_name(name).is_none());
        if !identifier_list {
            return self.parse_parameter_list();
        }
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        let mut names = HashSet::new();
        loop {
            let token = self.bump().clone();
            let TokenKind::Identifier(name) = token.kind else {
                return Err(Diagnostic::error(
                    "expected identifier in old-style function parameter list",
                    token.span,
                ));
            };
            if !names.insert(name.clone()) {
                return Err(Diagnostic::error(
                    "duplicate name in old-style function parameter list",
                    token.span,
                ));
            }
            params.push(Parameter {
                name: Some(name),
                ty: CType::Int,
                vla_bounds: Vec::new(),
                static_array_bound: None,
                prototype_vla_star: false,
                adjusted_from_array_or_function: false,
                storage_class: None,
                span: token.span,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok(ParsedFunctionInfo {
            params,
            old_style: true,
            ..ParsedFunctionInfo::default()
        })
    }

    fn parse_old_style_parameter_declarations(
        &mut self,
        params: &mut [Parameter],
        function_name: &str,
        header_span: Span,
    ) -> Result<(), Diagnostic> {
        let mut declared = HashSet::new();
        while !self.at(TokenKind::LBrace) {
            if self.at(TokenKind::Eof) {
                return Err(Diagnostic::error(
                    format!("function header for {function_name} is missing a body or semicolon"),
                    header_span,
                ));
            }
            let declarations = self.parse_declaration_list(false)?;
            if declarations.is_empty() {
                return Err(Diagnostic::error(
                    "expected old-style parameter declaration",
                    self.current_span(),
                ));
            }
            for declaration in declarations {
                let Some(param) = params
                    .iter_mut()
                    .find(|param| param.name.as_deref() == Some(&declaration.name))
                else {
                    return Err(Diagnostic::error(
                        format!(
                            "declaration for {} does not name an old-style parameter",
                            declaration.name
                        ),
                        declaration.span,
                    ));
                };
                if !declared.insert(declaration.name.clone()) {
                    return Err(Diagnostic::error(
                        format!(
                            "duplicate declaration of old-style parameter {}",
                            declaration.name
                        ),
                        declaration.span,
                    ));
                }
                if declaration.init.is_some()
                    || !matches!(
                        declaration.storage_class,
                        None | Some(StorageClass::Register)
                    )
                {
                    return Err(Diagnostic::error(
                        "old-style parameter declarations cannot have initializers or this storage class",
                        declaration.span,
                    ));
                }
                let adjusted = self.adjust_parameter_type(declaration.ty.clone());
                self.validate_parameter_type(&adjusted, declaration.span)?;
                param.ty = adjusted;
                param.vla_bounds = declaration.vla_bounds;
                param.adjusted_from_array_or_function = matches!(
                    declaration.ty.unqualified(),
                    CType::Array(..) | CType::Function(..)
                );
                param.storage_class = declaration.storage_class;
                param.span = declaration.span;
                if let Some(entry) = self.current_scope_mut().get_mut(&declaration.name) {
                    entry.ordinary_ty = Some(param.ty.clone());
                }
            }
        }
        if let Some(param) = params.iter().find(|param| {
            param
                .name
                .as_ref()
                .is_some_and(|name| !declared.contains(name))
        }) {
            let name = param
                .name
                .as_deref()
                .expect("old-style parameters always have names");
            return Err(Diagnostic::error(
                format!("old-style parameter {name} is missing its declaration"),
                param.span,
            ));
        }
        Ok(())
    }

    pub(super) fn parse_block_declaration_items(&mut self) -> Result<Vec<BlockItem>, Diagnostic> {
        self.skip_gnu_attributes()?;
        let specs = self.parse_declaration_specifiers(DeclContext::BlockScope)?;
        if self.eat(TokenKind::Semicolon) {
            if specs.storage_class.is_some() || specs.is_inline || specs.is_noreturn {
                return Err(Diagnostic::error(
                    "storage class specifiers and inline require a declarator",
                    self.prev_span(),
                ));
            }
            if !specs.declares_tag_or_enumerators {
                return Err(Diagnostic::error(
                    "declaration does not declare a declarator, tag, or enumeration constant",
                    self.prev_span(),
                ));
            }
            return Ok(Vec::new());
        }

        let mut items = Vec::new();
        loop {
            let mut declarator =
                self.parse_declarator(specs.base_type.clone(), DeclContext::BlockScope)?;
            declarator.vla_bounds.extend(specs.vla_bounds.clone());
            let (init, predeclared) = if self.eat(TokenKind::Equal) {
                let predeclared = declarator.function_params.is_none()
                    && specs.storage_class != Some(ParsedStorageClass::Typedef);
                if predeclared {
                    self.declare_object_symbol(
                        &declarator.name,
                        &declarator.ty,
                        specs.storage_class,
                        declarator.span,
                        DeclContext::BlockScope,
                    )?;
                }
                (Some(self.parse_initializer()?), predeclared)
            } else {
                (None, false)
            };
            self.finish_single_block_declarator(&mut items, declarator, &specs, init, predeclared)?;
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        let end = self.expect(TokenKind::Semicolon)?.span;
        for item in &mut items {
            match item {
                BlockItem::Declaration(declaration) => {
                    declaration.span = declaration.span.merge(end);
                }
                BlockItem::FunctionDeclaration(function_decl) => {
                    function_decl.span = function_decl.span.merge(end);
                }
                BlockItem::Statement(_) => unreachable!(),
            }
        }
        Ok(items)
    }

    pub(super) fn parse_declaration_list(
        &mut self,
        is_for_initializer: bool,
    ) -> Result<Vec<Declaration>, Diagnostic> {
        self.skip_gnu_attributes()?;
        let specs = self.parse_declaration_specifiers(DeclContext::BlockScope)?;
        if is_for_initializer
            && !matches!(
                specs.storage_class,
                None | Some(ParsedStorageClass::Auto | ParsedStorageClass::Register)
            )
        {
            return Err(Diagnostic::error(
                "a for-loop declaration may only use auto or register storage class",
                self.prev_span(),
            ));
        }
        if self.eat(TokenKind::Semicolon) {
            if specs.storage_class.is_some() || specs.is_inline || specs.is_noreturn {
                return Err(Diagnostic::error(
                    "storage class specifiers and inline require a declarator",
                    self.prev_span(),
                ));
            }
            if !specs.declares_tag_or_enumerators {
                return Err(Diagnostic::error(
                    "declaration does not declare a declarator, tag, or enumeration constant",
                    self.prev_span(),
                ));
            }
            return Ok(Vec::new());
        }
        let declaration_base = specs.base_type.clone();
        let ResolvedDeclarator {
            name,
            ty,
            mut vla_bounds,
            span,
            function_params: _function_params,
            ..
        } = self.parse_declarator(declaration_base.clone(), DeclContext::BlockScope)?;
        vla_bounds.extend(specs.vla_bounds.clone());
        self.finish_declarator_list(
            name,
            ty,
            declaration_base,
            vla_bounds,
            span,
            specs.storage_class,
            specs.is_inline,
            specs.alignment,
            DeclContext::BlockScope,
        )
    }

    fn finish_declarator_list(
        &mut self,
        first_name: String,
        first_ty: CType,
        declaration_base: CType,
        first_vla_bounds: Vec<Option<Expr>>,
        first_span: Span,
        storage_class: Option<ParsedStorageClass>,
        is_inline: bool,
        alignment: Option<usize>,
        context: DeclContext,
    ) -> Result<Vec<Declaration>, Diagnostic> {
        if is_inline {
            return Err(Diagnostic::error(
                "inline is only valid on function declarations",
                first_span,
            ));
        }
        let is_typedef = storage_class == Some(ParsedStorageClass::Typedef);
        let mut declarations = Vec::new();
        let (first_init, first_predeclared) = if self.eat(TokenKind::Equal) {
            let predeclared = !is_typedef && !first_ty.is_function();
            if predeclared {
                self.declare_object_symbol(
                    &first_name,
                    &first_ty,
                    storage_class,
                    first_span,
                    context,
                )?;
            }
            (Some(self.parse_initializer()?), predeclared)
        } else {
            (None, false)
        };
        self.finish_single_declarator(
            &mut declarations,
            first_name,
            first_ty.clone(),
            first_vla_bounds,
            storage_class,
            alignment,
            first_init,
            first_predeclared,
            first_span,
            context,
        )?;
        while self.eat(TokenKind::Comma) {
            let ResolvedDeclarator {
                name,
                ty,
                vla_bounds,
                span,
                function_params,
                ..
            } = self.parse_declarator(declaration_base.clone(), context)?;
            let _ = function_params;
            let (init, predeclared) = if self.eat(TokenKind::Equal) {
                let predeclared = !is_typedef && !ty.is_function();
                if predeclared {
                    self.declare_object_symbol(&name, &ty, storage_class, span, context)?;
                }
                (Some(self.parse_initializer()?), predeclared)
            } else {
                (None, false)
            };
            self.finish_single_declarator(
                &mut declarations,
                name,
                ty,
                vla_bounds,
                storage_class,
                alignment,
                init,
                predeclared,
                span,
                context,
            )?;
        }
        let end = self.expect(TokenKind::Semicolon)?.span;
        if !is_typedef {
            for declaration in &mut declarations {
                declaration.span = declaration.span.merge(end);
            }
        }
        Ok(declarations)
    }

    fn finish_single_block_declarator(
        &mut self,
        items: &mut Vec<BlockItem>,
        declarator: ResolvedDeclarator,
        specs: &DeclarationSpecifiers,
        init: Option<Initializer>,
        symbol_predeclared: bool,
    ) -> Result<(), Diagnostic> {
        let ResolvedDeclarator {
            name,
            ty,
            vla_bounds,
            function_params,
            span,
            ..
        } = declarator;
        let DeclarationSpecifiers {
            storage_class,
            is_inline,
            is_noreturn,
            alignment,
            ..
        } = *specs;
        if is_noreturn && storage_class == Some(ParsedStorageClass::Typedef) {
            return Err(Diagnostic::error(
                "_Noreturn is not valid in a typedef declaration",
                span,
            ));
        }
        if is_noreturn && function_params.is_none() {
            return Err(Diagnostic::error(
                "_Noreturn is only valid on functions",
                span,
            ));
        }
        if alignment.is_some() && function_params.is_some() {
            return Err(Diagnostic::error(
                "_Alignas is not valid on functions",
                span,
            ));
        }
        if is_inline
            && (function_params.is_none() || storage_class == Some(ParsedStorageClass::Typedef))
        {
            return Err(Diagnostic::error(
                "inline is only valid on function declarations",
                span,
            ));
        }
        if function_params.is_some() && storage_class != Some(ParsedStorageClass::Typedef) {
            if init.is_some() {
                return Err(Diagnostic::error(
                    "function declaration cannot have an initializer",
                    span,
                ));
            }
            if storage_class == Some(ParsedStorageClass::Static) {
                return Err(Diagnostic::error(
                    "a block-scope function declaration may only explicitly specify extern",
                    span,
                ));
            }
            self.validate_function_decl_specifiers(&specs, span)?;
            let linkage = self.declare_function_symbol(
                &name,
                &ty,
                storage_class,
                span,
                DeclContext::BlockScope,
            )?;
            let decl = self.build_function_declaration(name, ty, span, specs, linkage)?;
            self.block_linkage_declarations
                .push(ExternalDeclaration::FunctionDeclaration(decl.clone()));
            items.push(BlockItem::FunctionDeclaration(decl));
            return Ok(());
        }
        let mut declarations = Vec::new();
        self.finish_single_declarator(
            &mut declarations,
            name,
            ty,
            vla_bounds,
            storage_class,
            alignment,
            init,
            symbol_predeclared,
            span,
            DeclContext::BlockScope,
        )?;
        items.extend(declarations.into_iter().map(BlockItem::Declaration));
        Ok(())
    }

    fn finish_single_declarator(
        &mut self,
        declarations: &mut Vec<Declaration>,
        name: String,
        ty: CType,
        vla_bounds: Vec<Option<Expr>>,
        storage_class: Option<ParsedStorageClass>,
        alignment: Option<usize>,
        init: Option<Initializer>,
        symbol_predeclared: bool,
        span: Span,
        context: DeclContext,
    ) -> Result<(), Diagnostic> {
        let ty = if context == DeclContext::FileScope {
            init.as_ref().map_or_else(
                || ty.clone(),
                |initializer| self.complete_file_scope_array_bound(&ty, initializer),
            )
        } else {
            ty
        };
        if symbol_predeclared && let Some(entry) = self.current_scope_mut().get_mut(&name) {
            entry.ordinary_ty = Some(ty.clone());
        }
        self.validate_restrict_usage(&ty, span)?;
        if alignment.is_some() && storage_class == Some(ParsedStorageClass::Typedef) {
            return Err(Diagnostic::error(
                "_Alignas cannot appear in a typedef",
                span,
            ));
        }
        if alignment.is_some() && storage_class == Some(ParsedStorageClass::Register) {
            return Err(Diagnostic::error(
                "_Alignas cannot appear on a register object",
                span,
            ));
        }
        if let Some(alignment) = alignment
            && alignment < self.type_align_of(&ty)?
        {
            return Err(Diagnostic::error(
                "_Alignas cannot request an alignment weaker than the type's natural alignment",
                span,
            ));
        }
        let has_variably_modified_type = vla_bounds.iter().any(|bound| bound.is_some());
        if !has_variably_modified_type
            && let CType::Array(inner, _) = ty.unqualified()
            && !self.type_is_complete(inner)
        {
            return Err(Diagnostic::error(
                format!("array element has incomplete type {inner}"),
                span,
            ));
        }
        let has_vla_object_type =
            has_variably_modified_type && matches!(ty.unqualified(), CType::Array(_, _));
        if has_variably_modified_type {
            if context != DeclContext::BlockScope {
                return Err(Diagnostic::error(
                    "variable length array type is only supported at block scope or in parameters",
                    span,
                ));
            }
            if has_vla_object_type && init.is_some() {
                return Err(Diagnostic::error(
                    "variable length array objects cannot have an initializer",
                    span,
                ));
            }
            if context == DeclContext::BlockScope
                && storage_class == Some(ParsedStorageClass::Extern)
            {
                return Err(Diagnostic::error(
                    "an identifier with linkage cannot have variably modified type",
                    span,
                ));
            }
        }
        if storage_class == Some(ParsedStorageClass::Typedef) {
            if let Some(initializer) = init.as_ref() {
                return Err(Diagnostic::error(
                    "typedef declaration cannot have an initializer",
                    initializer.span(),
                ));
            }
            let mut captured_bounds = Vec::with_capacity(vla_bounds.len());
            for bound in vla_bounds {
                let Some(bound) = bound else {
                    captured_bounds.push(None);
                    continue;
                };
                let hidden_name = format!(
                    "__cboxes_vla_typedef_bound_{}",
                    self.next_hidden_vla_bound_id
                );
                self.next_hidden_vla_bound_id += 1;
                self.declare_object_symbol(
                    &hidden_name,
                    &CType::UnsignedLong,
                    None,
                    span,
                    DeclContext::BlockScope,
                )?;
                declarations.push(Declaration {
                    name: hidden_name.clone(),
                    ty: CType::UnsignedLong,
                    vla_bounds: Vec::new(),
                    storage_class: None,
                    linkage: None,
                    alignment: None,
                    init: Some(Initializer::Expr(bound)),
                    declarator_span: span,
                    span,
                });
                captured_bounds.push(Some(Expr::Variable(hidden_name, span)));
            }
            self.declare_typedef_name(&name, ty, captured_bounds, span)?;
            return Ok(());
        }
        if matches!(ty.unqualified(), CType::Void) {
            return Err(Diagnostic::error("an object cannot have void type", span));
        }
        if context == DeclContext::BlockScope
            && storage_class == Some(ParsedStorageClass::Extern)
            && init.is_some()
        {
            return Err(Diagnostic::error(
                "block-scope extern declaration cannot have an initializer",
                span,
            ));
        }
        if !symbol_predeclared {
            self.declare_object_symbol(&name, &ty, storage_class, span, context)?;
        }
        let linkage = self
            .current_scope_entry(&name)
            .and_then(|entry| entry.ordinary_linkage);
        let declaration = Declaration {
            name,
            ty,
            vla_bounds,
            storage_class: storage_class.and_then(ast_storage_class),
            linkage,
            alignment,
            init,
            declarator_span: span,
            span,
        };
        if context == DeclContext::BlockScope && storage_class == Some(ParsedStorageClass::Extern) {
            self.block_linkage_declarations
                .push(ExternalDeclaration::ObjectDeclaration(declaration.clone()));
        }
        declarations.push(declaration);
        Ok(())
    }

    fn complete_file_scope_array_bound(&self, ty: &CType, initializer: &Initializer) -> CType {
        let CType::Array(inner, 0) = ty.unqualified() else {
            return ty.clone();
        };
        let len = match initializer {
            Initializer::Expr(Expr::StringLiteral(text, _)) if inner.is_character() => {
                text.narrow_len() + 1
            }
            Initializer::Expr(Expr::WideStringLiteral(text, _))
                if *inner.unqualified() == CType::Int =>
            {
                text.utf32_units().len() + 1
            }
            Initializer::Expr(Expr::Utf16StringLiteral(text, _))
                if *inner.unqualified() == CType::UnsignedShort =>
            {
                text.utf16_units().len() + 1
            }
            Initializer::Expr(Expr::Utf32StringLiteral(text, _))
                if *inner.unqualified() == CType::UnsignedInt =>
            {
                text.utf32_units().len() + 1
            }
            Initializer::List { items, .. } => {
                let scalar_slots = self.initializer_scalar_slots(inner).unwrap_or(1).max(1);
                let inner_is_aggregate = matches!(
                    inner.unqualified(),
                    CType::Array(_, _) | CType::Struct(_, _) | CType::Union(_, _)
                );
                let mut next_index = 0usize;
                let mut scalar_offset = 0usize;
                let mut max_len = 0usize;
                for item in items {
                    if let Some(index) = item.designators.iter().find_map(|designator| {
                        if let Designator::Index(index, _) = designator {
                            Some(*index)
                        } else {
                            None
                        }
                    }) {
                        next_index = index;
                        scalar_offset = 0;
                    }
                    let consumes_whole_element = !inner_is_aggregate
                        || !item.designators.is_empty()
                        || matches!(
                            item.initializer,
                            Initializer::List { .. }
                                | Initializer::Expr(Expr::CompoundLiteral { .. })
                                | Initializer::Expr(Expr::StringLiteral(..))
                                | Initializer::Expr(Expr::WideStringLiteral(..))
                                | Initializer::Expr(Expr::Utf16StringLiteral(..))
                                | Initializer::Expr(Expr::Utf32StringLiteral(..))
                        );
                    max_len = max_len.max(next_index.saturating_add(1));
                    if consumes_whole_element {
                        next_index = next_index.saturating_add(1);
                        scalar_offset = 0;
                    } else {
                        scalar_offset += 1;
                        if scalar_offset == scalar_slots {
                            next_index = next_index.saturating_add(1);
                            scalar_offset = 0;
                        }
                    }
                }
                max_len
            }
            _ => return ty.clone(),
        };
        CType::qualified(
            CType::array_of((**inner).clone(), len),
            ty.top_level_qualifiers(),
        )
    }

    fn initializer_scalar_slots(&self, ty: &CType) -> Option<usize> {
        match ty.unqualified() {
            CType::Array(inner, len) if *len != 0 => {
                self.initializer_scalar_slots(inner)?.checked_mul(*len)
            }
            CType::Struct(id, _) => {
                self.records
                    .get(id)?
                    .members
                    .iter()
                    .try_fold(0usize, |total, member| {
                        if member.bit_width == Some(0)
                            || matches!(member.ty.unqualified(), CType::Array(_, 0))
                        {
                            Some(total)
                        } else {
                            total.checked_add(self.initializer_scalar_slots(&member.ty)?)
                        }
                    })
            }
            CType::Union(id, _) => self
                .records
                .get(id)?
                .members
                .iter()
                .find(|member| member.bit_width != Some(0))
                .and_then(|member| self.initializer_scalar_slots(&member.ty)),
            _ => Some(1),
        }
    }

    pub(super) fn parse_initializer(&mut self) -> Result<Initializer, Diagnostic> {
        if self.eat(TokenKind::LBrace) {
            let start = self.prev_span();
            let mut items = Vec::new();
            if self.at(TokenKind::RBrace) {
                return Err(Diagnostic::error(
                    "initializer list cannot be empty in C11",
                    start.merge(self.current_span()),
                ));
            }
            loop {
                let item_start = self.current_span();
                let designators = self.parse_initializer_designators()?;
                let initializer = self.parse_initializer()?;
                let span = if designators.is_empty() {
                    initializer.span()
                } else {
                    item_start.merge(initializer.span())
                };
                items.push(InitializerItem {
                    designators,
                    initializer,
                    span,
                });
                if !self.eat(TokenKind::Comma) || self.at(TokenKind::RBrace) {
                    break;
                }
            }
            let end = self.expect(TokenKind::RBrace)?.span;
            Ok(Initializer::List {
                items,
                span: start.merge(end),
            })
        } else {
            Ok(Initializer::Expr(self.parse_assignment()?))
        }
    }

    fn parse_initializer_designators(&mut self) -> Result<Vec<Designator>, Diagnostic> {
        let mut designators = Vec::new();
        loop {
            if self.eat(TokenKind::Dot) {
                let token = self.bump().clone();
                let TokenKind::Identifier(name) = token.kind else {
                    return Err(Diagnostic::error(
                        "expected member name in initializer designator",
                        token.span,
                    ));
                };
                designators.push(Designator::Member(name, token.span));
                continue;
            }
            if self.eat(TokenKind::LBracket) {
                let start = self.prev_span();
                let expr = self.parse_assignment()?;
                let value = self.eval_integer_constant_expr(&expr)?;
                if value < 0 {
                    return Err(Diagnostic::error(
                        "array designator index must be non-negative",
                        expr.span(),
                    ));
                }
                let end = self.expect(TokenKind::RBracket)?.span;
                designators.push(Designator::Index(
                    usize::try_from(value).map_err(|_| {
                        Diagnostic::error("array designator index is out of range", expr.span())
                    })?,
                    start.merge(end),
                ));
                continue;
            }
            break;
        }
        if !designators.is_empty() {
            self.expect(TokenKind::Equal)?;
        }
        Ok(designators)
    }

    pub(super) fn parse_static_assertion(&mut self) -> Result<(), Diagnostic> {
        let start = self.bump().span;
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_conditional()?;
        self.expect(TokenKind::Comma)?;
        let message = match self.bump().clone() {
            Token {
                kind: TokenKind::StringLiteral(text),
                ..
            }
            | Token {
                kind: TokenKind::Utf8StringLiteral(text),
                ..
            }
            | Token {
                kind: TokenKind::WideStringLiteral(text),
                ..
            }
            | Token {
                kind: TokenKind::Utf16StringLiteral(text),
                ..
            }
            | Token {
                kind: TokenKind::Utf32StringLiteral(text),
                ..
            } => text,
            token => {
                return Err(Diagnostic::error(
                    "_Static_assert requires a string literal message",
                    token.span,
                ));
            }
        };
        self.expect(TokenKind::RParen)?;
        let end = self.expect(TokenKind::Semicolon)?.span;
        if self.eval_typed_integer_constant_expr(&condition)?.value == 0 {
            return Err(Diagnostic::error(
                format!("static assertion failed: {message}"),
                start.merge(end),
            ));
        }
        Ok(())
    }
}
