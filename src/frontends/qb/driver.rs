//! Port of `qbopt/frontend/qb/driver.py`: invoke the isolated Rust parser and
//! decode its common-HIR document.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::hir::{codec, model};
use crate::support::pyjson::{self, Json};

#[allow(non_snake_case)]
pub fn ROOT() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[allow(non_snake_case)]
pub fn MANIFEST() -> PathBuf {
    ROOT().join("frontends").join("qb").join("Cargo.toml")
}

/// `DIALECTS`: the QB-family `hir.Dialect` values.
pub const DIALECTS: [&str; 4] = ["qbasic11", "qb45", "pds71", "vbdos"];
/// `RUNTIMES`: the QB-family `hir.RuntimeProfile` values.
pub const RUNTIMES: [&str; 3] = ["qb45", "pds71", "vbdos"];
/// `ARRAY_ORDERS`: every `hir.ArrayOrder` value.
pub const ARRAY_ORDERS: [&str; 2] = ["column-major", "row-major"];

/// Source parsing or semantic analysis failed above HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendError(pub String);

impl fmt::Display for FrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for FrontendError {}

/// The configured installed producer, or the in-tree Cargo executable.
pub fn command() -> Vec<String> {
    if let Ok(configured) = std::env::var("QBOPT_QBFRONT") {
        if !configured.is_empty() {
            return vec![configured];
        }
    }
    // Do not execute target/release/qbfront directly merely because it exists:
    // that made stage dumps silently use an older semantic frontend after a
    // source edit. Cargo's own dependency check is cheap when the build is
    // current and authoritative when it is not.
    ["cargo", "run", "--quiet", "--release", "--manifest-path"]
        .iter()
        .map(|one| (*one).to_owned())
        .chain([MANIFEST().display().to_string(), "--".to_owned()])
        .collect()
}

pub fn build_release() -> Result<PathBuf, FrontendError> {
    let result = Command::new("cargo")
        .args(["build", "--quiet", "--release", "--manifest-path"])
        .arg(MANIFEST())
        .args(["--bin", "qbfront", "--message-format=json"])
        .current_dir(ROOT())
        .output()
        .map_err(|error| FrontendError(format!("could not start cargo build: {error}")))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("cargo build exited with status {}", result.status.code().unwrap_or(-1))
        } else {
            stderr
        };
        return Err(FrontendError(message));
    }

    for line in String::from_utf8_lossy(&result.stdout).lines() {
        let message = pyjson::loads(line).map_err(|_| FrontendError("cargo build emitted invalid JSON".into()))?;
        let Json::Dict(message) = message else { continue };
        let field = |name: &str| match message.get(name) {
            Some(Json::Str(text)) => Some(text.as_str()),
            _ => None,
        };
        let Some(Json::Dict(target)) = message.get("target") else { continue };
        let named = matches!(target.get("name"), Some(Json::Str(name)) if name == "qbfront");
        let binary = matches!(target.get("kind"), Some(Json::List(kinds)) if kinds.iter().any(|one| matches!(one, Json::Str(kind) if kind == "bin")));
        if field("reason") == Some("compiler-artifact") && named && binary {
            if let Some(executable) = field("executable").filter(|one| !one.is_empty()) {
                return Ok(PathBuf::from(executable));
            }
        }
    }
    Err(FrontendError("cargo build did not report the qbfront executable".into()))
}

#[allow(clippy::too_many_arguments)]
fn _options(
    source: &Path,
    dialect: &str,
    runtime: &str,
    include_dirs: &[PathBuf],
    array_order: &str,
    huge_arrays: bool,
    checked_arrays: bool,
    unchecked_bounds: bool,
    mbf: bool,
    alternate_math: bool,
) -> Result<Vec<String>, FrontendError> {
    if !DIALECTS.contains(&dialect) {
        return Err(FrontendError(format!("unknown QB dialect '{dialect}'")));
    }
    if !RUNTIMES.contains(&runtime) {
        return Err(FrontendError(format!("unknown QB runtime '{runtime}'")));
    }
    if !ARRAY_ORDERS.contains(&array_order) {
        return Err(FrontendError(format!("unknown QB array order '{array_order}'")));
    }
    let mut out: Vec<String> =
        vec!["--dialect".into(), dialect.into(), "--runtime".into(), runtime.into(), "--array-order".into(), array_order.into()];
    if huge_arrays {
        out.push("--huge-arrays".into());
    }
    if checked_arrays {
        out.push("--checked-arrays".into());
    }
    if unchecked_bounds {
        out.push("--unchecked-bounds".into());
    }
    if mbf {
        out.push("--mbf".into());
    }
    if alternate_math {
        out.push("--alternate-math".into());
    }
    for directory in include_dirs {
        out.push("--include".into());
        out.push(directory.display().to_string());
    }
    out.push(source.display().to_string());
    Ok(out)
}

/// Run `command() + arguments` from ROOT: stdout, or the refusal.
fn _run(arguments: Vec<String>) -> Result<String, FrontendError> {
    let command = command();
    let result = Command::new(&command[0])
        .args(&command[1..])
        .args(arguments)
        .current_dir(ROOT())
        .output()
        .map_err(|error| FrontendError(error.to_string()))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!("qbfront exited with status {}", result.status.code().unwrap_or(-1))
        } else {
            stderr
        };
        return Err(FrontendError(message));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

/// Run only source loading and parsing, independently of semantic HIR support.
#[allow(clippy::too_many_arguments)]
pub fn syntax_checked(
    source: &Path,
    dialect: &str,
    runtime: &str,
    include_dirs: &[PathBuf],
    array_order: &str,
    huge_arrays: bool,
    checked_arrays: bool,
    unchecked_bounds: bool,
    mbf: bool,
    alternate_math: bool,
) -> Result<(), FrontendError> {
    let mut arguments = vec!["--syntax".to_owned()];
    arguments.extend(_options(
        source,
        dialect,
        runtime,
        include_dirs,
        array_order,
        huge_arrays,
        checked_arrays,
        unchecked_bounds,
        mbf,
        alternate_math,
    )?);
    _run(arguments).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub fn parsed(
    source: &Path,
    dialect: &str,
    runtime: &str,
    dump: Option<&Path>,
    include_dirs: &[PathBuf],
    array_order: &str,
    huge_arrays: bool,
    checked_arrays: bool,
    unchecked_bounds: bool,
    mbf: bool,
    alternate_math: bool,
) -> Result<model::Program, FrontendError> {
    let stdout = _run(_options(
        source,
        dialect,
        runtime,
        include_dirs,
        array_order,
        huge_arrays,
        checked_arrays,
        unchecked_bounds,
        mbf,
        alternate_math,
    )?)?;
    if let Some(dump) = dump {
        if let Some(parent) = dump.parent() {
            std::fs::create_dir_all(parent).map_err(|error| FrontendError(error.to_string()))?;
        }
        std::fs::write(dump, &stdout).map_err(|error| FrontendError(error.to_string()))?;
    }
    let mut program =
        codec::decode(&stdout).map_err(|error| FrontendError(format!("qbfront produced invalid HIR: {error}")))?;
    let family = program.runtime.value();
    for object_ in program.modules.iter_mut().flat_map(|module| &mut module.data) {
        if object_.linkage == model::DataLinkage::External && crate::abi::runtime::named_only(&object_.name, family) {
            object_.addressed = false;
        }
    }
    Ok(program)
}
