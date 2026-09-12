mod ast;
mod browser;
mod browser_json;
mod diag;
mod fast_hash;
mod integer;
mod interpreter;
mod lexer;
mod linker;
mod native;
mod number;
mod parser;
mod preprocess;
mod source;
mod token;
mod types;

#[cfg(all(not(target_os = "wasi"), feature = "native-mimalloc"))]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ast::TranslationUnit;
use browser::CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT;
pub use browser::{cboxes_alloc, cboxes_execute, cboxes_free, cboxes_last_result_len};
use browser_json::cboxes_value_literal_text;
use diag::Diagnostic;
use interpreter::{Interpreter, ProgramExpressionEvalRequest, ProgramOutput};
use lexer::Lexer;
#[cfg(not(target_os = "wasi"))]
use linker::fallback_span;
use linker::{merge_translation_units, normalize_single_translation_unit};
pub use native::{
    NativeExecutionOptions, NativeExecutionResult, NativeStreamIo, run_native_source,
};
use parser::Parser;
use preprocess::Preprocessor;
use source::{FileId, SourceManager, Span};
use std::collections::HashSet;

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

const DEFAULT_ALLOCATION_LIMIT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
struct RunOptions {
    #[cfg(test)]
    pub include_dirs: Vec<PathBuf>,
    pub stdin: String,
    pub native_stream_io: Option<Arc<dyn NativeStreamIo>>,
    pub expression_eval: Option<RunExpressionEvalRequest>,
    pub ub_detection_mode: UbDetectionMode,
    pub allocation_limit_bytes: Option<usize>,
    pub optimizing_precomputations: bool,
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

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            #[cfg(test)]
            include_dirs: Vec::new(),
            stdin: String::new(),
            native_stream_io: None,
            expression_eval: None,
            ub_detection_mode: UbDetectionMode::Standard,
            allocation_limit_bytes: Some(DEFAULT_ALLOCATION_LIMIT_BYTES),
            optimizing_precomputations: false,
            capture_visualization: true,
            synthetic_address_base: 0x1000,
            execution_step_limit: None,
            execution_trace_following_limit: CBOXES_BROWSER_EXECUTION_TRACE_FOLLOWING_LIMIT,
        }
    }
}

#[cfg(test)]
fn run_file(path: impl AsRef<Path>) -> Result<ProgramOutput, Diagnostic> {
    run_file_with_options(path, &RunOptions::default())
}

#[cfg(test)]
fn run_file_with_options(
    path: impl AsRef<Path>,
    options: &RunOptions,
) -> Result<ProgramOutput, Diagnostic> {
    run_files_with_options([path.as_ref().to_path_buf()], options)
}

#[cfg(test)]
fn run_source(
    virtual_path: impl Into<PathBuf>,
    source: impl Into<String>,
) -> Result<ProgramOutput, Diagnostic> {
    run_source_with_options(virtual_path, source, &RunOptions::default())
}

fn run_source_with_options(
    virtual_path: impl Into<PathBuf>,
    source: impl Into<String>,
    options: &RunOptions,
) -> Result<ProgramOutput, Diagnostic> {
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
) -> Result<ProgramOutput, Diagnostic> {
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
fn run_files<I, P>(paths: I) -> Result<ProgramOutput, Diagnostic>
where
    I: IntoIterator<Item = P>,
    P: Into<PathBuf>,
{
    run_files_with_options(paths, &RunOptions::default())
}

#[cfg(test)]
fn run_files_with_options<I, P>(paths: I, options: &RunOptions) -> Result<ProgramOutput, Diagnostic>
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

fn run_with_sources(
    sources: &mut SourceManager,
    root_file: source::FileId,
    cwd: &Path,
    options: &RunOptions,
) -> Result<ProgramOutput, Diagnostic> {
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
) -> Result<ProgramOutput, Diagnostic> {
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

    execution.map_err(|diag| diag.with_sources(sources))
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
