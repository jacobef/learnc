//! Struct, union, and enum declarations and layout.

use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_record_specifier(&mut self, kind: RecordKind) -> Result<CType, Diagnostic> {
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
            RecordKind::Struct => CType::Struct(id, tag.map(|tag| Arc::new(tag.to_owned()))),
            RecordKind::Union => CType::Union(id, tag.map(|tag| Arc::new(tag.to_owned()))),
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
                    alignment: specs.alignment,
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
                    alignment: specs.alignment,
                    declaration_span: None,
                });
            } else {
                let ResolvedDeclarator {
                    name: first_name,
                    ty: first_ty,
                    vla_bounds: first_vla_bounds,
                    span: first_span,
                    function_params,
                    ..
                } = self.parse_declarator(base.clone(), DeclContext::RecordMember)?;
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
                    alignment: specs.alignment,
                    declaration_span: Some(first_span),
                });
                while self.eat(TokenKind::Comma) {
                    let ResolvedDeclarator {
                        name,
                        ty,
                        vla_bounds,
                        span,
                        function_params,
                        ..
                    } = self.parse_declarator(base.clone(), DeclContext::RecordMember)?;
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
                        alignment: specs.alignment,
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
        if let Some(alignment) = member.alignment
            && alignment != 0
            && alignment < self.type_align_of(&member.ty)?
        {
            return Err(Diagnostic::error(
                "_Alignas cannot request an alignment weaker than the type's natural alignment",
                span,
            ));
        }
        if let Some(width) = member.bit_width {
            if member.alignment.is_some() {
                return Err(Diagnostic::error(
                    "_Alignas cannot appear on a bit-field",
                    span,
                ));
            }
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
        let mut used_bits = 0u8;
        let mut open_bit_offset = 0usize;
        match kind {
            RecordKind::Struct => {
                for member in &mut members {
                    if let Some(width) = member.bit_width {
                        let storage_size = self.type_size_of(&member.ty)?;
                        let storage_align = self.type_align_of(&member.ty)?;
                        align = align.max(storage_align);
                        if width == 0 {
                            size = align_up(size, storage_align);
                            open_bit_size = 0;
                            used_bits = 0;
                            member.offset = size;
                            member.bit_offset = 0;
                            member.bit_storage_size = storage_size;
                            continue;
                        }
                        let needs_new_unit = open_bit_size == 0
                            || usize::from(used_bits) + usize::from(width) > open_bit_size * 8;
                        if needs_new_unit {
                            size = align_up(size, storage_align);
                            open_bit_offset = size;
                            size += storage_size;
                            open_bit_size = storage_size;
                            used_bits = 0;
                        }
                        member.offset = open_bit_offset;
                        member.bit_offset = used_bits;
                        member.bit_storage_size = open_bit_size;
                        used_bits += width;
                        continue;
                    }
                    open_bit_size = 0;
                    used_bits = 0;
                    let member_align = member
                        .alignment
                        .filter(|alignment| *alignment != 0)
                        .unwrap_or(self.type_align_of(&member.ty)?);
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
                    let member_align = member
                        .alignment
                        .filter(|alignment| *alignment != 0)
                        .unwrap_or(self.type_align_of(&member.ty)?);
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
            CType::Struct(_, None) | CType::Union(_, None) if self.type_is_complete(ty) => Ok(()),
            CType::Struct(_, _) | CType::Union(_, _) => Err(Diagnostic::error(
                format!(
                    "a declarator-less record member must be a complete tagless anonymous structure or union, not {ty}"
                ),
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

    pub(super) fn parse_enum_specifier(&mut self) -> Result<CType, Diagnostic> {
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
                    for file in self.tokens.iter().map(|token| token.span.file) {
                        self.enum_constants.insert((file, name.clone()), value);
                    }
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
        } else if !self.enums.get(&id).is_some_and(|enum_ty| enum_ty.complete) {
            return Err(Diagnostic::error(
                "an enum tag without an enumerator list must refer to a complete enum type",
                start,
            ));
        }
        Ok(CType::Enum(id, tag.map(|tag| Arc::new(tag.to_owned()))))
    }
}
