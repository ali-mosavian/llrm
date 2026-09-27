//! Diagnostics: the frontend's first error, on the file it is in and, when
//! that is a module imported, also on the import that reaches it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::documents::{Documents, module_file};
use super::protocol::{Diagnostic, ERROR, Range};
use super::text;
use crate::{check, lex, module_path, parser};

/// Checks the open document at `path`, keeping what the check learned: each
/// file the error is reported on, and the diagnostic there.
pub fn checked(documents: &mut Documents, path: &Path) -> Vec<(PathBuf, Diagnostic)> {
    let Ok(source) = documents.text(path) else {
        return Vec::new();
    };
    let checked = check(&source, &mut |name| documents.text(&module_path(path, name)));
    let mut found = Vec::new();
    if let Some((module, error)) = checked.error {
        let file = module_file(path, &module);
        let text = documents.source(path, &module).unwrap_or_default();
        found.push((file, diagnostic(text::range(&text, error.span), error.message.clone())));
        if !module.is_empty() {
            let line = reaching(documents, path, &source, &module).unwrap_or(1);
            found.push((path.to_path_buf(), diagnostic(text::line_range(&source, line), format!("{module}: {}", error.message))));
        }
    }
    if let (Some(document), Some(loaded)) = (documents.get_mut(path), checked.loaded) {
        document.loaded = Some(loaded);
        document.facts = checked.facts;
    }
    found
}

fn diagnostic(range: Range, message: String) -> Diagnostic {
    Diagnostic { range, severity: ERROR, source: "nib", message }
}

/// The line of the import in `source` through which module `target` is loaded.
fn reaching(documents: &Documents, main: &Path, source: &str, target: &str) -> Option<usize> {
    let imports = |source: &str| lex(source).ok().and_then(|tokens| parser::imports(&tokens).ok()).unwrap_or_default();
    imports(source).into_iter().find_map(|import| {
        let mut pending = vec![import.module.clone()];
        let mut seen = BTreeSet::new();
        while let Some(module) = pending.pop() {
            if module == target {
                return Some(import.span.line);
            }
            if seen.insert(module.clone()) {
                let source = documents.source(main, &module).unwrap_or_default();
                pending.extend(imports(&source).into_iter().map(|one| one.module));
            }
        }
        None
    })
}

/// What each check reported on each file: a file shows every one's.
#[derive(Default)]
pub struct Published {
    files: BTreeMap<PathBuf, BTreeMap<PathBuf, Vec<Diagnostic>>>,
}

impl Published {
    /// Replaces what checking `main` reported: each file whose diagnostics
    /// may have changed, and all it now has.
    pub fn replace(&mut self, main: &Path, found: Vec<(PathBuf, Diagnostic)>) -> Vec<(PathBuf, Vec<Diagnostic>)> {
        let mut changed: BTreeSet<PathBuf> = found.iter().map(|(file, _)| file.clone()).collect();
        for (file, by_check) in &mut self.files {
            if by_check.remove(main).is_some() {
                changed.insert(file.clone());
            }
        }
        for (file, one) in found {
            self.files.entry(file).or_default().entry(main.to_path_buf()).or_default().push(one);
        }
        changed
            .into_iter()
            .map(|file| {
                let mut all: Vec<Diagnostic> = Vec::new();
                for one in self.files.get(&file).into_iter().flat_map(BTreeMap::values).flatten() {
                    if !all.contains(one) {
                        all.push(one.clone());
                    }
                }
                (file, all)
            })
            .collect()
    }
}
