use std::fmt::Write as _;
#[cfg(test)]
use std::io;
use std::path::PathBuf;

use crate::interpreter::ProgramStateBox;
use crate::source::{Snippet, SourceManager, Span};

#[derive(Debug, Clone)]
pub struct DiagnosticRuntimeContext {
    pub executed_steps: usize,
    pub line_execution_count: Option<usize>,
    pub state: Vec<ProgramStateBox>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    UndefinedBehavior,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::UndefinedBehavior => "undefined behavior",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    severity: Severity,
    message: String,
    span: Option<Span>,
    notes: Vec<String>,
    standard_reference: Option<&'static str>,
    rendered_with_sources: Option<String>,
    display_range: Option<DiagnosticDisplayRange>,
    related_spans: Vec<DiagnosticRelatedSpan>,
    display_annotations: Vec<DiagnosticDisplayAnnotation>,
    control: Option<DiagnosticControl>,
    runtime_context: Option<DiagnosticRuntimeContext>,
}

#[derive(Debug, Clone)]
struct DiagnosticRelatedSpan {
    id: String,
    label: String,
    span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticDisplayAnnotation {
    pub id: String,
    pub range: DiagnosticDisplayRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticDisplayRange {
    pub path: PathBuf,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Copy)]
enum DiagnosticControl {
    Blocked(&'static str),
    ExecutionStepLimit,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span: Some(span),
            notes: Vec::new(),
            standard_reference: None,
            rendered_with_sources: None,
            display_range: None,
            related_spans: Vec::new(),
            display_annotations: Vec::new(),
            control: None,
            runtime_context: None,
        }
    }

    pub fn ub(
        message: impl Into<String>,
        span: Span,
        standard_reference: Option<&'static str>,
    ) -> Self {
        Self {
            severity: Severity::UndefinedBehavior,
            message: message.into(),
            span: Some(span),
            notes: Vec::new(),
            standard_reference,
            rendered_with_sources: None,
            display_range: None,
            related_spans: Vec::new(),
            display_annotations: Vec::new(),
            control: None,
            runtime_context: None,
        }
    }

    pub fn blocked(function_name: &'static str, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: format!("{function_name} is waiting for input"),
            span: Some(span),
            notes: Vec::new(),
            standard_reference: None,
            rendered_with_sources: None,
            display_range: None,
            related_spans: Vec::new(),
            display_annotations: Vec::new(),
            control: Some(DiagnosticControl::Blocked(function_name)),
            runtime_context: None,
        }
    }

    pub fn blocked_info(&self) -> Option<(&'static str, Span)> {
        match self.control? {
            DiagnosticControl::Blocked(function_name) => Some((function_name, self.span?)),
            DiagnosticControl::ExecutionStepLimit => None,
        }
    }

    pub fn execution_step_limit(span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: "execution step limit reached".to_owned(),
            span: Some(span),
            notes: Vec::new(),
            standard_reference: None,
            rendered_with_sources: None,
            display_range: None,
            related_spans: Vec::new(),
            display_annotations: Vec::new(),
            control: Some(DiagnosticControl::ExecutionStepLimit),
            runtime_context: None,
        }
    }

    pub fn execution_step_limit_span(&self) -> Option<Span> {
        matches!(self.control, Some(DiagnosticControl::ExecutionStepLimit)).then_some(self.span?)
    }

    #[cfg(test)]
    pub fn io(path: PathBuf, err: io::Error) -> Self {
        Self {
            severity: Severity::Error,
            message: format!("{}: {}", path.display(), err),
            span: None,
            notes: Vec::new(),
            standard_reference: None,
            rendered_with_sources: None,
            display_range: None,
            related_spans: Vec::new(),
            display_annotations: Vec::new(),
            control: None,
            runtime_context: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_runtime_context(mut self, context: DiagnosticRuntimeContext) -> Self {
        self.runtime_context = Some(context);
        self
    }

    pub fn runtime_context(&self) -> Option<&DiagnosticRuntimeContext> {
        self.runtime_context.as_ref()
    }

    pub fn span(&self) -> Option<Span> {
        self.span
    }

    pub fn with_message_prefix(mut self, prefix: impl AsRef<str>) -> Self {
        self.message = format!("{}: {}", prefix.as_ref(), self.message);
        self
    }

    pub fn with_related_span(
        mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        span: Span,
    ) -> Self {
        let id = id.into();
        if !self.related_spans.iter().any(|related| related.id == id) {
            self.related_spans.push(DiagnosticRelatedSpan {
                id,
                label: label.into(),
                span,
            });
        }
        self
    }

    pub(crate) fn replace_placeholder_span(mut self, fallback: Span) -> Self {
        if self.span == Some(Span::new(fallback.file, 0, 0)) {
            self.span = Some(fallback);
        }
        self
    }

    pub fn render(&self) -> String {
        if let Some(rendered) = &self.rendered_with_sources {
            return rendered.clone();
        }
        let mut out = String::new();
        let _ = writeln!(out, "{}: {}", self.severity.label(), self.message);
        for note in &self.notes {
            let _ = writeln!(out, "note: {}", note);
        }
        if let Some(standard_reference) = self.standard_reference {
            let _ = writeln!(out, "standard: {}", standard_reference);
        }
        out
    }

    pub fn render_with_sources(&self, sources: &SourceManager) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{}: {}", self.severity.label(), self.message);
        if let Some(span) = self.span {
            let snippet = sources.snippet(span);
            render_snippet(&mut out, snippet);
        }
        for related in &self.related_spans {
            let _ = writeln!(out, "note: {}", related.label);
            render_snippet(&mut out, sources.snippet(related.span));
        }
        for note in &self.notes {
            let _ = writeln!(out, "note: {}", note);
        }
        if let Some(standard_reference) = self.standard_reference {
            let _ = writeln!(out, "standard: {}", standard_reference);
        }
        out
    }

    pub fn with_sources(mut self, sources: &SourceManager) -> Self {
        self.display_range = self.span.map(|span| {
            let (path, start_line, start_column, end_line, end_column) =
                sources.span_display_range(span);
            DiagnosticDisplayRange {
                path,
                start_line,
                start_column,
                end_line,
                end_column,
            }
        });
        self.display_annotations = self
            .related_spans
            .iter()
            .map(|related| {
                let (path, start_line, start_column, end_line, end_column) =
                    sources.span_display_range(related.span);
                DiagnosticDisplayAnnotation {
                    id: related.id.clone(),
                    range: DiagnosticDisplayRange {
                        path,
                        start_line,
                        start_column,
                        end_line,
                        end_column,
                    },
                }
            })
            .collect();
        self.rendered_with_sources = Some(self.render_with_sources(sources));
        self
    }

    pub fn display_range(&self) -> Option<&DiagnosticDisplayRange> {
        self.display_range.as_ref()
    }

    pub fn display_annotations(&self) -> &[DiagnosticDisplayAnnotation] {
        &self.display_annotations
    }
}

fn render_snippet(out: &mut String, snippet: Snippet) {
    let _ = writeln!(
        out,
        " --> {}:{}:{}",
        snippet.path.display(),
        snippet.line_number,
        snippet.column
    );
    let _ = writeln!(out, "{:>4} | {}", snippet.line_number, snippet.line_text);
    let _ = writeln!(
        out,
        "     | {}{}",
        " ".repeat(snippet.marker_start),
        "^".repeat(snippet.marker_len)
    );
}
