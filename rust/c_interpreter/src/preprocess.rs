use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::fs;
use std::mem;
use std::path::{Component, Path, PathBuf};

use crate::diag::Diagnostic;
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
    Number(i128),
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
        let mut macros = HashMap::new();
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
        let text = self.normalize_source_text(sources.file(file_id).text(), file_id)?;
        let mut out = ExpandedFile::default();
        let mut conditionals = Vec::new();
        let mut presumed = PresumedLocation {
            file_name: path.display().to_string(),
            line_delta: 0,
        };

        let lines = text.lines().collect::<Vec<_>>();
        let mut line_idx = 0usize;
        while line_idx < lines.len() {
            let line_number = line_idx + 1;
            let line = lines[line_idx];
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix('#') {
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
                )?;
                out.push_line("", file_id, line_number);
                line_idx += 1;
                continue;
            }

            if self.is_active(&conditionals) {
                let presumed_line = Self::presumed_line_number(line_number, presumed.line_delta);
                let mut expanded_source = line.to_owned();
                let mut end_line_idx = line_idx;
                loop {
                    match self.expand_macros_in_line(
                        &expanded_source,
                        macros,
                        &mut HashSet::new(),
                        ExpansionMode::Normal,
                        file_id,
                        presumed_line,
                        &presumed.file_name,
                    ) {
                        Ok(expanded) => {
                            for (offset, expanded_line) in expanded.split('\n').enumerate() {
                                out.push_line(expanded_line, file_id, line_number + offset);
                            }
                            break;
                        }
                        Err(diag)
                            if self.is_unterminated_macro_invocation(&diag)
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
            return Err(Diagnostic::error(
                format!("unterminated conditional directive in {}", path.display()),
                Self::span(file_id),
            ));
        }

        Ok(out)
    }

    fn is_unterminated_macro_invocation(&self, diag: &Diagnostic) -> bool {
        diag.render()
            .starts_with("error: unterminated macro invocation")
    }

    fn normalize_source_text(&self, text: &str, file_id: FileId) -> Result<String, Diagnostic> {
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
                Self::span(file_id),
            ));
        }

        let bytes = text.as_bytes();
        let mut idx = 0usize;
        let mut out = String::with_capacity(text.len());
        let mut mode = Mode::Normal;

        while idx < bytes.len() {
            if let Some(consumed) = line_splice_length(bytes, idx) {
                idx += consumed;
                continue;
            }

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
                                idx += 2;
                                mode = Mode::BlockComment;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    out.push(ch);
                    idx += 1;
                    if ch == '"' {
                        mode = Mode::String;
                    } else if ch == '\'' {
                        mode = Mode::Char;
                    }
                }
                Mode::String | Mode::Char => {
                    let ch = bytes[idx] as char;
                    out.push(ch);
                    idx += 1;
                    if ch == '\\' && idx < bytes.len() {
                        out.push(bytes[idx] as char);
                        idx += 1;
                        continue;
                    }
                    if (mode == Mode::String && ch == '"') || (mode == Mode::Char && ch == '\'') {
                        mode = Mode::Normal;
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
                        }
                        mode = Mode::Normal;
                    } else if ch == '\n' {
                        out.push('\n');
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
                        continue;
                    }
                    let ch = bytes[idx] as char;
                    idx += 1;
                    if ch == '\r' {
                        out.push('\r');
                        if idx < bytes.len() && bytes[idx] as char == '\n' {
                            out.push('\n');
                            idx += 1;
                        }
                    } else if ch == '\n' {
                        out.push('\n');
                    }
                }
            }
        }

        if mode == Mode::BlockComment {
            return Err(Diagnostic::error(
                "unterminated block comment during preprocessing",
                Self::span(file_id),
            ));
        }

        Ok(out)
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
    ) -> Result<(), Diagnostic> {
        let (keyword, rest) = split_directive(directive);
        let active = self.is_active(conditionals);

        match keyword {
            "ifdef" => {
                let Some(name) = parse_identifier(rest) else {
                    return Err(Diagnostic::error(
                        "expected macro name after #ifdef",
                        Self::span(file_id),
                    ));
                };
                let condition = active && macros.contains_key(name);
                conditionals.push(ConditionalFrame {
                    parent_active: active,
                    branch_taken: condition,
                    currently_active: condition,
                    saw_else: false,
                });
                Ok(())
            }
            "ifndef" => {
                let Some(name) = parse_identifier(rest) else {
                    return Err(Diagnostic::error(
                        "expected macro name after #ifndef",
                        Self::span(file_id),
                    ));
                };
                let condition = active && !macros.contains_key(name);
                conditionals.push(ConditionalFrame {
                    parent_active: active,
                    branch_taken: condition,
                    currently_active: condition,
                    saw_else: false,
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
            "pragma" if active => Ok(()),
            "line" if active => self.handle_line(rest, line_number, presumed, file_id),
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
            _ if active => Err(Diagnostic::error(
                format!(
                    "unsupported preprocessing directive on line {} in {}",
                    line_number,
                    path.display()
                ),
                Self::span(file_id),
            )),
            _ => Ok(()),
        }
    }

    fn is_active(&self, conditionals: &[ConditionalFrame]) -> bool {
        conditionals
            .last()
            .map_or(true, |frame| frame.currently_active)
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

        #[cfg(not(test))]
        {
            return Err(Diagnostic::error(
                format!("header file {name:?} is not available"),
                Self::span(file_id),
            ));
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
            macros.insert(
                name.to_owned(),
                MacroDefinition::Function {
                    params,
                    variadic,
                    replacement,
                },
            );
            return Ok(());
        }
        let replacement = tail.trim_start().to_owned();
        macros.insert(name.to_owned(), MacroDefinition::Object(replacement));
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
            if is_ident_start(ch) {
                let start = idx;
                idx += 1;
                while idx < bytes.len() && is_ident_continue(bytes[idx] as char) {
                    idx += 1;
                }
                let name = &line[start..idx];

                if mode == ExpansionMode::IfExpression && name == "defined" {
                    let (defined, next_idx) =
                        self.consume_defined_operand(line, idx, macros, file_id)?;
                    out.push_str(if defined { "1" } else { "0" });
                    idx = next_idx;
                    continue;
                }

                if name == "__LINE__" {
                    out.push_str(&presumed_line.to_string());
                    continue;
                }
                if name == "__FILE__" {
                    out.push_str(&string_literal_token(presumed_file));
                    continue;
                }

                let Some(definition) = macros.get(name).cloned() else {
                    out.push_str(name);
                    continue;
                };

                match definition {
                    MacroDefinition::Object(replacement) => {
                        if active.insert(name.to_owned()) {
                            out.push_str(&self.expand_macros_in_line(
                                &replacement,
                                macros,
                                active,
                                mode,
                                file_id,
                                presumed_line,
                                presumed_file,
                            )?);
                            active.remove(name);
                        } else {
                            out.push_str(name);
                        }
                    }
                    MacroDefinition::Function {
                        params,
                        variadic,
                        replacement,
                    } => {
                        let call_start = skip_whitespace(line, idx);
                        if call_start >= bytes.len() || bytes[call_start] as char != '(' {
                            out.push_str(name);
                            continue;
                        }
                        if !active.insert(name.to_owned()) {
                            out.push_str(name);
                            continue;
                        }
                        let (raw_args, end_idx) =
                            self.parse_macro_invocation(line, call_start, file_id)?;
                        if (!variadic && raw_args.len() != params.len())
                            || (variadic && raw_args.len() < params.len())
                        {
                            return Err(Diagnostic::error(
                                format!(
                                    "macro {} expects {}{} argument(s), got {}",
                                    name,
                                    params.len(),
                                    if variadic { "+" } else { "" },
                                    raw_args.len()
                                ),
                                Self::span(file_id),
                            ));
                        }
                        let mut expanded_args = Vec::with_capacity(raw_args.len());
                        for arg in raw_args.iter().take(params.len()) {
                            expanded_args.push(self.expand_macros_in_line(
                                arg,
                                macros,
                                &mut HashSet::new(),
                                mode,
                                file_id,
                                presumed_line,
                                presumed_file,
                            )?);
                        }
                        let raw_variadic_args = if variadic {
                            raw_args
                                .iter()
                                .skip(params.len())
                                .map(|arg| arg.trim())
                                .collect::<Vec<_>>()
                                .join(", ")
                        } else {
                            String::new()
                        };
                        let expanded_variadic_args = if variadic {
                            raw_args
                                .iter()
                                .skip(params.len())
                                .map(|arg| {
                                    self.expand_macros_in_line(
                                        arg,
                                        macros,
                                        &mut HashSet::new(),
                                        mode,
                                        file_id,
                                        presumed_line,
                                        presumed_file,
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?
                                .join(", ")
                        } else {
                            String::new()
                        };
                        let substituted = self.substitute_macro_parameters(
                            &replacement,
                            &params,
                            &raw_args,
                            &expanded_args,
                            variadic,
                            &raw_variadic_args,
                            &expanded_variadic_args,
                            file_id,
                        )?;
                        out.push_str(&self.expand_macros_in_line(
                            &substituted,
                            macros,
                            active,
                            mode,
                            file_id,
                            presumed_line,
                            presumed_file,
                        )?);
                        active.remove(name);
                        idx = end_idx;
                    }
                }
                continue;
            }
            out.push(ch);
            idx += 1;
        }

        if mode == ExpansionMode::Normal {
            self.strip_pragma_operators(&out, file_id)
        } else {
            Ok(out)
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
                if cursor >= bytes.len() || bytes[cursor] as char != '(' {
                    out.push_str(name);
                    continue;
                }
                cursor += 1;
                cursor = skip_whitespace(line, cursor);
                if cursor >= bytes.len() || bytes[cursor] as char != '"' {
                    return Err(Diagnostic::error(
                        "_Pragma requires a parenthesized string literal",
                        Self::span(file_id),
                    ));
                }
                cursor = skip_quoted_literal(line, cursor, file_id)?;
                cursor = skip_whitespace(line, cursor);
                if cursor >= bytes.len() || bytes[cursor] as char != ')' {
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

    fn consume_defined_operand(
        &self,
        line: &str,
        start: usize,
        macros: &HashMap<String, MacroDefinition>,
        file_id: FileId,
    ) -> Result<(bool, usize), Diagnostic> {
        let mut idx = skip_whitespace(line, start);
        let bytes = line.as_bytes();
        if idx < bytes.len() && bytes[idx] as char == '(' {
            idx += 1;
            idx = skip_whitespace(line, idx);
            let Some((name, end)) = parse_identifier_with_end(&line[idx..]) else {
                return Err(Diagnostic::error(
                    "expected macro name after defined(",
                    Self::span(file_id),
                ));
            };
            idx += end;
            idx = skip_whitespace(line, idx);
            if idx >= bytes.len() || bytes[idx] as char != ')' {
                return Err(Diagnostic::error(
                    "expected ) after defined(...)",
                    Self::span(file_id),
                ));
            }
            return Ok((macros.contains_key(name), idx + 1));
        }
        let Some((name, end)) = parse_identifier_with_end(&line[idx..]) else {
            return Err(Diagnostic::error(
                "expected macro name after defined",
                Self::span(file_id),
            ));
        };
        idx += end;
        Ok((macros.contains_key(name), idx))
    }

    fn parse_macro_invocation(
        &self,
        line: &str,
        open_paren: usize,
        file_id: FileId,
    ) -> Result<(Vec<String>, usize), Diagnostic> {
        let bytes = line.as_bytes();
        let mut args = Vec::new();
        let mut idx = open_paren + 1;
        let mut arg_start = idx;
        let mut depth = 0usize;

        while idx < bytes.len() {
            let ch = bytes[idx] as char;
            if ch == '"' || ch == '\'' {
                idx = skip_quoted_literal(line, idx, file_id)?;
                continue;
            }
            match ch {
                '(' => {
                    depth += 1;
                    idx += 1;
                }
                ')' => {
                    if depth == 0 {
                        let between = line[open_paren + 1..idx].trim();
                        if args.is_empty() && between.is_empty() {
                            return Ok((Vec::new(), idx + 1));
                        }
                        args.push(line[arg_start..idx].trim().to_owned());
                        return Ok((args, idx + 1));
                    }
                    depth -= 1;
                    idx += 1;
                }
                ',' if depth == 0 => {
                    args.push(line[arg_start..idx].trim().to_owned());
                    idx += 1;
                    arg_start = idx;
                }
                _ => idx += 1,
            }
        }

        Err(Diagnostic::error(
            "unterminated macro invocation",
            Self::span(file_id),
        ))
    }

    fn substitute_macro_parameters(
        &self,
        replacement: &str,
        params: &[String],
        raw_args: &[String],
        expanded_args: &[String],
        variadic: bool,
        raw_variadic_args: &str,
        expanded_variadic_args: &str,
        file_id: FileId,
    ) -> Result<String, Diagnostic> {
        let raw_substitutions: HashMap<&str, &str> = params
            .iter()
            .zip(raw_args.iter())
            .map(|(param, arg)| (param.as_str(), arg.as_str()))
            .collect();
        let expanded_substitutions: HashMap<&str, &str> = params
            .iter()
            .zip(expanded_args.iter())
            .map(|(param, arg)| (param.as_str(), arg.as_str()))
            .collect();
        let bytes = replacement.as_bytes();
        let mut idx = 0usize;
        let mut out = String::new();

        while idx < bytes.len() {
            let ch = bytes[idx] as char;
            if ch == '"' || ch == '\'' {
                let end = skip_quoted_literal(replacement, idx, FileId(0))?;
                out.push_str(&replacement[idx..end]);
                idx = end;
                continue;
            }
            if ch == '#' {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '#' {
                    idx += 2;
                    trim_trailing_whitespace(&mut out);
                    idx = skip_whitespace(replacement, idx);
                    let (token, next_idx) = self.macro_replacement_token(
                        replacement,
                        idx,
                        &raw_substitutions,
                        variadic,
                        raw_variadic_args,
                        file_id,
                    )?;
                    out.push_str(&token);
                    idx = next_idx;
                    continue;
                }
                idx += 1;
                let ident_start = skip_whitespace(replacement, idx);
                let Some((name, end)) = parse_identifier_with_end(&replacement[ident_start..])
                else {
                    return Err(Diagnostic::error(
                        "# in macro replacement must be followed by a parameter name",
                        Self::span(file_id),
                    ));
                };
                let raw = if variadic && name == "__VA_ARGS__" {
                    raw_variadic_args
                } else {
                    raw_substitutions.get(name).copied().ok_or_else(|| {
                        Diagnostic::error(
                            "# in macro replacement must be followed by a parameter name",
                            Self::span(file_id),
                        )
                    })?
                };
                out.push_str(&string_literal_token(&normalize_macro_argument_whitespace(
                    raw,
                )));
                idx = ident_start + end;
                continue;
            }
            if is_ident_start(ch) {
                let start = idx;
                idx += 1;
                while idx < bytes.len() && is_ident_continue(bytes[idx] as char) {
                    idx += 1;
                }
                let name = &replacement[start..idx];
                if variadic && name == "__VA_ARGS__" {
                    out.push_str(expanded_variadic_args);
                } else if let Some(arg) = expanded_substitutions.get(name) {
                    out.push_str(arg);
                } else {
                    out.push_str(name);
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
        Ok(parser.parse_expression()? != 0)
    }

    fn handle_line(
        &self,
        rest: &str,
        physical_line: usize,
        presumed: &mut PresumedLocation,
        file_id: FileId,
    ) -> Result<(), Diagnostic> {
        let mut parts = rest.split_whitespace();
        let Some(number_text) = parts.next() else {
            return Err(Diagnostic::error(
                "expected line number after #line",
                Self::span(file_id),
            ));
        };
        let number = number_text.parse::<usize>().map_err(|_| {
            Diagnostic::error(
                "invalid line number in #line directive",
                Self::span(file_id),
            )
        })?;
        presumed.line_delta = number as isize - (physical_line as isize + 1);
        if let Some(file_name) = parts.next() {
            presumed.file_name = parse_line_file_name(file_name).ok_or_else(|| {
                Diagnostic::error("invalid file name in #line directive", Self::span(file_id))
            })?;
        }
        Ok(())
    }

    fn presumed_line_number(physical_line: usize, delta: isize) -> usize {
        physical_line.saturating_add_signed(delta)
    }

    fn macro_replacement_token(
        &self,
        replacement: &str,
        start: usize,
        raw_substitutions: &HashMap<&str, &str>,
        variadic: bool,
        raw_variadic_args: &str,
        file_id: FileId,
    ) -> Result<(String, usize), Diagnostic> {
        if start >= replacement.len() {
            return Ok((String::new(), start));
        }
        let bytes = replacement.as_bytes();
        let ch = bytes[start] as char;
        if ch == '"' || ch == '\'' {
            let end = skip_quoted_literal(replacement, start, file_id)?;
            return Ok((replacement[start..end].to_owned(), end));
        }
        if is_ident_start(ch) {
            let mut end = start + 1;
            while end < bytes.len() && is_ident_continue(bytes[end] as char) {
                end += 1;
            }
            let name = &replacement[start..end];
            let token = if variadic && name == "__VA_ARGS__" {
                raw_variadic_args.to_owned()
            } else if let Some(arg) = raw_substitutions.get(name) {
                (*arg).to_owned()
            } else {
                name.to_owned()
            };
            return Ok((token, end));
        }
        Ok((ch.to_string(), start + 1))
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
            if is_ident_start(ch) {
                self.idx += 1;
                while self.idx < bytes.len() && is_ident_continue(bytes[self.idx] as char) {
                    self.idx += 1;
                }
                tokens.push(IfToken::Number(0));
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
}

impl IfExprParser {
    fn new(tokens: Vec<IfToken>, span: Span) -> Self {
        Self {
            tokens,
            idx: 0,
            span,
        }
    }

    fn parse_expression(&mut self) -> Result<i128, Diagnostic> {
        let value = self.parse_conditional()?;
        if !matches!(self.peek(), IfToken::End) {
            return Err(Diagnostic::error(
                "unexpected trailing tokens in #if expression",
                self.span,
            ));
        }
        Ok(value)
    }

    fn parse_conditional(&mut self) -> Result<i128, Diagnostic> {
        let condition = self.parse_logical_or()?;
        if self.consume_simple(IfToken::Question) {
            let if_true = self.parse_conditional()?;
            self.expect_simple(IfToken::Colon, "expected : in conditional expression")?;
            let if_false = self.parse_conditional()?;
            return Ok(if condition != 0 { if_true } else { if_false });
        }
        Ok(condition)
    }

    fn parse_logical_or(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_logical_and()?;
        while self.consume_simple(IfToken::OrOr) {
            let rhs = self.parse_logical_and()?;
            value = bool_to_int(value != 0 || rhs != 0);
        }
        Ok(value)
    }

    fn parse_logical_and(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_bitwise_or()?;
        while self.consume_simple(IfToken::AndAnd) {
            let rhs = self.parse_bitwise_or()?;
            value = bool_to_int(value != 0 && rhs != 0);
        }
        Ok(value)
    }

    fn parse_bitwise_or(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_bitwise_xor()?;
        while self.consume_simple(IfToken::Pipe) {
            value |= self.parse_bitwise_xor()?;
        }
        Ok(value)
    }

    fn parse_bitwise_xor(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_bitwise_and()?;
        while self.consume_simple(IfToken::Caret) {
            value ^= self.parse_bitwise_and()?;
        }
        Ok(value)
    }

    fn parse_bitwise_and(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_equality()?;
        while self.consume_simple(IfToken::Amp) {
            value &= self.parse_equality()?;
        }
        Ok(value)
    }

    fn parse_equality(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_relational()?;
        loop {
            if self.consume_simple(IfToken::EqEq) {
                let rhs = self.parse_relational()?;
                value = bool_to_int(value == rhs);
            } else if self.consume_simple(IfToken::NotEq) {
                let rhs = self.parse_relational()?;
                value = bool_to_int(value != rhs);
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_relational(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_shift()?;
        loop {
            if self.consume_simple(IfToken::Less) {
                let rhs = self.parse_shift()?;
                value = bool_to_int(value < rhs);
            } else if self.consume_simple(IfToken::LessEq) {
                let rhs = self.parse_shift()?;
                value = bool_to_int(value <= rhs);
            } else if self.consume_simple(IfToken::Greater) {
                let rhs = self.parse_shift()?;
                value = bool_to_int(value > rhs);
            } else if self.consume_simple(IfToken::GreaterEq) {
                let rhs = self.parse_shift()?;
                value = bool_to_int(value >= rhs);
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_shift(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_additive()?;
        loop {
            if self.consume_simple(IfToken::Shl) {
                let rhs = self.parse_additive()?;
                let shift = to_shift_count(rhs, self.span)?;
                value = value.wrapping_shl(shift);
            } else if self.consume_simple(IfToken::Shr) {
                let rhs = self.parse_additive()?;
                let shift = to_shift_count(rhs, self.span)?;
                value >>= shift;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_additive(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_multiplicative()?;
        loop {
            if self.consume_simple(IfToken::Plus) {
                value = value.wrapping_add(self.parse_multiplicative()?);
            } else if self.consume_simple(IfToken::Minus) {
                value = value.wrapping_sub(self.parse_multiplicative()?);
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_multiplicative(&mut self) -> Result<i128, Diagnostic> {
        let mut value = self.parse_unary()?;
        loop {
            if self.consume_simple(IfToken::Star) {
                value = value.wrapping_mul(self.parse_unary()?);
            } else if self.consume_simple(IfToken::Slash) {
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(Diagnostic::error(
                        "division by zero in #if expression",
                        self.span,
                    ));
                }
                value /= rhs;
            } else if self.consume_simple(IfToken::Percent) {
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(Diagnostic::error(
                        "division by zero in #if expression",
                        self.span,
                    ));
                }
                value %= rhs;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<i128, Diagnostic> {
        if self.consume_simple(IfToken::Bang) {
            return Ok(bool_to_int(self.parse_unary()? == 0));
        }
        if self.consume_simple(IfToken::Tilde) {
            return Ok(!self.parse_unary()?);
        }
        if self.consume_simple(IfToken::Plus) {
            return self.parse_unary();
        }
        if self.consume_simple(IfToken::Minus) {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i128, Diagnostic> {
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

fn bool_to_int(value: bool) -> i128 {
    if value { 1 } else { 0 }
}

fn to_shift_count(value: i128, span: Span) -> Result<u32, Diagnostic> {
    if !(0..128).contains(&value) {
        return Err(Diagnostic::error(
            "invalid shift count in #if expression",
            span,
        ));
    }
    Ok(value as u32)
}

fn same_token_kind(left: &IfToken, right: &IfToken) -> bool {
    mem::discriminant(left) == mem::discriminant(right)
}

fn parse_pp_integer_literal(text: &str, file_id: FileId) -> Result<i128, Diagnostic> {
    let digits_end = text
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(text.len());
    let token = &text[..digits_end];
    let suffix_start = token
        .find(|ch: char| !(ch.is_ascii_hexdigit() || ch == 'x' || ch == 'X'))
        .unwrap_or(token.len());
    let (digits, suffix) = token.split_at(suffix_start);
    if !suffix.chars().all(|ch| matches!(ch, 'u' | 'U' | 'l' | 'L')) {
        return Err(Diagnostic::error(
            "invalid integer literal in #if expression",
            Span::new(file_id, 0, 0),
        ));
    }
    let (radix, digits) = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, rest)
    } else if digits.starts_with('0') && digits.len() > 1 {
        (8, digits)
    } else {
        (10, digits)
    };
    let parsed = i128::from_str_radix(digits, radix).map_err(|_| {
        Diagnostic::error(
            "invalid integer literal in #if expression",
            Span::new(file_id, 0, 0),
        )
    })?;
    Ok(parsed)
}

fn parse_char_constant(text: &str, idx: &mut usize, file_id: FileId) -> Result<i128, Diagnostic> {
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
            return Ok(value);
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
            i128::from_str_radix(&text[start..*idx], 16).map_err(|_| {
                Diagnostic::error(
                    "invalid hexadecimal escape sequence in #if expression",
                    Span::new(file_id, 0, 0),
                )
            })?
        }
        'u' => parse_fixed_hex_escape(text, idx, 4, file_id, "\\u")?,
        'U' => parse_fixed_hex_escape(text, idx, 8, file_id, "\\U")?,
        '0'..='7' => {
            let start = *idx - 1;
            while *idx < bytes.len()
                && (*idx - start) < 3
                && matches!(bytes[*idx] as char, '0'..='7')
            {
                *idx += 1;
            }
            i128::from_str_radix(&text[start..*idx], 8).map_err(|_| {
                Diagnostic::error(
                    "invalid octal escape sequence in #if expression",
                    Span::new(file_id, 0, 0),
                )
            })?
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
    let Some((_, first)) = chars.next() else {
        return None;
    };
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

fn trim_trailing_whitespace(text: &mut String) {
    while text
        .chars()
        .last()
        .is_some_and(|ch| ch.is_ascii_whitespace())
    {
        text.pop();
    }
}

fn normalize_macro_argument_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
