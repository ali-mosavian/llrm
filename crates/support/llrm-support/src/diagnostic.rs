//! Diagnostic values shared by llrm subsystems.
//!
//! Diagnostics carry information only. Rendering and output policy belong to
//! the driver, so parsers and compiler passes can report failures without
//! acquiring I/O side effects.

/// The importance of a diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

/// An identifier for the source associated with a span.
///
/// This deliberately is not a filesystem path: frontends may use a module
/// name, an in-memory buffer name, or no identifier at all.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SourceId(pub String);

impl SourceId {
    pub fn new(identifier: impl Into<String>) -> Self {
        Self(identifier.into())
    }
}

/// A half-open byte range in an optional source.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Span {
    pub source: Option<SourceId>,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(source: Option<SourceId>, start: usize, end: usize) -> Self {
        Self { source, start, end }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A message attached to a source range.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

/// A diagnostic returned by a compiler subsystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }
}
