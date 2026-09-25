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
pub const DIALECTS: [&str; 5] = ["qbasic11", "qb45", "pds71", "vbdos", "quickr"];
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

/// How qbfront compiles a source: its options, as BC's switches name them.
#[derive(Clone, Debug)]
pub struct Frontend {
    pub dialect: String,
    pub runtime: String,
    pub array_order: String,
    pub huge_arrays: bool,
    pub checked_arrays: bool,
    pub unchecked_bounds: bool,
    pub mbf: bool,
    pub alternate_math: bool,
    /// Nothing outside the source calls its procedures: `--whole-program`.
    pub whole_program: bool,
    pub includes: Vec<PathBuf>,
}

impl Frontend {
    /// `dialect` on `runtime`, column-major, every switch off.
    pub fn new(dialect: &str, runtime: &str) -> Self {
        Self {
            dialect: dialect.into(),
            runtime: runtime.into(),
            array_order: "column-major".into(),
            huge_arrays: false,
            checked_arrays: false,
            unchecked_bounds: false,
            mbf: false,
            alternate_math: false,
            whole_program: false,
            includes: Vec::new(),
        }
    }
}

fn _options(source: &Path, frontend: &Frontend) -> Result<Vec<String>, FrontendError> {
    let Frontend { dialect, runtime, array_order, .. } = frontend;
    if !DIALECTS.contains(&dialect.as_str()) {
        return Err(FrontendError(format!("unknown QB dialect '{dialect}'")));
    }
    if !RUNTIMES.contains(&runtime.as_str()) {
        return Err(FrontendError(format!("unknown QB runtime '{runtime}'")));
    }
    if !ARRAY_ORDERS.contains(&array_order.as_str()) {
        return Err(FrontendError(format!("unknown QB array order '{array_order}'")));
    }
    let mut out: Vec<String> =
        vec!["--dialect".into(), dialect.clone(), "--runtime".into(), runtime.clone(), "--array-order".into(), array_order.clone()];
    for (on, flag) in [
        (frontend.huge_arrays, "--huge-arrays"),
        (frontend.checked_arrays, "--checked-arrays"),
        (frontend.unchecked_bounds, "--unchecked-bounds"),
        (frontend.mbf, "--mbf"),
        (frontend.alternate_math, "--alternate-math"),
        (frontend.whole_program, "--whole-program"),
    ] {
        if on {
            out.push(flag.into());
        }
    }
    for directory in &frontend.includes {
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
pub fn syntax_checked(source: &Path, frontend: &Frontend) -> Result<(), FrontendError> {
    let mut arguments = vec!["--syntax".to_owned()];
    arguments.extend(_options(source, frontend)?);
    _run(arguments).map(|_| ())
}

pub fn parsed(source: &Path, frontend: &Frontend, dump: Option<&Path>) -> Result<model::Program, FrontendError> {
    let stdout = _run(_options(source, frontend)?)?;
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
