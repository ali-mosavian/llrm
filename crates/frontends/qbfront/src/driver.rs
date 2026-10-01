//! One source compiled to HIR text, as the `qbfront` program and llrm-qb both
//! do: the one path from a file and its options to a program or a refusal.

use std::fs;
use std::path::{Path, PathBuf};

use crate::semantic::{compile_debugged, DebugSource, Options};
use crate::{parse, source, Dialect};

/// What to compile and how.
#[derive(Clone, Debug)]
pub struct Args {
    pub dialect: Dialect,
    pub runtime: String,
    pub options: Options,
    /// `-g`: each statement's source line.
    pub debug: bool,
    /// Parse only; the answer is a one-line summary, not HIR.
    pub syntax: bool,
    pub include_dirs: Vec<PathBuf>,
    /// Where to write the source after its includes are expanded.
    pub dump_source: Option<PathBuf>,
    pub input: PathBuf,
}

/// What a compile answers: HIR text (or the summary of `syntax`), and the
/// warnings it raised, each as the program prints it.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub text: String,
    pub warnings: Vec<String>,
}

/// `args.input` as HIR text, or why not: the message as the program prints it.
pub fn compile(args: &Args) -> Result<Compiled, String> {
    let input = args.input.display().to_string();
    let source = source::load_with_map(&args.input, &args.include_dirs).map_err(|error| format!("qbfront: {input}: {error}"))?;
    if let Some(path) = &args.dump_source {
        fs::write(path, &source.text).map_err(|error| format!("qbfront: {}: {error}", path.display()))?;
    }
    let module = parse(&source.text, args.dialect).map_err(|error| {
        let location = source.location(error.span.line);
        let path = location.map(|location| location.path.display().to_string()).unwrap_or_else(|| input.clone());
        let line = location.map_or(error.span.line, |location| location.line);
        format!("{path}:{}:{}: {}", line, error.span.start + 1, error.message)
    })?;
    if args.syntax {
        let text = format!("ok: {} module statements, {} procedures\n", module.statements.len(), module.procedures.len());
        return Ok(Compiled { text, warnings: Vec::new() });
    }
    let name = Path::new(&input).file_stem().and_then(|one| one.to_str()).unwrap_or("module");
    let expanded = source.text.lines().count();
    let debugged = Some(DebugSource {
        text: if args.debug { source.text.lines().map(str::to_owned).collect() } else { Vec::new() },
        lines: (1..=expanded).map(|line| source.location(line).map_or(0, |one| one.main_line)).collect(),
        debug: args.debug,
    });
    let (text, warnings) = compile_debugged(&module, name, args.dialect, &args.runtime, &args.options, debugged).map_err(|error| format!("{input}: {}", error.message))?;
    Ok(Compiled { text, warnings: warnings.into_iter().map(|one| format!("{input}: {one}")).collect() })
}
