//! One source compiled to HIR text, as llrm-qb does: the one path from a file
//! and its options to a program or a refusal.

use std::fs;
use std::path::{Path, PathBuf};

use crate::semantic::{DebugSource, Options, compile_debugged};
use crate::{Dialect, parse, source};

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
    let source =
        source::load_with_map(&args.input, &args.include_dirs).map_err(|error| format!("qbfront: {input}: {error}"))?;
    if let Some(path) = &args.dump_source {
        fs::write(path, &source.text).map_err(|error| format!("qbfront: {}: {error}", path.display()))?;
    }
    let module = parse(&source.text, args.dialect).map_err(|error| {
        let location = source.location(error.span.line);
        let path = location.map(|location| location.path.display().to_string()).unwrap_or_else(|| input.clone());
        let line = location.map_or(error.span.line, |location| location.line);
        render(&error, &path, line, &source.text)
    })?;
    if args.syntax {
        let text =
            format!("ok: {} module statements, {} procedures\n", module.statements.len(), module.procedures.len());
        return Ok(Compiled { text, warnings: Vec::new() });
    }
    let name = Path::new(&input).file_stem().and_then(|one| one.to_str()).unwrap_or("module");
    let expanded = source.text.lines().count();
    let debugged = Some(DebugSource {
        text: if args.debug { source.text.lines().map(str::to_owned).collect() } else { Vec::new() },
        lines: (1..=expanded).map(|line| source.location(line).map_or(0, |one| one.main_line)).collect(),
        debug: args.debug,
    });
    let (text, warnings) = compile_debugged(&module, name, args.dialect, &args.runtime, &args.options, debugged)
        .map_err(|error| render_semantic(&error.message, &input, &source))?;
    Ok(Compiled { text, warnings: warnings.into_iter().map(|one| format!("{input}: {one}")).collect() })
}

/// An error as the program prints it: where, what, the source line with the
/// place marked, and what to do.
fn render(
    error: &crate::ParseError,
    path: &str,
    line: usize,
    text: &str,
) -> String {
    let mut out = format!("{path}:{line}:{}: {}", error.span.start + 1, error.message);
    if let Some(source_line) = error.span.line.checked_sub(1).and_then(|index| text.lines().nth(index)) {
        let width = error.span.end.saturating_sub(error.span.start).max(1);
        let gutter = line.to_string();
        out.push_str(&format!("\n {gutter} | {source_line}"));
        out.push_str(&format!(
            "\n {} | {}{}",
            " ".repeat(gutter.len()),
            " ".repeat(error.span.start),
            "^".repeat(width)
        ));
    }
    if let Some(help) = &error.help {
        for (index, row) in help.lines().enumerate() {
            out.push_str(&format!("\n {}{row}", if index == 0 { "help: " } else { "      " }));
        }
    }
    out
}

/// A compile error as the program prints it. The pass names the line it was at
/// as `line N: ` in the expanded source; that becomes the file and line the
/// person wrote, with the line itself under it.
fn render_semantic(
    message: &str,
    input: &str,
    source: &source::LoadedSource,
) -> String {
    let Some((number, rest)) = message.strip_prefix("line ").and_then(|tail| tail.split_once(": ")) else {
        return format!("{input}: {message}");
    };
    let Ok(expanded) = number.parse::<usize>() else { return format!("{input}: {message}") };
    let location = source.location(expanded);
    let path = location.map(|one| one.path.display().to_string()).unwrap_or_else(|| input.to_owned());
    let line = location.map_or(expanded, |one| one.line);
    let mut out = format!("{path}:{line}: {rest}");
    if let Some(text) = source.text.lines().nth(expanded.saturating_sub(1)) {
        let gutter = line.to_string();
        out.push_str(&format!("\n {gutter} | {}", text.trim_end()));
    }
    out
}
