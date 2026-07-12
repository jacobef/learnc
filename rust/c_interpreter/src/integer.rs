use crate::diag::Diagnostic;
use crate::source::Span;
use crate::types::CType;

pub fn parse_integer_literal(text: &str, span: Span) -> Result<(CType, i128, bool), Diagnostic> {
    let (digits, suffix, decimal_constant, radix) =
        if let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            let digits_len = rest
                .find(|ch: char| !ch.is_ascii_hexdigit())
                .unwrap_or(rest.len());
            if digits_len == 0 {
                return Err(Diagnostic::error(
                    "invalid hexadecimal integer literal",
                    span,
                ));
            }
            (&rest[..digits_len], &rest[digits_len..], false, 16)
        } else {
            let digits_len = text
                .find(|ch: char| !ch.is_ascii_digit())
                .unwrap_or(text.len());
            let (digits, suffix) = text.split_at(digits_len);
            if digits.is_empty() {
                return Err(Diagnostic::error("invalid integer literal", span));
            }
            let decimal_constant = !(digits.starts_with('0') && digits.len() > 1) && digits != "0";
            if !decimal_constant && digits.chars().any(|ch| matches!(ch, '8' | '9')) {
                return Err(Diagnostic::error("invalid octal integer literal", span));
            }
            (
                digits,
                suffix,
                decimal_constant,
                if decimal_constant { 10 } else { 8 },
            )
        };
    let value = u128::from_str_radix(digits, radix)
        .map_err(|_| Diagnostic::error("integer literal is out of supported range", span))?;

    let has_suffix = !suffix.is_empty();
    let suffix = suffix.to_ascii_lowercase();
    let mut unsigned = false;
    let mut long_count = 0usize;
    let mut idx = 0usize;
    let bytes = suffix.as_bytes();
    while idx < bytes.len() {
        match bytes[idx] {
            b'u' if !unsigned => {
                unsigned = true;
                idx += 1;
            }
            b'l' => {
                if idx + 1 < bytes.len() && bytes[idx + 1] == b'l' {
                    long_count += 2;
                    idx += 2;
                } else {
                    long_count += 1;
                    idx += 1;
                }
            }
            _ => {
                return Err(Diagnostic::error(
                    "unsupported integer literal suffix",
                    span,
                ));
            }
        }
    }
    if long_count > 2 {
        return Err(Diagnostic::error(
            "unsupported integer literal suffix",
            span,
        ));
    }

    let candidates = match (decimal_constant, unsigned, long_count) {
        (true, false, 0) => vec![CType::Int, CType::Long, CType::LongLong],
        (false, false, 0) => vec![
            CType::Int,
            CType::UnsignedInt,
            CType::Long,
            CType::UnsignedLong,
            CType::LongLong,
            CType::UnsignedLongLong,
        ],
        (_, true, 0) => vec![
            CType::UnsignedInt,
            CType::UnsignedLong,
            CType::UnsignedLongLong,
        ],
        (true, false, 1) => vec![CType::Long, CType::LongLong],
        (false, false, 1) => vec![
            CType::Long,
            CType::UnsignedLong,
            CType::LongLong,
            CType::UnsignedLongLong,
        ],
        (_, true, 1) => vec![CType::UnsignedLong, CType::UnsignedLongLong],
        (true, false, 2) => vec![CType::LongLong],
        (false, false, 2) => vec![CType::LongLong, CType::UnsignedLongLong],
        (_, true, 2) => vec![CType::UnsignedLongLong],
        _ => unreachable!(),
    };

    for ty in candidates {
        let (_, max) = ty.integer_bounds().unwrap();
        if value <= max as u128 {
            let normalized = ty.normalize_integer_value(value as i128).unwrap();
            return Ok((ty, normalized, has_suffix));
        }
    }

    Err(Diagnostic::error(
        "integer literal is out of supported range",
        span,
    ))
}
