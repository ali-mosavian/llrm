//! The LSP types the server reads and writes.

use serde::{Deserialize, Serialize};

/// A zero-based line and UTF-16 offset in it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Debug, Serialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: u8,
    pub source: &'static str,
    pub message: String,
}

pub const ERROR: u8 = 1;

#[derive(Deserialize)]
pub struct TextDocument {
    pub uri: String,
}

#[derive(Deserialize)]
pub struct TextItem {
    pub uri: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub text_document: TextItem,
}

#[derive(Deserialize)]
pub struct Change {
    pub text: String,
}

/// `didChange` with full sync: the last change is the whole text.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    pub text_document: TextDocument,
    pub content_changes: Vec<Change>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub text_document: TextDocument,
    pub text: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub text_document: TextDocument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct At {
    pub text_document: TextDocument,
    pub position: Position,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: u8,
    pub range: Range,
    pub selection_range: Range,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DocumentSymbol>,
}

#[derive(Serialize)]
pub struct Hover {
    pub contents: Markup,
}

#[derive(Serialize)]
pub struct Markup {
    pub kind: &'static str,
    pub value: String,
}

#[derive(Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: u8,
}
