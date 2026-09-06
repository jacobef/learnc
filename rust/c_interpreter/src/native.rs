use super::{DEFAULT_ALLOCATION_LIMIT_BYTES, RunOptions, run_source_with_options};
use std::path::PathBuf;
use std::sync::Arc;

/// Host callbacks for native standard streams.
pub trait NativeStreamIo: std::fmt::Debug + Send + Sync + std::panic::RefUnwindSafe {
    fn read_stdin(&self, buffer: &mut [u8]) -> std::io::Result<usize>;
    fn write_stdout(&self, bytes: &[u8]) -> std::io::Result<()>;
    fn write_stderr(&self, bytes: &[u8]) -> std::io::Result<()>;
}

/// Settings for native, non-visual execution.
#[derive(Clone, Debug)]
pub struct NativeExecutionOptions {
    pub stdin: String,
    pub stream_io: Option<Arc<dyn NativeStreamIo>>,
    pub allocation_limit_bytes: Option<usize>,
    pub execution_step_limit: Option<usize>,
    pub optimizing_precomputations: bool,
}

impl Default for NativeExecutionOptions {
    fn default() -> Self {
        Self {
            stdin: String::new(),
            stream_io: None,
            allocation_limit_bytes: Some(DEFAULT_ALLOCATION_LIMIT_BYTES),
            execution_step_limit: None,
            optimizing_precomputations: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NativeExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_status: i32,
}

/// Run one C translation unit through the native interpreter without browser visualization.
pub fn run_native_source(
    virtual_path: impl Into<PathBuf>,
    source: impl Into<String>,
    options: &NativeExecutionOptions,
) -> Result<NativeExecutionResult, String> {
    let result = run_source_with_options(
        virtual_path,
        source,
        &RunOptions {
            stdin: options.stdin.clone(),
            native_stream_io: options.stream_io.clone(),
            allocation_limit_bytes: options.allocation_limit_bytes,
            execution_step_limit: options.execution_step_limit,
            optimizing_precomputations: options.optimizing_precomputations,
            capture_visualization: false,
            ..RunOptions::default()
        },
    )
    .map_err(|diagnostic| diagnostic.render())?;
    Ok(NativeExecutionResult {
        stdout: result.stdout,
        stderr: result.stderr,
        exit_status: result.exit_status,
    })
}
