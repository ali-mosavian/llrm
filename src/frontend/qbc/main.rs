//! Port of `qbopt/frontend/qb/__main__.py`, plus `tools/qbstages.py`'s
//! `--dump DIR` and `--mbf`.
//!
//! ```text
//! llrm-qb SOURCE [--dialect D] [--runtime R] [--array-order O] [--dump-hir PATH]
//!         [--huge-arrays] [--checked-arrays] [--unchecked-bounds] [--alternate-math]
//!         [--mbf] [--include DIR]... [--mir] [-o OUTPUT] [-O {s,2}] [--dump DIR]
//! ```

use std::path::PathBuf;

use super::compile;
use super::driver::parsed;
use super::qbstages;
use crate::flow;
use crate::hir::{codec, dump, lower};
use crate::model::passes::{Options, O2};

const USAGE: &str = "usage: llrm-qb [-h] [--dialect DIALECT] [--runtime RUNTIME] \
[--array-order {column-major,row-major}] [--dump-hir DUMP_HIR] [--huge-arrays] [--checked-arrays] \
[--unchecked-bounds] [--alternate-math] [--mbf] [--include INCLUDE] [--mir] [-o OUTPUT] [-O {s,2}] \
[--dump DUMP] source";

pub(super) struct Arguments {
    pub(super) source: PathBuf,
    pub(super) frontend: qbstages::Frontend,
    pub(super) dump_hir: Option<PathBuf>,
    pub(super) mir: bool,
    pub(super) output: Option<PathBuf>,
    pub(super) options: Options,
    pub(super) dump: Option<PathBuf>,
}

pub(super) fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let mut source = None;
    let mut frontend = qbstages::Frontend {
        dialect: "vbdos".into(),
        runtime: "vbdos".into(),
        array_order: "column-major".into(),
        ..Default::default()
    };
    let (mut dump_hir, mut mir, mut output, mut options, mut dump) = (None, false, None, O2(), None);
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
            "--dialect" => frontend.dialect = value("--dialect")?,
            "--runtime" => frontend.runtime = value("--runtime")?,
            "--array-order" => {
                let order = value("--array-order")?;
                if order != "column-major" && order != "row-major" {
                    return Err(format!(
                        "argument --array-order: invalid choice: '{order}' (choose from 'column-major', 'row-major')"
                    ));
                }
                frontend.array_order = order;
            }
            "--dump-hir" => dump_hir = Some(PathBuf::from(value("--dump-hir")?)),
            "--huge-arrays" => frontend.huge_arrays = true,
            "--checked-arrays" => frontend.checked_arrays = true,
            "--unchecked-bounds" => frontend.unchecked_bounds = true,
            "--alternate-math" => frontend.alternate_math = true,
            "--mbf" => frontend.mbf = true,
            "--include" => frontend.includes.push(PathBuf::from(value("--include")?)),
            "--mir" => mir = true,
            "-o" | "--output" => output = Some(PathBuf::from(value("-o/--output")?)),
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "-O" => options = flow::level_option(&value("-O")?)?,
            _ if flag.starts_with("-O") && flag.len() > 2 => options = flow::level_option(&flag[2..])?,
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let source = source.ok_or("the following arguments are required: source")?;
    if mir && output.is_some() {
        return Err("--mir and --output cannot be used together".into());
    }
    Ok(Arguments { source, frontend, dump_hir, mir, output, options, dump })
}

pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{USAGE}\nllrm-qb: error: {message}");
            return 2;
        }
    };
    let result = (|| -> Result<(), String> {
        if let Some(dump) = &args.dump {
            qbstages::dumped(&args.source, dump, &args.frontend, &args.options)?;
        }
        let frontend = &args.frontend;
        let program = parsed(
            &args.source,
            &frontend.dialect,
            &frontend.runtime,
            args.dump_hir.as_deref(),
            &frontend.includes,
            &frontend.array_order,
            frontend.huge_arrays,
            frontend.checked_arrays,
            frontend.unchecked_bounds,
            frontend.mbf,
            frontend.alternate_math,
        )
        .map_err(|error| error.0)?;
        if let Some(output) = &args.output {
            let bytes = compile::object_bytes(&program, &args.source, None, &args.options).map_err(|error| error.to_string())?;
            std::fs::write(output, bytes).map_err(|error| error.to_string())?;
        } else if args.mir {
            for body in lower::lower(&program).map_err(|error| error.to_string())? {
                print!("{}", dump::mir_text(&body));
            }
        } else if args.dump.is_none() {
            print!("{}", codec::encode(&program, None).map_err(|error| error.0)?);
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("llrm-qb: {error}");
            1
        }
    }
}
