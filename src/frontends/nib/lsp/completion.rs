//! `textDocument/completion`: keywords and the module's own names, or after
//! `alias.` the public names of the module imported as `alias`.

use std::path::Path;

use super::documents::Documents;
use super::index;
use super::protocol::{CompletionItem, Position};
use super::text;
use crate::frontends::nib::lexer::KEYWORDS;

const KEYWORD: u8 = 14;
const MODULE: u8 = 9;

pub fn completion(documents: &Documents, path: &Path, position: Position) -> Vec<CompletionItem> {
    let (Some(main), Some(document)) = (documents.module(path), documents.get(path)) else {
        return Vec::new();
    };
    let before = text::before(&document.text, position);
    let typed = before.rsplit(|one: char| !(one.is_ascii_alphanumeric() || one == '_' || one == '.')).next().unwrap_or("");
    let item = |label: &str, kind| CompletionItem { label: label.to_owned(), kind };
    if let Some((qualifier, _)) = typed.rsplit_once('.') {
        let Some(import) = main.imports.iter().find(|one| one.name == qualifier) else {
            return Vec::new();
        };
        let Some(module) = document.loaded.as_ref().and_then(|loaded| loaded.modules.get(&import.module)) else {
            return Vec::new();
        };
        return index::declarations(module)
            .iter()
            .filter(|one| one.public && !one.name.contains('.'))
            .map(|one| item(&one.name, one.kind.completion()))
            .collect();
    }
    let keywords = KEYWORDS.iter().map(|(spelling, _)| item(spelling, KEYWORD));
    let imports = main.imports.iter().map(|one| item(&one.name, MODULE));
    let own = index::declarations(&main).into_iter().filter(|one| !one.name.contains('.')).map(|one| item(&one.name, one.kind.completion()));
    keywords.chain(imports).chain(own).collect()
}
