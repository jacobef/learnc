mod ast;
mod diag;
mod integer;
mod interpreter;
mod lexer;
mod number;
mod parser;
mod preprocess;
mod source;
mod token;
mod types;

use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ast::{
    Block, BlockItem, Declaration, Designator, Expr, ExternalDeclaration, ForInit, FunctionDecl,
    FunctionDef, Initializer, InitializerItem, Linkage, Parameter, Statement, StorageClass,
    SwitchLabel, TranslationUnit,
};
use diag::Diagnostic;
use interpreter::{
    Interpreter, ProgramBlocked, ProgramExecutionLimit, ProgramExpressionEvalRequest,
    ProgramExpressionResult, ProgramOutput, ProgramSourceLocation, ProgramSourceRange,
    ProgramStateBox, ProgramTraceEvent, ProgramTypeHelpNode, ProgramTypeInfo, ProgramValueLiteral,
};
use lexer::Lexer;
use parser::Parser;
use preprocess::Preprocessor;
use source::{FileId, SourceManager, Span};
use types::{CType, EnumType, RecordMember, RecordType};

#[derive(Debug)]
struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_status: i32,
    pub state: Vec<ProgramStateBox>,
    pub trace: Vec<ProgramTraceEvent>,
    pub main_close: ProgramSourceLocation,
    pub blocked: Option<ProgramBlocked>,
    pub execution_limit: Option<ProgramExecutionLimit>,
    pub expression: Option<ProgramExpressionResult>,
}

#[derive(Clone, Debug)]
struct RunExpressionEvalRequest {
    pub expression: String,
    pub event_index: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum UbDetectionMode {
    /// Preserve the interpreter's standards-oriented behavior.
    #[default]
    Standard,
    /// Also reject a small set of legal-but-dangerous beginner patterns.
    /// This remains an internal execution option until the frontend deliberately exposes it.
    Simple,
}

#[derive(Clone, Debug)]
struct RunOptions {
    #[cfg(test)]
    pub include_dirs: Vec<PathBuf>,
    pub stdin: String,
    pub expression_eval: Option<RunExpressionEvalRequest>,
    pub ub_detection_mode: UbDetectionMode,
    pub capture_visualization: bool,
    pub synthetic_address_base: u64,
    pub execution_step_limit: Option<usize>,
    pub execution_trace_following_limit: usize,
}

type VirtualSource = (PathBuf, String);

// The recursive evaluator gets the same stack budget in debug and release native builds. The Wasm
// build script reserves the same amount of linear memory for its stack.
#[cfg(not(target_os = "wasi"))]
const INTERPRETER_STACK_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct SourceDisplay {
    line_offset: usize,
    line_count: usize,
    eof_column: usize,
    normalized_final_newline: bool,
}

impl SourceDisplay {
    fn unbounded() -> Self {
        Self {
            line_offset: 0,
            line_count: usize::MAX,
            eof_column: 0,
            normalized_final_newline: false,
        }
    }
}

type SourceDisplayMap = HashMap<String, SourceDisplay>;

#[derive(Clone, Debug)]
struct CboxesImplicitMain {
    applied: bool,
    notice: Option<String>,
}

#[derive(Debug)]
struct CboxesVirtualRun {
    result: Result<RunResult, Diagnostic>,
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

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            #[cfg(test)]
            include_dirs: Vec::new(),
            stdin: String::new(),
            expression_eval: None,
            ub_detection_mode: UbDetectionMode::Standard,
            capture_visualization: true,
            synthetic_address_base: 0x1000,
            execution_step_limit: None,
            execution_trace_following_limit: CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT,
        }
    }
}

#[cfg(test)]
fn run_file(path: impl AsRef<Path>) -> Result<RunResult, Diagnostic> {
    run_file_with_options(path, &RunOptions::default())
}

#[cfg(test)]
fn run_file_with_options(
    path: impl AsRef<Path>,
    options: &RunOptions,
) -> Result<RunResult, Diagnostic> {
    run_files_with_options([path.as_ref().to_path_buf()], options)
}

#[cfg(test)]
fn run_source(
    virtual_path: impl Into<PathBuf>,
    source: impl Into<String>,
) -> Result<RunResult, Diagnostic> {
    run_source_with_options(virtual_path, source, &RunOptions::default())
}

fn run_source_with_options(
    virtual_path: impl Into<PathBuf>,
    source: impl Into<String>,
    options: &RunOptions,
) -> Result<RunResult, Diagnostic> {
    let virtual_path = virtual_path.into();
    let cwd = virtual_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut sources = SourceManager::default();
    let file_id = sources.add_file(virtual_path, source.into());
    run_with_sources(&mut sources, file_id, &cwd, options)
}

fn run_virtual_sources_with_options(
    files: &[VirtualSource],
    options: &RunOptions,
) -> Result<RunResult, Diagnostic> {
    let (mut sources, translation_unit) = parse_virtual_sources_with_options(files, options)?;
    run_translation_unit(&mut sources, translation_unit, options)
}

fn parse_virtual_sources_with_options(
    files: &[VirtualSource],
    options: &RunOptions,
) -> Result<(SourceManager, TranslationUnit), Diagnostic> {
    if files.is_empty() {
        return Err(Diagnostic::error(
            "at least one input file is required",
            Span::new(FileId(0), 0, 0),
        ));
    }

    let mut sources = SourceManager::default();
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for (path, text) in files {
        if !seen.insert(path.clone()) {
            return Err(Diagnostic::error(
                format!("the file {} was provided more than once", path.display()),
                Span::new(FileId(0), 0, 0),
            ));
        }
        let file_id = sources.add_file(path.clone(), text.clone());
        if path.extension().is_some_and(|extension| extension == "c") {
            roots.push((file_id, path.clone()));
        }
    }
    if roots.is_empty() {
        return Err(Diagnostic::error(
            "the project needs at least one .c file",
            Span::new(FileId(0), 0, 0),
        ));
    }

    let mut units = Vec::with_capacity(roots.len());
    for (file_id, path) in roots {
        let cwd = path.parent().unwrap_or_else(|| Path::new(""));
        units.push(parse_translation_unit(&mut sources, file_id, cwd, options)?);
    }
    let translation_unit =
        merge_translation_units(units).map_err(|diag| diag.with_sources(&sources))?;
    Ok((sources, translation_unit))
}

#[cfg(test)]
fn run_files<I, P>(paths: I) -> Result<RunResult, Diagnostic>
where
    I: IntoIterator<Item = P>,
    P: Into<PathBuf>,
{
    run_files_with_options(paths, &RunOptions::default())
}

#[cfg(test)]
fn run_files_with_options<I, P>(paths: I, options: &RunOptions) -> Result<RunResult, Diagnostic>
where
    I: IntoIterator<Item = P>,
    P: Into<PathBuf>,
{
    let paths = paths.into_iter().map(Into::into).collect::<Vec<_>>();
    if paths.is_empty() {
        return Err(Diagnostic::error(
            "at least one input file is required",
            Span::new(FileId(0), 0, 0),
        ));
    }
    let mut sources = SourceManager::default();
    let mut units = Vec::new();
    for path in &paths {
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));
        let text = fs::read_to_string(path).map_err(|err| Diagnostic::io(path.clone(), err))?;
        let file_id = sources.add_file(path.clone(), text);
        units.push(parse_translation_unit(&mut sources, file_id, cwd, options)?);
    }
    let translation_unit =
        merge_translation_units(units).map_err(|diag| diag.with_sources(&sources))?;
    run_translation_unit(&mut sources, translation_unit, options)
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
) -> Result<RunResult, Diagnostic> {
    if options.expression_eval.is_none() {
        run_virtual_sources_with_options(files, options)
    } else {
        run_virtual_sources_with_options(
            files,
            &RunOptions {
                #[cfg(test)]
                include_dirs: options.include_dirs.clone(),
                stdin: options.stdin.clone(),
                expression_eval: None,
                ub_detection_mode: options.ub_detection_mode,
                capture_visualization: options.capture_visualization,
                synthetic_address_base: options.synthetic_address_base,
                execution_step_limit: options.execution_step_limit,
                execution_trace_following_limit: options.execution_trace_following_limit,
            },
        )
    }
}

const CBOXES_BROWSER_EXECUTION_STEP_LIMIT: usize = 10_000;
const CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT: usize = 256;
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_run_source(
    ptr: *const u8,
    len: usize,
    stdin_ptr: *const u8,
    stdin_len: usize,
    synthetic_address_base: u32,
) -> *mut u8 {
    unsafe {
        cboxes_run_source_configured(
            ptr,
            len,
            stdin_ptr,
            stdin_len,
            synthetic_address_base,
            true,
            Some(CBOXES_BROWSER_EXECUTION_STEP_LIMIT),
        )
    }
}

/// Runs a single source file without producing website visualization data or
/// imposing the browser step limit. The returned JSON uses the same ownership
/// and error-reporting contract as `cboxes_run_source`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_run_source_without_visualization(
    ptr: *const u8,
    len: usize,
    stdin_ptr: *const u8,
    stdin_len: usize,
    synthetic_address_base: u32,
) -> *mut u8 {
    unsafe {
        cboxes_run_source_configured(
            ptr,
            len,
            stdin_ptr,
            stdin_len,
            synthetic_address_base,
            false,
            None,
        )
    }
}

unsafe fn cboxes_run_source_configured(
    ptr: *const u8,
    len: usize,
    stdin_ptr: *const u8,
    stdin_len: usize,
    synthetic_address_base: u32,
    capture_visualization: bool,
    execution_step_limit: Option<usize>,
) -> *mut u8 {
    let input = if len == 0 {
        ""
    } else if ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null source pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(ptr, len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "source is not valid UTF-8",
                    None,
                ));
            }
        }
    };
    let stdin = if stdin_len == 0 {
        ""
    } else if stdin_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null stdin pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(stdin_ptr, stdin_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "stdin is not valid UTF-8",
                    None,
                ));
            }
        }
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
    let result = std::panic::catch_unwind(|| {
        run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                #[cfg(test)]
                include_dirs: Vec::new(),
                stdin: stdin.to_owned(),
                expression_eval: None,
                ub_detection_mode: UbDetectionMode::Standard,
                capture_visualization,
                synthetic_address_base: synthetic_address_base.into(),
                execution_step_limit,
                execution_trace_following_limit: CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT,
            },
        )
    });
    let source_display = HashMap::from([(
        "program.c".to_owned(),
        cboxes_source_display(input, line_offset),
    )]);
    let json = match result {
        Ok(Ok(result)) => cboxes_success_json(&result, &source_display),
        Ok(Err(diag)) => cboxes_diagnostic_json(&diag, &source_display),
        Err(_) => cboxes_error_json(
            "compile",
            "internal interpreter error while running this program",
            None,
        ),
    };
    cboxes_store_json(json)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_run_files(
    bundle_ptr: *const u8,
    bundle_len: usize,
    stdin_ptr: *const u8,
    stdin_len: usize,
    synthetic_address_base: u32,
    implicit_main_requested: u32,
    execution_step_limit: u32,
    execution_trace_following_limit: u32,
) -> *mut u8 {
    if bundle_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null file bundle pointer",
            None,
        ));
    }
    let bundle = unsafe { std::slice::from_raw_parts(bundle_ptr, bundle_len) };
    let files = match cboxes_decode_file_bundle(bundle) {
        Ok(files) => files,
        Err(message) => {
            return cboxes_store_json(cboxes_error_json("compile", &message, None));
        }
    };
    let stdin = if stdin_len == 0 {
        ""
    } else if stdin_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null stdin pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(stdin_ptr, stdin_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "stdin is not valid UTF-8",
                    None,
                ));
            }
        }
    };

    let options = RunOptions {
        #[cfg(test)]
        include_dirs: Vec::new(),
        stdin: stdin.to_owned(),
        expression_eval: None,
        ub_detection_mode: UbDetectionMode::Standard,
        capture_visualization: true,
        synthetic_address_base: synthetic_address_base.into(),
        execution_step_limit: Some(execution_step_limit.max(1) as usize),
        execution_trace_following_limit: execution_trace_following_limit.max(1) as usize,
    };
    let result = std::panic::catch_unwind(|| {
        cboxes_run_virtual_sources(files, &options, implicit_main_requested != 0)
    });
    let json = match result {
        Ok(Ok(run)) => {
            let json = match run.result {
                Ok(result) => cboxes_success_json(&result, &run.source_display),
                Err(diag) => cboxes_diagnostic_json(&diag, &run.source_display),
            };
            cboxes_implicit_main_json(json, &run.implicit_main)
        }
        Ok(Err(message)) => cboxes_error_json("compile", &message, None),
        Err(_) => cboxes_error_json(
            "compile",
            "internal interpreter error while running this project",
            None,
        ),
    };
    cboxes_store_json(json)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_eval_expression(
    source_ptr: *const u8,
    source_len: usize,
    expr_ptr: *const u8,
    expr_len: usize,
    event_index: usize,
    stdin_ptr: *const u8,
    stdin_len: usize,
    synthetic_address_base: u32,
) -> *mut u8 {
    let input = if source_len == 0 {
        ""
    } else if source_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null source pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(source_ptr, source_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "source is not valid UTF-8",
                    None,
                ));
            }
        }
    };
    let stdin = if stdin_len == 0 {
        ""
    } else if stdin_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null stdin pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(stdin_ptr, stdin_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "stdin is not valid UTF-8",
                    None,
                ));
            }
        }
    };
    let expression = if expr_len == 0 {
        ""
    } else if expr_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null expression pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(expr_ptr, expr_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "expression is not valid UTF-8",
                    None,
                ));
            }
        }
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
    let expression = expression.to_owned();
    let result = std::panic::catch_unwind(|| {
        run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                #[cfg(test)]
                include_dirs: Vec::new(),
                stdin: stdin.to_owned(),
                expression_eval: Some(RunExpressionEvalRequest {
                    expression,
                    event_index,
                }),
                ub_detection_mode: UbDetectionMode::Standard,
                capture_visualization: true,
                synthetic_address_base: synthetic_address_base.into(),
                execution_step_limit: Some(CBOXES_BROWSER_EXECUTION_STEP_LIMIT),
                execution_trace_following_limit: CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT,
            },
        )
    });
    let source_display = HashMap::from([(
        "program.c".to_owned(),
        cboxes_source_display(input, line_offset),
    )]);
    let json = match result {
        Ok(Ok(result)) => match result.expression {
            Some(expression) => cboxes_expression_success_json(&expression),
            None => cboxes_error_json(
                "compile",
                "No program state is available for that expression yet.",
                None,
            ),
        },
        Ok(Err(diag)) => cboxes_diagnostic_json(&diag, &source_display),
        Err(_) => cboxes_error_json(
            "compile",
            "internal interpreter error while evaluating this expression",
            None,
        ),
    };
    cboxes_store_json(json)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cboxes_eval_expression_files(
    bundle_ptr: *const u8,
    bundle_len: usize,
    expr_ptr: *const u8,
    expr_len: usize,
    event_index: usize,
    stdin_ptr: *const u8,
    stdin_len: usize,
    synthetic_address_base: u32,
    implicit_main_requested: u32,
    execution_step_limit: u32,
) -> *mut u8 {
    if bundle_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null file bundle pointer",
            None,
        ));
    }
    let bundle = unsafe { std::slice::from_raw_parts(bundle_ptr, bundle_len) };
    let files = match cboxes_decode_file_bundle(bundle) {
        Ok(files) => files,
        Err(message) => {
            return cboxes_store_json(cboxes_error_json("compile", &message, None));
        }
    };
    let expression = if expr_len == 0 {
        ""
    } else if expr_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null expression pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(expr_ptr, expr_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "expression is not valid UTF-8",
                    None,
                ));
            }
        }
    };
    let stdin = if stdin_len == 0 {
        ""
    } else if stdin_ptr.is_null() {
        return cboxes_store_json(cboxes_error_json(
            "compile",
            "internal error: null stdin pointer",
            None,
        ));
    } else {
        match std::str::from_utf8(unsafe { std::slice::from_raw_parts(stdin_ptr, stdin_len) }) {
            Ok(input) => input,
            Err(_) => {
                return cboxes_store_json(cboxes_error_json(
                    "compile",
                    "stdin is not valid UTF-8",
                    None,
                ));
            }
        }
    };

    let expression = expression.to_owned();
    let options = RunOptions {
        #[cfg(test)]
        include_dirs: Vec::new(),
        stdin: stdin.to_owned(),
        expression_eval: Some(RunExpressionEvalRequest {
            expression,
            event_index,
        }),
        ub_detection_mode: UbDetectionMode::Standard,
        capture_visualization: true,
        synthetic_address_base: synthetic_address_base.into(),
        execution_step_limit: Some(execution_step_limit.max(1) as usize),
        execution_trace_following_limit: CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT,
    };
    let result = std::panic::catch_unwind(|| {
        cboxes_run_virtual_sources(files, &options, implicit_main_requested != 0)
    });
    let json = match result {
        Ok(Ok(run)) => {
            let json = match run.result {
                Ok(result) => match result.expression {
                    Some(expression) => cboxes_expression_success_json(&expression),
                    None => cboxes_error_json(
                        "compile",
                        "No program state is available for that expression yet.",
                        None,
                    ),
                },
                Err(diag) => cboxes_diagnostic_json(&diag, &run.source_display),
            };
            cboxes_implicit_main_json(json, &run.implicit_main)
        }
        Ok(Err(message)) => cboxes_error_json("compile", &message, None),
        Err(_) => cboxes_error_json(
            "compile",
            "internal interpreter error while evaluating this expression",
            None,
        ),
    };
    cboxes_store_json(json)
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

fn cboxes_implicit_main_json(mut json: String, implicit_main: &CboxesImplicitMain) -> String {
    debug_assert!(json.ends_with('}'));
    json.pop();
    json.push_str(",\"implicitMainApplied\":");
    json.push_str(if implicit_main.applied {
        "true"
    } else {
        "false"
    });
    json.push_str(",\"implicitMainNotice\":");
    if let Some(notice) = &implicit_main.notice {
        json.push_str(&cboxes_json_string(notice));
    } else {
        json.push_str("null");
    }
    json.push('}');
    json
}

fn cboxes_success_json(result: &RunResult, source_display: &SourceDisplayMap) -> String {
    format!(
        "{{\"ok\":true,\"stdout\":{},\"stderr\":{},\"exitStatus\":{},\"state\":{},\"trace\":{},\"mainClose\":{},\"blocked\":{},\"executionLimit\":{}}}",
        cboxes_json_string(&result.stdout),
        cboxes_json_string(&result.stderr),
        result.exit_status,
        cboxes_state_json(&result.state),
        cboxes_trace_json(&result.trace, source_display),
        cboxes_source_location_json(&result.main_close, source_display),
        cboxes_blocked_json(result.blocked.as_ref(), source_display),
        cboxes_execution_limit_json(result.execution_limit.as_ref(), source_display),
    )
}

fn cboxes_expression_success_json(result: &ProgramExpressionResult) -> String {
    format!(
        "{{\"ok\":true,\"result\":{}}}",
        cboxes_expression_result_json(result)
    )
}

fn cboxes_expression_result_json(result: &ProgramExpressionResult) -> String {
    let address = result
        .address
        .map(|address| cboxes_json_string(&address.to_string()))
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "{{\"kind\":{},\"type\":{},\"value\":{},\"displayValue\":{},\"exactValue\":{},\"address\":{},\"name\":{},\"valueLiteral\":{},\"typeInfo\":{}}}",
        cboxes_json_string(&result.kind),
        cboxes_json_string(&result.ty),
        cboxes_json_string(&result.value),
        cboxes_json_string(&result.display_value),
        cboxes_json_string(&result.exact_value),
        address,
        cboxes_json_string(&result.name),
        cboxes_value_literal_json(result.value_literal.as_ref()),
        cboxes_type_info_json(&result.type_info),
    )
}

fn cboxes_type_info_json(info: &ProgramTypeInfo) -> String {
    let size = info
        .size
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let align = info
        .align
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned());
    let help = info
        .help
        .as_deref()
        .map(cboxes_json_string)
        .unwrap_or_else(|| "null".to_owned());
    let help_tree = info
        .help_tree
        .as_ref()
        .map(cboxes_type_help_node_json)
        .unwrap_or_else(|| "null".to_owned());
    format!(
        "{{\"kind\":{},\"help\":{},\"helpTypeNames\":{},\"helpTree\":{},\"pointerDepth\":{},\"arrayShape\":{},\"pointeeArrayShape\":{},\"size\":{},\"align\":{}}}",
        cboxes_json_string(&info.kind),
        help,
        cboxes_string_array_json(&info.help_type_names),
        help_tree,
        info.pointer_depth,
        cboxes_usize_array_json(&info.array_shape),
        cboxes_usize_array_json(&info.pointee_array_shape),
        size,
        align,
    )
}

fn cboxes_type_help_node_json(node: &ProgramTypeHelpNode) -> String {
    let type_name = node
        .type_name
        .as_deref()
        .map(cboxes_json_string)
        .unwrap_or_else(|| "null".to_owned());
    let mut children = String::from("[");
    for (index, child) in node.children.iter().enumerate() {
        if index > 0 {
            children.push(',');
        }
        children.push_str(&format!(
            "{{\"relation\":{},\"node\":{}}}",
            cboxes_json_string(&child.relation),
            cboxes_type_help_node_json(&child.node),
        ));
    }
    children.push(']');
    format!(
        "{{\"kind\":{},\"label\":{},\"typeName\":{},\"children\":{}}}",
        cboxes_json_string(&node.kind),
        cboxes_json_string(&node.label),
        type_name,
        children,
    )
}

fn cboxes_string_array_json(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&cboxes_json_string(value));
    }
    out.push(']');
    out
}

fn cboxes_value_literal_json(literal: Option<&ProgramValueLiteral>) -> String {
    let Some(literal) = literal else {
        return "null".to_owned();
    };
    format!(
        "{{\"kind\":{},\"hasSuffix\":{}}}",
        cboxes_json_string(&literal.kind),
        literal.has_suffix,
    )
}

fn cboxes_value_literal_text(tokens: &[token::Token]) -> Option<String> {
    use token::TokenKind;

    match tokens {
        [
            token::Token {
                kind: TokenKind::Number(text),
                ..
            },
            token::Token {
                kind: TokenKind::Eof,
                ..
            },
        ]
        | [
            token::Token {
                kind: TokenKind::Plus | TokenKind::Minus,
                ..
            },
            token::Token {
                kind: TokenKind::Number(text),
                ..
            },
            token::Token {
                kind: TokenKind::Eof,
                ..
            },
        ] => Some(text.clone()),
        _ => None,
    }
}

fn cboxes_diagnostic_json(diag: &Diagnostic, source_display: &SourceDisplayMap) -> String {
    let rendered = diag.render();
    let kind = if rendered.starts_with("undefined behavior:") {
        "ub"
    } else {
        "compile"
    };
    let range = diag
        .display_range()
        .and_then(|range| {
            let file = range.path.to_string_lossy().into_owned();
            cboxes_display_range(
                source_display,
                file,
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            )
        })
        .or_else(|| {
            cboxes_rendered_location(&rendered).and_then(|(file, line, col)| {
                cboxes_display_range(source_display, file, line, col, line, col + 1)
            })
        });
    let annotations = diag
        .display_annotations()
        .iter()
        .filter_map(|annotation| {
            let annotation_range = &annotation.range;
            let file = annotation_range.path.to_string_lossy().into_owned();
            cboxes_display_range(
                source_display,
                file,
                annotation_range.start_line,
                annotation_range.start_column,
                annotation_range.end_line,
                annotation_range.end_column,
            )
            .map(|range| (annotation.id.clone(), range))
        })
        .collect::<Vec<_>>();
    cboxes_error_json_with_annotations(kind, &rendered, range, &annotations)
}

type CboxesDiagnosticRange = (String, usize, usize, usize, usize);

fn cboxes_display_range(
    source_display: &SourceDisplayMap,
    file: String,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
) -> Option<CboxesDiagnosticRange> {
    let display = source_display
        .get(&file)
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let start_line = start_line.saturating_sub(display.line_offset);
    if start_line == display.line_count && display.normalized_final_newline {
        let eof_line = display.line_count.saturating_sub(1);
        return Some((
            file,
            eof_line,
            display.eof_column,
            eof_line,
            display.eof_column,
        ));
    }
    if start_line >= display.line_count {
        return None;
    }
    let end_line = end_line
        .saturating_sub(display.line_offset)
        .max(start_line)
        .min(display.line_count.saturating_sub(1));
    Some((file, start_line, start_col, end_line, end_col))
}

fn cboxes_error_json(kind: &str, message: &str, range: Option<CboxesDiagnosticRange>) -> String {
    cboxes_error_json_with_annotations(kind, message, range, &[])
}

fn cboxes_error_json_with_annotations(
    kind: &str,
    message: &str,
    range: Option<CboxesDiagnosticRange>,
    annotations: &[(String, CboxesDiagnosticRange)],
) -> String {
    let (file, line, col, end_line, end_col) = range
        .map(|(file, line, col, end_line, end_col)| {
            (
                cboxes_json_string(&file),
                line.to_string(),
                col.to_string(),
                end_line.to_string(),
                end_col.to_string(),
            )
        })
        .unwrap_or_else(|| {
            (
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
                "null".to_owned(),
            )
        });
    let mut annotations_json = String::from("[");
    for (index, (id, (file, line, col, end_line, end_col))) in annotations.iter().enumerate() {
        if index > 0 {
            annotations_json.push(',');
        }
        annotations_json.push_str(&format!(
            "{{\"id\":{},\"file\":{},\"line\":{},\"column\":{},\"endLine\":{},\"endColumn\":{}}}",
            cboxes_json_string(id),
            cboxes_json_string(file),
            line,
            col,
            end_line,
            end_col,
        ));
    }
    annotations_json.push(']');
    format!(
        "{{\"ok\":false,\"kind\":{},\"message\":{},\"file\":{},\"line\":{},\"column\":{},\"endLine\":{},\"endColumn\":{},\"annotations\":{}}}",
        cboxes_json_string(kind),
        cboxes_json_string(message),
        file,
        line,
        col,
        end_line,
        end_col,
        annotations_json,
    )
}

fn cboxes_rendered_location(rendered: &str) -> Option<(String, usize, usize)> {
    for line in rendered.lines() {
        let Some(rest) = line.trim_start().strip_prefix("--> ") else {
            continue;
        };
        let mut parts = rest.rsplitn(3, ':');
        let col = parts.next()?.parse::<usize>().ok()?;
        let line = parts.next()?.parse::<usize>().ok()?;
        let file = parts.next()?.to_owned();
        return Some((file, line.saturating_sub(1), col.saturating_sub(1)));
    }
    None
}

fn cboxes_state_json(state: &[ProgramStateBox]) -> String {
    let mut out = String::from("[");
    for (index, item) in state.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        out.push_str("\"name\":");
        out.push_str(&cboxes_json_string(&item.name));
        out.push_str(",\"type\":");
        out.push_str(&cboxes_json_string(&item.ty));
        out.push_str(",\"value\":");
        out.push_str(&cboxes_json_string(&item.value));
        out.push_str(",\"displayValue\":");
        out.push_str(&cboxes_json_string(&item.display_value));
        out.push_str(",\"exactValue\":");
        out.push_str(&cboxes_json_string(&item.exact_value));
        out.push_str(",\"address\":");
        if let Some(address) = item.address {
            out.push_str(&cboxes_json_string(&address.to_string()));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"arrayRoot\":");
        if let Some(root) = &item.array_root {
            out.push_str(&cboxes_json_string(root));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"arrayShape\":");
        out.push_str(&cboxes_usize_array_json(&item.array_shape));
        out.push_str(",\"arrayIndices\":");
        out.push_str(&cboxes_usize_array_json(&item.array_indices));
        out.push_str(",\"aggregateRoot\":");
        if let Some(root) = &item.aggregate_root {
            out.push_str(&cboxes_json_string(root));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"aggregatePath\":");
        out.push_str(&cboxes_string_array_json(&item.aggregate_path));
        out.push_str(",\"aggregateKind\":");
        if let Some(kind) = &item.aggregate_kind {
            out.push_str(&cboxes_json_string(kind));
        } else {
            out.push_str("null");
        }
        out.push_str(",\"aliases\":");
        out.push_str(&cboxes_string_array_json(&item.aliases));
        out.push_str(",\"typeInfo\":");
        out.push_str(&cboxes_type_info_json(&item.type_info));
        out.push('}');
    }
    out.push(']');
    out
}

fn cboxes_trace_json(trace: &[ProgramTraceEvent], source_display: &SourceDisplayMap) -> String {
    let mut out = String::from("[");
    let mut wrote_event = false;
    for event in trace {
        let display = source_display
            .get(&event.file)
            .copied()
            .unwrap_or_else(SourceDisplay::unbounded);
        let start_line = event.start_line.saturating_sub(display.line_offset);
        let end_line = event.end_line.saturating_sub(display.line_offset);
        if start_line >= display.line_count {
            continue;
        }
        if wrote_event {
            out.push(',');
        }
        wrote_event = true;
        out.push('{');
        out.push_str("\"kind\":");
        out.push_str(&cboxes_json_string(&event.kind));
        out.push_str(",\"file\":");
        out.push_str(&cboxes_json_string(&event.file));
        out.push_str(",\"startLine\":");
        out.push_str(&start_line.to_string());
        out.push_str(",\"endLine\":");
        out.push_str(
            &end_line
                .min(display.line_count.saturating_sub(1))
                .to_string(),
        );
        out.push_str(",\"state\":");
        out.push_str(&cboxes_state_json(&event.state));
        out.push_str(",\"skippedRange\":");
        out.push_str(&cboxes_source_range_json(
            event.skipped_range.as_ref(),
            source_display,
        ));
        out.push('}');
    }
    out.push(']');
    out
}

fn cboxes_source_range_json(
    range: Option<&ProgramSourceRange>,
    source_display: &SourceDisplayMap,
) -> String {
    let Some(range) = range else {
        return "null".to_owned();
    };
    let display = source_display
        .get(&range.file)
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let start_line = range.start_line.saturating_sub(display.line_offset);
    if start_line >= display.line_count {
        return "null".to_owned();
    }
    let end_line = range
        .end_line
        .saturating_sub(display.line_offset)
        .min(display.line_count.saturating_sub(1));
    format!(
        "{{\"file\":{},\"startLine\":{},\"startColumn\":{},\"endLine\":{},\"endColumn\":{}}}",
        cboxes_json_string(&range.file),
        start_line,
        range.start_column,
        end_line,
        range.end_column,
    )
}

fn cboxes_source_location_json(
    location: &ProgramSourceLocation,
    source_display: &SourceDisplayMap,
) -> String {
    let display = source_display
        .get(&location.file)
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let line = location.line.saturating_sub(display.line_offset);
    if line >= display.line_count {
        return "null".to_owned();
    }
    format!(
        "{{\"file\":{},\"line\":{}}}",
        cboxes_json_string(&location.file),
        line
    )
}

fn cboxes_blocked_json(
    blocked: Option<&ProgramBlocked>,
    source_display: &SourceDisplayMap,
) -> String {
    let Some(blocked) = blocked else {
        return "null".to_owned();
    };
    let display = source_display
        .get(&blocked.file)
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let start_line = blocked.start_line.saturating_sub(display.line_offset);
    if start_line >= display.line_count {
        return "null".to_owned();
    }
    let end_line = blocked
        .end_line
        .saturating_sub(display.line_offset)
        .min(display.line_count.saturating_sub(1));
    format!(
        "{{\"file\":{},\"startLine\":{},\"endLine\":{},\"function\":{},\"state\":{}}}",
        cboxes_json_string(&blocked.file),
        start_line,
        end_line,
        cboxes_json_string(&blocked.function),
        cboxes_state_json(&blocked.state)
    )
}

fn cboxes_execution_limit_json(
    execution_limit: Option<&ProgramExecutionLimit>,
    source_display: &SourceDisplayMap,
) -> String {
    let Some(execution_limit) = execution_limit else {
        return "null".to_owned();
    };
    let display = source_display
        .get(&execution_limit.file)
        .copied()
        .unwrap_or_else(SourceDisplay::unbounded);
    let start_line = execution_limit
        .start_line
        .saturating_sub(display.line_offset);
    if start_line >= display.line_count {
        return "null".to_owned();
    }
    let end_line = execution_limit
        .end_line
        .saturating_sub(display.line_offset)
        .min(display.line_count.saturating_sub(1));
    format!(
        "{{\"file\":{},\"startLine\":{},\"endLine\":{},\"tracePosition\":{}}}",
        cboxes_json_string(&execution_limit.file),
        start_line,
        end_line,
        execution_limit.trace_position,
    )
}

fn cboxes_usize_array_json(values: &[usize]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&value.to_string());
    }
    out.push(']');
    out
}

fn cboxes_json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn run_with_sources(
    sources: &mut SourceManager,
    root_file: source::FileId,
    cwd: &Path,
    options: &RunOptions,
) -> Result<RunResult, Diagnostic> {
    let translation_unit = normalize_single_translation_unit(parse_translation_unit(
        sources, root_file, cwd, options,
    )?)
    .map_err(|diag| diag.with_sources(sources))?;
    run_translation_unit(sources, translation_unit, options)
}

fn run_translation_unit(
    sources: &mut SourceManager,
    translation_unit: TranslationUnit,
    options: &RunOptions,
) -> Result<RunResult, Diagnostic> {
    let expression_eval = if let Some(request) = &options.expression_eval {
        let mut text = request.expression.clone();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        let expr_file = sources.add_file(PathBuf::from("<expression>"), text);
        let tokens = Lexer::new(sources, expr_file)
            .lex()
            .map_err(|diag| diag.with_sources(sources))?;
        let value_literal_text = cboxes_value_literal_text(&tokens);
        let expr = Parser::new(sources, tokens)
            .parse_expression_only()
            .map_err(|diag| diag.with_sources(sources))?;
        Some(ProgramExpressionEvalRequest {
            event_index: request.event_index,
            expr,
            value_literal_text,
        })
    } else {
        None
    };

    #[cfg(not(target_os = "wasi"))]
    let execution = std::thread::scope(|scope| {
        let diagnostic_span = fallback_span(&translation_unit);
        let execution_sources: &SourceManager = sources;
        let handle = std::thread::Builder::new()
            .name("cboxes-interpreter".to_owned())
            .stack_size(INTERPRETER_STACK_BYTES)
            .spawn_scoped(scope, move || {
                execute_translation_unit(
                    execution_sources,
                    translation_unit,
                    options,
                    expression_eval,
                )
            })
            .map_err(|error| {
                Diagnostic::error(
                    format!("failed to create the cBoxes interpreter thread: {error}"),
                    diagnostic_span,
                )
            })?;
        match handle.join() {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    });
    #[cfg(target_os = "wasi")]
    let execution = execute_translation_unit(sources, translation_unit, options, expression_eval);

    let ProgramOutput {
        stdout,
        stderr,
        exit_status,
        state,
        trace,
        main_close,
        blocked,
        execution_limit,
        expression,
    } = execution.map_err(|diag| diag.with_sources(sources))?;
    Ok(RunResult {
        stdout,
        stderr,
        exit_status,
        state,
        trace,
        main_close,
        blocked,
        execution_limit,
        expression,
    })
}

fn execute_translation_unit(
    sources: &SourceManager,
    translation_unit: TranslationUnit,
    options: &RunOptions,
    expression_eval: Option<ProgramExpressionEvalRequest>,
) -> Result<ProgramOutput, Diagnostic> {
    let mut interpreter = Interpreter::new(sources, translation_unit, options);
    if let Some(request) = expression_eval {
        interpreter.set_cboxes_expression_eval(request);
    }
    interpreter.run()
}

fn parse_translation_unit(
    sources: &mut SourceManager,
    root_file: source::FileId,
    cwd: &Path,
    options: &RunOptions,
) -> Result<TranslationUnit, Diagnostic> {
    #[cfg(test)]
    let preprocessor = Preprocessor::new(cwd.to_path_buf(), options.include_dirs.clone());
    #[cfg(not(test))]
    let preprocessor = {
        let _ = (cwd, options);
        Preprocessor::new()
    };
    let preprocessed = preprocessor
        .preprocess(sources, root_file)
        .map_err(|diag| diag.with_sources(sources))?;
    let tokens = Lexer::new(sources, preprocessed.file_id)
        .lex()
        .map_err(|diag| diag.with_sources(sources))?;
    Parser::new(sources, tokens)
        .parse_translation_unit()
        .map_err(|diag| diag.with_sources(sources))
}

fn normalize_single_translation_unit(
    mut unit: TranslationUnit,
) -> Result<TranslationUnit, Diagnostic> {
    let original_externals = unit.externals.clone();
    let normalized = normalize_translation_unit(
        std::mem::take(&mut unit.externals),
        &unit.records,
        &unit.enums,
    )?;
    unit.externals = original_externals;
    unit.function_declarations = normalized.function_declarations;
    unit.functions = normalized.function_definitions;
    unit.globals = normalized.global_declarations;
    unit.global_definitions = normalized.global_definitions;
    unit.inline_function_definitions = normalized.inline_function_definitions;
    Ok(unit)
}

fn merge_translation_units(units: Vec<TranslationUnit>) -> Result<TranslationUnit, Diagnostic> {
    let mut merged = TranslationUnit {
        externals: Vec::new(),
        functions: Vec::new(),
        function_declarations: Vec::new(),
        globals: Vec::new(),
        global_definitions: Vec::new(),
        inline_function_definitions: Vec::new(),
        records: Default::default(),
        enums: Default::default(),
        enum_constants: Default::default(),
    };
    let mut external_function_decls = HashMap::<String, FunctionDecl>::new();
    let mut external_function_defs = HashMap::<String, FunctionDef>::new();
    let mut external_object_decls = HashMap::<String, Declaration>::new();
    let mut external_object_defs = HashMap::<String, Declaration>::new();
    let mut external_symbol_kinds = HashMap::<String, ExternalSymbolKind>::new();
    let mut next_record_id = 0usize;
    let mut next_enum_id = 0usize;

    for unit in units {
        let record_map = unit
            .records
            .keys()
            .copied()
            .map(|old_id| {
                let new_id = next_record_id;
                next_record_id += 1;
                (old_id, new_id)
            })
            .collect::<std::collections::HashMap<_, _>>();
        let enum_map = unit
            .enums
            .keys()
            .copied()
            .map(|old_id| {
                let new_id = next_enum_id;
                next_enum_id += 1;
                (old_id, new_id)
            })
            .collect::<std::collections::HashMap<_, _>>();

        for (old_id, record) in unit.records {
            let new_id = record_map[&old_id];
            merged.records.insert(
                new_id,
                RecordType {
                    id: new_id,
                    kind: record.kind,
                    tag: record.tag,
                    complete: record.complete,
                    members: record
                        .members
                        .into_iter()
                        .map(|member| remap_record_member(member, &record_map, &enum_map))
                        .collect(),
                    size: record.size,
                    align: record.align,
                },
            );
        }
        for (old_id, enum_ty) in unit.enums {
            let new_id = enum_map[&old_id];
            merged.enums.insert(
                new_id,
                EnumType {
                    id: new_id,
                    tag: enum_ty.tag,
                    complete: enum_ty.complete,
                },
            );
        }
        let mut remapped_externals = Vec::new();
        for external in unit.externals {
            remapped_externals.push(remap_external_declaration(external, &record_map, &enum_map));
        }
        merged.externals.extend(remapped_externals.iter().cloned());
        let normalized =
            normalize_translation_unit(remapped_externals, &merged.records, &merged.enums)?;
        merged
            .inline_function_definitions
            .extend(normalized.inline_function_definitions.iter().cloned());
        for function_decl in normalized.function_declarations {
            if function_decl.storage_class == Some(StorageClass::Static) {
                merged.function_declarations.push(function_decl);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &function_decl.name,
                ExternalSymbolKind::Function,
                function_decl.span,
            )?;
            external_symbol_kinds.insert(function_decl.name.clone(), ExternalSymbolKind::Function);
            if let Some(existing) = external_function_decls.get_mut(&function_decl.name) {
                merge_function_declaration(
                    existing,
                    &function_decl,
                    &merged.records,
                    &merged.enums,
                )?;
            } else {
                external_function_decls.insert(function_decl.name.clone(), function_decl);
            }
        }
        for function in normalized.function_definitions {
            if function.storage_class == Some(StorageClass::Static) {
                merged.functions.push(function);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &function.name,
                ExternalSymbolKind::Function,
                function.span,
            )?;
            external_symbol_kinds.insert(function.name.clone(), ExternalSymbolKind::Function);
            let function_decl = function_decl_from_definition(&function);
            if let Some(existing) = external_function_decls.get_mut(&function.name) {
                merge_function_declaration(
                    existing,
                    &function_decl,
                    &merged.records,
                    &merged.enums,
                )?;
            } else {
                external_function_decls.insert(function.name.clone(), function_decl);
            }
            if let Some(existing) = external_function_defs.get(&function.name) {
                return Err(Diagnostic::error(
                    format!("multiple definitions of function {}", function.name),
                    function.span,
                )
                .with_note(format!(
                    "previous definition is at {}:{}:{}",
                    existing.span.file.0, existing.span.start, existing.span.end
                )));
            }
            external_function_defs.insert(function.name.clone(), function);
        }
        for global_decl in normalized.global_declarations {
            if global_decl.storage_class == Some(StorageClass::Static) {
                merged.globals.push(global_decl);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &global_decl.name,
                ExternalSymbolKind::Object,
                global_decl.span,
            )?;
            external_symbol_kinds.insert(global_decl.name.clone(), ExternalSymbolKind::Object);
            if let Some(existing) = external_object_decls.get_mut(&global_decl.name) {
                merge_object_declaration(existing, &global_decl, &merged.records, &merged.enums)?;
            } else {
                external_object_decls.insert(global_decl.name.clone(), global_decl);
            }
        }
        for global_def in normalized.global_definitions {
            if global_def.storage_class == Some(StorageClass::Static) {
                merged.global_definitions.push(global_def);
                continue;
            }
            ensure_external_symbol_kind(
                &external_symbol_kinds,
                &global_def.name,
                ExternalSymbolKind::Object,
                global_def.span,
            )?;
            external_symbol_kinds.insert(global_def.name.clone(), ExternalSymbolKind::Object);
            if let Some(existing) = external_object_decls.get_mut(&global_def.name) {
                merge_object_declaration(existing, &global_def, &merged.records, &merged.enums)?;
            } else {
                external_object_decls.insert(global_def.name.clone(), global_def.clone());
            }
            if let Some(existing) = external_object_defs.get(&global_def.name) {
                return Err(Diagnostic::error(
                    format!("multiple definitions of object {}", global_def.name),
                    global_def.span,
                )
                .with_note(format!(
                    "previous definition is at {}:{}:{}",
                    existing.span.file.0, existing.span.start, existing.span.end
                )));
            }
            external_object_defs.insert(global_def.name.clone(), global_def);
        }
        for (name, value) in unit.enum_constants {
            if let Some(existing) = merged.enum_constants.insert(name.clone(), value) {
                if existing != value {
                    return Err(Diagnostic::error(
                        format!("conflicting enum constant {}", name),
                        fallback_span(&merged),
                    ));
                }
            }
        }
    }

    for (name, definition) in &mut external_function_defs {
        if let Some(declaration) = external_function_decls.get(name) {
            definition.is_noreturn = declaration.is_noreturn;
        }
    }
    for (name, definition) in &mut external_object_defs {
        if let Some(declaration) = external_object_decls.get(name) {
            definition.alignment = declaration.alignment;
        }
    }

    merged
        .function_declarations
        .extend(external_function_decls.into_values());
    merged
        .functions
        .extend(external_function_defs.into_values());
    merged.globals.extend(external_object_decls.into_values());
    merged
        .global_definitions
        .extend(external_object_defs.into_values());

    Ok(merged)
}

fn fallback_span(unit: &TranslationUnit) -> Span {
    unit.functions
        .first()
        .map(|function| function.span)
        .or_else(|| unit.function_declarations.first().map(|decl| decl.span))
        .or_else(|| unit.global_definitions.first().map(|global| global.span))
        .or_else(|| unit.globals.first().map(|global| global.span))
        .or_else(|| {
            unit.externals.iter().find_map(|external| match external {
                ExternalDeclaration::Function(function) => Some(function.span),
                ExternalDeclaration::FunctionDeclaration(decl) => Some(decl.span),
                ExternalDeclaration::ObjectDeclaration(decl) => Some(decl.span),
            })
        })
        .unwrap_or(Span::new(FileId(0), 0, 0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalSymbolKind {
    Function,
    Object,
}

#[derive(Debug, Clone)]
struct NormalizedTranslationUnit {
    function_declarations: Vec<FunctionDecl>,
    function_definitions: Vec<FunctionDef>,
    global_declarations: Vec<Declaration>,
    global_definitions: Vec<Declaration>,
    inline_function_definitions: Vec<FunctionDef>,
}

#[derive(Debug, Clone)]
struct UnitFunctionEntry {
    declaration: FunctionDecl,
    definition: Option<FunctionDef>,
    linkage: Linkage,
    saw_inline: bool,
    saw_non_inline: bool,
    saw_extern: bool,
    inline_span: Option<Span>,
}

#[derive(Debug, Clone)]
struct UnitObjectEntry {
    declaration: Declaration,
    real_definition: Option<Declaration>,
    has_tentative_definition: bool,
}

#[derive(Debug, Clone)]
struct PriorSymbol {
    kind: ExternalSymbolKind,
    linkage: Linkage,
    span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ScopedSymbol {
    Internal(FileId, String),
    External(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectRole {
    Declaration,
    TentativeDefinition,
    Definition,
}

fn normalize_translation_unit(
    externals: Vec<ExternalDeclaration>,
    records: &HashMap<usize, RecordType>,
    enums: &HashMap<usize, EnumType>,
) -> Result<NormalizedTranslationUnit, Diagnostic> {
    let mut prior_symbols = HashMap::<String, PriorSymbol>::new();
    let mut function_entries = HashMap::<ScopedSymbol, UnitFunctionEntry>::new();
    let mut object_entries = HashMap::<ScopedSymbol, UnitObjectEntry>::new();

    for external in externals {
        match external {
            ExternalDeclaration::Function(function) => {
                let decl = function_decl_from_definition(&function);
                let linkage = validate_linkage(
                    &function.name,
                    function.linkage,
                    ExternalSymbolKind::Function,
                    function.span,
                    &mut prior_symbols,
                )?;
                let key = scoped_symbol(function.span.file, &function.name, linkage);
                let normalized_decl = normalize_function_decl_linkage(decl, linkage);
                let normalized_def = normalize_function_def_linkage(function, linkage);
                if let Some(entry) = function_entries.get_mut(&key) {
                    merge_function_declaration(
                        &mut entry.declaration,
                        &normalized_decl,
                        records,
                        enums,
                    )?;
                    if entry.definition.is_some() {
                        return Err(Diagnostic::error(
                            format!("multiple definitions of function {}", normalized_def.name),
                            normalized_def.span,
                        ));
                    }
                    entry.definition = Some(normalized_def);
                    entry.saw_inline |= normalized_decl.is_inline;
                    entry.saw_non_inline |= !normalized_decl.is_inline;
                    entry.saw_extern |= normalized_decl.storage_class == Some(StorageClass::Extern);
                    entry.inline_span = entry
                        .inline_span
                        .or(normalized_decl.is_inline.then_some(normalized_decl.span));
                } else {
                    let is_inline = normalized_decl.is_inline;
                    let storage_class = normalized_decl.storage_class;
                    let inline_span = is_inline.then_some(normalized_decl.span);
                    function_entries.insert(
                        key,
                        UnitFunctionEntry {
                            declaration: normalized_decl,
                            definition: Some(normalized_def),
                            linkage,
                            saw_inline: is_inline,
                            saw_non_inline: !is_inline,
                            saw_extern: storage_class == Some(StorageClass::Extern),
                            inline_span,
                        },
                    );
                }
            }
            ExternalDeclaration::FunctionDeclaration(function_decl) => {
                let linkage = validate_linkage(
                    &function_decl.name,
                    function_decl.linkage,
                    ExternalSymbolKind::Function,
                    function_decl.span,
                    &mut prior_symbols,
                )?;
                let key = scoped_symbol(function_decl.span.file, &function_decl.name, linkage);
                let normalized_decl = normalize_function_decl_linkage(function_decl, linkage);
                if let Some(entry) = function_entries.get_mut(&key) {
                    merge_function_declaration(
                        &mut entry.declaration,
                        &normalized_decl,
                        records,
                        enums,
                    )?;
                    entry.saw_inline |= normalized_decl.is_inline;
                    entry.saw_non_inline |= !normalized_decl.is_inline;
                    entry.saw_extern |= normalized_decl.storage_class == Some(StorageClass::Extern);
                    entry.inline_span = entry
                        .inline_span
                        .or(normalized_decl.is_inline.then_some(normalized_decl.span));
                } else {
                    let is_inline = normalized_decl.is_inline;
                    let storage_class = normalized_decl.storage_class;
                    let inline_span = is_inline.then_some(normalized_decl.span);
                    function_entries.insert(
                        key,
                        UnitFunctionEntry {
                            declaration: normalized_decl,
                            definition: None,
                            linkage,
                            saw_inline: is_inline,
                            saw_non_inline: !is_inline,
                            saw_extern: storage_class == Some(StorageClass::Extern),
                            inline_span,
                        },
                    );
                }
            }
            ExternalDeclaration::ObjectDeclaration(decl) => {
                let role = classify_object_role(&decl);
                let linkage = validate_linkage(
                    &decl.name,
                    decl.linkage
                        .expect("translation-unit object declaration must have linkage"),
                    ExternalSymbolKind::Object,
                    decl.span,
                    &mut prior_symbols,
                )?;
                let key = scoped_symbol(decl.span.file, &decl.name, linkage);
                let normalized_decl = normalize_object_linkage(decl, linkage);
                if let Some(entry) = object_entries.get_mut(&key) {
                    merge_object_declaration(
                        &mut entry.declaration,
                        &normalized_decl,
                        records,
                        enums,
                    )?;
                    match role {
                        ObjectRole::Declaration => {}
                        ObjectRole::TentativeDefinition => entry.has_tentative_definition = true,
                        ObjectRole::Definition => {
                            if entry.real_definition.is_some() {
                                return Err(Diagnostic::error(
                                    format!(
                                        "multiple definitions of object {}",
                                        normalized_decl.name
                                    ),
                                    normalized_decl.span,
                                ));
                            }
                            entry.real_definition = Some(normalized_decl);
                        }
                    }
                } else {
                    object_entries.insert(
                        key,
                        UnitObjectEntry {
                            declaration: normalized_decl.clone(),
                            real_definition: (role == ObjectRole::Definition)
                                .then_some(normalized_decl),
                            has_tentative_definition: role == ObjectRole::TentativeDefinition,
                        },
                    );
                }
            }
        }
    }

    let mut normalized = NormalizedTranslationUnit {
        function_declarations: Vec::new(),
        function_definitions: Vec::new(),
        global_declarations: Vec::new(),
        global_definitions: Vec::new(),
        inline_function_definitions: Vec::new(),
    };

    let internal_linkage_names = collect_internal_linkage_names(&function_entries, &object_entries);
    for entry in function_entries.into_values() {
        if let Some(definition) = entry.definition.as_ref()
            && !definition.has_prototype
            && entry.declaration.has_prototype
            && composite_type(
                &function_decl_type(&entry.declaration),
                &old_style_promoted_definition_type(definition),
                records,
                enums,
            )
            .is_none()
        {
            return Err(Diagnostic::error(
                format!(
                    "old-style definition of {} is incompatible with its prototype",
                    definition.name
                ),
                definition.span,
            ));
        }
        if entry.declaration.name == "main" && entry.saw_inline {
            return Err(Diagnostic::error(
                "main shall not be declared inline",
                entry.inline_span.unwrap_or(entry.declaration.span),
            ));
        }
        if entry.linkage == Linkage::External && entry.saw_inline && entry.definition.is_none() {
            return Err(Diagnostic::error(
                format!(
                    "inline declaration of function {} with external linkage requires a definition in the same translation unit",
                    entry.declaration.name
                ),
                entry.inline_span.unwrap_or(entry.declaration.span),
            ));
        }
        let is_inline_definition = entry.linkage == Linkage::External
            && entry.definition.is_some()
            && entry.saw_inline
            && !entry.saw_non_inline
            && !entry.saw_extern;
        normalized
            .function_declarations
            .push(entry.declaration.clone());
        if let Some(mut definition) = entry.definition {
            definition.is_noreturn = entry.declaration.is_noreturn;
            if is_inline_definition {
                validate_inline_definition_constraints(
                    &definition,
                    &internal_linkage_names,
                    records,
                )?;
                normalized.inline_function_definitions.push(definition);
            } else {
                normalized.function_definitions.push(definition);
            }
        }
    }
    for mut entry in object_entries.into_values() {
        normalized
            .global_declarations
            .push(entry.declaration.clone());
        if let Some(mut definition) = entry.real_definition.take() {
            definition.alignment = entry.declaration.alignment;
            normalized.global_definitions.push(definition);
        } else if entry.has_tentative_definition {
            if entry.declaration.linkage == Some(Linkage::Internal)
                && !type_is_complete_for_linkage(&entry.declaration.ty, records)
            {
                return Err(Diagnostic::error(
                    format!(
                        "tentative definition of internal-linkage object {} must have complete type",
                        entry.declaration.name
                    ),
                    entry.declaration.span,
                ));
            }
            normalized
                .global_definitions
                .push(finalize_tentative_definition(entry.declaration));
        }
    }

    Ok(normalized)
}

fn type_is_complete_for_linkage(ty: &CType, records: &HashMap<usize, RecordType>) -> bool {
    match ty.unqualified() {
        CType::Void | CType::Function(..) | CType::Array(_, 0) => false,
        CType::Array(inner, _) => type_is_complete_for_linkage(inner, records),
        CType::Struct(id, _) | CType::Union(id, _) => {
            records.get(id).is_some_and(|record| record.complete)
        }
        _ => true,
    }
}

fn validate_linkage(
    name: &str,
    linkage: Linkage,
    kind: ExternalSymbolKind,
    span: Span,
    prior_symbols: &mut HashMap<String, PriorSymbol>,
) -> Result<Linkage, Diagnostic> {
    let prior = prior_symbols.get(name).cloned();
    if let Some(prior) = &prior {
        if prior.kind != kind {
            return Err(Diagnostic::error(
                format!(
                    "identifier {} is declared as both a {} and an {}",
                    name,
                    describe_external_symbol_kind(prior.kind),
                    describe_external_symbol_kind(kind)
                ),
                span,
            ));
        }
        if prior.linkage != linkage {
            return Err(Diagnostic::ub(
                format!(
                    "identifier {} is declared with both internal and external linkage",
                    name
                ),
                span,
                Some("6.2.2"),
            )
            .with_note(format!(
                "previous declaration is at {}:{}:{}",
                prior.span.file.0, prior.span.start, prior.span.end
            )));
        }
    }
    prior_symbols.insert(
        name.to_owned(),
        PriorSymbol {
            kind,
            linkage,
            span,
        },
    );
    Ok(linkage)
}

fn scoped_symbol(file: FileId, name: &str, linkage: Linkage) -> ScopedSymbol {
    match linkage {
        Linkage::Internal => ScopedSymbol::Internal(file, name.to_owned()),
        Linkage::External => ScopedSymbol::External(name.to_owned()),
    }
}

fn classify_object_role(decl: &Declaration) -> ObjectRole {
    if decl.init.is_some() {
        ObjectRole::Definition
    } else if decl.storage_class == Some(StorageClass::Extern) {
        ObjectRole::Declaration
    } else {
        ObjectRole::TentativeDefinition
    }
}

fn collect_internal_linkage_names(
    function_entries: &HashMap<ScopedSymbol, UnitFunctionEntry>,
    object_entries: &HashMap<ScopedSymbol, UnitObjectEntry>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for entry in function_entries.values() {
        if entry.linkage == Linkage::Internal {
            names.insert(entry.declaration.name.clone());
        }
    }
    for entry in object_entries.values() {
        if entry.declaration.storage_class == Some(StorageClass::Static) {
            names.insert(entry.declaration.name.clone());
        }
    }
    names
}

fn validate_inline_definition_constraints(
    function: &FunctionDef,
    internal_linkage_names: &HashSet<String>,
    records: &HashMap<usize, RecordType>,
) -> Result<(), Diagnostic> {
    let mut local_scopes = vec![HashSet::new()];
    for param in &function.params {
        if let Some(name) = &param.name {
            local_scopes
                .last_mut()
                .expect("parameter scope exists")
                .insert(name.clone());
        }
    }
    validate_inline_block(
        &function.body,
        internal_linkage_names,
        records,
        &mut local_scopes,
    )
}

fn validate_inline_block(
    block: &Block,
    internal_linkage_names: &HashSet<String>,
    records: &HashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    local_scopes.push(HashSet::new());
    for item in &block.items {
        match item {
            BlockItem::Declaration(decl) => {
                validate_inline_vla_bounds(&decl.vla_bounds, internal_linkage_names, local_scopes)?;
                if decl.storage_class == Some(StorageClass::Static)
                    && type_is_modifiable_object(&decl.ty, records)
                {
                    local_scopes.pop();
                    return Err(Diagnostic::error(
                        "inline definition with external linkage shall not define a modifiable object with static storage duration",
                        decl.span,
                    ));
                }
                local_scopes
                    .last_mut()
                    .expect("block scope exists")
                    .insert(decl.name.clone());
                if let Some(init) = &decl.init {
                    validate_inline_initializer(
                        init,
                        internal_linkage_names,
                        records,
                        local_scopes,
                    )?;
                }
            }
            BlockItem::FunctionDeclaration(decl) => {
                local_scopes
                    .last_mut()
                    .expect("block scope exists")
                    .insert(decl.name.clone());
            }
            BlockItem::Statement(stmt) => {
                validate_inline_statement(stmt, internal_linkage_names, records, local_scopes)?;
            }
        }
    }
    local_scopes.pop();
    Ok(())
}

fn validate_inline_statement(
    stmt: &Statement,
    internal_linkage_names: &HashSet<String>,
    records: &HashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    match stmt {
        Statement::Block(block) => {
            validate_inline_block(block, internal_linkage_names, records, local_scopes)
        }
        Statement::Break(_) | Statement::Continue(_) => Ok(()),
        Statement::DoWhile {
            body, condition, ..
        } => {
            validate_inline_statement(body, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)
        }
        Statement::Expression(expr, _) => expr.as_ref().map_or(Ok(()), |expr| {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }),
        Statement::For {
            init,
            condition,
            step,
            body,
            ..
        } => {
            local_scopes.push(HashSet::new());
            if let Some(init) = init {
                match init {
                    ForInit::Declarations(decls) => {
                        for decl in decls {
                            validate_inline_vla_bounds(
                                &decl.vla_bounds,
                                internal_linkage_names,
                                local_scopes,
                            )?;
                            if decl.storage_class == Some(StorageClass::Static)
                                && type_is_modifiable_object(&decl.ty, records)
                            {
                                local_scopes.pop();
                                return Err(Diagnostic::error(
                                    "inline definition with external linkage shall not define a modifiable object with static storage duration",
                                    decl.span,
                                ));
                            }
                            local_scopes
                                .last_mut()
                                .expect("for scope exists")
                                .insert(decl.name.clone());
                            if let Some(init) = &decl.init {
                                validate_inline_initializer(
                                    init,
                                    internal_linkage_names,
                                    records,
                                    local_scopes,
                                )?;
                            }
                        }
                    }
                    ForInit::Expression(expr) => {
                        validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
                    }
                }
            }
            if let Some(condition) = condition {
                validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            }
            if let Some(step) = step {
                validate_inline_expr(step, internal_linkage_names, records, local_scopes)?;
            }
            let result =
                validate_inline_statement(body, internal_linkage_names, records, local_scopes);
            local_scopes.pop();
            result
        }
        Statement::Goto { .. } => Ok(()),
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            validate_inline_statement(then_branch, internal_linkage_names, records, local_scopes)?;
            if let Some(else_branch) = else_branch {
                validate_inline_statement(
                    else_branch,
                    internal_linkage_names,
                    records,
                    local_scopes,
                )?;
            }
            Ok(())
        }
        Statement::Labeled {
            label, statement, ..
        } => {
            if let SwitchLabel::Case { expr, .. } = label {
                validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
            }
            validate_inline_statement(statement, internal_linkage_names, records, local_scopes)
        }
        Statement::Return(expr, _) => expr.as_ref().map_or(Ok(()), |expr| {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }),
        Statement::Switch { expr, body, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
            validate_inline_block(body, internal_linkage_names, records, local_scopes)
        }
        Statement::UserLabeled { statement, .. } => {
            validate_inline_statement(statement, internal_linkage_names, records, local_scopes)
        }
        Statement::While {
            condition, body, ..
        } => {
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            validate_inline_statement(body, internal_linkage_names, records, local_scopes)
        }
    }
}

fn validate_inline_initializer(
    init: &Initializer,
    internal_linkage_names: &HashSet<String>,
    records: &HashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    match init {
        Initializer::Expr(expr) => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Initializer::List { items, .. } => {
            for item in items {
                validate_inline_initializer(
                    &item.initializer,
                    internal_linkage_names,
                    records,
                    local_scopes,
                )?;
            }
            Ok(())
        }
    }
}

fn validate_inline_vla_bounds(
    bounds: &[Option<Expr>],
    internal_linkage_names: &HashSet<String>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    for expr in bounds.iter().flatten() {
        validate_inline_expr(expr, internal_linkage_names, &HashMap::new(), local_scopes)?;
    }
    Ok(())
}

fn validate_inline_expr(
    expr: &Expr,
    internal_linkage_names: &HashSet<String>,
    records: &HashMap<usize, RecordType>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<(), Diagnostic> {
    match expr {
        Expr::Number(_, _)
        | Expr::CharLiteral(_, _)
        | Expr::WideCharLiteral(_, _)
        | Expr::Utf16CharLiteral(_, _)
        | Expr::Utf32CharLiteral(_, _)
        | Expr::StringLiteral(_, _)
        | Expr::WideStringLiteral(_, _) => Ok(()),
        Expr::Utf16StringLiteral(_, _) | Expr::Utf32StringLiteral(_, _) => Ok(()),
        Expr::Variable(name, span) => {
            if !local_scopes.iter().rev().any(|scope| scope.contains(name))
                && internal_linkage_names.contains(name)
            {
                Err(Diagnostic::error(
                    format!(
                        "inline definition with external linkage shall not reference internal-linkage identifier {}",
                        name
                    ),
                    *span,
                ))
            } else {
                Ok(())
            }
        }
        Expr::Unary { expr, .. } | Expr::Postfix { expr, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Subscript {
            base: lhs,
            index: rhs,
            ..
        }
        | Expr::Assign { lhs, rhs, .. }
        | Expr::CompoundAssign { lhs, rhs, .. } => {
            validate_inline_expr(lhs, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(rhs, internal_linkage_names, records, local_scopes)
        }
        Expr::SizeofType { vla_bounds, .. } => {
            for expr in vla_bounds.iter().flatten() {
                validate_inline_expr(expr, internal_linkage_names, records, local_scopes)?;
            }
            Ok(())
        }
        Expr::SizeofExpr { expr, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Expr::OffsetOf { .. } => Ok(()),
        Expr::Cast { expr, .. } => {
            validate_inline_expr(expr, internal_linkage_names, records, local_scopes)
        }
        Expr::CompoundLiteral { initializer, .. } => {
            validate_inline_initializer(initializer, internal_linkage_names, records, local_scopes)
        }
        Expr::GenericSelection {
            control,
            associations,
            default,
            ..
        } => {
            validate_inline_expr(control, internal_linkage_names, records, local_scopes)?;
            for association in associations {
                validate_inline_expr(
                    &association.expr,
                    internal_linkage_names,
                    records,
                    local_scopes,
                )?;
            }
            if let Some(default) = default {
                validate_inline_expr(default, internal_linkage_names, records, local_scopes)?;
            }
            Ok(())
        }
        Expr::VaArg { ap, .. } => {
            validate_inline_expr(ap, internal_linkage_names, records, local_scopes)
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            validate_inline_expr(condition, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(then_expr, internal_linkage_names, records, local_scopes)?;
            validate_inline_expr(else_expr, internal_linkage_names, records, local_scopes)
        }
        Expr::Call { callee, args, .. } => {
            validate_inline_expr(callee, internal_linkage_names, records, local_scopes)?;
            for arg in args {
                validate_inline_expr(arg, internal_linkage_names, records, local_scopes)?;
            }
            Ok(())
        }
        Expr::Member { base, .. } => {
            validate_inline_expr(base, internal_linkage_names, records, local_scopes)
        }
    }
}

fn type_is_modifiable_object(ty: &CType, records: &HashMap<usize, RecordType>) -> bool {
    if ty.is_const_qualified() {
        return false;
    }
    match ty.unqualified() {
        CType::Array(inner, _) => type_is_modifiable_object(inner, records),
        CType::Struct(id, _) | CType::Union(id, _) => records
            .get(id)
            .map(|record| {
                record
                    .members
                    .iter()
                    .all(|member| type_is_modifiable_object(&member.ty, records))
            })
            .unwrap_or(true),
        _ => true,
    }
}

fn normalize_function_decl_linkage(mut decl: FunctionDecl, linkage: Linkage) -> FunctionDecl {
    if linkage == Linkage::Internal {
        decl.storage_class = Some(StorageClass::Static);
    }
    decl
}

fn normalize_function_def_linkage(mut function: FunctionDef, linkage: Linkage) -> FunctionDef {
    if linkage == Linkage::Internal {
        function.storage_class = Some(StorageClass::Static);
    }
    function
}

fn normalize_object_linkage(mut decl: Declaration, linkage: Linkage) -> Declaration {
    if linkage == Linkage::Internal {
        decl.storage_class = Some(StorageClass::Static);
    }
    decl
}

fn finalize_tentative_definition(mut decl: Declaration) -> Declaration {
    if let CType::Array(inner, 0) = decl.ty.unqualified() {
        decl.ty = CType::array_of((**inner).clone(), 1);
    }
    decl.init = None;
    decl
}

fn ensure_external_symbol_kind(
    kinds: &HashMap<String, ExternalSymbolKind>,
    name: &str,
    expected: ExternalSymbolKind,
    span: Span,
) -> Result<(), Diagnostic> {
    if let Some(existing) = kinds.get(name) {
        if *existing != expected {
            return Err(Diagnostic::error(
                format!(
                    "external identifier {} is declared as both a {} and an {}",
                    name,
                    describe_external_symbol_kind(*existing),
                    describe_external_symbol_kind(expected),
                ),
                span,
            ));
        }
    }
    Ok(())
}

fn describe_external_symbol_kind(kind: ExternalSymbolKind) -> &'static str {
    match kind {
        ExternalSymbolKind::Function => "function",
        ExternalSymbolKind::Object => "object",
    }
}

fn merge_function_declaration(
    existing: &mut FunctionDecl,
    new_decl: &FunctionDecl,
    records: &HashMap<usize, RecordType>,
    enums: &HashMap<usize, EnumType>,
) -> Result<(), Diagnostic> {
    let existing_ty = function_decl_type(existing);
    let new_ty = function_decl_type(new_decl);
    let composite = composite_type(&existing_ty, &new_ty, records, enums).ok_or_else(|| {
        Diagnostic::error(
            format!(
                "conflicting declarations of function {}: the earlier declaration has type {}, but this one has type {}",
                new_decl.name, existing_ty, new_ty
            ),
            new_decl.span,
        )
    })?;
    *existing = function_decl_from_type(
        new_decl.name.clone(),
        composite,
        combine_function_storage(existing.storage_class, new_decl.storage_class),
        existing.linkage,
        existing.is_inline || new_decl.is_inline,
        existing.is_noreturn || new_decl.is_noreturn,
        existing.has_prototype || new_decl.has_prototype,
        combine_spans(existing.span, new_decl.span),
    )?;
    Ok(())
}

fn merge_object_declaration(
    existing: &mut Declaration,
    new_decl: &Declaration,
    records: &HashMap<usize, RecordType>,
    enums: &HashMap<usize, EnumType>,
) -> Result<(), Diagnostic> {
    let composite =
        composite_type(&existing.ty, &new_decl.ty, records, enums).ok_or_else(|| {
            Diagnostic::error(
                format!(
                    "conflicting declarations of object {}: the earlier declaration has type {}, but this one has type {}",
                    new_decl.name, existing.ty, new_decl.ty
                ),
                new_decl.declarator_span,
            )
        })?;
    existing.ty = composite;
    existing.storage_class = combine_object_storage(existing.storage_class, new_decl.storage_class);
    match (existing.alignment, new_decl.alignment) {
        (Some(lhs), Some(rhs)) if lhs != rhs => {
            return Err(Diagnostic::error(
                format!(
                    "conflicting alignment specifiers for object {}",
                    new_decl.name
                ),
                new_decl.span,
            ));
        }
        (None, Some(alignment)) => existing.alignment = Some(alignment),
        _ => {}
    }
    existing.span = combine_spans(existing.span, new_decl.span);
    Ok(())
}

fn remap_record_member(
    member: RecordMember,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> RecordMember {
    RecordMember {
        name: member.name,
        storage_name: member.storage_name,
        ty: remap_type(member.ty, record_map, enum_map),
        offset: member.offset,
        bit_width: member.bit_width,
        bit_width_span: member.bit_width_span,
        bit_offset: member.bit_offset,
        bit_storage_size: member.bit_storage_size,
        declaration_span: member.declaration_span,
    }
}

fn remap_external_declaration(
    decl: ExternalDeclaration,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> ExternalDeclaration {
    match decl {
        ExternalDeclaration::Function(function) => {
            ExternalDeclaration::Function(remap_function(function, record_map, enum_map))
        }
        ExternalDeclaration::FunctionDeclaration(decl) => ExternalDeclaration::FunctionDeclaration(
            remap_function_decl(decl, record_map, enum_map),
        ),
        ExternalDeclaration::ObjectDeclaration(decl) => {
            ExternalDeclaration::ObjectDeclaration(remap_declaration(decl, record_map, enum_map))
        }
    }
}

fn remap_function(
    function: FunctionDef,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> FunctionDef {
    FunctionDef {
        name: function.name,
        return_type: remap_type(function.return_type, record_map, enum_map),
        return_type_span: function.return_type_span,
        params: function
            .params
            .into_iter()
            .map(|param| remap_parameter(param, record_map, enum_map))
            .collect(),
        is_variadic: function.is_variadic,
        storage_class: function.storage_class,
        linkage: function.linkage,
        is_inline: function.is_inline,
        is_noreturn: function.is_noreturn,
        has_prototype: function.has_prototype,
        body: remap_block(function.body, record_map, enum_map),
        span: function.span,
    }
}

fn remap_function_decl(
    function: FunctionDecl,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> FunctionDecl {
    FunctionDecl {
        name: function.name,
        return_type: remap_type(function.return_type, record_map, enum_map),
        params: function
            .params
            .into_iter()
            .map(|param| remap_parameter(param, record_map, enum_map))
            .collect(),
        is_variadic: function.is_variadic,
        storage_class: function.storage_class,
        linkage: function.linkage,
        is_inline: function.is_inline,
        is_noreturn: function.is_noreturn,
        has_prototype: function.has_prototype,
        span: function.span,
    }
}

fn remap_parameter(
    param: Parameter,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> Parameter {
    Parameter {
        name: param.name,
        ty: remap_type(param.ty, record_map, enum_map),
        vla_bounds: param
            .vla_bounds
            .into_iter()
            .map(|expr| expr.map(|expr| remap_expr(expr, record_map, enum_map)))
            .collect(),
        static_array_bound: param
            .static_array_bound
            .map(|expr| remap_expr(expr, record_map, enum_map)),
        adjusted_from_array_or_function: param.adjusted_from_array_or_function,
        storage_class: param.storage_class,
        span: param.span,
    }
}

fn remap_block(
    block: Block,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> Block {
    Block {
        items: block
            .items
            .into_iter()
            .map(|item| match item {
                BlockItem::Declaration(decl) => {
                    BlockItem::Declaration(remap_declaration(decl, record_map, enum_map))
                }
                BlockItem::FunctionDeclaration(decl) => {
                    BlockItem::FunctionDeclaration(FunctionDecl {
                        name: decl.name,
                        return_type: remap_type(decl.return_type, record_map, enum_map),
                        params: decl
                            .params
                            .into_iter()
                            .map(|param| remap_parameter(param, record_map, enum_map))
                            .collect(),
                        is_variadic: decl.is_variadic,
                        storage_class: decl.storage_class,
                        linkage: decl.linkage,
                        is_inline: decl.is_inline,
                        is_noreturn: decl.is_noreturn,
                        has_prototype: decl.has_prototype,
                        span: decl.span,
                    })
                }
                BlockItem::Statement(stmt) => {
                    BlockItem::Statement(remap_statement(stmt, record_map, enum_map))
                }
            })
            .collect(),
        span: block.span,
    }
}

fn remap_statement(
    stmt: Statement,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> Statement {
    match stmt {
        Statement::Block(block) => Statement::Block(remap_block(block, record_map, enum_map)),
        Statement::Break(span) => Statement::Break(span),
        Statement::Continue(span) => Statement::Continue(span),
        Statement::DoWhile {
            body,
            condition,
            span,
        } => Statement::DoWhile {
            body: Box::new(remap_statement(*body, record_map, enum_map)),
            condition: remap_expr(condition, record_map, enum_map),
            span,
        },
        Statement::Expression(expr, span) => Statement::Expression(
            expr.map(|expr| remap_expr(expr, record_map, enum_map)),
            span,
        ),
        Statement::For {
            init,
            condition,
            step,
            body,
            span,
        } => Statement::For {
            init: init.map(|init| remap_for_init(init, record_map, enum_map)),
            condition: condition.map(|expr| remap_expr(expr, record_map, enum_map)),
            step: step.map(|expr| remap_expr(expr, record_map, enum_map)),
            body: Box::new(remap_statement(*body, record_map, enum_map)),
            span,
        },
        Statement::Goto { label, span } => Statement::Goto { label, span },
        Statement::If {
            condition,
            then_branch,
            else_branch,
            else_keyword_span,
            branch_keyword_span,
            span,
        } => Statement::If {
            condition: remap_expr(condition, record_map, enum_map),
            then_branch: Box::new(remap_statement(*then_branch, record_map, enum_map)),
            else_branch: else_branch
                .map(|stmt| Box::new(remap_statement(*stmt, record_map, enum_map))),
            else_keyword_span,
            branch_keyword_span,
            span,
        },
        Statement::Labeled {
            label,
            statement,
            span,
        } => Statement::Labeled {
            label: remap_switch_label(label, record_map, enum_map),
            statement: Box::new(remap_statement(*statement, record_map, enum_map)),
            span,
        },
        Statement::Return(expr, span) => Statement::Return(
            expr.map(|expr| remap_expr(expr, record_map, enum_map)),
            span,
        ),
        Statement::Switch { expr, body, span } => Statement::Switch {
            expr: remap_expr(expr, record_map, enum_map),
            body: remap_block(body, record_map, enum_map),
            span,
        },
        Statement::UserLabeled {
            label,
            statement,
            span,
        } => Statement::UserLabeled {
            label,
            statement: Box::new(remap_statement(*statement, record_map, enum_map)),
            span,
        },
        Statement::While {
            condition,
            body,
            span,
        } => Statement::While {
            condition: remap_expr(condition, record_map, enum_map),
            body: Box::new(remap_statement(*body, record_map, enum_map)),
            span,
        },
    }
}

fn remap_switch_label(
    label: SwitchLabel,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> SwitchLabel {
    match label {
        SwitchLabel::Case { expr, span } => SwitchLabel::Case {
            expr: remap_expr(expr, record_map, enum_map),
            span,
        },
        SwitchLabel::Default { span } => SwitchLabel::Default { span },
    }
}

fn remap_for_init(
    init: ForInit,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> ForInit {
    match init {
        ForInit::Declarations(decls) => ForInit::Declarations(
            decls
                .into_iter()
                .map(|decl| remap_declaration(decl, record_map, enum_map))
                .collect(),
        ),
        ForInit::Expression(expr) => ForInit::Expression(remap_expr(expr, record_map, enum_map)),
    }
}

fn remap_declaration(
    decl: Declaration,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> Declaration {
    Declaration {
        name: decl.name,
        ty: remap_type(decl.ty, record_map, enum_map),
        vla_bounds: decl
            .vla_bounds
            .into_iter()
            .map(|expr| expr.map(|expr| remap_expr(expr, record_map, enum_map)))
            .collect(),
        storage_class: decl.storage_class,
        linkage: decl.linkage,
        alignment: decl.alignment,
        init: decl
            .init
            .map(|init| remap_initializer(init, record_map, enum_map)),
        declarator_span: decl.declarator_span,
        span: decl.span,
    }
}

fn remap_initializer(
    init: Initializer,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> Initializer {
    match init {
        Initializer::Expr(expr) => Initializer::Expr(remap_expr(expr, record_map, enum_map)),
        Initializer::List { items, span } => Initializer::List {
            items: items
                .into_iter()
                .map(|item| remap_initializer_item(item, record_map, enum_map))
                .collect(),
            span,
        },
    }
}

fn remap_initializer_item(
    item: InitializerItem,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> InitializerItem {
    InitializerItem {
        designators: item.designators.into_iter().map(remap_designator).collect(),
        initializer: remap_initializer(item.initializer, record_map, enum_map),
        span: item.span,
    }
}

fn remap_designator(designator: Designator) -> Designator {
    match designator {
        Designator::Member(name, span) => Designator::Member(name, span),
        Designator::Index(index, span) => Designator::Index(index, span),
    }
}

fn remap_expr(
    expr: Expr,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> Expr {
    match expr {
        Expr::Number(literal, span) => Expr::Number(literal, span),
        Expr::CharLiteral(value, span) => Expr::CharLiteral(value, span),
        Expr::WideCharLiteral(value, span) => Expr::WideCharLiteral(value, span),
        Expr::Utf16CharLiteral(value, span) => Expr::Utf16CharLiteral(value, span),
        Expr::Utf32CharLiteral(value, span) => Expr::Utf32CharLiteral(value, span),
        Expr::StringLiteral(text, span) => Expr::StringLiteral(text, span),
        Expr::WideStringLiteral(text, span) => Expr::WideStringLiteral(text, span),
        Expr::Utf16StringLiteral(text, span) => Expr::Utf16StringLiteral(text, span),
        Expr::Utf32StringLiteral(text, span) => Expr::Utf32StringLiteral(text, span),
        Expr::Variable(name, span) => Expr::Variable(name, span),
        Expr::Unary { op, expr, span } => Expr::Unary {
            op,
            expr: Box::new(remap_expr(*expr, record_map, enum_map)),
            span,
        },
        Expr::Postfix { op, expr, span } => Expr::Postfix {
            op,
            expr: Box::new(remap_expr(*expr, record_map, enum_map)),
            span,
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op,
            lhs: Box::new(remap_expr(*lhs, record_map, enum_map)),
            rhs: Box::new(remap_expr(*rhs, record_map, enum_map)),
            span,
        },
        Expr::Subscript { base, index, span } => Expr::Subscript {
            base: Box::new(remap_expr(*base, record_map, enum_map)),
            index: Box::new(remap_expr(*index, record_map, enum_map)),
            span,
        },
        Expr::Assign { lhs, rhs, span } => Expr::Assign {
            lhs: Box::new(remap_expr(*lhs, record_map, enum_map)),
            rhs: Box::new(remap_expr(*rhs, record_map, enum_map)),
            span,
        },
        Expr::CompoundAssign { op, lhs, rhs, span } => Expr::CompoundAssign {
            op,
            lhs: Box::new(remap_expr(*lhs, record_map, enum_map)),
            rhs: Box::new(remap_expr(*rhs, record_map, enum_map)),
            span,
        },
        Expr::SizeofType {
            ty,
            vla_bounds,
            span,
        } => Expr::SizeofType {
            ty: remap_type(ty, record_map, enum_map),
            vla_bounds: vla_bounds
                .into_iter()
                .map(|expr| expr.map(|expr| remap_expr(expr, record_map, enum_map)))
                .collect(),
            span,
        },
        Expr::SizeofExpr { expr, span } => Expr::SizeofExpr {
            expr: Box::new(remap_expr(*expr, record_map, enum_map)),
            span,
        },
        Expr::OffsetOf {
            ty,
            designators,
            span,
        } => Expr::OffsetOf {
            ty: remap_type(ty, record_map, enum_map),
            designators: designators.into_iter().map(remap_designator).collect(),
            span,
        },
        Expr::Cast {
            ty,
            vla_bounds,
            expr,
            span,
        } => Expr::Cast {
            ty: remap_type(ty, record_map, enum_map),
            vla_bounds: vla_bounds
                .into_iter()
                .map(|bound| bound.map(|expr| remap_expr(expr, record_map, enum_map)))
                .collect(),
            expr: Box::new(remap_expr(*expr, record_map, enum_map)),
            span,
        },
        Expr::CompoundLiteral {
            ty,
            vla_bounds,
            initializer,
            span,
        } => Expr::CompoundLiteral {
            ty: remap_type(ty, record_map, enum_map),
            vla_bounds: vla_bounds
                .into_iter()
                .map(|bound| bound.map(|expr| remap_expr(expr, record_map, enum_map)))
                .collect(),
            initializer: Box::new(remap_initializer(*initializer, record_map, enum_map)),
            span,
        },
        Expr::GenericSelection {
            control,
            associations,
            default,
            span,
        } => Expr::GenericSelection {
            control: Box::new(remap_expr(*control, record_map, enum_map)),
            associations: associations
                .into_iter()
                .map(|association| crate::ast::GenericAssociation {
                    ty: remap_type(association.ty, record_map, enum_map),
                    expr: remap_expr(association.expr, record_map, enum_map),
                    span: association.span,
                })
                .collect(),
            default: default.map(|expr| Box::new(remap_expr(*expr, record_map, enum_map))),
            span,
        },
        Expr::VaArg { ap, ty, span } => Expr::VaArg {
            ap: Box::new(remap_expr(*ap, record_map, enum_map)),
            ty: remap_type(ty, record_map, enum_map),
            span,
        },
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            span,
        } => Expr::Conditional {
            condition: Box::new(remap_expr(*condition, record_map, enum_map)),
            then_expr: Box::new(remap_expr(*then_expr, record_map, enum_map)),
            else_expr: Box::new(remap_expr(*else_expr, record_map, enum_map)),
            span,
        },
        Expr::Call {
            callee,
            args,
            declared_callee_type,
            span,
        } => Expr::Call {
            callee: Box::new(remap_expr(*callee, record_map, enum_map)),
            args: args
                .into_iter()
                .map(|arg| remap_expr(arg, record_map, enum_map))
                .collect(),
            declared_callee_type: declared_callee_type
                .map(|ty| remap_type(ty, record_map, enum_map)),
            span,
        },
        Expr::Member { base, member, span } => Expr::Member {
            base: Box::new(remap_expr(*base, record_map, enum_map)),
            member,
            span,
        },
    }
}

fn remap_type(
    ty: CType,
    record_map: &std::collections::HashMap<usize, usize>,
    enum_map: &std::collections::HashMap<usize, usize>,
) -> CType {
    match ty {
        CType::Struct(id, tag) => CType::Struct(*record_map.get(&id).unwrap_or(&id), tag),
        CType::Union(id, tag) => CType::Union(*record_map.get(&id).unwrap_or(&id), tag),
        CType::Enum(id, tag) => CType::Enum(*enum_map.get(&id).unwrap_or(&id), tag),
        CType::Function(ret, params, is_variadic) => {
            let params = params
                .into_iter()
                .map(|param| remap_type(param, record_map, enum_map))
                .collect();
            if is_variadic {
                CType::variadic_function(remap_type(*ret, record_map, enum_map), params)
            } else {
                CType::function(remap_type(*ret, record_map, enum_map), params)
            }
        }
        CType::Qualified(inner, qualifiers) => {
            CType::qualified(remap_type(*inner, record_map, enum_map), qualifiers)
        }
        CType::Pointer(inner) => CType::pointer_to(remap_type(*inner, record_map, enum_map)),
        CType::Array(inner, len) => CType::array_of(remap_type(*inner, record_map, enum_map), len),
        other => other,
    }
}

fn function_decl_from_definition(function: &FunctionDef) -> FunctionDecl {
    FunctionDecl {
        name: function.name.clone(),
        return_type: function.return_type.clone(),
        params: function.params.clone(),
        is_variadic: function.is_variadic,
        storage_class: function.storage_class,
        linkage: function.linkage,
        is_inline: function.is_inline,
        is_noreturn: function.is_noreturn,
        has_prototype: function.has_prototype,
        span: function.span,
    }
}

fn function_decl_type(decl: &FunctionDecl) -> CType {
    if !decl.has_prototype {
        return CType::function(decl.return_type.clone(), Vec::new());
    }
    if decl.is_variadic {
        CType::variadic_function(
            decl.return_type.clone(),
            decl.params.iter().map(|param| param.ty.clone()).collect(),
        )
    } else {
        CType::function(
            decl.return_type.clone(),
            decl.params.iter().map(|param| param.ty.clone()).collect(),
        )
    }
}

fn old_style_promoted_definition_type(function: &FunctionDef) -> CType {
    let params = function
        .params
        .iter()
        .map(|param| match param.ty.unqualified() {
            CType::Float => CType::Double,
            CType::Bool
            | CType::Char
            | CType::SignedChar
            | CType::UnsignedChar
            | CType::Short
            | CType::UnsignedShort
            | CType::Enum(..) => CType::Int,
            _ => param.ty.clone(),
        })
        .collect();
    CType::function(function.return_type.clone(), params)
}

fn function_decl_from_type(
    name: String,
    ty: CType,
    storage_class: Option<StorageClass>,
    linkage: Linkage,
    is_inline: bool,
    is_noreturn: bool,
    has_prototype: bool,
    span: Span,
) -> Result<FunctionDecl, Diagnostic> {
    let CType::Function(return_type, params, is_variadic) = ty.unqualified() else {
        return Err(Diagnostic::error("expected function type", span));
    };
    Ok(FunctionDecl {
        name,
        return_type: (**return_type).clone(),
        params: params
            .iter()
            .cloned()
            .map(|ty| Parameter {
                name: None,
                ty,
                vla_bounds: Vec::new(),
                static_array_bound: None,
                adjusted_from_array_or_function: false,
                storage_class: None,
                span,
            })
            .collect(),
        is_variadic: *is_variadic,
        storage_class,
        linkage,
        is_inline,
        is_noreturn,
        has_prototype,
        span,
    })
}

fn combine_function_storage(
    lhs: Option<StorageClass>,
    rhs: Option<StorageClass>,
) -> Option<StorageClass> {
    match (lhs, rhs) {
        (Some(StorageClass::Static), _) | (_, Some(StorageClass::Static)) => {
            Some(StorageClass::Static)
        }
        (Some(StorageClass::Extern), _) | (_, Some(StorageClass::Extern)) => {
            Some(StorageClass::Extern)
        }
        (lhs, None) => lhs,
        (None, rhs) => rhs,
        (lhs, rhs) => lhs.or(rhs),
    }
}

fn combine_object_storage(
    lhs: Option<StorageClass>,
    rhs: Option<StorageClass>,
) -> Option<StorageClass> {
    match (lhs, rhs) {
        (Some(StorageClass::Static), _) | (_, Some(StorageClass::Static)) => {
            Some(StorageClass::Static)
        }
        (Some(StorageClass::Extern), Some(StorageClass::Extern)) => Some(StorageClass::Extern),
        (Some(StorageClass::Extern), None) | (None, Some(StorageClass::Extern)) => None,
        (lhs, None) => lhs,
        (None, rhs) => rhs,
        (lhs, rhs) => lhs.or(rhs),
    }
}

pub(crate) fn composite_type(
    lhs: &CType,
    rhs: &CType,
    records: &HashMap<usize, RecordType>,
    enums: &HashMap<usize, EnumType>,
) -> Option<CType> {
    let mut seen_records = HashSet::new();
    let mut seen_enums = HashSet::new();
    composite_type_inner(lhs, rhs, records, enums, &mut seen_records, &mut seen_enums)
}

fn composite_type_inner(
    lhs: &CType,
    rhs: &CType,
    records: &HashMap<usize, RecordType>,
    enums: &HashMap<usize, EnumType>,
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
                return Some(CType::function(return_type, rhs_params.clone()));
            }
            if rhs_params.is_empty() && !rhs_variadic {
                if *lhs_variadic
                    || !function_prototype_compatible_with_unspecified_parameters(lhs_params)
                {
                    return None;
                }
                return Some(CType::function(return_type, lhs_params.clone()));
            }
            if lhs_variadic != rhs_variadic || lhs_params.len() != rhs_params.len() {
                return None;
            }
            let mut params = Vec::new();
            for (lhs_param, rhs_param) in lhs_params.iter().zip(rhs_params) {
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
                lhs_tag.as_deref(),
                *rhs_id,
                rhs_tag.as_deref(),
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
            lhs_tag.as_deref(),
            *rhs_id,
            rhs_tag.as_deref(),
            RecordKindForComposite::Union,
            records,
            enums,
            seen_records,
            seen_enums,
        )
        .then(|| select_record_composite(lhs, rhs, records)),
        (CType::Enum(lhs_id, lhs_tag), CType::Enum(rhs_id, rhs_tag)) => compatible_enum_types(
            *lhs_id,
            lhs_tag.as_deref(),
            *rhs_id,
            rhs_tag.as_deref(),
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
            .all(|param| type_unchanged_by_default_argument_promotions(param))
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
    records: &HashMap<usize, RecordType>,
    enums: &HashMap<usize, EnumType>,
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
    enums: &HashMap<usize, EnumType>,
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
    records: &HashMap<usize, RecordType>,
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

fn select_enum_composite(lhs: &CType, rhs: &CType, enums: &HashMap<usize, EnumType>) -> CType {
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

fn combine_spans(lhs: Span, rhs: Span) -> Span {
    if lhs.file == rhs.file {
        lhs.merge(rhs)
    } else {
        lhs
    }
}

#[cfg(test)]
mod browser_api_tests {
    use super::*;

    #[test]
    fn no_visualization_entry_point_runs_without_browser_limits_or_trace_data() {
        let source = b"int main(void) { unsigned long sum = 0; for (int i = 0; i < 12000; ++i) sum += i; return sum != 71994000; }\n";
        let result_ptr = unsafe {
            cboxes_run_source_without_visualization(
                source.as_ptr(),
                source.len(),
                std::ptr::null(),
                0,
                0x1000,
            )
        };
        let result_len = cboxes_last_result_len();
        let json = unsafe { std::slice::from_raw_parts(result_ptr, result_len) }.to_vec();
        unsafe { cboxes_free(result_ptr, result_len) };
        let json = String::from_utf8(json).unwrap();

        assert!(json.contains("\"ok\":true"), "{json}");
        assert!(json.contains("\"exitStatus\":0"), "{json}");
        assert!(json.contains("\"state\":[]"), "{json}");
        assert!(json.contains("\"trace\":[]"), "{json}");
        assert!(json.contains("\"executionLimit\":null"), "{json}");
    }

    #[test]
    fn browser_diagnostics_preserve_the_full_source_range() {
        let source = "int main(void) {\n  char a = \"hi\";\n}\n";
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();

        assert_eq!(range.path, PathBuf::from("program.c"));
        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (1, 11, 1, 15)
        );

        let json = cboxes_diagnostic_json(
            &diagnostic,
            &HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]),
        );
        assert!(json.contains("\"line\":1,\"column\":11"));
        assert!(json.contains("\"endLine\":1,\"endColumn\":15"));
    }

    #[test]
    fn browser_eof_diagnostics_stay_at_the_original_source_end() {
        let source = "int f(int x) {\n}\n\nint a";
        let run = cboxes_run_virtual_sources(
            vec![(PathBuf::from("program.c"), source.to_owned())],
            &RunOptions::default(),
            false,
        )
        .unwrap();
        let diagnostic = run.result.unwrap_err();
        let json = cboxes_diagnostic_json(&diagnostic, &run.source_display);

        assert!(json.contains("\"file\":\"program.c\""), "{json}");
        assert!(json.contains("\"line\":3,\"column\":5"), "{json}");
    }

    #[test]
    fn missing_semicolon_at_an_include_boundary_points_into_the_header() {
        let files = vec![
            (
                PathBuf::from("main.c"),
                "#include \"bad.h\"\nint main(void) {}".to_owned(),
            ),
            (PathBuf::from("bad.h"), "extern int object".to_owned()),
        ];
        let run = cboxes_run_virtual_sources(files, &RunOptions::default(), false).unwrap();
        let diagnostic = run.result.unwrap_err();
        let range = diagnostic.display_range().unwrap();

        assert_eq!(range.path, PathBuf::from("bad.h"));
        assert_eq!((range.start_line, range.start_column), (0, 11));
    }

    #[test]
    fn member_and_initializer_diagnostics_do_not_blame_following_punctuation() {
        let cases = [
            ("struct S { int x; char x; }; int main(void) {}\n", "x"),
            ("struct S { void x; }; int main(void) {}\n", "x"),
            ("struct S { int x : 0; }; int main(void) {}\n", "0"),
            ("struct S { int a[]; int x; }; int main(void) {}\n", "a[]"),
            ("int main(void) { int x = {1, 2}; }\n", "2"),
            ("int main(void) { int a[2] = {[3] = 1}; }\n", "[3]"),
        ];

        for (source, expected) in cases {
            let diagnostic = run_source("program.c", source).unwrap_err();
            let range = diagnostic.display_range().unwrap();
            let start = source.rfind(expected).unwrap();
            assert_eq!(
                (range.start_column, range.end_column),
                (start, start + expected.len()),
                "{expected}: {}",
                diagnostic.render()
            );
        }
    }

    #[test]
    fn conversions_link_call_parameters_and_function_return_types() {
        let cases = [
            (
                "void f(int count) {} int main(void) { f(\"wrong\"); }\n",
                "int count",
            ),
            ("int *f(void) { return 1; } int main(void) {}\n", "int"),
        ];

        for (source, destination) in cases {
            let diagnostic = run_source("program.c", source).unwrap_err();
            let annotation = diagnostic.display_annotations().first().unwrap();
            let start = source.find(destination).unwrap();
            assert_eq!(annotation.id, "destination");
            assert_eq!(
                (annotation.range.start_column, annotation.range.end_column),
                (start, start + destination.len())
            );
        }
    }

    #[test]
    fn c11_type_context_diagnostics_highlight_the_type_or_compound_literal() {
        let cases = [
            (
                "struct S; int main(void) { return _Alignof(struct S); }\n",
                "_Alignof(struct S)",
            ),
            (
                "struct S; int main(void) { return _Generic(1, struct S: 1, default: 2); }\n",
                "struct S",
            ),
            (
                "int main(void) { return _Generic(1, int: 1, signed int: 2); }\n",
                "signed int",
            ),
            (
                "struct S; int main(void) { (struct S){0}; }\n",
                "(struct S){0}",
            ),
        ];

        for (source, expected) in cases {
            let diagnostic = run_source("program.c", source).unwrap_err();
            let range = diagnostic.display_range().unwrap();
            let start = source.rfind(expected).unwrap();
            assert_eq!(
                (range.start_column, range.end_column),
                (start, start + expected.len()),
                "{expected}: {}",
                diagnostic.render()
            );
        }
    }

    #[test]
    fn hexadecimal_integer_constants_with_e_digits_remain_integer_constants() {
        let source = "enum E { VALUE = 0xdead }; int main(void) { return VALUE != 0xdead; }\n";
        let result = run_source("program.c", source).unwrap();
        assert_eq!(result.exit_status, 0);
    }

    #[test]
    fn c11_rejects_undeclared_old_style_parameters() {
        let source = "int function(value) { return value; }\nint main(void) { return 0; }\n";
        let diagnostic = run_source("program.c", source).unwrap_err();

        assert!(
            diagnostic
                .render()
                .contains("old-style parameter value is missing its declaration"),
            "{}",
            diagnostic.render()
        );
        let range = diagnostic.display_range().unwrap();
        let start = source.find("value").unwrap();
        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (0, start, 0, start + "value".len())
        );
    }

    #[test]
    fn c11_still_accepts_declared_old_style_parameters() {
        let source = "int identity(value) int value; { return value; }\nint main(void) { return identity(7) != 7; }\n";
        let result = run_source("program.c", source).unwrap();
        assert_eq!(result.exit_status, 0);
    }

    #[test]
    fn main_must_use_a_standard_parameter_list() {
        for source in [
            "int main(int value) { return value; }\n",
            "int main(value) int value; { return value; }\n",
        ] {
            let diagnostic = run_source("program.c", source).unwrap_err();
            assert!(
                diagnostic
                    .render()
                    .contains("this definition gives main 1 parameter"),
                "{}",
                diagnostic.render()
            );
            if source.starts_with("int main(int value)") {
                let start = source.find("int value").unwrap();
                let range = diagnostic.display_range().unwrap();
                assert_eq!(
                    (
                        range.start_line,
                        range.start_column,
                        range.end_line,
                        range.end_column,
                    ),
                    (0, start, 0, start + "int value".len())
                );
            }
        }

        for source in [
            "int main(void) { return 0; }\n",
            "int main() { return 0; }\n",
            "int main(int argc, char **argv) { return argc < 1 || argv[argc] != 0; }\n",
        ] {
            let result = run_source("program.c", source).unwrap();
            assert_eq!(result.exit_status, 0, "{source}");
        }
    }

    #[test]
    fn incomplete_function_header_is_not_misdiagnosed_as_an_unknown_type() {
        for source in ["int main()\n", "int function(value) int value;\n"] {
            let diagnostic = run_source("program.c", source).unwrap_err();
            assert!(
                diagnostic
                    .render()
                    .contains("is missing a body or semicolon"),
                "{}",
                diagnostic.render()
            );
            let range = diagnostic.display_range().unwrap();
            assert_eq!(range.start_line, 0);
            assert!(range.start_column > 0, "{}", diagnostic.render());
        }
    }

    #[test]
    fn missing_main_points_to_the_end_instead_of_blaming_another_function() {
        let source = "int f(int value) { return value; }\n";
        let diagnostic = run_source("program.c", source).unwrap_err();

        assert!(
            diagnostic.render().contains("program does not define main"),
            "{}",
            diagnostic.render()
        );
        let range = diagnostic.display_range().unwrap();
        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (0, source.trim_end().len(), 0, source.trim_end().len())
        );
    }

    #[test]
    fn prefix_unary_diagnostics_include_the_operator_in_their_range() {
        let cases = [
            ("int *b = 0; int *c = *b;", "*b"),
            ("int *b = 0; int *c = *(b);", "*(b)"),
            ("int ***b = 0; int **c = **b;", "**b"),
            ("int b; int c = &b;", "&b"),
            ("struct S { int x; } s; int x = +s;", "+s"),
            ("struct S { int x; } s; int x = -s;", "-s"),
            ("struct S { int x; } s; int x = !s;", "!s"),
            ("double d = 1; int x = ~d;", "~d"),
            ("int x = ++3;", "++3"),
            ("int x = --3;", "--3"),
        ];

        for (body, expression) in cases {
            let source = format!("int main(void) {{ {body} }}\n");
            let diagnostic = run_source("program.c", &source).unwrap_err();
            let range = diagnostic.display_range().unwrap();
            let start = source.rfind(expression).unwrap();
            assert_eq!(
                (
                    range.start_line,
                    range.start_column,
                    range.end_line,
                    range.end_column,
                ),
                (0, start, 0, start + expression.len()),
                "{expression}: {}",
                diagnostic.render()
            );
        }
    }

    #[test]
    fn conversion_diagnostics_highlight_the_converted_expression_in_every_context() {
        let cases = [
            ("int main(void) { int **p; int *q = **p; }\n", "**p"),
            ("int main(void) { int **p; int *q; q = **p; }\n", "**p"),
            (
                "void f(int **value) {} int main(void) { int **p; f(*p); }\n",
                "*p",
            ),
            (
                "int **f(void) { int **p; return *p; } int main(void) { return 0; }\n",
                "*p",
            ),
        ];

        for (source, expression) in cases {
            let diagnostic = run_source("program.c", source).unwrap_err();
            let range = diagnostic.display_range().unwrap();
            let start = source.rfind(expression).unwrap();
            assert_eq!(
                (
                    range.start_line,
                    range.start_column,
                    range.end_line,
                    range.end_column,
                ),
                (0, start, 0, start + expression.len()),
                "{expression}: {}",
                diagnostic.render()
            );
        }
    }

    #[test]
    fn aggregate_initializer_diagnostics_identify_and_annotate_the_failing_member() {
        let cases = [
            (
                "struct S { int a; int b; }; int main(void) { struct S s = {0, \"x\"}; }\n",
                "b",
                "int b",
            ),
            (
                "struct S { int a; int b; }; int main(void) { struct S s = {.b = \"x\"}; }\n",
                "b",
                "int b",
            ),
            (
                "union U { int a; double d; }; int main(void) { union U u = {.a = \"x\"}; }\n",
                "a",
                "int a",
            ),
            (
                "struct I { int x; }; struct O { int lead; struct I inner; }; int main(void) { struct O o = {0, {\"x\"}}; }\n",
                "x",
                "int x",
            ),
        ];

        for (source, member_name, member_declaration) in cases {
            let diagnostic = run_source("program.c", source).unwrap_err();
            assert!(
                diagnostic
                    .render()
                    .contains(&format!("initializing member {member_name}")),
                "{}",
                diagnostic.render()
            );
            let annotation = diagnostic.display_annotations().first().unwrap();
            let member_start = source.find(member_declaration).unwrap() + "int ".len();
            assert_eq!(annotation.id, "destination");
            assert_eq!(
                (
                    annotation.range.start_line,
                    annotation.range.start_column,
                    annotation.range.end_line,
                    annotation.range.end_column,
                ),
                (0, member_start, 0, member_start + member_name.len()),
            );

            let json = cboxes_diagnostic_json(
                &diagnostic,
                &HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]),
            );
            assert!(json.contains("\"annotations\":[{\"id\":\"destination\""));
        }
    }

    #[test]
    fn unterminated_literals_highlight_from_the_quote_to_the_line_end() {
        let source = "int main(void) {\n  char a[] = \"hi\n}\n";
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();

        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (1, 13, 1, 16)
        );
    }

    #[test]
    fn preprocessor_fallback_ranges_cover_the_relevant_line() {
        let source = "  #define\nint main(void) { return 0; }\n";
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();

        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (0, 2, 0, 9)
        );
    }

    #[test]
    fn diagnostic_columns_are_mapped_back_across_macro_expansion() {
        let source = "#include <limits.h>\nint main(void){int x=INT_MIN; int y=-1; return x/y;}\n";
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();
        let expression_start = source.lines().nth(1).unwrap().find("x/y").unwrap();

        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (1, expression_start, 1, expression_start + 3)
        );
    }

    #[test]
    fn translation_unit_validation_errors_keep_their_source_range() {
        let source = "int f(void){return 1;} int f(void){return 2;} int main(void){return f();}\n";
        let diagnostic = run_source("program.c", source).unwrap_err();
        let range = diagnostic.display_range().unwrap();
        let definition_start = source.match_indices("int f").nth(1).unwrap().0;
        let start = definition_start + source[definition_start..].find('f').unwrap();
        let end = start + source[start..].find('}').unwrap() + 1;

        assert_eq!(
            (
                range.start_line,
                range.start_column,
                range.end_line,
                range.end_column,
            ),
            (0, start, 0, end)
        );
    }

    #[test]
    fn virtual_project_links_sources_and_resolves_headers() {
        let files = vec![
            (
                PathBuf::from("program.c"),
                "#include \"answer.h\"\n#include <stdio.h>\nint main(void) {\n  int x = answer();\n  printf(\"%d\\n\", x);\n}\n"
                    .to_owned(),
            ),
            (
                PathBuf::from("answer.c"),
                "#include \"answer.h\"\nint answer(void) { return 42; }\n".to_owned(),
            ),
            (
                PathBuf::from("answer.h"),
                "int answer(void);\n".to_owned(),
            ),
        ];
        let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();

        assert_eq!(result.stdout, "42\n");
        assert_eq!(result.state[0].name, "x");
        assert_eq!(result.state[0].value, "42");
        assert!(result.trace.iter().all(|event| event.file == "program.c"));
        assert!(result.trace.iter().any(|event| event.start_line == 3));
    }

    #[test]
    fn virtual_project_normalizes_a_missing_final_newline_in_headers() {
        let files = vec![
            (
                PathBuf::from("program.c"),
                "#include \"answer.h\"\nint main(void) { return answer() != 42; }".to_owned(),
            ),
            (
                PathBuf::from("answer.c"),
                "#include \"answer.h\"\nint answer(void) { return 42; }".to_owned(),
            ),
            (PathBuf::from("answer.h"), "int answer(void);".to_owned()),
        ];

        let (files, _, _) = cboxes_prepare_virtual_sources(files, false).unwrap();
        assert!(files.iter().all(|(_, source)| source.ends_with('\n')));
        let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();
        assert_eq!(result.exit_status, 0);
    }

    #[test]
    fn browser_state_exposes_semantic_metadata_and_aliases() {
        let source = "int main(void) { double value = 0.1; double *p = &value; double **pp = &p; int items[2] = {1, 2}; }\n";
        let result = run_source("program.c", source).unwrap();
        let value = result
            .state
            .iter()
            .find(|item| item.name == "value")
            .unwrap();
        assert_eq!(value.type_info.kind, "floating");
        assert_eq!(value.type_info.size, Some(8));
        assert_eq!(value.display_value, "0.1");
        assert!(value.exact_value.starts_with("0.10000000000000000555"));
        assert_eq!(value.aliases, ["*p", "**pp"]);

        let items = result
            .state
            .iter()
            .find(|item| item.name == "items")
            .unwrap();
        assert_eq!(items.type_info.kind, "array");
        assert_eq!(items.type_info.array_shape, [2]);

        let json = cboxes_success_json(
            &result,
            &HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]),
        );
        assert!(json.contains("\"typeInfo\""));
        assert!(json.contains("\"exactValue\""));
        assert!(json.contains("\"aliases\":[\"*p\",\"**pp\"]"));
    }

    #[test]
    fn browser_state_uses_c_syntax_for_type_names() {
        let source = concat!(
            "int global;\n",
            "int *returns_pointer(void) { return &global; }\n",
            "int main(void) {\n",
            "  int (*function_pointer)(void) = main;\n",
            "  int *(*pointer_to_pointer_returning_function)(void) = returns_pointer;\n",
            "  int values[3] = {0};\n",
            "  int (*pointer_to_array)[3] = &values;\n",
            "  int *array_of_pointers[3] = {0};\n",
            "  int (*pointer_to_incomplete_array)[];\n",
            "  const int *pointer_to_const = 0;\n",
            "  int * const const_pointer = 0;\n",
            "  _Bool flag = 0;\n",
            "}\n",
        );
        let result = run_source("program.c", source).unwrap();
        let type_of = |name: &str| {
            result
                .state
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.ty.as_str())
        };
        let help_for = |name: &str| {
            result
                .state
                .iter()
                .find(|item| item.name == name)
                .and_then(|item| item.type_info.help.as_deref())
        };
        let info_for = |name: &str| {
            &result
                .state
                .iter()
                .find(|item| item.name == name)
                .unwrap()
                .type_info
        };

        assert_eq!(type_of("function_pointer"), Some("int (*)(void)"));
        assert_eq!(
            help_for("function_pointer"),
            Some("pointer to function taking no arguments and returning int")
        );
        let function_pointer_tree = info_for("function_pointer").help_tree.as_ref().unwrap();
        assert_eq!(function_pointer_tree.kind, "pointer");
        assert_eq!(function_pointer_tree.children[0].relation, "to");
        assert_eq!(function_pointer_tree.children[0].node.kind, "function");
        assert_eq!(info_for("function_pointer").help_type_names, ["int"]);
        assert_eq!(
            type_of("pointer_to_pointer_returning_function"),
            Some("int*(*)(void)")
        );
        assert_eq!(
            help_for("pointer_to_pointer_returning_function"),
            Some("pointer to function taking no arguments and returning pointer to int")
        );
        assert_eq!(type_of("pointer_to_array"), Some("int (*)[3]"));
        assert_eq!(
            help_for("pointer_to_array"),
            Some("pointer to array of 3 ints")
        );
        assert_eq!(type_of("array_of_pointers"), Some("int*[3]"));
        assert_eq!(
            help_for("array_of_pointers"),
            Some("array of 3 pointers to int")
        );
        assert_eq!(type_of("pointer_to_incomplete_array"), Some("int (*)[]"));
        assert_eq!(
            help_for("pointer_to_incomplete_array"),
            Some("pointer to array of unknown length containing ints")
        );
        let incomplete_array_tree = info_for("pointer_to_incomplete_array")
            .help_tree
            .as_ref()
            .unwrap();
        assert_eq!(incomplete_array_tree.kind, "pointer");
        assert_eq!(incomplete_array_tree.children[0].node.kind, "array");
        assert_eq!(
            incomplete_array_tree.children[0].node.label,
            "array of unknown length"
        );
        assert_eq!(info_for("pointer_to_incomplete_array").size, Some(8));
        assert_eq!(type_of("pointer_to_const"), Some("const int*"));
        assert_eq!(help_for("pointer_to_const"), None);
        assert!(info_for("pointer_to_const").help_tree.is_none());
        assert!(info_for("pointer_to_const").help_type_names.is_empty());
        assert_eq!(type_of("const_pointer"), Some("int* const"));
        assert_eq!(help_for("const_pointer"), None);
        assert!(info_for("const_pointer").help_tree.is_none());
        assert_eq!(type_of("flag"), Some("_Bool"));
        assert_eq!(help_for("flag"), None);
    }

    #[test]
    fn browser_state_displays_struct_and_union_values() {
        let source = format!(
            "{}\n",
            r#"
            struct Point { int x; double y; };
            struct Shape { struct Point point; int sides[2]; };
            union Number { int integer; double decimal; };

            int main(void) {
                struct Shape shape = {{3, 2.5}, {4, 5}};
                union Number number = {.decimal = 1.25};
            }
        "#
            .trim()
        );
        let result = run_source("program.c", source).unwrap();

        let shape = result
            .state
            .iter()
            .find(|item| item.name == "shape")
            .unwrap();
        assert_eq!(shape.value, "");
        assert_eq!(shape.type_info.kind, "aggregate");
        assert_eq!(shape.aggregate_kind.as_deref(), Some("struct"));
        let state_value = |name: &str| {
            result
                .state
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.value.as_str())
        };
        assert_eq!(state_value("shape.point.x"), Some("3"));
        assert_eq!(state_value("shape.point.y"), Some("2.5"));
        assert_eq!(state_value("shape.sides[0]"), Some("4"));
        assert_eq!(state_value("shape.sides[1]"), Some("5"));
        let point = result
            .state
            .iter()
            .find(|item| item.name == "shape.point")
            .unwrap();
        assert_eq!(point.aggregate_root.as_deref(), Some("shape"));
        assert_eq!(point.aggregate_path, ["point"]);
        let point_x = result
            .state
            .iter()
            .find(|item| item.name == "shape.point.x")
            .unwrap();
        let point_y = result
            .state
            .iter()
            .find(|item| item.name == "shape.point.y")
            .unwrap();
        assert_eq!(point_x.address, shape.address);
        assert_eq!(point_y.address, shape.address.map(|address| address + 8));

        let number = result
            .state
            .iter()
            .find(|item| item.name == "number")
            .unwrap();
        assert_eq!(number.value, "");
        assert_eq!(number.type_info.kind, "aggregate");
        assert_eq!(number.aggregate_kind.as_deref(), Some("union"));
        assert_eq!(state_value("number.decimal"), Some("1.25"));
        assert!(
            result
                .state
                .iter()
                .all(|item| item.name != "number.integer")
        );
    }

    #[test]
    fn browser_state_does_not_invent_a_union_member_after_byte_copying() {
        let source = concat!(
            "#include <string.h>\n",
            "union Number { int integer; double decimal; };\n",
            "int main(void) {\n",
            "  union Number source = {.integer = 7};\n",
            "  union Number copied;\n",
            "  memcpy(&copied, &source, sizeof copied);\n",
            "}\n",
        );
        let result = run_source("program.c", source).unwrap();
        let copied = result
            .state
            .iter()
            .find(|item| item.name == "copied")
            .unwrap();

        assert_eq!(copied.value, "active member unknown");
        assert!(
            result
                .state
                .iter()
                .all(|item| !item.name.starts_with("copied."))
        );
    }

    #[test]
    fn browser_state_includes_visible_program_globals() {
        let source = format!(
            "{}\n",
            r#"
            #include <stdio.h>
            struct Counter { int current; int limit; };
            int total = 3;
            static int zeroed;
            struct Counter counter = {1, 10};

            int main(void) {
                int local = 4;
                total = 8;
            }
        "#
            .trim()
        );
        let result = run_source("program.c", source).unwrap();

        let state_value = |name: &str| {
            result
                .state
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.value.as_str())
        };
        assert_eq!(state_value("total"), Some("8"));
        assert_eq!(state_value("zeroed"), Some("0"));
        assert_eq!(state_value("counter"), Some(""));
        assert_eq!(state_value("counter.current"), Some("1"));
        assert_eq!(state_value("counter.limit"), Some("10"));
        assert_eq!(state_value("local"), Some("4"));
        assert!(
            result
                .state
                .iter()
                .all(|item| !matches!(item.name.as_str(), "stdin" | "stdout" | "stderr"))
        );
    }

    #[test]
    fn browser_state_uses_locals_instead_of_shadowed_globals() {
        let source = "int value = 1; int main(void) { int value = 2; }\n";
        let result = run_source("program.c", source).unwrap();
        let values = result
            .state
            .iter()
            .filter(|item| item.name == "value")
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 1);
        assert_eq!(values[0].value, "2");
    }

    #[test]
    fn browser_state_includes_cross_file_globals_visible_to_main() {
        let files = vec![
            (
                PathBuf::from("program.c"),
                "extern int answer; int main(void) { answer = 42; }\n".to_owned(),
            ),
            (
                PathBuf::from("answer.c"),
                "int answer = 7; static int hidden = 9;\n".to_owned(),
            ),
        ];
        let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();

        assert_eq!(
            result
                .state
                .iter()
                .find(|item| item.name == "answer")
                .map(|item| item.value.as_str()),
            Some("42")
        );
        assert!(result.state.iter().all(|item| item.name != "hidden"));
    }

    #[test]
    fn implicit_main_trace_excludes_generated_lines() {
        let original = "int a = 1;\na = 2;";
        let (files, source_display, implicit_main) = cboxes_prepare_virtual_sources(
            vec![(PathBuf::from("program.c"), original.to_owned())],
            true,
        )
        .unwrap();
        let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();
        let json = cboxes_success_json(&result, &source_display);

        assert!(implicit_main.applied);
        assert!(json.contains("\"startLine\":0"));
        assert!(json.contains("\"startLine\":1"));
        assert!(!json.contains("\"startLine\":2"));
        assert!(json.contains("\"mainClose\":null"));
    }

    #[test]
    fn branch_trace_reports_the_exact_skipped_source_range() {
        let original = "if (0) {\n  int first = 1;\n} else if (0) {\n  int second = 2;\n} else {\n  int taken = 3;\n}\n";
        let (files, source_display, implicit_main) = cboxes_prepare_virtual_sources(
            vec![(PathBuf::from("program.c"), original.to_owned())],
            true,
        )
        .unwrap();
        let result = run_virtual_sources_with_options(&files, &RunOptions::default()).unwrap();
        let json = cboxes_success_json(&result, &source_display);

        assert!(implicit_main.applied);
        assert!(json.contains("\"kind\":\"branch\""));
        assert!(json.contains(
            "\"skippedRange\":{\"file\":\"program.c\",\"startLine\":0,\"startColumn\":0,\"endLine\":2,\"endColumn\":1}"
        ));
        assert!(json.contains(
            "\"skippedRange\":{\"file\":\"program.c\",\"startLine\":2,\"startColumn\":2,\"endLine\":4,\"endColumn\":1}"
        ));
    }

    #[test]
    fn explicit_main_reports_its_closing_brace_as_the_program_end() {
        let source = "int main(void) {\n  int a = 1;\n}\n";
        let result = run_source("program.c", source).unwrap();
        let source_display =
            HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]);
        let json = cboxes_success_json(&result, &source_display);

        assert_eq!(result.trace.len(), 1);
        assert_eq!(result.trace[0].start_line, 1);
        assert_eq!(result.main_close.file, "program.c");
        assert_eq!(result.main_close.line, 2);
        assert!(json.contains("\"mainClose\":{\"file\":\"program.c\",\"line\":2}"));
    }

    #[test]
    fn execution_step_limit_returns_the_trace_before_an_infinite_loop() {
        let source = "int main(void) {\n  int before = 1;\n  while (1) {\n    before++;\n  }\n  int after = 2;\n}\n";
        let result = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                execution_step_limit: Some(40),
                execution_trace_following_limit: 2,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let source_display =
            HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]);
        let json = cboxes_success_json(&result, &source_display);

        let execution_limit = result.execution_limit.as_ref().unwrap();
        assert_eq!(execution_limit.start_line, 2);
        assert_eq!(execution_limit.trace_position, 1);
        assert_eq!(result.trace.len(), execution_limit.trace_position + 2);
        assert!(result.trace.iter().all(|event| event.start_line < 5));
        assert_eq!(
            result.state.first().map(|item| (&item.name, &item.value)),
            result
                .trace
                .get(execution_limit.trace_position - 1)
                .and_then(|event| event.state.first())
                .map(|item| (&item.name, &item.value)),
        );
        assert!(
            json.contains(
                "\"executionLimit\":{\"file\":\"program.c\",\"startLine\":2,\"endLine\":2,\"tracePosition\":1}"
            )
        );

        let expanded = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                execution_step_limit: Some(80),
                execution_trace_following_limit: 4,
                ..RunOptions::default()
            },
        )
        .unwrap();
        let expanded_limit = expanded.execution_limit.as_ref().unwrap();
        assert_eq!(
            expanded_limit.trace_position,
            execution_limit.trace_position
        );
        assert_eq!(expanded.trace.len(), expanded_limit.trace_position + 4);
    }

    #[test]
    fn execution_step_limit_rewinds_before_a_goto_cycle() {
        let source = "int main(void) {\n  int before = 1;\nrepeat:\n  before++;\n  goto repeat;\n  int after = 2;\n}\n";
        let result = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                execution_step_limit: Some(600),
                ..RunOptions::default()
            },
        )
        .unwrap();

        let execution_limit = result.execution_limit.as_ref().unwrap();
        assert_eq!(execution_limit.start_line, 3);
        assert_eq!(execution_limit.trace_position, 1);
        assert!(result.trace.len() > execution_limit.trace_position);
        assert_eq!(result.state[0].name, "before");
        assert_eq!(result.state[0].value, "1");
    }

    #[test]
    fn execution_step_limit_preserves_trace_when_no_location_repeats() {
        let mut source = "int main(void) {\n  int value = 0;\n".to_owned();
        for _ in 0..50 {
            source.push_str("  value++;\n");
        }
        source.push_str("}\n");
        let result = run_source_with_options(
            "program.c",
            source,
            &RunOptions {
                execution_step_limit: Some(20),
                ..RunOptions::default()
            },
        )
        .unwrap();

        let execution_limit = result.execution_limit.as_ref().unwrap();
        assert_eq!(result.state[0].name, "value");
        assert!(result.state[0].value.parse::<usize>().unwrap() > 0);
        assert!(execution_limit.start_line > result.trace.last().unwrap().end_line);
        assert_eq!(execution_limit.trace_position, result.trace.len());
    }

    #[test]
    fn blocked_stdin_location_is_exposed_to_the_browser() {
        let source =
            "#include <stdio.h>\nint main(void) {\n  int before = 1;\n  int ch = getchar();\n}\n";
        let result = run_source("program.c", source).unwrap();
        let source_display =
            HashMap::from([("program.c".to_owned(), cboxes_source_display(source, 0))]);
        let json = cboxes_success_json(&result, &source_display);

        assert!(result.blocked.is_some());
        assert!(json.contains(
            "\"blocked\":{\"file\":\"program.c\",\"startLine\":3,\"endLine\":3,\"function\":\"getchar\""
        ));
        assert!(json.contains("\"name\":\"before\""));
    }

    #[test]
    fn implicit_main_wraps_the_entire_entry_source() {
        let original = "int a = 1;\na += 2;";
        let (files, _, implicit_main) = cboxes_prepare_virtual_sources(
            vec![(PathBuf::from("program.c"), original.to_owned())],
            true,
        )
        .unwrap();

        assert!(implicit_main.applied);
        assert_eq!(
            files[0].1,
            "int main(void) {\nint a = 1;\na += 2;\nreturn 0;\n}\n"
        );
    }

    #[test]
    fn implicit_main_wraps_header_includes_too() {
        let original = "#include <stdio.h>\nprintf(\"hello\\n\");";
        let (files, _, implicit_main) = cboxes_prepare_virtual_sources(
            vec![(PathBuf::from("program.c"), original.to_owned())],
            true,
        )
        .unwrap();

        assert!(implicit_main.applied);
        assert!(implicit_main.notice.is_none());
        assert!(
            files[0]
                .1
                .starts_with("int main(void) {\n#include <stdio.h>")
        );
    }

    #[test]
    fn legacy_implicit_main_detection_ignores_comments_and_literals() {
        let source = r#"#include <stdio.h>
            // main() is introduced later in the course.
            printf("main() is where programs start\n");
        "#;
        assert!(!cboxes_has_explicit_main(source));
        let result = run_source("program.c", cboxes_wrap_implicit_main(source)).unwrap();
        assert_eq!(result.stdout, "main() is where programs start\n");

        assert!(!cboxes_has_explicit_main("/* int main(void) {} */\n"));
        assert!(!cboxes_has_explicit_main(
            "char text[] = \"main (void)\";\n"
        ));
        assert!(cboxes_has_explicit_main(
            "int ma\\\nin /* comment */ (void) { return 0; }\n"
        ));
    }

    #[test]
    fn implicit_main_stays_on_and_suggests_the_toggle_for_an_existing_main() {
        let files = vec![(
            PathBuf::from("program.c"),
            "int main(void) { return 0; }".to_owned(),
        )];
        let run = cboxes_run_virtual_sources(files, &RunOptions::default(), true).unwrap();

        assert!(run.result.is_err());
        assert!(run.implicit_main.applied);
        assert!(
            run.implicit_main
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("Try turning off Implicit main"))
        );
    }

    #[test]
    fn implicit_main_does_not_suggest_the_toggle_when_off_still_errors() {
        let files = vec![(
            PathBuf::from("program.c"),
            "int answer(void) { return 42; }".to_owned(),
        )];
        let run = cboxes_run_virtual_sources(files, &RunOptions::default(), true).unwrap();

        assert!(run.result.is_err());
        assert!(run.implicit_main.applied);
        assert!(run.implicit_main.notice.is_none());
    }

    #[test]
    fn implicit_main_stays_on_and_suggests_the_toggle_for_header_programs() {
        let files = vec![(
            PathBuf::from("program.c"),
            "#include <stdio.h>\nint main(void) { printf(\"hello\\n\"); }".to_owned(),
        )];
        let run = cboxes_run_virtual_sources(files, &RunOptions::default(), true).unwrap();

        assert!(run.result.is_err());
        assert!(run.implicit_main.applied);
        assert!(
            run.implicit_main
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("Try turning off Implicit main"))
        );
    }

    #[test]
    fn tutorial_implicit_main_keeps_includes_outside_the_wrapper() {
        let source = cboxes_wrap_implicit_main(
            "#include <stdio.h>\nprintf(\"tutorial wrapper still works\\n\");",
        );
        let result = run_source("program.c", source).unwrap();

        assert_eq!(result.stdout, "tutorial wrapper still works\n");
    }
}
