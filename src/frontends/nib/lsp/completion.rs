//! `textDocument/completion`: the locals in scope, keywords and the module's own names; after
//! `alias.` the public names of the module imported as `alias`; after any
//! other value and `.`, the fields and methods of its type.

use std::path::Path;

use super::documents::Documents;
use super::index::{self, Kind};
use super::protocol::{CompletionItem, Position};
use super::documents::Document;
use super::resolve::{declaring, visible};
use super::text;
use crate::frontends::nib::lexer::KEYWORDS;
use crate::frontends::nib::semantic::Known;
use crate::frontends::nib::{check, lex, module_path, parse};

const KEYWORD: u8 = 14;
const MODULE: u8 = 9;
const VARIABLE: u8 = 6;
/// A name put where one is being typed, so that the text parses and checks as far as it.
const TYPING: &str = "nib_lsp_typing";

pub fn completion(documents: &Documents, path: &Path, position: Position) -> Vec<CompletionItem> {
    let (Some(main), Some(document)) = (documents.module(path), documents.get(path)) else {
        return Vec::new();
    };
    let before = text::before(&document.text, position);
    let typed = before.rsplit(|one: char| !(one.is_ascii_alphanumeric() || one == '_' || one == '.')).next().unwrap_or("");
    let item = |label: &str, kind| CompletionItem { label: label.to_owned(), kind };
    if let Some((qualifier, _)) = typed.rsplit_once('.') {
        let Some(import) = main.imports.iter().find(|one| one.name == qualifier) else {
            return members(documents, path, position);
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
    let (typing, line, column) = typing(document, position);
    let module = lex(&typing).ok().and_then(|tokens| parse(tokens).ok());
    let locals = module.map(|module| visible(&module, line, column)).unwrap_or_default();
    let locals = locals.iter().map(|name| item(name, VARIABLE));
    let keywords = KEYWORDS.iter().map(|(spelling, _)| item(spelling, KEYWORD));
    let imports = main.imports.iter().map(|one| item(&one.name, MODULE));
    let own = index::declarations(&main).into_iter().filter(|one| !one.name.contains('.')).map(|one| item(&one.name, one.kind.completion()));
    locals.chain(keywords).chain(imports).chain(own).collect()
}

/// The fields and methods of the type the checker finds the value before the
/// `.` at `position` has.
fn members(documents: &Documents, path: &Path, position: Position) -> Vec<CompletionItem> {
    let Some(document) = documents.get(path) else {
        return Vec::new();
    };
    let (typing, line, column) = typing(document, position);
    let checked = check(&typing, &mut |name| documents.text(&module_path(path, name)));
    let owner = checked.facts.iter().find_map(|fact| match &fact.known {
        Known::Member(owner) if fact.span.module == 0 && text::contains(fact.span, line, column) => Some(owner),
        _ => None,
    });
    let (Some(owner), Some(loaded)) = (owner, &checked.loaded) else {
        return Vec::new();
    };
    let (module, owner) = declaring(loaded, owner);
    let declarations = index::declarations(&loaded.modules[&module]);
    let fields = declarations.iter().filter(|one| one.name == owner).flat_map(|one| &one.members).filter(|one| one.kind == Kind::Field);
    let methods = declarations.iter().filter(|one| one.kind == Kind::Method && one.name.strip_prefix(owner).is_some_and(|rest| rest.starts_with('.')) && (module.is_empty() || one.public));
    fields.chain(methods).map(|one| CompletionItem { label: one.short().to_owned(), kind: one.kind.completion() }).collect()
}

/// The document's text with a name put at `position`, and where it stands.
fn typing(document: &Document, position: Position) -> (String, usize, usize) {
    let (line, column) = text::at(&document.text, position);
    let mut typing = document.text.clone();
    typing.insert_str(text::offset(&typing, line, column), TYPING);
    (typing, line, column)
}
