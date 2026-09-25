//! `textDocument/definition`: where the name under the cursor is declared.

use std::path::Path;

use super::documents::{self, Documents, module_file};
use super::protocol::{Location, Position, Range};
use super::resolve::{Target, resolve};
use super::text;

pub fn definition(documents: &Documents, path: &Path, position: Position) -> Option<Location> {
    let (module, range) = match resolve(documents, path, position)? {
        Target::Declaration { module, declaration } => {
            let text = documents.source(path, &module)?;
            let range = text::range(&text, text::named(&text, declaration.span, declaration.short()));
            (module, range)
        }
        Target::Local { name, bound, .. } => {
            let text = documents.source(path, "")?;
            (String::new(), text::range(&text, text::named(&text, bound, &name)))
        }
        Target::Module(module) => (module, Range::default()),
    };
    Some(Location { uri: documents::uri(&module_file(path, &module)), range })
}
