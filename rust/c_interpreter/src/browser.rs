//! Browser request decoding, virtual source adaptation, and the Wasm ABI.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::browser_json::{
    cboxes_diagnostic_json, cboxes_error_json, cboxes_expression_success_json,
    cboxes_implicit_main_json, cboxes_success_json,
};
use crate::diag::Diagnostic;
use crate::interpreter::ProgramOutput;
use crate::{
    RunExpressionEvalRequest, RunOptions, VirtualSource, run_source_with_options,
    run_virtual_sources_with_options,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceDisplay {
    pub(crate) line_offset: usize,
    pub(crate) line_count: usize,
    pub(crate) eof_column: usize,
    pub(crate) normalized_final_newline: bool,
}

impl SourceDisplay {
    pub(crate) fn unbounded() -> Self {
        Self {
            line_offset: 0,
            line_count: usize::MAX,
            eof_column: 0,
            normalized_final_newline: false,
        }
    }
}

pub(crate) type SourceDisplayMap = HashMap<String, SourceDisplay>;

#[derive(Clone, Debug, Default)]
pub(crate) struct CboxesImplicitMain {
    pub(crate) applied: bool,
    pub(crate) notice: Option<String>,
}

#[derive(Debug)]
struct CboxesVirtualRun {
    result: Result<ProgramOutput, Diagnostic>,
    source_display: SourceDisplayMap,
    implicit_main: CboxesImplicitMain,
}

fn cboxes_source_line_count(source: &str) -> usize {
    source.split('\n').count().max(1)
}

fn cboxes_source_display(source: &str, line_offset: usize) -> SourceDisplay {
    SourceDisplay {
        line_offset,
        line_count: cboxes_source_line_count(source),
        eof_column: source.rsplit('\n').next().unwrap_or("").len(),
        normalized_final_newline: !source.is_empty() && !source.ends_with('\n'),
    }
}

fn cboxes_decode_file_bundle(bytes: &[u8]) -> Result<Vec<VirtualSource>, String> {
    fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<usize, String> {
        let end = cursor
            .checked_add(4)
            .ok_or_else(|| "the file bundle is too large".to_owned())?;
        let raw = bytes
            .get(*cursor..end)
            .ok_or_else(|| "the file bundle ended unexpectedly".to_owned())?;
        *cursor = end;
        Ok(u32::from_le_bytes(raw.try_into().expect("four bytes")) as usize)
    }

    let mut cursor = 0;
    let count = read_u32(bytes, &mut cursor)?;
    if count == 0 {
        return Err("the project needs at least one file".to_owned());
    }
    if count > 256 {
        return Err("the project has too many files".to_owned());
    }

    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let path_len = read_u32(bytes, &mut cursor)?;
        let source_len = read_u32(bytes, &mut cursor)?;
        let path_end = cursor
            .checked_add(path_len)
            .ok_or_else(|| "the file path is too large".to_owned())?;
        let path_bytes = bytes
            .get(cursor..path_end)
            .ok_or_else(|| "the file bundle ended inside a file name".to_owned())?;
        cursor = path_end;
        let source_end = cursor
            .checked_add(source_len)
            .ok_or_else(|| "the source file is too large".to_owned())?;
        let source_bytes = bytes
            .get(cursor..source_end)
            .ok_or_else(|| "the file bundle ended inside a source file".to_owned())?;
        cursor = source_end;

        let raw_path = std::str::from_utf8(path_bytes)
            .map_err(|_| "a file name is not valid UTF-8".to_owned())?;
        let path = cboxes_normalize_virtual_path(raw_path)?;
        let source = std::str::from_utf8(source_bytes)
            .map_err(|_| format!("{} is not valid UTF-8", path.display()))?
            .to_owned();
        files.push((path, source));
    }
    if cursor != bytes.len() {
        return Err("the file bundle contains unexpected trailing data".to_owned());
    }
    Ok(files)
}

fn cboxes_normalize_virtual_path(raw_path: &str) -> Result<PathBuf, String> {
    if raw_path.trim().is_empty() {
        return Err("file names cannot be empty".to_owned());
    }
    if raw_path.contains('\0') {
        return Err("file names cannot contain NUL characters".to_owned());
    }
    let mut path = PathBuf::new();
    for component in Path::new(raw_path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => path.push(part),
            Component::ParentDir => {
                if !path.pop() {
                    return Err(format!(
                        "file name {raw_path:?} goes above the project root"
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("file name {raw_path:?} must be relative"));
            }
        }
    }
    if path.as_os_str().is_empty() {
        return Err("file names cannot be empty".to_owned());
    }
    Ok(path)
}

fn cboxes_prepare_virtual_sources(
    mut files: Vec<VirtualSource>,
    implicit_main_requested: bool,
) -> Result<(Vec<VirtualSource>, SourceDisplayMap, CboxesImplicitMain), String> {
    let c_files = files
        .iter()
        .enumerate()
        .filter(|(_, (path, _))| path.extension().is_some_and(|extension| extension == "c"))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if c_files.is_empty() {
        return Err("the project needs at least one .c file".to_owned());
    }

    let mut source_display = files
        .iter()
        .map(|(path, source)| (path.display().to_string(), cboxes_source_display(source, 0)))
        .collect::<HashMap<_, _>>();

    for (_, source) in &mut files {
        if !source.is_empty() && !source.ends_with('\n') {
            source.push('\n');
        }
    }

    if !implicit_main_requested {
        return Ok((
            files,
            source_display,
            CboxesImplicitMain {
                applied: false,
                notice: None,
            },
        ));
    }
    let entry_index = c_files[0];
    let entry_path = files[entry_index].0.display().to_string();
    files[entry_index].1 = cboxes_wrap_entire_program_in_main(&files[entry_index].1);
    if let Some(display) = source_display.get_mut(&entry_path) {
        display.line_offset = 1;
    }
    Ok((
        files,
        source_display,
        CboxesImplicitMain {
            applied: true,
            notice: None,
        },
    ))
}

fn cboxes_run_virtual_sources(
    files: Vec<VirtualSource>,
    options: &RunOptions,
    implicit_main_requested: bool,
) -> Result<CboxesVirtualRun, String> {
    let (prepared, source_display, implicit_main) =
        cboxes_prepare_virtual_sources(files.clone(), implicit_main_requested)?;

    if !implicit_main.applied {
        return Ok(CboxesVirtualRun {
            result: run_virtual_sources_with_options(&prepared, options),
            source_display,
            implicit_main,
        });
    }

    let result = run_virtual_sources_with_options(&prepared, options);
    let notice = if result.is_err() {
        let (unwrapped, _, _) = cboxes_prepare_virtual_sources(files, false)?;
        cboxes_run_virtual_sources_without_expression(&unwrapped, options)
            .is_ok()
            .then(|| {
                "This program works with implicit main off. Try turning off Implicit main."
                    .to_owned()
            })
    } else {
        None
    };
    Ok(CboxesVirtualRun {
        result,
        source_display,
        implicit_main: CboxesImplicitMain {
            applied: true,
            notice,
        },
    })
}

fn cboxes_run_virtual_sources_without_expression(
    files: &[VirtualSource],
    options: &RunOptions,
) -> Result<ProgramOutput, Diagnostic> {
    if options.expression_eval.is_none() {
        run_virtual_sources_with_options(files, options)
    } else {
        run_virtual_sources_with_options(
            files,
            &RunOptions {
                expression_eval: None,
                ..options.clone()
            },
        )
    }
}

pub(crate) const CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT: usize = 256;
static CBOXES_LAST_RESULT_LEN: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn cboxes_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let mut buffer = Vec::<u8>::with_capacity(len);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

/// Releases a buffer returned by `cboxes_alloc` or a JSON-producing browser
/// ABI function.
///
/// # Safety
///
/// `ptr` must not already have been freed. For a non-empty buffer, it must be
/// the exact pointer returned by this crate and `len` must be its original
/// allocation length. A JSON result's length is reported by
/// `cboxes_last_result_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_free(ptr: *mut u8, len: usize) {
    if len == 0 || ptr.is_null() {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cboxes_last_result_len() -> usize {
    CBOXES_LAST_RESULT_LEN.load(Ordering::Relaxed)
}

const CBOXES_BRIDGE_SCHEMA_ID: u32 = 1;
const CBOXES_BRIDGE_HEADER_WORDS: usize = 10;
const CBOXES_BRIDGE_FLAG_IMPLICIT_MAIN: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CboxesBridgeOperation {
    RunSource,
    RunFiles,
    EvaluateSource,
    EvaluateFiles,
}

impl CboxesBridgeOperation {
    fn decode(value: u32) -> Result<Self, String> {
        match value {
            0 => Ok(Self::RunSource),
            1 => Ok(Self::RunFiles),
            2 => Ok(Self::EvaluateSource),
            3 => Ok(Self::EvaluateFiles),
            _ => Err(format!("unsupported interpreter operation {value}")),
        }
    }

    fn evaluates_expression(self) -> bool {
        matches!(self, Self::EvaluateSource | Self::EvaluateFiles)
    }

    fn uses_file_bundle(self) -> bool {
        matches!(self, Self::RunFiles | Self::EvaluateFiles)
    }
}

#[derive(Debug)]
struct CboxesBridgeRequest<'a> {
    operation: CboxesBridgeOperation,
    implicit_main: bool,
    synthetic_address_base: u32,
    event_index: usize,
    execution_step_limit: usize,
    execution_trace_following_limit: usize,
    primary: &'a [u8],
    expression: &'a str,
    stdin: &'a str,
}

impl<'a> CboxesBridgeRequest<'a> {
    fn decode(bytes: &'a [u8]) -> Result<Self, String> {
        let header_bytes = CBOXES_BRIDGE_HEADER_WORDS * std::mem::size_of::<u32>();
        let header = bytes
            .get(..header_bytes)
            .ok_or_else(|| "the interpreter request header is incomplete".to_owned())?;
        let word = |index: usize| {
            let start = index * 4;
            u32::from_le_bytes(header[start..start + 4].try_into().expect("four bytes"))
        };
        let schema_id = word(0);
        if schema_id != CBOXES_BRIDGE_SCHEMA_ID {
            return Err(format!(
                "interpreter request schema {schema_id} does not match {CBOXES_BRIDGE_SCHEMA_ID}"
            ));
        }
        let operation = CboxesBridgeOperation::decode(word(1))?;
        let flags = word(2);
        if flags & !CBOXES_BRIDGE_FLAG_IMPLICIT_MAIN != 0 {
            return Err(format!("unsupported interpreter request flags 0x{flags:x}"));
        }
        let lengths = [word(7) as usize, word(8) as usize, word(9) as usize];
        let payload_len = lengths.iter().try_fold(0usize, |total, length| {
            total
                .checked_add(*length)
                .ok_or_else(|| "the interpreter request is too large".to_owned())
        })?;
        if bytes.len() != header_bytes.saturating_add(payload_len) {
            return Err("the interpreter request payload length is invalid".to_owned());
        }
        let mut cursor = header_bytes;
        let mut take = |length: usize| {
            let start = cursor;
            cursor += length;
            &bytes[start..cursor]
        };
        let primary = take(lengths[0]);
        let expression_bytes = take(lengths[1]);
        let stdin_bytes = take(lengths[2]);
        let expression = std::str::from_utf8(expression_bytes)
            .map_err(|_| "expression is not valid UTF-8".to_owned())?;
        let stdin =
            std::str::from_utf8(stdin_bytes).map_err(|_| "stdin is not valid UTF-8".to_owned())?;
        if !operation.evaluates_expression() && !expression.is_empty() {
            return Err("a run request cannot contain an expression".to_owned());
        }
        Ok(Self {
            operation,
            implicit_main: flags & CBOXES_BRIDGE_FLAG_IMPLICIT_MAIN != 0,
            synthetic_address_base: word(3),
            event_index: word(4) as usize,
            execution_step_limit: word(5).max(1) as usize,
            execution_trace_following_limit: word(6).max(1) as usize,
            primary,
            expression,
            stdin,
        })
    }

    fn run_options(&self) -> RunOptions {
        RunOptions {
            stdin: self.stdin.to_owned(),
            expression_eval: self.operation.evaluates_expression().then(|| {
                RunExpressionEvalRequest {
                    expression: self.expression.to_owned(),
                    event_index: self.event_index,
                }
            }),
            synthetic_address_base: self.synthetic_address_base.into(),
            execution_step_limit: Some(self.execution_step_limit),
            execution_trace_following_limit: self.execution_trace_following_limit,
            ..RunOptions::default()
        }
    }
}

/// Executes one versioned browser-bridge request and returns an allocated JSON
/// result. The request contains a fixed-width little-endian header followed by
/// the source or file bundle, expression, and standard-input byte strings.
///
/// # Safety
///
/// When `len` is nonzero, `ptr` must be valid for reads of `len` bytes for
/// the duration of this call. Free the returned buffer with `cboxes_free`,
/// using the length from `cboxes_last_result_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_execute(ptr: *const u8, len: usize) -> *mut u8 {
    let bytes = if len == 0 {
        &[]
    } else if ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null interpreter request pointer",
        ));
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let request = match CboxesBridgeRequest::decode(bytes) {
        Ok(request) => request,
        Err(message) => {
            return cboxes_store_json(cboxes_error_json("compile", &message));
        }
    };
    let json = if request.operation.uses_file_bundle() {
        cboxes_execute_file_request(&request)
    } else {
        cboxes_execute_source_request(&request)
    };
    cboxes_store_json(json)
}

fn cboxes_execute_source_request(request: &CboxesBridgeRequest<'_>) -> String {
    let input = match std::str::from_utf8(request.primary) {
        Ok(input) => input,
        Err(_) => return cboxes_error_json("compile", "source is not valid UTF-8"),
    };
    let mut source = input.to_owned();
    let line_offset = if cboxes_has_explicit_main(&source) {
        if !source.is_empty() && !source.ends_with('\n') {
            source.push('\n');
        }
        0
    } else {
        source = cboxes_wrap_implicit_main(&source);
        1
    };
    let options = request.run_options();
    let result =
        std::panic::catch_unwind(|| run_source_with_options("program.c", source, &options));
    let source_display = HashMap::from([(
        "program.c".to_owned(),
        cboxes_source_display(input, line_offset),
    )]);
    match result {
        Ok(Ok(result)) => cboxes_bridge_success_json(
            result,
            &source_display,
            request.operation.evaluates_expression(),
        ),
        Ok(Err(diagnostic)) => cboxes_diagnostic_json(&diagnostic, &source_display),
        Err(_) => cboxes_error_json(
            "compile",
            if request.operation.evaluates_expression() {
                "internal interpreter error while evaluating this expression"
            } else {
                "internal interpreter error while running this program"
            },
        ),
    }
}

fn cboxes_execute_file_request(request: &CboxesBridgeRequest<'_>) -> String {
    let files = match cboxes_decode_file_bundle(request.primary) {
        Ok(files) => files,
        Err(message) => {
            return cboxes_implicit_main_json(
                cboxes_error_json("compile", &message),
                &CboxesImplicitMain::default(),
            );
        }
    };
    let options = request.run_options();
    let result = std::panic::catch_unwind(|| {
        cboxes_run_virtual_sources(files, &options, request.implicit_main)
    });
    match result {
        Ok(Ok(run)) => {
            let json = match run.result {
                Ok(result) => cboxes_bridge_success_json(
                    result,
                    &run.source_display,
                    request.operation.evaluates_expression(),
                ),
                Err(diagnostic) => cboxes_diagnostic_json(&diagnostic, &run.source_display),
            };
            cboxes_implicit_main_json(json, &run.implicit_main)
        }
        Ok(Err(message)) => cboxes_implicit_main_json(
            cboxes_error_json("compile", &message),
            &CboxesImplicitMain::default(),
        ),
        Err(_) => cboxes_implicit_main_json(
            cboxes_error_json(
                "compile",
                if request.operation.evaluates_expression() {
                    "internal interpreter error while evaluating this expression"
                } else {
                    "internal interpreter error while running this project"
                },
            ),
            &CboxesImplicitMain::default(),
        ),
    }
}

fn cboxes_bridge_success_json(
    result: ProgramOutput,
    source_display: &SourceDisplayMap,
    expects_expression: bool,
) -> String {
    if !expects_expression {
        return cboxes_success_json(&result, source_display);
    }
    match result.expression {
        Some(expression) => cboxes_expression_success_json(&expression),
        None => cboxes_error_json(
            "compile",
            "No program state is available for that expression yet.",
        ),
    }
}
fn cboxes_has_explicit_main(source: &str) -> bool {
    fn skip_quoted(bytes: &[u8], mut index: usize, quote: u8) -> usize {
        index += 1;
        while index < bytes.len() {
            match bytes[index] {
                b'\\' => index = (index + 2).min(bytes.len()),
                byte if byte == quote => return index + 1,
                _ => index += 1,
            }
        }
        index
    }

    fn skip_trivia(bytes: &[u8], mut index: usize) -> usize {
        loop {
            while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                index += 1;
            }
            if bytes.get(index..index + 2) == Some(b"//") {
                index += 2;
                while bytes.get(index).is_some_and(|byte| *byte != b'\n') {
                    index += 1;
                }
            } else if bytes.get(index..index + 2) == Some(b"/*") {
                index += 2;
                while index < bytes.len() && bytes.get(index..index + 2) != Some(b"*/") {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            } else {
                return index;
            }
        }
    }

    let spliced = source.replace("\\\r\n", "").replace("\\\n", "");
    let bytes = spliced.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes.get(i..i + 2) == Some(b"//") || bytes.get(i..i + 2) == Some(b"/*") {
            i = skip_trivia(bytes, i);
        } else if matches!(bytes[i], b'\'' | b'"') {
            i = skip_quoted(bytes, i, bytes[i]);
        } else if bytes[i] == b'_' || bytes[i].is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while bytes
                .get(i)
                .is_some_and(|byte| *byte == b'_' || byte.is_ascii_alphanumeric())
            {
                i += 1;
            }
            if &bytes[start..i] == b"main" && bytes.get(skip_trivia(bytes, i)) == Some(&b'(') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

fn cboxes_wrap_implicit_main(source: &str) -> String {
    let mut wrapped = String::with_capacity(source.len() + 32);
    let mut body_start = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            wrapped.push_str(line);
            body_start += line.len();
        } else {
            break;
        }
    }
    if body_start != 0 && !wrapped.ends_with('\n') {
        wrapped.push('\n');
    }
    wrapped.push_str("int main(void) {\n");
    wrapped.push_str(&source[body_start..]);
    if !wrapped.ends_with('\n') {
        wrapped.push('\n');
    }
    wrapped.push_str("return 0;\n}\n");
    wrapped
}

fn cboxes_wrap_entire_program_in_main(source: &str) -> String {
    let mut wrapped = String::with_capacity(source.len() + 32);
    wrapped.push_str("int main(void) {\n");
    wrapped.push_str(source);
    if !wrapped.ends_with('\n') {
        wrapped.push('\n');
    }
    wrapped.push_str("return 0;\n}\n");
    wrapped
}

fn cboxes_store_json(json: String) -> *mut u8 {
    let mut bytes = json.into_bytes();
    let ptr = bytes.as_mut_ptr();
    CBOXES_LAST_RESULT_LEN.store(bytes.len(), Ordering::Relaxed);
    std::mem::forget(bytes);
    ptr
}
