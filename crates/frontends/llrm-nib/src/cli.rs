//! `tools/modernexe.py`'s compile, to an object rather than a linked
//! executable, plus `tools/modernstages.py`'s `--dump DIR`.
//!
//! ```text
//! llrm-nib SOURCE [--entry ENTRY] [--dump DIR] [--procedure-segments] [--used-by OBJ]... [--unchecked-bounds] [--legacy] [OPTIONS]
//! ```
//!
//! OPTIONS are gcc's, as `llrm_core::driver::flags` takes them. Without
//! `-o`, the object, or with `-S` the assembly, goes beside the source
//! unless `--dump` is given.
//! `--procedure-segments` gives each procedure a code segment, all of one
//! name, for a linker that drops unreferenced ones (jwlink's `option
//! eliminate`); Microsoft LINK wants each name defined once. With
//! `--used-by`, only the exports those objects name stay exported, and the
//! rest is dropped with whatever only they call. `--legacy` compiles through
//! the old MIR, for a program the rich MIR refuses.

use std::path::PathBuf;

use super::compile as nib;
use super::driver;
use super::nibstages;
use llrm_core::backend::cpu::ProfileOrName;
use llrm_core::backend::masm;
use llrm_core::backend::omfwrite::CodeLayout;
use llrm_core::driver::{self as codegen, flags::{self, Flags}};
use llrm_core::model::passes::Options;

fn usage() -> String {
    format!("usage: llrm-nib [-h] [--entry ENTRY] [--dump DUMP] [--procedure-segments] [--used-by OBJ]... [--unchecked-bounds] [--legacy] {} source", flags::USAGE)
}

struct Arguments {
    source: PathBuf,
    flags: Flags,
    entry: String,
    options: Options,
    dump: Option<PathBuf>,
    layout: CodeLayout,
    used_by: Vec<PathBuf>,
    frontend: super::Frontend,
    legacy: bool,
    /// The target, the built-in DOS on `nib::CPU` unless `--machine` names
    /// another, and the pipeline.
    codegen: codegen::Options,
}

fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let (mut source, mut flags, mut entry, mut dump) = (None, Flags::default(), "main".to_owned(), None);
    let mut layout = CodeLayout::OneSegment;
    let mut used_by = Vec::new();
    let mut frontend = super::Frontend::default();
    let mut legacy = false;
    let mut at = 0;
    while at < argv.len() {
        if flags.take(argv, &mut at)? {
            at += 1;
            continue;
        }
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
            "--entry" => entry = value("--entry")?,
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--procedure-segments" => layout = CodeLayout::PerProcedure,
            "--used-by" => used_by.push(PathBuf::from(value("--used-by")?)),
            "--unchecked-bounds" => frontend.unchecked_bounds = true,
            "--legacy" => legacy = true,
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let source = source.ok_or("the following arguments are required: source")?;
    let codegen = flags.driver(flags.machine(nib::machine())?);
    Ok(Arguments { source, options: flags.legacy(), flags, entry, dump, layout, used_by, frontend, legacy, codegen })
}

/// The symbols `objects` import.
fn used(objects: &[PathBuf]) -> Result<std::collections::BTreeSet<String>, String> {
    let mut names = std::collections::BTreeSet::new();
    for object in objects {
        let bytes = std::fs::read(object).map_err(|error| format!("{}: {error}", object.display()))?;
        let records = llrm_core::objectfile::omf::parse(&bytes).map_err(|error| format!("{}: {error:?}", object.display()))?;
        names.extend(llrm_core::objectfile::omf::externals(&records).into_iter().skip(1));
    }
    Ok(names)
}

pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{}\nllrm-nib: error: {message}", usage());
            return 2;
        }
    };
    let result = (|| -> Result<(), String> {
        if let Some(dump) = &args.dump {
            nibstages::dumped(&args.source, dump, &args.frontend, &args.options)?;
        }
        let output = match (&args.flags.output, &args.dump) {
            (Some(output), _) => output.clone(),
            (None, None) => args.source.with_extension(if args.flags.assembly { "asm" } else { "obj" }),
            (None, Some(_)) => return Ok(()),
        };
        let mut program = driver::parsed(&args.source, &args.frontend, None).map_err(|error| error.0)?;
        if !args.used_by.is_empty() {
            nib::keep_exports(&mut program, &used(&args.used_by)?);
        }
        let module = if args.legacy {
            nib::assembled(&program, &args.entry, ProfileOrName::Name(nib::CPU), &args.options)?
        } else {
            nib::assembled_from_mir(&program, &args.entry, &args.codegen)?
        };
        let bytes = if args.flags.assembly {
            masm::text(&module).map_err(|error| error.to_string())?.into_bytes()
        } else {
            nib::object(&module, &args.source, args.layout)?
        };
        std::fs::write(&output, &bytes).map_err(|error| error.to_string())?;
        println!("{} ({} bytes)", output.display(), bytes.len());
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("llrm-nib: {error}");
            1
        }
    }
}
