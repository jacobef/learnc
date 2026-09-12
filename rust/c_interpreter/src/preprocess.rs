mod expansion;

use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::fs;
use std::mem;
use std::path::{Component, Path, PathBuf};

use crate::diag::Diagnostic;
use crate::integer::parse_integer_literal as parse_c_integer_literal;
use crate::source::{FileId, LineOrigin, SourceManager, Span};

const HIDDEN_STDIO_H: &str = include_str!("../include/stdio.h");
const HIDDEN_STDLIB_H: &str = include_str!("../include/stdlib.h");
const HIDDEN_STRING_H: &str = include_str!("../include/string.h");
const HIDDEN_STDARG_H: &str = include_str!("../include/stdarg.h");
const HIDDEN_MATH_H: &str = include_str!("../include/math.h");
const HIDDEN_COMPLEX_H: &str = include_str!("../include/complex.h");
const HIDDEN_TGMATH_H: &str = include_str!("../include/tgmath.h");
const HIDDEN_WCHAR_H: &str = include_str!("../include/wchar.h");
const HIDDEN_WCTYPE_H: &str = include_str!("../include/wctype.h");
const HIDDEN_STDDEF_H: &str = include_str!("../include/stddef.h");
const HIDDEN_STDINT_H: &str = include_str!("../include/stdint.h");
const HIDDEN_SETJMP_H: &str = include_str!("../include/setjmp.h");
const HIDDEN_ASSERT_H: &str = include_str!("../include/assert.h");
const HIDDEN_CTYPE_H: &str = include_str!("../include/ctype.h");
const HIDDEN_ERRNO_H: &str = include_str!("../include/errno.h");
const HIDDEN_FLOAT_H: &str = include_str!("../include/float.h");
const HIDDEN_ISO646_H: &str = include_str!("../include/iso646.h");
const HIDDEN_LIMITS_H: &str = include_str!("../include/limits.h");
const HIDDEN_STDBOOL_H: &str = include_str!("../include/stdbool.h");
const HIDDEN_LOCALE_H: &str = include_str!("../include/locale.h");
const HIDDEN_SIGNAL_H: &str = include_str!("../include/signal.h");
const HIDDEN_INTTYPES_H: &str = include_str!("../include/inttypes.h");
const HIDDEN_TIME_H: &str = include_str!("../include/time.h");
const HIDDEN_FENV_H: &str = include_str!("../include/fenv.h");
const HIDDEN_STDALIGN_H: &str = include_str!("../include/stdalign.h");
const HIDDEN_STDNORETURN_H: &str = include_str!("../include/stdnoreturn.h");
const HIDDEN_UCHAR_H: &str = include_str!("../include/uchar.h");

pub struct Preprocessed {
    pub file_id: FileId,
}

#[derive(Default)]
struct ExpandedFile {
    text: String,
    line_origins: Vec<LineOrigin>,
}

impl ExpandedFile {
    fn push_line(&mut self, text: &str, file: FileId, line_number: usize) {
        self.line_origins.push(LineOrigin { file, line_number });
        self.text.push_str(text);
        self.text.push('\n');
    }

    fn append(&mut self, nested: ExpandedFile) {
        self.text.push_str(&nested.text);
        self.line_origins.extend(nested.line_origins);
    }
}

pub struct Preprocessor {
    #[cfg(test)]
    include_root: PathBuf,
    #[cfg(test)]
    include_dirs: Vec<PathBuf>,
}

#[derive(Clone)]
enum MacroDefinition {
    Object(String),
    Function {
        params: Vec<String>,
        variadic: bool,
        replacement: String,
    },
}

struct ConditionalFrame {
    parent_active: bool,
    branch_taken: bool,
    currently_active: bool,
    saw_else: bool,
    opening_span: Span,
}

struct PresumedLocation {
    file_name: String,
    line_delta: isize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpansionMode {
    Normal,
    IfExpression,
}

#[derive(Clone, Debug)]
enum IfToken {
    Number(PpInt),
    LParen,
    RParen,
    Question,
    Colon,
    OrOr,
    AndAnd,
    Pipe,
    Caret,
    Amp,
    EqEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Shl,
    Shr,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Tilde,
    End,
}

#[derive(Clone, Copy, Debug)]
struct PpInt {
    bits: u64,
    unsigned: bool,
}

impl PpInt {
    fn signed(value: i64) -> Self {
        Self {
            bits: value as u64,
            unsigned: false,
        }
    }

    fn unsigned(value: u64) -> Self {
        Self {
            bits: value,
            unsigned: true,
        }
    }

    fn truthy(self) -> bool {
        self.bits != 0
    }

    fn with_common_type(self, other: Self) -> Self {
        Self {
            bits: self.bits,
            unsigned: self.unsigned || other.unsigned,
        }
    }
}

impl Preprocessor {
    #[cfg(not(test))]
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(test)]
    pub fn new(include_root: PathBuf, include_dirs: Vec<PathBuf>) -> Self {
        Self {
            include_root,
            include_dirs,
        }
    }

    pub fn preprocess(
        &self,
        sources: &mut SourceManager,
        root_file: FileId,
    ) -> Result<Preprocessed, Diagnostic> {
        let root_path = sources.file(root_file).path().clone();
        let mut macros = predefined_macros();
        let expanded = self.expand_file(sources, root_file, &root_path, &mut macros)?;
        let file_id = sources.add_generated_file(root_path, expanded.text, expanded.line_origins);
        Ok(Preprocessed { file_id })
    }

    fn expand_file(
        &self,
        sources: &mut SourceManager,
        file_id: FileId,
        path: &Path,
        macros: &mut HashMap<String, MacroDefinition>,
    ) -> Result<ExpandedFile, Diagnostic> {
        let (text, physical_line_map) =
            self.normalize_source_text(sources.file(file_id).text(), file_id)?;
        let mut out = ExpandedFile::default();
        let mut conditionals = Vec::new();
        let mut presumed = PresumedLocation {
            file_name: path.display().to_string(),
            line_delta: 0,
        };

        let lines = text.lines().collect::<Vec<_>>();
        let mut line_idx = 0usize;
        while line_idx < lines.len() {
            let line_number = Self::physical_line_number(&physical_line_map, line_idx);
            let line = lines[line_idx];
            let trimmed = line.trim_start();
            let first_token = line.len().saturating_sub(trimmed.len());
            let line_span =
                sources.span_on_line(file_id, line_number, first_token, line.len() - first_token);
            if let Some(rest) = trimmed
                .strip_prefix('#')
                .or_else(|| trimmed.strip_prefix("%:"))
            {
                self.handle_directive(
                    rest.trim_start(),
                    sources,
                    file_id,
                    path,
                    line_number,
                    macros,
                    &mut conditionals,
                    &mut presumed,
                    &mut out,
                    line_span,
                )
                .map_err(|diag| diag.replace_placeholder_span(line_span))?;
                out.push_line("", file_id, line_number);
                line_idx += 1;
                continue;
            }

            if self.is_active(&conditionals) {
                let presumed_line = Self::presumed_line_number(line_number, presumed.line_delta);
                let mut expanded_source = line.to_owned();
                let mut end_line_idx = line_idx;
                loop {
                    match self
                        .expand_macros_in_line(
                            &expanded_source,
                            macros,
                            &mut HashSet::new(),
                            ExpansionMode::Normal,
                            file_id,
                            presumed_line,
                            &presumed.file_name,
                        )
                        .map_err(|diag| diag.replace_placeholder_span(line_span))
                    {
                        Ok(expanded) => {
                            if end_line_idx + 1 < lines.len()
                                && trailing_function_macro(&expanded, macros).is_some()
                            {
                                end_line_idx += 1;
                                expanded_source.push('\n');
                                expanded_source.push_str(lines[end_line_idx]);
                                continue;
                            }
                            let expanded = canonicalize_digraphs_outside_literals(&expanded);
                            let expanded = separate_macro_generated_comment_openers(&expanded);
                            for (offset, expanded_line) in expanded.split('\n').enumerate() {
                                out.push_line(
                                    expanded_line,
                                    file_id,
                                    Self::physical_line_number(
                                        &physical_line_map,
                                        line_idx + offset,
                                    ),
                                );
                            }
                            break;
                        }
                        Err(diag)
                            if (self.is_unterminated_macro_invocation(&diag)
                                || self.is_unterminated_pragma_operator(&diag))
                                && end_line_idx + 1 < lines.len() =>
                        {
                            end_line_idx += 1;
                            expanded_source.push('\n');
                            expanded_source.push_str(lines[end_line_idx]);
                        }
                        Err(diag) => return Err(diag),
                    }
                }
                line_idx = end_line_idx;
            } else {
                out.push_line("", file_id, line_number);
            }
            line_idx += 1;
        }

        if !conditionals.is_empty() {
            let opening_span = conditionals
                .last()
                .map(|frame| frame.opening_span)
                .unwrap_or_else(|| Self::span(file_id));
            return Err(Diagnostic::error(
                format!("unterminated conditional directive in {}", path.display()),
                opening_span,
            ));
        }

        Ok(out)
    }

    fn physical_line_number(line_map: &[usize], normalized_line_index: usize) -> usize {
        line_map
            .get(normalized_line_index)
            .copied()
            .unwrap_or_else(|| {
                line_map.last().copied().unwrap_or(1)
                    + normalized_line_index
                        .saturating_add(1)
                        .saturating_sub(line_map.len())
            })
    }

    fn is_unterminated_macro_invocation(&self, diag: &Diagnostic) -> bool {
        diag.message().starts_with("unterminated macro invocation")
    }

    fn is_unterminated_pragma_operator(&self, diag: &Diagnostic) -> bool {
        diag.message().starts_with("unterminated _Pragma operator")
    }

    fn normalize_source_text(
        &self,
        text: &str,
        file_id: FileId,
    ) -> Result<(String, Vec<usize>), Diagnostic> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mode {
            Normal,
            String,
            Char,
            LineComment,
            BlockComment,
        }

        if !text.is_empty() && !text.ends_with('\n') {
            return Err(Diagnostic::error(
                "nonempty source file must end in a newline character",
                Span::new(file_id, text.len(), text.len()),
            ));
        }

        let trigraphs = replace_trigraphs(text);
        // Phase 2 line splicing precedes phase 3 preprocessing-token
        // recognition.  In particular, a splice may join the digits of a
        // universal character name, so UCNs cannot be canonicalized first.
        let (spliced, spliced_line_map) = splice_source_lines(&trigraphs);
        let translated = canonicalize_universal_characters(&spliced, file_id)?;
        let bytes = translated.as_bytes();
        let mut idx = 0usize;
        let mut out = String::with_capacity(text.len());
        let mut mode = Mode::Normal;
        let mut construct_start = None;
        // Maps each line of the normalized text (1-based) to the physical line
        // of the original file it came from. Line splices join two physical
        // lines, so counting normalized lines alone would shift every later
        // diagnostic to an earlier physical line.

        let mut source_line_index = 0usize;
        let mut line_map = vec![spliced_line_map[0]];

        while idx < bytes.len() {
            match mode {
                Mode::Normal => {
                    let ch = bytes[idx] as char;
                    if ch == '/' && idx + 1 < bytes.len() {
                        match bytes[idx + 1] as char {
                            '/' => {
                                out.push(' ');
                                idx += 2;
                                mode = Mode::LineComment;
                                continue;
                            }
                            '*' => {
                                out.push(' ');
                                construct_start = Some(idx);
                                idx += 2;
                                mode = Mode::BlockComment;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    out.push(ch);
                    idx += 1;
                    if ch == '\n' {
                        source_line_index += 1;
                        line_map.push(spliced_line_map[source_line_index]);
                    } else if ch == '"' {
                        construct_start = Some(idx - 1);
                        mode = Mode::String;
                    } else if ch == '\'' {
                        construct_start = Some(idx - 1);
                        mode = Mode::Char;
                    }
                }
                Mode::String | Mode::Char => {
                    let ch = bytes[idx] as char;
                    if ch == '\n' || ch == '\r' {
                        let start = construct_start.unwrap_or(idx);
                        return Err(Diagnostic::error(
                            "unterminated quoted literal during preprocessing",
                            Span::new(file_id, start, idx),
                        ));
                    }
                    out.push(ch);
                    idx += 1;
                    if ch == '\\' && idx < bytes.len() {
                        out.push(bytes[idx] as char);
                        idx += 1;
                        continue;
                    }
                    if (mode == Mode::String && ch == '"') || (mode == Mode::Char && ch == '\'') {
                        mode = Mode::Normal;
                        construct_start = None;
                    }
                }
                Mode::LineComment => {
                    let ch = bytes[idx] as char;
                    idx += 1;
                    if ch == '\r' {
                        out.push('\r');
                        if idx < bytes.len() && bytes[idx] as char == '\n' {
                            out.push('\n');
                            idx += 1;
                            source_line_index += 1;
                            line_map.push(spliced_line_map[source_line_index]);
                        }
                        mode = Mode::Normal;
                    } else if ch == '\n' {
                        out.push('\n');
                        source_line_index += 1;
                        line_map.push(spliced_line_map[source_line_index]);
                        mode = Mode::Normal;
                    }
                }
                Mode::BlockComment => {
                    if idx + 1 < bytes.len()
                        && bytes[idx] as char == '*'
                        && bytes[idx + 1] as char == '/'
                    {
                        idx += 2;
                        mode = Mode::Normal;
                        construct_start = None;
                        continue;
                    }
                    let ch = bytes[idx] as char;
                    idx += 1;
                    if ch == '\r' {
                        if idx < bytes.len() && bytes[idx] as char == '\n' {
                            idx += 1;
                            source_line_index += 1;
                        }
                    } else if ch == '\n' {
                        source_line_index += 1;
                    }
                }
            }
        }

        if mode == Mode::BlockComment {
            let start = construct_start.unwrap_or(text.len());
            return Err(Diagnostic::error(
                "unterminated block comment during preprocessing",
                Span::new(file_id, start, start.saturating_add(2)),
            ));
        }

        Ok((out, line_map))
    }

    fn handle_directive(
        &self,
        directive: &str,
        sources: &mut SourceManager,
        file_id: FileId,
        path: &Path,
        line_number: usize,
        macros: &mut HashMap<String, MacroDefinition>,
        conditionals: &mut Vec<ConditionalFrame>,
        presumed: &mut PresumedLocation,
        out: &mut ExpandedFile,
        directive_span: Span,
    ) -> Result<(), Diagnostic> {
        let (keyword, rest) = split_directive(directive);
        let active = self.is_active(conditionals);

        match keyword {
            "ifdef" => {
                let condition = if active {
                    let Some(name) = parse_identifier(rest) else {
                        return Err(Diagnostic::error(
                            "expected macro name after #ifdef",
                            Self::span(file_id),
                        ));
                    };
                    macros.contains_key(name)
                } else {
                    false
                };
                conditionals.push(ConditionalFrame {
                    parent_active: active,
                    branch_taken: condition,
                    currently_active: condition,
                    saw_else: false,
                    opening_span: directive_span,
                });
                Ok(())
            }
            "ifndef" => {
                let condition = if active {
                    let Some(name) = parse_identifier(rest) else {
                        return Err(Diagnostic::error(
                            "expected macro name after #ifndef",
                            Self::span(file_id),
                        ));
                    };
                    !macros.contains_key(name)
                } else {
                    false
                };
                conditionals.push(ConditionalFrame {
                    parent_active: active,
                    branch_taken: condition,
                    currently_active: condition,
                    saw_else: false,
                    opening_span: directive_span,
                });
                Ok(())
            }
            "if" => {
                let condition = if active {
                    self.evaluate_if_expression(rest, macros, file_id)?
                } else {
                    false
                };
                conditionals.push(ConditionalFrame {
                    parent_active: active,
                    branch_taken: condition,
                    currently_active: active && condition,
                    saw_else: false,
                    opening_span: directive_span,
                });
                Ok(())
            }
            "elif" => {
                let Some(frame) = conditionals.last_mut() else {
                    return Err(Diagnostic::error(
                        format!(
                            "#elif without a matching conditional group on line {} in {}",
                            line_number,
                            path.display()
                        ),
                        Self::span(file_id),
                    ));
                };
                if frame.saw_else {
                    return Err(Diagnostic::error(
                        "#elif cannot appear after #else",
                        Self::span(file_id),
                    ));
                }
                if !frame.parent_active || frame.branch_taken {
                    frame.currently_active = false;
                    return Ok(());
                }
                let condition = self.evaluate_if_expression(rest, macros, file_id)?;
                frame.currently_active = condition;
                frame.branch_taken = condition;
                Ok(())
            }
            "else" => {
                if active && !rest.trim().is_empty() {
                    return Err(Diagnostic::error(
                        "unexpected tokens after #else",
                        Self::span(file_id),
                    ));
                }
                let Some(frame) = conditionals.last_mut() else {
                    return Err(Diagnostic::error(
                        format!(
                            "#else without a matching conditional group on line {} in {}",
                            line_number,
                            path.display()
                        ),
                        Self::span(file_id),
                    ));
                };
                if frame.saw_else {
                    return Err(Diagnostic::error(
                        "duplicate #else in conditional group",
                        Self::span(file_id),
                    ));
                }
                frame.currently_active = frame.parent_active && !frame.branch_taken;
                frame.branch_taken = true;
                frame.saw_else = true;
                Ok(())
            }
            "endif" => {
                if active && !rest.trim().is_empty() {
                    return Err(Diagnostic::error(
                        "unexpected tokens after #endif",
                        Self::span(file_id),
                    ));
                }
                if conditionals.pop().is_none() {
                    return Err(Diagnostic::error(
                        format!(
                            "#endif without a matching conditional group on line {} in {}",
                            line_number,
                            path.display()
                        ),
                        Self::span(file_id),
                    ));
                }
                Ok(())
            }
            "" if active => Ok(()),
            "pragma" if active => self.validate_pragma(rest, file_id),
            "line" if active => {
                let expanded = self.expand_macros_in_line(
                    rest,
                    macros,
                    &mut HashSet::new(),
                    ExpansionMode::Normal,
                    file_id,
                    Self::presumed_line_number(line_number, presumed.line_delta),
                    &presumed.file_name,
                )?;
                self.handle_line(&expanded, line_number, presumed, file_id)
            }
            "error" if active => Err(Diagnostic::error(
                rest.trim().to_owned(),
                Self::span(file_id),
            )),
            "include" if active => self.handle_include(
                rest,
                sources,
                path,
                macros,
                out,
                file_id,
                line_number,
                presumed,
            ),
            "define" if active => self.handle_define(rest, macros, file_id),
            "undef" if active => self.handle_undef(rest, macros, file_id),
            _ if active => Ok(()),
            _ => Ok(()),
        }
    }

    fn validate_pragma(&self, body: &str, file_id: FileId) -> Result<(), Diagnostic> {
        let tokens: Vec<_> = body.split_whitespace().collect();
        if tokens.first() != Some(&"STDC") {
            return Ok(());
        }
        let valid_name = matches!(
            tokens.get(1),
            Some(&"FP_CONTRACT") | Some(&"FENV_ACCESS") | Some(&"CX_LIMITED_RANGE")
        );
        let valid_switch = matches!(tokens.get(2), Some(&"ON") | Some(&"OFF") | Some(&"DEFAULT"));
        if tokens.len() == 3 && valid_name && valid_switch {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "invalid #pragma STDC directive",
                Self::span(file_id),
            ))
        }
    }

    fn is_active(&self, conditionals: &[ConditionalFrame]) -> bool {
        conditionals
            .last()
            .is_none_or(|frame| frame.currently_active)
    }

    fn handle_include(
        &self,
        rest: &str,
        sources: &mut SourceManager,
        including_path: &Path,
        macros: &mut HashMap<String, MacroDefinition>,
        out: &mut ExpandedFile,
        file_id: FileId,
        line_number: usize,
        presumed: &PresumedLocation,
    ) -> Result<(), Diagnostic> {
        let direct = rest.trim();
        if let Some((path, text)) = self.internal_header(direct) {
            let nested_id = sources.add_file(path, text.to_owned());
            let nested_path = sources.file(nested_id).path().clone();
            out.append(self.expand_file(sources, nested_id, &nested_path, macros)?);
            return Ok(());
        }
        let rest = if direct.starts_with('"') || direct.starts_with('<') {
            direct.to_owned()
        } else {
            self.expand_macros_in_line(
                direct,
                macros,
                &mut HashSet::new(),
                ExpansionMode::Normal,
                file_id,
                Self::presumed_line_number(line_number, presumed.line_delta),
                &presumed.file_name,
            )?
        };
        let rest = rest.trim();
        if let Some((path, text)) = self.internal_header(rest) {
            let nested_id = sources.add_file(path, text.to_owned());
            let nested_path = sources.file(nested_id).path().clone();
            out.append(self.expand_file(sources, nested_id, &nested_path, macros)?);
            return Ok(());
        }
        let (name, quoted) =
            if let Some(name) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                (name, true)
            } else if let Some(name) = rest.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
                (name, false)
            } else {
                return Err(Diagnostic::error(
                    "unsupported #include syntax",
                    Self::span(file_id),
                ));
            };
        if name.trim().is_empty() {
            return Err(Diagnostic::error(
                "empty header name in #include",
                Self::span(file_id),
            ));
        }

        let mut candidates = Vec::new();
        if quoted {
            let parent = including_path.parent().unwrap_or_else(|| Path::new(""));
            candidates.push(Self::normalize_virtual_path(&parent.join(name)));
        }
        candidates.push(Self::normalize_virtual_path(Path::new(name)));
        candidates.dedup();
        for candidate in candidates {
            if let Some(nested_id) = sources.find_file(&candidate) {
                let nested_path = sources.file(nested_id).path().clone();
                out.append(self.expand_file(sources, nested_id, &nested_path, macros)?);
                return Ok(());
            }
        }

        if quoted {
            let angle_form = format!("<{name}>");
            if let Some((path, text)) = self.internal_header(&angle_form) {
                let nested_id = sources.add_file(path, text.to_owned());
                let nested_path = sources.file(nested_id).path().clone();
                out.append(self.expand_file(sources, nested_id, &nested_path, macros)?);
                return Ok(());
            }
        }

        #[cfg(not(test))]
        {
            Err(Diagnostic::error(
                format!("header file {name:?} is not available"),
                Self::span(file_id),
            ))
        }

        #[cfg(test)]
        {
            let mut include_path = including_path
                .parent()
                .unwrap_or(self.include_root.as_path())
                .join(name);
            let include_text = match fs::read_to_string(&include_path) {
                Ok(text) => text,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    let mut found = None;
                    for dir in &self.include_dirs {
                        let candidate = dir.join(name);
                        match fs::read_to_string(&candidate) {
                            Ok(text) => {
                                found = Some((candidate, text));
                                break;
                            }
                            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                                include_path = candidate;
                            }
                            Err(err) => {
                                return Err(Diagnostic::io(candidate, err));
                            }
                        }
                    }
                    let Some((candidate, text)) = found else {
                        return Err(Diagnostic::io(include_path.clone(), err));
                    };
                    include_path = candidate;
                    text
                }
                Err(err) => {
                    return Err(Diagnostic::io(include_path.clone(), err));
                }
            };
            let nested_id = sources.add_file(include_path.clone(), include_text);
            out.append(self.expand_file(sources, nested_id, &include_path, macros)?);
            Ok(())
        }
    }

    fn normalize_virtual_path(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::Normal(part) => normalized.push(part),
                Component::RootDir | Component::Prefix(_) => {}
            }
        }
        normalized
    }

    fn internal_header(&self, rest: &str) -> Option<(PathBuf, &'static str)> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("include");
        match rest {
            "<stdio.h>" => Some((manifest.join("stdio.h"), HIDDEN_STDIO_H)),
            "<stdlib.h>" => Some((manifest.join("stdlib.h"), HIDDEN_STDLIB_H)),
            "<string.h>" => Some((manifest.join("string.h"), HIDDEN_STRING_H)),
            "<stdarg.h>" => Some((manifest.join("stdarg.h"), HIDDEN_STDARG_H)),
            "<math.h>" => Some((manifest.join("math.h"), HIDDEN_MATH_H)),
            "<complex.h>" => Some((manifest.join("complex.h"), HIDDEN_COMPLEX_H)),
            "<tgmath.h>" => Some((manifest.join("tgmath.h"), HIDDEN_TGMATH_H)),
            "<wchar.h>" => Some((manifest.join("wchar.h"), HIDDEN_WCHAR_H)),
            "<wctype.h>" => Some((manifest.join("wctype.h"), HIDDEN_WCTYPE_H)),
            "<stddef.h>" => Some((manifest.join("stddef.h"), HIDDEN_STDDEF_H)),
            "<stdint.h>" => Some((manifest.join("stdint.h"), HIDDEN_STDINT_H)),
            "<setjmp.h>" => Some((manifest.join("setjmp.h"), HIDDEN_SETJMP_H)),
            "<assert.h>" => Some((manifest.join("assert.h"), HIDDEN_ASSERT_H)),
            "<ctype.h>" => Some((manifest.join("ctype.h"), HIDDEN_CTYPE_H)),
            "<errno.h>" => Some((manifest.join("errno.h"), HIDDEN_ERRNO_H)),
            "<float.h>" => Some((manifest.join("float.h"), HIDDEN_FLOAT_H)),
            "<iso646.h>" => Some((manifest.join("iso646.h"), HIDDEN_ISO646_H)),
            "<limits.h>" => Some((manifest.join("limits.h"), HIDDEN_LIMITS_H)),
            "<stdbool.h>" => Some((manifest.join("stdbool.h"), HIDDEN_STDBOOL_H)),
            "<locale.h>" => Some((manifest.join("locale.h"), HIDDEN_LOCALE_H)),
            "<signal.h>" => Some((manifest.join("signal.h"), HIDDEN_SIGNAL_H)),
            "<inttypes.h>" => Some((manifest.join("inttypes.h"), HIDDEN_INTTYPES_H)),
            "<time.h>" => Some((manifest.join("time.h"), HIDDEN_TIME_H)),
            "<fenv.h>" => Some((manifest.join("fenv.h"), HIDDEN_FENV_H)),
            "<stdalign.h>" => Some((manifest.join("stdalign.h"), HIDDEN_STDALIGN_H)),
            "<stdnoreturn.h>" => Some((manifest.join("stdnoreturn.h"), HIDDEN_STDNORETURN_H)),
            "<uchar.h>" => Some((manifest.join("uchar.h"), HIDDEN_UCHAR_H)),
            _ => None,
        }
    }

    fn handle_define(
        &self,
        rest: &str,
        macros: &mut HashMap<String, MacroDefinition>,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        let Some((name, name_end)) = parse_identifier_with_end(rest) else {
            return Err(Diagnostic::error(
                "expected macro name after #define",
                Self::span(file_id),
            ));
        };
        if is_predefined_macro(name) || name == "defined" {
            return Err(Diagnostic::error(
                format!("predefined macro {name} cannot be redefined"),
                Self::span(file_id),
            ));
        }
        let tail = &rest[name_end..];
        if tail.starts_with('(') {
            let close = tail.find(')').ok_or_else(|| {
                Diagnostic::error(
                    "unterminated parameter list in #define",
                    Self::span(file_id),
                )
            })?;
            let params_text = &tail[1..close];
            let (params, variadic) = self.parse_macro_parameters(params_text, file_id)?;
            let replacement = tail[close + 1..].trim_start().to_owned();
            self.validate_macro_paste_boundaries(&replacement, file_id)?;
            self.validate_macro_stringification_operators(
                &replacement,
                &params,
                variadic,
                file_id,
            )?;
            return self.install_macro_definition(
                name,
                MacroDefinition::Function {
                    params,
                    variadic,
                    replacement,
                },
                macros,
                file_id,
            );
        }
        let replacement = tail.trim_start().to_owned();
        self.validate_macro_paste_boundaries(&replacement, file_id)?;
        self.install_macro_definition(name, MacroDefinition::Object(replacement), macros, file_id)?;
        Ok(())
    }

    fn validate_macro_paste_boundaries(
        &self,
        replacement: &str,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        let replacement = replacement.trim();
        if replacement.starts_with("##")
            || replacement.starts_with("%:%:")
            || replacement.ends_with("##")
            || replacement.ends_with("%:%:")
        {
            return Err(Diagnostic::error(
                "## cannot appear at the beginning or end of a macro replacement list",
                Self::span(file_id),
            ));
        }
        Ok(())
    }

    fn install_macro_definition(
        &self,
        name: &str,
        definition: MacroDefinition,
        macros: &mut HashMap<String, MacroDefinition>,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        if let Some(existing) = macros.get(name) {
            if macro_definitions_equivalent(existing, &definition) {
                return Ok(());
            }
            return Err(Diagnostic::error(
                format!("incompatible redefinition of macro {name}"),
                Self::span(file_id),
            ));
        }
        macros.insert(name.to_owned(), definition);
        Ok(())
    }

    fn parse_macro_parameters(
        &self,
        params_text: &str,
        file_id: FileId,
    ) -> Result<(Vec<String>, bool), Diagnostic> {
        let params_text = params_text.trim();
        if params_text.is_empty() {
            return Ok((Vec::new(), false));
        }
        let mut params = Vec::new();
        let mut variadic = false;
        for (index, param) in params_text.split(',').enumerate() {
            let name = param.trim();
            if name == "..." {
                if index + 1 != params_text.split(',').count() {
                    return Err(Diagnostic::error(
                        "variadic macro parameter must be last",
                        Self::span(file_id),
                    ));
                }
                variadic = true;
                break;
            }
            if !is_identifier(name) {
                return Err(Diagnostic::error(
                    "invalid macro parameter list",
                    Self::span(file_id),
                ));
            }
            if params.iter().any(|existing| existing == name) {
                return Err(Diagnostic::error(
                    "duplicate macro parameter name",
                    Self::span(file_id),
                ));
            }
            params.push(name.to_owned());
        }
        Ok((params, variadic))
    }

    fn validate_macro_stringification_operators(
        &self,
        replacement: &str,
        params: &[String],
        variadic: bool,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        let bytes = replacement.as_bytes();
        let mut idx = 0usize;
        while idx < bytes.len() {
            let ch = bytes[idx] as char;
            if ch == '"' || ch == '\'' {
                idx = skip_quoted_literal(replacement, idx, file_id)?;
                continue;
            }
            if ch != '#' {
                idx += 1;
                continue;
            }
            if idx + 1 < bytes.len() && bytes[idx + 1] as char == '#' {
                idx += 2;
                continue;
            }
            let ident_start = skip_whitespace(replacement, idx + 1);
            let Some((name, end)) = parse_identifier_with_end(&replacement[ident_start..]) else {
                return Err(Diagnostic::error(
                    "# in macro replacement must be followed by a parameter name",
                    Self::span(file_id),
                ));
            };
            if !(params.iter().any(|param| param == name) || variadic && name == "__VA_ARGS__") {
                return Err(Diagnostic::error(
                    "# in macro replacement must be followed by a parameter name",
                    Self::span(file_id),
                ));
            }
            idx = ident_start + end;
        }
        Ok(())
    }

    fn handle_undef(
        &self,
        rest: &str,
        macros: &mut HashMap<String, MacroDefinition>,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        let Some(name) = parse_identifier(rest.trim()) else {
            return Err(Diagnostic::error(
                "expected macro name after #undef",
                Self::span(file_id),
            ));
        };
        if is_predefined_macro(name) || name == "defined" {
            return Err(Diagnostic::error(
                format!("predefined macro {name} cannot be undefined"),
                Self::span(file_id),
            ));
        }
        macros.remove(name);
        Ok(())
    }

    fn expand_macros_in_line(
        &self,
        line: &str,
        macros: &HashMap<String, MacroDefinition>,
        active: &mut HashSet<String>,
        mode: ExpansionMode,
        file_id: FileId,
        presumed_line: usize,
        presumed_file: &str,
    ) -> Result<String, Diagnostic> {
        let expanded = expansion::expand(
            line,
            macros,
            active,
            mode,
            file_id,
            presumed_line,
            presumed_file,
        )?;
        if mode == ExpansionMode::Normal {
            self.strip_pragma_operators(&expanded, file_id)
        } else {
            Ok(expanded)
        }
    }

    fn strip_pragma_operators(&self, line: &str, file_id: FileId) -> Result<String, Diagnostic> {
        if !line.contains("_Pragma") {
            return Ok(line.to_owned());
        }
        let bytes = line.as_bytes();
        let mut idx = 0usize;
        let mut out = String::new();
        while idx < bytes.len() {
            let ch = bytes[idx] as char;
            if ch == '"' || ch == '\'' {
                let end = skip_quoted_literal(line, idx, file_id)?;
                out.push_str(&line[idx..end]);
                idx = end;
                continue;
            }
            if ch.is_ascii_digit()
                || (ch == '.' && bytes.get(idx + 1).is_some_and(u8::is_ascii_digit))
            {
                let end = preprocessing_number_end(line, idx);
                out.push_str(&line[idx..end]);
                idx = end;
                continue;
            }
            if ch == '_' || is_ident_start(ch) {
                let start = idx;
                idx += 1;
                while idx < bytes.len() && is_ident_continue(bytes[idx] as char) {
                    idx += 1;
                }
                let name = &line[start..idx];
                if name != "_Pragma" {
                    out.push_str(name);
                    continue;
                }
                let mut cursor = skip_whitespace(line, idx);
                if cursor >= bytes.len() {
                    return Err(Diagnostic::error(
                        "unterminated _Pragma operator",
                        Self::span(file_id),
                    ));
                }
                if bytes[cursor] as char != '(' {
                    out.push_str(name);
                    continue;
                }
                cursor += 1;
                cursor = skip_whitespace(line, cursor);
                if cursor >= bytes.len() {
                    return Err(Diagnostic::error(
                        "unterminated _Pragma operator",
                        Self::span(file_id),
                    ));
                }
                let quote = pragma_string_literal_quote(line, cursor);
                let Some(quote) = quote else {
                    return Err(Diagnostic::error(
                        "_Pragma requires a parenthesized string literal",
                        Self::span(file_id),
                    ));
                };
                cursor = skip_quoted_literal(line, quote, file_id)?;
                let pragma_body = destringize_pragma(&line[quote + 1..cursor - 1]);
                self.validate_pragma(&pragma_body, file_id)?;
                cursor = skip_whitespace(line, cursor);
                if cursor >= bytes.len() {
                    return Err(Diagnostic::error(
                        "unterminated _Pragma operator",
                        Self::span(file_id),
                    ));
                }
                if bytes[cursor] as char != ')' {
                    return Err(Diagnostic::error(
                        "_Pragma requires a closing )",
                        Self::span(file_id),
                    ));
                }
                idx = cursor + 1;
                if !out.chars().last().is_some_and(char::is_whitespace) {
                    out.push(' ');
                }
                continue;
            }
            out.push(ch);
            idx += 1;
        }
        Ok(out)
    }

    fn evaluate_if_expression(
        &self,
        expr: &str,
        macros: &HashMap<String, MacroDefinition>,
        file_id: FileId,
    ) -> Result<bool, Diagnostic> {
        let expanded = self.expand_macros_in_line(
            expr,
            macros,
            &mut HashSet::new(),
            ExpansionMode::IfExpression,
            file_id,
            1,
            "",
        )?;
        let mut parser = IfExprParser::new(
            IfExprLexer::new(&expanded, file_id).tokenize()?,
            Self::span(file_id),
        );
        Ok(parser.parse_expression()?.truthy())
    }

    fn handle_line(
        &self,
        rest: &str,
        physical_line: usize,
        presumed: &mut PresumedLocation,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        let rest = rest.trim();
        let number_end = rest
            .find(|ch: char| ch.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let number_text = &rest[..number_end];
        if number_text.is_empty() {
            return Err(Diagnostic::error(
                "expected line number after #line",
                Self::span(file_id),
            ));
        }
        let number = number_text.parse::<usize>().map_err(|_| {
            Diagnostic::error(
                "invalid line number in #line directive",
                Self::span(file_id),
            )
        })?;
        if number == 0 || number > i32::MAX as usize {
            return Err(Diagnostic::error(
                "#line number must be between 1 and 2147483647",
                Self::span(file_id),
            ));
        }
        presumed.line_delta = number as isize - (physical_line as isize + 1);
        let file_name = rest[number_end..].trim();
        if !file_name.is_empty() {
            let literal_end = if file_name.starts_with('"') {
                skip_quoted_literal(file_name, 0, file_id)?
            } else {
                0
            };
            if literal_end != file_name.len() {
                return Err(Diagnostic::error(
                    "invalid file name in #line directive",
                    Self::span(file_id),
                ));
            }
            presumed.file_name = parse_line_file_name(file_name).ok_or_else(|| {
                Diagnostic::error("invalid file name in #line directive", Self::span(file_id))
            })?;
        }
        Ok(())
    }

    fn presumed_line_number(physical_line: usize, delta: isize) -> usize {
        physical_line.saturating_add_signed(delta)
    }

    fn span(file_id: FileId) -> Span {
        Span::new(file_id, 0, 0)
    }
}

struct IfExprLexer<'a> {
    text: &'a str,
    idx: usize,
    file_id: FileId,
}

impl<'a> IfExprLexer<'a> {
    fn new(text: &'a str, file_id: FileId) -> Self {
        Self {
            text,
            idx: 0,
            file_id,
        }
    }

    fn tokenize(&mut self) -> Result<Vec<IfToken>, Diagnostic> {
        let bytes = self.text.as_bytes();
        let mut tokens = Vec::new();

        while self.idx < bytes.len() {
            let ch = bytes[self.idx] as char;
            if ch.is_ascii_whitespace() {
                self.idx += 1;
                continue;
            }
            if matches!(ch, 'L' | 'u' | 'U')
                && self.idx + 1 < bytes.len()
                && bytes[self.idx + 1] as char == '\''
            {
                self.idx += 1;
                tokens.push(IfToken::Number(parse_char_constant(
                    self.text,
                    &mut self.idx,
                    self.file_id,
                )?));
                continue;
            }
            if is_ident_start(ch) {
                self.idx += 1;
                while self.idx < bytes.len() && is_ident_continue(bytes[self.idx] as char) {
                    self.idx += 1;
                }
                tokens.push(IfToken::Number(PpInt::signed(0)));
                continue;
            }
            if ch.is_ascii_digit() {
                let start = self.idx;
                self.idx += 1;
                while self.idx < bytes.len() {
                    let current = bytes[self.idx] as char;
                    if current.is_ascii_alphanumeric() || current == '_' {
                        self.idx += 1;
                    } else {
                        break;
                    }
                }
                tokens.push(IfToken::Number(parse_pp_integer_literal(
                    &self.text[start..self.idx],
                    self.file_id,
                )?));
                continue;
            }
            if ch == '\'' {
                tokens.push(IfToken::Number(parse_char_constant(
                    self.text,
                    &mut self.idx,
                    self.file_id,
                )?));
                continue;
            }

            let two_char = if self.idx + 1 < bytes.len() {
                Some(&self.text[self.idx..self.idx + 2])
            } else {
                None
            };
            let token = match two_char {
                Some("++" | "--") => {
                    return Err(Diagnostic::error(
                        "increment and decrement are not valid in #if expressions",
                        Span::new(self.file_id, 0, 0),
                    ));
                }
                Some("||") => {
                    self.idx += 2;
                    IfToken::OrOr
                }
                Some("&&") => {
                    self.idx += 2;
                    IfToken::AndAnd
                }
                Some("==") => {
                    self.idx += 2;
                    IfToken::EqEq
                }
                Some("!=") => {
                    self.idx += 2;
                    IfToken::NotEq
                }
                Some("<=") => {
                    self.idx += 2;
                    IfToken::LessEq
                }
                Some(">=") => {
                    self.idx += 2;
                    IfToken::GreaterEq
                }
                Some("<<") => {
                    self.idx += 2;
                    IfToken::Shl
                }
                Some(">>") => {
                    self.idx += 2;
                    IfToken::Shr
                }
                _ => {
                    self.idx += 1;
                    match ch {
                        '(' => IfToken::LParen,
                        ')' => IfToken::RParen,
                        '?' => IfToken::Question,
                        ':' => IfToken::Colon,
                        '|' => IfToken::Pipe,
                        '^' => IfToken::Caret,
                        '&' => IfToken::Amp,
                        '<' => IfToken::Less,
                        '>' => IfToken::Greater,
                        '+' => IfToken::Plus,
                        '-' => IfToken::Minus,
                        '*' => IfToken::Star,
                        '/' => IfToken::Slash,
                        '%' => IfToken::Percent,
                        '!' => IfToken::Bang,
                        '~' => IfToken::Tilde,
                        _ => {
                            return Err(Diagnostic::error(
                                "invalid token in #if expression",
                                Span::new(self.file_id, 0, 0),
                            ));
                        }
                    }
                }
            };
            tokens.push(token);
        }

        tokens.push(IfToken::End);
        Ok(tokens)
    }
}

struct IfExprParser {
    tokens: Vec<IfToken>,
    idx: usize,
    span: Span,
    evaluating: bool,
}

impl IfExprParser {
    fn new(tokens: Vec<IfToken>, span: Span) -> Self {
        Self {
            tokens,
            idx: 0,
            span,
            evaluating: true,
        }
    }

    fn parse_expression(&mut self) -> Result<PpInt, Diagnostic> {
        let value = self.parse_conditional()?;
        if !matches!(self.peek(), IfToken::End) {
            return Err(Diagnostic::error(
                "unexpected trailing tokens in #if expression",
                self.span,
            ));
        }
        Ok(value)
    }

    fn parse_conditional(&mut self) -> Result<PpInt, Diagnostic> {
        let condition = self.parse_logical_or()?;
        if self.consume_simple(IfToken::Question) {
            let outer_evaluating = self.evaluating;
            self.evaluating = outer_evaluating && condition.truthy();
            let if_true = self.parse_conditional()?;
            self.expect_simple(IfToken::Colon, "expected : in conditional expression")?;
            self.evaluating = outer_evaluating && !condition.truthy();
            let if_false = self.parse_conditional()?;
            self.evaluating = outer_evaluating;
            let selected = if outer_evaluating && condition.truthy() {
                if_true
            } else {
                if_false
            };
            return Ok(selected.with_common_type(if_true.with_common_type(if_false)));
        }
        Ok(condition)
    }

    fn parse_logical_or(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_logical_and()?;
        while self.consume_simple(IfToken::OrOr) {
            let outer_evaluating = self.evaluating;
            self.evaluating = outer_evaluating && !value.truthy();
            let rhs = self.parse_logical_and()?;
            self.evaluating = outer_evaluating;
            value = pp_bool(outer_evaluating && (value.truthy() || rhs.truthy()));
        }
        Ok(value)
    }

    fn parse_logical_and(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_bitwise_or()?;
        while self.consume_simple(IfToken::AndAnd) {
            let outer_evaluating = self.evaluating;
            self.evaluating = outer_evaluating && value.truthy();
            let rhs = self.parse_bitwise_or()?;
            self.evaluating = outer_evaluating;
            value = pp_bool(outer_evaluating && value.truthy() && rhs.truthy());
        }
        Ok(value)
    }

    fn parse_bitwise_or(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_bitwise_xor()?;
        while self.consume_simple(IfToken::Pipe) {
            let rhs = self.parse_bitwise_xor()?;
            value = PpInt {
                bits: value.bits | rhs.bits,
                unsigned: value.unsigned || rhs.unsigned,
            };
        }
        Ok(value)
    }

    fn parse_bitwise_xor(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_bitwise_and()?;
        while self.consume_simple(IfToken::Caret) {
            let rhs = self.parse_bitwise_and()?;
            value = PpInt {
                bits: value.bits ^ rhs.bits,
                unsigned: value.unsigned || rhs.unsigned,
            };
        }
        Ok(value)
    }

    fn parse_bitwise_and(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_equality()?;
        while self.consume_simple(IfToken::Amp) {
            let rhs = self.parse_equality()?;
            value = PpInt {
                bits: value.bits & rhs.bits,
                unsigned: value.unsigned || rhs.unsigned,
            };
        }
        Ok(value)
    }

    fn parse_equality(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_relational()?;
        loop {
            if self.consume_simple(IfToken::EqEq) {
                let rhs = self.parse_relational()?;
                value = pp_bool(value.bits == rhs.bits);
            } else if self.consume_simple(IfToken::NotEq) {
                let rhs = self.parse_relational()?;
                value = pp_bool(value.bits != rhs.bits);
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_relational(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_shift()?;
        loop {
            if self.consume_simple(IfToken::Less) {
                let rhs = self.parse_shift()?;
                value = pp_bool(pp_compare(value, rhs).is_lt());
            } else if self.consume_simple(IfToken::LessEq) {
                let rhs = self.parse_shift()?;
                value = pp_bool(pp_compare(value, rhs).is_le());
            } else if self.consume_simple(IfToken::Greater) {
                let rhs = self.parse_shift()?;
                value = pp_bool(pp_compare(value, rhs).is_gt());
            } else if self.consume_simple(IfToken::GreaterEq) {
                let rhs = self.parse_shift()?;
                value = pp_bool(pp_compare(value, rhs).is_ge());
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_shift(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_additive()?;
        loop {
            if self.consume_simple(IfToken::Shl) {
                let rhs = self.parse_additive()?;
                if self.evaluating {
                    let shift = to_shift_count(rhs, self.span)?;
                    if value.unsigned {
                        value.bits = value.bits.wrapping_shl(shift);
                    } else {
                        let signed = value.bits as i64;
                        let shifted = (signed as i128) << shift;
                        if signed < 0 || shifted > i64::MAX as i128 {
                            return Err(Diagnostic::ub(
                                "signed left shift has a negative operand or an unrepresentable result",
                                self.span,
                                Some("6.5.7"),
                            ));
                        }
                        value.bits = shifted as u64;
                    }
                }
            } else if self.consume_simple(IfToken::Shr) {
                let rhs = self.parse_additive()?;
                if self.evaluating {
                    let shift = to_shift_count(rhs, self.span)?;
                    value.bits = if value.unsigned {
                        value.bits >> shift
                    } else {
                        ((value.bits as i64) >> shift) as u64
                    };
                }
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_additive(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_multiplicative()?;
        loop {
            if self.consume_simple(IfToken::Plus) {
                let rhs = self.parse_multiplicative()?;
                value = PpInt {
                    bits: value.bits.wrapping_add(rhs.bits),
                    unsigned: value.unsigned || rhs.unsigned,
                };
            } else if self.consume_simple(IfToken::Minus) {
                let rhs = self.parse_multiplicative()?;
                value = PpInt {
                    bits: value.bits.wrapping_sub(rhs.bits),
                    unsigned: value.unsigned || rhs.unsigned,
                };
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_multiplicative(&mut self) -> Result<PpInt, Diagnostic> {
        let mut value = self.parse_unary()?;
        loop {
            if self.consume_simple(IfToken::Star) {
                let rhs = self.parse_unary()?;
                value = PpInt {
                    bits: value.bits.wrapping_mul(rhs.bits),
                    unsigned: value.unsigned || rhs.unsigned,
                };
            } else if self.consume_simple(IfToken::Slash) {
                let rhs = self.parse_unary()?;
                if self.evaluating {
                    value = pp_div_rem(value, rhs, false, self.span)?;
                } else {
                    value = PpInt {
                        bits: 0,
                        unsigned: value.unsigned || rhs.unsigned,
                    };
                }
            } else if self.consume_simple(IfToken::Percent) {
                let rhs = self.parse_unary()?;
                if self.evaluating {
                    value = pp_div_rem(value, rhs, true, self.span)?;
                } else {
                    value = PpInt {
                        bits: 0,
                        unsigned: value.unsigned || rhs.unsigned,
                    };
                }
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<PpInt, Diagnostic> {
        if self.consume_simple(IfToken::Bang) {
            return Ok(pp_bool(!self.parse_unary()?.truthy()));
        }
        if self.consume_simple(IfToken::Tilde) {
            let mut value = self.parse_unary()?;
            value.bits = !value.bits;
            return Ok(value);
        }
        if self.consume_simple(IfToken::Plus) {
            return self.parse_unary();
        }
        if self.consume_simple(IfToken::Minus) {
            let mut value = self.parse_unary()?;
            value.bits = value.bits.wrapping_neg();
            return Ok(value);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<PpInt, Diagnostic> {
        match self.next() {
            IfToken::Number(value) => Ok(value),
            IfToken::LParen => {
                let value = self.parse_conditional()?;
                self.expect_simple(IfToken::RParen, "expected ) in #if expression")?;
                Ok(value)
            }
            _ => Err(Diagnostic::error(
                "expected primary expression in #if",
                self.span,
            )),
        }
    }

    fn peek(&self) -> &IfToken {
        self.tokens.get(self.idx).unwrap_or(&IfToken::End)
    }

    fn next(&mut self) -> IfToken {
        let token = self.tokens.get(self.idx).cloned().unwrap_or(IfToken::End);
        self.idx += 1;
        token
    }

    fn consume_simple(&mut self, expected: IfToken) -> bool {
        if same_token_kind(self.peek(), &expected) {
            self.idx += 1;
            true
        } else {
            false
        }
    }

    fn expect_simple(
        &mut self,
        expected: IfToken,
        message: &'static str,
    ) -> Result<(), Diagnostic> {
        if self.consume_simple(expected) {
            Ok(())
        } else {
            Err(Diagnostic::error(message, self.span))
        }
    }
}

fn pp_bool(value: bool) -> PpInt {
    PpInt::signed(i64::from(value))
}

fn pp_compare(left: PpInt, right: PpInt) -> std::cmp::Ordering {
    if left.unsigned || right.unsigned {
        left.bits.cmp(&right.bits)
    } else {
        (left.bits as i64).cmp(&(right.bits as i64))
    }
}

fn pp_div_rem(left: PpInt, right: PpInt, remainder: bool, span: Span) -> Result<PpInt, Diagnostic> {
    let unsigned = left.unsigned || right.unsigned;
    if right.bits == 0 {
        return Err(Diagnostic::error(
            "division by zero in #if expression",
            span,
        ));
    }
    let bits = if unsigned {
        if remainder {
            left.bits % right.bits
        } else {
            left.bits / right.bits
        }
    } else {
        let left = left.bits as i64;
        let right = right.bits as i64;
        let result = if remainder {
            left.checked_rem(right)
        } else {
            left.checked_div(right)
        }
        .ok_or_else(|| Diagnostic::error("integer overflow in #if expression", span))?;
        result as u64
    };
    Ok(PpInt { bits, unsigned })
}

fn to_shift_count(value: PpInt, span: Span) -> Result<u32, Diagnostic> {
    if value.unsigned {
        if value.bits < 64 {
            return Ok(value.bits as u32);
        }
    } else if (value.bits as i64) >= 0 && value.bits < 64 {
        return Ok(value.bits as u32);
    }
    {
        Err(Diagnostic::error(
            "invalid shift count in #if expression",
            span,
        ))
    }
}

fn same_token_kind(left: &IfToken, right: &IfToken) -> bool {
    mem::discriminant(left) == mem::discriminant(right)
}

fn parse_pp_integer_literal(text: &str, file_id: FileId) -> Result<PpInt, Diagnostic> {
    let span = Span::new(file_id, 0, 0);
    let (ty, value, _) = parse_c_integer_literal(text, span)?;
    Ok(if ty.is_unsigned_integer() {
        PpInt::unsigned(value as u64)
    } else {
        PpInt::signed(value as i64)
    })
}

fn parse_char_constant(text: &str, idx: &mut usize, file_id: FileId) -> Result<PpInt, Diagnostic> {
    let bytes = text.as_bytes();
    *idx += 1;
    let mut saw_any = false;
    let mut value = 0i128;

    while *idx < bytes.len() {
        let ch = bytes[*idx] as char;
        if ch == '\'' {
            *idx += 1;
            if !saw_any {
                return Err(Diagnostic::error(
                    "empty character constant in #if expression",
                    Span::new(file_id, 0, 0),
                ));
            }
            return Ok(PpInt::signed(value as i64));
        }
        let unit = if ch == '\\' {
            *idx += 1;
            parse_escape_sequence(text, idx, file_id)?
        } else {
            *idx += 1;
            ch as u8 as i128
        };
        value = (value << 8) | unit;
        saw_any = true;
    }

    Err(Diagnostic::error(
        "unterminated character constant in #if expression",
        Span::new(file_id, 0, 0),
    ))
}

fn parse_escape_sequence(text: &str, idx: &mut usize, file_id: FileId) -> Result<i128, Diagnostic> {
    let bytes = text.as_bytes();
    if *idx >= bytes.len() {
        return Err(Diagnostic::error(
            "unterminated escape sequence in #if expression",
            Span::new(file_id, 0, 0),
        ));
    }
    let ch = bytes[*idx] as char;
    *idx += 1;
    let value = match ch {
        '\'' => '\'' as i128,
        '"' => '"' as i128,
        '?' => '?' as i128,
        '\\' => '\\' as i128,
        'a' => 0x07,
        'b' => 0x08,
        'f' => 0x0c,
        'n' => 0x0a,
        'r' => 0x0d,
        't' => 0x09,
        'v' => 0x0b,
        'x' => {
            let start = *idx;
            while *idx < bytes.len() && (bytes[*idx] as char).is_ascii_hexdigit() {
                *idx += 1;
            }
            if start == *idx {
                return Err(Diagnostic::error(
                    "expected hexadecimal digits after \\x in #if expression",
                    Span::new(file_id, 0, 0),
                ));
            }
            let value = i128::from_str_radix(&text[start..*idx], 16).map_err(|_| {
                Diagnostic::error(
                    "invalid hexadecimal escape sequence in #if expression",
                    Span::new(file_id, 0, 0),
                )
            })?;
            require_if_character_escape_range(value, file_id)?;
            value
        }
        'u' => validate_if_universal_character_name(
            parse_fixed_hex_escape(text, idx, 4, file_id, "\\u")?,
            file_id,
        )?,
        'U' => validate_if_universal_character_name(
            parse_fixed_hex_escape(text, idx, 8, file_id, "\\U")?,
            file_id,
        )?,
        '0'..='7' => {
            let start = *idx - 1;
            while *idx < bytes.len()
                && (*idx - start) < 3
                && matches!(bytes[*idx] as char, '0'..='7')
            {
                *idx += 1;
            }
            let value = i128::from_str_radix(&text[start..*idx], 8).map_err(|_| {
                Diagnostic::error(
                    "invalid octal escape sequence in #if expression",
                    Span::new(file_id, 0, 0),
                )
            })?;
            require_if_character_escape_range(value, file_id)?;
            value
        }
        _ => {
            return Err(Diagnostic::error(
                "unsupported escape sequence in #if expression",
                Span::new(file_id, 0, 0),
            ));
        }
    };
    Ok(value)
}

fn require_if_character_escape_range(value: i128, file_id: FileId) -> Result<(), Diagnostic> {
    if value > u8::MAX as i128 {
        return Err(Diagnostic::error(
            "numeric escape sequence is outside the range of an ordinary character constant",
            Span::new(file_id, 0, 0),
        ));
    }
    Ok(())
}

fn validate_if_universal_character_name(value: i128, file_id: FileId) -> Result<i128, Diagnostic> {
    if (value < 0xa0 && !matches!(value, 0x24 | 0x40 | 0x60))
        || (0xd800..=0xdfff).contains(&value)
        || value > 0x10ffff
    {
        return Err(Diagnostic::error(
            "invalid universal character name in #if expression",
            Span::new(file_id, 0, 0),
        ));
    }
    Ok(value)
}

fn parse_fixed_hex_escape(
    text: &str,
    idx: &mut usize,
    digits: usize,
    file_id: FileId,
    prefix: &str,
) -> Result<i128, Diagnostic> {
    let start = *idx;
    for _ in 0..digits {
        if *idx >= text.len() || !(text.as_bytes()[*idx] as char).is_ascii_hexdigit() {
            return Err(Diagnostic::error(
                format!("{prefix} escape sequence requires exactly {digits} hexadecimal digits"),
                Span::new(file_id, 0, 0),
            ));
        }
        *idx += 1;
    }
    i128::from_str_radix(&text[start..*idx], 16).map_err(|_| {
        Diagnostic::error(
            format!("invalid {prefix} escape sequence in #if expression"),
            Span::new(file_id, 0, 0),
        )
    })
}

fn predefined_macros() -> HashMap<String, MacroDefinition> {
    [
        ("__DATE__", "\"Jan  1 1970\""),
        ("__TIME__", "\"00:00:00\""),
        ("__STDC__", "1"),
        ("__STDC_HOSTED__", "1"),
        ("__STDC_VERSION__", "201112L"),
        ("__STDC_UTF_16__", "1"),
        ("__STDC_UTF_32__", "1"),
        ("__STDC_NO_ATOMICS__", "1"),
        ("__STDC_NO_THREADS__", "1"),
        ("__FILE__", ""),
        ("__LINE__", ""),
    ]
    .into_iter()
    .map(|(name, replacement)| {
        (
            name.to_owned(),
            MacroDefinition::Object(replacement.to_owned()),
        )
    })
    .collect()
}

fn is_predefined_macro(name: &str) -> bool {
    matches!(
        name,
        "__DATE__"
            | "__TIME__"
            | "__STDC__"
            | "__STDC_HOSTED__"
            | "__STDC_VERSION__"
            | "__STDC_UTF_16__"
            | "__STDC_UTF_32__"
            | "__STDC_NO_ATOMICS__"
            | "__STDC_NO_THREADS__"
            | "__FILE__"
            | "__LINE__"
    )
}

fn canonicalize_universal_characters(text: &str, file_id: FileId) -> Result<String, Diagnostic> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Normal,
        String,
        Char,
        LineComment,
        BlockComment,
    }

    let chars = text.char_indices().collect::<Vec<_>>();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;
    let mut mode = Mode::Normal;
    let mut in_identifier = false;
    while index < chars.len() {
        let (byte_offset, ch) = chars[index];
        if ch == '\\'
            && index + 1 < chars.len()
            && matches!(chars[index + 1].1, '\n' | '\r')
            && matches!(mode, Mode::LineComment | Mode::BlockComment)
        {
            out.push(ch);
            out.push(chars[index + 1].1);
            if chars[index + 1].1 == '\r' && index + 2 < chars.len() && chars[index + 2].1 == '\n' {
                out.push('\n');
                index += 3;
            } else {
                index += 2;
            }
            continue;
        }
        if matches!(mode, Mode::String | Mode::Char) && ch == '\\' {
            out.push(ch);
            index += 1;
            if index < chars.len() {
                out.push(chars[index].1);
                index += 1;
            }
            continue;
        }
        if mode == Mode::Normal && ch == '/' && index + 1 < chars.len() {
            match chars[index + 1].1 {
                '/' => {
                    out.push_str("//");
                    index += 2;
                    mode = Mode::LineComment;
                    in_identifier = false;
                    continue;
                }
                '*' => {
                    out.push_str("/*");
                    index += 2;
                    mode = Mode::BlockComment;
                    in_identifier = false;
                    continue;
                }
                _ => {}
            }
        }
        if mode == Mode::BlockComment
            && ch == '*'
            && index + 1 < chars.len()
            && chars[index + 1].1 == '/'
        {
            out.push_str("*/");
            index += 2;
            mode = Mode::Normal;
            in_identifier = false;
            continue;
        }
        if mode == Mode::Normal && ch == '\\' && index + 1 < chars.len() {
            let marker = chars[index + 1].1;
            let digits = match marker {
                'u' => 4,
                'U' => 8,
                _ => 0,
            };
            if digits != 0 {
                if index + 2 + digits > chars.len() {
                    return Err(Diagnostic::error(
                        "incomplete universal character name in identifier",
                        Span::new(file_id, byte_offset, byte_offset + 1),
                    ));
                }
                let mut value = 0u32;
                for (_, digit) in &chars[index + 2..index + 2 + digits] {
                    value = value
                        .checked_mul(16)
                        .and_then(|value| digit.to_digit(16).map(|digit| value + digit))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "invalid universal character name in identifier",
                                Span::new(file_id, byte_offset, byte_offset + 1),
                            )
                        })?;
                }
                let decoded = char::from_u32(value).ok_or_else(|| {
                    Diagnostic::error(
                        "universal character name does not name a valid character",
                        Span::new(file_id, byte_offset, byte_offset + 1),
                    )
                })?;
                if !unicode_identifier_character(decoded)
                    || (!in_identifier
                        && unicode_identifier_character_disallowed_initially(decoded))
                {
                    return Err(Diagnostic::error(
                        "universal character name is not permitted in an identifier",
                        Span::new(file_id, byte_offset, byte_offset + 1),
                    ));
                }
                out.push_str(&format!("__cboxes_uc_{value:08x}_"));
                in_identifier = true;
                index += 2 + digits;
                continue;
            }
        }
        if !ch.is_ascii() {
            if mode == Mode::String || mode == Mode::Char {
                out.push_str(&format!("\\U{:08x}", ch as u32));
            } else if matches!(mode, Mode::LineComment | Mode::BlockComment) {
                out.push('?');
            } else if unicode_identifier_character(ch)
                && (in_identifier || !unicode_identifier_character_disallowed_initially(ch))
            {
                out.push_str(&format!("__cboxes_uc_{:08x}_", ch as u32));
                in_identifier = true;
            } else {
                return Err(Diagnostic::error(
                    "character is not permitted in a C identifier",
                    Span::new(file_id, byte_offset, byte_offset + ch.len_utf8()),
                ));
            }
            index += 1;
            continue;
        }
        out.push(ch);
        let was_normal = mode == Mode::Normal;
        if mode == Mode::LineComment && matches!(ch, '\n' | '\r') {
            mode = Mode::Normal;
            in_identifier = false;
        } else if ch == '"'
            && mode != Mode::Char
            && mode != Mode::LineComment
            && mode != Mode::BlockComment
        {
            mode = if mode == Mode::String {
                Mode::Normal
            } else {
                Mode::String
            };
        } else if ch == '\''
            && mode != Mode::String
            && mode != Mode::LineComment
            && mode != Mode::BlockComment
        {
            mode = if mode == Mode::Char {
                Mode::Normal
            } else {
                Mode::Char
            };
        }
        if was_normal {
            in_identifier = ch == '_' || ch.is_ascii_alphanumeric();
        }
        index += 1;
    }
    Ok(out)
}

fn unicode_identifier_character(ch: char) -> bool {
    let value = ch as u32;
    matches!(
        value,
        0x00a8
            | 0x00aa
            | 0x00ad
            | 0x00af
            | 0x00b2..=0x00b5
            | 0x00b7..=0x00ba
            | 0x00bc..=0x00be
            | 0x00c0..=0x00d6
            | 0x00d8..=0x00f6
            | 0x00f8..=0x00ff
            | 0x0100..=0x167f
            | 0x1681..=0x180d
            | 0x180f..=0x1fff
            | 0x200b..=0x200d
            | 0x202a..=0x202e
            | 0x203f..=0x2040
            | 0x2054
            | 0x2060..=0x206f
            | 0x2070..=0x218f
            | 0x2460..=0x24ff
            | 0x2776..=0x2793
            | 0x2c00..=0x2dff
            | 0x2e80..=0x2fff
            | 0x3004..=0x3007
            | 0x3021..=0x302f
            | 0x3031..=0x303f
            | 0x3040..=0xd7ff
            | 0xf900..=0xfd3d
            | 0xfd40..=0xfdcf
            | 0xfdf0..=0xfe44
            | 0xfe47..=0xfffd
    ) || ((0x10000..=0xefffd).contains(&value) && value & 0xffff <= 0xfffd)
}

fn unicode_identifier_character_disallowed_initially(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036f | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f
    )
}

fn replace_trigraphs(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if idx + 2 < bytes.len() && bytes[idx] == b'?' && bytes[idx + 1] == b'?' {
            let replacement = match bytes[idx + 2] {
                b'=' => Some('#'),
                b'/' => Some('\\'),
                b'\'' => Some('^'),
                b'(' => Some('['),
                b')' => Some(']'),
                b'!' => Some('|'),
                b'<' => Some('{'),
                b'>' => Some('}'),
                b'-' => Some('~'),
                _ => None,
            };
            if let Some(replacement) = replacement {
                out.push(replacement);
                idx += 3;
                continue;
            }
        }
        let ch = text[idx..].chars().next().expect("idx is in bounds");
        out.push(ch);
        idx += ch.len_utf8();
    }
    out
}

fn digraph_at(bytes: &[u8], idx: usize) -> Option<(&'static str, usize)> {
    let rest = bytes.get(idx..)?;
    if rest.starts_with(b"%:%:") {
        return Some(("##", 4));
    }
    [
        (b"<:".as_slice(), "["),
        (b":>".as_slice(), "]"),
        (b"<%".as_slice(), "{"),
        (b"%>".as_slice(), "}"),
        (b"%:".as_slice(), "#"),
    ]
    .into_iter()
    .find_map(|(digraph, replacement)| rest.starts_with(digraph).then_some((replacement, 2)))
}

fn line_splice_length(bytes: &[u8], idx: usize) -> Option<usize> {
    if bytes[idx] != b'\\' {
        return None;
    }
    if idx + 1 < bytes.len() && bytes[idx + 1] == b'\n' {
        return Some(2);
    }
    if idx + 1 < bytes.len() && bytes[idx + 1] == b'\r' {
        if idx + 2 < bytes.len() && bytes[idx + 2] == b'\n' {
            return Some(3);
        }
        return Some(2);
    }
    None
}

fn splice_source_lines(text: &str) -> (String, Vec<usize>) {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(text.len());
    let mut line_map = vec![1usize];
    let mut physical_line = 1usize;
    let mut idx = 0usize;
    while idx < bytes.len() {
        if let Some(consumed) = line_splice_length(bytes, idx) {
            idx += consumed;
            physical_line += 1;
            continue;
        }
        let byte = bytes[idx];
        out.push(byte);
        idx += 1;
        if byte == b'\n' {
            physical_line += 1;
            line_map.push(physical_line);
        }
    }
    (
        String::from_utf8(out).expect("line splicing preserves UTF-8 source bytes"),
        line_map,
    )
}

fn skip_quoted_literal(text: &str, start: usize, file_id: FileId) -> Result<usize, Diagnostic> {
    let bytes = text.as_bytes();
    let quote = bytes[start] as char;
    let mut idx = start + 1;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        idx += 1;
        if ch == '\\' && idx < bytes.len() {
            idx += 1;
            continue;
        }
        if ch == quote {
            return Ok(idx);
        }
    }
    Err(Diagnostic::error(
        "unterminated quoted literal during preprocessing",
        Span::new(file_id, 0, 0),
    ))
}

fn split_directive(text: &str) -> (&str, &str) {
    let keyword_end = text
        .find(|ch: char| ch.is_ascii_whitespace())
        .unwrap_or(text.len());
    let keyword = &text[..keyword_end];
    let rest = text[keyword_end..].trim_start();
    (keyword, rest)
}

fn parse_identifier_with_end(text: &str) -> Option<(&str, usize)> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if !is_ident_start(first) {
        return None;
    }
    let mut end = first.len_utf8();
    for (idx, ch) in chars {
        if is_ident_continue(ch) {
            end = idx + ch.len_utf8();
        } else {
            return Some((&text[..idx], idx));
        }
    }
    Some((&text[..end], end))
}

fn parse_identifier(text: &str) -> Option<&str> {
    parse_identifier_with_end(text.trim()).and_then(|(name, end)| {
        if text.trim()[end..].trim().is_empty() {
            Some(name)
        } else {
            None
        }
    })
}

fn is_identifier(text: &str) -> bool {
    parse_identifier(text) == Some(text)
}

fn skip_whitespace(text: &str, mut idx: usize) -> usize {
    let bytes = text.as_bytes();
    while idx < bytes.len() && (bytes[idx] as char).is_ascii_whitespace() {
        idx += 1;
    }
    idx
}

fn normalize_macro_argument_whitespace(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut idx = 0usize;
    let mut pending_space = false;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch.is_ascii_whitespace() {
            pending_space = !out.is_empty();
            idx += 1;
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        if matches!(ch, '\'' | '"')
            && let Ok(end) = skip_quoted_literal(text, idx, FileId(0))
        {
            out.push_str(&text[idx..end]);
            idx = end;
            continue;
        }
        if let Some(rest) = text[idx..].strip_prefix("__cboxes_uc_")
            && rest.len() >= 9
            && rest.as_bytes()[8] == b'_'
            && let Ok(value) = u32::from_str_radix(&rest[..8], 16)
            && let Some(value) = char::from_u32(value)
        {
            out.push(value);
            idx += "__cboxes_uc_".len() + 9;
            continue;
        }
        out.push(ch);
        idx += 1;
    }
    out
}

fn preprocessing_token_end(text: &str, start: usize, file_id: FileId) -> Result<usize, Diagnostic> {
    let bytes = text.as_bytes();
    if matches!(bytes[start], b'\'' | b'"') {
        return skip_quoted_literal(text, start, file_id);
    }
    if (bytes[start] as char).is_ascii_digit()
        || (bytes[start] == b'.' && bytes.get(start + 1).is_some_and(u8::is_ascii_digit))
    {
        return Ok(preprocessing_number_end(text, start));
    }
    if is_ident_start(bytes[start] as char) {
        let mut end = start + 1;
        while end < bytes.len() && is_ident_continue(bytes[end] as char) {
            end += 1;
        }
        return Ok(end);
    }
    if let Some((_, consumed)) = digraph_at(bytes, start) {
        return Ok(start + consumed);
    }
    Ok(start + 1)
}

fn preprocessing_number_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut end = start + 1;
    while end < bytes.len() {
        let ch = bytes[end] as char;
        if ch == '.' || is_ident_continue(ch) || ch.is_ascii_digit() {
            end += 1;
            continue;
        }
        if matches!(ch, '+' | '-')
            && end > start
            && matches!(bytes[end - 1] as char, 'e' | 'E' | 'p' | 'P')
        {
            end += 1;
            continue;
        }
        break;
    }
    end
}

fn pragma_string_literal_quote(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    if rest.starts_with('"') {
        Some(start)
    } else if rest.starts_with("u8\"") {
        Some(start + 2)
    } else if rest.starts_with("u\"") || rest.starts_with("U\"") || rest.starts_with("L\"") {
        Some(start + 1)
    } else {
        None
    }
}

fn destringize_pragma(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let Some(next) = chars.next() else {
                result.push(ch);
                break;
            };
            if matches!(next, '\\' | '"') {
                result.push(next);
            } else {
                result.push(ch);
                result.push(next);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn trailing_function_macro<'a>(
    expanded: &'a str,
    macros: &HashMap<String, MacroDefinition>,
) -> Option<&'a str> {
    let trimmed = expanded.trim_end();
    let start = trimmed
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_ident_continue(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let name = &trimmed[start..];
    matches!(macros.get(name), Some(MacroDefinition::Function { .. })).then_some(name)
}

fn canonicalize_digraphs_outside_literals(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut idx = 0usize;
    while idx < bytes.len() {
        if matches!(bytes[idx], b'\'' | b'"')
            && let Ok(end) = skip_quoted_literal(text, idx, FileId(0))
        {
            out.push_str(&text[idx..end]);
            idx = end;
            continue;
        }
        if let Some((replacement, consumed)) = digraph_at(bytes, idx) {
            out.push_str(replacement);
            idx += consumed;
            continue;
        }
        let ch = text[idx..].chars().next().expect("valid UTF-8 boundary");
        out.push(ch);
        idx += ch.len_utf8();
    }
    out
}

fn string_literal_token(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn separate_macro_generated_comment_openers(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut idx = 0usize;
    while idx < bytes.len() {
        if matches!(bytes[idx], b'\'' | b'"')
            && let Ok(end) = skip_quoted_literal(text, idx, FileId(0))
        {
            out.push_str(&text[idx..end]);
            idx = end;
            continue;
        }
        if bytes[idx] == b'/'
            && bytes
                .get(idx + 1)
                .is_some_and(|next| matches!(next, b'/' | b'*'))
        {
            out.push('/');
            out.push(' ');
            out.push(bytes[idx + 1] as char);
            idx += 2;
            continue;
        }
        let ch = text[idx..].chars().next().expect("valid UTF-8 boundary");
        out.push(ch);
        idx += ch.len_utf8();
    }
    out
}

fn macro_definitions_equivalent(lhs: &MacroDefinition, rhs: &MacroDefinition) -> bool {
    match (lhs, rhs) {
        (MacroDefinition::Object(lhs), MacroDefinition::Object(rhs)) => {
            normalize_macro_replacement(lhs) == normalize_macro_replacement(rhs)
        }
        (
            MacroDefinition::Function {
                params: lhs_params,
                variadic: lhs_variadic,
                replacement: lhs_replacement,
            },
            MacroDefinition::Function {
                params: rhs_params,
                variadic: rhs_variadic,
                replacement: rhs_replacement,
            },
        ) => {
            lhs_params == rhs_params
                && lhs_variadic == rhs_variadic
                && normalize_macro_replacement(lhs_replacement)
                    == normalize_macro_replacement(rhs_replacement)
        }
        _ => false,
    }
}

fn normalize_macro_replacement(replacement: &str) -> String {
    let bytes = replacement.as_bytes();
    let mut normalized = String::new();
    let mut idx = 0usize;
    let mut pending_space = false;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch.is_ascii_whitespace() {
            pending_space = !normalized.is_empty();
            idx += 1;
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        if ch == '"' || ch == '\'' {
            match skip_quoted_literal(replacement, idx, FileId(0)) {
                Ok(end) => {
                    normalized.push_str(&replacement[idx..end]);
                    idx = end;
                }
                Err(_) => {
                    normalized.push_str(&replacement[idx..]);
                    break;
                }
            }
        } else {
            normalized.push(ch);
            idx += 1;
        }
    }
    normalized
}

fn parse_line_file_name(text: &str) -> Option<String> {
    text.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .map(|name| name.to_owned())
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}
