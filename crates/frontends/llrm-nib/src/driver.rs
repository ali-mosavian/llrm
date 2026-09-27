//! Port of `qbopt/frontend/modern/driver.py`: run the modern frontend and
//! decode its common-HIR document.

use std::fmt;
use std::path::Path;

use llrm_core::hir::{codec, model};

/// Source parsing or semantic analysis failed above HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendError(pub String);

impl fmt::Display for FrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FrontendError {}

/// The program at `source`. In process: `cargo run` of `nibfront` rebuilt
/// this crate in release after every edit before the first parse.
pub fn parsed(source: &Path, frontend: &super::Frontend, dump: Option<&Path>) -> Result<model::Program, FrontendError> {
    let text = super::compile_file(source, frontend).map_err(|(path, error)| refused(&path, &error))?;
    if let Some(dump) = dump {
        std::fs::write(dump, &text).map_err(|error| FrontendError(error.to_string()))?;
    }
    codec::decode(&text).map_err(|error| FrontendError(format!("the Nib frontend emitted invalid HIR: {error}")))
}

/// A diagnostic at `path`, as `nibfront` reports it.
pub fn refused(path: &Path, error: &super::Diagnostic) -> FrontendError {
    FrontendError(format!("{}:{error}", path.display()))
}
