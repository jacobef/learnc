use super::*;
use std::collections::hash_map::Entry;

#[derive(Clone, Debug)]
struct Token {
    text: String,
    space: String,
    line: usize,
    hidden: HashSet<String>,
    paste: bool,
}

type MacroArguments = (Vec<Vec<Token>>, Vec<Token>, usize);

fn tokens(text: &str, file: FileId, mut line: usize) -> Result<Vec<Token>, Diagnostic> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let start = i;
        while i < text.len() && text.as_bytes()[i].is_ascii_whitespace() {
            if text.as_bytes()[i] == b'\n' {
                line += 1;
            }
            i += 1;
        }
        if i == text.len() {
            break;
        }
        let space = text[start..i].to_owned();
        let rest = &text[i..];
        let prefix = ["u8\"", "u\"", "U\"", "L\"", "u'", "U'", "L'"]
            .iter()
            .find(|p| rest.starts_with(**p));
        let end = if let Some(prefix) = prefix {
            skip_quoted_literal(text, i + prefix.len() - 1, file)?
        } else if let Some(punct) = [
            "%:%:", ">>=", "<<=", "...", "##", "->", "++", "--", "<<", ">>", "<=", ">=", "==",
            "!=", "&&", "||", "*=", "/=", "%=", "+=", "-=", "&=", "^=", "|=",
        ]
        .iter()
        .find(|p| rest.starts_with(**p))
        {
            i + punct.len()
        } else if !text.is_char_boundary(i + 1) {
            i + rest.chars().next().unwrap().len_utf8()
        } else {
            preprocessing_token_end(text, i, file)?
        };
        result.push(Token {
            text: text[i..end].to_owned(),
            space,
            line,
            hidden: HashSet::new(),
            paste: false,
        });
        i = end;
    }
    Ok(result)
}

fn render(input: &[Token], file: FileId) -> Result<String, Diagnostic> {
    let mut result = String::new();
    let mut previous: Option<&Token> = None;
    for token in input {
        if token.text.is_empty() {
            continue;
        }
        result.push_str(&token.space);
        if token.space.is_empty()
            && let Some(prev) = previous
        {
            let joined = format!("{}{}", prev.text, token.text);
            // Macro substitution never joins tokens except through ##.
            if joined.starts_with("/*")
                || joined.starts_with("//")
                || (result.ends_with("..") && token.text.starts_with('.'))
                || tokens(&joined, file, 1)?.len() != 2
            {
                result.push(' ');
            }
        }
        result.push_str(&token.text);
        previous = Some(token);
    }
    Ok(result)
}

fn stringize(input: &[Token]) -> String {
    let mut text = String::new();
    for token in input {
        if !text.is_empty() && !token.space.is_empty() {
            text.push(' ');
        }
        text.push_str(&token.text);
    }
    string_literal_token(&normalize_macro_argument_whitespace(&text))
}

struct Expander<'a> {
    macros: &'a HashMap<String, MacroDefinition>,
    file: FileId,
    filename: &'a str,
    mode: ExpansionMode,
}

impl Expander<'_> {
    fn error(&self, text: impl Into<String>) -> Diagnostic {
        Diagnostic::error(text, Preprocessor::span(self.file))
    }

    fn expand(&self, mut input: Vec<Token>, depth: usize) -> Result<Vec<Token>, Diagnostic> {
        if depth > 256 {
            return Err(self.error("macro argument expansion is too deeply nested"));
        }
        let mut i = 0;
        while i < input.len() {
            let token = input[i].clone();
            let name = token.text.as_str();
            if self.mode == ExpansionMode::IfExpression && name == "defined" {
                let parenthesized = input.get(i + 1).is_some_and(|t| t.text == "(");
                let operand = i + 1 + usize::from(parenthesized);
                let identifier = input
                    .get(operand)
                    .ok_or_else(|| self.error("defined requires an identifier"))?;
                if !is_identifier(&identifier.text) {
                    return Err(self.error("defined requires an identifier"));
                }
                let end = operand + 1 + usize::from(parenthesized);
                if parenthesized && input.get(operand + 1).is_none_or(|t| t.text != ")") {
                    return Err(self.error("defined requires a closing parenthesis"));
                }
                let value = if self.macros.contains_key(&identifier.text) {
                    "1"
                } else {
                    "0"
                };
                let mut replacement = token.clone();
                replacement.text = value.into();
                input.splice(i..end, [replacement]);
                i += 1;
                continue;
            }
            if name == "__LINE__" || name == "__FILE__" {
                input[i].text = if name == "__LINE__" {
                    token.line.to_string()
                } else {
                    string_literal_token(self.filename)
                };
                i += 1;
                continue;
            }
            if token.hidden.contains(name) {
                i += 1;
                continue;
            }
            let Some(definition) = self.macros.get(name) else {
                i += 1;
                continue;
            };
            let (mut replacement, end, mut hidden) = match definition {
                MacroDefinition::Object(body) => (
                    replacement_tokens(body, self.file, token.line)?,
                    i + 1,
                    token.hidden.clone(),
                ),
                MacroDefinition::Function {
                    params,
                    variadic,
                    replacement,
                } => {
                    if input.get(i + 1).is_none_or(|t| t.text != "(") {
                        i += 1;
                        continue;
                    }
                    let (mut arguments, separators, close) = match self.arguments(&input, i + 1) {
                        Err(_) if depth > 0 => {
                            i += 1;
                            continue;
                        }
                        other => other?,
                    };
                    if arguments.is_empty() && (!params.is_empty() || *variadic) {
                        arguments.push(Vec::new());
                    }
                    if (!variadic && arguments.len() != params.len())
                        || (*variadic && arguments.len() <= params.len())
                    {
                        return Err(self.error(format!(
                            "macro {name} expects {}{} argument(s), got {}",
                            params.len() + usize::from(*variadic),
                            if *variadic { "+" } else { "" },
                            arguments.len()
                        )));
                    }
                    let body = replacement_tokens(replacement, self.file, token.line)?;
                    let mut names = params.clone();
                    if *variadic {
                        let tail = arguments.split_off(params.len());
                        let mut joined = Vec::new();
                        for (j, arg) in tail.into_iter().enumerate() {
                            if j != 0 {
                                joined.push(separators[params.len() + j - 1].clone());
                            }
                            joined.extend(arg);
                        }
                        arguments.push(joined);
                        names.push("__VA_ARGS__".into());
                    }
                    let mut expanded: HashMap<usize, Vec<Token>> = HashMap::new();
                    let mut substituted = Vec::new();
                    let mut j = 0;
                    while j < body.len() {
                        let current = &body[j];
                        if matches!(current.text.as_str(), "#" | "%:") {
                            let next = body
                                .get(j + 1)
                                .and_then(|t| names.iter().position(|n| n == &t.text))
                                .ok_or_else(|| {
                                    self.error("# must be followed by a macro parameter")
                                })?;
                            let mut quoted = current.clone();
                            quoted.text = stringize(&arguments[next]);
                            substituted.push(quoted);
                            j += 2;
                            continue;
                        }
                        if let Some(index) = names.iter().position(|n| n == &current.text) {
                            let pasted = j.checked_sub(1).is_some_and(|p| is_paste(&body[p]))
                                || body.get(j + 1).is_some_and(is_paste);
                            let mut arg = if pasted {
                                arguments[index].clone()
                            } else {
                                match expanded.entry(index) {
                                    Entry::Occupied(entry) => entry.get().clone(),
                                    Entry::Vacant(entry) => entry
                                        .insert(self.expand(arguments[index].clone(), depth + 1)?)
                                        .clone(),
                                }
                            };
                            if arg.is_empty() && pasted {
                                let mut empty = current.clone();
                                empty.text.clear();
                                arg.push(empty);
                            }
                            if let Some(first) = arg.first_mut() {
                                first.space = current.space.clone();
                            }
                            substituted.extend(arg);
                        } else {
                            substituted.push(current.clone());
                        }
                        j += 1;
                    }
                    let hidden = token
                        .hidden
                        .intersection(&input[close].hidden)
                        .cloned()
                        .collect();
                    (substituted, close + 1, hidden)
                }
            };
            // Only operators from the replacement list may paste. Arguments may
            // themselves contain ## as ordinary preprocessing tokens.
            replacement = self.paste(replacement)?;
            hidden.insert(name.to_owned());
            for t in &mut replacement {
                t.hidden.extend(hidden.iter().cloned());
            }
            if let Some(first) = replacement.first_mut() {
                first.space = token.space;
            } else if let Some(next) = input.get_mut(end) {
                next.space = format!("{}{}", token.space, next.space);
            }
            input.splice(i..end, replacement);
        }
        Ok(input)
    }

    fn arguments(&self, input: &[Token], open: usize) -> Result<MacroArguments, Diagnostic> {
        let mut separators = Vec::new();
        let mut args = Vec::new();
        let mut start = open + 1;
        let mut nesting = 0;
        for i in open + 1..input.len() {
            match input[i].text.as_str() {
                "(" => nesting += 1,
                ")" if nesting == 0 => {
                    if i != start || !args.is_empty() {
                        args.push(input[start..i].to_vec());
                    }
                    return Ok((args, separators, i));
                }
                ")" => nesting -= 1,
                "," if nesting == 0 => {
                    args.push(input[start..i].to_vec());
                    separators.push(input[i].clone());
                    start = i + 1;
                }
                _ => {}
            }
        }
        Err(self.error("unterminated macro invocation"))
    }

    fn paste(&self, input: Vec<Token>) -> Result<Vec<Token>, Diagnostic> {
        let mut result: Vec<Token> = Vec::new();
        let mut iter = input.into_iter();
        while let Some(token) = iter.next() {
            if is_paste(&token) {
                let mut left = result
                    .pop()
                    .ok_or_else(|| self.error("## cannot start a replacement list"))?;
                let right = iter
                    .next()
                    .ok_or_else(|| self.error("## cannot end a replacement list"))?;
                left.text.push_str(&right.text);
                if !left.text.is_empty() && tokens(&left.text, self.file, left.line)?.len() != 1 {
                    return Err(
                        self.error("token pasting does not produce a valid preprocessing token")
                    );
                }
                left.hidden = left.hidden.intersection(&right.hidden).cloned().collect();
                result.push(left);
            } else {
                result.push(token);
            }
        }
        result.retain(|t| !t.text.is_empty());
        Ok(result)
    }
}

fn is_paste(token: &Token) -> bool {
    token.paste
}
fn replacement_tokens(text: &str, file: FileId, line: usize) -> Result<Vec<Token>, Diagnostic> {
    let mut result = tokens(text, file, line)?;
    for token in &mut result {
        token.paste = matches!(token.text.as_str(), "##" | "%:%:");
    }
    Ok(result)
}
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(is_ident_start) && chars.all(is_ident_continue)
}

pub(super) fn expand(
    line: &str,
    macros: &HashMap<String, MacroDefinition>,
    active: &HashSet<String>,
    mode: ExpansionMode,
    file: FileId,
    line_number: usize,
    filename: &str,
) -> Result<String, Diagnostic> {
    let mut input = tokens(line, file, line_number)?;
    for token in &mut input {
        token.hidden.extend(active.iter().cloned());
    }
    let expanded = Expander {
        macros,
        file,
        filename,
        mode,
    }
    .expand(input, 0)?;
    render(&expanded, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preprocess(source: &str) -> String {
        let mut sources = SourceManager::default();
        let root = sources.add_file(PathBuf::from("example.c"), source.into());
        let output = Preprocessor::new(PathBuf::from("."), Vec::new())
            .preprocess(&mut sources, root)
            .unwrap();
        sources.file(output.file_id).text().to_owned()
    }

    #[test]
    fn n1570_rescanning_example_matches_the_actual_preprocessing_tokens() {
        let output = preprocess(
            r#"
#define x 3
#define f(a) f(x * (a))
#undef x
#define x 2
#define g f
#define z z[0]
#define h g(~
#define m(a) a(w)
#define w 0,1
#define t(a) a
#define p() int
#define q(x) x
#define r(x,y) x ## y
#define str(x) # x
f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);
g(x+(3,4)-w) | h 5) & m
    (f)^m(m);
p() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };
char c[2][6] = { str(hello), str() };
"#,
        );
        let expected = r#"
f(2 * (y+1)) + f(2 * (f(2 * (z[0])))) % f(2 * (0)) + t(1);
f(2 * (2+(3,4)-0,1)) | f(2 * (~ 5)) & f(2 * (0,1))^m(0,1);
int i[] = { 1, 23, 4, 5, };
char c[2][6] = { "hello", "" };
"#;
        let spellings = |s: &str| {
            tokens(s, FileId(0), 1)
                .unwrap()
                .into_iter()
                .map(|t| t.text)
                .collect::<Vec<_>>()
        };
        assert_eq!(spellings(&output), spellings(expected));
    }

    #[test]
    fn replacement_does_not_merge_tokens_or_execute_argument_pastes() {
        let output =
            preprocess("#define ID(x) x\n#define A +\n#define DOT .\nID(a ## b)\nA+\nDOT..\n");
        assert!(output.contains("a ## b"));
        assert!(output.contains("+ +"));
        assert!(!output.contains("..."));
    }
}
