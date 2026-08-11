use std::ffi::CString;

use libc::c_char;

use crate::diag::Diagnostic;
use crate::integer::parse_integer_literal;
use crate::source::Span;
use crate::types::CType;

unsafe extern "C" {
    fn strtod(nptr: *const c_char, endptr: *mut *mut c_char) -> f64;
}

#[derive(Debug, Clone, Copy)]
pub enum NumberValue {
    Integer(i128),
    Floating(f64),
}

#[derive(Debug, Clone)]
pub struct NumberLiteral {
    pub ty: CType,
    pub value: NumberValue,
    pub has_suffix: bool,
}

pub fn parse_number_literal(text: String, span: Span) -> Result<NumberLiteral, Diagnostic> {
    if is_floating_literal(&text) {
        let (ty, value, has_suffix) = parse_floating_literal(&text, span)?;
        Ok(NumberLiteral {
            ty,
            value: NumberValue::Floating(value),
            has_suffix,
        })
    } else {
        let (ty, value, has_suffix) = parse_integer_literal(&text, span)?;
        Ok(NumberLiteral {
            ty,
            value: NumberValue::Integer(value),
            has_suffix,
        })
    }
}

fn is_floating_literal(text: &str) -> bool {
    let body = text.trim_end_matches(|ch: char| matches!(ch, 'f' | 'F' | 'l' | 'L'));
    if body.starts_with("0x") || body.starts_with("0X") {
        body.contains('.') || body.contains('p') || body.contains('P')
    } else {
        body.contains('.') || body.contains('e') || body.contains('E')
    }
}

fn parse_floating_literal(text: &str, span: Span) -> Result<(CType, f64, bool), Diagnostic> {
    let (body, ty, has_suffix) = match text.chars().last() {
        Some('f' | 'F') => (&text[..text.len() - 1], CType::Float, true),
        Some('l' | 'L') => (&text[..text.len() - 1], CType::LongDouble, true),
        _ => (text, CType::Double, false),
    };
    if (body.starts_with("0x") || body.starts_with("0X")) && !body.contains(['p', 'P']) {
        return Err(Diagnostic::error(
            "hexadecimal floating literal requires a binary exponent",
            span,
        ));
    }
    let c_string = CString::new(body)
        .map_err(|_| Diagnostic::error("floating literal contains an interior NUL byte", span))?;
    let mut end_ptr = std::ptr::null_mut();
    let value = unsafe { strtod(c_string.as_ptr(), &mut end_ptr) };
    if end_ptr != unsafe { c_string.as_ptr().add(body.len()) }.cast_mut() {
        return Err(Diagnostic::error("invalid floating literal", span));
    }
    if !value.is_finite() {
        return Err(Diagnostic::error(
            "floating literal is out of supported range",
            span,
        ));
    }
    let converted = match ty {
        CType::Float => {
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(Diagnostic::error(
                    "floating literal is out of range for float",
                    span,
                ));
            }
            narrowed as f64
        }
        CType::Double | CType::LongDouble => value,
        _ => unreachable!(),
    };
    Ok((ty, converted, has_suffix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FileId;

    fn span() -> Span {
        Span::new(FileId(0), 0, 1)
    }

    #[test]
    fn numeric_literals_are_typed_and_decoded_once() {
        let integer = parse_number_literal("0xffffffff".to_owned(), span()).unwrap();
        assert_eq!(integer.ty, CType::UnsignedInt);
        assert!(matches!(integer.value, NumberValue::Integer(4_294_967_295)));
        assert!(!integer.has_suffix);

        let floating = parse_number_literal("0x1.8p+1f".to_owned(), span()).unwrap();
        assert_eq!(floating.ty, CType::Float);
        assert!(matches!(floating.value, NumberValue::Floating(3.0)));
        assert!(floating.has_suffix);
    }

    #[test]
    fn numeric_literal_diagnostics_are_preserved_during_parsing() {
        let diagnostic = parse_number_literal("0x1.0".to_owned(), span())
            .unwrap_err()
            .render();
        assert!(diagnostic.contains("requires a binary exponent"));
    }
}
