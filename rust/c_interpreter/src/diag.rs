use std::fmt::Write as _;
#[cfg(test)]
use std::io;
#[cfg(test)]
use std::path::PathBuf;

use crate::source::{Snippet, SourceManager, Span};

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
    control: Option<DiagnosticControl>,
}

#[derive(Debug, Clone, Copy)]
enum DiagnosticControl {
    Blocked(&'static str),
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
            control: None,
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
            control: None,
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
            control: Some(DiagnosticControl::Blocked(function_name)),
        }
    }

    pub fn blocked_info(&self) -> Option<(&'static str, Span)> {
        let DiagnosticControl::Blocked(function_name) = self.control?;
        Some((function_name, self.span?))
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
            control: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
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
        for note in &self.notes {
            let _ = writeln!(out, "note: {}", note);
        }
        if let Some(standard_reference) = self.standard_reference {
            let _ = writeln!(out, "standard: {}", standard_reference);
        }
        out
    }

    pub fn with_sources(mut self, sources: &SourceManager) -> Self {
        self.rendered_with_sources = Some(self.render_with_sources(sources));
        self
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
