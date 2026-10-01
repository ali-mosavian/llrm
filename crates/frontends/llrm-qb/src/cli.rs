//! Port of `qbopt/frontend/qb/__main__.py`, plus `tools/qbstages.py`'s
//! `--dump DIR` and `--mbf`.
//!
//! ```text
//! llrm-qb SOURCE [--dialect D] [--runtime R] [--array-order O] [--dump-hir PATH]
//!         [--huge-arrays] [--unchecked-bounds] [--alternate-math]
//!         [--mbf] [--whole-program] [--array-merging] [--own-frames] [--error-lines] [--include DIR]... [--mir] [--dump DIR] [--legacy] [OPTIONS]
//! ```
//!
//! OPTIONS are gcc's, as `llrm_core::driver::flags` takes them;
//! `-fsanitize=bounds,integer-divide-by-zero,signed-integer-overflow` (all
//! three: `undefined`) are BC's /D checks, and `-g` its /Zi line numbers.
//! `--legacy` compiles through the old MIR.

use std::path::PathBuf;

use super::compile;
use super::driver::{parsed, Frontend};
use super::qbstages;
use llrm_core::backend::masm;
use llrm_core::driver::{self as codegen, flags::{self, Flags}};
use llrm_core::hir::{codec, dump, lower};
use llrm_core::model::passes::Options;

fn usage() -> String {
    format!(
        "usage: llrm-qb [-h] [--dialect DIALECT] [--runtime RUNTIME] [--array-order {{column-major,row-major}}] [--dump-hir DUMP_HIR] \
[--huge-arrays] [--unchecked-bounds] [--alternate-math] [--mbf] [--whole-program] [--array-merging] [--own-frames] [--error-lines] \
[--include INCLUDE] [--mir] [--dump DUMP] [--legacy] {} source",
        flags::USAGE
    )
}

pub(super) struct Arguments {
    pub(super) source: PathBuf,
    pub(super) frontend: Frontend,
    pub(super) dump_hir: Option<PathBuf>,
    pub(super) mir: bool,
    pub(super) flags: Flags,
    /// The old MIR's options, for `--legacy`.
    pub(super) options: Options,
    pub(super) dump: Option<PathBuf>,
    pub(super) route: compile::Route,
    /// The target, the built-in DOS unless `--machine` names another, and
    /// the pipeline.
    pub(super) codegen: codegen::Options,
}

pub(super) fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let mut source = None;
    let mut frontend = Frontend::new("vbdos", "vbdos");
    let (mut dump_hir, mut mir, mut flags, mut dump) = (None, false, Flags::default(), None);
    let mut route = compile::Route::Selected;
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
            "--unchecked-bounds" => frontend.unchecked_bounds = true,
            "--alternate-math" => frontend.alternate_math = true,
            "--mbf" => frontend.mbf = true,
            "--whole-program" => frontend.whole_program = true,
            "--array-merging" => frontend.array_merging = true,
            "--own-frames" => frontend.own_frames = true,
            "--error-lines" => frontend.error_lines = true,
            "--include" => frontend.includes.push(PathBuf::from(value("--include")?)),
            "--mir" => mir = true,
            "--legacy" => route = compile::Route::Lowered,
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let source = source.ok_or("the following arguments are required: source")?;
    if mir && flags.output.is_some() {
        return Err("--mir and --output cannot be used together".into());
    }
    frontend.checked_arrays = flags.sanitize.bounds;
    frontend.checked_division = flags.sanitize.integer_divide_by_zero;
    frontend.debug = flags.debug;
    frontend.checked_overflow = flags.sanitize.signed_integer_overflow;
    let codegen = flags.driver(flags.machine(llrm_core::abi::machine::BASIC.clone())?);
    Ok(Arguments { source, frontend, dump_hir, mir, options: flags.legacy(), flags, dump, route, codegen })
}

pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{}\nllrm-qb: error: {message}", usage());
            return 2;
        }
    };
    let result = (|| -> Result<(), String> {
        if let Some(dump) = &args.dump {
            qbstages::dumped(&args.source, dump, &args.frontend, &args.options, args.route, &args.codegen)?;
        }
        let program = parsed(&args.source, &args.frontend, args.dump_hir.as_deref()).map_err(|error| error.0)?;
        if args.flags.assembly {
            let module = compile::assembled_by(&program, None, &args.options, args.route, &args.codegen).map_err(|error| error.to_string())?;
            let output = args.flags.output.clone().unwrap_or_else(|| args.source.with_extension("asm"));
            std::fs::write(output, llrm_core::driver::basic::text(&module)?).map_err(|error| error.to_string())?;
        } else if let Some(output) = &args.flags.output {
            let bytes = compile::object_bytes_by(&program, &args.source, None, &args.options, args.route, &args.codegen).map_err(|error| error.to_string())?;
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
