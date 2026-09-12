//! Declaration specifiers and declarator type construction.

use super::*;

#[derive(Default)]
struct BuiltinTypeSpecifiers {
    void: bool,
    bool: bool,
    char: bool,
    float: bool,
    double: bool,
    complex: bool,
    int: bool,
    short: bool,
    signed: bool,
    unsigned: bool,
    long_count: usize,
}

impl BuiltinTypeSpecifiers {
    fn add(&mut self, keyword: Keyword, span: Span) -> Result<(), Diagnostic> {
        let flag = match keyword {
            Keyword::Void => &mut self.void,
            Keyword::Bool => &mut self.bool,
            Keyword::Char => &mut self.char,
            Keyword::Float => &mut self.float,
            Keyword::Double => &mut self.double,
            Keyword::Complex => &mut self.complex,
            Keyword::Int => &mut self.int,
            Keyword::Short => &mut self.short,
            Keyword::Signed => &mut self.signed,
            Keyword::Unsigned => &mut self.unsigned,
            Keyword::Long => {
                self.long_count += 1;
                return Ok(());
            }
            _ => unreachable!("expected a built-in type keyword"),
        };
        if *flag {
            return Err(Diagnostic::error(
                format!("duplicate type specifier {}", keyword.spelling()),
                span,
            ));
        }
        *flag = true;
        Ok(())
    }

    fn has_any(&self) -> bool {
        self.void
            || self.bool
            || self.char
            || self.float
            || self.double
            || self.complex
            || self.int
            || self.short
            || self.signed
            || self.unsigned
            || self.long_count > 0
    }

    fn finish(&self, start: Span) -> Result<CType, Diagnostic> {
        if self.signed && self.unsigned {
            return Err(Diagnostic::error(
                "type specifier cannot be both signed and unsigned",
                start,
            ));
        }
        if self.void {
            if self.bool
                || self.char
                || self.float
                || self.double
                || self.complex
                || self.int
                || self.short
                || self.long_count > 0
                || self.signed
                || self.unsigned
            {
                return Err(Diagnostic::error(
                    "void cannot be combined with other type specifiers",
                    start,
                ));
            }
            return Ok(CType::Void);
        }
        if self.bool {
            if self.char
                || self.float
                || self.double
                || self.complex
                || self.int
                || self.short
                || self.long_count > 0
                || self.signed
                || self.unsigned
            {
                return Err(Diagnostic::error(
                    "_Bool cannot be combined with other type specifiers",
                    start,
                ));
            }
            return Ok(CType::Bool);
        }
        if self.char {
            if self.float
                || self.double
                || self.complex
                || self.short
                || self.long_count > 0
                || self.int
            {
                return Err(Diagnostic::error(
                    "char cannot be combined with float, double, short, long, or int",
                    start,
                ));
            }
            return Ok(if self.unsigned {
                CType::UnsignedChar
            } else if self.signed {
                CType::SignedChar
            } else {
                CType::Char
            });
        }
        if self.float {
            if self.complex {
                if self.double
                    || self.char
                    || self.int
                    || self.short
                    || self.long_count > 0
                    || self.signed
                    || self.unsigned
                {
                    return Err(Diagnostic::error(
                        "float _Complex cannot be combined with other type specifiers",
                        start,
                    ));
                }
                return Ok(CType::complex_of(CType::Float));
            }
            if self.double
                || self.char
                || self.int
                || self.short
                || self.long_count > 0
                || self.signed
                || self.unsigned
            {
                return Err(Diagnostic::error(
                    "float cannot be combined with other type specifiers",
                    start,
                ));
            }
            return Ok(CType::Float);
        }
        if self.complex {
            if self.char
                || self.int
                || self.short
                || self.signed
                || self.unsigned
                || self.long_count > 1
            {
                return Err(Diagnostic::error(
                    "_Complex cannot be combined with these type specifiers",
                    start,
                ));
            }
            if self.double {
                return Ok(if self.long_count == 1 {
                    CType::complex_of(CType::LongDouble)
                } else {
                    CType::complex_of(CType::Double)
                });
            }
            if self.long_count > 0 {
                return Err(Diagnostic::error("long _Complex requires double", start));
            }
            return Err(Diagnostic::error(
                "_Complex requires float or double",
                start,
            ));
        }
        if self.double {
            if self.char
                || self.int
                || self.short
                || self.signed
                || self.unsigned
                || self.long_count > 1
            {
                return Err(Diagnostic::error(
                    "double cannot be combined with these type specifiers",
                    start,
                ));
            }
            return Ok(if self.long_count == 1 {
                CType::LongDouble
            } else {
                CType::Double
            });
        }
        if self.short {
            if self.long_count > 0 {
                return Err(Diagnostic::error(
                    "short cannot be combined with long",
                    start,
                ));
            }
            return Ok(if self.unsigned {
                CType::UnsignedShort
            } else {
                CType::Short
            });
        }
        if self.long_count > 0 {
            if self.long_count > 2 {
                return Err(Diagnostic::error("only long long is supported", start));
            }
            return Ok(match (self.unsigned, self.long_count) {
                (true, 1) => CType::UnsignedLong,
                (true, 2) => CType::UnsignedLongLong,
                (false, 1) => CType::Long,
                (false, 2) => CType::LongLong,
                _ => unreachable!(),
            });
        }
        if self.int || self.unsigned || self.signed {
            return Ok(if self.unsigned {
                CType::UnsignedInt
            } else {
                CType::Int
            });
        }
        Err(Diagnostic::error("expected type specifier", start))
    }
}

impl<'a> Parser<'a> {
    fn parse_type_qualifiers(&mut self) -> TypeQualifiers {
        let mut qualifiers = TypeQualifiers::default();
        while matches!(
            self.peek_kind(0),
            Some(TokenKind::Keyword(
                Keyword::Atomic | Keyword::Const | Keyword::Restrict | Keyword::Volatile
            ))
        ) {
            match self.bump().kind {
                TokenKind::Keyword(Keyword::Atomic) => qualifiers.is_atomic = true,
                TokenKind::Keyword(Keyword::Const) => qualifiers.is_const = true,
                TokenKind::Keyword(Keyword::Restrict) => qualifiers.is_restrict = true,
                TokenKind::Keyword(Keyword::Volatile) => qualifiers.is_volatile = true,
                _ => unreachable!(),
            }
        }
        qualifiers
    }

    pub(super) fn parse_declaration_specifiers(
        &mut self,
        context: DeclContext,
    ) -> Result<DeclarationSpecifiers, Diagnostic> {
        let start = self.current_span();
        let mut storage_class = None;
        let mut is_inline = false;
        let mut is_noreturn = false;
        let mut alignment = None;
        let mut qualifiers = TypeQualifiers::default();
        let mut direct_type = None;
        let mut direct_vla_bounds = Vec::new();
        let mut saw_atomic_type_specifier = false;
        let mut declares_tag_or_enumerators = false;
        let mut saw_any = false;
        let mut builtin = BuiltinTypeSpecifiers::default();

        loop {
            self.skip_gnu_attributes()?;
            match self.peek_kind(0).cloned() {
                Some(TokenKind::Keyword(Keyword::Auto)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(
                        &mut storage_class,
                        ParsedStorageClass::Auto,
                        self.prev_span(),
                    )?;
                }
                Some(TokenKind::Keyword(Keyword::Extern)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(
                        &mut storage_class,
                        ParsedStorageClass::Extern,
                        self.prev_span(),
                    )?;
                }
                Some(TokenKind::Keyword(Keyword::Register)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(
                        &mut storage_class,
                        ParsedStorageClass::Register,
                        self.prev_span(),
                    )?;
                }
                Some(TokenKind::Keyword(Keyword::Static)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(
                        &mut storage_class,
                        ParsedStorageClass::Static,
                        self.prev_span(),
                    )?;
                }
                Some(TokenKind::Keyword(Keyword::Typedef)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(
                        &mut storage_class,
                        ParsedStorageClass::Typedef,
                        self.prev_span(),
                    )?;
                }
                Some(TokenKind::Keyword(Keyword::Inline)) => {
                    self.bump();
                    saw_any = true;
                    is_inline = true;
                }
                Some(TokenKind::Keyword(Keyword::Noreturn)) => {
                    self.bump();
                    saw_any = true;
                    is_noreturn = true;
                }
                Some(TokenKind::Keyword(Keyword::Alignas)) => {
                    let align = self.parse_alignment_specifier()?;
                    saw_any = true;
                    alignment = Some(alignment.map_or(align, |old: usize| old.max(align)));
                }
                Some(TokenKind::Keyword(Keyword::Atomic))
                    if self.peek_kind(1) == Some(&TokenKind::LParen) =>
                {
                    if direct_type.is_some() || builtin.has_any() {
                        return Err(Diagnostic::error(
                            "_Atomic type specifier cannot be combined with another type specifier",
                            self.current_span(),
                        ));
                    }
                    let atomic_span = self.bump().span;
                    self.expect(TokenKind::LParen)?;
                    let parsed = self.parse_type_name()?;
                    self.expect(TokenKind::RParen)?;
                    if !parsed.vla_bounds.is_empty()
                        || parsed.ty.top_level_qualifiers().is_atomic
                        || matches!(
                            parsed.ty.unqualified(),
                            CType::Array(..) | CType::Function(..) | CType::Void
                        )
                    {
                        return Err(Diagnostic::error(
                            "_Atomic requires a non-atomic, unqualified scalar, structure, or union type",
                            atomic_span,
                        ));
                    }
                    saw_any = true;
                    saw_atomic_type_specifier = true;
                    direct_type = Some(CType::qualified(
                        parsed.ty,
                        TypeQualifiers {
                            is_atomic: true,
                            ..TypeQualifiers::default()
                        },
                    ));
                }
                Some(TokenKind::Keyword(Keyword::Atomic)) => {
                    self.bump();
                    saw_any = true;
                    qualifiers.is_atomic = true;
                }
                Some(TokenKind::Keyword(Keyword::Const)) => {
                    self.bump();
                    saw_any = true;
                    qualifiers.is_const = true;
                }
                Some(TokenKind::Keyword(Keyword::Restrict)) => {
                    self.bump();
                    saw_any = true;
                    qualifiers.is_restrict = true;
                }
                Some(TokenKind::Keyword(Keyword::Volatile)) => {
                    self.bump();
                    saw_any = true;
                    qualifiers.is_volatile = true;
                }
                Some(TokenKind::Keyword(Keyword::Struct)) => {
                    saw_any = true;
                    if direct_type.is_some() || builtin.has_any() {
                        return Err(Diagnostic::error(
                            "struct type cannot be combined with other type specifiers",
                            self.current_span(),
                        ));
                    }
                    let ty = self.parse_record_specifier(RecordKind::Struct)?;
                    declares_tag_or_enumerators = matches!(
                        ty.unqualified(),
                        CType::Struct(_, Some(_)) | CType::Union(_, Some(_))
                    );
                    direct_type = Some(ty);
                }
                Some(TokenKind::Keyword(Keyword::Union)) => {
                    saw_any = true;
                    if direct_type.is_some() || builtin.has_any() {
                        return Err(Diagnostic::error(
                            "union type cannot be combined with other type specifiers",
                            self.current_span(),
                        ));
                    }
                    let ty = self.parse_record_specifier(RecordKind::Union)?;
                    declares_tag_or_enumerators = matches!(
                        ty.unqualified(),
                        CType::Struct(_, Some(_)) | CType::Union(_, Some(_))
                    );
                    direct_type = Some(ty);
                }
                Some(TokenKind::Keyword(Keyword::Enum)) => {
                    saw_any = true;
                    if direct_type.is_some() || builtin.has_any() {
                        return Err(Diagnostic::error(
                            "enum type cannot be combined with other type specifiers",
                            self.current_span(),
                        ));
                    }
                    let ty = self.parse_enum_specifier()?;
                    declares_tag_or_enumerators = match ty.unqualified() {
                        CType::Enum(id, _) => {
                            self.enums.get(id).is_some_and(|enum_ty| enum_ty.complete)
                        }
                        _ => false,
                    };
                    direct_type = Some(ty);
                }
                Some(TokenKind::Keyword(
                    keyword @ (Keyword::Void
                    | Keyword::Bool
                    | Keyword::Char
                    | Keyword::Float
                    | Keyword::Double
                    | Keyword::Complex
                    | Keyword::Int
                    | Keyword::Short
                    | Keyword::Long
                    | Keyword::Signed
                    | Keyword::Unsigned),
                )) => {
                    self.bump();
                    saw_any = true;
                    builtin.add(keyword, self.prev_span())?;
                }
                Some(TokenKind::Identifier(name)) => {
                    let Some(typedef_ty) = self.lookup_typedef_name(&name) else {
                        break;
                    };
                    if direct_type.is_some() || builtin.has_any() {
                        break;
                    }
                    saw_any = true;
                    self.bump();
                    direct_type = Some(typedef_ty);
                    direct_vla_bounds = self.lookup_typedef_vla_bounds(&name);
                }
                _ => break,
            }
        }

        if !saw_any {
            return Err(Diagnostic::error("expected declaration specifiers", start));
        }
        self.validate_decl_specifier_context(storage_class, is_inline, context, start)?;
        if is_noreturn && context != DeclContext::FileScope && context != DeclContext::BlockScope {
            return Err(Diagnostic::error(
                "_Noreturn is only valid in a function declaration",
                start,
            ));
        }
        if alignment.is_some() && matches!(context, DeclContext::Parameter | DeclContext::TypeName)
        {
            return Err(Diagnostic::error(
                "_Alignas is not valid in this declaration context",
                start,
            ));
        }
        if let Some(ty) = direct_type.as_ref()
            && builtin.has_any()
        {
            return Err(Diagnostic::error(
                format!("{} cannot be combined with a built-in type specifier", ty),
                start,
            ));
        }
        let type_error_span = self.prev_span();
        if saw_atomic_type_specifier && qualifiers.is_atomic {
            return Err(Diagnostic::error(
                "_Atomic qualifier cannot qualify an atomic type",
                start,
            ));
        }
        if qualifiers.is_atomic
            && direct_type.as_ref().is_some_and(|ty| {
                matches!(ty.unqualified(), CType::Array(..) | CType::Function(..))
                    || ty.top_level_qualifiers().is_atomic
            })
        {
            return Err(Diagnostic::error(
                "_Atomic qualifier requires a non-atomic scalar, structure, or union type",
                start,
            ));
        }
        let base_type = if let Some(ty) = direct_type {
            CType::qualified(ty, qualifiers)
        } else {
            CType::qualified(builtin.finish(type_error_span)?, qualifiers)
        };
        Ok(DeclarationSpecifiers {
            base_type,
            vla_bounds: direct_vla_bounds,
            storage_class,
            is_inline,
            is_noreturn,
            alignment,
            declares_tag_or_enumerators,
        })
    }

    fn parse_alignment_specifier(&mut self) -> Result<usize, Diagnostic> {
        let start = self.bump().span;
        self.expect(TokenKind::LParen)?;
        let alignment = if self.starts_type_name_at(0) {
            let parsed = self.parse_type_name()?;
            if parsed.vla_bounds.iter().any(Option::is_some) {
                return Err(Diagnostic::error(
                    "_Alignas type name cannot be variably modified",
                    start,
                ));
            }
            self.type_align_of(&parsed.ty)?
        } else {
            let expr = self.parse_conditional()?;
            let value = self.eval_typed_integer_constant_expr(&expr)?.value;
            usize::try_from(value).map_err(|_| {
                Diagnostic::error("_Alignas requires a nonnegative alignment", expr.span())
            })?
        };
        let end = self.expect(TokenKind::RParen)?.span;
        if alignment != 0 && (!alignment.is_power_of_two() || alignment > (1usize << 29)) {
            return Err(Diagnostic::error(
                "requested alignment is not supported",
                start.merge(end),
            ));
        }
        Ok(alignment)
    }

    pub(super) fn parse_type_name(&mut self) -> Result<ParsedTypeName, Diagnostic> {
        self.skip_gnu_attributes()?;
        let start = self.current_span();
        let specs = self.parse_declaration_specifiers(DeclContext::TypeName)?;
        let (ty, mut vla_bounds) = {
            let declarator = self.parse_abstract_declarator_tree(DeclContext::TypeName)?;
            let ResolvedDeclarator {
                ty,
                vla_bounds,
                static_array_bound,
                ..
            } = self.apply_parsed_declarator(declarator, specs.base_type)?;
            if static_array_bound.is_some() {
                return Err(Diagnostic::error(
                    "static array bounds are only allowed in function parameter declarators",
                    self.prev_span(),
                ));
            }
            (ty, vla_bounds)
        };
        vla_bounds.extend(specs.vla_bounds);
        self.validate_restrict_usage(&ty, self.current_span())?;
        Ok(ParsedTypeName {
            ty,
            vla_bounds,
            span: start.merge(self.prev_span()),
        })
    }

    pub(super) fn parse_declarator(
        &mut self,
        base: CType,
        context: DeclContext,
    ) -> Result<ResolvedDeclarator, Diagnostic> {
        let declarator = self.parse_declarator_tree(context)?;
        let mut resolved = self.apply_parsed_declarator(declarator, base)?;
        if !resolved.ty.is_function() {
            resolved.function_params = None;
        }
        Ok(resolved)
    }

    fn parse_declarator_tree(
        &mut self,
        context: DeclContext,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        self.parse_declarator_tree_inner(context, false)
    }

    pub(super) fn parse_declarator_tree_inner(
        &mut self,
        context: DeclContext,
        allow_abstract: bool,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        let pointer_qualifiers = self.parse_pointer_qualifiers()?;

        let declarator = if self.eat(TokenKind::LParen) {
            let inner = self.parse_declarator_tree_inner(context, allow_abstract)?;
            self.expect(TokenKind::RParen)?;
            inner
        } else if allow_abstract && !matches!(self.peek_kind(0), Some(TokenKind::Identifier(_))) {
            ParsedDeclarator::Abstract(self.current_span())
        } else {
            let token = self.bump().clone();
            match token.kind {
                TokenKind::Identifier(name) => ParsedDeclarator::Identifier(name, token.span),
                _ => return Err(Diagnostic::error("expected identifier", token.span)),
            }
        };

        self.finish_declarator_tree(declarator, pointer_qualifiers, context)
    }

    fn parse_abstract_declarator_tree(
        &mut self,
        context: DeclContext,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        let pointer_qualifiers = self.parse_pointer_qualifiers()?;

        let declarator = if self.at(TokenKind::LParen)
            && !self.starts_type_name_at(1)
            && !matches!(self.peek_kind(1), Some(TokenKind::RParen))
        {
            self.bump();
            let inner = self.parse_abstract_declarator_tree(context)?;
            self.expect(TokenKind::RParen)?;
            inner
        } else {
            ParsedDeclarator::Abstract(self.current_span())
        };

        self.finish_declarator_tree(declarator, pointer_qualifiers, context)
    }

    fn parse_pointer_qualifiers(&mut self) -> Result<Vec<TypeQualifiers>, Diagnostic> {
        self.skip_gnu_attributes()?;
        let mut pointer_qualifiers = Vec::new();
        while self.eat(TokenKind::Star) {
            pointer_qualifiers.push(self.parse_type_qualifiers());
            self.skip_gnu_attributes()?;
        }
        Ok(pointer_qualifiers)
    }

    fn finish_declarator_tree(
        &mut self,
        mut declarator: ParsedDeclarator,
        pointer_qualifiers: Vec<TypeQualifiers>,
        context: DeclContext,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        self.skip_gnu_attributes()?;

        loop {
            if self.eat(TokenKind::LBracket) {
                let spec = self.parse_array_spec(context)?;
                let end = self.expect(TokenKind::RBracket)?.span;
                let span = declarator.span().merge(end);
                declarator = ParsedDeclarator::Array {
                    inner: Box::new(declarator),
                    spec,
                    span,
                };
                self.skip_gnu_attributes()?;
                continue;
            }
            if self.at(TokenKind::LParen) {
                let info = self.parse_function_parameter_clause()?;
                let end = self.expect(TokenKind::RParen)?.span;
                let span = declarator.span().merge(end);
                declarator = ParsedDeclarator::Function {
                    inner: Box::new(declarator),
                    info,
                    span,
                };
                self.skip_gnu_attributes()?;
                continue;
            }
            break;
        }

        for qualifiers in pointer_qualifiers.into_iter().rev() {
            let span = declarator.span();
            declarator = ParsedDeclarator::Pointer {
                qualifiers,
                inner: Box::new(declarator),
                span,
            };
        }
        Ok(declarator)
    }

    pub(super) fn apply_parsed_declarator(
        &self,
        declarator: ParsedDeclarator,
        base: CType,
    ) -> Result<ResolvedDeclarator, Diagnostic> {
        match declarator {
            ParsedDeclarator::Abstract(span) => Ok(ResolvedDeclarator {
                name: String::new(),
                ty: base,
                vla_bounds: Vec::new(),
                static_array_bound: None,
                span,
                function_params: None,
            }),
            ParsedDeclarator::Identifier(name, span) => Ok(ResolvedDeclarator {
                name,
                ty: base,
                vla_bounds: Vec::new(),
                static_array_bound: None,
                span,
                function_params: None,
            }),
            ParsedDeclarator::Pointer {
                qualifiers,
                inner,
                span,
            } => {
                let mut resolved = self.apply_parsed_declarator(
                    *inner,
                    CType::qualified(CType::pointer_to(base), qualifiers),
                )?;
                resolved.span = span.merge(resolved.span);
                Ok(resolved)
            }
            ParsedDeclarator::Array { inner, spec, span } => {
                let ParsedArraySpec {
                    bound,
                    static_bound,
                    qualifiers,
                    ..
                } = spec;
                if self.type_contains_flexible_array_structure(&base) {
                    return Err(Diagnostic::error(
                        "a structure containing a flexible array member cannot be an array element",
                        span,
                    ));
                }
                let len = match &bound {
                    ParsedArrayBound::Fixed(len) => *len,
                    ParsedArrayBound::Unspecified | ParsedArrayBound::Variable(_) => 0,
                };
                let array_ty = CType::qualified(CType::array_of(base, len), qualifiers);
                let mut resolved = self.apply_parsed_declarator(*inner, array_ty)?;
                if static_bound.is_none() {
                    match bound {
                        ParsedArrayBound::Variable(expr) => resolved.vla_bounds.push(Some(expr)),
                        ParsedArrayBound::Unspecified => resolved.vla_bounds.push(None),
                        ParsedArrayBound::Fixed(_) => {}
                    }
                }
                resolved.static_array_bound = resolved.static_array_bound.or(static_bound);
                resolved.span = span.merge(resolved.span);
                Ok(resolved)
            }
            ParsedDeclarator::Function { inner, info, span } => {
                let function_ty = if info.old_style {
                    CType::function(base, Vec::new())
                } else if info.is_variadic {
                    CType::variadic_function(
                        base,
                        info.params.iter().map(|param| param.ty.clone()).collect(),
                    )
                } else {
                    CType::function(
                        base,
                        info.params.iter().map(|param| param.ty.clone()).collect(),
                    )
                };
                let mut resolved = self.apply_parsed_declarator(*inner, function_ty)?;
                if resolved.ty.is_function() {
                    resolved.function_params = Some(info);
                }
                resolved.span = span.merge(resolved.span);
                Ok(resolved)
            }
        }
    }

    fn parse_array_spec(&mut self, context: DeclContext) -> Result<ParsedArraySpec, Diagnostic> {
        if context != DeclContext::Parameter
            && matches!(
                self.peek_kind(0),
                Some(TokenKind::Keyword(
                    Keyword::Const | Keyword::Restrict | Keyword::Static | Keyword::Volatile
                )) | Some(TokenKind::Star)
            )
        {
            return Err(Diagnostic::error(
                "array bracket qualifiers, static, and * are only valid in function parameter declarators",
                self.current_span(),
            ));
        }
        let static_before_qualifiers =
            context == DeclContext::Parameter && self.eat(TokenKind::Keyword(Keyword::Static));
        let qualifiers = self.parse_type_qualifiers();
        let static_after_qualifiers =
            context == DeclContext::Parameter && self.eat(TokenKind::Keyword(Keyword::Static));
        if static_before_qualifiers && static_after_qualifiers {
            return Err(Diagnostic::error(
                "static may appear only once in an array parameter declarator",
                self.prev_span(),
            ));
        }
        let has_static = static_before_qualifiers || static_after_qualifiers;
        let prototype_vla_star = context == DeclContext::Parameter && self.eat(TokenKind::Star);
        if prototype_vla_star && has_static {
            return Err(Diagnostic::error(
                "a prototype-scope [*] array declarator cannot also use static",
                self.prev_span(),
            ));
        }
        let (bound, static_bound) = if prototype_vla_star {
            (ParsedArrayBound::Unspecified, None)
        } else if has_static {
            if self.at(TokenKind::RBracket) {
                return Err(Diagnostic::error(
                    "static array parameter bound requires an expression",
                    self.current_span(),
                ));
            }
            let expr = self.parse_assignment()?;
            let bound = self.classify_array_bound_expr(&expr, context)?;
            (bound, Some(expr))
        } else {
            (self.parse_array_bound(context)?, None)
        };
        Ok(ParsedArraySpec {
            bound,
            static_bound,
            qualifiers,
            prototype_vla_star,
        })
    }

    pub(super) fn parsed_declarator_has_vla_star(declarator: &ParsedDeclarator) -> bool {
        match declarator {
            ParsedDeclarator::Array { inner, spec, .. } => {
                spec.prototype_vla_star || Self::parsed_declarator_has_vla_star(inner)
            }
            ParsedDeclarator::Pointer { inner, .. } | ParsedDeclarator::Function { inner, .. } => {
                Self::parsed_declarator_has_vla_star(inner)
            }
            ParsedDeclarator::Abstract(_) | ParsedDeclarator::Identifier(..) => false,
        }
    }

    fn parse_array_bound(&mut self, context: DeclContext) -> Result<ParsedArrayBound, Diagnostic> {
        if self.at(TokenKind::RBracket) {
            return Ok(ParsedArrayBound::Unspecified);
        }
        let expr = self.parse_assignment()?;
        self.classify_array_bound_expr(&expr, context)
    }

    fn classify_array_bound_expr(
        &self,
        expr: &Expr,
        context: DeclContext,
    ) -> Result<ParsedArrayBound, Diagnostic> {
        match self.eval_integer_constant_expr(expr) {
            Ok(value) => {
                if value <= 0 {
                    return Err(Diagnostic::error(
                        "array bound must be greater than zero",
                        expr.span(),
                    ));
                }
                Ok(ParsedArrayBound::Fixed(usize::try_from(value).map_err(
                    |_| Diagnostic::error("array bound is out of supported range", expr.span()),
                )?))
            }
            Err(_)
                if matches!(
                    context,
                    DeclContext::BlockScope | DeclContext::Parameter | DeclContext::TypeName
                ) =>
            {
                Ok(ParsedArrayBound::Variable(expr.clone()))
            }
            Err(_) => Err(Diagnostic::error(
                "array bound must be an integer constant expression in this context",
                expr.span(),
            )),
        }
    }
}
