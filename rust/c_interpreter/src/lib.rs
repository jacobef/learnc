mod ast;
mod diag;
mod interpreter;
mod lexer;
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
    FunctionDef, Initializer, InitializerItem, Parameter, Statement, StorageClass, SwitchLabel,
    TranslationUnit,
};
use diag::Diagnostic;
use interpreter::{
    Interpreter, ProgramBlocked, ProgramExpressionEvalRequest, ProgramExpressionResult,
    ProgramOutput, ProgramSourceLocation, ProgramStateBox, ProgramTraceEvent, ProgramValueLiteral,
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
    pub expression: Option<ProgramExpressionResult>,
}

#[derive(Clone, Debug)]
struct RunExpressionEvalRequest {
    pub expression: String,
    pub event_index: usize,
}

#[derive(Clone, Debug)]
struct RunOptions {
    #[cfg(test)]
    pub include_dirs: Vec<PathBuf>,
    pub stdin: String,
    pub expression_eval: Option<RunExpressionEvalRequest>,
    pub synthetic_address_base: u64,
}

type VirtualSource = (PathBuf, String);
type SourceDisplayMap = HashMap<String, (usize, usize)>;

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
    source.lines().count().max(1)
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            #[cfg(test)]
            include_dirs: Vec::new(),
            stdin: String::new(),
            expression_eval: None,
            synthetic_address_base: 0x1000,
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
    let translation_unit = merge_translation_units(units)?;
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
    let translation_unit = merge_translation_units(units)?;
    let ProgramOutput {
        stdout,
        stderr,
        exit_status,
        state,
        trace,
        main_close,
        blocked,
        expression: _,
    } = Interpreter::new(&sources, translation_unit, options)
        .run()
        .map_err(|diag| diag.with_sources(&sources))?;
    Ok(RunResult {
        stdout,
        stderr,
        exit_status,
        state,
        trace,
        main_close,
        blocked,
        expression: None,
    })
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
        .map(|(path, source)| {
            (
                path.display().to_string(),
                (0, cboxes_source_line_count(source)),
            )
        })
        .collect::<HashMap<_, _>>();

    for index in &c_files {
        let source = &mut files[*index].1;
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
        display.0 = 1;
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
                synthetic_address_base: options.synthetic_address_base,
            },
        )
    }
}

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
                synthetic_address_base: synthetic_address_base.into(),
            },
        )
    });
    let source_display = HashMap::from([(
        "program.c".to_owned(),
        (line_offset, cboxes_source_line_count(input)),
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
        synthetic_address_base: synthetic_address_base.into(),
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
                synthetic_address_base: synthetic_address_base.into(),
            },
        )
    });
    let source_display = HashMap::from([(
        "program.c".to_owned(),
        (line_offset, cboxes_source_line_count(input)),
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
        synthetic_address_base: synthetic_address_base.into(),
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
    let bytes = source.as_bytes();
    let needle = b"main";
    let mut i = 0usize;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let before = if i == 0 { b' ' } else { bytes[i - 1] };
            let after = bytes.get(i + needle.len()).copied().unwrap_or(b' ');
            let before_ident = before == b'_' || before.is_ascii_alphanumeric();
            let after_ident = after == b'_' || after.is_ascii_alphanumeric();
            if !before_ident && !after_ident {
                let tail = &source[i + needle.len()..];
                if tail.trim_start().starts_with('(') {
                    return true;
                }
            }
        }
        i += 1;
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
        "{{\"ok\":true,\"stdout\":{},\"stderr\":{},\"exitStatus\":{},\"state\":{},\"trace\":{},\"mainClose\":{},\"blocked\":{}}}",
        cboxes_json_string(&result.stdout),
        cboxes_json_string(&result.stderr),
        result.exit_status,
        cboxes_state_json(&result.state),
        cboxes_trace_json(&result.trace, source_display),
        cboxes_source_location_json(&result.main_close, source_display),
        cboxes_blocked_json(result.blocked.as_ref(), source_display)
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
        "{{\"kind\":{},\"type\":{},\"value\":{},\"address\":{},\"name\":{},\"valueLiteral\":{}}}",
        cboxes_json_string(&result.kind),
        cboxes_json_string(&result.ty),
        cboxes_json_string(&result.value),
        address,
        cboxes_json_string(&result.name),
        cboxes_value_literal_json(result.value_literal.as_ref()),
    )
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
    let location = cboxes_rendered_location(&rendered).map(|(file, line, col)| {
        let line_offset = source_display
            .get(&file)
            .map(|display| display.0)
            .unwrap_or(0);
        (file, line.saturating_sub(line_offset), col)
    });
    cboxes_error_json(kind, &rendered, location)
}

fn cboxes_error_json(
    kind: &str,
    message: &str,
    location: Option<(String, usize, usize)>,
) -> String {
    let (file, line, col) = location
        .map(|(file, line, col)| (cboxes_json_string(&file), line.to_string(), col.to_string()))
        .unwrap_or_else(|| ("null".to_owned(), "null".to_owned(), "null".to_owned()));
    format!(
        "{{\"ok\":false,\"kind\":{},\"message\":{},\"file\":{},\"line\":{},\"column\":{}}}",
        cboxes_json_string(kind),
        cboxes_json_string(message),
        file,
        line,
        col
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
        out.push('}');
    }
    out.push(']');
    out
}

fn cboxes_trace_json(trace: &[ProgramTraceEvent], source_display: &SourceDisplayMap) -> String {
    let mut out = String::from("[");
    let mut wrote_event = false;
    for event in trace {
        let (line_offset, line_count) = source_display
            .get(&event.file)
            .copied()
            .unwrap_or((0, usize::MAX));
        let start_line = event.start_line.saturating_sub(line_offset);
        let end_line = event.end_line.saturating_sub(line_offset);
        if start_line >= line_count {
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
        out.push_str(&end_line.min(line_count.saturating_sub(1)).to_string());
        out.push_str(",\"state\":");
        out.push_str(&cboxes_state_json(&event.state));
        out.push('}');
    }
    out.push(']');
    out
}

fn cboxes_source_location_json(
    location: &ProgramSourceLocation,
    source_display: &SourceDisplayMap,
) -> String {
    let (line_offset, line_count) = source_display
        .get(&location.file)
        .copied()
        .unwrap_or((0, usize::MAX));
    let line = location.line.saturating_sub(line_offset);
    if line >= line_count {
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
    let (line_offset, line_count) = source_display
        .get(&blocked.file)
        .copied()
        .unwrap_or((0, usize::MAX));
    let start_line = blocked.start_line.saturating_sub(line_offset);
    if start_line >= line_count {
        return "null".to_owned();
    }
    let end_line = blocked
        .end_line
        .saturating_sub(line_offset)
        .min(line_count.saturating_sub(1));
    format!(
        "{{\"file\":{},\"startLine\":{},\"endLine\":{},\"function\":{},\"state\":{}}}",
        cboxes_json_string(&blocked.file),
        start_line,
        end_line,
        cboxes_json_string(&blocked.function),
        cboxes_state_json(&blocked.state)
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
    )?)?;
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
    let mut interpreter = Interpreter::new(sources, translation_unit, options);
    if let Some(request) = expression_eval {
        interpreter.set_cboxes_expression_eval(request);
    }
    let ProgramOutput {
        stdout,
        stderr,
        exit_status,
        state,
        trace,
        main_close,
        blocked,
        expression,
    } = interpreter
        .run()
        .map_err(|diag| diag.with_sources(sources))?;
    Ok(RunResult {
        stdout,
        stderr,
        exit_status,
        state,
        trace,
        main_close,
        blocked,
        expression,
    })
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
    let normalized = normalize_translation_unit(
        std::mem::take(&mut unit.externals),
        &unit.records,
        &unit.enums,
    )?;
    unit.function_declarations = normalized.function_declarations;
    unit.functions = normalized.function_definitions;
    unit.globals = normalized.global_declarations;
    unit.global_definitions = normalized.global_definitions;
    Ok(unit)
}

fn merge_translation_units(units: Vec<TranslationUnit>) -> Result<TranslationUnit, Diagnostic> {
    let mut merged = TranslationUnit {
        externals: Vec::new(),
        functions: Vec::new(),
        function_declarations: Vec::new(),
        globals: Vec::new(),
        global_definitions: Vec::new(),
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
        let normalized =
            normalize_translation_unit(remapped_externals, &merged.records, &merged.enums)?;
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
enum Linkage {
    Internal,
    External,
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
                let linkage = resolve_linkage(
                    &function.name,
                    function.storage_class,
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
                let linkage = resolve_linkage(
                    &function_decl.name,
                    function_decl.storage_class,
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
                let linkage = resolve_linkage(
                    &decl.name,
                    decl.storage_class,
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
    };

    let internal_linkage_names = collect_internal_linkage_names(&function_entries, &object_entries);
    for entry in function_entries.into_values() {
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
        if let Some(definition) = entry.definition {
            if is_inline_definition {
                validate_inline_definition_constraints(
                    &definition,
                    &internal_linkage_names,
                    records,
                )?;
            } else {
                normalized.function_definitions.push(definition);
            }
        }
    }
    for mut entry in object_entries.into_values() {
        normalized
            .global_declarations
            .push(entry.declaration.clone());
        if let Some(definition) = entry.real_definition.take() {
            normalized.global_definitions.push(definition);
        } else if entry.has_tentative_definition {
            normalized
                .global_definitions
                .push(finalize_tentative_definition(entry.declaration));
        }
    }

    Ok(normalized)
}

fn resolve_linkage(
    name: &str,
    storage_class: Option<StorageClass>,
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
        if storage_class == Some(StorageClass::Static) && prior.linkage == Linkage::External {
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
    let linkage = match storage_class {
        Some(StorageClass::Static) => Linkage::Internal,
        _ => prior
            .map(|prior| prior.linkage)
            .unwrap_or(Linkage::External),
    };
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
        | Expr::StringLiteral(_, _)
        | Expr::WideStringLiteral(_, _) => Ok(()),
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
            format!("conflicting declarations of function {}", new_decl.name),
            new_decl.span,
        )
    })?;
    *existing = function_decl_from_type(
        new_decl.name.clone(),
        composite,
        combine_function_storage(existing.storage_class, new_decl.storage_class),
        existing.is_inline || new_decl.is_inline,
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
                format!("conflicting declarations of object {}", new_decl.name),
                new_decl.span,
            )
        })?;
    existing.ty = composite;
    existing.storage_class = combine_object_storage(existing.storage_class, new_decl.storage_class);
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
        bit_offset: member.bit_offset,
        bit_storage_size: member.bit_storage_size,
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
        params: function
            .params
            .into_iter()
            .map(|param| remap_parameter(param, record_map, enum_map))
            .collect(),
        is_variadic: function.is_variadic,
        storage_class: function.storage_class,
        is_inline: function.is_inline,
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
        is_inline: function.is_inline,
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
                        is_inline: decl.is_inline,
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
            span,
        } => Statement::If {
            condition: remap_expr(condition, record_map, enum_map),
            then_branch: Box::new(remap_statement(*then_branch, record_map, enum_map)),
            else_branch: else_branch
                .map(|stmt| Box::new(remap_statement(*stmt, record_map, enum_map))),
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
        init: decl
            .init
            .map(|init| remap_initializer(init, record_map, enum_map)),
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
        Expr::Number(text, span) => Expr::Number(text, span),
        Expr::CharLiteral(value, span) => Expr::CharLiteral(value, span),
        Expr::WideCharLiteral(value, span) => Expr::WideCharLiteral(value, span),
        Expr::StringLiteral(text, span) => Expr::StringLiteral(text, span),
        Expr::WideStringLiteral(text, span) => Expr::WideStringLiteral(text, span),
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
        Expr::Call { callee, args, span } => Expr::Call {
            callee: Box::new(remap_expr(*callee, record_map, enum_map)),
            args: args
                .into_iter()
                .map(|arg| remap_expr(arg, record_map, enum_map))
                .collect(),
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
        is_inline: function.is_inline,
        span: function.span,
    }
}

fn function_decl_type(decl: &FunctionDecl) -> CType {
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

fn function_decl_from_type(
    name: String,
    ty: CType,
    storage_class: Option<StorageClass>,
    is_inline: bool,
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
                storage_class: None,
                span,
            })
            .collect(),
        is_variadic: *is_variadic,
        storage_class,
        is_inline,
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

fn composite_type(
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
        ) if lhs_variadic == rhs_variadic && lhs_params.len() == rhs_params.len() => {
            let return_type =
                composite_type_inner(lhs_ret, rhs_ret, records, enums, seen_records, seen_enums)?;
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
    fn explicit_main_reports_its_closing_brace_as_the_program_end() {
        let source = "int main(void) {\n  int a = 1;\n}\n";
        let result = run_source("program.c", source).unwrap();
        let source_display = HashMap::from([(
            "program.c".to_owned(),
            (0, cboxes_source_line_count(source)),
        )]);
        let json = cboxes_success_json(&result, &source_display);

        assert_eq!(result.trace.len(), 1);
        assert_eq!(result.trace[0].start_line, 1);
        assert_eq!(result.main_close.file, "program.c");
        assert_eq!(result.main_close.line, 2);
        assert!(json.contains("\"mainClose\":{\"file\":\"program.c\",\"line\":2}"));
    }

    #[test]
    fn blocked_stdin_location_is_exposed_to_the_browser() {
        let source =
            "#include <stdio.h>\nint main(void) {\n  int before = 1;\n  int ch = getchar();\n}\n";
        let result = run_source("program.c", source).unwrap();
        let source_display = HashMap::from([(
            "program.c".to_owned(),
            (0, cboxes_source_line_count(source)),
        )]);
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
