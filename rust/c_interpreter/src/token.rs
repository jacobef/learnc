use crate::source::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralUnit {
    Character(char),
    NumericEscape(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteralValue {
    pub units: Vec<LiteralUnit>,
}

impl StringLiteralValue {
    pub fn new() -> Self {
        Self { units: Vec::new() }
    }

    pub fn append(&mut self, mut other: Self) {
        self.units.append(&mut other.units);
    }

    pub fn narrow_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for unit in &self.units {
            match unit {
                LiteralUnit::Character(ch) => {
                    let mut encoded = [0; 4];
                    bytes.extend_from_slice(ch.encode_utf8(&mut encoded).as_bytes());
                }
                LiteralUnit::NumericEscape(value) => bytes.push(*value as u8),
            }
        }
        bytes
    }

    pub fn narrow_len(&self) -> usize {
        self.narrow_bytes().len()
    }

    pub fn utf16_units(&self) -> Vec<u16> {
        let mut units = Vec::new();
        for unit in &self.units {
            match unit {
                LiteralUnit::Character(ch) => {
                    let mut encoded = [0; 2];
                    units.extend_from_slice(ch.encode_utf16(&mut encoded));
                }
                LiteralUnit::NumericEscape(value) => units.push(*value as u16),
            }
        }
        units
    }

    pub fn utf32_units(&self) -> Vec<u32> {
        self.units
            .iter()
            .map(|unit| match unit {
                LiteralUnit::Character(ch) => *ch as u32,
                LiteralUnit::NumericEscape(value) => *value,
            })
            .collect()
    }
}

impl fmt::Display for StringLiteralValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for unit in &self.units {
            match unit {
                LiteralUnit::Character(ch) => formatter.write_str(&ch.to_string())?,
                LiteralUnit::NumericEscape(value) => write!(formatter, "\\x{value:x}")?,
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Alignas,
    Alignof,
    Atomic,
    Auto,
    Bool,
    Break,
    Case,
    Char,
    Const,
    Complex,
    Continue,
    Default,
    Double,
    Do,
    Else,
    Enum,
    Extern,
    Float,
    For,
    Generic,
    Goto,
    If,
    Inline,
    Imaginary,
    Int,
    Long,
    Noreturn,
    Register,
    Restrict,
    Return,
    Short,
    Signed,
    Sizeof,
    Static,
    StaticAssert,
    Struct,
    Switch,
    Typedef,
    ThreadLocal,
    Union,
    Unsigned,
    Void,
    Volatile,
    While,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    Number(String),
    CharLiteral(i64),
    WideCharLiteral(i64),
    Utf16CharLiteral(u16),
    Utf32CharLiteral(u32),
    StringLiteral(StringLiteralValue),
    Utf8StringLiteral(StringLiteralValue),
    WideStringLiteral(StringLiteralValue),
    Utf16StringLiteral(StringLiteralValue),
    Utf32StringLiteral(StringLiteralValue),
    Keyword(Keyword),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Ellipsis,
    Semicolon,
    Star,
    StarEqual,
    Amp,
    DoubleAmp,
    AmpEqual,
    Caret,
    CaretEqual,
    Colon,
    Equal,
    DoubleEqual,
    Bang,
    BangEqual,
    Plus,
    PlusEqual,
    DoublePlus,
    Minus,
    MinusEqual,
    DoubleMinus,
    Arrow,
    Pipe,
    PipeEqual,
    DoublePipe,
    Slash,
    SlashEqual,
    Percent,
    PercentEqual,
    Question,
    Less,
    LessEqual,
    LeftShift,
    LeftShiftEqual,
    Greater,
    GreaterEqual,
    RightShift,
    RightShiftEqual,
    Tilde,
    Eof,
}

impl TokenKind {
    pub fn diagnostic_name(&self) -> String {
        let punctuation = match self {
            Self::LParen => Some("("),
            Self::RParen => Some(")"),
            Self::LBrace => Some("{"),
            Self::RBrace => Some("}"),
            Self::LBracket => Some("["),
            Self::RBracket => Some("]"),
            Self::Comma => Some(","),
            Self::Dot => Some("."),
            Self::Ellipsis => Some("..."),
            Self::Semicolon => Some(";"),
            Self::Star => Some("*"),
            Self::StarEqual => Some("*="),
            Self::Amp => Some("&"),
            Self::DoubleAmp => Some("&&"),
            Self::AmpEqual => Some("&="),
            Self::Caret => Some("^"),
            Self::CaretEqual => Some("^="),
            Self::Colon => Some(":"),
            Self::Equal => Some("="),
            Self::DoubleEqual => Some("=="),
            Self::Bang => Some("!"),
            Self::BangEqual => Some("!="),
            Self::Plus => Some("+"),
            Self::PlusEqual => Some("+="),
            Self::DoublePlus => Some("++"),
            Self::Minus => Some("-"),
            Self::MinusEqual => Some("-="),
            Self::DoubleMinus => Some("--"),
            Self::Arrow => Some("->"),
            Self::Pipe => Some("|"),
            Self::PipeEqual => Some("|="),
            Self::DoublePipe => Some("||"),
            Self::Slash => Some("/"),
            Self::SlashEqual => Some("/="),
            Self::Percent => Some("%"),
            Self::PercentEqual => Some("%="),
            Self::Question => Some("?"),
            Self::Less => Some("<"),
            Self::LessEqual => Some("<="),
            Self::LeftShift => Some("<<"),
            Self::LeftShiftEqual => Some("<<="),
            Self::Greater => Some(">"),
            Self::GreaterEqual => Some(">="),
            Self::RightShift => Some(">>"),
            Self::RightShiftEqual => Some(">>="),
            Self::Tilde => Some("~"),
            _ => None,
        };
        if let Some(punctuation) = punctuation {
            return format!("'{punctuation}'");
        }
        match self {
            Self::Identifier(name) => format!("identifier '{name}'"),
            Self::Number(text) => format!("number '{text}'"),
            Self::CharLiteral(..)
            | Self::WideCharLiteral(..)
            | Self::Utf16CharLiteral(..)
            | Self::Utf32CharLiteral(..) => "a character literal".to_owned(),
            Self::StringLiteral(..)
            | Self::Utf8StringLiteral(..)
            | Self::WideStringLiteral(..)
            | Self::Utf16StringLiteral(..)
            | Self::Utf32StringLiteral(..) => "a string literal".to_owned(),
            Self::Keyword(keyword) => format!("keyword '{}'", keyword.spelling()),
            Self::Eof => "the end of the file".to_owned(),
            _ => unreachable!("punctuation token was handled above"),
        }
    }
}

impl Keyword {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Alignas => "_Alignas",
            Self::Alignof => "_Alignof",
            Self::Atomic => "_Atomic",
            Self::Auto => "auto",
            Self::Bool => "_Bool",
            Self::Break => "break",
            Self::Case => "case",
            Self::Char => "char",
            Self::Const => "const",
            Self::Complex => "_Complex",
            Self::Continue => "continue",
            Self::Default => "default",
            Self::Double => "double",
            Self::Do => "do",
            Self::Else => "else",
            Self::Enum => "enum",
            Self::Extern => "extern",
            Self::Float => "float",
            Self::For => "for",
            Self::Generic => "_Generic",
            Self::Goto => "goto",
            Self::If => "if",
            Self::Inline => "inline",
            Self::Imaginary => "_Imaginary",
            Self::Int => "int",
            Self::Long => "long",
            Self::Noreturn => "_Noreturn",
            Self::Register => "register",
            Self::Restrict => "restrict",
            Self::Return => "return",
            Self::Short => "short",
            Self::Signed => "signed",
            Self::Sizeof => "sizeof",
            Self::Static => "static",
            Self::StaticAssert => "_Static_assert",
            Self::Struct => "struct",
            Self::Switch => "switch",
            Self::Typedef => "typedef",
            Self::ThreadLocal => "_Thread_local",
            Self::Union => "union",
            Self::Unsigned => "unsigned",
            Self::Void => "void",
            Self::Volatile => "volatile",
            Self::While => "while",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
