//! What the name under the cursor is: a local of the function it is in, or
//! a declaration of this module or of one it imports.

use std::path::Path;

use super::documents::Documents;
use super::index::{self, Declaration};
use super::protocol::Position;
use super::text;
use crate::frontends::modern::modules::Loaded;
use crate::frontends::modern::scopes;
use crate::frontends::modern::semantic::{Fact, Known};
use crate::frontends::modern::syntax::{Expr, Function, Module, Span};

pub enum Target {
    /// A declaration of module `module`; `""` is the one being edited.
    Declaration { module: String, declaration: Declaration },
    /// A local or parameter, where it is bound, and its type when the checker knows it.
    Local { name: String, bound: Span, type_: Option<String> },
    Module(String),
}

/// What the name at `position` of the open document at `path` is.
pub fn resolve(documents: &Documents, path: &Path, position: Position) -> Option<Target> {
    let document = documents.get(path)?;
    let loaded = document.loaded.as_ref()?;
    let text = document.text.as_str();
    let (line, column) = text::at(text, position);
    let main = &loaded.modules[""];
    if let Some(local) = local(main, text, &document.facts, line, column) {
        return Some(local);
    }
    if let Some(member) = member(loaded, text, &document.facts, line, column) {
        return Some(member);
    }
    let path = text::path_at(text, line, column)?;
    for import in &main.imports {
        if path == import.name || path == import.module {
            return Some(Target::Module(import.module.clone()));
        }
        if let Some(rest) = path.strip_prefix(&format!("{}.", import.name)) {
            let declaration = index::find(loaded.modules.get(&import.module)?, rest)?;
            return Some(Target::Declaration { module: import.module.clone(), declaration });
        }
    }
    let declaration = index::find(main, &path)?;
    Some(Target::Declaration { module: String::new(), declaration })
}

/// The local named at the cursor, or bound there.
fn local(main: &Module, text: &str, facts: &[Fact], line: usize, column: usize) -> Option<Target> {
    for function in &main.functions {
        let (uses, bound) = locals(function);
        let at = uses.iter().find(|(span, _)| text::contains(*span, line, column)).map(|(_, local)| local);
        let Some((name, span)) = at.or_else(|| bound.iter().find(|(name, span)| text::contains(text::named(text, *span, name), line, column))) else {
            continue;
        };
        let type_ = uses.iter().filter(|(_, local)| local == &(name.clone(), *span)).find_map(|(used, _)| {
            facts.iter().find_map(|fact| match &fact.known {
                Known::Local(type_) if fact.span == *used && &fact.name == name => Some(type_.clone()),
                _ => None,
            })
        });
        return Some(Target::Local { name: name.clone(), bound: *span, type_ });
    }
    None
}

/// Each name `function` reads that is a local, and that local; and each
/// local it binds that something after it can see.
fn locals(function: &Function) -> (Vec<(Span, (String, Span))>, Vec<(String, Span)>) {
    let mut bound: Vec<(String, Span)> = function.parameters.iter().map(|one| (one.name.clone(), one.span)).collect();
    let mut locals = bound.clone();
    let mut uses = Vec::new();
    let Ok(()) = scopes::walk_mut(&mut function.body.clone(), &mut locals, &mut |expression, locals: &[(String, Span)]| -> Result<(), std::convert::Infallible> {
        for local in locals {
            if !bound.contains(local) {
                bound.push(local.clone());
            }
        }
        if let Expr::Name(name, span) = expression {
            if let Some(local) = locals.iter().rev().find(|(one, _)| one == name) {
                uses.push((*span, local.clone()));
            }
        }
        Ok(())
    });
    (uses, bound)
}

/// The field or method at the cursor, of the type the checker found its value has.
fn member(loaded: &Loaded, text: &str, facts: &[Fact], line: usize, column: usize) -> Option<Target> {
    let (owner, name) = facts.iter().find_map(|fact| match &fact.known {
        Known::Member(owner) if fact.span.module == 0 && text::contains(fact.span, line, column) && text::spells(text, fact.span, &fact.name) => {
            Some((owner, &fact.name))
        }
        _ => None,
    })?;
    // A linked name is qualified by its module's.
    let module = loaded
        .modules
        .keys()
        .filter(|module| !module.is_empty() && owner.starts_with(&format!("{module}.")))
        .max_by_key(|module| module.len())
        .cloned()
        .unwrap_or_default();
    let owner = if module.is_empty() { owner.as_str() } else { &owner[module.len() + 1..] };
    let declaration = index::find(&loaded.modules[&module], &format!("{owner}.{name}"))?;
    Some(Target::Declaration { module, declaration })
}
