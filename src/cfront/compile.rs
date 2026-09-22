//! Port of `qbopt/cfront/compile.py`: C through Open Watcom's front end and
//! the backend, to an object or jwasm source.
//!
//! ```text
//! llrm-c pal.cgs -o pal.obj [--dump DIR] [--opt]
//! ```
//!
//! Stages not yet ported stop with [`CompileError::NotPorted`], naming the
//! Python function the port has reached.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use super::{hir, stream};

#[derive(Debug)]
pub enum CompileError {
    Unsupported(hir::Unsupported),
    NotPorted(&'static str),
    Io(std::io::Error),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(error) => write!(formatter, "{error}"),
            Self::NotPorted(function) => write!(formatter, "not yet ported: {function}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<hir::Unsupported> for CompileError {
    fn from(error: hir::Unsupported) -> Self {
        Self::Unsupported(error)
    }
}

impl From<std::io::Error> for CompileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn assembled(
    text: &str,
    _module: &str,
    _optimise: bool,
    dump: Option<&Path>,
    _cpu: &str,
) -> Result<(), CompileError> {
    let unit = hir::unit(&stream::parse(text))?;
    write(dump, "stream", text)?;
    write(dump, "hir", &hir::text(&unit))?;
    Err(CompileError::NotPorted("qbopt.cfront.raise_hir.raised"))
}

fn write(dump: Option<&Path>, stage: &str, text: &str) -> std::io::Result<()> {
    if let Some(dump) = dump {
        let path = dump.join(stage);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
    }
    Ok(())
}

struct Args {
    source: PathBuf,
    output: Option<PathBuf>,
    dump: Option<PathBuf>,
    opt: bool,
    cpu: String,
}

const USAGE: &str =
    "usage: llrm-c [-h] [-o OUTPUT] [-I INCLUDE] [--dump DUMP] [--opt] [--cpu CPU] source";

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let (mut source, mut output, mut dump, mut opt, mut cpu) =
        (None, None, None, false, "386".to_owned());
    let mut rest = argv.iter();
    while let Some(one) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or(format!("argument {name}: expected one argument"))
        };
        match one.as_str() {
            "-o" | "--output" => output = Some(PathBuf::from(value("-o/--output")?)),
            "-I" | "--include" => {
                value("-I/--include")?;
            }
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--opt" => opt = true,
            "--cpu" => cpu = value("--cpu")?,
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unrecognized arguments: {flag}"));
            }
            path if source.is_none() => source = Some(PathBuf::from(path)),
            extra => return Err(format!("unrecognized arguments: {extra}")),
        }
    }
    let source = source.ok_or("the following arguments are required: source")?;
    Ok(Args {
        source,
        output,
        dump,
        opt,
        cpu,
    })
}

/// `main`: exit status 0 on success, 1 on a refusal, 2 on bad arguments.
pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{USAGE}\nllrm-c: error: {message}");
            return 2;
        }
    };
    let result = (|| {
        if args.source.extension().and_then(|one| one.to_str()) != Some("cgs") {
            return Err(CompileError::NotPorted("qbopt.cfront.compile.recorded"));
        }
        let text = fs::read_to_string(&args.source)?;
        let _output = args
            .output
            .clone()
            .unwrap_or_else(|| args.source.with_extension("asm"));
        let module = args
            .source
            .file_stem()
            .and_then(|one| one.to_str())
            .unwrap_or_default();
        assembled(&text, module, args.opt, args.dump.as_deref(), &args.cpu)
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("llrm-c: {error}");
            1
        }
    }
}
