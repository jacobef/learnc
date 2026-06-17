use std::collections::{HashMap, HashSet};

use crate::ast::{
    BinaryOp, Block, BlockItem, Declaration, Designator, Expr, ExternalDeclaration, ForInit,
    FunctionDecl, FunctionDef, GenericAssociation, Initializer, InitializerItem, Parameter,
    PostfixOp, Statement, StorageClass, SwitchLabel, TranslationUnit, UnaryOp,
};
use crate::diag::Diagnostic;
use crate::source::{SourceManager, Span};
use crate::token::{Keyword, Token, TokenKind};
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

#[derive(Debug, Clone, Default)]
struct ScopeEntry {
    ordinary: Option<SymbolKind>,
    typedef_ty: Option<CType>,
    enum_constant: Option<i128>,
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
}

#[derive(Debug, Clone)]
struct ParsedTypeName {
    ty: CType,
    vla_bounds: Vec<Option<Expr>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclContext {
    FileScope,
    BlockScope,
    Parameter,
    RecordMember,
    TypeName,
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
        span: Span,
    },
}

#[derive(Debug, Clone)]
struct ParsedArraySpec {
    bound: ParsedArrayBound,
    static_bound: Option<Expr>,
}

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
    _sources: &'a SourceManager,
    tokens: Vec<Token>,
    index: usize,
    record_tags: HashMap<(RecordKind, String), usize>,
    records: HashMap<usize, RecordType>,
    enum_tags: HashMap<String, usize>,
    enums: HashMap<usize, EnumType>,
    enum_constants: HashMap<String, i128>,
    next_tag_id: usize,
    next_member_id: usize,
    file_scope: HashMap<String, ScopeEntry>,
    block_scopes: Vec<HashMap<String, ScopeEntry>>,
    allow_undeclared_identifiers: bool,
}

impl<'a> Parser<'a> {
    pub fn new(sources: &'a SourceManager, tokens: Vec<Token>) -> Self {
        let mut file_scope = HashMap::new();
        file_scope.insert(
            "va_list".to_owned(),
            ScopeEntry {
                ordinary: None,
                typedef_ty: Some(CType::VaList),
                enum_constant: None,
            },
        );
        Self {
            _sources: sources,
            tokens,
            index: 0,
            record_tags: HashMap::new(),
            records: HashMap::new(),
            enum_tags: HashMap::new(),
            enums: HashMap::new(),
            enum_constants: HashMap::new(),
            next_tag_id: 0,
            next_member_id: 0,
            file_scope,
            block_scopes: Vec::new(),
            allow_undeclared_identifiers: false,
        }
    }

    pub fn parse_translation_unit(mut self) -> Result<TranslationUnit, Diagnostic> {
        let mut externals = Vec::new();
        while !self.at(TokenKind::Eof) {
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
        Ok(TranslationUnit {
            externals,
            functions: Vec::new(),
            function_declarations: Vec::new(),
            globals: Vec::new(),
            global_definitions: Vec::new(),
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
        let specs = self.parse_declaration_specifiers(DeclContext::FileScope)?;
        if self.eat(TokenKind::Semicolon) {
            if specs.storage_class.is_some() || specs.is_inline {
                return Err(Diagnostic::error(
                    "storage class specifiers and inline require a declarator",
                    self.prev_span(),
                ));
            }
            return Ok(ExternalDecl::Empty);
        }
        let (name, ty, vla_bounds, _, base_span, function_params) =
            self.parse_declarator(specs.base_type.clone(), DeclContext::FileScope)?;
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
                vla_bounds,
                base_span,
                specs.storage_class,
                specs.is_inline,
                DeclContext::FileScope,
            )?));
        }
        if ty.is_function() && function_params.is_none() {
            self.validate_function_decl_specifiers(&specs, base_span)?;
            let mut decls = Vec::new();
            self.declare_ordinary_symbol(&name, SymbolKind::Function);
            decls.push(self.build_function_declaration(
                name,
                ty,
                base_span,
                specs.storage_class,
                specs.is_inline,
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
                self.declare_ordinary_symbol(&name, SymbolKind::Function);
                decls.push(self.build_function_declaration(
                    name,
                    ty,
                    span,
                    specs.storage_class,
                    specs.is_inline,
                )?);
            }
            let end = self.expect(TokenKind::Semicolon)?.span;
            for decl in &mut decls {
                decl.span = decl.span.merge(end);
            }
            return Ok(ExternalDecl::FunctionDeclarations(decls));
        }
        let Some((params, is_variadic)) = function_params else {
            return Ok(ExternalDecl::Globals(self.finish_declarator_list(
                name,
                ty,
                vla_bounds,
                base_span,
                specs.storage_class,
                specs.is_inline,
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
        self.declare_ordinary_symbol(&name, SymbolKind::Function);
        let return_type = match ty.unqualified() {
            CType::Function(return_type, _, _) => (**return_type).clone(),
            _ => unreachable!("outermost function declarator must produce a function type"),
        };
        if self.eat(TokenKind::Semicolon) {
            return Ok(ExternalDecl::FunctionDeclarations(vec![FunctionDecl {
                name,
                return_type,
                params,
                is_variadic,
                storage_class: specs.storage_class.and_then(ast_storage_class),
                is_inline: specs.is_inline,
                span: base_span,
            }]));
        }
        self.push_block_scope();
        for param in &params {
            if let Some(name) = &param.name {
                self.declare_ordinary_symbol(name, SymbolKind::Object);
            }
        }
        let body = self.parse_block()?;
        self.pop_block_scope();
        self.validate_function_labels(&body)?;
        let span = base_span.merge(body.span);
        Ok(ExternalDecl::Function(FunctionDef {
            name,
            return_type,
            params,
            is_variadic,
            storage_class: specs.storage_class.and_then(ast_storage_class),
            is_inline: specs.is_inline,
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
    ) -> Result<FunctionDecl, Diagnostic> {
        let (return_type, params, is_variadic) = match ty.unqualified() {
            CType::Function(return_type, params, is_variadic) => {
                ((**return_type).clone(), params.clone(), *is_variadic)
            }
            _ => return Err(Diagnostic::error("expected function declarator", span)),
        };
        let params = params
            .into_iter()
            .map(|ty| Parameter {
                name: None,
                ty,
                vla_bounds: Vec::new(),
                static_array_bound: None,
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
            is_inline,
            span,
        })
    }

    fn parse_parameter_list(&mut self) -> Result<(Vec<Parameter>, bool), Diagnostic> {
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
                    is_variadic = true;
                    break;
                }
                let start = self.current_span();
                let specs = self.parse_declaration_specifiers(DeclContext::Parameter)?;
                if self.at(TokenKind::Comma) || self.at(TokenKind::RParen) {
                    self.validate_parameter_decl_specifiers(&specs, start)?;
                    params.push(Parameter {
                        name: None,
                        ty: self.adjust_parameter_type(specs.base_type),
                        vla_bounds: Vec::new(),
                        static_array_bound: None,
                        storage_class: specs.storage_class.and_then(ast_storage_class),
                        span: start,
                    });
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                    continue;
                }
                let (name, ty, vla_bounds, static_array_bound, span, _) =
                    self.parse_declarator(specs.base_type.clone(), DeclContext::Parameter)?;
                self.validate_parameter_decl_specifiers(&specs, span)?;
                self.declare_ordinary_symbol(&name, SymbolKind::Object);
                params.push(Parameter {
                    name: Some(name),
                    ty: self.adjust_parameter_type(ty),
                    vla_bounds,
                    static_array_bound,
                    storage_class: specs.storage_class.and_then(ast_storage_class),
                    span,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            Ok((params, is_variadic))
        })();
        self.pop_block_scope();
        result
    }

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect(TokenKind::LBrace)?.span;
        let mut items = Vec::new();
        while !self.at(TokenKind::RBrace) {
            if self.is_declaration_start() {
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
        let (first_name, first_ty, first_vla_bounds, _, first_span, first_function_params) =
            self.parse_declarator(specs.base_type, DeclContext::BlockScope)?;
        let redecl_base = self.base_type_for_redeclaration(&first_ty);
        let first_init = if self.eat(TokenKind::Equal) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        self.finish_single_block_declarator(
            &mut items,
            first_name,
            first_ty,
            first_vla_bounds,
            first_function_params,
            specs.storage_class,
            specs.is_inline,
            first_init,
            first_span,
        )?;

        while self.eat(TokenKind::Comma) {
            let (name, ty, vla_bounds, _, span, function_params) =
                self.parse_declarator(redecl_base.clone(), DeclContext::BlockScope)?;
            let init = if self.eat(TokenKind::Equal) {
                Some(self.parse_initializer()?)
            } else {
                None
            };
            self.finish_single_block_declarator(
                &mut items,
                name,
                ty,
                vla_bounds,
                function_params,
                specs.storage_class,
                specs.is_inline,
                init,
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
        let (name, ty, vla_bounds, _, span, function_params) =
            self.parse_declarator(specs.base_type, DeclContext::BlockScope)?;
        if function_params.is_some() && specs.storage_class != Some(ParsedStorageClass::Typedef) {
            return Err(Diagnostic::error(
                "function declarations are only supported at file scope",
                span,
            ));
        }
        self.finish_declarator_list(
            name,
            ty,
            vla_bounds,
            span,
            specs.storage_class,
            specs.is_inline,
            DeclContext::BlockScope,
        )
    }

    fn finish_declarator_list(
        &mut self,
        first_name: String,
        first_ty: CType,
        first_vla_bounds: Vec<Option<Expr>>,
        first_span: Span,
        storage_class: Option<ParsedStorageClass>,
        is_inline: bool,
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
        let first_init = if self.eat(TokenKind::Equal) {
            Some(self.parse_initializer()?)
        } else {
            None
        };
        self.finish_single_declarator(
            &mut declarations,
            first_name,
            first_ty.clone(),
            first_vla_bounds,
            storage_class,
            first_init,
            first_span,
            context,
        )?;
        while self.eat(TokenKind::Comma) {
            let (name, ty, vla_bounds, _, span, function_params) =
                self.parse_declarator(self.base_type_for_redeclaration(&first_ty), context)?;
            if function_params.is_some() && storage_class != Some(ParsedStorageClass::Typedef) {
                return Err(Diagnostic::error(
                    "function declarations are only supported at file scope",
                    span,
                ));
            }
            let init = if self.eat(TokenKind::Equal) {
                Some(self.parse_initializer()?)
            } else {
                None
            };
            self.finish_single_declarator(
                &mut declarations,
                name,
                ty,
                vla_bounds,
                storage_class,
                init,
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
        function_params: Option<(Vec<Parameter>, bool)>,
        storage_class: Option<ParsedStorageClass>,
        is_inline: bool,
        init: Option<Initializer>,
        span: Span,
    ) -> Result<(), Diagnostic> {
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
            };
            self.validate_function_decl_specifiers(&specs, span)?;
            self.declare_ordinary_symbol(&name, SymbolKind::Function);
            items.push(BlockItem::FunctionDeclaration(
                self.build_function_declaration(name, ty, span, storage_class, is_inline)?,
            ));
            return Ok(());
        }
        let mut declarations = Vec::new();
        self.finish_single_declarator(
            &mut declarations,
            name,
            ty,
            vla_bounds,
            storage_class,
            init,
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
        init: Option<Initializer>,
        span: Span,
        context: DeclContext,
    ) -> Result<(), Diagnostic> {
        self.validate_restrict_usage(&ty, span)?;
        let has_variably_modified_type = vla_bounds.iter().any(|bound| bound.is_some());
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
        }
        if storage_class == Some(ParsedStorageClass::Typedef) {
            if init.is_some() {
                return Err(Diagnostic::error(
                    "typedef declaration cannot have an initializer",
                    span,
                ));
            }
            self.declare_typedef_name(&name, ty);
            return Ok(());
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
        self.declare_ordinary_symbol(&name, SymbolKind::Object);
        declarations.push(Declaration {
            name,
            ty,
            vla_bounds,
            storage_class: storage_class.and_then(ast_storage_class),
            init,
            span,
        });
        Ok(())
    }

    fn parse_initializer(&mut self) -> Result<Initializer, Diagnostic> {
        if self.eat(TokenKind::LBrace) {
            let start = self.prev_span();
            let mut items = Vec::new();
            if !self.at(TokenKind::RBrace) {
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
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                    if self.at(TokenKind::RBrace) {
                        break;
                    }
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
                    expr.span().merge(end),
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
            let body = match self.parse_statement()? {
                Statement::Block(block) => block,
                other => {
                    return Err(Diagnostic::error(
                        "switch body is currently required to be a compound statement",
                        other.span(),
                    ));
                }
            };
            let span = start.merge(body.span);
            return Ok(Statement::Switch { expr, body, span });
        }
        if self.at_keyword(Keyword::If) {
            let start = self.bump().span;
            self.expect(TokenKind::LParen)?;
            let condition = self.parse_expression()?;
            self.expect(TokenKind::RParen)?;
            let then_branch = Box::new(self.parse_statement()?);
            let else_branch = if self.at_keyword(Keyword::Else) {
                self.bump();
                Some(Box::new(self.parse_statement()?))
            } else {
                None
            };
            let span = else_branch
                .as_ref()
                .map(|else_branch| start.merge(else_branch.span()))
                .unwrap_or_else(|| start.merge(then_branch.span()));
            return Ok(Statement::If {
                condition,
                then_branch,
                else_branch,
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
        if self.eat(TokenKind::DoublePlus) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::PreIncrement,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::DoubleMinus) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::PreDecrement,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::Amp) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::AddressOf,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::Star) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Dereference,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::Plus) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::Minus) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::Bang) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::LogicalNot,
                expr: Box::new(expr),
                span,
            });
        }
        if self.eat(TokenKind::Tilde) {
            let expr = self.parse_unary()?;
            let span = self.prev_span().merge(expr.span());
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_postfix()
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
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
                continue;
            }
            if self.eat(TokenKind::LBracket) {
                let index = self.parse_expression()?;
                let end = self.expect(TokenKind::RBracket)?.span;
                let base_span = expr.span();
                let add = Expr::Binary {
                    op: BinaryOp::Add,
                    lhs: Box::new(expr),
                    rhs: Box::new(index),
                    span: base_span.merge(end),
                };
                let span = add.span().merge(end);
                expr = Expr::Unary {
                    op: UnaryOp::Dereference,
                    expr: Box::new(add),
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
            TokenKind::Number(text) => Ok(Expr::Number(text, token.span)),
            TokenKind::CharLiteral(value) => Ok(Expr::CharLiteral(value, token.span)),
            TokenKind::WideCharLiteral(value) => Ok(Expr::WideCharLiteral(value, token.span)),
            TokenKind::StringLiteral(text) => {
                self.parse_string_literal_primary(text, false, token.span)
            }
            TokenKind::WideStringLiteral(text) => {
                self.parse_string_literal_primary(text, true, token.span)
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
                    return Ok(Expr::Number(value.to_string(), token.span));
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
                let expr = self.parse_expression()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            _ => Err(Diagnostic::error("expected expression", token.span)),
        }
    }

    fn parse_string_literal_primary(
        &mut self,
        text: String,
        is_wide: bool,
        span: Span,
    ) -> Result<Expr, Diagnostic> {
        let mut combined = text;
        let mut combined_span = span;
        let mut wide = is_wide;
        loop {
            match self.peek_kind(0).cloned() {
                Some(TokenKind::StringLiteral(text)) => {
                    let next_span = self.tokens[self.index].span;
                    self.bump();
                    combined.push_str(&text);
                    combined_span = combined_span.merge(next_span);
                }
                Some(TokenKind::WideStringLiteral(text)) => {
                    let next_span = self.tokens[self.index].span;
                    self.bump();
                    combined.push_str(&text);
                    combined_span = combined_span.merge(next_span);
                    wide = true;
                }
                _ => break,
            }
        }
        Ok(if wide {
            Expr::WideStringLiteral(combined, combined_span)
        } else {
            Expr::StringLiteral(combined, combined_span)
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
                    set_storage_class(&mut storage_class, ParsedStorageClass::Auto, start)?;
                }
                Some(TokenKind::Keyword(Keyword::Extern)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(&mut storage_class, ParsedStorageClass::Extern, start)?;
                }
                Some(TokenKind::Keyword(Keyword::Register)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(&mut storage_class, ParsedStorageClass::Register, start)?;
                }
                Some(TokenKind::Keyword(Keyword::Static)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(&mut storage_class, ParsedStorageClass::Static, start)?;
                }
                Some(TokenKind::Keyword(Keyword::Typedef)) => {
                    self.bump();
                    saw_any = true;
                    set_storage_class(&mut storage_class, ParsedStorageClass::Typedef, start)?;
                }
                Some(TokenKind::Keyword(Keyword::Inline)) => {
                    self.bump();
                    saw_any = true;
                    is_inline = true;
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
                            start,
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
                            start,
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
                            start,
                        ));
                    }
                    direct_type = Some(self.parse_enum_specifier()?);
                }
                Some(TokenKind::Keyword(Keyword::Void)) => {
                    self.bump();
                    saw_any = true;
                    saw_void = true;
                }
                Some(TokenKind::Keyword(Keyword::Bool)) => {
                    self.bump();
                    saw_any = true;
                    saw_bool = true;
                }
                Some(TokenKind::Keyword(Keyword::Char)) => {
                    self.bump();
                    saw_any = true;
                    saw_char = true;
                }
                Some(TokenKind::Keyword(Keyword::Float)) => {
                    self.bump();
                    saw_any = true;
                    saw_float = true;
                }
                Some(TokenKind::Keyword(Keyword::Complex)) => {
                    self.bump();
                    saw_any = true;
                    saw_complex = true;
                }
                Some(TokenKind::Keyword(Keyword::Double)) => {
                    self.bump();
                    saw_any = true;
                    saw_double = true;
                }
                Some(TokenKind::Keyword(Keyword::Int)) => {
                    self.bump();
                    saw_any = true;
                    saw_int = true;
                }
                Some(TokenKind::Keyword(Keyword::Short)) => {
                    self.bump();
                    saw_any = true;
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
                    saw_signed = true;
                }
                Some(TokenKind::Keyword(Keyword::Unsigned)) => {
                    self.bump();
                    saw_any = true;
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
                    start,
                )?,
                qualifiers,
            )
        };
        Ok(DeclarationSpecifiers {
            base_type,
            storage_class,
            is_inline,
        })
    }

    fn parse_type_name(&mut self) -> Result<ParsedTypeName, Diagnostic> {
        self.skip_gnu_attributes()?;
        let specs = self.parse_declaration_specifiers(DeclContext::TypeName)?;
        let (ty, vla_bounds) = if self.at(TokenKind::LParen) {
            let declarator = self.parse_abstract_declarator_tree(DeclContext::TypeName)?;
            let (_, ty, vla_bounds, _, _, _) =
                self.apply_parsed_declarator(declarator, specs.base_type, true)?;
            (ty, vla_bounds)
        } else {
            self.parse_type_suffix(specs.base_type)?
        };
        self.validate_restrict_usage(&ty, self.current_span())?;
        Ok(ParsedTypeName { ty, vla_bounds })
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
            return Ok(CType::complex_of(CType::Double));
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
    }

    fn pop_block_scope(&mut self) {
        let _ = self.block_scopes.pop();
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
            Option<(Vec<Parameter>, bool)>,
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
        self.skip_gnu_attributes()?;
        let mut pointer_qualifiers = Vec::new();
        while self.eat(TokenKind::Star) {
            pointer_qualifiers.push(self.parse_type_qualifiers());
            self.skip_gnu_attributes()?;
        }

        let mut declarator = if self.eat(TokenKind::LParen) {
            let inner = self.parse_declarator_tree(context)?;
            self.expect(TokenKind::RParen)?;
            inner
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
                let (params, is_variadic) = self.parse_parameter_list()?;
                let end = self.expect(TokenKind::RParen)?.span;
                let span = declarator.span().merge(end);
                declarator = ParsedDeclarator::Function {
                    inner: Box::new(declarator),
                    params,
                    is_variadic,
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
                let (params, is_variadic) = self.parse_parameter_list()?;
                let end = self.expect(TokenKind::RParen)?.span;
                let span = declarator.span().merge(end);
                declarator = ParsedDeclarator::Function {
                    inner: Box::new(declarator),
                    params,
                    is_variadic,
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
            Option<(Vec<Parameter>, bool)>,
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
                let array_ty = match &spec.bound {
                    ParsedArrayBound::Fixed(len) => CType::array_of(base, *len),
                    ParsedArrayBound::Unspecified | ParsedArrayBound::Variable(_) => {
                        CType::array_of(base, 0)
                    }
                };
                let (name, ty, mut vla_bounds, static_array_bound, inner_span, function_params) =
                    self.apply_parsed_declarator(*inner, array_ty, false)?;
                if let ParsedArrayBound::Variable(expr) = spec.bound {
                    vla_bounds.push(Some(expr));
                } else if matches!(spec.bound, ParsedArrayBound::Unspecified) {
                    vla_bounds.push(None);
                }
                Ok((
                    name,
                    ty,
                    vla_bounds,
                    static_array_bound.or(spec.static_bound),
                    span.merge(inner_span),
                    function_params,
                ))
            }
            ParsedDeclarator::Function {
                inner,
                params,
                is_variadic,
                span,
            } => {
                let function_ty = if is_variadic {
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
                        Some((params, is_variadic))
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
                if value < 0 {
                    return Err(Diagnostic::error(
                        "array bound must be non-negative",
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
        let id = match &tag {
            Some(name) => {
                if let Some(existing) = self.record_tags.get(&(kind, name.clone())) {
                    *existing
                } else {
                    let id = self.alloc_tag_id();
                    self.record_tags.insert((kind, name.clone()), id);
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
                    id
                }
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
                return Err(Diagnostic::error(
                    "structs and unions must declare at least one member",
                    start,
                ));
            }
            if let Some((index, _)) = members
                .iter()
                .enumerate()
                .find(|(_, member)| matches!(member.ty.unqualified(), CType::Array(_, 0)))
            {
                if kind != RecordKind::Struct {
                    return Err(Diagnostic::error(
                        "flexible array members are only allowed in structures",
                        start,
                    ));
                }
                if index + 1 != members.len() {
                    return Err(Diagnostic::error(
                        "flexible array member must be the last member of a structure",
                        start,
                    ));
                }
                if index == 0 {
                    return Err(Diagnostic::error(
                        "structure with a flexible array member must have at least one other member",
                        start,
                    ));
                }
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
            return Err(Diagnostic::error(
                "record type specifier requires a tag or a definition",
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
            self.skip_gnu_attributes()?;
            let specs = self.parse_declaration_specifiers(DeclContext::RecordMember)?;
            let base = specs.base_type;
            let mut parsed_members = Vec::new();
            if self.at(TokenKind::Colon) {
                let width = self.parse_bit_field_width()?;
                parsed_members.push(RecordMember {
                    name: None,
                    storage_name: self.alloc_member_storage_name("__bitfield"),
                    ty: base,
                    offset: 0,
                    bit_width: Some(width),
                    bit_offset: 0,
                    bit_storage_size: 0,
                });
            } else if self.at(TokenKind::Semicolon) {
                self.validate_anonymous_record_member_type(&base, self.current_span())?;
                parsed_members.push(RecordMember {
                    name: None,
                    storage_name: self.alloc_member_storage_name("__anon"),
                    ty: base,
                    offset: 0,
                    bit_width: None,
                    bit_offset: 0,
                    bit_storage_size: 0,
                });
            } else {
                let (first_name, first_ty, first_vla_bounds, _, first_span, function_params) =
                    self.parse_declarator(base, DeclContext::RecordMember)?;
                if function_params.is_some() || first_ty.is_function() {
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
                let bit_width = if self.at(TokenKind::Colon) {
                    Some(self.parse_bit_field_width()?)
                } else {
                    None
                };
                parsed_members.push(RecordMember {
                    name: Some(first_name.clone()),
                    storage_name: first_name,
                    ty: first_ty.clone(),
                    offset: 0,
                    bit_width,
                    bit_offset: 0,
                    bit_storage_size: 0,
                });
                while self.eat(TokenKind::Comma) {
                    let (name, ty, vla_bounds, _, span, function_params) = self.parse_declarator(
                        self.base_type_for_redeclaration(&first_ty),
                        DeclContext::RecordMember,
                    )?;
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
                    let bit_width = if self.at(TokenKind::Colon) {
                        Some(self.parse_bit_field_width()?)
                    } else {
                        None
                    };
                    parsed_members.push(RecordMember {
                        name: Some(name.clone()),
                        storage_name: name,
                        ty,
                        offset: 0,
                        bit_width,
                        bit_offset: 0,
                        bit_storage_size: 0,
                    });
                }
            }
            for member in parsed_members {
                self.validate_record_member_type(&member, self.current_span())?;
                for visible_name in self.visible_member_names_for(&member)? {
                    if !visible_names.insert(visible_name.clone()) {
                        return Err(Diagnostic::error(
                            format!("duplicate member declaration {}", visible_name),
                            self.current_span(),
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
                    span,
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
                "record members must have complete type",
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

    fn parse_bit_field_width(&mut self) -> Result<u8, Diagnostic> {
        self.expect(TokenKind::Colon)?;
        let expr = self.parse_assignment()?;
        let value = self.eval_integer_constant_expr(&expr)?;
        if value < 0 {
            return Err(Diagnostic::error(
                "bit-field width must be non-negative",
                expr.span(),
            ));
        }
        u8::try_from(value).map_err(|_| {
            Diagnostic::error("bit-field width is out of supported range", expr.span())
        })
    }

    fn validate_anonymous_record_member_type(
        &self,
        ty: &CType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match ty.unqualified() {
            CType::Struct(_, _) | CType::Union(_, _) if self.type_is_complete(ty) => Ok(()),
            CType::Struct(_, _) | CType::Union(_, _) => Err(Diagnostic::error(
                "anonymous structure or union member must have complete type",
                span,
            )),
            _ => Err(Diagnostic::error(
                "declaration without a declarator is only allowed for anonymous structure or union members",
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
        let id = match &tag {
            Some(name) => {
                if let Some(existing) = self.enum_tags.get(name) {
                    *existing
                } else {
                    let id = self.alloc_tag_id();
                    self.enum_tags.insert(name.clone(), id);
                    self.enums.insert(
                        id,
                        EnumType {
                            id,
                            tag: Some(name.clone()),
                            complete: false,
                        },
                    );
                    id
                }
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
                if self.enum_constants.contains_key(&name) {
                    return Err(Diagnostic::error(
                        format!("redefinition of enumerator {}", name),
                        token.span,
                    ));
                }
                let value = if self.eat(TokenKind::Equal) {
                    let expr = self.parse_assignment()?;
                    self.eval_integer_constant_expr(&expr)?
                } else {
                    next_value
                };
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
        match expr {
            Expr::Number(text, span) => self.parse_integer_constant_text(text, *span),
            Expr::CharLiteral(value, _) => Ok(*value as i128),
            Expr::Variable(name, span) => self.lookup_enum_constant_value(name).ok_or_else(|| {
                Diagnostic::error(
                    format!("identifier {} is not an integer constant expression", name),
                    *span,
                )
            }),
            Expr::Unary { op, expr, span } => {
                let value = self.eval_integer_constant_expr(expr)?;
                match op {
                    UnaryOp::Plus => Ok(value),
                    UnaryOp::Minus => value
                        .checked_neg()
                        .ok_or_else(|| Diagnostic::error("constant expression overflow", *span)),
                    UnaryOp::LogicalNot => Ok((value == 0) as i128),
                    UnaryOp::BitNot => Ok(!value),
                    _ => Err(Diagnostic::error(
                        "unsupported operator in integer constant expression",
                        *span,
                    )),
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lhs = self.eval_integer_constant_expr(lhs)?;
                let rhs = self.eval_integer_constant_expr(rhs)?;
                match op {
                    BinaryOp::Add => lhs
                        .checked_add(rhs)
                        .ok_or_else(|| Diagnostic::error("constant expression overflow", *span)),
                    BinaryOp::Sub => lhs
                        .checked_sub(rhs)
                        .ok_or_else(|| Diagnostic::error("constant expression overflow", *span)),
                    BinaryOp::Mul => lhs
                        .checked_mul(rhs)
                        .ok_or_else(|| Diagnostic::error("constant expression overflow", *span)),
                    BinaryOp::Div => {
                        if rhs == 0 {
                            Err(Diagnostic::error(
                                "division by zero in constant expression",
                                *span,
                            ))
                        } else {
                            lhs.checked_div(rhs).ok_or_else(|| {
                                Diagnostic::error("constant expression overflow", *span)
                            })
                        }
                    }
                    BinaryOp::Rem => {
                        if rhs == 0 {
                            Err(Diagnostic::error(
                                "division by zero in constant expression",
                                *span,
                            ))
                        } else {
                            lhs.checked_rem(rhs).ok_or_else(|| {
                                Diagnostic::error("constant expression overflow", *span)
                            })
                        }
                    }
                    BinaryOp::ShiftLeft => Ok(lhs << rhs),
                    BinaryOp::ShiftRight => Ok(lhs >> rhs),
                    BinaryOp::BitAnd => Ok(lhs & rhs),
                    BinaryOp::BitXor => Ok(lhs ^ rhs),
                    BinaryOp::BitOr => Ok(lhs | rhs),
                    BinaryOp::LogicalAnd => Ok(((lhs != 0) && (rhs != 0)) as i128),
                    BinaryOp::LogicalOr => Ok(((lhs != 0) || (rhs != 0)) as i128),
                    BinaryOp::Equal => Ok((lhs == rhs) as i128),
                    BinaryOp::NotEqual => Ok((lhs != rhs) as i128),
                    BinaryOp::Less => Ok((lhs < rhs) as i128),
                    BinaryOp::LessEqual => Ok((lhs <= rhs) as i128),
                    BinaryOp::Greater => Ok((lhs > rhs) as i128),
                    BinaryOp::GreaterEqual => Ok((lhs >= rhs) as i128),
                    BinaryOp::Comma => Ok(rhs),
                }
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                if self.eval_integer_constant_expr(condition)? != 0 {
                    self.eval_integer_constant_expr(then_expr)
                } else {
                    self.eval_integer_constant_expr(else_expr)
                }
            }
            Expr::Cast { expr, .. } => self.eval_integer_constant_expr(expr),
            Expr::OffsetOf {
                ty,
                designators,
                span,
            } => self.eval_offsetof_constant_expr(ty, designators, *span),
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

    fn parse_integer_constant_text(&self, text: &str, span: Span) -> Result<i128, Diagnostic> {
        let trimmed = text.trim_end_matches(|ch: char| ch.is_ascii_alphabetic());
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            i128::from_str_radix(hex, 16)
                .map_err(|_| Diagnostic::error("invalid hexadecimal constant", span))
        } else if trimmed.starts_with('0') && trimmed.len() > 1 {
            i128::from_str_radix(&trimmed[1..], 8)
                .map_err(|_| Diagnostic::error("invalid octal constant", span))
        } else {
            trimmed
                .parse::<i128>()
                .map_err(|_| Diagnostic::error("invalid integer constant", span))
        }
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

    fn type_size_of(&self, ty: &CType) -> Result<usize, Diagnostic> {
        match ty.unqualified() {
            CType::Struct(id, _) | CType::Union(id, _) => self
                .records
                .get(id)
                .filter(|record| record.complete)
                .map(|record| record.size)
                .ok_or_else(|| Diagnostic::error("type is incomplete", self.current_span())),
            _ => ty
                .size_of()
                .ok_or_else(|| Diagnostic::error("type is incomplete", self.current_span())),
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
        self.starts_declaration_specifier_at(0)
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
                if !matches!(inner.unqualified(), CType::Pointer(_)) {
                    return Err(Diagnostic::error(
                        "restrict qualifier requires a pointer type",
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

    fn visible_scope_entry(&self, name: &str) -> Option<&ScopeEntry> {
        self.block_scopes
            .iter()
            .rev()
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

    fn declare_ordinary_symbol(&mut self, name: &str, kind: SymbolKind) {
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = Some(kind);
        entry.typedef_ty = None;
        if kind != SymbolKind::EnumConstant {
            entry.enum_constant = None;
        }
    }

    fn declare_typedef_name(&mut self, name: &str, ty: CType) {
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = None;
        entry.enum_constant = None;
        entry.typedef_ty = Some(ty);
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
            Err(Diagnostic::error(
                format!("expected {:?}", kind),
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
    if let Some(existing) = *slot {
        if existing != value {
            return Err(Diagnostic::error(
                "multiple storage class specifiers are not allowed",
                span,
            ));
        }
    } else {
        *slot = Some(value);
    }
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

trait StatementSpan {
    fn span(&self) -> Span;
}

impl StatementSpan for Statement {
    fn span(&self) -> Span {
        match self {
            Statement::Block(block) => block.span,
            Statement::Break(span)
            | Statement::Continue(span)
            | Statement::DoWhile { span, .. }
            | Statement::Expression(_, span)
            | Statement::For { span, .. }
            | Statement::Goto { span, .. }
            | Statement::Labeled { span, .. }
            | Statement::Return(_, span)
            | Statement::Switch { span, .. }
            | Statement::UserLabeled { span, .. }
            | Statement::If { span, .. }
            | Statement::While { span, .. } => *span,
        }
    }
}
