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
    if let Some(limit) = result.execution_limit {
        let reason = match limit.reason {
            "repeating-loop" => "the loop's exit state repeats",
            "unreachable-loop-exit" => "the counter cannot reach the loop's stopping condition",
            _ => "the execution step limit was reached",
        };
        return Err(format!(
            "execution stopped at {}:{} after {} steps: {reason}",
            limit.file,
            limit.start_line + 1,
            limit.executed_steps,
        ));
    }
    if let Some(blocked) = result.blocked {
        return Err(format!(
            "execution paused at {}:{}: {} is waiting for more input",
            blocked.file,
            blocked.start_line + 1,
            blocked.function,
        ));
    }
    Ok(NativeExecutionResult {
        stdout: result.stdout,
        stderr: result.stderr,
        exit_status: result.exit_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_native_execution_is_not_reported_as_success() {
        for (source, expected) in [
            ("int main(void) { while(1) {} }\n", "exit state repeats"),
            (
                "int main(void) { int i=0; while(i<10) i--; }\n",
                "cannot reach",
            ),
            (
                "#include <stdlib.h>\nint main(void) { while(rand()>=0) {} }\n",
                "step limit was reached",
            ),
            (
                "#include <stdio.h>\nint main(void) { return getchar(); }\n",
                "waiting for more input",
            ),
        ] {
            let error = run_native_source(
                "program.c",
                source,
                &NativeExecutionOptions {
                    execution_step_limit: Some(100),
                    ..NativeExecutionOptions::default()
                },
            )
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(error.contains("program.c:"), "{error}");
        }
    }

    #[test]
    fn completed_native_execution_preserves_its_output_and_status() {
        let result = run_native_source(
            "program.c",
            "#include <stdio.h>\nint main(void) { puts(\"done\"); return 3; }\n",
            &NativeExecutionOptions::default(),
        )
        .unwrap();
        assert_eq!(result.stdout, "done\n");
        assert_eq!(result.exit_status, 3);
    }
}
