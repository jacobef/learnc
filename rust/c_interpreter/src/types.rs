use std::fmt;

pub const HOST_LONG_DOUBLE_SIZE: usize = if cfg!(all(target_os = "macos", target_arch = "aarch64"))
{
    8
} else {
    16
};

pub const HOST_LONG_DOUBLE_ALIGN: usize = if cfg!(all(target_os = "macos", target_arch = "aarch64"))
{
    8
} else {
    16
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct TypeQualifiers {
    pub is_const: bool,
    pub is_restrict: bool,
    pub is_volatile: bool,
}

impl TypeQualifiers {
    pub fn is_empty(self) -> bool {
        !self.is_const && !self.is_restrict && !self.is_volatile
    }

    pub fn contains(self, other: TypeQualifiers) -> bool {
        (!other.is_const || self.is_const)
            && (!other.is_restrict || self.is_restrict)
            && (!other.is_volatile || self.is_volatile)
    }

    pub fn union(self, other: TypeQualifiers) -> TypeQualifiers {
        TypeQualifiers {
            is_const: self.is_const || other.is_const,
            is_restrict: self.is_restrict || other.is_restrict,
            is_volatile: self.is_volatile || other.is_volatile,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordKind {
    Struct,
    Union,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordMember {
    pub name: Option<String>,
    pub storage_name: String,
    pub ty: CType,
    pub offset: usize,
    pub bit_width: Option<u8>,
    pub bit_offset: u8,
    pub bit_storage_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordType {
    pub id: usize,
    pub kind: RecordKind,
    pub tag: Option<String>,
    pub complete: bool,
    pub members: Vec<RecordMember>,
    pub size: usize,
    pub align: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
    pub id: usize,
    pub tag: Option<String>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CType {
    Void,
    Bool,
    Char,
    SignedChar,
    UnsignedChar,
    Short,
    UnsignedShort,
    Int,
    UnsignedInt,
    Long,
    UnsignedLong,
    LongLong,
    UnsignedLongLong,
    Float,
    Double,
    LongDouble,
    Complex(Box<CType>),
    VaList,
    Struct(usize, Option<String>),
    Union(usize, Option<String>),
    Enum(usize, Option<String>),
    Function(Box<CType>, Vec<CType>, bool),
    Qualified(Box<CType>, TypeQualifiers),
    Pointer(Box<CType>),
    Array(Box<CType>, usize),
}

impl CType {
    pub fn qualified(inner: CType, qualifiers: TypeQualifiers) -> Self {
        if qualifiers.is_empty() {
            inner
        } else {
            CType::Qualified(Box::new(inner), qualifiers)
        }
    }

    pub fn pointer_to(inner: CType) -> Self {
        CType::Pointer(Box::new(inner))
    }

    pub fn function(return_type: CType, params: Vec<CType>) -> Self {
        CType::Function(Box::new(return_type), params, false)
    }

    pub fn variadic_function(return_type: CType, params: Vec<CType>) -> Self {
        CType::Function(Box::new(return_type), params, true)
    }

    pub fn complex_of(real: CType) -> Self {
        CType::Complex(Box::new(real))
    }

    pub fn array_of(inner: CType, len: usize) -> Self {
        CType::Array(Box::new(inner), len)
    }

    pub fn unqualified(&self) -> &CType {
        match self {
            CType::Qualified(inner, _) => inner,
            _ => self,
        }
    }

    pub fn top_level_qualifiers(&self) -> TypeQualifiers {
        match self {
            CType::Qualified(_, qualifiers) => *qualifiers,
            _ => TypeQualifiers::default(),
        }
    }

    pub fn is_const_qualified(&self) -> bool {
        self.top_level_qualifiers().is_const
    }

    pub fn is_volatile_qualified(&self) -> bool {
        self.top_level_qualifiers().is_volatile
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self.unqualified(),
            CType::Bool
                | CType::Char
                | CType::SignedChar
                | CType::UnsignedChar
                | CType::Short
                | CType::UnsignedShort
                | CType::Int
                | CType::UnsignedInt
                | CType::Long
                | CType::UnsignedLong
                | CType::LongLong
                | CType::UnsignedLongLong
                | CType::Enum(_, _)
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self.unqualified(),
            CType::Char
                | CType::SignedChar
                | CType::Short
                | CType::Int
                | CType::Long
                | CType::LongLong
                | CType::Enum(_, _)
        )
    }

    pub fn is_unsigned_integer(&self) -> bool {
        matches!(
            self.unqualified(),
            CType::UnsignedChar
                | CType::UnsignedShort
                | CType::UnsignedInt
                | CType::UnsignedLong
                | CType::UnsignedLongLong
        )
    }

    pub fn is_floating(&self) -> bool {
        matches!(
            self.unqualified(),
            CType::Float | CType::Double | CType::LongDouble
        )
    }

    pub fn is_complex(&self) -> bool {
        matches!(self.unqualified(), CType::Complex(_))
    }

    pub fn complex_component_type(&self) -> Option<&CType> {
        match self.unqualified() {
            CType::Complex(inner) => Some(inner),
            CType::Qualified(_, _) => unreachable!("unqualified() strips top-level qualifiers"),
            _ => None,
        }
    }

    pub fn is_arithmetic(&self) -> bool {
        self.is_integer() || self.is_floating() || self.is_complex()
    }

    pub fn is_character(&self) -> bool {
        matches!(
            self.unqualified(),
            CType::Char | CType::SignedChar | CType::UnsignedChar
        )
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self.unqualified(), CType::Pointer(_))
    }

    pub fn is_function(&self) -> bool {
        matches!(self.unqualified(), CType::Function(_, _, _))
    }

    pub fn element_type(&self) -> Option<&CType> {
        match self.unqualified() {
            CType::Pointer(inner) | CType::Array(inner, _) => Some(inner),
            CType::Qualified(_, _) => unreachable!("unqualified() strips top-level qualifiers"),
            _ => None,
        }
    }

    pub fn size_of(&self) -> Option<usize> {
        match self.unqualified() {
            CType::Void => None,
            CType::Bool => Some(1),
            CType::Char | CType::SignedChar | CType::UnsignedChar => Some(1),
            CType::Short | CType::UnsignedShort => Some(2),
            CType::Int | CType::UnsignedInt => Some(4),
            CType::Enum(_, _) => Some(4),
            CType::Float => Some(4),
            CType::Double => Some(8),
            CType::LongDouble => Some(HOST_LONG_DOUBLE_SIZE),
            CType::Complex(inner) => inner.size_of().map(|size| size * 2),
            CType::VaList => Some(8),
            CType::Long | CType::UnsignedLong | CType::LongLong | CType::UnsignedLongLong => {
                Some(8)
            }
            CType::Struct(_, _) | CType::Union(_, _) => None,
            CType::Function(_, _, _) => None,
            CType::Pointer(_) => Some(8),
            CType::Array(inner, len) => {
                if *len == 0 {
                    None
                } else {
                    inner.size_of().and_then(|size| size.checked_mul(*len))
                }
            }
            CType::Qualified(_, _) => unreachable!("unqualified() strips top-level qualifiers"),
        }
    }

    pub fn integer_bits(&self) -> Option<u32> {
        if self.is_integer() {
            self.size_of().map(|size| (size * 8) as u32)
        } else {
            None
        }
    }

    pub fn integer_rank(&self) -> Option<u8> {
        match self.unqualified() {
            CType::Bool => Some(0),
            CType::Char | CType::SignedChar | CType::UnsignedChar => Some(1),
            CType::Short | CType::UnsignedShort => Some(2),
            CType::Int | CType::UnsignedInt => Some(3),
            CType::Enum(_, _) => Some(3),
            CType::Long | CType::UnsignedLong => Some(4),
            CType::LongLong | CType::UnsignedLongLong => Some(5),
            CType::Qualified(_, _) => unreachable!("unqualified() strips top-level qualifiers"),
            _ => None,
        }
    }

    pub fn integer_bounds(&self) -> Option<(i128, i128)> {
        if matches!(self.unqualified(), CType::Bool) {
            return Some((0, 1));
        }
        let bits = self.integer_bits()?;
        if self.is_signed_integer() {
            let max = (1i128 << (bits - 1)) - 1;
            Some((-(1i128 << (bits - 1)), max))
        } else if self.is_unsigned_integer() {
            Some((0, (1i128 << bits) - 1))
        } else {
            None
        }
    }

    pub fn unsigned_variant(&self) -> Option<CType> {
        let variant = match self.unqualified() {
            CType::Bool => CType::UnsignedInt,
            CType::Char | CType::SignedChar | CType::UnsignedChar => CType::UnsignedChar,
            CType::Short | CType::UnsignedShort => CType::UnsignedShort,
            CType::Int | CType::UnsignedInt => CType::UnsignedInt,
            CType::Enum(_, _) => CType::UnsignedInt,
            CType::Long | CType::UnsignedLong => CType::UnsignedLong,
            CType::LongLong | CType::UnsignedLongLong => CType::UnsignedLongLong,
            CType::Qualified(_, _) => unreachable!("unqualified() strips top-level qualifiers"),
            _ => return None,
        };
        Some(CType::qualified(variant, self.top_level_qualifiers()))
    }

    pub fn normalize_integer_value(&self, value: i128) -> Option<i128> {
        if matches!(self.unqualified(), CType::Bool) {
            return Some((value != 0) as i128);
        }
        let bits = self.integer_bits()?;
        let modulo = 1i128 << bits;
        let wrapped = value.rem_euclid(modulo);
        Some(if self.is_signed_integer() {
            let sign_bit = 1i128 << (bits - 1);
            if wrapped >= sign_bit {
                wrapped - modulo
            } else {
                wrapped
            }
        } else {
            wrapped
        })
    }
}

impl fmt::Display for CType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CType::Void => write!(f, "void"),
            CType::Bool => write!(f, "_Bool"),
            CType::Char => write!(f, "char"),
            CType::SignedChar => write!(f, "signed char"),
            CType::UnsignedChar => write!(f, "unsigned char"),
            CType::Short => write!(f, "short"),
            CType::UnsignedShort => write!(f, "unsigned short"),
            CType::Int => write!(f, "int"),
            CType::UnsignedInt => write!(f, "unsigned int"),
            CType::Long => write!(f, "long"),
            CType::UnsignedLong => write!(f, "unsigned long"),
            CType::LongLong => write!(f, "long long"),
            CType::UnsignedLongLong => write!(f, "unsigned long long"),
            CType::Float => write!(f, "float"),
            CType::Double => write!(f, "double"),
            CType::LongDouble => write!(f, "long double"),
            CType::Complex(inner) => write!(f, "{} _Complex", inner),
            CType::VaList => write!(f, "va_list"),
            CType::Struct(id, tag) => match tag {
                Some(tag) => write!(f, "struct {}", tag),
                None => write!(f, "anonymous struct#{}", id),
            },
            CType::Union(id, tag) => match tag {
                Some(tag) => write!(f, "union {}", tag),
                None => write!(f, "anonymous union#{}", id),
            },
            CType::Enum(id, tag) => match tag {
                Some(tag) => write!(f, "enum {}", tag),
                None => write!(f, "anonymous enum#{}", id),
            },
            CType::Function(return_type, params, is_variadic) => {
                write!(f, "function(")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                if *is_variadic {
                    if !params.is_empty() {
                        write!(f, ", ")?;
                    }
                    write!(f, "...")?;
                }
                write!(f, ") returning {}", return_type)
            }
            CType::Qualified(inner, qualifiers) => {
                if qualifiers.is_const {
                    write!(f, "const ")?;
                }
                if qualifiers.is_restrict {
                    write!(f, "restrict ")?;
                }
                if qualifiers.is_volatile {
                    write!(f, "volatile ")?;
                }
                write!(f, "{}", inner)
            }
            CType::Pointer(inner) => write!(f, "{}*", inner),
            CType::Array(inner, len) => write!(f, "{}[{}]", inner, len),
        }
    }
}
