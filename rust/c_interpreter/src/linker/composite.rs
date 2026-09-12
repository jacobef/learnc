//! Structural compatibility and composite types across translation units.

use crate::fast_hash::FastHashMap;
use crate::types::{CType, EnumType, RecordType};
use std::collections::HashSet;

pub(crate) fn composite_type(
    lhs: &CType,
    rhs: &CType,
    records: &FastHashMap<usize, RecordType>,
    enums: &FastHashMap<usize, EnumType>,
) -> Option<CType> {
    let mut seen_records = HashSet::new();
    let mut seen_enums = HashSet::new();
    composite_type_inner(lhs, rhs, records, enums, &mut seen_records, &mut seen_enums)
}

fn composite_type_inner(
    lhs: &CType,
    rhs: &CType,
    records: &FastHashMap<usize, RecordType>,
    enums: &FastHashMap<usize, EnumType>,
    seen_records: &mut HashSet<(usize, usize)>,
    seen_enums: &mut HashSet<(usize, usize)>,
) -> Option<CType> {
    match (lhs, rhs) {
        (
            CType::Qualified(lhs_inner, lhs_qualifiers),
            CType::Qualified(rhs_inner, rhs_qualifiers),
        ) if lhs_qualifiers == rhs_qualifiers => Some(CType::qualified(
            composite_type_inner(
                lhs_inner,
                rhs_inner,
                records,
                enums,
                seen_records,
                seen_enums,
            )?,
            *lhs_qualifiers,
        )),
        (CType::Qualified(_, _), _) | (_, CType::Qualified(_, _)) => None,
        (CType::Pointer(lhs_inner), CType::Pointer(rhs_inner)) => {
            Some(CType::pointer_to(composite_type_inner(
                lhs_inner,
                rhs_inner,
                records,
                enums,
                seen_records,
                seen_enums,
            )?))
        }
        (CType::Array(lhs_inner, lhs_len), CType::Array(rhs_inner, rhs_len)) => {
            let len = match (*lhs_len, *rhs_len) {
                (0, 0) => 0,
                (0, len) | (len, 0) => len,
                (lhs_len, rhs_len) if lhs_len == rhs_len => lhs_len,
                _ => return None,
            };
            Some(CType::array_of(
                composite_type_inner(
                    lhs_inner,
                    rhs_inner,
                    records,
                    enums,
                    seen_records,
                    seen_enums,
                )?,
                len,
            ))
        }
        (
            CType::Function(lhs_ret, lhs_params, lhs_variadic),
            CType::Function(rhs_ret, rhs_params, rhs_variadic),
        ) => {
            let return_type =
                composite_type_inner(lhs_ret, rhs_ret, records, enums, seen_records, seen_enums)?;
            if lhs_params.is_empty() && !lhs_variadic {
                if *rhs_variadic
                    || !function_prototype_compatible_with_unspecified_parameters(rhs_params)
                {
                    return None;
                }
                return Some(CType::function(return_type, rhs_params.to_vec()));
            }
            if rhs_params.is_empty() && !rhs_variadic {
                if *lhs_variadic
                    || !function_prototype_compatible_with_unspecified_parameters(lhs_params)
                {
                    return None;
                }
                return Some(CType::function(return_type, lhs_params.to_vec()));
            }
            if lhs_variadic != rhs_variadic || lhs_params.len() != rhs_params.len() {
                return None;
            }
            let mut params = Vec::new();
            for (lhs_param, rhs_param) in lhs_params.iter().zip(rhs_params.iter()) {
                params.push(composite_type_inner(
                    lhs_param.unqualified(),
                    rhs_param.unqualified(),
                    records,
                    enums,
                    seen_records,
                    seen_enums,
                )?);
            }
            Some(if *lhs_variadic {
                CType::variadic_function(return_type, params)
            } else {
                CType::function(return_type, params)
            })
        }
        (CType::Struct(lhs_id, lhs_tag), CType::Struct(rhs_id, rhs_tag)) => {
            compatible_record_types(
                *lhs_id,
                lhs_tag.as_deref().map(String::as_str),
                *rhs_id,
                rhs_tag.as_deref().map(String::as_str),
                RecordKindForComposite::Struct,
                records,
                enums,
                seen_records,
                seen_enums,
            )
            .then(|| select_record_composite(lhs, rhs, records))
        }
        (CType::Union(lhs_id, lhs_tag), CType::Union(rhs_id, rhs_tag)) => compatible_record_types(
            *lhs_id,
            lhs_tag.as_deref().map(String::as_str),
            *rhs_id,
            rhs_tag.as_deref().map(String::as_str),
            RecordKindForComposite::Union,
            records,
            enums,
            seen_records,
            seen_enums,
        )
        .then(|| select_record_composite(lhs, rhs, records)),
        (CType::Enum(lhs_id, lhs_tag), CType::Enum(rhs_id, rhs_tag)) => compatible_enum_types(
            *lhs_id,
            lhs_tag.as_deref().map(String::as_str),
            *rhs_id,
            rhs_tag.as_deref().map(String::as_str),
            enums,
            seen_enums,
        )
        .then(|| select_enum_composite(lhs, rhs, enums)),
        _ if lhs == rhs => Some(lhs.clone()),
        _ => None,
    }
}

fn function_prototype_compatible_with_unspecified_parameters(params: &[CType]) -> bool {
    params == [CType::Void]
        || params
            .iter()
            .all(type_unchanged_by_default_argument_promotions)
}

fn type_unchanged_by_default_argument_promotions(ty: &CType) -> bool {
    !matches!(
        ty.unqualified(),
        CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(..)
            | CType::Float
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordKindForComposite {
    Struct,
    Union,
}

fn compatible_record_types(
    lhs_id: usize,
    lhs_tag: Option<&str>,
    rhs_id: usize,
    rhs_tag: Option<&str>,
    expected_kind: RecordKindForComposite,
    records: &FastHashMap<usize, RecordType>,
    enums: &FastHashMap<usize, EnumType>,
    seen_records: &mut HashSet<(usize, usize)>,
    seen_enums: &mut HashSet<(usize, usize)>,
) -> bool {
    if lhs_id == rhs_id {
        return true;
    }
    if !seen_records.insert((lhs_id, rhs_id)) {
        return true;
    }
    let Some(lhs) = records.get(&lhs_id) else {
        return false;
    };
    let Some(rhs) = records.get(&rhs_id) else {
        return false;
    };
    let expected_kind = match expected_kind {
        RecordKindForComposite::Struct => crate::types::RecordKind::Struct,
        RecordKindForComposite::Union => crate::types::RecordKind::Union,
    };
    if lhs.kind != expected_kind || rhs.kind != expected_kind {
        return false;
    }
    if lhs_tag != rhs_tag {
        return false;
    }
    if !lhs.complete || !rhs.complete {
        return lhs.complete == rhs.complete || lhs_tag == rhs_tag;
    }
    if lhs.members.len() != rhs.members.len() {
        return false;
    }
    lhs.members
        .iter()
        .zip(&rhs.members)
        .all(|(lhs_member, rhs_member)| {
            lhs_member.name == rhs_member.name
                && lhs_member.offset == rhs_member.offset
                && lhs_member.bit_width == rhs_member.bit_width
                && lhs_member.bit_offset == rhs_member.bit_offset
                && lhs_member.bit_storage_size == rhs_member.bit_storage_size
                && composite_type_inner(
                    &lhs_member.ty,
                    &rhs_member.ty,
                    records,
                    enums,
                    seen_records,
                    seen_enums,
                )
                .is_some()
        })
}

fn compatible_enum_types(
    lhs_id: usize,
    lhs_tag: Option<&str>,
    rhs_id: usize,
    rhs_tag: Option<&str>,
    enums: &FastHashMap<usize, EnumType>,
    seen_enums: &mut HashSet<(usize, usize)>,
) -> bool {
    if lhs_id == rhs_id {
        return true;
    }
    if !seen_enums.insert((lhs_id, rhs_id)) {
        return true;
    }
    let Some(lhs) = enums.get(&lhs_id) else {
        return false;
    };
    let Some(rhs) = enums.get(&rhs_id) else {
        return false;
    };
    if lhs_tag != rhs_tag {
        return false;
    }
    lhs.complete == rhs.complete || lhs_tag == rhs_tag
}

fn select_record_composite(
    lhs: &CType,
    rhs: &CType,
    records: &FastHashMap<usize, RecordType>,
) -> CType {
    let lhs_complete = match lhs.unqualified() {
        CType::Struct(id, _) | CType::Union(id, _) => records
            .get(id)
            .map(|record| record.complete)
            .unwrap_or(false),
        _ => false,
    };
    let rhs_complete = match rhs.unqualified() {
        CType::Struct(id, _) | CType::Union(id, _) => records
            .get(id)
            .map(|record| record.complete)
            .unwrap_or(false),
        _ => false,
    };
    if rhs_complete && !lhs_complete {
        rhs.clone()
    } else {
        lhs.clone()
    }
}

fn select_enum_composite(lhs: &CType, rhs: &CType, enums: &FastHashMap<usize, EnumType>) -> CType {
    let lhs_complete = match lhs.unqualified() {
        CType::Enum(id, _) => enums.get(id).map(|ty| ty.complete).unwrap_or(false),
        _ => false,
    };
    let rhs_complete = match rhs.unqualified() {
        CType::Enum(id, _) => enums.get(id).map(|ty| ty.complete).unwrap_or(false),
        _ => false,
    };
    if rhs_complete && !lhs_complete {
        rhs.clone()
    } else {
        lhs.clone()
    }
}
