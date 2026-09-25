//! `textDocument/hover`: a declaration's signature and the comment above it,
//! or a local's type.

use std::path::Path;

use super::documents::Documents;
use super::protocol::{Hover, Markup, Position};
use super::resolve::{Target, resolve};
use super::text;

pub fn hover(documents: &Documents, path: &Path, position: Position) -> Option<Hover> {
    let value = match resolve(documents, path, position)? {
        Target::Declaration { module, declaration } => {
            let text = documents.source(path, &module)?;
            let comments = text::comments_above(&text, declaration.span.line);
            let signature = code(text::signature(&text, declaration.span.line));
            if comments.is_empty() { signature } else { format!("{signature}\n\n{}", comments.join("\n")) }
        }
        Target::Local { name, type_: Some(type_), .. } => code(&format!("{name}: {type_}")),
        Target::Local { bound, .. } => code(text::signature(&documents.source(path, "")?, bound.line)),
        Target::Module(module) => code(&format!("import {module}")),
    };
    Some(Hover { contents: Markup { kind: "markdown", value } })
}

fn code(text: &str) -> String {
    format!("```nib\n{text}\n```")
}
