//! Port of `qbopt/frontend/qb/__main__.py`, plus `tools/qbstages.py`'s
//! `--dump DIR` and `--mbf`.
//!
//! ```text
//! llrm-qb SOURCE [--dialect D] [--runtime R] [--array-order O] [--dump-hir PATH]
//!         [--huge-arrays] [--alternate-math]
//!         [--mbf] [--whole-program] [--array-merging] [--runtime-frames] [--error-lines] [--include DIR]... [--dump DIR] [OPTIONS]
//! ```
//!
//! OPTIONS are gcc's, as `llrm_core::driver::flags` takes them;
//! `-fsanitize=bounds,integer-divide-by-zero,signed-integer-overflow` (all
//! three: `undefined`) are BC's /D checks, and `-g` its /Zi line numbers.

use std::path::PathBuf;

use llrm_core::driver::{
    self as codegen,
    flags::{self, Flags},
};
use llrm_core::hir::codec;

use super::compile;
use super::driver::{self, Frontend, parsed};
use super::qbstages;

fn usage() -> String {
    format!(
        "usage: llrm-qb [-h] [--dialect DIALECT] [--runtime RUNTIME] [--array-order {{column-major,row-major}}] [--dump-hir DUMP_HIR] \
[--huge-arrays] [--alternate-math] [--mbf] [--whole-program] [--array-merging] [--runtime-frames] [--error-lines] \
[--include INCLUDE] [--dump DUMP] {} source",
        flags::USAGE
    )
}

pub(super) struct Arguments {
    pub(super) source: PathBuf,
    pub(super) frontend: Frontend,
    pub(super) dump_hir: Option<PathBuf>,
    pub(super) flags: Flags,
    pub(super) dump: Option<PathBuf>,
    /// The target, the built-in DOS unless `--machine` names another, and
    /// the pipeline.
    pub(super) codegen: codegen::Options,
}

pub(super) fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let mut source = None;
    let mut frontend = Frontend::new("vbdos", "vbdos");
    let (mut dump_hir, mut flags, mut dump) = (None, Flags::default(), None);
    let mut at = 0;
    while at < argv.len() {
        // The runtime the program calls: the Microsoft ones by their stack ABI, or llrm's by the target's own.
        if let Some(runtime) = argv[at].strip_prefix("-fqb-runtime=") {
            frontend.runtime = runtime.to_owned();
            at += 1;
            continue;
        }
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
            "--alternate-math" => frontend.alternate_math = true,
            "--mbf" => frontend.mbf = true,
            "--whole-program" => frontend.whole_program = true,
            "--array-merging" => frontend.array_merging = true,
            "--own-frames" => {}
            "--runtime-frames" => frontend.runtime_frames = true,
            "--error-lines" => frontend.error_lines = true,
            "--include" => frontend.includes.push(PathBuf::from(value("--include")?)),
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let source = source.ok_or("the following arguments are required: source")?;
    frontend.checked_arrays = flags.sanitize.bounds;
    frontend.checked_division = flags.sanitize.integer_divide_by_zero;
    frontend.debug = flags.debug;
    frontend.checked_overflow = flags.sanitize.signed_integer_overflow;
    frontend.checked_stack = flags.sanitize.stack;
    let bound =
        llrm_driver::planned(&flags, Some(&["x86-m16", "x86-m32"]), Some("https://github.com/ali-mosavian/llrm/issues/1160"))?;
    frontend.segment_bytes = bound.target.layout().segment_bytes();
    // The bytes of the pointer that is the near space, and of the one that is the far: a target with one
    // space has the same for both.
    let spaces = &bound.target.layout().spaces;
    let width = |space: u32| spaces.unmarked.keys().copied().filter(|bytes| spaces.unmarked(*bytes) == Ok(space)).max();
    frontend.near_bytes = width(spaces.near).map_or(0, |bytes| bytes as usize);
    frontend.far_bytes = width(spaces.far).map_or(frontend.near_bytes, |bytes| bytes as usize);
    let codegen = bound.options(&flags, flags.machine(&*bound.target, bound.target.machine().with_stack_in_data())?);
    Ok(Arguments { source, frontend, dump_hir, flags, dump, codegen })
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
            qbstages::dumped(&args.source, dump, &args.frontend, &args.codegen)?;
        }
        let mut program = llrm_core::support::debug::timed("frontend", || {
            parsed(&args.source, &args.frontend, args.dump_hir.as_deref())
        })
        .map_err(|error| error.0)?;
        if program.runtime.calls_natively() {
            driver::natively_called(&mut program, args.codegen.arch.calling().native().cc.as_deref());
        }
        if args.flags.assembly {
            let module = compile::assembled(&program, None, &args.codegen).map_err(|error| error.to_string())?;
            let output = args.flags.output.clone().unwrap_or_else(|| args.source.with_extension("asm"));
            std::fs::write(output, llrm_core::driver::basic::text(&module)?).map_err(|error| error.to_string())?;
        } else if let Some(output) = &args.flags.output {
            let bytes = compile::object_bytes(&program, &args.source, None, &args.codegen)
                .map_err(|error| error.to_string())?;
            llrm_core::support::debug::timed("write output", || std::fs::write(output, bytes))
                .map_err(|error| error.to_string())?;
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
