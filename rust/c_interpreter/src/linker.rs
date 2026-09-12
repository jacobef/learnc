//! Translation-unit normalization, C linkage, and composite type construction.

mod composite;
mod remap;
pub(crate) use composite::composite_type;

use remap::TagRemapper;
use std::collections::{HashMap, HashSet};

use crate::ast::{
    Block, BlockItem, Declaration, Expr, ExternalDeclaration, ForInit, FunctionDecl, FunctionDef,
    Initializer, Linkage, Parameter, Statement, StorageClass, SwitchLabel, TranslationUnit,
};
use crate::diag::Diagnostic;
use crate::fast_hash::FastHashMap;
use crate::source::{FileId, Span};
use crate::types::{CType, EnumType, RecordType};

pub(crate) fn normalize_single_translation_unit(
    mut unit: TranslationUnit,
) -> Result<TranslationUnit, Diagnostic> {
    let original_externals = unit.externals.clone();
    let normalized = normalize_translation_unit(
        std::mem::take(&mut unit.externals),
        &unit.records,
        &unit.enums,
    )?;
    unit.externals = original_externals;
    unit.function_declarations = normalized.function_declarations;
    unit.functions = normalized.function_definitions;
    unit.globals = normalized.global_declarations;
    unit.global_definitions = normalized.global_definitions;
    unit.inline_function_definitions = normalized.inline_function_definitions;
    Ok(unit)
}

pub(crate) fn merge_translation_units(
    units: Vec<TranslationUnit>,
) -> Result<TranslationUnit, Diagnostic> {
    let mut merged = TranslationUnit {
        externals: Vec::new(),
        functions: Vec::new(),
        function_declarations: Vec::new(),
        globals: Vec::new(),
        global_definitions: Vec::new(),
        inline_function_definitions: Vec::new(),
        records: Default::default(),
        enums: Default::default(),
        enum_constants: Default::default(),
    };
    let mut external_function_decls = HashMap::<String, FunctionDecl>::new();
    let mut external_function_defs = HashMap::<String, FunctionDef>::new();
    let mut external_object_decls = HashMap::<String, Declaration>::new();
    let mut external_object_defs = HashMap::<String, Declaration>::new();
    let mut external_symbol_kinds = HashMap::<String, ExternalSymbolKind>::new();
    let mut next_record_id = 0usize;
    let mut next_enum_id = 0usize;

    for unit in units {
        let record_map = unit
            .records
            .keys()
            .copied()
            .map(|old_id| {
                let new_id = next_record_id;
                next_record_id += 1;
                (old_id, new_id)
            })
            .collect::<std::collections::HashMap<_, _>>();
        let enum_map = unit
            .enums
            .keys()
            .copied()
            .map(|old_id| {
                let new_id = next_enum_id;
                next_enum_id += 1;
                (old_id, new_id)
            })
            .collect::<std::collections::HashMap<_, _>>();

        let remapper = TagRemapper {
            records: record_map,
            enums: enum_map,
        };
        for (old_id, mut record) in unit.records {
            record.id = remapper.records[&old_id];
            for member in &mut record.members {
                remapper.ty(&mut member.ty);
            }
            merged.records.insert(record.id, record);
        }
        for (old_id, mut enum_ty) in unit.enums {
            enum_ty.id = remapper.enums[&old_id];
            merged.enums.insert(enum_ty.id, enum_ty);
        }
        let mut remapped_externals = unit.externals;
        for external in &mut remapped_externals {
            remapper.external(external);
        }
        merged.externals.extend(remapped_externals.iter().cloned());
        let normalized =
            normalize_translation_unit(remapped_externals, &merged.records, &merged.enums)?;
        merged
            .inline_function_definitions
            .extend(normalized.inline_function_definitions.iter().cloned());
        for function_decl in normalized.function_declarations {
            if function_decl.storage_class == Some(StorageClass::Static) {
                merged.function_declarations.push(function_decl);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &function_decl.name,
                ExternalSymbolKind::Function,
                function_decl.span,
            )?;
            external_symbol_kinds.insert(function_decl.name.clone(), ExternalSymbolKind::Function);
            if let Some(existing) = external_function_decls.get_mut(&function_decl.name) {
                merge_function_declaration(
                    existing,
                    &function_decl,
                    &merged.records,
                    &merged.enums,
                )?;
            } else {
                external_function_decls.insert(function_decl.name.clone(), function_decl);
            }
        }
        for function in normalized.function_definitions {
            if function.storage_class == Some(StorageClass::Static) {
                merged.functions.push(function);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &function.name,
                ExternalSymbolKind::Function,
                function.span,
            )?;
            external_symbol_kinds.insert(function.name.clone(), ExternalSymbolKind::Function);
            let function_decl = function_decl_from_definition(&function);
            if let Some(existing) = external_function_decls.get_mut(&function.name) {
                merge_function_declaration(
                    existing,
                    &function_decl,
                    &merged.records,
                    &merged.enums,
                )?;
            } else {
                external_function_decls.insert(function.name.clone(), function_decl);
            }
            if let Some(existing) = external_function_defs.get(&function.name) {
                return Err(Diagnostic::error(
                    format!("multiple definitions of function {}", function.name),
                    function.span,
                )
                .with_note(format!(
                    "previous definition is at {}:{}:{}",
                    existing.span.file.0, existing.span.start, existing.span.end
                )));
            }
            external_function_defs.insert(function.name.clone(), function);
        }
        for global_decl in normalized.global_declarations {
            if global_decl.storage_class == Some(StorageClass::Static) {
                merged.globals.push(global_decl);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &global_decl.name,
                ExternalSymbolKind::Object,
                global_decl.span,
            )?;
            external_symbol_kinds.insert(global_decl.name.clone(), ExternalSymbolKind::Object);
            if let Some(existing) = external_object_decls.get_mut(&global_decl.name) {
                merge_object_declaration(existing, &global_decl, &merged.records, &merged.enums)?;
            } else {
                external_object_decls.insert(global_decl.name.clone(), global_decl);
            }
        }
        for global_def in normalized.global_definitions {
            if global_def.storage_class == Some(StorageClass::Static) {
                merged.global_definitions.push(global_def);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &global_def.name,
                ExternalSymbolKind::Object,
                global_def.span,
            )?;
            external_symbol_kinds.insert(global_def.name.clone(), ExternalSymbolKind::Object);
            if let Some(existing) = external_object_decls.get_mut(&global_def.name) {
                merge_object_declaration(existing, &global_def, &merged.records, &merged.enums)?;
            } else {
                external_object_decls.insert(global_def.name.clone(), global_def.clone());
            }
            if let Some(existing) = external_object_defs.get(&global_def.name) {
                return Err(Diagnostic::error(
                    format!("multiple definitions of object {}", global_def.name),
                    global_def.span,
                )
                .with_note(format!(
                    "previous definition is at {}:{}:{}",
                    existing.span.file.0, existing.span.start, existing.span.end
                )));
            }
            external_object_defs.insert(global_def.name.clone(), global_def);
        }
        for (key, value) in unit.enum_constants {
            if let Some(existing) = merged.enum_constants.insert(key.clone(), value)
                && existing != value
            {
                return Err(Diagnostic::error(
                    format!("conflicting enum constant {}", key.1),
                    fallback_span(&merged),
                ));
            }
        }
    }

    for (name, definition) in &mut external_function_defs {
        if let Some(declaration) = external_function_decls.get(name) {
            definition.is_noreturn = declaration.is_noreturn;
        }
    }
    for (name, definition) in &mut external_object_defs {
        if let Some(declaration) = external_object_decls.get(name) {
            definition.alignment = declaration.alignment;
        }
    }

    merged
        .function_declarations
        .extend(external_function_decls.into_values());
    merged
        .functions
        .extend(external_function_defs.into_values());
    merged.globals.extend(external_object_decls.into_values());
    merged
        .global_definitions
        .extend(external_object_defs.into_values());

    Ok(merged)
}

pub(crate) fn fallback_span(unit: &TranslationUnit) -> Span {
    unit.functions
        .first()
        .map(|function| function.span)
        .or_else(|| unit.function_declarations.first().map(|decl| decl.span))
        .or_else(|| unit.global_definitions.first().map(|global| global.span))
        .or_else(|| unit.globals.first().map(|global| global.span))
        .or_else(|| {
            unit.externals
                .iter()
                .map(|external| match external {
                    ExternalDeclaration::Function(function) => function.span,
                    ExternalDeclaration::FunctionDeclaration(decl) => decl.span,
                    ExternalDeclaration::ObjectDeclaration(decl) => decl.span,
                })
                .next()
        })
        .unwrap_or(Span::new(FileId(0), 0, 0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalSymbolKind {
    Function,
    Object,
}

#[derive(Debug, Clone)]
struct NormalizedTranslationUnit {
    function_declarations: Vec<FunctionDecl>,
    function_definitions: Vec<FunctionDef>,
    global_declarations: Vec<Declaration>,
    global_definitions: Vec<Declaration>,
    inline_function_definitions: Vec<FunctionDef>,
}

#[derive(Debug, Clone)]
struct UnitFunctionEntry {
    declaration: FunctionDecl,
    definition: Option<FunctionDef>,
    linkage: Linkage,
    saw_inline: bool,
    saw_non_inline: bool,
    saw_extern: bool,
    inline_span: Option<Span>,
}

#[derive(Debug, Clone)]
struct UnitObjectEntry {
    declaration: Declaration,
    real_definition: Option<Declaration>,
    has_tentative_definition: bool,
}

#[derive(Debug, Clone)]
struct PriorSymbol {
    kind: ExternalSymbolKind,
    linkage: Linkage,
    span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ScopedSymbol {
    Internal(FileId, String),
    External(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectRole {
    Declaration,
    TentativeDefinition,
    Definition,
}

fn normalize_translation_unit(
    externals: Vec<ExternalDeclaration>,
    records: &FastHashMap<usize, RecordType>,
    enums: &FastHashMap<usize, EnumType>,
) -> Result<NormalizedTranslationUnit, Diagnostic> {
    let mut prior_symbols = HashMap::<String, PriorSymbol>::new();
    let mut function_entries = HashMap::<ScopedSymbol, UnitFunctionEntry>::new();
    let mut object_entries = HashMap::<ScopedSymbol, UnitObjectEntry>::new();

    for external in externals {
        match external {
            ExternalDeclaration::Function(function) => {
                let decl = function_decl_from_definition(&function);
                let linkage = validate_linkage(
                    &function.name,
                    function.linkage,
                    ExternalSymbolKind::Function,
                    function.span,
                    &mut prior_symbols,
                )?;
                let key = scoped_symbol(function.span.file, &function.name, linkage);
                let normalized_decl = normalize_function_decl_linkage(decl, linkage);
                let normalized_def = normalize_function_def_linkage(function, linkage);
                if let Some(entry) = function_entries.get_mut(&key) {
                    merge_function_declaration(
                        &mut entry.declaration,
                        &normalized_decl,
                        records,
                        enums,
                    )?;
                    if entry.definition.is_some() {
                        return Err(Diagnostic::error(
                            format!("multiple definitions of function {}", normalized_def.name),
                            normalized_def.span,
                        ));
                    }
                    entry.definition = Some(normalized_def);
                    entry.saw_inline |= normalized_decl.is_inline;
                    entry.saw_non_inline |= !normalized_decl.is_inline;
                    entry.saw_extern |= normalized_decl.storage_class == Some(StorageClass::Extern);
                    entry.inline_span = entry
                        .inline_span
                        .or(normalized_decl.is_inline.then_some(normalized_decl.span));
                } else {
                    let is_inline = normalized_decl.is_inline;
                    let storage_class = normalized_decl.storage_class;
                    let inline_span = is_inline.then_some(normalized_decl.span);
                    function_entries.insert(
                        key,
                        UnitFunctionEntry {
                            declaration: normalized_decl,
                            definition: Some(normalized_def),
                            linkage,
                            saw_inline: is_inline,
                            saw_non_inline: !is_inline,
                            saw_extern: storage_class == Some(StorageClass::Extern),
                            inline_span,
                        },
                    );
                }
            }
            ExternalDeclaration::FunctionDeclaration(function_decl) => {
                let linkage = validate_linkage(
                    &function_decl.name,
                    function_decl.linkage,
                    ExternalSymbolKind::Function,
                    function_decl.span,
                    &mut prior_symbols,
                )?;
                let key = scoped_symbol(function_decl.span.file, &function_decl.name, linkage);
                let normalized_decl = normalize_function_decl_linkage(function_decl, linkage);
                if let Some(entry) = function_entries.get_mut(&key) {
                    merge_function_declaration(
                        &mut entry.declaration,
                        &normalized_decl,
                        records,
                        enums,
                    )?;
                    entry.saw_inline |= normalized_decl.is_inline;
                    entry.saw_non_inline |= !normalized_decl.is_inline;
                    entry.saw_extern |= normalized_decl.storage_class == Some(StorageClass::Extern);
                    entry.inline_span = entry
                        .inline_span
                        .or(normalized_decl.is_inline.then_some(normalized_decl.span));
                } else {
                    let is_inline = normalized_decl.is_inline;
                    let storage_class = normalized_decl.storage_class;
                    let inline_span = is_inline.then_some(normalized_decl.span);
                    function_entries.insert(
                        key,
                        UnitFunctionEntry {
                            declaration: normalized_decl,
                            definition: None,
                            linkage,
                            saw_inline: is_inline,
                            saw_non_inline: !is_inline,
                            saw_extern: storage_class == Some(StorageClass::Extern),
                            inline_span,
                        },
                    );
                }
            }
            ExternalDeclaration::ObjectDeclaration(decl) => {
                let role = classify_object_role(&decl);
                let linkage = validate_linkage(
                    &decl.name,
                    decl.linkage
                        .expect("translation-unit object declaration must have linkage"),
                    ExternalSymbolKind::Object,
                    decl.span,
                    &mut prior_symbols,
                )?;
                let key = scoped_symbol(decl.span.file, &decl.name, linkage);
                let normalized_decl = normalize_object_linkage(decl, linkage);
                if let Some(entry) = object_entries.get_mut(&key) {
                    merge_object_declaration(
                        &mut entry.declaration,
                        &normalized_decl,
                        records,
                        enums,
                    )?;
                    match role {
                        ObjectRole::Declaration => {}
                        ObjectRole::TentativeDefinition => entry.has_tentative_definition = true,
                        ObjectRole::Definition => {
                            if entry.real_definition.is_some() {
                                return Err(Diagnostic::error(
                                    format!(
                                        "multiple definitions of object {}",
                                        normalized_decl.name
                                    ),
                                    normalized_decl.span,
                                ));
                            }
                            entry.real_definition = Some(normalized_decl);
                        }
                    }
                } else {
                    object_entries.insert(
                        key,
                        UnitObjectEntry {
                            declaration: normalized_decl.clone(),
                            real_definition: (role == ObjectRole::Definition)
                                .then_some(normalized_decl),
                            has_tentative_definition: role == ObjectRole::TentativeDefinition,
                        },
                    );
                }
            }
        }
    }

    let mut normalized = NormalizedTranslationUnit {
        function_declarations: Vec::new(),
        function_definitions: Vec::new(),
        global_declarations: Vec::new(),
        global_definitions: Vec::new(),
        inline_function_definitions: Vec::new(),
    };

    let internal_linkage_names = collect_internal_linkage_names(&function_entries, &object_entries);
    for entry in function_entries.into_values() {
        if let Some(definition) = entry.definition.as_ref()
            && !definition.has_prototype
            && entry.declaration.has_prototype
            && composite_type(
                &function_decl_type(&entry.declaration),
                &old_style_promoted_definition_type(definition),
                records,
                enums,
            )
            .is_none()
        {
            return Err(Diagnostic::error(
                format!(
                    "old-style definition of {} is incompatible with its prototype",
                    definition.name
                ),
                definition.span,
            ));
        }
        if entry.declaration.name == "main" && entry.saw_inline {
            return Err(Diagnostic::error(
                "main shall not be declared inline",
                entry.inline_span.unwrap_or(entry.declaration.span),
            ));
        }
        if entry.linkage == Linkage::External && entry.saw_inline && entry.definition.is_none() {
            return Err(Diagnostic::error(
                format!(
                    "inline declaration of function {} with external linkage requires a definition in the same translation unit",
                    entry.declaration.name
                ),
                entry.inline_span.unwrap_or(entry.declaration.span),
            ));
        }
        let is_inline_definition = entry.linkage == Linkage::External
            && entry.definition.is_some()
            && entry.saw_inline
            && !entry.saw_non_inline
            && !entry.saw_extern;
        normalized
            .function_declarations
            .push(entry.declaration.clone());
        if let Some(mut definition) = entry.definition {
            definition.is_noreturn = entry.declaration.is_noreturn;
            if is_inline_definition {
                validate_inline_definition_constraints(
                    &definition,
                    &internal_linkage_names,
                    records,
                )?;
                normalized.inline_function_definitions.push(definition);
            } else {
                normalized.function_definitions.push(definition);
            }
        }
    }
    for mut entry in object_entries.into_values() {
        normalized
            .global_declarations
            .push(entry.declaration.clone());
        if let Some(mut definition) = entry.real_definition.take() {
            definition.alignment = entry.declaration.alignment;
            normalized.global_definitions.push(definition);
        } else if entry.has_tentative_definition {
            if entry.declaration.linkage == Some(Linkage::Internal)
                && !type_is_complete_for_linkage(&entry.declaration.ty, records)
            {
                return Err(Diagnostic::error(
                    format!(
                        "tentative definition of internal-linkage object {} must have complete type",
                        entry.declaration.name
                    ),
                    entry.declaration.span,
                ));
            }
            normalized
                .global_definitions
                .push(finalize_tentative_definition(entry.declaration));
        }
    }

    Ok(normalized)
}

fn type_is_complete_for_linkage(ty: &CType, records: &FastHashMap<usize, RecordType>) -> bool {
    match ty.unqualified() {
        CType::Void | CType::Function(..) | CType::Array(_, 0) => false,
        CType::Array(inner, _) => type_is_complete_for_linkage(inner, records),
        CType::Struct(id, _) | CType::Union(id, _) => {
            records.get(id).is_some_and(|record| record.complete)
        }
        _ => true,
    }
}

fn validate_linkage(
    name: &str,
    linkage: Linkage,
    kind: ExternalSymbolKind,
    span: Span,
    prior_symbols: &mut HashMap<String, PriorSymbol>,
) -> Result<Linkage, Diagnostic> {
    let prior = prior_symbols.get(name).cloned();
    if let Some(prior) = &prior {
        if prior.kind != kind {
            return Err(Diagnostic::error(
                format!(
                    "identifier {} is declared as both a {} and an {}",
                    name,
                    describe_external_symbol_kind(prior.kind),
                    describe_external_symbol_kind(kind)
                ),
                span,
            ));
        }
        if prior.linkage != linkage {
            return Err(Diagnostic::ub(
                format!(
                    "identifier {} is declared with both internal and external linkage",
                    name
                ),
                span,
                Some("6.2.2"),
            )
            .with_related_span(
                "previous-declaration",
                "previous declaration is here",
                prior.span,
            ));
        }
    }
    prior_symbols.insert(
        name.to_owned(),
        PriorSymbol {
            kind,
            linkage,
            span,
        },
    );
    Ok(linkage)
}

fn scoped_symbol(file: FileId, name: &str, linkage: Linkage) -> ScopedSymbol {
    match linkage {
        Linkage::Internal => ScopedSymbol::Internal(file, name.to_owned()),
        Linkage::External => ScopedSymbol::External(name.to_owned()),
    }
}

fn classify_object_role(decl: &Declaration) -> ObjectRole {
    if decl.init.is_some() {
        ObjectRole::Definition
    } else if decl.storage_class == Some(StorageClass::Extern) {
        ObjectRole::Declaration
    } else {
        ObjectRole::TentativeDefinition
    }
}

fn collect_internal_linkage_names(
    function_entries: &HashMap<ScopedSymbol, UnitFunctionEntry>,
    object_entries: &HashMap<ScopedSymbol, UnitObjectEntry>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for entry in function_entries.values() {
        if entry.linkage == Linkage::Internal {
            names.insert(entry.declaration.name.clone());
        }
    }
    for entry in object_entries.values() {
        if entry.declaration.storage_class == Some(StorageClass::Static) {
            names.insert(entry.declaration.name.clone());
        }
    }
    names
}

fn validate_inline_definition_constraints(
    function: &FunctionDef,
    internal_linkage_names: &HashSet<String>,
    records: &FastHashMap<usize, RecordType>,
) -> Result<(), Diagnostic> {
    let mut local_scopes = vec![HashSet::new()];
    for param in &function.params {
        validate_inline_vla_bounds(&param.vla_bounds, internal_linkage_names, &mut local_scopes)?;
        if let Some(name) = &param.name {
            local_scopes
                .last_mut()
                .expect("parameter scope exists")
                .insert(name.clone());
        }
    }
    validate_inline_block(
        &function.body,
        internal_linkage_names,
        records,
        &mut local_scopes,
    )
}

fn validate_inline_block(
    block: &Block,
    internal_linkage_names: &HashSet<String>,
    records: &FastHashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    local_scopes.push(HashSet::new());
    for item in &block.items {
        match item {
            BlockItem::Declaration(decl) => {
                validate_inline_vla_bounds(&decl.vla_bounds, internal_linkage_names, local_scopes)?;
                if decl.storage_class == Some(StorageClass::Static)
                    && type_is_modifiable_object(&decl.ty, records)
                {
                    local_scopes.pop();
                    return Err(Diagnostic::error(
                        "inline definition with external linkage shall not define a modifiable object with static storage duration",
                        decl.span,
                    ));
                }
                if decl.storage_class != Some(StorageClass::Extern) {
                    local_scopes
                        .last_mut()
                        .expect("block scope exists")
                        .insert(decl.name.clone());
                }
                if let Some(init) = &decl.init {
                    validate_inline_initializer(
                        init,
                        internal_linkage_names,
                        records,
                        local_scopes,
                    )?;
                }
            }
            BlockItem::FunctionDeclaration(_) => {}
            BlockItem::Statement(stmt) => {
                validate_inline_statement(stmt, internal_linkage_names, records, local_scopes)?;
            }
        }
    }
    local_scopes.pop();
    Ok(())
}

fn validate_inline_statement(
    stmt: &Statement,
    internal_linkage_names: &HashSet<String>,
    records: &FastHashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    match stmt {
        Statement::Block(block) => {
            validate_inline_block(block, internal_linkage_names, records, local_scopes)
        }
        Statement::Break(_) | Statement::Continue(_) => Ok(()),
        Statement::DoWhile {
            body, condition, ..
        } => {
            validate_inline_statement(body, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)
        }
        Statement::Expression(expr, _) => expr.as_ref().map_or(Ok(()), |expr| {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }),
        Statement::For {
            init,
            condition,
            step,
            body,
            ..
        } => {
            local_scopes.push(HashSet::new());
            if let Some(init) = init {
                match init {
                    ForInit::Declarations(decls) => {
                        for decl in decls {
                            validate_inline_vla_bounds(
                                &decl.vla_bounds,
                                internal_linkage_names,
                                local_scopes,
                            )?;
                            if decl.storage_class == Some(StorageClass::Static)
                                && type_is_modifiable_object(&decl.ty, records)
                            {
                                local_scopes.pop();
                                return Err(Diagnostic::error(
                                    "inline definition with external linkage shall not define a modifiable object with static storage duration",
                                    decl.span,
                                ));
                            }
                            if decl.storage_class != Some(StorageClass::Extern) {
                                local_scopes
                                    .last_mut()
                                    .expect("for scope exists")
                                    .insert(decl.name.clone());
                            }
                            if let Some(init) = &decl.init {
                                validate_inline_initializer(
                                    init,
                                    internal_linkage_names,
                                    records,
                                    local_scopes,
                                )?;
                            }
                        }
                    }
                    ForInit::Expression(expr) => {
                        validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
                    }
                }
            }
            if let Some(condition) = condition {
                validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            }
            if let Some(step) = step {
                validate_inline_expr(step, internal_linkage_names, records, local_scopes)?;
            }
            let result =
                validate_inline_statement(body, internal_linkage_names, records, local_scopes);
            local_scopes.pop();
            result
        }
        Statement::Goto { .. } => Ok(()),
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            validate_inline_statement(then_branch, internal_linkage_names, records, local_scopes)?;
            if let Some(else_branch) = else_branch {
                validate_inline_statement(
                    else_branch,
                    internal_linkage_names,
                    records,
                    local_scopes,
                )?;
            }
            Ok(())
        }
        Statement::Labeled {
            label, statement, ..
        } => {
            if let SwitchLabel::Case { expr, .. } = label {
                validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
            }
            validate_inline_statement(statement, internal_linkage_names, records, local_scopes)
        }
        Statement::Return(expr, _) => expr.as_ref().map_or(Ok(()), |expr| {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }),
        Statement::Switch { expr, body, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
            validate_inline_block(body, internal_linkage_names, records, local_scopes)
        }
        Statement::UserLabeled { statement, .. } => {
            validate_inline_statement(statement, internal_linkage_names, records, local_scopes)
        }
        Statement::While {
            condition, body, ..
        } => {
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            validate_inline_statement(body, internal_linkage_names, records, local_scopes)
        }
    }
}

fn validate_inline_initializer(
    init: &Initializer,
    internal_linkage_names: &HashSet<String>,
    records: &FastHashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    match init {
        Initializer::Expr(expr) => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Initializer::List { items, .. } => {
            for item in items {
                validate_inline_initializer(
                    &item.initializer,
                    internal_linkage_names,
                    records,
                    local_scopes,
                )?;
            }
            Ok(())
        }
    }
}

fn validate_inline_vla_bounds(
    bounds: &[Option<Expr>],
    internal_linkage_names: &HashSet<String>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    for expr in bounds.iter().flatten() {
        validate_inline_expr(
            expr,
            internal_linkage_names,
            &FastHashMap::default(),
            local_scopes,
        )?;
    }
    Ok(())
}

fn validate_inline_expr(
    expr: &Expr,
    internal_linkage_names: &HashSet<String>,
    records: &FastHashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    match expr {
        Expr::Number(_, _)
        | Expr::CharLiteral(_, _)
        | Expr::WideCharLiteral(_, _)
        | Expr::Utf16CharLiteral(_, _)
        | Expr::Utf32CharLiteral(_, _)
        | Expr::StringLiteral(_, _)
        | Expr::WideStringLiteral(_, _) => Ok(()),
        Expr::Utf16StringLiteral(_, _) | Expr::Utf32StringLiteral(_, _) => Ok(()),
        Expr::Variable(name, span) => {
            if !local_scopes.iter().rev().any(|scope| scope.contains(name))
                && internal_linkage_names.contains(name)
            {
                Err(Diagnostic::error(
                    format!(
                        "inline definition with external linkage shall not reference internal-linkage identifier {}",
                        name
                    ),
                    *span,
                ))
            } else {
                Ok(())
            }
        }
        Expr::Unary { expr, .. } | Expr::Postfix { expr, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Subscript {
            base: lhs,
            index: rhs,
            ..
        }
        | Expr::Assign { lhs, rhs, .. }
        | Expr::CompoundAssign { lhs, rhs, .. } => {
            validate_inline_expr(lhs, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(rhs, internal_linkage_names, records, local_scopes)
        }
        Expr::SizeofType { vla_bounds, .. } => {
            for expr in vla_bounds.iter().flatten() {
                validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
            }
            Ok(())
        }
        Expr::SizeofExpr { expr, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Expr::OffsetOf { .. } => Ok(()),
        Expr::Cast { expr, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Expr::CompoundLiteral { initializer, .. } => {
            validate_inline_initializer(initializer, internal_linkage_names, records, local_scopes)
        }
        Expr::GenericSelection {
            control,
            associations,
            default,
            ..
        } => {
            validate_inline_expr(control, internal_linkage_names, records, local_scopes)?;
            for association in associations {
                validate_inline_expr(
                    &association.expr,
                    internal_linkage_names,
                    records,
                    local_scopes,
                )?;
            }
            if let Some(default) = default {
                validate_inline_expr(default, internal_linkage_names, records, local_scopes)?;
            }
            Ok(())
        }
        Expr::VaArg { ap, .. } => {
            validate_inline_expr(ap, internal_linkage_names, records, local_scopes)
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(then_expr, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(else_expr, internal_linkage_names, records, local_scopes)
        }
        Expr::Call { callee, args, .. } => {
            validate_inline_expr(callee, internal_linkage_names, records, local_scopes)?;
            for arg in args {
                validate_inline_expr(arg, internal_linkage_names, records, local_scopes)?;
            }
            Ok(())
        }
        Expr::Member { base, .. } => {
            validate_inline_expr(base, internal_linkage_names, records, local_scopes)
        }
    }
}

fn type_is_modifiable_object(ty: &CType, records: &FastHashMap<usize, RecordType>) -> bool {
    if ty.is_const_qualified() {
        return false;
    }
    match ty.unqualified() {
        CType::Array(inner, _) => type_is_modifiable_object(inner, records),
        CType::Struct(id, _) | CType::Union(id, _) => records
            .get(id)
            .map(|record| {
                record
                    .members
                    .iter()
                    .all(|member| type_is_modifiable_object(&member.ty, records))
            })
            .unwrap_or(true),
        _ => true,
    }
}

fn normalize_function_decl_linkage(mut decl: FunctionDecl, linkage: Linkage) -> FunctionDecl {
    if linkage == Linkage::Internal {
        decl.storage_class = Some(StorageClass::Static);
    }
    decl
}

fn normalize_function_def_linkage(mut function: FunctionDef, linkage: Linkage) -> FunctionDef {
    if linkage == Linkage::Internal {
        function.storage_class = Some(StorageClass::Static);
    }
    function
}

fn normalize_object_linkage(mut decl: Declaration, linkage: Linkage) -> Declaration {
    if linkage == Linkage::Internal {
        decl.storage_class = Some(StorageClass::Static);
    }
    decl
}

fn finalize_tentative_definition(mut decl: Declaration) -> Declaration {
    if let CType::Array(inner, 0) = decl.ty.unqualified() {
        decl.ty = CType::array_of((**inner).clone(), 1);
    }
    decl.init = None;
    decl
}

fn ensure_external_symbol_kind(
    kinds: &HashMap<String, ExternalSymbolKind>,
    name: &str,
    expected: ExternalSymbolKind,
    span: Span,
) -> Result<(), Diagnostic> {
    if let Some(existing) = kinds.get(name)
        && *existing != expected
    {
        return Err(Diagnostic::error(
            format!(
                "external identifier {} is declared as both a {} and an {}",
                name,
                describe_external_symbol_kind(*existing),
                describe_external_symbol_kind(expected),
            ),
            span,
        ));
    }
    Ok(())
}

fn describe_external_symbol_kind(kind: ExternalSymbolKind) -> &'static str {
    match kind {
        ExternalSymbolKind::Function => "function",
        ExternalSymbolKind::Object => "object",
    }
}

fn merge_function_declaration(
    existing: &mut FunctionDecl,
    new_decl: &FunctionDecl,
    records: &FastHashMap<usize, RecordType>,
    enums: &FastHashMap<usize, EnumType>,
) -> Result<(), Diagnostic> {
    let existing_ty = function_decl_type(existing);
    let new_ty = function_decl_type(new_decl);
    let composite = composite_type(&existing_ty, &new_ty, records, enums).ok_or_else(|| {
        Diagnostic::error(
            format!(
                "conflicting declarations of function {}: the earlier declaration has type {}, but this one has type {}",
                new_decl.name, existing_ty, new_ty
            ),
            new_decl.span,
        )
    })?;
    *existing = function_decl_from_type(
        new_decl.name.clone(),
        composite,
        combine_function_storage(existing.storage_class, new_decl.storage_class),
        existing.linkage,
        existing.is_inline || new_decl.is_inline,
        existing.is_noreturn || new_decl.is_noreturn,
        existing.has_prototype || new_decl.has_prototype,
        combine_spans(existing.span, new_decl.span),
    )?;
    Ok(())
}

fn merge_object_declaration(
    existing: &mut Declaration,
    new_decl: &Declaration,
    records: &FastHashMap<usize, RecordType>,
    enums: &FastHashMap<usize, EnumType>,
) -> Result<(), Diagnostic> {
    let composite =
        composite_type(&existing.ty, &new_decl.ty, records, enums).ok_or_else(|| {
            Diagnostic::error(
                format!(
                    "conflicting declarations of object {}: the earlier declaration has type {}, but this one has type {}",
                    new_decl.name, existing.ty, new_decl.ty
                ),
                new_decl.declarator_span,
            )
        })?;
    existing.ty = composite;
    existing.storage_class = combine_object_storage(existing.storage_class, new_decl.storage_class);
    match (existing.alignment, new_decl.alignment) {
        (Some(lhs), Some(rhs)) if lhs != rhs => {
            return Err(Diagnostic::error(
                format!(
                    "conflicting alignment specifiers for object {}",
                    new_decl.name
                ),
                new_decl.span,
            ));
        }
        (None, Some(alignment)) => existing.alignment = Some(alignment),
        _ => {}
    }
    existing.span = combine_spans(existing.span, new_decl.span);
    Ok(())
}

fn function_decl_from_definition(function: &FunctionDef) -> FunctionDecl {
    FunctionDecl {
        name: function.name.clone(),
        return_type: function.return_type.clone(),
        params: function.params.clone(),
        is_variadic: function.is_variadic,
        storage_class: function.storage_class,
        linkage: function.linkage,
        is_inline: function.is_inline,
        is_noreturn: function.is_noreturn,
        has_prototype: function.has_prototype,
        span: function.span,
    }
}

fn function_decl_type(decl: &FunctionDecl) -> CType {
    if !decl.has_prototype {
        return CType::function(decl.return_type.clone(), Vec::new());
    }
    if decl.is_variadic {
        CType::variadic_function(
            decl.return_type.clone(),
            decl.params.iter().map(|param| param.ty.clone()).collect(),
        )
    } else {
        CType::function(
            decl.return_type.clone(),
            decl.params.iter().map(|param| param.ty.clone()).collect(),
        )
    }
}

fn old_style_promoted_definition_type(function: &FunctionDef) -> CType {
    let params = function
        .params
        .iter()
        .map(|param| match param.ty.unqualified() {
            CType::Float => CType::Double,
            CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(..) => CType::Int,
            _ => param.ty.clone(),
        })
        .collect();
    CType::function(function.return_type.clone(), params)
}

fn function_decl_from_type(
    name: String,
    ty: CType,
    storage_class: Option<StorageClass>,
    linkage: Linkage,
    is_inline: bool,
    is_noreturn: bool,
    has_prototype: bool,
    span: Span,
) -> Result<FunctionDecl, Diagnostic> {
    let CType::Function(return_type, params, is_variadic) = ty.unqualified() else {
        return Err(Diagnostic::error("expected function type", span));
    };
    Ok(FunctionDecl {
        name,
        return_type: (**return_type).clone(),
        params: params
            .iter()
            .cloned()
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
            .collect(),
        is_variadic: *is_variadic,
        storage_class,
        linkage,
        is_inline,
        is_noreturn,
        has_prototype,
        span,
    })
}

fn combine_function_storage(
    lhs: Option<StorageClass>,
    rhs: Option<StorageClass>,
) -> Option<StorageClass> {
    match (lhs, rhs) {
        (Some(StorageClass::Static), _) | (_, Some(StorageClass::Static)) => {
            Some(StorageClass::Static)
        }
        (Some(StorageClass::Extern), _) | (_, Some(StorageClass::Extern)) => {
            Some(StorageClass::Extern)
        }
        (lhs, None) => lhs,
        (None, rhs) => rhs,
        (lhs, rhs) => lhs.or(rhs),
    }
}

fn combine_object_storage(
    lhs: Option<StorageClass>,
    rhs: Option<StorageClass>,
) -> Option<StorageClass> {
    match (lhs, rhs) {
        (Some(StorageClass::Static), _) | (_, Some(StorageClass::Static)) => {
            Some(StorageClass::Static)
        }
        (Some(StorageClass::Extern), Some(StorageClass::Extern)) => Some(StorageClass::Extern),
        (Some(StorageClass::Extern), None) | (None, Some(StorageClass::Extern)) => None,
        (lhs, None) => lhs,
        (None, rhs) => rhs,
        (lhs, rhs) => lhs.or(rhs),
    }
}

fn combine_spans(lhs: Span, rhs: Span) -> Span {
    if lhs.file == rhs.file {
        lhs.merge(rhs)
    } else {
        lhs
    }
}
