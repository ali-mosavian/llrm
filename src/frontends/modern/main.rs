//! `tools/modernexe.py`'s compile, to an object rather than a linked
//! executable, plus `tools/modernstages.py`'s `--dump DIR`.
//!
//! ```text
//! llrm-modern SOURCE [-o OUTPUT] [--entry ENTRY] [-O {s,2}] [--dump DIR] [--procedure-segments] [--used-by OBJ]...
//! ```
//!
//! Without `-o`, the object goes beside the source unless `--dump` is given.
//! `--procedure-segments` gives each procedure a code segment, all of one
//! name, for a linker that drops unreferenced ones (jwlink's `option
//! eliminate`); Microsoft LINK wants each name defined once. With
//! `--used-by`, only the exports those objects name stay exported, and the
//! rest is dropped with whatever only they call.

use std::path::PathBuf;

use super::compile as modern;
use super::driver;
use super::modernstages;
use crate::backend::omfwrite::CodeLayout;
use crate::flow;
use crate::model::passes::{Options, O2};

const USAGE: &str = "usage: llrm-modern [-h] [-o OUTPUT] [--entry ENTRY] [-O {s,2}] [--dump DUMP] [--procedure-segments] [--used-by OBJ]... source";

struct Arguments {
    source: PathBuf,
    output: Option<PathBuf>,
    entry: String,
    options: Options,
    dump: Option<PathBuf>,
    layout: CodeLayout,
    used_by: Vec<PathBuf>,
}

fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let (mut source, mut output, mut entry, mut options, mut dump) = (None, None, "main".to_owned(), O2(), None);
    let mut layout = CodeLayout::OneSegment;
    let mut used_by = Vec::new();
    let mut at = 0;
    while at < argv.len() {
        let argument = argv[at].as_str();
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_owned())),
            _ => (argument, None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            if let Some(value) = inline.clone() {
                return Ok(value);
            }
            at += 1;
            argv.get(at).cloned().ok_or_else(|| format!("argument {name}: expected one argument"))
        };
        match flag {
            "-o" | "--output" => output = Some(PathBuf::from(value("-o/--output")?)),
            "--entry" => entry = value("--entry")?,
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--procedure-segments" => layout = CodeLayout::PerProcedure,
            "--used-by" => used_by.push(PathBuf::from(value("--used-by")?)),
            "-O" => options = flow::level_option(&value("-O")?)?,
            _ if flag.starts_with("-O") && flag.len() > 2 => options = flow::level_option(&flag[2..])?,
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let source = source.ok_or("the following arguments are required: source")?;
    Ok(Arguments { source, output, entry, options, dump, layout, used_by })
}

/// The symbols `objects` import.
fn used(objects: &[PathBuf]) -> Result<std::collections::BTreeSet<String>, String> {
    let mut names = std::collections::BTreeSet::new();
    for object in objects {
        let bytes = std::fs::read(object).map_err(|error| format!("{}: {error}", object.display()))?;
        let records = crate::objectfile::omf::parse(&bytes).map_err(|error| format!("{}: {error:?}", object.display()))?;
        names.extend(crate::objectfile::omf::externals(&records).into_iter().skip(1));
    }
    Ok(names)
}

pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{USAGE}\nllrm-modern: error: {message}");
            return 2;
        }
    };
    let result = (|| -> Result<(), String> {
        if let Some(dump) = &args.dump {
            modernstages::dumped(&args.source, dump, &args.options)?;
        }
        let output = match (&args.output, &args.dump) {
            (Some(output), _) => output.clone(),
            (None, None) => args.source.with_extension("obj"),
            (None, Some(_)) => return Ok(()),
        };
        let mut program = driver::parsed(&args.source, None).map_err(|error| error.0)?;
        if !args.used_by.is_empty() {
            modern::keep_exports(&mut program, &used(&args.used_by)?);
        }
        let bytes = modern::written_as(&program, &args.entry, &args.source, &args.options, args.layout)?;
        std::fs::write(&output, &bytes).map_err(|error| error.to_string())?;
        println!("{} ({} bytes)", output.display(), bytes.len());
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("llrm-modern: {error}");
            1
        }
    }
}
