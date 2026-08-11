use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinaryOp, Block, BlockItem, Declaration, Designator, Expr, ExternalDeclaration, ForInit,
    FunctionDecl, FunctionDef, GenericAssociation, Initializer, InitializerItem, Linkage,
    Parameter, PostfixOp, Statement, StorageClass, SwitchLabel, TranslationUnit, UnaryOp,
};
use crate::diag::Diagnostic;
use crate::number::{NumberValue, parse_number_literal};
use crate::source::{SourceManager, Span};
use crate::token::{Keyword, StringLiteralValue, Token, TokenKind};
use crate::types::{
    CType, EnumType, HOST_LONG_DOUBLE_ALIGN, RecordKind, RecordMember, RecordType, TypeQualifiers,
};

enum ExternalDecl {
    Function(FunctionDef),
    FunctionDeclarations(Vec<FunctionDecl>),
    Globals(Vec<Declaration>),
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Object,
    Function,
    EnumConstant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Record(RecordKind),
    Enum,
}

#[derive(Debug, Clone, Copy)]
struct TagBinding {
    kind: TagKind,
    id: usize,
}

#[derive(Debug, Clone, Default)]
struct ScopeEntry {
    ordinary: Option<SymbolKind>,
    ordinary_ty: Option<CType>,
    ordinary_storage_class: Option<ParsedStorageClass>,
    ordinary_linkage: Option<Linkage>,
    typedef_ty: Option<CType>,
    enum_constant: Option<i128>,
}

#[derive(Debug, Clone)]
struct ConstantInteger {
    ty: CType,
    value: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParsedStorageClass {
    Auto,
    Extern,
    Register,
    Static,
    Typedef,
}

#[derive(Debug, Clone)]
struct DeclarationSpecifiers {
    base_type: CType,
    storage_class: Option<ParsedStorageClass>,
    is_inline: bool,
    is_noreturn: bool,
    alignment: Option<usize>,
}

#[derive(Debug, Clone)]
struct ParsedTypeName {
    ty: CType,
    vla_bounds: Vec<Option<Expr>>,
    span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclContext {
    FileScope,
    BlockScope,
    Parameter,
    RecordMember,
    TypeName,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StringEncoding {
    Narrow,
    Wide,
    Utf16,
    Utf32,
}

#[derive(Debug, Clone)]
enum ParsedArrayBound {
    Fixed(usize),
    Unspecified,
    Variable(Expr),
}

#[derive(Debug, Clone)]
enum ParsedDeclarator {
    Abstract(Span),
    Identifier(String, Span),
    Pointer {
        qualifiers: TypeQualifiers,
        inner: Box<ParsedDeclarator>,
        span: Span,
    },
    Array {
        inner: Box<ParsedDeclarator>,
        spec: ParsedArraySpec,
        span: Span,
    },
    Function {
        inner: Box<ParsedDeclarator>,
        params: Vec<Parameter>,
        is_variadic: bool,
        parameter_tags: HashMap<String, TagBinding>,
        old_style: bool,
        span: Span,
    },
}

#[derive(Debug, Clone)]
struct ParsedArraySpec {
    bound: ParsedArrayBound,
    static_bound: Option<Expr>,
}

type ParsedFunctionInfo = (Vec<Parameter>, bool, HashMap<String, TagBinding>, bool);

impl ParsedDeclarator {
    fn span(&self) -> Span {
        match self {
            ParsedDeclarator::Abstract(span)
            | ParsedDeclarator::Identifier(_, span)
            | ParsedDeclarator::Pointer { span, .. }
            | ParsedDeclarator::Array { span, .. }
            | ParsedDeclarator::Function { span, .. } => *span,
        }
    }
}

pub struct Parser<'a> {
    sources: &'a SourceManager,
    tokens: Vec<Token>,
    index: usize,
    file_tags: HashMap<String, TagBinding>,
    block_tag_scopes: Vec<HashMap<String, TagBinding>>,
    records: HashMap<usize, RecordType>,
    enums: HashMap<usize, EnumType>,
    enum_constants: HashMap<String, i128>,
    next_tag_id: usize,
    next_member_id: usize,
    file_scope: HashMap<String, ScopeEntry>,
    block_scopes: Vec<HashMap<String, ScopeEntry>>,
    block_linkage_declarations: Vec<ExternalDeclaration>,
    allow_undeclared_identifiers: bool,
}

impl<'a> Parser<'a> {
    pub fn new(sources: &'a SourceManager, tokens: Vec<Token>) -> Self {
        let mut file_scope = HashMap::new();
        file_scope.insert(
            "va_list".to_owned(),
            ScopeEntry {
                ordinary: None,
                ordinary_ty: None,
                ordinary_storage_class: None,
                ordinary_linkage: None,
                typedef_ty: Some(CType::VaList),
                enum_constant: None,
            },
        );
        Self {
            sources,
            tokens,
            index: 0,
            file_tags: HashMap::new(),
            block_tag_scopes: Vec::new(),
            records: HashMap::new(),
            enums: HashMap::new(),
            enum_constants: HashMap::new(),
            next_tag_id: 0,
            next_member_id: 0,
            file_scope,
            block_scopes: Vec::new(),
            block_linkage_declarations: Vec::new(),
            allow_undeclared_identifiers: false,
        }
    }

    pub fn parse_translation_unit(mut self) -> Result<TranslationUnit, Diagnostic> {
        let mut externals = Vec::new();
        while !self.at(TokenKind::Eof) {
            if self.at_keyword(Keyword::StaticAssert) {
                self.parse_static_assertion()?;
                continue;
            }
            match self.parse_external_declaration()? {
                ExternalDecl::Function(function) => {
                    externals.push(ExternalDeclaration::Function(function));
                }
                ExternalDecl::FunctionDeclarations(decls) => {
                    externals.extend(
                        decls
                            .into_iter()
                            .map(ExternalDeclaration::FunctionDeclaration),
                    );
                }
                ExternalDecl::Globals(decls) => {
                    externals.extend(
                        decls
                            .into_iter()
                            .map(ExternalDeclaration::ObjectDeclaration),
                    );
                }
                ExternalDecl::Empty => {}
            }
        }
        externals.append(&mut self.block_linkage_declarations);
        Ok(TranslationUnit {
            externals,
            functions: Vec::new(),
            function_declarations: Vec::new(),
            globals: Vec::new(),
            global_definitions: Vec::new(),
            inline_function_definitions: Vec::new(),
            records: self.records,
            enums: self.enums,
            enum_constants: self.enum_constants,
        })
    }

    pub fn parse_expression_only(mut self) -> Result<Expr, Diagnostic> {
        self.allow_undeclared_identifiers = true;
        let expr = self.parse_expression()?;
        self.expect(TokenKind::Eof)?;
        Ok(expr)
    }

    fn parse_external_declaration(&mut self) -> Result<ExternalDecl, Diagnostic> {
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
            return Ok(ExternalDecl::Empty);
        }
        let (name, ty, vla_bounds, _, base_span, function_params) =
            self.parse_declarator(specs.base_type.clone(), DeclContext::FileScope)?;
        if specs.is_noreturn && !ty.is_function() {
            return Err(Diagnostic::error(
                "_Noreturn is only valid on functions",
                base_span,
            ));
        }
        if specs.alignment.is_some() && ty.is_function() {
            return Err(Diagnostic::error(
                "_Alignas is not valid on functions",
                base_span,
            ));
        }
        if vla_bounds.iter().any(|bound| bound.is_some()) {
            return Err(Diagnostic::error(
                "file-scope declarations cannot have variable length array type",
                base_span,
            ));
        }
        if specs.storage_class == Some(ParsedStorageClass::Typedef) {
            return Ok(ExternalDecl::Globals(self.finish_declarator_list(
                name,
                ty,
                specs.base_type.clone(),
                vla_bounds,
                base_span,
                specs.storage_class,
                specs.is_inline,
                specs.alignment,
                DeclContext::FileScope,
            )?));
        }
        if ty.is_function() && function_params.is_none() {
            self.validate_function_decl_specifiers(&specs, base_span)?;
            let mut decls = Vec::new();
            let linkage = self.declare_function_symbol(
                &name,
                &ty,
                specs.storage_class,
                base_span,
                DeclContext::FileScope,
            )?;
            decls.push(self.build_function_declaration(
                name,
                ty,
                base_span,
                specs.storage_class,
                specs.is_inline,
                specs.is_noreturn,
                linkage,
            )?);
            while self.eat(TokenKind::Comma) {
                let (name, ty, vla_bounds, _, span, _) =
                    self.parse_declarator(specs.base_type.clone(), DeclContext::FileScope)?;
                if vla_bounds.iter().any(|bound| bound.is_some()) {
                    return Err(Diagnostic::error(
                        "file-scope declarations cannot have variable length array type",
                        span,
                    ));
                }
                if !ty.is_function() {
                    return Err(Diagnostic::error(
                        "mixed function and object declarations are not allowed",
                        span,
                    ));
                }
                let linkage = self.declare_function_symbol(
                    &name,
                    &ty,
                    specs.storage_class,
                    span,
                    DeclContext::FileScope,
                )?;
                decls.push(self.build_function_declaration(
                    name,
                    ty,
                    span,
                    specs.storage_class,
                    specs.is_inline,
                    specs.is_noreturn,
                    linkage,
                )?);
            }
            let end = self.expect(TokenKind::Semicolon)?.span;
            for decl in &mut decls {
                decl.span = decl.span.merge(end);
            }
            return Ok(ExternalDecl::FunctionDeclarations(decls));
        }
        let Some((mut params, is_variadic, parameter_tags, old_style)) = function_params else {
            return Ok(ExternalDecl::Globals(self.finish_declarator_list(
                name,
                ty,
                specs.base_type.clone(),
                vla_bounds,
                base_span,
                specs.storage_class,
                specs.is_inline,
                specs.alignment,
                DeclContext::FileScope,
            )?));
        };
        if specs.storage_class == Some(ParsedStorageClass::Typedef) {
            return Err(Diagnostic::error(
                "typedef declaration cannot declare a function definition",
                base_span,
            ));
        }
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
        if self.eat(TokenKind::Semicolon) {
            if old_style && !params.is_empty() {
                return Err(Diagnostic::error(
                    "an identifier-list function declarator is only valid in a definition",
                    base_span,
                ));
            }
            return Ok(ExternalDecl::FunctionDeclarations(vec![FunctionDecl {
                name,
                return_type,
                params,
                is_variadic,
                storage_class: specs.storage_class.and_then(ast_storage_class),
                linkage,
                is_inline: specs.is_inline,
                is_noreturn: specs.is_noreturn,
                has_prototype: !old_style,
                span: base_span,
            }]));
        }
        if self.at(TokenKind::Eof) {
            return Err(Diagnostic::error(
                format!("function header for {name} is missing a body or semicolon"),
                base_span,
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
        Ok(ExternalDecl::Function(FunctionDef {
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
        }))
    }

    fn build_function_declaration(
        &self,
        name: String,
        ty: CType,
        span: Span,
        storage_class: Option<ParsedStorageClass>,
        is_inline: bool,
        is_noreturn: bool,
        linkage: Linkage,
    ) -> Result<FunctionDecl, Diagnostic> {
        let (return_type, params, is_variadic) = match ty.unqualified() {
            CType::Function(return_type, params, is_variadic) => {
                ((**return_type).clone(), params.clone(), *is_variadic)
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
                        vla_bounds: Vec::new(),
                        static_array_bound: None,
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
                let (name, ty, vla_bounds, static_array_bound, span, _) =
                    self.apply_parsed_declarator(declarator, specs.base_type.clone(), true)?;
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
        self.pop_block_scope();
        result.map(|(params, is_variadic)| (params, is_variadic, parameter_tags, false))
    }

    fn parse_function_parameter_clause(&mut self) -> Result<ParsedFunctionInfo, Diagnostic> {
        if self.peek_kind(0) == Some(&TokenKind::LParen)
            && self.peek_kind(1) == Some(&TokenKind::RParen)
        {
            self.bump();
            return Ok((Vec::new(), false, HashMap::new(), true));
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
                adjusted_from_array_or_function: false,
                storage_class: None,
                span: token.span,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        Ok((params, false, HashMap::new(), true))
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
            let declarations = self.parse_declaration_list()?;
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

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect(TokenKind::LBrace)?.span;
        let mut items = Vec::new();
        while !self.at(TokenKind::RBrace) {
            if self.at_keyword(Keyword::StaticAssert) {
                self.parse_static_assertion()?;
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

    fn parse_block_declaration_items(&mut self) -> Result<Vec<BlockItem>, Diagnostic> {
        self.skip_gnu_attributes()?;
        let specs = self.parse_declaration_specifiers(DeclContext::BlockScope)?;
        if self.eat(TokenKind::Semicolon) {
            if specs.storage_class.is_some() || specs.is_inline {
                return Err(Diagnostic::error(
                    "storage class specifiers and inline require a declarator",
                    self.prev_span(),
                ));
            }
            return Ok(Vec::new());
        }

        let mut items = Vec::new();
        let declaration_base = specs.base_type.clone();
        let (first_name, first_ty, first_vla_bounds, _, first_span, first_function_params) =
            self.parse_declarator(declaration_base.clone(), DeclContext::BlockScope)?;
        let (first_init, first_predeclared) = if self.eat(TokenKind::Equal) {
            let predeclared = first_function_params.is_none()
                && specs.storage_class != Some(ParsedStorageClass::Typedef);
            if predeclared {
                self.declare_object_symbol(
                    &first_name,
                    &first_ty,
                    specs.storage_class,
                    first_span,
                    DeclContext::BlockScope,
                )?;
            }
            (Some(self.parse_initializer()?), predeclared)
        } else {
            (None, false)
        };
        self.finish_single_block_declarator(
            &mut items,
            first_name,
            first_ty,
            first_vla_bounds,
            first_function_params,
            specs.storage_class,
            specs.is_inline,
            specs.is_noreturn,
            specs.alignment,
            first_init,
            first_predeclared,
            first_span,
        )?;

        while self.eat(TokenKind::Comma) {
            let (name, ty, vla_bounds, _, span, function_params) =
                self.parse_declarator(declaration_base.clone(), DeclContext::BlockScope)?;
            let (init, predeclared) = if self.eat(TokenKind::Equal) {
                let predeclared = function_params.is_none()
                    && specs.storage_class != Some(ParsedStorageClass::Typedef);
                if predeclared {
                    self.declare_object_symbol(
                        &name,
                        &ty,
                        specs.storage_class,
                        span,
                        DeclContext::BlockScope,
                    )?;
                }
                (Some(self.parse_initializer()?), predeclared)
            } else {
                (None, false)
            };
            self.finish_single_block_declarator(
                &mut items,
                name,
                ty,
                vla_bounds,
                function_params,
                specs.storage_class,
                specs.is_inline,
                specs.is_noreturn,
                specs.alignment,
                init,
                predeclared,
                span,
            )?;
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

    fn parse_declaration_list(&mut self) -> Result<Vec<Declaration>, Diagnostic> {
        self.skip_gnu_attributes()?;
        let specs = self.parse_declaration_specifiers(DeclContext::BlockScope)?;
        if self.eat(TokenKind::Semicolon) {
            if specs.storage_class.is_some() || specs.is_inline {
                return Err(Diagnostic::error(
                    "storage class specifiers and inline require a declarator",
                    self.prev_span(),
                ));
            }
            return Ok(Vec::new());
        }
        let declaration_base = specs.base_type.clone();
        let (name, ty, vla_bounds, _, span, function_params) =
            self.parse_declarator(declaration_base.clone(), DeclContext::BlockScope)?;
        if function_params.is_some() && specs.storage_class != Some(ParsedStorageClass::Typedef) {
            return Err(Diagnostic::error(
                "function declaration is not allowed in this declaration context",
                span,
            ));
        }
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
            let (name, ty, vla_bounds, _, span, function_params) =
                self.parse_declarator(declaration_base.clone(), context)?;
            if function_params.is_some() && storage_class != Some(ParsedStorageClass::Typedef) {
                return Err(Diagnostic::error(
                    "function declaration is not allowed in this declaration context",
                    span,
                ));
            }
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
        name: String,
        ty: CType,
        vla_bounds: Vec<Option<Expr>>,
        function_params: Option<ParsedFunctionInfo>,
        storage_class: Option<ParsedStorageClass>,
        is_inline: bool,
        is_noreturn: bool,
        alignment: Option<usize>,
        init: Option<Initializer>,
        symbol_predeclared: bool,
        span: Span,
    ) -> Result<(), Diagnostic> {
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
            let specs = DeclarationSpecifiers {
                base_type: self.base_type_for_redeclaration(&ty),
                storage_class,
                is_inline,
                is_noreturn,
                alignment,
            };
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
            let decl = self.build_function_declaration(
                name,
                ty,
                span,
                storage_class,
                is_inline,
                is_noreturn,
                linkage,
            )?;
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
            if storage_class == Some(ParsedStorageClass::Typedef) {
                return Err(Diagnostic::error(
                    "typedef declaration cannot have variable length array type",
                    span,
                ));
            }
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
            self.declare_typedef_name(&name, ty, span)?;
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

    fn parse_initializer(&mut self) -> Result<Initializer, Diagnostic> {
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

    fn base_type_for_redeclaration(&self, ty: &CType) -> CType {
        match ty {
            CType::Function(return_type, _, _) => self.base_type_for_redeclaration(return_type),
            CType::Qualified(inner, qualifiers) => {
                CType::qualified(self.base_type_for_redeclaration(inner), *qualifiers)
            }
            CType::Pointer(inner) => self.base_type_for_redeclaration(inner),
            CType::Array(inner, _) => self.base_type_for_redeclaration(inner),
            other => other.clone(),
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        if self.at(TokenKind::LBrace) {
            self.push_block_scope();
            let block = self.parse_block()?;
            self.pop_block_scope();
            return Ok(Statement::Block(block));
        }
        if let Some(TokenKind::Identifier(label)) = self.peek_kind(0).cloned() {
            if self.peek_kind(1) == Some(&TokenKind::Colon) {
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
                Some(ForInit::Declarations(self.parse_declaration_list()?))
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
            self.expect(TokenKind::LParen)?;
            let expr = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            let statement = self.parse_statement()?;
            let body = match statement {
                Statement::Block(block) => block,
                statement => Block {
                    span: statement.span(),
                    items: vec![BlockItem::Statement(statement)],
                },
            };
            self.validate_switch_labels(&body)?;
            let span = start.merge(body.span);
            return Ok(Statement::Switch { expr, body, span });
        }
        if self.at_keyword(Keyword::If) {
            let start = self.bump().span;
            self.expect(TokenKind::LParen)?;
            let condition = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            let then_branch = Box::new(self.parse_statement()?);
            let (else_keyword_span, else_branch) = if self.at_keyword(Keyword::Else) {
                let else_span = self.bump().span;
                let mut else_statement = self.parse_statement()?;
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

    fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_comma()
    }

    fn parse_comma(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_assignment()?;
        while self.eat(TokenKind::Comma) {
            let rhs = self.parse_assignment()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::Comma,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_assignment(&mut self) -> Result<Expr, Diagnostic> {
        let lhs = self.parse_conditional()?;
        if self.eat(TokenKind::Equal) {
            let rhs = self.parse_assignment()?;
            let span = lhs.span().merge(rhs.span());
            return Ok(Expr::Assign {
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            });
        }
        let op = if self.eat(TokenKind::PlusEqual) {
            Some(BinaryOp::Add)
        } else if self.eat(TokenKind::MinusEqual) {
            Some(BinaryOp::Sub)
        } else if self.eat(TokenKind::StarEqual) {
            Some(BinaryOp::Mul)
        } else if self.eat(TokenKind::SlashEqual) {
            Some(BinaryOp::Div)
        } else if self.eat(TokenKind::PercentEqual) {
            Some(BinaryOp::Rem)
        } else if self.eat(TokenKind::LeftShiftEqual) {
            Some(BinaryOp::ShiftLeft)
        } else if self.eat(TokenKind::RightShiftEqual) {
            Some(BinaryOp::ShiftRight)
        } else if self.eat(TokenKind::AmpEqual) {
            Some(BinaryOp::BitAnd)
        } else if self.eat(TokenKind::CaretEqual) {
            Some(BinaryOp::BitXor)
        } else if self.eat(TokenKind::PipeEqual) {
            Some(BinaryOp::BitOr)
        } else {
            return Ok(lhs);
        };
        let rhs = self.parse_assignment()?;
        let span = lhs.span().merge(rhs.span());
        Ok(Expr::CompoundAssign {
            op: op.expect("compound assignment operator must be present"),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            span,
        })
    }

    fn parse_conditional(&mut self) -> Result<Expr, Diagnostic> {
        let condition = self.parse_logical_or()?;
        if self.eat(TokenKind::Question) {
            let then_expr = self.parse_expression()?;
            self.expect(TokenKind::Colon)?;
            let else_expr = self.parse_conditional()?;
            let span = condition.span().merge(else_expr.span());
            Ok(Expr::Conditional {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            })
        } else {
            Ok(condition)
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_logical_and()?;
        while self.eat(TokenKind::DoublePipe) {
            let rhs = self.parse_logical_and()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::LogicalOr,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_bitwise_or()?;
        while self.eat(TokenKind::DoubleAmp) {
            let rhs = self.parse_bitwise_or()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::LogicalAnd,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_bitwise_xor()?;
        while self.eat(TokenKind::Pipe) {
            let rhs = self.parse_bitwise_xor()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::BitOr,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_bitwise_and()?;
        while self.eat(TokenKind::Caret) {
            let rhs = self.parse_bitwise_and()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::BitXor,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality()?;
        while self.eat(TokenKind::Amp) {
            let rhs = self.parse_equality()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op: BinaryOp::BitAnd,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_relational()?;
        loop {
            let op = if self.eat(TokenKind::DoubleEqual) {
                Some(BinaryOp::Equal)
            } else if self.eat(TokenKind::BangEqual) {
                Some(BinaryOp::NotEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let rhs = self.parse_relational()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_relational(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_shift()?;
        loop {
            let op = if self.eat(TokenKind::Less) {
                Some(BinaryOp::Less)
            } else if self.eat(TokenKind::LessEqual) {
                Some(BinaryOp::LessEqual)
            } else if self.eat(TokenKind::Greater) {
                Some(BinaryOp::Greater)
            } else if self.eat(TokenKind::GreaterEqual) {
                Some(BinaryOp::GreaterEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let rhs = self.parse_shift()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_shift(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_additive()?;
        loop {
            let op = if self.eat(TokenKind::LeftShift) {
                Some(BinaryOp::ShiftLeft)
            } else if self.eat(TokenKind::RightShift) {
                Some(BinaryOp::ShiftRight)
            } else {
                None
            };
            let Some(op) = op else { break };
            let rhs = self.parse_additive()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.eat(TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.eat(TokenKind::Minus) {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else { break };
            let rhs = self.parse_term()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.eat(TokenKind::Star) {
                Some(BinaryOp::Mul)
            } else if self.eat(TokenKind::Slash) {
                Some(BinaryOp::Div)
            } else if self.eat(TokenKind::Percent) {
                Some(BinaryOp::Rem)
            } else {
                None
            };
            let Some(op) = op else { break };
            let rhs = self.parse_unary()?;
            let span = expr.span().merge(rhs.span());
            expr = Expr::Binary {
                op,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.at_keyword(Keyword::Alignof) {
            let start = self.bump().span;
            self.expect(TokenKind::LParen)?;
            let parsed = self.parse_type_name()?;
            let end = self.expect(TokenKind::RParen)?.span;
            if parsed.vla_bounds.iter().any(Option::is_some) {
                return Err(Diagnostic::error(
                    "_Alignof cannot be applied to a variably modified type",
                    start.merge(end),
                ));
            }
            let alignment = self.type_align_of(&parsed.ty).map_err(|_| {
                Diagnostic::error("_Alignof requires a complete object type", start.merge(end))
            })?;
            let span = start.merge(end);
            return parse_number_literal(alignment.to_string(), span)
                .map(|literal| Expr::Number(literal, span));
        }
        if self.at_keyword(Keyword::Sizeof) {
            let start = self.bump().span;
            if self.at(TokenKind::LParen) && self.is_type_name_start() {
                self.bump();
                let ty = self.parse_type_name()?;
                let end = self.expect(TokenKind::RParen)?.span;
                return Ok(Expr::SizeofType {
                    ty: ty.ty,
                    vla_bounds: ty.vla_bounds,
                    span: start.merge(end),
                });
            }
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::SizeofExpr {
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::LParen) && self.is_type_name_start() {
            let start = self.bump().span;
            let ty = self.parse_type_name()?;
            let end = self.expect(TokenKind::RParen)?.span;
            if self.at(TokenKind::LBrace) {
                let initializer = self.parse_initializer()?;
                let span = start.merge(initializer.span());
                return Ok(Expr::CompoundLiteral {
                    ty: ty.ty,
                    vla_bounds: ty.vla_bounds,
                    initializer: Box::new(initializer),
                    span,
                });
            }
            let expr = self.parse_unary()?;
            let span = start.merge(end).merge(expr.span());
            return Ok(Expr::Cast {
                ty: ty.ty,
                vla_bounds: ty.vla_bounds,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::DoublePlus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::PreIncrement,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::DoubleMinus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::PreDecrement,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Amp) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::AddressOf,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Star) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Dereference,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Plus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Minus) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Bang) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::LogicalNot,
                expr: Box::new(expr),
                span,
            });
        }
        if self.at(TokenKind::Tilde) {
            let start = self.bump().span;
            let expr = self.parse_unary()?;
            let span = start.merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_static_assertion(&mut self) -> Result<(), Diagnostic> {
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

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.at(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_assignment()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end = self.expect(TokenKind::RParen)?.span;
                let span = expr.span().merge(end);
                let declared_callee_type = match &expr {
                    Expr::Variable(name, _) => self
                        .visible_scope_entry(name)
                        .and_then(|entry| entry.ordinary_ty.clone()),
                    _ => None,
                };
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    declared_callee_type,
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::LBracket) {
                let index = self.parse_expression()?;
                let end = self.expect(TokenKind::RBracket)?.span;
                let span = expr.span().merge(end);
                expr = Expr::Subscript {
                    base: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::DoublePlus) {
                let span = expr.span().merge(self.prev_span());
                expr = Expr::Postfix {
                    op: PostfixOp::PostIncrement,
                    expr: Box::new(expr),
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::DoubleMinus) {
                let span = expr.span().merge(self.prev_span());
                expr = Expr::Postfix {
                    op: PostfixOp::PostDecrement,
                    expr: Box::new(expr),
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::Dot) {
                let member = match self.bump().clone().kind {
                    TokenKind::Identifier(name) => name,
                    _ => {
                        return Err(Diagnostic::error(
                            "expected member name after .",
                            self.prev_span(),
                        ));
                    }
                };
                let span = expr.span().merge(self.prev_span());
                expr = Expr::Member {
                    base: Box::new(expr),
                    member,
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::Arrow) {
                let member_token = self.bump().clone();
                let member = match member_token.kind {
                    TokenKind::Identifier(name) => name,
                    _ => {
                        return Err(Diagnostic::error(
                            "expected member name after ->",
                            member_token.span,
                        ));
                    }
                };
                let expr_span = expr.span();
                let deref = Expr::Unary {
                    op: UnaryOp::Dereference,
                    expr: Box::new(expr),
                    span: expr_span.merge(member_token.span),
                };
                let deref_span = deref.span();
                expr = Expr::Member {
                    base: Box::new(deref),
                    member,
                    span: deref_span.merge(member_token.span),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump().clone();
        match token.kind {
            TokenKind::Number(text) => parse_number_literal(text, token.span)
                .map(|literal| Expr::Number(literal, token.span)),
            TokenKind::CharLiteral(value) => Ok(Expr::CharLiteral(value, token.span)),
            TokenKind::WideCharLiteral(value) => Ok(Expr::WideCharLiteral(value, token.span)),
            TokenKind::Utf16CharLiteral(value) => Ok(Expr::Utf16CharLiteral(value, token.span)),
            TokenKind::Utf32CharLiteral(value) => Ok(Expr::Utf32CharLiteral(value, token.span)),
            TokenKind::StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Narrow, token.span)
            }
            TokenKind::WideStringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Wide, token.span)
            }
            TokenKind::Utf16StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Utf16, token.span)
            }
            TokenKind::Utf32StringLiteral(text) => {
                self.parse_string_literal_primary(text, StringEncoding::Utf32, token.span)
            }
            TokenKind::Keyword(Keyword::Generic) => self.parse_generic_selection(token.span),
            TokenKind::Identifier(name) => {
                if name == "va_arg" && self.at(TokenKind::LParen) {
                    self.expect(TokenKind::LParen)?;
                    let ap = self.parse_assignment()?;
                    self.expect(TokenKind::Comma)?;
                    let ty = self.parse_type_name()?;
                    if ty.vla_bounds.iter().any(|bound| bound.is_some()) {
                        return Err(Diagnostic::error(
                            "va_arg does not support variable length array type names",
                            token.span,
                        ));
                    }
                    let end = self.expect(TokenKind::RParen)?.span;
                    return Ok(Expr::VaArg {
                        ap: Box::new(ap),
                        ty: ty.ty,
                        span: token.span.merge(end),
                    });
                }
                if name == "__builtin_offsetof" && self.at(TokenKind::LParen) {
                    self.expect(TokenKind::LParen)?;
                    let ty = self.parse_type_name()?;
                    if ty.vla_bounds.iter().any(|bound| bound.is_some()) {
                        return Err(Diagnostic::error(
                            "__builtin_offsetof does not support variable length array type names",
                            token.span,
                        ));
                    }
                    if !matches!(ty.ty.unqualified(), CType::Struct(..) | CType::Union(..)) {
                        return Err(Diagnostic::error(
                            "__builtin_offsetof requires a struct or union type",
                            ty.span,
                        ));
                    }
                    self.expect(TokenKind::Comma)?;
                    let designators = self.parse_offsetof_designators()?;
                    let end = self.expect(TokenKind::RParen)?.span;
                    return Ok(Expr::OffsetOf {
                        ty: ty.ty,
                        designators,
                        span: token.span.merge(end),
                    });
                }
                if let Some(value) = self.lookup_enum_constant_value(&name) {
                    return Ok(Self::enum_constant_expr(value, token.span));
                }
                if !self.allow_undeclared_identifiers
                    && !self.identifier_is_visible(&name)
                    && !self.is_builtin_identifier(&name)
                {
                    return Err(Diagnostic::error(
                        format!("use of undeclared identifier {}", name),
                        token.span,
                    ));
                }
                Ok(Expr::Variable(name, token.span))
            }
            TokenKind::LParen => {
                let mut expr = self.parse_expression()?;
                let end = self.expect(TokenKind::RParen)?.span;
                expr.set_span(token.span.merge(end));
                Ok(expr)
            }
            _ => Err(Diagnostic::error("expected expression", token.span)),
        }
    }

    fn enum_constant_expr(value: i128, span: Span) -> Expr {
        if value >= 0 {
            return Self::generated_number_expr(value.to_string(), span);
        }
        let magnitude_before_last = value
            .checked_add(1)
            .and_then(i128::checked_neg)
            .expect("enumerator values are constrained to the range of int");
        Expr::Binary {
            op: BinaryOp::Sub,
            lhs: Box::new(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(Self::generated_number_expr(
                    magnitude_before_last.to_string(),
                    span,
                )),
                span,
            }),
            rhs: Box::new(Self::generated_number_expr("1".to_owned(), span)),
            span,
        }
    }

    fn generated_number_expr(text: String, span: Span) -> Expr {
        Expr::Number(
            parse_number_literal(text, span).expect("generated number literal must be valid"),
            span,
        )
    }

    fn parse_string_literal_primary(
        &mut self,
        text: StringLiteralValue,
        mut encoding: StringEncoding,
        span: Span,
    ) -> Result<Expr, Diagnostic> {
        let mut combined = text;
        let mut combined_span = span;
        loop {
            let (text, next_encoding) = match self.peek_kind(0).cloned() {
                Some(TokenKind::StringLiteral(text)) => (text, StringEncoding::Narrow),
                Some(TokenKind::WideStringLiteral(text)) => (text, StringEncoding::Wide),
                Some(TokenKind::Utf16StringLiteral(text)) => (text, StringEncoding::Utf16),
                Some(TokenKind::Utf32StringLiteral(text)) => (text, StringEncoding::Utf32),
                _ => break,
            };
            if encoding != StringEncoding::Narrow
                && next_encoding != StringEncoding::Narrow
                && encoding != next_encoding
            {
                return Err(Diagnostic::error(
                    "adjacent string literals have incompatible encoding prefixes",
                    combined_span.merge(self.tokens[self.index].span),
                ));
            }
            if encoding == StringEncoding::Narrow {
                encoding = next_encoding;
            }
            {
                let next_span = self.tokens[self.index].span;
                self.bump();
                combined.append(text);
                combined_span = combined_span.merge(next_span);
            }
        }
        Ok(match encoding {
            StringEncoding::Narrow => Expr::StringLiteral(combined, combined_span),
            StringEncoding::Wide => Expr::WideStringLiteral(combined, combined_span),
            StringEncoding::Utf16 => Expr::Utf16StringLiteral(combined, combined_span),
            StringEncoding::Utf32 => Expr::Utf32StringLiteral(combined, combined_span),
        })
    }

    fn parse_generic_selection(&mut self, start: Span) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::LParen)?;
        let control = self.parse_assignment()?;
        self.expect(TokenKind::Comma)?;
        let mut associations = Vec::new();
        let mut default_expr = None;
        loop {
            if self.at_keyword(Keyword::Default) {
                let default_span = self.bump().span;
                if default_expr.is_some() {
                    return Err(Diagnostic::error(
                        "_Generic may specify default at most once",
                        default_span,
                    ));
                }
                self.expect(TokenKind::Colon)?;
                default_expr = Some(Box::new(self.parse_assignment()?));
            } else {
                let ty = self.parse_type_name()?;
                if ty.vla_bounds.iter().any(|bound| bound.is_some()) {
                    return Err(Diagnostic::error(
                        "_Generic does not support variably modified association types",
                        start,
                    ));
                }
                if !self.type_is_complete(&ty.ty) {
                    return Err(Diagnostic::error(
                        "_Generic association type must be a complete object type",
                        ty.span,
                    ));
                }
                if associations.iter().any(|association: &GenericAssociation| {
                    generic_types_compatible(&association.ty, &ty.ty)
                }) {
                    return Err(Diagnostic::error(
                        "_Generic associations may not specify compatible types more than once",
                        ty.span,
                    ));
                }
                self.expect(TokenKind::Colon)?;
                let expr = self.parse_assignment()?;
                associations.push(GenericAssociation {
                    ty: ty.ty,
                    span: expr.span(),
                    expr,
                });
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(TokenKind::RParen)?.span;
        if associations.is_empty() && default_expr.is_none() {
            return Err(Diagnostic::error(
                "_Generic requires at least one association",
                start.merge(end),
            ));
        }
        Ok(Expr::GenericSelection {
            control: Box::new(control),
            associations,
            default: default_expr,
            span: start.merge(end),
        })
    }

    fn parse_type_qualifiers(&mut self) -> TypeQualifiers {
        let mut qualifiers = TypeQualifiers::default();
        while matches!(
            self.peek_kind(0),
            Some(TokenKind::Keyword(
                Keyword::Const | Keyword::Restrict | Keyword::Volatile
            ))
        ) {
            match self.bump().kind {
                TokenKind::Keyword(Keyword::Const) => qualifiers.is_const = true,
                TokenKind::Keyword(Keyword::Restrict) => qualifiers.is_restrict = true,
                TokenKind::Keyword(Keyword::Volatile) => qualifiers.is_volatile = true,
                _ => unreachable!(),
            }
        }
        qualifiers
    }

    fn parse_offsetof_designators(&mut self) -> Result<Vec<Designator>, Diagnostic> {
        let mut designators = Vec::new();
        let first = self.bump().clone();
        let TokenKind::Identifier(name) = first.kind else {
            return Err(Diagnostic::error(
                "offsetof requires a member designator after the type name",
                first.span,
            ));
        };
        designators.push(Designator::Member(name, first.span));
        loop {
            if self.eat(TokenKind::Dot) {
                let token = self.bump().clone();
                let TokenKind::Identifier(name) = token.kind else {
                    return Err(Diagnostic::error(
                        "expected member name after . in offsetof designator",
                        token.span,
                    ));
                };
                designators.push(Designator::Member(name, token.span));
                continue;
            }
            if self.eat(TokenKind::LBracket) {
                let expr = self.parse_assignment()?;
                let value = self.eval_integer_constant_expr(&expr)?;
                if value < 0 {
                    return Err(Diagnostic::error(
                        "offsetof array designator index must be non-negative",
                        expr.span(),
                    ));
                }
                let end = self.expect(TokenKind::RBracket)?.span;
                designators.push(Designator::Index(
                    usize::try_from(value).map_err(|_| {
                        Diagnostic::error(
                            "offsetof array designator index is out of range",
                            expr.span(),
                        )
                    })?,
                    expr.span().merge(end),
                ));
                continue;
            }
            break;
        }
        Ok(designators)
    }

    fn is_builtin_identifier(&self, name: &str) -> bool {
        matches!(
            name,
            "va_start" | "va_end" | "va_copy" | "va_arg" | "__builtin_offsetof"
        )
    }

    fn is_attribute_name(kind: Option<&TokenKind>) -> bool {
        matches!(
            kind,
            Some(TokenKind::Identifier(name)) if name == "__attribute__" || name == "__attribute"
        )
    }

    fn at_gnu_attribute_with_offset(&self, offset: usize) -> bool {
        Self::is_attribute_name(self.peek_kind(offset))
            && self.peek_kind(offset + 1) == Some(&TokenKind::LParen)
            && self.peek_kind(offset + 2) == Some(&TokenKind::LParen)
    }

    fn skip_gnu_attributes_at_offset(&self, mut offset: usize) -> usize {
        while self.at_gnu_attribute_with_offset(offset) {
            offset += 3;
            let mut depth = 2usize;
            while let Some(kind) = self.peek_kind(offset) {
                match kind {
                    TokenKind::LParen => depth += 1,
                    TokenKind::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            offset += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                offset += 1;
            }
        }
        offset
    }

    fn skip_gnu_attributes(&mut self) -> Result<(), Diagnostic> {
        while self.at_gnu_attribute_with_offset(0) {
            let start = self.bump().span;
            self.expect(TokenKind::LParen)?;
            self.expect(TokenKind::LParen)?;
            let mut depth = 2usize;
            while depth > 0 {
                let token = self.bump().clone();
                match token.kind {
                    TokenKind::LParen => depth += 1,
                    TokenKind::RParen => depth -= 1,
                    TokenKind::Eof => {
                        return Err(Diagnostic::error(
                            "unterminated __attribute__((...))",
                            start,
                        ));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn parse_declaration_specifiers(
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
        let mut saw_any = false;
        let mut saw_void = false;
        let mut saw_bool = false;
        let mut saw_char = false;
        let mut saw_float = false;
        let mut saw_double = false;
        let mut saw_complex = false;
        let mut saw_int = false;
        let mut saw_short = false;
        let mut long_count = 0usize;
        let mut saw_signed = false;
        let mut saw_unsigned = false;

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
                    if align != 0 {
                        alignment = Some(alignment.map_or(align, |old: usize| old.max(align)));
                    }
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
                    if direct_type.is_some()
                        || saw_builtin_type_specifier(
                            saw_void,
                            saw_bool,
                            saw_char,
                            saw_float,
                            saw_double,
                            saw_complex,
                            saw_int,
                            saw_short,
                            long_count,
                            saw_signed,
                            saw_unsigned,
                        )
                    {
                        return Err(Diagnostic::error(
                            "struct type cannot be combined with other type specifiers",
                            self.current_span(),
                        ));
                    }
                    direct_type = Some(self.parse_record_specifier(RecordKind::Struct)?);
                }
                Some(TokenKind::Keyword(Keyword::Union)) => {
                    saw_any = true;
                    if direct_type.is_some()
                        || saw_builtin_type_specifier(
                            saw_void,
                            saw_bool,
                            saw_char,
                            saw_float,
                            saw_double,
                            saw_complex,
                            saw_int,
                            saw_short,
                            long_count,
                            saw_signed,
                            saw_unsigned,
                        )
                    {
                        return Err(Diagnostic::error(
                            "union type cannot be combined with other type specifiers",
                            self.current_span(),
                        ));
                    }
                    direct_type = Some(self.parse_record_specifier(RecordKind::Union)?);
                }
                Some(TokenKind::Keyword(Keyword::Enum)) => {
                    saw_any = true;
                    if direct_type.is_some()
                        || saw_builtin_type_specifier(
                            saw_void,
                            saw_bool,
                            saw_char,
                            saw_float,
                            saw_double,
                            saw_complex,
                            saw_int,
                            saw_short,
                            long_count,
                            saw_signed,
                            saw_unsigned,
                        )
                    {
                        return Err(Diagnostic::error(
                            "enum type cannot be combined with other type specifiers",
                            self.current_span(),
                        ));
                    }
                    direct_type = Some(self.parse_enum_specifier()?);
                }
                Some(TokenKind::Keyword(Keyword::Void)) => {
                    self.bump();
                    saw_any = true;
                    if saw_void {
                        return Err(Diagnostic::error(
                            "duplicate type specifier void",
                            self.prev_span(),
                        ));
                    }
                    saw_void = true;
                }
                Some(TokenKind::Keyword(Keyword::Bool)) => {
                    self.bump();
                    saw_any = true;
                    if saw_bool {
                        return Err(Diagnostic::error(
                            "duplicate type specifier _Bool",
                            self.prev_span(),
                        ));
                    }
                    saw_bool = true;
                }
                Some(TokenKind::Keyword(Keyword::Char)) => {
                    self.bump();
                    saw_any = true;
                    if saw_char {
                        return Err(Diagnostic::error(
                            "duplicate type specifier char",
                            self.prev_span(),
                        ));
                    }
                    saw_char = true;
                }
                Some(TokenKind::Keyword(Keyword::Float)) => {
                    self.bump();
                    saw_any = true;
                    if saw_float {
                        return Err(Diagnostic::error(
                            "duplicate type specifier float",
                            self.prev_span(),
                        ));
                    }
                    saw_float = true;
                }
                Some(TokenKind::Keyword(Keyword::Complex)) => {
                    self.bump();
                    saw_any = true;
                    if saw_complex {
                        return Err(Diagnostic::error(
                            "duplicate type specifier _Complex",
                            self.prev_span(),
                        ));
                    }
                    saw_complex = true;
                }
                Some(TokenKind::Keyword(Keyword::Double)) => {
                    self.bump();
                    saw_any = true;
                    if saw_double {
                        return Err(Diagnostic::error(
                            "duplicate type specifier double",
                            self.prev_span(),
                        ));
                    }
                    saw_double = true;
                }
                Some(TokenKind::Keyword(Keyword::Int)) => {
                    self.bump();
                    saw_any = true;
                    if saw_int {
                        return Err(Diagnostic::error(
                            "duplicate type specifier int",
                            self.prev_span(),
                        ));
                    }
                    saw_int = true;
                }
                Some(TokenKind::Keyword(Keyword::Short)) => {
                    self.bump();
                    saw_any = true;
                    if saw_short {
                        return Err(Diagnostic::error(
                            "duplicate type specifier short",
                            self.prev_span(),
                        ));
                    }
                    saw_short = true;
                }
                Some(TokenKind::Keyword(Keyword::Long)) => {
                    self.bump();
                    saw_any = true;
                    long_count += 1;
                }
                Some(TokenKind::Keyword(Keyword::Signed)) => {
                    self.bump();
                    saw_any = true;
                    if saw_signed {
                        return Err(Diagnostic::error(
                            "duplicate type specifier signed",
                            self.prev_span(),
                        ));
                    }
                    saw_signed = true;
                }
                Some(TokenKind::Keyword(Keyword::Unsigned)) => {
                    self.bump();
                    saw_any = true;
                    if saw_unsigned {
                        return Err(Diagnostic::error(
                            "duplicate type specifier unsigned",
                            self.prev_span(),
                        ));
                    }
                    saw_unsigned = true;
                }
                Some(TokenKind::Identifier(name)) => {
                    let Some(typedef_ty) = self.lookup_typedef_name(&name) else {
                        break;
                    };
                    if direct_type.is_some()
                        || saw_builtin_type_specifier(
                            saw_void,
                            saw_bool,
                            saw_char,
                            saw_float,
                            saw_double,
                            saw_complex,
                            saw_int,
                            saw_short,
                            long_count,
                            saw_signed,
                            saw_unsigned,
                        )
                    {
                        break;
                    }
                    saw_any = true;
                    self.bump();
                    direct_type = Some(typedef_ty);
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
        if alignment.is_some()
            && matches!(
                context,
                DeclContext::Parameter | DeclContext::RecordMember | DeclContext::TypeName
            )
        {
            return Err(Diagnostic::error(
                "_Alignas is not valid in this declaration context",
                start,
            ));
        }
        if let Some(ty) = direct_type.as_ref()
            && saw_builtin_type_specifier(
                saw_void,
                saw_bool,
                saw_char,
                saw_float,
                saw_double,
                saw_complex,
                saw_int,
                saw_short,
                long_count,
                saw_signed,
                saw_unsigned,
            )
        {
            return Err(Diagnostic::error(
                format!("{} cannot be combined with a built-in type specifier", ty),
                start,
            ));
        }
        let type_error_span = self.prev_span();
        let base_type = if let Some(ty) = direct_type {
            CType::qualified(ty, qualifiers)
        } else {
            CType::qualified(
                self.finish_builtin_type_specifier(
                    saw_void,
                    saw_bool,
                    saw_char,
                    saw_float,
                    saw_double,
                    saw_complex,
                    saw_int,
                    saw_short,
                    long_count,
                    saw_signed,
                    saw_unsigned,
                    type_error_span,
                )?,
                qualifiers,
            )
        };
        Ok(DeclarationSpecifiers {
            base_type,
            storage_class,
            is_inline,
            is_noreturn,
            alignment,
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

    fn parse_type_name(&mut self) -> Result<ParsedTypeName, Diagnostic> {
        self.skip_gnu_attributes()?;
        let start = self.current_span();
        let specs = self.parse_declaration_specifiers(DeclContext::TypeName)?;
        let (ty, vla_bounds) = if self.at(TokenKind::LParen) {
            let declarator = self.parse_abstract_declarator_tree(DeclContext::TypeName)?;
            let (_, ty, vla_bounds, static_array_bound, _, _) =
                self.apply_parsed_declarator(declarator, specs.base_type, true)?;
            if static_array_bound.is_some() {
                return Err(Diagnostic::error(
                    "static array bounds are only allowed in function parameter declarators",
                    self.prev_span(),
                ));
            }
            (ty, vla_bounds)
        } else {
            self.parse_type_suffix(specs.base_type)?
        };
        self.validate_restrict_usage(&ty, self.current_span())?;
        Ok(ParsedTypeName {
            ty,
            vla_bounds,
            span: start.merge(self.prev_span()),
        })
    }

    fn finish_builtin_type_specifier(
        &self,
        saw_void: bool,
        saw_bool: bool,
        saw_char: bool,
        saw_float: bool,
        saw_double: bool,
        saw_complex: bool,
        saw_int: bool,
        saw_short: bool,
        long_count: usize,
        saw_signed: bool,
        saw_unsigned: bool,
        start: Span,
    ) -> Result<CType, Diagnostic> {
        if saw_signed && saw_unsigned {
            return Err(Diagnostic::error(
                "type specifier cannot be both signed and unsigned",
                start,
            ));
        }
        if saw_void {
            if saw_bool
                || saw_char
                || saw_float
                || saw_double
                || saw_complex
                || saw_int
                || saw_short
                || long_count > 0
                || saw_signed
                || saw_unsigned
            {
                return Err(Diagnostic::error(
                    "void cannot be combined with other type specifiers",
                    start,
                ));
            }
            return Ok(CType::Void);
        }
        if saw_bool {
            if saw_char
                || saw_float
                || saw_double
                || saw_complex
                || saw_int
                || saw_short
                || long_count > 0
                || saw_signed
                || saw_unsigned
            {
                return Err(Diagnostic::error(
                    "_Bool cannot be combined with other type specifiers",
                    start,
                ));
            }
            return Ok(CType::Bool);
        }
        if saw_char {
            if saw_float || saw_double || saw_complex || saw_short || long_count > 0 || saw_int {
                return Err(Diagnostic::error(
                    "char cannot be combined with float, double, short, long, or int",
                    start,
                ));
            }
            return Ok(if saw_unsigned {
                CType::UnsignedChar
            } else if saw_signed {
                CType::SignedChar
            } else {
                CType::Char
            });
        }
        if saw_float {
            if saw_complex {
                if saw_double
                    || saw_char
                    || saw_int
                    || saw_short
                    || long_count > 0
                    || saw_signed
                    || saw_unsigned
                {
                    return Err(Diagnostic::error(
                        "float _Complex cannot be combined with other type specifiers",
                        start,
                    ));
                }
                return Ok(CType::complex_of(CType::Float));
            }
            if saw_double
                || saw_char
                || saw_int
                || saw_short
                || long_count > 0
                || saw_signed
                || saw_unsigned
            {
                return Err(Diagnostic::error(
                    "float cannot be combined with other type specifiers",
                    start,
                ));
            }
            return Ok(CType::Float);
        }
        if saw_complex {
            if saw_char || saw_int || saw_short || saw_signed || saw_unsigned || long_count > 1 {
                return Err(Diagnostic::error(
                    "_Complex cannot be combined with these type specifiers",
                    start,
                ));
            }
            if saw_double {
                return Ok(if long_count == 1 {
                    CType::complex_of(CType::LongDouble)
                } else {
                    CType::complex_of(CType::Double)
                });
            }
            if long_count > 0 {
                return Err(Diagnostic::error("long _Complex requires double", start));
            }
            return Err(Diagnostic::error(
                "_Complex requires float or double",
                start,
            ));
        }
        if saw_double {
            if saw_char || saw_int || saw_short || saw_signed || saw_unsigned || long_count > 1 {
                return Err(Diagnostic::error(
                    "double cannot be combined with these type specifiers",
                    start,
                ));
            }
            return Ok(if long_count == 1 {
                CType::LongDouble
            } else {
                CType::Double
            });
        }
        if saw_short {
            if long_count > 0 {
                return Err(Diagnostic::error(
                    "short cannot be combined with long",
                    start,
                ));
            }
            return Ok(if saw_unsigned {
                CType::UnsignedShort
            } else {
                CType::Short
            });
        }
        if long_count > 0 {
            if long_count > 2 {
                return Err(Diagnostic::error("only long long is supported", start));
            }
            return Ok(match (saw_unsigned, long_count) {
                (true, 1) => CType::UnsignedLong,
                (true, 2) => CType::UnsignedLongLong,
                (false, 1) => CType::Long,
                (false, 2) => CType::LongLong,
                _ => unreachable!(),
            });
        }
        if saw_int || saw_unsigned || saw_signed {
            return Ok(if saw_unsigned {
                CType::UnsignedInt
            } else {
                CType::Int
            });
        }
        Err(Diagnostic::error("expected type specifier", start))
    }

    fn identifier_is_visible(&self, name: &str) -> bool {
        self.lookup_ordinary_symbol(name).is_some()
    }

    fn push_block_scope(&mut self) {
        self.block_scopes.push(HashMap::new());
        self.block_tag_scopes.push(HashMap::new());
    }

    fn pop_block_scope(&mut self) {
        let _ = self.block_scopes.pop();
        let _ = self.block_tag_scopes.pop();
    }

    fn parse_declarator(
        &mut self,
        base: CType,
        context: DeclContext,
    ) -> Result<
        (
            String,
            CType,
            Vec<Option<Expr>>,
            Option<Expr>,
            Span,
            Option<ParsedFunctionInfo>,
        ),
        Diagnostic,
    > {
        let declarator = self.parse_declarator_tree(context)?;
        let (name, ty, vla_bounds, static_array_bound, span, function_params) =
            self.apply_parsed_declarator(declarator, base, true)?;
        Ok((
            name,
            ty.clone(),
            vla_bounds,
            static_array_bound,
            span,
            if ty.is_function() {
                function_params
            } else {
                None
            },
        ))
    }

    fn parse_declarator_tree(
        &mut self,
        context: DeclContext,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        self.parse_declarator_tree_inner(context, false)
    }

    fn parse_declarator_tree_inner(
        &mut self,
        context: DeclContext,
        allow_abstract: bool,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        self.skip_gnu_attributes()?;
        let mut pointer_qualifiers = Vec::new();
        while self.eat(TokenKind::Star) {
            pointer_qualifiers.push(self.parse_type_qualifiers());
            self.skip_gnu_attributes()?;
        }

        let mut declarator = if self.eat(TokenKind::LParen) {
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
                let (params, is_variadic, parameter_tags, old_style) =
                    self.parse_function_parameter_clause()?;
                let end = self.expect(TokenKind::RParen)?.span;
                let span = declarator.span().merge(end);
                declarator = ParsedDeclarator::Function {
                    inner: Box::new(declarator),
                    params,
                    is_variadic,
                    parameter_tags,
                    old_style,
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

    fn parse_abstract_declarator_tree(
        &mut self,
        context: DeclContext,
    ) -> Result<ParsedDeclarator, Diagnostic> {
        self.skip_gnu_attributes()?;
        let mut pointer_qualifiers = Vec::new();
        while self.eat(TokenKind::Star) {
            pointer_qualifiers.push(self.parse_type_qualifiers());
            self.skip_gnu_attributes()?;
        }

        let mut declarator = if self.eat(TokenKind::LParen) {
            let inner = self.parse_abstract_declarator_tree(context)?;
            self.expect(TokenKind::RParen)?;
            inner
        } else {
            ParsedDeclarator::Abstract(self.current_span())
        };

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
                let (params, is_variadic, parameter_tags, old_style) =
                    self.parse_function_parameter_clause()?;
                let end = self.expect(TokenKind::RParen)?.span;
                let span = declarator.span().merge(end);
                declarator = ParsedDeclarator::Function {
                    inner: Box::new(declarator),
                    params,
                    is_variadic,
                    parameter_tags,
                    old_style,
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

    fn apply_parsed_declarator(
        &self,
        declarator: ParsedDeclarator,
        base: CType,
        _outermost: bool,
    ) -> Result<
        (
            String,
            CType,
            Vec<Option<Expr>>,
            Option<Expr>,
            Span,
            Option<ParsedFunctionInfo>,
        ),
        Diagnostic,
    > {
        match declarator {
            ParsedDeclarator::Abstract(span) => {
                Ok((String::new(), base, Vec::new(), None, span, None))
            }
            ParsedDeclarator::Identifier(name, span) => {
                Ok((name, base, Vec::new(), None, span, None))
            }
            ParsedDeclarator::Pointer {
                qualifiers,
                inner,
                span,
            } => {
                let (name, ty, vla_bounds, static_array_bound, inner_span, function_params) = self
                    .apply_parsed_declarator(
                        *inner,
                        CType::qualified(CType::pointer_to(base), qualifiers),
                        false,
                    )?;
                Ok((
                    name,
                    ty,
                    vla_bounds,
                    static_array_bound,
                    span.merge(inner_span),
                    function_params,
                ))
            }
            ParsedDeclarator::Array { inner, spec, span } => {
                let ParsedArraySpec {
                    bound,
                    static_bound,
                } = spec;
                if self.type_contains_flexible_array_structure(&base) {
                    return Err(Diagnostic::error(
                        "a structure containing a flexible array member cannot be an array element",
                        span,
                    ));
                }
                let array_ty = match &bound {
                    ParsedArrayBound::Fixed(len) => CType::array_of(base, *len),
                    ParsedArrayBound::Unspecified | ParsedArrayBound::Variable(_) => {
                        CType::array_of(base, 0)
                    }
                };
                let (name, ty, mut vla_bounds, static_array_bound, inner_span, function_params) =
                    self.apply_parsed_declarator(*inner, array_ty, false)?;
                if static_bound.is_none() {
                    if let ParsedArrayBound::Variable(expr) = bound {
                        vla_bounds.push(Some(expr));
                    } else if matches!(bound, ParsedArrayBound::Unspecified) {
                        vla_bounds.push(None);
                    }
                }
                Ok((
                    name,
                    ty,
                    vla_bounds,
                    static_array_bound.or(static_bound),
                    span.merge(inner_span),
                    function_params,
                ))
            }
            ParsedDeclarator::Function {
                inner,
                params,
                is_variadic,
                parameter_tags,
                old_style,
                span,
            } => {
                let function_ty = if old_style {
                    CType::function(base, Vec::new())
                } else if is_variadic {
                    CType::variadic_function(
                        base,
                        params.iter().map(|param| param.ty.clone()).collect(),
                    )
                } else {
                    CType::function(base, params.iter().map(|param| param.ty.clone()).collect())
                };
                let (name, ty, vla_bounds, static_array_bound, inner_span, nested_function_params) =
                    self.apply_parsed_declarator(*inner, function_ty, false)?;
                let surfaces_function = ty.is_function();
                Ok((
                    name,
                    ty,
                    vla_bounds,
                    static_array_bound,
                    span.merge(inner_span),
                    if surfaces_function {
                        Some((params, is_variadic, parameter_tags, old_style))
                    } else {
                        nested_function_params
                    },
                ))
            }
        }
    }

    fn parse_type_suffix(
        &mut self,
        mut base: CType,
    ) -> Result<(CType, Vec<Option<Expr>>), Diagnostic> {
        self.skip_gnu_attributes()?;
        let mut vla_bounds = Vec::new();
        while self.eat(TokenKind::Star) {
            let qualifiers = self.parse_type_qualifiers();
            base = CType::qualified(CType::pointer_to(base), qualifiers);
            self.skip_gnu_attributes()?;
        }
        while self.eat(TokenKind::LBracket) {
            let spec = self.parse_array_spec(DeclContext::TypeName)?;
            self.expect(TokenKind::RBracket)?;
            if spec.static_bound.is_some() {
                return Err(Diagnostic::error(
                    "static array bounds are only allowed in function parameter declarators",
                    self.prev_span(),
                ));
            }
            base = match spec.bound {
                ParsedArrayBound::Fixed(len) => CType::array_of(base, len),
                ParsedArrayBound::Unspecified => {
                    vla_bounds.push(None);
                    CType::array_of(base, 0)
                }
                ParsedArrayBound::Variable(expr) => {
                    vla_bounds.push(Some(expr));
                    CType::array_of(base, 0)
                }
            };
            self.skip_gnu_attributes()?;
        }
        Ok((base, vla_bounds))
    }

    fn parse_array_spec(&mut self, context: DeclContext) -> Result<ParsedArraySpec, Diagnostic> {
        let static_bound = if context == DeclContext::Parameter && self.at_keyword(Keyword::Static)
        {
            self.bump();
            Some(self.parse_assignment()?)
        } else {
            None
        };
        let bound = if let Some(expr) = &static_bound {
            self.classify_array_bound_expr(expr, context)?
        } else {
            self.parse_array_bound(context)?
        };
        Ok(ParsedArraySpec {
            bound,
            static_bound,
        })
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
        match self.eval_integer_constant_expr(&expr) {
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

    fn parse_record_specifier(&mut self, kind: RecordKind) -> Result<CType, Diagnostic> {
        let start = self.bump().span;
        let tag = if let Some(TokenKind::Identifier(name)) = self.peek_kind(0) {
            let name = name.clone();
            self.bump();
            Some(name)
        } else {
            None
        };
        let is_definition = self.at(TokenKind::LBrace);
        let declares_tag_in_current_scope = is_definition || self.at(TokenKind::Semicolon);
        let id = match &tag {
            Some(name) => {
                let (id, is_new) = self.resolve_tag(
                    name,
                    TagKind::Record(kind),
                    declares_tag_in_current_scope,
                    start,
                )?;
                if is_new {
                    self.records.insert(
                        id,
                        RecordType {
                            id,
                            kind,
                            tag: Some(name.clone()),
                            complete: false,
                            members: Vec::new(),
                            size: 0,
                            align: 1,
                        },
                    );
                }
                id
            }
            None => self.alloc_tag_id(),
        };
        if self.eat(TokenKind::LBrace) {
            if self.records.get(&id).is_some_and(|record| record.complete) {
                return Err(Diagnostic::error(
                    "record type is already complete and cannot be redefined",
                    start,
                ));
            }
            let members = self.parse_record_members()?;
            if members.is_empty() {
                let kind_name = match kind {
                    RecordKind::Struct => "struct",
                    RecordKind::Union => "union",
                };
                return Err(Diagnostic::error(
                    format!("{kind_name} must declare at least one member"),
                    start,
                ));
            }
            let has_named_member = members.iter().try_fold(false, |found, member| {
                Ok::<_, Diagnostic>(found || !self.visible_member_names_for(member)?.is_empty())
            })?;
            if !has_named_member {
                return Err(Diagnostic::ub(
                    "structure or union declaration list contains no named members",
                    start,
                    Some("6.7.2.1p8"),
                ));
            }
            if let Some((index, _)) = members
                .iter()
                .enumerate()
                .find(|(_, member)| matches!(member.ty.unqualified(), CType::Array(_, 0)))
            {
                let flexible_member_span = members[index].declaration_span.unwrap_or(start);
                if kind != RecordKind::Struct {
                    return Err(Diagnostic::error(
                        "flexible array members are only allowed in structures",
                        flexible_member_span,
                    ));
                }
                if index + 1 != members.len() {
                    return Err(Diagnostic::error(
                        "flexible array member must be the last member of a structure",
                        flexible_member_span,
                    ));
                }
                let has_other_named_member =
                    members[..index].iter().try_fold(false, |found, member| {
                        Ok::<_, Diagnostic>(
                            found || !self.visible_member_names_for(member)?.is_empty(),
                        )
                    })?;
                if !has_other_named_member {
                    return Err(Diagnostic::error(
                        "structure with a flexible array member must have at least one other named member",
                        flexible_member_span,
                    ));
                }
            }
            if kind == RecordKind::Struct
                && members
                    .iter()
                    .any(|member| self.type_contains_flexible_array_structure(&member.ty))
            {
                return Err(Diagnostic::error(
                    "a structure containing a flexible array member cannot be a member of another structure",
                    start,
                ));
            }
            let end = self.expect(TokenKind::RBrace)?.span;
            let (members, size, align) = self.layout_record_members(kind, members)?;
            self.records.insert(
                id,
                RecordType {
                    id,
                    kind,
                    tag: tag.clone(),
                    complete: true,
                    members,
                    size,
                    align,
                },
            );
            let span = start.merge(end);
            let _ = span;
        } else if tag.is_none() {
            let kind_name = match kind {
                RecordKind::Struct => "struct",
                RecordKind::Union => "union",
            };
            return Err(Diagnostic::error(
                format!("{kind_name} type specifier requires a tag or a definition"),
                start,
            ));
        }
        Ok(match kind {
            RecordKind::Struct => CType::Struct(id, tag),
            RecordKind::Union => CType::Union(id, tag),
        })
    }

    fn parse_record_members(&mut self) -> Result<Vec<RecordMember>, Diagnostic> {
        let mut members = Vec::new();
        let mut visible_names = HashSet::new();
        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) {
                return Err(Diagnostic::error(
                    "struct or union definition is missing a closing brace",
                    self.current_span(),
                ));
            }
            if self.at_keyword(Keyword::StaticAssert) {
                self.parse_static_assertion()?;
                continue;
            }
            self.skip_gnu_attributes()?;
            let member_start = self.current_span();
            let specs = self.parse_declaration_specifiers(DeclContext::RecordMember)?;
            let base = specs.base_type;
            let mut parsed_members = Vec::new();
            if self.at(TokenKind::Colon) {
                let (width, width_span) = self.parse_bit_field_width()?;
                parsed_members.push(RecordMember {
                    name: None,
                    storage_name: self.alloc_member_storage_name("__bitfield"),
                    ty: base,
                    offset: 0,
                    bit_width: Some(width),
                    bit_width_span: Some(width_span),
                    bit_offset: 0,
                    bit_storage_size: 0,
                    declaration_span: None,
                });
            } else if self.at(TokenKind::Semicolon) {
                self.validate_anonymous_record_member_type(&base, self.current_span())?;
                parsed_members.push(RecordMember {
                    name: None,
                    storage_name: self.alloc_member_storage_name("__anon"),
                    ty: base,
                    offset: 0,
                    bit_width: None,
                    bit_width_span: None,
                    bit_offset: 0,
                    bit_storage_size: 0,
                    declaration_span: None,
                });
            } else {
                let (first_name, first_ty, first_vla_bounds, _, first_span, function_params) =
                    self.parse_declarator(base.clone(), DeclContext::RecordMember)?;
                if function_params.is_some() || first_ty.is_function() {
                    if self.at(TokenKind::LBrace) && !members.is_empty() {
                        return Err(Diagnostic::error(
                            "struct or union definition is missing } and ; before this function definition",
                            member_start.merge(first_span),
                        ));
                    }
                    return Err(Diagnostic::error(
                        "record members cannot have function type",
                        first_span,
                    ));
                }
                if first_vla_bounds.iter().any(|bound| bound.is_some()) {
                    return Err(Diagnostic::error(
                        "record members cannot have variable length array type",
                        first_span,
                    ));
                }
                let (bit_width, bit_width_span) = if self.at(TokenKind::Colon) {
                    let (width, span) = self.parse_bit_field_width()?;
                    (Some(width), Some(span))
                } else {
                    (None, None)
                };
                parsed_members.push(RecordMember {
                    name: Some(first_name.clone()),
                    storage_name: first_name,
                    ty: first_ty.clone(),
                    offset: 0,
                    bit_width,
                    bit_width_span,
                    bit_offset: 0,
                    bit_storage_size: 0,
                    declaration_span: Some(first_span),
                });
                while self.eat(TokenKind::Comma) {
                    let (name, ty, vla_bounds, _, span, function_params) =
                        self.parse_declarator(base.clone(), DeclContext::RecordMember)?;
                    if function_params.is_some() || ty.is_function() {
                        return Err(Diagnostic::error(
                            "record members cannot have function type",
                            span,
                        ));
                    }
                    if vla_bounds.iter().any(|bound| bound.is_some()) {
                        return Err(Diagnostic::error(
                            "record members cannot have variable length array type",
                            span,
                        ));
                    }
                    let (bit_width, bit_width_span) = if self.at(TokenKind::Colon) {
                        let (width, span) = self.parse_bit_field_width()?;
                        (Some(width), Some(span))
                    } else {
                        (None, None)
                    };
                    parsed_members.push(RecordMember {
                        name: Some(name.clone()),
                        storage_name: name,
                        ty,
                        offset: 0,
                        bit_width,
                        bit_width_span,
                        bit_offset: 0,
                        bit_storage_size: 0,
                        declaration_span: Some(span),
                    });
                }
            }
            for member in parsed_members {
                let member_span = member.declaration_span.unwrap_or_else(|| {
                    member.bit_width_span.unwrap_or_else(|| self.current_span())
                });
                self.validate_record_member_type(&member, member_span)?;
                for visible_name in self.visible_member_names_for(&member)? {
                    if !visible_names.insert(visible_name.clone()) {
                        return Err(Diagnostic::error(
                            format!("duplicate member declaration {}", visible_name),
                            member_span,
                        ));
                    }
                }
                members.push(member);
            }
            self.expect(TokenKind::Semicolon)?;
        }
        Ok(members)
    }

    fn validate_record_member_type(
        &self,
        member: &RecordMember,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.validate_restrict_usage(&member.ty, span)?;
        if let Some(width) = member.bit_width {
            if !member.ty.is_integer() {
                return Err(Diagnostic::error(
                    "bit-field must have an integer or _Bool type",
                    span,
                ));
            }
            let max_width = self.bit_field_precision(&member.ty).ok_or_else(|| {
                Diagnostic::error("bit-field must have an integer or _Bool type", span)
            })?;
            if width > max_width {
                return Err(Diagnostic::error(
                    "bit-field width exceeds the width of its type",
                    span,
                ));
            }
            if width == 0 && member.name.is_some() {
                return Err(Diagnostic::error(
                    "zero-width bit-field must be unnamed",
                    member.bit_width_span.unwrap_or(span),
                ));
            }
            return Ok(());
        }
        match member.ty.unqualified() {
            CType::Void => Err(Diagnostic::error(
                "record members cannot have void type",
                span,
            )),
            CType::Function(_, _, _) => Err(Diagnostic::error(
                "record members cannot have function type",
                span,
            )),
            CType::Array(_, 0) => Ok(()),
            _ if !self.type_is_complete(&member.ty) => Err(Diagnostic::error(
                format!("record member has incomplete type {}", member.ty),
                span,
            )),
            _ => Ok(()),
        }
    }

    fn layout_record_members(
        &self,
        kind: RecordKind,
        mut members: Vec<RecordMember>,
    ) -> Result<(Vec<RecordMember>, usize, usize), Diagnostic> {
        let mut size = 0usize;
        let mut align = 1usize;
        let mut open_bit_size = 0usize;
        let mut open_bit_bits = 0u8;
        let mut used_bits = 0u8;
        let mut open_bit_offset = 0usize;
        match kind {
            RecordKind::Struct => {
                for member in &mut members {
                    if let Some(width) = member.bit_width {
                        let storage_size = self.type_size_of(&member.ty)?;
                        let storage_align = self.type_align_of(&member.ty)?;
                        let storage_bits = self.bit_field_precision(&member.ty).unwrap();
                        member.bit_storage_size = storage_size;
                        align = align.max(storage_align);
                        if width == 0 {
                            size = align_up(size, storage_align);
                            open_bit_size = 0;
                            open_bit_bits = 0;
                            used_bits = 0;
                            member.offset = size;
                            member.bit_offset = 0;
                            continue;
                        }
                        let needs_new_unit = open_bit_size == 0
                            || open_bit_size != storage_size
                            || open_bit_bits != storage_bits
                            || used_bits + width > storage_bits;
                        if needs_new_unit {
                            size = align_up(size, storage_align);
                            open_bit_offset = size;
                            size += storage_size;
                            open_bit_size = storage_size;
                            open_bit_bits = storage_bits;
                            used_bits = 0;
                        }
                        member.offset = open_bit_offset;
                        member.bit_offset = used_bits;
                        used_bits += width;
                        continue;
                    }
                    open_bit_size = 0;
                    open_bit_bits = 0;
                    used_bits = 0;
                    let member_align = self.type_align_of(&member.ty)?;
                    align = align.max(member_align);
                    size = align_up(size, member_align);
                    member.offset = size;
                    if !matches!(member.ty.unqualified(), CType::Array(_, 0)) {
                        size += self.type_size_of(&member.ty)?;
                    }
                }
                size = align_up(size, align);
            }
            RecordKind::Union => {
                for member in &mut members {
                    let member_align = self.type_align_of(&member.ty)?;
                    align = align.max(member_align);
                    member.offset = 0;
                    if member.bit_width.is_some() {
                        member.bit_offset = 0;
                        member.bit_storage_size = self.type_size_of(&member.ty)?;
                    }
                    size = size.max(self.type_size_of(&member.ty)?);
                }
                size = align_up(size, align);
            }
        }
        Ok((members, size, align))
    }

    fn parse_bit_field_width(&mut self) -> Result<(u8, Span), Diagnostic> {
        self.expect(TokenKind::Colon)?;
        let expr = self.parse_assignment()?;
        let span = expr.span();
        let value = self.eval_integer_constant_expr(&expr)?;
        if value < 0 {
            return Err(Diagnostic::error(
                "bit-field width must be non-negative",
                expr.span(),
            ));
        }
        let width = u8::try_from(value).map_err(|_| {
            Diagnostic::error("bit-field width is out of supported range", expr.span())
        })?;
        Ok((width, span))
    }

    fn validate_anonymous_record_member_type(
        &self,
        ty: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match ty.unqualified() {
            CType::Struct(_, _) | CType::Union(_, _) if self.type_is_complete(ty) => Ok(()),
            CType::Struct(_, _) | CType::Union(_, _) => Err(Diagnostic::error(
                format!("anonymous member has incomplete type {ty}"),
                span,
            )),
            _ => Err(Diagnostic::error(
                format!("declaration of type {ty} requires a name"),
                span,
            )),
        }
    }

    fn bit_field_precision(&self, ty: &CType) -> Option<u8> {
        match ty.unqualified() {
            CType::Bool => Some(1),
            _ if ty.is_integer() => ty.integer_bits().and_then(|bits| u8::try_from(bits).ok()),
            _ => None,
        }
    }

    fn visible_member_names_for(&self, member: &RecordMember) -> Result<Vec<String>, Diagnostic> {
        if let Some(name) = &member.name {
            return Ok(vec![name.clone()]);
        }
        if member.bit_width.is_some() {
            return Ok(Vec::new());
        }
        self.collect_visible_member_names(&member.ty)
    }

    fn collect_visible_member_names(&self, ty: &CType) -> Result<Vec<String>, Diagnostic> {
        let Some(record) = (match ty.unqualified() {
            CType::Struct(id, _) | CType::Union(id, _) => self.records.get(id),
            _ => None,
        }) else {
            return Ok(Vec::new());
        };
        let mut names = Vec::new();
        for member in &record.members {
            if let Some(name) = &member.name {
                names.push(name.clone());
            } else if member.bit_width.is_none() {
                names.extend(self.collect_visible_member_names(&member.ty)?);
            }
        }
        Ok(names)
    }

    fn parse_enum_specifier(&mut self) -> Result<CType, Diagnostic> {
        let start = self.bump().span;
        let tag = if let Some(TokenKind::Identifier(name)) = self.peek_kind(0) {
            let name = name.clone();
            self.bump();
            Some(name)
        } else {
            None
        };
        let is_definition = self.at(TokenKind::LBrace);
        let declares_tag_in_current_scope = is_definition || self.at(TokenKind::Semicolon);
        let id = match &tag {
            Some(name) => {
                let (id, is_new) =
                    self.resolve_tag(name, TagKind::Enum, declares_tag_in_current_scope, start)?;
                if is_new {
                    self.enums.insert(
                        id,
                        EnumType {
                            id,
                            tag: Some(name.clone()),
                            complete: false,
                        },
                    );
                }
                id
            }
            None => self.alloc_tag_id(),
        };
        if self.eat(TokenKind::LBrace) {
            if self.enums.get(&id).is_some_and(|enum_ty| enum_ty.complete) {
                return Err(Diagnostic::error(
                    "enum type is already complete and cannot be redefined",
                    start,
                ));
            }
            let mut next_value = 0i128;
            loop {
                let token = self.bump().clone();
                let name = match token.kind {
                    TokenKind::Identifier(name) => name,
                    _ => return Err(Diagnostic::error("expected enumerator name", token.span)),
                };
                let value = if self.eat(TokenKind::Equal) {
                    let expr = self.parse_assignment()?;
                    self.eval_integer_constant_expr(&expr)?
                } else {
                    next_value
                };
                let (int_min, int_max) =
                    CType::Int.integer_bounds().expect("int has integer bounds");
                if !(int_min..=int_max).contains(&value) {
                    return Err(Diagnostic::error(
                        "enumerator value must be representable as int",
                        token.span,
                    ));
                }
                self.declare_enum_constant(&name, value, token.span)?;
                if self.block_scopes.is_empty() {
                    self.enum_constants.insert(name, value);
                }
                next_value = value
                    .checked_add(1)
                    .ok_or_else(|| Diagnostic::error("enumerator value overflow", token.span))?;
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                if self.at(TokenKind::RBrace) {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
            self.enums.insert(
                id,
                EnumType {
                    id,
                    tag: tag.clone(),
                    complete: true,
                },
            );
        } else if tag.is_none() {
            return Err(Diagnostic::error(
                "enum specifier requires a tag or a definition",
                start,
            ));
        }
        Ok(CType::Enum(id, tag))
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

    fn validate_function_labels(&self, body: &Block) -> Result<(), Diagnostic> {
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

    fn eval_integer_constant_expr(&self, expr: &Expr) -> Result<i128, Diagnostic> {
        Ok(self.eval_typed_integer_constant_expr(expr)?.value)
    }

    fn eval_typed_integer_constant_expr(&self, expr: &Expr) -> Result<ConstantInteger, Diagnostic> {
        match expr {
            Expr::Number(literal, span) => match literal.value {
                NumberValue::Integer(value) => Ok(ConstantInteger {
                    ty: literal.ty.clone(),
                    value,
                }),
                NumberValue::Floating(_) => Err(Diagnostic::error(
                    "an integer constant expression cannot use a floating-point value",
                    *span,
                )),
            },
            Expr::CharLiteral(value, _) => Ok(ConstantInteger {
                ty: CType::Int,
                value: *value as i128,
            }),
            Expr::Variable(name, span) => self
                .lookup_enum_constant_value(name)
                .map(|value| ConstantInteger {
                    ty: CType::Int,
                    value,
                })
                .ok_or_else(|| {
                    Diagnostic::error(
                        format!("identifier {} is not an integer constant expression", name),
                        *span,
                    )
                }),
            Expr::Unary { op, expr, span } => {
                let value = self.eval_typed_integer_constant_expr(expr)?;
                match op {
                    UnaryOp::Plus => self.promote_constant_integer(value),
                    UnaryOp::Minus => {
                        let value = self.promote_constant_integer(value)?;
                        self.constant_integer_negate(value, *span)
                    }
                    UnaryOp::LogicalNot => Ok(ConstantInteger {
                        ty: CType::Int,
                        value: (value.value == 0) as i128,
                    }),
                    UnaryOp::BitNot => {
                        let value = self.promote_constant_integer(value)?;
                        Ok(ConstantInteger {
                            value: value
                                .ty
                                .normalize_integer_value(!value.value)
                                .expect("promoted integer has an integer representation"),
                            ty: value.ty,
                        })
                    }
                    _ => Err(Diagnostic::error(
                        "unsupported operator in integer constant expression",
                        *span,
                    )),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.eval_typed_integer_constant_expr(lhs)?;
                if *op == BinaryOp::LogicalAnd && lhs.value == 0 {
                    return Ok(ConstantInteger {
                        ty: CType::Int,
                        value: 0,
                    });
                }
                if *op == BinaryOp::LogicalOr && lhs.value != 0 {
                    return Ok(ConstantInteger {
                        ty: CType::Int,
                        value: 1,
                    });
                }
                let rhs = self.eval_typed_integer_constant_expr(rhs)?;
                self.eval_constant_integer_binary(*op, lhs, rhs, *span)
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let result_ty = self.usual_constant_integer_type(
                    &self.integer_constant_expr_type(then_expr)?,
                    &self.integer_constant_expr_type(else_expr)?,
                );
                let selected = if self.eval_typed_integer_constant_expr(condition)?.value != 0 {
                    self.eval_typed_integer_constant_expr(then_expr)
                } else {
                    self.eval_typed_integer_constant_expr(else_expr)
                }?;
                Ok(self.convert_constant_integer(selected, &result_ty))
            }
            Expr::Cast { ty, expr, span, .. } => {
                if !ty.is_integer() {
                    return Err(Diagnostic::error(
                        "integer constant expression must have integer type",
                        *span,
                    ));
                }
                let value = self.eval_typed_integer_constant_expr(expr)?;
                Ok(self.convert_constant_integer(value, ty))
            }
            Expr::SizeofType {
                ty,
                vla_bounds,
                span,
            } => {
                if vla_bounds.iter().any(Option::is_some) {
                    return Err(Diagnostic::error(
                        "sizeof a variably modified type is not an integer constant expression",
                        *span,
                    ));
                }
                Ok(ConstantInteger {
                    ty: CType::UnsignedLong,
                    value: i128::try_from(self.type_size_of(ty)?)
                        .map_err(|_| Diagnostic::error("sizeof result is too large", *span))?,
                })
            }
            Expr::OffsetOf {
                ty,
                designators,
                span,
            } => Ok(ConstantInteger {
                ty: CType::UnsignedLong,
                value: self.eval_offsetof_constant_expr(ty, designators, *span)?,
            }),
            _ => Err(Diagnostic::error(
                "expression is not a supported integer constant expression",
                expr.span(),
            )),
        }
    }

    fn promote_constant_integer(
        &self,
        value: ConstantInteger,
    ) -> Result<ConstantInteger, Diagnostic> {
        let promoted = match value.ty.unqualified() {
            CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(_, _) => CType::Int,
            _ if value.ty.is_integer() => value.ty.clone(),
            _ => {
                return Err(Diagnostic::error(
                    "integer constant expression requires integer operands",
                    Span::new(crate::source::FileId(0), 0, 0),
                ));
            }
        };
        Ok(self.convert_constant_integer(value, &promoted))
    }

    fn convert_constant_integer(&self, value: ConstantInteger, target: &CType) -> ConstantInteger {
        ConstantInteger {
            value: target
                .normalize_integer_value(value.value)
                .expect("integer target has an integer representation"),
            ty: target.clone(),
        }
    }

    fn usual_constant_integer_type(&self, lhs: &CType, rhs: &CType) -> CType {
        if lhs == rhs {
            return lhs.clone();
        }
        if lhs.is_signed_integer() == rhs.is_signed_integer() {
            return if lhs.integer_rank() >= rhs.integer_rank() {
                lhs.clone()
            } else {
                rhs.clone()
            };
        }
        let (signed_ty, unsigned_ty) = if lhs.is_signed_integer() {
            (lhs, rhs)
        } else {
            (rhs, lhs)
        };
        if signed_ty.integer_rank() > unsigned_ty.integer_rank() {
            let (_, signed_max) = signed_ty.integer_bounds().unwrap();
            let (_, unsigned_max) = unsigned_ty.integer_bounds().unwrap();
            if signed_max >= unsigned_max {
                signed_ty.clone()
            } else {
                signed_ty.unsigned_variant().unwrap()
            }
        } else {
            unsigned_ty.clone()
        }
    }

    fn constant_integer_negate(
        &self,
        value: ConstantInteger,
        span: Span,
    ) -> Result<ConstantInteger, Diagnostic> {
        if value.ty.is_unsigned_integer() {
            return Ok(ConstantInteger {
                value: value
                    .ty
                    .normalize_integer_value(-value.value)
                    .expect("unsigned integer has an integer representation"),
                ty: value.ty,
            });
        }
        let negated = value
            .value
            .checked_neg()
            .filter(|result| {
                value
                    .ty
                    .integer_bounds()
                    .is_some_and(|(min, max)| (min..=max).contains(result))
            })
            .ok_or_else(|| Diagnostic::error("constant expression overflow", span))?;
        Ok(ConstantInteger {
            ty: value.ty,
            value: negated,
        })
    }

    fn eval_constant_integer_binary(
        &self,
        op: BinaryOp,
        lhs: ConstantInteger,
        rhs: ConstantInteger,
        span: Span,
    ) -> Result<ConstantInteger, Diagnostic> {
        if op == BinaryOp::Comma {
            return Err(Diagnostic::error(
                "comma operator is not allowed in an evaluated integer constant expression",
                span,
            ));
        }
        if matches!(op, BinaryOp::LogicalAnd | BinaryOp::LogicalOr) {
            let value = match op {
                BinaryOp::LogicalAnd => lhs.value != 0 && rhs.value != 0,
                BinaryOp::LogicalOr => lhs.value != 0 || rhs.value != 0,
                _ => unreachable!(),
            };
            return Ok(ConstantInteger {
                ty: CType::Int,
                value: value as i128,
            });
        }
        if matches!(op, BinaryOp::ShiftLeft | BinaryOp::ShiftRight) {
            let lhs = self.promote_constant_integer(lhs)?;
            let rhs = self.promote_constant_integer(rhs)?;
            let bits = lhs.ty.integer_bits().unwrap() as i128;
            if !(0..bits).contains(&rhs.value) {
                return Err(Diagnostic::error(
                    "shift count is negative or too large in constant expression",
                    span,
                ));
            }
            let shift = rhs.value as u32;
            let shifted = match op {
                BinaryOp::ShiftLeft if lhs.ty.is_unsigned_integer() => lhs
                    .ty
                    .normalize_integer_value(lhs.value << shift)
                    .expect("unsigned integer has an integer representation"),
                BinaryOp::ShiftLeft => {
                    if lhs.value < 0 {
                        return Err(Diagnostic::error(
                            "left shift of a negative value in constant expression",
                            span,
                        ));
                    }
                    let result = lhs.value << shift;
                    if !lhs
                        .ty
                        .integer_bounds()
                        .is_some_and(|(min, max)| (min..=max).contains(&result))
                    {
                        return Err(Diagnostic::error("constant expression overflow", span));
                    }
                    result
                }
                BinaryOp::ShiftRight => lhs.value >> shift,
                _ => unreachable!(),
            };
            return Ok(ConstantInteger {
                ty: lhs.ty,
                value: shifted,
            });
        }

        let lhs = self.promote_constant_integer(lhs)?;
        let rhs = self.promote_constant_integer(rhs)?;
        let common_ty = self.usual_constant_integer_type(&lhs.ty, &rhs.ty);
        let lhs = self.convert_constant_integer(lhs, &common_ty).value;
        let rhs = self.convert_constant_integer(rhs, &common_ty).value;
        let comparison = |value: bool| ConstantInteger {
            ty: CType::Int,
            value: value as i128,
        };
        if matches!(
            op,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        ) {
            return Ok(comparison(match op {
                BinaryOp::Equal => lhs == rhs,
                BinaryOp::NotEqual => lhs != rhs,
                BinaryOp::Less => lhs < rhs,
                BinaryOp::LessEqual => lhs <= rhs,
                BinaryOp::Greater => lhs > rhs,
                BinaryOp::GreaterEqual => lhs >= rhs,
                _ => unreachable!(),
            }));
        }
        if matches!(op, BinaryOp::Div | BinaryOp::Rem) && rhs == 0 {
            return Err(Diagnostic::error(
                "division by zero in constant expression",
                span,
            ));
        }
        if common_ty.is_unsigned_integer()
            && matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
        {
            let bits = common_ty.integer_bits().unwrap();
            let mask = (1u128 << bits) - 1;
            let lhs = lhs as u128;
            let rhs = rhs as u128;
            let value = match op {
                BinaryOp::Add => lhs.wrapping_add(rhs),
                BinaryOp::Sub => lhs.wrapping_sub(rhs),
                BinaryOp::Mul => lhs.wrapping_mul(rhs),
                _ => unreachable!(),
            } & mask;
            return Ok(ConstantInteger {
                ty: common_ty,
                value: value as i128,
            });
        }
        let raw = match op {
            BinaryOp::Add => lhs + rhs,
            BinaryOp::Sub => lhs - rhs,
            BinaryOp::Mul => lhs * rhs,
            BinaryOp::Div => {
                if common_ty.is_signed_integer()
                    && common_ty
                        .integer_bounds()
                        .is_some_and(|(min, _)| lhs == min && rhs == -1)
                {
                    return Err(Diagnostic::error("constant expression overflow", span));
                }
                lhs / rhs
            }
            BinaryOp::Rem => {
                if common_ty.is_signed_integer()
                    && common_ty
                        .integer_bounds()
                        .is_some_and(|(min, _)| lhs == min && rhs == -1)
                {
                    return Err(Diagnostic::error("constant expression overflow", span));
                }
                lhs % rhs
            }
            BinaryOp::BitAnd => lhs & rhs,
            BinaryOp::BitXor => lhs ^ rhs,
            BinaryOp::BitOr => lhs | rhs,
            _ => unreachable!(),
        };
        if common_ty.is_signed_integer()
            && !common_ty
                .integer_bounds()
                .is_some_and(|(min, max)| (min..=max).contains(&raw))
        {
            return Err(Diagnostic::error("constant expression overflow", span));
        }
        Ok(ConstantInteger {
            value: common_ty
                .normalize_integer_value(raw)
                .expect("integer result has an integer representation"),
            ty: common_ty,
        })
    }

    fn integer_constant_expr_type(&self, expr: &Expr) -> Result<CType, Diagnostic> {
        match expr {
            Expr::Number(literal, span) => match literal.value {
                NumberValue::Integer(_) => Ok(literal.ty.clone()),
                NumberValue::Floating(_) => Err(Diagnostic::error(
                    "an integer constant expression cannot use a floating-point value",
                    *span,
                )),
            },
            Expr::CharLiteral(_, _) | Expr::Variable(_, _) => Ok(CType::Int),
            Expr::Unary { op, expr, span } => match op {
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::BitNot => Ok(self
                    .promote_constant_integer(ConstantInteger {
                        ty: self.integer_constant_expr_type(expr)?,
                        value: 0,
                    })?
                    .ty),
                UnaryOp::LogicalNot => Ok(CType::Int),
                _ => Err(Diagnostic::error(
                    "unsupported operator in integer constant expression",
                    *span,
                )),
            },
            Expr::Binary { op, lhs, rhs, span } => match op {
                BinaryOp::LogicalAnd
                | BinaryOp::LogicalOr
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => Ok(CType::Int),
                BinaryOp::ShiftLeft | BinaryOp::ShiftRight => Ok(self
                    .promote_constant_integer(ConstantInteger {
                        ty: self.integer_constant_expr_type(lhs)?,
                        value: 0,
                    })?
                    .ty),
                BinaryOp::Comma => Err(Diagnostic::error(
                    "comma operator is not allowed in an evaluated integer constant expression",
                    *span,
                )),
                _ => {
                    let lhs = self.promote_constant_integer(ConstantInteger {
                        ty: self.integer_constant_expr_type(lhs)?,
                        value: 0,
                    })?;
                    let rhs = self.promote_constant_integer(ConstantInteger {
                        ty: self.integer_constant_expr_type(rhs)?,
                        value: 0,
                    })?;
                    Ok(self.usual_constant_integer_type(&lhs.ty, &rhs.ty))
                }
            },
            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => Ok(self.usual_constant_integer_type(
                &self.integer_constant_expr_type(then_expr)?,
                &self.integer_constant_expr_type(else_expr)?,
            )),
            Expr::Cast { ty, span, .. } if ty.is_integer() => Ok(ty.clone()),
            Expr::Cast { span, .. } => Err(Diagnostic::error(
                "integer constant expression must have integer type",
                *span,
            )),
            Expr::SizeofType { .. } | Expr::OffsetOf { .. } => Ok(CType::UnsignedLong),
            _ => Err(Diagnostic::error(
                "expression is not a supported integer constant expression",
                expr.span(),
            )),
        }
    }

    fn eval_offsetof_constant_expr(
        &self,
        ty: &CType,
        designators: &[Designator],
        span: Span,
    ) -> Result<i128, Diagnostic> {
        let mut current = ty.clone();
        let mut offset = 0usize;
        for designator in designators {
            match designator {
                Designator::Member(name, designator_span) => {
                    let chain = self
                        .resolve_visible_member_chain(&current, name)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                format!("{} has no member named {}", current, name),
                                *designator_span,
                            )
                        })?;
                    for member in chain {
                        offset = offset
                            .checked_add(member.offset)
                            .ok_or_else(|| Diagnostic::error("offsetof overflow", span))?;
                        current = member.ty.clone();
                    }
                }
                Designator::Index(index, designator_span) => match current.unqualified() {
                    CType::Array(inner, len) => {
                        if *index >= *len {
                            return Err(Diagnostic::error(
                                "offsetof array designator is outside the bounds of the array",
                                *designator_span,
                            ));
                        }
                        let stride = self.type_size_of(inner)?;
                        offset = offset
                            .checked_add(index.checked_mul(stride).ok_or_else(|| {
                                Diagnostic::error("offsetof overflow", *designator_span)
                            })?)
                            .ok_or_else(|| {
                                Diagnostic::error("offsetof overflow", *designator_span)
                            })?;
                        current = (**inner).clone();
                    }
                    _ => {
                        return Err(Diagnostic::error(
                            "offsetof array designator requires an array type",
                            *designator_span,
                        ));
                    }
                },
            }
        }
        i128::try_from(offset).map_err(|_| Diagnostic::error("offsetof overflow", span))
    }

    fn resolve_visible_member_chain(&self, ty: &CType, member: &str) -> Option<Vec<RecordMember>> {
        let record = match ty.unqualified() {
            CType::Struct(id, _) | CType::Union(id, _) => self.records.get(id)?,
            _ => return None,
        };
        for candidate in &record.members {
            if candidate.name.as_deref() == Some(member) {
                return Some(vec![candidate.clone()]);
            }
        }
        for candidate in &record.members {
            if candidate.name.is_none()
                && candidate.bit_width.is_none()
                && matches!(
                    candidate.ty.unqualified(),
                    CType::Struct(_, _) | CType::Union(_, _)
                )
            {
                if let Some(mut tail) = self.resolve_visible_member_chain(&candidate.ty, member) {
                    let mut chain = vec![candidate.clone()];
                    chain.append(&mut tail);
                    return Some(chain);
                }
            }
        }
        None
    }

    fn type_is_complete(&self, ty: &CType) -> bool {
        match ty.unqualified() {
            CType::Void | CType::Function(_, _, _) => false,
            CType::Array(inner, len) => *len != 0 && self.type_is_complete(inner),
            CType::Struct(id, _) | CType::Union(id, _) => {
                self.records.get(id).is_some_and(|record| record.complete)
            }
            CType::Enum(id, _) => self.enums.get(id).is_some_and(|enum_ty| enum_ty.complete),
            _ => true,
        }
    }

    fn type_contains_flexible_array_structure(&self, ty: &CType) -> bool {
        match ty.unqualified() {
            CType::Struct(id, _) => self.records.get(id).is_some_and(|record| {
                record
                    .members
                    .last()
                    .is_some_and(|member| matches!(member.ty.unqualified(), CType::Array(_, 0)))
            }),
            CType::Union(id, _) => self.records.get(id).is_some_and(|record| {
                record
                    .members
                    .iter()
                    .any(|member| self.type_contains_flexible_array_structure(&member.ty))
            }),
            CType::Array(inner, _) => self.type_contains_flexible_array_structure(inner),
            _ => false,
        }
    }

    fn validate_parameter_type(&self, ty: &CType, span: Span) -> Result<(), Diagnostic> {
        if matches!(ty.unqualified(), CType::Void | CType::Function(..))
            || !self.type_is_complete(ty)
        {
            return Err(Diagnostic::error(
                "a function parameter must have complete object type after adjustment",
                span,
            ));
        }
        Ok(())
    }

    fn type_size_of(&self, ty: &CType) -> Result<usize, Diagnostic> {
        match ty.unqualified() {
            CType::Array(inner, len) if *len != 0 => {
                self.type_size_of(inner)?.checked_mul(*len).ok_or_else(|| {
                    Diagnostic::error("type size is out of supported range", self.current_span())
                })
            }
            CType::Struct(id, _) | CType::Union(id, _) => self
                .records
                .get(id)
                .filter(|record| record.complete)
                .map(|record| record.size)
                .ok_or_else(|| {
                    Diagnostic::error(format!("type is incomplete: {ty}"), self.current_span())
                }),
            _ => ty.size_of().ok_or_else(|| {
                Diagnostic::error(format!("type is incomplete: {ty}"), self.current_span())
            }),
        }
    }

    fn type_align_of(&self, ty: &CType) -> Result<usize, Diagnostic> {
        match ty.unqualified() {
            CType::Bool => Ok(1),
            CType::Char | CType::SignedChar | CType::UnsignedChar => Ok(1),
            CType::Float => Ok(4),
            CType::Short | CType::UnsignedShort => Ok(2),
            CType::Int | CType::UnsignedInt | CType::Enum(_, _) => Ok(4),
            CType::Complex(inner) => self.type_align_of(inner),
            CType::Long
            | CType::UnsignedLong
            | CType::LongLong
            | CType::UnsignedLongLong
            | CType::Pointer(_)
            | CType::VaList => Ok(8),
            CType::Double => Ok(8),
            CType::LongDouble => Ok(HOST_LONG_DOUBLE_ALIGN),
            CType::Struct(id, _) | CType::Union(id, _) => self
                .records
                .get(id)
                .filter(|record| record.complete)
                .map(|record| record.align)
                .ok_or_else(|| Diagnostic::error("type is incomplete", self.current_span())),
            CType::Array(inner, _) => self.type_align_of(inner),
            CType::Qualified(inner, _) => self.type_align_of(inner),
            CType::Void | CType::Function(_, _, _) => {
                Err(Diagnostic::error("type is incomplete", self.current_span()))
            }
        }
    }

    fn alloc_tag_id(&mut self) -> usize {
        let id = self.next_tag_id;
        self.next_tag_id += 1;
        id
    }

    fn alloc_member_storage_name(&mut self, prefix: &str) -> String {
        let id = self.next_member_id;
        self.next_member_id += 1;
        format!("{prefix}{id}")
    }

    fn is_declaration_start(&self) -> bool {
        self.at_keyword(Keyword::Alignas) || self.starts_declaration_specifier_at(0)
    }

    fn is_type_name_start(&self) -> bool {
        self.peek_kind(0) == Some(&TokenKind::LParen) && self.starts_type_name_at(1)
    }

    fn starts_declaration_specifier_at(&self, mut offset: usize) -> bool {
        offset = self.skip_gnu_attributes_at_offset(offset);
        while matches!(
            self.peek_kind(offset),
            Some(TokenKind::Keyword(
                Keyword::Auto
                    | Keyword::Const
                    | Keyword::Extern
                    | Keyword::Inline
                    | Keyword::Noreturn
                    | Keyword::Register
                    | Keyword::Restrict
                    | Keyword::Static
                    | Keyword::Typedef
                    | Keyword::Volatile
            ))
        ) {
            offset += 1;
        }
        matches!(
            self.peek_kind(offset),
            Some(TokenKind::Keyword(
                Keyword::Bool
                    | Keyword::Complex
                    | Keyword::Double
                    | Keyword::Char
                    | Keyword::Enum
                    | Keyword::Float
                    | Keyword::Int
                    | Keyword::Long
                    | Keyword::Short
                    | Keyword::Signed
                    | Keyword::Struct
                    | Keyword::Unsigned
                    | Keyword::Union
                    | Keyword::Void
            ))
        ) || matches!(
            self.peek_kind(offset),
            Some(TokenKind::Identifier(name)) if self.lookup_typedef_name(name).is_some()
        )
    }

    fn starts_type_name_at(&self, mut offset: usize) -> bool {
        offset = self.skip_gnu_attributes_at_offset(offset);
        while matches!(
            self.peek_kind(offset),
            Some(TokenKind::Keyword(
                Keyword::Const | Keyword::Restrict | Keyword::Volatile
            ))
        ) {
            offset += 1;
        }
        matches!(
            self.peek_kind(offset),
            Some(TokenKind::Keyword(
                Keyword::Bool
                    | Keyword::Complex
                    | Keyword::Double
                    | Keyword::Char
                    | Keyword::Enum
                    | Keyword::Float
                    | Keyword::Int
                    | Keyword::Long
                    | Keyword::Short
                    | Keyword::Signed
                    | Keyword::Struct
                    | Keyword::Unsigned
                    | Keyword::Union
                    | Keyword::Void
            ))
        ) || matches!(
            self.peek_kind(offset),
            Some(TokenKind::Identifier(name)) if self.lookup_typedef_name(name).is_some()
        )
    }

    fn validate_decl_specifier_context(
        &self,
        storage_class: Option<ParsedStorageClass>,
        is_inline: bool,
        context: DeclContext,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match context {
            DeclContext::FileScope => {
                if matches!(
                    storage_class,
                    Some(ParsedStorageClass::Auto | ParsedStorageClass::Register)
                ) {
                    return Err(Diagnostic::error(
                        "auto and register are invalid at file scope",
                        span,
                    ));
                }
            }
            DeclContext::BlockScope => {}
            DeclContext::Parameter => {
                if matches!(
                    storage_class,
                    Some(
                        ParsedStorageClass::Auto
                            | ParsedStorageClass::Extern
                            | ParsedStorageClass::Static
                            | ParsedStorageClass::Typedef
                    )
                ) {
                    return Err(Diagnostic::error(
                        "only register is allowed in parameter declarations",
                        span,
                    ));
                }
                if is_inline {
                    return Err(Diagnostic::error(
                        "inline is not allowed in parameter declarations",
                        span,
                    ));
                }
            }
            DeclContext::RecordMember | DeclContext::TypeName => {
                if storage_class.is_some() || is_inline {
                    return Err(Diagnostic::error(
                        "storage class specifiers and inline are not allowed here",
                        span,
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_function_decl_specifiers(
        &self,
        specs: &DeclarationSpecifiers,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if matches!(
            specs.storage_class,
            Some(
                ParsedStorageClass::Auto
                    | ParsedStorageClass::Register
                    | ParsedStorageClass::Typedef
            )
        ) {
            return Err(Diagnostic::error(
                "invalid storage class on function declaration",
                span,
            ));
        }
        if specs.alignment.is_some() {
            return Err(Diagnostic::error(
                "_Alignas is not valid on functions",
                span,
            ));
        }
        Ok(())
    }

    fn validate_parameter_decl_specifiers(
        &self,
        specs: &DeclarationSpecifiers,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if specs.is_inline {
            return Err(Diagnostic::error(
                "inline is not allowed in parameter declarations",
                span,
            ));
        }
        if matches!(
            specs.storage_class,
            Some(
                ParsedStorageClass::Auto
                    | ParsedStorageClass::Extern
                    | ParsedStorageClass::Static
                    | ParsedStorageClass::Typedef
            )
        ) {
            return Err(Diagnostic::error(
                "only register is allowed in parameter declarations",
                span,
            ));
        }
        Ok(())
    }

    fn validate_restrict_usage(&self, ty: &CType, span: Span) -> Result<(), Diagnostic> {
        match ty {
            CType::Qualified(inner, qualifiers) if qualifiers.is_restrict => {
                let CType::Pointer(pointee) = inner.unqualified() else {
                    return Err(Diagnostic::error(
                        "restrict qualifier requires a pointer type",
                        span,
                    ));
                };
                if matches!(pointee.unqualified(), CType::Function(..)) {
                    return Err(Diagnostic::error(
                        "restrict-qualified pointers must point to object or incomplete types",
                        span,
                    ));
                }
                self.validate_restrict_usage(inner, span)
            }
            CType::Pointer(inner) | CType::Array(inner, _) => {
                self.validate_restrict_usage(inner, span)
            }
            CType::Function(ret, params, _) => {
                self.validate_restrict_usage(ret, span)?;
                for param in params {
                    self.validate_restrict_usage(param, span)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn adjust_parameter_type(&self, ty: CType) -> CType {
        match ty.unqualified() {
            CType::Array(inner, _) => CType::pointer_to((**inner).clone()),
            CType::Function(_, _, _) => CType::pointer_to(ty),
            _ => ty,
        }
    }

    fn current_scope_mut(&mut self) -> &mut HashMap<String, ScopeEntry> {
        if let Some(scope) = self.block_scopes.last_mut() {
            scope
        } else {
            &mut self.file_scope
        }
    }

    fn current_scope_entry(&self, name: &str) -> Option<&ScopeEntry> {
        if let Some(scope) = self.block_scopes.last() {
            scope.get(name)
        } else {
            self.file_scope.get(name)
        }
    }

    fn current_tag_scope_mut(&mut self) -> &mut HashMap<String, TagBinding> {
        if let Some(scope) = self.block_tag_scopes.last_mut() {
            scope
        } else {
            &mut self.file_tags
        }
    }

    fn current_tag_binding(&self, name: &str) -> Option<TagBinding> {
        if let Some(scope) = self.block_tag_scopes.last() {
            scope.get(name).copied()
        } else {
            self.file_tags.get(name).copied()
        }
    }

    fn visible_tag_binding(&self, name: &str) -> Option<TagBinding> {
        self.block_tag_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.file_tags.get(name))
            .copied()
    }

    fn declare_tag(&mut self, name: &str, kind: TagKind, id: usize) {
        self.current_tag_scope_mut()
            .insert(name.to_owned(), TagBinding { kind, id });
    }

    fn resolve_tag(
        &mut self,
        name: &str,
        kind: TagKind,
        current_scope_only: bool,
        span: Span,
    ) -> Result<(usize, bool), Diagnostic> {
        let existing = if current_scope_only {
            self.current_tag_binding(name)
        } else {
            self.visible_tag_binding(name)
        };
        if let Some(existing) = existing {
            if existing.kind != kind {
                return Err(Diagnostic::error(
                    format!("tag {} was previously declared with a different kind", name),
                    span,
                ));
            }
            return Ok((existing.id, false));
        }
        let id = self.alloc_tag_id();
        self.declare_tag(name, kind, id);
        Ok((id, true))
    }

    fn visible_scope_entry(&self, name: &str) -> Option<&ScopeEntry> {
        self.block_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.file_scope.get(name))
    }

    fn outer_scope_entry(&self, name: &str) -> Option<&ScopeEntry> {
        self.block_scopes
            .iter()
            .rev()
            .skip(1)
            .find_map(|scope| scope.get(name))
            .or_else(|| self.file_scope.get(name))
    }

    fn lookup_typedef_name(&self, name: &str) -> Option<CType> {
        self.visible_scope_entry(name)
            .and_then(|entry| entry.typedef_ty.clone())
    }

    fn lookup_ordinary_symbol(&self, name: &str) -> Option<SymbolKind> {
        self.visible_scope_entry(name)
            .and_then(|entry| entry.ordinary)
    }

    fn lookup_enum_constant_value(&self, name: &str) -> Option<i128> {
        self.visible_scope_entry(name)
            .and_then(|entry| entry.enum_constant)
    }

    fn declare_object_symbol(
        &mut self,
        name: &str,
        ty: &CType,
        storage_class: Option<ParsedStorageClass>,
        span: Span,
        context: DeclContext,
    ) -> Result<Option<Linkage>, Diagnostic> {
        let linkage = self.object_linkage(name, storage_class, context);
        let existing = self.current_scope_entry(name).cloned();
        if let Some(existing) = existing {
            if existing.typedef_ty.is_some() || existing.enum_constant.is_some() {
                return Err(Diagnostic::error(
                    format!("{} is already declared as a different kind of symbol", name),
                    span,
                ));
            }
            if let Some(kind) = existing.ordinary {
                if kind != SymbolKind::Object {
                    return Err(Diagnostic::error(
                        format!("{} is already declared as a different kind of symbol", name),
                        span,
                    ));
                }
                let repeated_block_extern = context == DeclContext::BlockScope
                    && existing.ordinary_storage_class == Some(ParsedStorageClass::Extern)
                    && storage_class == Some(ParsedStorageClass::Extern);
                if context != DeclContext::FileScope && !repeated_block_extern {
                    return Err(Diagnostic::error(format!("redefinition of {}", name), span));
                }
                if existing.ordinary_linkage != linkage {
                    return Err(mixed_linkage_diag(name, span));
                }
                return Ok(linkage);
            }
        }
        if context == DeclContext::BlockScope
            && storage_class == Some(ParsedStorageClass::Extern)
            && let Some(outer) = self.outer_scope_entry(name)
            && outer.ordinary_linkage.is_some()
        {
            if outer.ordinary != Some(SymbolKind::Object) {
                return Err(Diagnostic::error(
                    format!("{} is already declared as a different kind of symbol", name),
                    span,
                ));
            }
        }
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = Some(SymbolKind::Object);
        entry.ordinary_ty = Some(ty.clone());
        entry.ordinary_storage_class = storage_class;
        entry.ordinary_linkage = linkage;
        entry.typedef_ty = None;
        entry.enum_constant = None;
        Ok(linkage)
    }

    fn object_linkage(
        &self,
        name: &str,
        storage_class: Option<ParsedStorageClass>,
        context: DeclContext,
    ) -> Option<Linkage> {
        match (context, storage_class) {
            (DeclContext::FileScope, Some(ParsedStorageClass::Static)) => Some(Linkage::Internal),
            (DeclContext::FileScope, Some(ParsedStorageClass::Extern))
            | (DeclContext::BlockScope, Some(ParsedStorageClass::Extern)) => self
                .visible_scope_entry(name)
                .and_then(|entry| entry.ordinary_linkage)
                .or(Some(Linkage::External)),
            (DeclContext::FileScope, _) => Some(Linkage::External),
            _ => None,
        }
    }

    fn declare_function_symbol(
        &mut self,
        name: &str,
        ty: &CType,
        storage_class: Option<ParsedStorageClass>,
        span: Span,
        context: DeclContext,
    ) -> Result<Linkage, Diagnostic> {
        let linkage = if storage_class == Some(ParsedStorageClass::Static) {
            Linkage::Internal
        } else {
            self.visible_scope_entry(name)
                .and_then(|entry| entry.ordinary_linkage)
                .unwrap_or(Linkage::External)
        };
        let existing = self.current_scope_entry(name).cloned();
        if let Some(existing) = existing {
            if existing.typedef_ty.is_some()
                || existing.enum_constant.is_some()
                || existing
                    .ordinary
                    .is_some_and(|kind| kind != SymbolKind::Function)
            {
                return Err(Diagnostic::error(
                    format!("{} is already declared as a different kind of symbol", name),
                    span,
                ));
            }
            if existing.ordinary == Some(SymbolKind::Function) {
                if existing.ordinary_linkage != Some(linkage) {
                    return Err(mixed_linkage_diag(name, span));
                }
                return Ok(linkage);
            }
        }
        if context == DeclContext::BlockScope
            && let Some(outer) = self.outer_scope_entry(name)
            && outer.ordinary_linkage.is_some()
        {
            if outer.ordinary != Some(SymbolKind::Function) {
                return Err(Diagnostic::error(
                    format!("{} is already declared as a different kind of symbol", name),
                    span,
                ));
            }
        }
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = Some(SymbolKind::Function);
        entry.ordinary_ty = Some(ty.clone());
        entry.ordinary_storage_class = storage_class;
        entry.ordinary_linkage = Some(linkage);
        entry.typedef_ty = None;
        entry.enum_constant = None;
        Ok(linkage)
    }

    fn declare_typedef_name(
        &mut self,
        name: &str,
        ty: CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let existing = self.current_scope_entry(name).cloned();
        if let Some(existing) = existing {
            if let Some(existing_ty) = existing.typedef_ty {
                if declaration_types_compatible(&existing_ty, &ty) {
                    return Ok(());
                }
                return Err(Diagnostic::error(
                    format!("conflicting types for typedef {}", name),
                    span,
                ));
            }
            if existing.ordinary.is_some() || existing.enum_constant.is_some() {
                return Err(Diagnostic::error(
                    format!("{} is already declared as a different kind of symbol", name),
                    span,
                ));
            }
        }
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = None;
        entry.ordinary_ty = None;
        entry.ordinary_storage_class = None;
        entry.ordinary_linkage = None;
        entry.enum_constant = None;
        entry.typedef_ty = Some(ty);
        Ok(())
    }

    fn declare_enum_constant(
        &mut self,
        name: &str,
        value: i128,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        if entry.ordinary.is_some() || entry.typedef_ty.is_some() || entry.enum_constant.is_some() {
            return Err(Diagnostic::error(
                format!("redefinition of enumerator {}", name),
                span,
            ));
        }
        entry.ordinary = Some(SymbolKind::EnumConstant);
        entry.ordinary_ty = None;
        entry.ordinary_storage_class = None;
        entry.ordinary_linkage = None;
        entry.enum_constant = Some(value);
        Ok(())
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek_kind(0) == Some(&kind)
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.peek_kind(0), Some(TokenKind::Keyword(current)) if *current == keyword)
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token, Diagnostic> {
        if self.at(kind.clone()) {
            let token = &self.tokens[self.index];
            self.index += 1;
            Ok(token)
        } else {
            let token = &self.tokens[self.index];
            if kind == TokenKind::Semicolon
                && let Some(previous) = self.tokens.get(self.index.saturating_sub(1))
            {
                let (previous_path, _, _) = self.sources.span_location(previous.span);
                let (current_path, _, _) = self.sources.span_location(token.span);
                if previous_path != current_path {
                    return Err(Diagnostic::error(
                        "expected ';' before continuing in another source file",
                        previous.span,
                    ));
                }
            }
            Err(Diagnostic::error(
                format!(
                    "expected {}, found {}",
                    kind.diagnostic_name(),
                    token.kind.diagnostic_name()
                ),
                token.span,
            ))
        }
    }

    fn bump(&mut self) -> &Token {
        let token = &self.tokens[self.index];
        self.index += 1;
        token
    }

    fn peek_kind(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens
            .get(self.index + offset)
            .map(|token| &token.kind)
    }

    fn prev_span(&self) -> Span {
        self.tokens[self.index - 1].span
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.index)
            .map(|token| token.span)
            .unwrap_or_else(|| self.tokens.last().unwrap().span)
    }
}

fn declaration_types_compatible(lhs: &CType, rhs: &CType) -> bool {
    match (lhs, rhs) {
        (CType::Qualified(lhs, lhs_qualifiers), CType::Qualified(rhs, rhs_qualifiers)) => {
            lhs_qualifiers == rhs_qualifiers && declaration_types_compatible(lhs, rhs)
        }
        (CType::Complex(lhs), CType::Complex(rhs)) | (CType::Pointer(lhs), CType::Pointer(rhs)) => {
            declaration_types_compatible(lhs, rhs)
        }
        (CType::Array(lhs, lhs_len), CType::Array(rhs, rhs_len)) => {
            (*lhs_len == 0 || *rhs_len == 0 || lhs_len == rhs_len)
                && declaration_types_compatible(lhs, rhs)
        }
        (
            CType::Function(lhs_return, lhs_params, lhs_variadic),
            CType::Function(rhs_return, rhs_params, rhs_variadic),
        ) => {
            lhs_variadic == rhs_variadic
                && lhs_params.len() == rhs_params.len()
                && declaration_types_compatible(lhs_return, rhs_return)
                && lhs_params.iter().zip(rhs_params).all(|(lhs, rhs)| {
                    declaration_types_compatible(lhs.unqualified(), rhs.unqualified())
                })
        }
        _ => lhs == rhs,
    }
}

fn generic_types_compatible(lhs: &CType, rhs: &CType) -> bool {
    match (lhs, rhs) {
        (CType::Qualified(lhs, lhs_qualifiers), CType::Qualified(rhs, rhs_qualifiers)) => {
            lhs_qualifiers == rhs_qualifiers && generic_types_compatible(lhs, rhs)
        }
        (CType::Qualified(..), _) | (_, CType::Qualified(..)) => false,
        (CType::Complex(lhs), CType::Complex(rhs)) | (CType::Pointer(lhs), CType::Pointer(rhs)) => {
            generic_types_compatible(lhs, rhs)
        }
        (CType::Array(lhs, lhs_len), CType::Array(rhs, rhs_len)) => {
            (*lhs_len == 0 || *rhs_len == 0 || lhs_len == rhs_len)
                && generic_types_compatible(lhs, rhs)
        }
        (
            CType::Function(lhs_return, lhs_params, lhs_variadic),
            CType::Function(rhs_return, rhs_params, rhs_variadic),
        ) => {
            lhs_variadic == rhs_variadic
                && generic_types_compatible(lhs_return, rhs_return)
                && function_parameter_lists_compatible(lhs_params, rhs_params)
        }
        _ => lhs == rhs,
    }
}

fn function_parameter_lists_compatible(lhs: &[CType], rhs: &[CType]) -> bool {
    if lhs.is_empty() {
        return prototype_compatible_with_unspecified_parameters(rhs);
    }
    if rhs.is_empty() {
        return prototype_compatible_with_unspecified_parameters(lhs);
    }
    lhs.len() == rhs.len()
        && lhs
            .iter()
            .zip(rhs)
            .all(|(lhs, rhs)| generic_types_compatible(lhs.unqualified(), rhs.unqualified()))
}

fn prototype_compatible_with_unspecified_parameters(params: &[CType]) -> bool {
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

fn mixed_linkage_diag(name: &str, span: Span) -> Diagnostic {
    Diagnostic::ub(
        format!(
            "identifier {} is declared with both internal and external linkage",
            name
        ),
        span,
        Some("6.2.2"),
    )
}

fn ast_storage_class(storage_class: ParsedStorageClass) -> Option<StorageClass> {
    match storage_class {
        ParsedStorageClass::Auto => Some(StorageClass::Auto),
        ParsedStorageClass::Extern => Some(StorageClass::Extern),
        ParsedStorageClass::Register => Some(StorageClass::Register),
        ParsedStorageClass::Static => Some(StorageClass::Static),
        ParsedStorageClass::Typedef => None,
    }
}

fn set_storage_class(
    slot: &mut Option<ParsedStorageClass>,
    value: ParsedStorageClass,
    span: Span,
) -> Result<(), Diagnostic> {
    if slot.is_some() {
        return Err(Diagnostic::error(
            "multiple storage class specifiers are not allowed",
            span,
        ));
    }
    *slot = Some(value);
    Ok(())
}

fn saw_builtin_type_specifier(
    saw_void: bool,
    saw_bool: bool,
    saw_char: bool,
    saw_float: bool,
    saw_double: bool,
    saw_complex: bool,
    saw_int: bool,
    saw_short: bool,
    long_count: usize,
    saw_signed: bool,
    saw_unsigned: bool,
) -> bool {
    saw_void
        || saw_bool
        || saw_char
        || saw_float
        || saw_double
        || saw_complex
        || saw_int
        || saw_short
        || long_count > 0
        || saw_signed
        || saw_unsigned
}

fn align_up(value: usize, align: usize) -> usize {
    if align == 0 {
        value
    } else {
        ((value + align - 1) / align) * align
    }
}
