//! `textDocument/documentSymbol`: the declarations of the document's module.

use std::path::Path;

use super::documents::Documents;
use super::index::{self, Declaration};
use super::protocol::{DocumentSymbol, Range};
use super::text;

pub fn symbols(documents: &Documents, path: &Path) -> Vec<DocumentSymbol> {
    let (Some(module), Some(document)) = (documents.module(path), documents.get(path)) else {
        return Vec::new();
    };
    index::declarations(&module).iter().map(|one| symbol(one, &one.name, &document.text)).collect()
}

fn symbol(declaration: &Declaration, name: &str, text: &str) -> DocumentSymbol {
    let last = declaration.members.last().map_or(declaration.span.line, |one| one.span.line);
    let range = Range {
        start: text::line_range(text, declaration.span.line).start,
        end: text::line_range(text, last).end,
    };
    DocumentSymbol {
        name: name.to_owned(),
        kind: declaration.kind.symbol(),
        range,
        selection_range: text::range(text, text::named(text, declaration.span, declaration.short())),
        children: declaration.members.iter().map(|one| symbol(one, one.short(), text)).collect(),
    }
}
