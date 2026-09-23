//! Port of `qbopt/frontend/modern/driver.py`: invoke the root Rust frontend
//! binary and decode its common-HIR document.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::hir::{codec, model};

#[allow(non_snake_case)]
pub fn ROOT() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[allow(non_snake_case)]
pub fn MANIFEST() -> PathBuf {
    ROOT().join("Cargo.toml")
}

/// Source parsing or semantic analysis failed above HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendError(pub String);

impl fmt::Display for FrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FrontendError {}

pub fn command() -> Vec<String> {
    if let Ok(configured) = std::env::var("QBOPT_MODERNFRONT") {
        if !configured.is_empty() {
            return vec![configured];
        }
    }
    ["cargo", "run", "--quiet", "--release", "--manifest-path"]
        .iter()
        .map(|one| (*one).to_owned())
        .chain([MANIFEST().display().to_string(), "--bin".to_owned(), "modernfront".to_owned(), "--".to_owned()])
        .collect()
}

pub fn parsed(source: &Path, dump: Option<&Path>) -> Result<model::Program, FrontendError> {
    let command = command();
    let result = Command::new(&command[0])
        .args(&command[1..])
        .arg(source)
        .current_dir(ROOT())
        .output()
        .map_err(|error| FrontendError(format!("could not start modern frontend: {error}")))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("modernfront exited with status {}", result.status.code().unwrap_or(-1))
        } else {
            stderr
        };
        return Err(FrontendError(message));
    }
    let stdout = String::from_utf8_lossy(&result.stdout).into_owned();
    if let Some(dump) = dump {
        std::fs::write(dump, &stdout).map_err(|error| FrontendError(error.to_string()))?;
    }
    codec::decode(&stdout).map_err(|error| FrontendError(format!("modernfront emitted invalid HIR: {error}")))
}
