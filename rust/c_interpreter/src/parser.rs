mod constant_eval;
mod declarations;
mod declarators;
mod expressions;
mod records;
mod statements;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::ast::{
    BinaryOp, Block, BlockItem, Declaration, Designator, Expr, ExternalDeclaration, ForInit,
    FunctionDecl, FunctionDef, GenericAssociation, Initializer, InitializerItem, Linkage,
    Parameter, PostfixOp, Statement, StorageClass, SwitchLabel, TranslationUnit, UnaryOp,
};
use crate::diag::Diagnostic;
use crate::fast_hash::FastHashMap;
use crate::number::{NumberValue, parse_number_literal};
use crate::source::{FileId, SourceManager, Span};
use crate::token::{Keyword, StringLiteralValue, Token, TokenKind};
use crate::types::{
    CType, EnumType, HOST_LONG_DOUBLE_ALIGN, RecordKind, RecordMember, RecordType, TypeQualifiers,
};

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
    typedef_vla_bounds: Vec<Option<Expr>>,
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
    vla_bounds: Vec<Option<Expr>>,
    storage_class: Option<ParsedStorageClass>,
    is_inline: bool,
    is_noreturn: bool,
    alignment: Option<usize>,
    declares_tag_or_enumerators: bool,
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
    Utf8,
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
        info: ParsedFunctionInfo,
        span: Span,
    },
}

#[derive(Debug, Clone)]
struct ParsedArraySpec {
    bound: ParsedArrayBound,
    static_bound: Option<Expr>,
    qualifiers: TypeQualifiers,
    prototype_vla_star: bool,
}

#[derive(Debug)]
struct ResolvedDeclarator {
    name: String,
    ty: CType,
    vla_bounds: Vec<Option<Expr>>,
    static_array_bound: Option<Expr>,
    span: Span,
    function_params: Option<ParsedFunctionInfo>,
}

#[derive(Debug, Clone, Default)]
struct ParsedFunctionInfo {
    params: Vec<Parameter>,
    is_variadic: bool,
    parameter_tags: HashMap<String, TagBinding>,
    parameter_enum_constants: HashMap<String, i128>,
    old_style: bool,
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
    sources: &'a SourceManager,
    tokens: Vec<Token>,
    index: usize,
    file_tags: HashMap<String, TagBinding>,
    block_tag_scopes: Vec<HashMap<String, TagBinding>>,
    records: FastHashMap<usize, RecordType>,
    enums: FastHashMap<usize, EnumType>,
    enum_constants: FastHashMap<(FileId, String), i128>,
    next_tag_id: usize,
    next_member_id: usize,
    next_hidden_vla_bound_id: usize,
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
                typedef_vla_bounds: Vec::new(),
                enum_constant: None,
            },
        );
        Self {
            sources,
            tokens,
            index: 0,
            file_tags: HashMap::new(),
            block_tag_scopes: Vec::new(),
            records: FastHashMap::default(),
            enums: FastHashMap::default(),
            enum_constants: FastHashMap::default(),
            next_tag_id: 0,
            next_member_id: 0,
            next_hidden_vla_bound_id: 0,
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
            self.parse_external_declaration(&mut externals)?;
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
                && let Some(mut tail) = self.resolve_visible_member_chain(&candidate.ty, member)
            {
                let mut chain = vec![candidate.clone()];
                chain.append(&mut tail);
                return Some(chain);
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
        loop {
            if self.peek_kind(offset) == Some(&TokenKind::Keyword(Keyword::Atomic))
                && self.peek_kind(offset + 1) == Some(&TokenKind::LParen)
            {
                break;
            }
            if matches!(
                self.peek_kind(offset),
                Some(TokenKind::Keyword(
                    Keyword::Auto
                        | Keyword::Atomic
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
                continue;
            }
            if self.peek_kind(offset) == Some(&TokenKind::Keyword(Keyword::Alignas))
                && self.peek_kind(offset + 1) == Some(&TokenKind::LParen)
            {
                offset += 2;
                let mut depth = 1usize;
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
                        TokenKind::Eof => break,
                        _ => {}
                    }
                    offset += 1;
                }
                continue;
            }
            break;
        }
        matches!(
            self.peek_kind(offset),
            Some(TokenKind::Keyword(
                Keyword::Bool
                    | Keyword::Atomic
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
                Keyword::Atomic | Keyword::Const | Keyword::Restrict | Keyword::Volatile
            ))
        ) && !(self.peek_kind(offset) == Some(&TokenKind::Keyword(Keyword::Atomic))
            && self.peek_kind(offset + 1) == Some(&TokenKind::LParen))
        {
            offset += 1;
        }
        matches!(
            self.peek_kind(offset),
            Some(TokenKind::Keyword(
                Keyword::Bool
                    | Keyword::Atomic
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
                if !self.type_is_complete(pointee) {
                    return Err(Diagnostic::error(
                        "restrict-qualified pointers must point to a complete object type",
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
                for param in params.iter() {
                    self.validate_restrict_usage(param, span)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn adjust_parameter_type(&self, ty: CType) -> CType {
        match ty.unqualified() {
            CType::Array(inner, _) => CType::qualified(
                CType::pointer_to((**inner).clone()),
                ty.top_level_qualifiers(),
            ),
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

    fn lookup_typedef_vla_bounds(&self, name: &str) -> Vec<Option<Expr>> {
        self.visible_scope_entry(name)
            .map(|entry| entry.typedef_vla_bounds.clone())
            .unwrap_or_default()
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
            && outer.ordinary != Some(SymbolKind::Object)
        {
            return Err(Diagnostic::error(
                format!("{} is already declared as a different kind of symbol", name),
                span,
            ));
        }
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = Some(SymbolKind::Object);
        entry.ordinary_ty = Some(ty.clone());
        entry.ordinary_storage_class = storage_class;
        entry.ordinary_linkage = linkage;
        entry.typedef_ty = None;
        entry.typedef_vla_bounds.clear();
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
                if existing.ordinary_ty.as_ref().is_some_and(|existing_ty| {
                    same_translation_unit_types_use_distinct_tags(existing_ty, ty)
                }) {
                    return Err(Diagnostic::error(
                        format!("conflicting declarations of function {name}"),
                        span,
                    ));
                }
                return Ok(linkage);
            }
        }
        if context == DeclContext::BlockScope
            && let Some(outer) = self.outer_scope_entry(name)
            && outer.ordinary_linkage.is_some()
            && outer.ordinary != Some(SymbolKind::Function)
        {
            return Err(Diagnostic::error(
                format!("{} is already declared as a different kind of symbol", name),
                span,
            ));
        }
        let entry = self.current_scope_mut().entry(name.to_owned()).or_default();
        entry.ordinary = Some(SymbolKind::Function);
        entry.ordinary_ty = Some(ty.clone());
        entry.ordinary_storage_class = storage_class;
        entry.ordinary_linkage = Some(linkage);
        entry.typedef_ty = None;
        entry.typedef_vla_bounds.clear();
        entry.enum_constant = None;
        Ok(linkage)
    }

    fn declare_typedef_name(
        &mut self,
        name: &str,
        ty: CType,
        vla_bounds: Vec<Option<Expr>>,
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
        entry.typedef_vla_bounds = vla_bounds;
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
                && lhs_params.iter().zip(rhs_params.iter()).all(|(lhs, rhs)| {
                    declaration_types_compatible(lhs.unqualified(), rhs.unqualified())
                })
        }
        (CType::Enum(..), CType::Int) | (CType::Int, CType::Enum(..)) => true,
        _ => lhs == rhs,
    }
}

fn same_translation_unit_types_use_distinct_tags(lhs: &CType, rhs: &CType) -> bool {
    match (lhs.unqualified(), rhs.unqualified()) {
        (CType::Struct(lhs_id, _), CType::Struct(rhs_id, _))
        | (CType::Union(lhs_id, _), CType::Union(rhs_id, _))
        | (CType::Enum(lhs_id, _), CType::Enum(rhs_id, _)) => lhs_id != rhs_id,
        (CType::Pointer(lhs), CType::Pointer(rhs)) | (CType::Complex(lhs), CType::Complex(rhs)) => {
            same_translation_unit_types_use_distinct_tags(lhs, rhs)
        }
        (CType::Array(lhs, _), CType::Array(rhs, _)) => {
            same_translation_unit_types_use_distinct_tags(lhs, rhs)
        }
        (CType::Function(lhs_ret, lhs_params, _), CType::Function(rhs_ret, rhs_params, _)) => {
            same_translation_unit_types_use_distinct_tags(lhs_ret, rhs_ret)
                || (lhs_params.len() == rhs_params.len()
                    && lhs_params
                        .iter()
                        .zip(rhs_params.iter())
                        .any(|(lhs, rhs)| same_translation_unit_types_use_distinct_tags(lhs, rhs)))
        }
        _ => false,
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
        (CType::Enum(..), CType::Int) | (CType::Int, CType::Enum(..)) => true,
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

fn align_up(value: usize, align: usize) -> usize {
    if align == 0 {
        value
    } else {
        value.div_ceil(align) * align
    }
}
