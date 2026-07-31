use crate::diag::Diagnostic;
use crate::source::{FileId, SourceManager, Span};
use crate::token::{Keyword, LiteralUnit, StringLiteralValue, Token, TokenKind};

enum EscapeValue {
    Character(char),
    Numeric(u32),
}

pub struct Lexer<'a> {
    file_id: FileId,
    text: &'a str,
    offset: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(sources: &'a SourceManager, file_id: FileId) -> Self {
        let text = sources.file(file_id).text();
        Self {
            file_id,
            text,
            offset: 0,
        }
    }

    pub fn lex(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            self.skip_ws_and_comments()?;
            if self.offset >= self.text.len() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: Span::new(self.file_id, self.offset, self.offset),
                });
                break;
            }
            tokens.push(self.next_token()?);
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, Diagnostic> {
        let start = self.offset;
        let ch = self.bump().unwrap();
        let kind = match ch {
            'u' if self.peek_char() == Some('8')
                && self.text[self.offset + 1..].starts_with('"') =>
            {
                self.bump();
                self.bump();
                TokenKind::StringLiteral(self.lex_string_literal(start, u8::MAX as u32)?)
            }
            'u' if matches!(self.peek_char(), Some('\'' | '"')) => match self.bump().unwrap() {
                '\'' => {
                    TokenKind::Utf16CharLiteral(self.lex_unicode_char_literal(start, true)? as u16)
                }
                '"' => {
                    TokenKind::Utf16StringLiteral(self.lex_string_literal(start, u16::MAX as u32)?)
                }
                _ => unreachable!(),
            },
            'U' if matches!(self.peek_char(), Some('\'' | '"')) => match self.bump().unwrap() {
                '\'' => TokenKind::Utf32CharLiteral(self.lex_unicode_char_literal(start, false)?),
                '"' => TokenKind::Utf32StringLiteral(self.lex_string_literal(start, u32::MAX)?),
                _ => unreachable!(),
            },
            'L' if matches!(self.peek_char(), Some('\'') | Some('"')) => match self.bump().unwrap()
            {
                '\'' => TokenKind::WideCharLiteral(self.lex_char_literal(start, u32::MAX)?),
                '"' => TokenKind::WideStringLiteral(self.lex_string_literal(start, u32::MAX)?),
                _ => unreachable!("guarded by peek_char"),
            },
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            '.' => {
                if self.peek_char() == Some('.') && self.peek_next_char() == Some('.') {
                    self.bump();
                    self.bump();
                    TokenKind::Ellipsis
                } else if self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
                    self.lex_number(start, true)
                } else {
                    TokenKind::Dot
                }
            }
            ':' => TokenKind::Colon,
            '?' => TokenKind::Question,
            ';' => TokenKind::Semicolon,
            '*' => {
                if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }
            '&' => {
                if self.peek_char() == Some('&') {
                    self.bump();
                    TokenKind::DoubleAmp
                } else if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::AmpEqual
                } else {
                    TokenKind::Amp
                }
            }
            '+' => {
                if self.peek_char() == Some('+') {
                    self.bump();
                    TokenKind::DoublePlus
                } else if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if self.peek_char() == Some('-') {
                    self.bump();
                    TokenKind::DoubleMinus
                } else if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::MinusEqual
                } else if self.peek_char() == Some('>') {
                    self.bump();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '|' => {
                if self.peek_char() == Some('|') {
                    self.bump();
                    TokenKind::DoublePipe
                } else if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::PipeEqual
                } else {
                    TokenKind::Pipe
                }
            }
            '^' => {
                if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::CaretEqual
                } else {
                    TokenKind::Caret
                }
            }
            '/' => {
                if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::PercentEqual
                } else {
                    TokenKind::Percent
                }
            }
            '=' => {
                if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::DoubleEqual
                } else {
                    TokenKind::Equal
                }
            }
            '!' => {
                if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.peek_char() == Some('<') {
                    self.bump();
                    if self.peek_char() == Some('=') {
                        self.bump();
                        TokenKind::LeftShiftEqual
                    } else {
                        TokenKind::LeftShift
                    }
                } else if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                if self.peek_char() == Some('>') {
                    self.bump();
                    if self.peek_char() == Some('=') {
                        self.bump();
                        TokenKind::RightShiftEqual
                    } else {
                        TokenKind::RightShift
                    }
                } else if self.peek_char() == Some('=') {
                    self.bump();
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            '~' => TokenKind::Tilde,
            '\'' => TokenKind::CharLiteral(self.lex_char_literal(start, u8::MAX as u32)?),
            '"' => TokenKind::StringLiteral(self.lex_string_literal(start, u8::MAX as u32)?),
            c if is_ident_start(c) => {
                while self.peek_char().is_some_and(is_ident_continue) {
                    self.bump();
                }
                let text = &self.text[start..self.offset];
                match text {
                    "_Alignas" => TokenKind::Keyword(Keyword::Alignas),
                    "_Alignof" => TokenKind::Keyword(Keyword::Alignof),
                    "auto" => TokenKind::Keyword(Keyword::Auto),
                    "break" => TokenKind::Keyword(Keyword::Break),
                    "_Bool" => TokenKind::Keyword(Keyword::Bool),
                    "case" => TokenKind::Keyword(Keyword::Case),
                    "char" => TokenKind::Keyword(Keyword::Char),
                    "const" => TokenKind::Keyword(Keyword::Const),
                    "_Complex" => TokenKind::Keyword(Keyword::Complex),
                    "continue" => TokenKind::Keyword(Keyword::Continue),
                    "default" => TokenKind::Keyword(Keyword::Default),
                    "double" => TokenKind::Keyword(Keyword::Double),
                    "do" => TokenKind::Keyword(Keyword::Do),
                    "else" => TokenKind::Keyword(Keyword::Else),
                    "enum" => TokenKind::Keyword(Keyword::Enum),
                    "extern" => TokenKind::Keyword(Keyword::Extern),
                    "float" => TokenKind::Keyword(Keyword::Float),
                    "for" => TokenKind::Keyword(Keyword::For),
                    "_Generic" => TokenKind::Keyword(Keyword::Generic),
                    "goto" => TokenKind::Keyword(Keyword::Goto),
                    "if" => TokenKind::Keyword(Keyword::If),
                    "inline" => TokenKind::Keyword(Keyword::Inline),
                    "int" => TokenKind::Keyword(Keyword::Int),
                    "long" => TokenKind::Keyword(Keyword::Long),
                    "_Noreturn" => TokenKind::Keyword(Keyword::Noreturn),
                    "register" => TokenKind::Keyword(Keyword::Register),
                    "restrict" => TokenKind::Keyword(Keyword::Restrict),
                    "return" => TokenKind::Keyword(Keyword::Return),
                    "short" => TokenKind::Keyword(Keyword::Short),
                    "signed" => TokenKind::Keyword(Keyword::Signed),
                    "sizeof" => TokenKind::Keyword(Keyword::Sizeof),
                    "static" => TokenKind::Keyword(Keyword::Static),
                    "_Static_assert" => TokenKind::Keyword(Keyword::StaticAssert),
                    "struct" => TokenKind::Keyword(Keyword::Struct),
                    "switch" => TokenKind::Keyword(Keyword::Switch),
                    "typedef" => TokenKind::Keyword(Keyword::Typedef),
                    "union" => TokenKind::Keyword(Keyword::Union),
                    "unsigned" => TokenKind::Keyword(Keyword::Unsigned),
                    "void" => TokenKind::Keyword(Keyword::Void),
                    "volatile" => TokenKind::Keyword(Keyword::Volatile),
                    "while" => TokenKind::Keyword(Keyword::While),
                    _ => TokenKind::Identifier(text.to_owned()),
                }
            }
            c if c.is_ascii_digit() => self.lex_number(start, false),
            _ => {
                return Err(Diagnostic::error(
                    format!("unexpected character {:?}", ch),
                    Span::new(self.file_id, start, self.offset),
                ));
            }
        };
        Ok(Token {
            kind,
            span: Span::new(self.file_id, start, self.offset),
        })
    }

    fn lex_char_literal(&mut self, start: usize, numeric_max: u32) -> Result<i64, Diagnostic> {
        let mut saw_char = false;
        let mut value = 0i64;
        loop {
            let ch = self.bump().ok_or_else(|| {
                Diagnostic::error(
                    "unterminated character constant",
                    Span::new(self.file_id, start, self.offset),
                )
            })?;
            if ch == '\'' {
                if !saw_char {
                    return Err(Diagnostic::error(
                        "empty character constant",
                        Span::new(self.file_id, start, self.offset),
                    ));
                }
                break;
            }
            let unit = if ch == '\\' {
                self.lex_char_escape(start, numeric_max)?
            } else {
                ch as i64
            };
            saw_char = true;
            value = (value << 8) | (unit & 0xff);
        }
        Ok(value)
    }

    fn lex_unicode_char_literal(&mut self, start: usize, utf16: bool) -> Result<u32, Diagnostic> {
        let ch = self.bump().ok_or_else(|| {
            Diagnostic::error(
                "unterminated character constant",
                Span::new(self.file_id, start, self.offset),
            )
        })?;
        if ch == '\'' {
            return Err(Diagnostic::error(
                "empty character constant",
                Span::new(self.file_id, start, self.offset),
            ));
        }
        let value = if ch == '\\' {
            match self.lex_escape_value(start)? {
                EscapeValue::Character(ch) => ch as u32,
                EscapeValue::Numeric(value) => {
                    let maximum = if utf16 { u16::MAX as u32 } else { u32::MAX };
                    self.require_numeric_escape_range(value, maximum, start)?;
                    value
                }
            }
        } else {
            ch as u32
        };
        if self.bump() != Some('\'') {
            return Err(Diagnostic::error(
                "u and U character constants must contain exactly one character",
                Span::new(self.file_id, start, self.offset),
            ));
        }
        if value > 0x10ffff || (0xd800..=0xdfff).contains(&value) || (utf16 && value > 0xffff) {
            return Err(Diagnostic::error(
                "character is not representable in the literal's character type",
                Span::new(self.file_id, start, self.offset),
            ));
        }
        Ok(value)
    }

    fn lex_char_escape(&mut self, start: usize, numeric_max: u32) -> Result<i64, Diagnostic> {
        let value = match self.lex_escape_value(start)? {
            EscapeValue::Character(ch) => ch as u32,
            EscapeValue::Numeric(value) => {
                self.require_numeric_escape_range(value, numeric_max, start)?;
                value
            }
        };
        Ok(value as i64)
    }

    fn lex_escape_value(&mut self, start: usize) -> Result<EscapeValue, Diagnostic> {
        let escaped = self.bump().ok_or_else(|| {
            Diagnostic::error(
                "unterminated escape sequence",
                Span::new(self.file_id, start, self.offset),
            )
        })?;
        let value = match escaped {
            'a' => 0x07,
            'b' => 0x08,
            'f' => 0x0c,
            'n' => '\n' as u32,
            'r' => '\r' as u32,
            't' => '\t' as u32,
            'v' => 0x0b,
            '\\' => '\\' as u32,
            '\'' => '\'' as u32,
            '"' => '"' as u32,
            '?' => '?' as u32,
            'x' => {
                let mut value = 0u32;
                let mut saw_digit = false;
                while let Some(next) = self.peek_char() {
                    if let Some(digit) = next.to_digit(16) {
                        saw_digit = true;
                        self.bump();
                        value = value
                            .checked_mul(16)
                            .and_then(|value| value.checked_add(digit))
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "hexadecimal escape sequence is out of range",
                                    Span::new(self.file_id, start, self.offset),
                                )
                            })?;
                    } else {
                        break;
                    }
                }
                if !saw_digit {
                    return Err(Diagnostic::error(
                        "\\x escape sequence requires at least one hexadecimal digit",
                        Span::new(self.file_id, start, self.offset),
                    ));
                }
                value
            }
            'u' => {
                let value = self.lex_fixed_hex_escape(start, 4, "\\u")?;
                return self.universal_escape_value(value, start);
            }
            'U' => {
                let value = self.lex_fixed_hex_escape(start, 8, "\\U")?;
                return self.universal_escape_value(value, start);
            }
            '0'..='7' => {
                let mut value = (escaped as u8 - b'0') as u32;
                for _ in 0..2 {
                    let Some(next) = self.peek_char() else {
                        break;
                    };
                    if ('0'..='7').contains(&next) {
                        self.bump();
                        value = (value << 3) | ((next as u8 - b'0') as u32);
                    } else {
                        break;
                    }
                }
                value
            }
            other => {
                return Err(Diagnostic::error(
                    format!("unknown escape sequence \\{other}"),
                    Span::new(self.file_id, start, self.offset),
                ));
            }
        };
        Ok(EscapeValue::Numeric(value))
    }

    fn universal_escape_value(&self, value: u32, start: usize) -> Result<EscapeValue, Diagnostic> {
        if (value < 0xa0 && !matches!(value, 0x24 | 0x40 | 0x60))
            || (0xd800..=0xdfff).contains(&value)
            || value > 0x10ffff
        {
            return Err(Diagnostic::error(
                "invalid universal character name",
                Span::new(self.file_id, start, self.offset),
            ));
        }
        Ok(EscapeValue::Character(
            char::from_u32(value).expect("validated Unicode scalar value"),
        ))
    }

    fn require_numeric_escape_range(
        &self,
        value: u32,
        maximum: u32,
        start: usize,
    ) -> Result<(), Diagnostic> {
        if value > maximum {
            return Err(Diagnostic::error(
                "numeric escape sequence is outside the range of the literal's character type",
                Span::new(self.file_id, start, self.offset),
            ));
        }
        Ok(())
    }

    fn lex_fixed_hex_escape(
        &mut self,
        start: usize,
        digits: usize,
        prefix: &str,
    ) -> Result<u32, Diagnostic> {
        let mut value = 0u32;
        for _ in 0..digits {
            let next = self.bump().ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "{prefix} escape sequence requires exactly {digits} hexadecimal digits"
                    ),
                    Span::new(self.file_id, start, self.offset),
                )
            })?;
            let digit = next.to_digit(16).ok_or_else(|| {
                Diagnostic::error(
                    format!(
                        "{prefix} escape sequence requires exactly {digits} hexadecimal digits"
                    ),
                    Span::new(self.file_id, start, self.offset),
                )
            })?;
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn lex_string_literal(
        &mut self,
        start: usize,
        numeric_max: u32,
    ) -> Result<StringLiteralValue, Diagnostic> {
        let mut value = StringLiteralValue::new();
        loop {
            let Some(next) = self.bump() else {
                return Err(Diagnostic::error(
                    "unterminated string literal",
                    Span::new(self.file_id, start, self.offset),
                ));
            };
            match next {
                '"' => break,
                '\\' => match self.lex_escape_value(start)? {
                    EscapeValue::Character(ch) => value.units.push(LiteralUnit::Character(ch)),
                    EscapeValue::Numeric(unit) => {
                        self.require_numeric_escape_range(unit, numeric_max, start)?;
                        value.units.push(LiteralUnit::NumericEscape(unit));
                    }
                },
                other => value.units.push(LiteralUnit::Character(other)),
            }
        }
        Ok(value)
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), Diagnostic> {
        loop {
            while self.peek_char().is_some_and(char::is_whitespace) {
                self.bump();
            }
            let Some('/') = self.peek_char() else {
                return Ok(());
            };
            if self.peek_next_char() == Some('/') {
                self.bump();
                self.bump();
                while self.peek_char().is_some_and(|c| c != '\n') {
                    self.bump();
                }
                continue;
            }
            if self.peek_next_char() == Some('*') {
                let start = self.offset;
                self.bump();
                self.bump();
                loop {
                    match (self.peek_char(), self.peek_next_char()) {
                        (Some('*'), Some('/')) => {
                            self.bump();
                            self.bump();
                            break;
                        }
                        (Some(_), _) => {
                            self.bump();
                        }
                        (None, _) => {
                            return Err(Diagnostic::error(
                                "unterminated block comment",
                                Span::new(self.file_id, start, self.offset),
                            ));
                        }
                    }
                }
                continue;
            }
            return Ok(());
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text[self.offset..].chars().next()
    }

    fn lex_number(&mut self, start: usize, started_with_dot: bool) -> TokenKind {
        if started_with_dot {
            while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                self.bump();
            }
            if matches!(self.peek_char(), Some('e' | 'E')) {
                self.bump();
                if matches!(self.peek_char(), Some('+' | '-')) {
                    self.bump();
                }
                while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.bump();
                }
            }
        } else if self.text[start..self.offset].starts_with("0")
            && matches!(self.peek_char(), Some('x' | 'X'))
        {
            self.bump();
            while self.peek_char().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                self.bump();
            }
            if self.peek_char() == Some('.') {
                self.bump();
                while self.peek_char().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                    self.bump();
                }
            }
            if matches!(self.peek_char(), Some('p' | 'P')) {
                self.bump();
                if matches!(self.peek_char(), Some('+' | '-')) {
                    self.bump();
                }
                while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.bump();
                }
            }
        } else {
            while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                self.bump();
            }
            if self.peek_char() == Some('.') {
                self.bump();
                while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.bump();
                }
            }
            if matches!(self.peek_char(), Some('e' | 'E')) {
                self.bump();
                if matches!(self.peek_char(), Some('+' | '-')) {
                    self.bump();
                }
                while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.bump();
                }
            }
        }
        while self.peek_char().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            self.bump();
        }
        TokenKind::Number(self.text[start..self.offset].to_owned())
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut chars = self.text[self.offset..].chars();
        chars.next()?;
        chars.next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}
