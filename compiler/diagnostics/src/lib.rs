//! Source spans and compiler diagnostics for Tarn.
//!
//! This crate has no dependencies and sits at the bottom of the compiler.
//! Diagnostics render as human-readable text or as one JSON object per line
//! (see `docs/errors.md`).

mod json;
mod render;
mod source;

pub use source::{FileId, LineCol, SourceFile, SourceMap, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub primary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable code such as `E0002`.
    pub code: &'static str,
    /// Machine name such as `unterminated_string`.
    pub kind: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, kind: &'static str, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code,
            kind,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn primary(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label { span, message: message.into(), primary: true });
        self
    }

    pub fn secondary(mut self, span: Span, message: impl Into<String>) -> Diagnostic {
        self.labels.push(Label { span, message: message.into(), primary: false });
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Diagnostic {
        self.notes.push(note.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Diagnostic {
        self.help = Some(help.into());
        self
    }

    pub fn primary_span(&self) -> Option<Span> {
        self.labels.iter().find(|l| l.primary).map(|l| l.span)
    }

    /// Human-readable rendering (no colors), as described in `docs/errors.md`.
    pub fn render(&self, sources: &SourceMap) -> String {
        render::render(self, sources)
    }

    /// One-line JSON object.
    pub fn to_json(&self, sources: &SourceMap) -> String {
        json::to_json(self, sources)
    }
}
