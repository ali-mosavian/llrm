//! The command line, plus `--dump DIR` (the stage
//! dump, `qbstages`) and `--mbf`.
//!
//! ```text
//! llrm-qb SOURCE [--dialect D] [--runtime R] [--array-order O] [--dump-hir PATH]
//!         [--huge-arrays] [--alternate-math]
//!         [--mbf] [--whole-program] [--array-merging] [--runtime-frames] [--error-lines] [--bc-codeview] [--run-time-module] [--include DIR]... [--dump DIR] [OPTIONS]
//! ```
//!
//! OPTIONS are gcc's, as `llrm_codegen::driver::flags` takes them;
//! `-fsanitize=bounds,integer-divide-by-zero,signed-integer-overflow` (all
//! three: `undefined`) are BC's /D checks, and `-g` its /Zi line numbers.

use std::path::PathBuf;

use llrm_codegen::driver::{
    self as codegen,
    flags::{self, Flags},
};
use llrm_codegen::hir::codec;

use super::compile;
use super::driver::{self, Frontend, parsed};
use super::qbstages;

fn usage() -> String {
    format!(
        "usage: llrm-qb [-h] [--dialect DIALECT] [--runtime RUNTIME] [--array-order {{column-major,row-major}}] [--dump-hir DUMP_HIR] \
[--huge-arrays] [--alternate-math] [--mbf] [--whole-program] [--array-merging] [--runtime-frames] [--error-lines] [--bc-codeview] [--run-time-module] \
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
    /// `--arm64=linux|darwin`: the object is arm64's, from the 64-bit lowering.
    pub(super) arm64: Option<llrm_arm64::machine::Platform>,
    /// `--wasm32`: the object is wasm32's, from the 32-bit lowering.
    pub(super) wasm32: bool,
}

pub(super) fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let mut source = None;
    // The runtime is llrm's unless `--runtime` or `-fqb-runtime=` names a
    // Microsoft one.
    let mut frontend = Frontend::new("vbdos", "llrm");
    // An assignment to a SINGLE or DOUBLE variable is not rounded to its type,
    // as in C: `-fexcess-precision=standard` asks for BC's rounding.
    let mut flags = Flags::default();
    flags.float_store = false;
    let (mut dump_hir, mut dump) = (None, None);
    let mut arm64 = None;
    let mut wasm32 = false;
    let mut wasm_exceptions = false;
    let mut at = 0;
    while at < argv.len() {
        // The runtime the program calls: the Microsoft ones by their stack ABI,
        // or llrm's by the target's own.
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
            "--bc-codeview" => frontend.bc_codeview = true,
            "--run-time-module" => frontend.run_time_module = true,
            "--include" => frontend.includes.push(PathBuf::from(value("--include")?)),
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--arm64" => {
                arm64 = Some(match value("--arm64")?.as_str() {
                    "linux" => llrm_arm64::machine::Platform::Linux,
                    "darwin" => llrm_arm64::machine::Platform::Darwin,
                    other => {
                        return Err(format!(
                            "argument --arm64: invalid choice: '{other}' (choose from 'linux', 'darwin')"
                        ));
                    }
                });
                flags.take(&["-m64".to_owned()], &mut 0)?;
            }
            "--wasm-exceptions" => wasm_exceptions = true,
            "--wasm32" => {
                wasm32 = true;
                flags.take(&["-m32".to_owned()], &mut 0)?;
            }
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let source = source.ok_or("the following arguments are required: source")?;
    frontend.float_store = flags.float_store;
    frontend.checked_arrays = flags.sanitize.bounds;
    frontend.checked_division = flags.sanitize.integer_divide_by_zero;
    frontend.debug = flags.debug;
    frontend.checked_overflow = flags.sanitize.signed_integer_overflow;
    frontend.checked_stack = flags.sanitize.stack;
    let bound = llrm_driver::target(&flags, None)?;
    frontend.segment_bytes = bound.target.layout().segment_bytes();
    // The bytes of the pointer that is the near space, and of the one that is
    // the far: a target with one space has the same for both.
    let spaces = &bound.target.layout().spaces;
    let width = |space: u32| spaces.unmarked.keys().copied().filter(|bytes| spaces.unmarked(*bytes) == Ok(space)).max();
    frontend.near_bytes = width(spaces.near).map_or(0, |bytes| bytes as usize);
    frontend.far_bytes = width(spaces.far).map_or(frontend.near_bytes, |bytes| bytes as usize);
    // `--arm64` and `--wasm32` lower through an x86 target's mid-end, but the
    // object is the host target's: its datalayout says whether it has `f80`.
    let object_layout = match (arm64, wasm32) {
        (Some(platform), _) => platform.datalayout().to_owned(),
        (None, true) => llrm_wasm::driver::datalayout(llrm_wasm::Width::W32),
        (None, false) => bound.target.layout().datalayout.clone(),
    };
    frontend.without_extended_float = !llrm_target::layout::states_extended_float(&object_layout);
    let codegen = llrm_codegen::driver::Options {
        spills_extended: true,
        wasm_exceptions: wasm32 && wasm_exceptions,
        ..bound.options(&flags, flags.machine(&*bound.target, bound.target.machine().with_stack_in_data())?)
    };
    // A host (ELF, Mach-O) has no handler of the runtime's for the fault.
    // Where the OS layer's runtime installs a handler that reports error 11
    // for the divide fault (`divide_fault_error`), a divide's overflow needs
    // no check of ours; a host's layer is the POSIX one.
    let layer = if is_hosted(&codegen.object_format) {
        Some(llrm_target::os::POSIX_LAYER)
    } else {
        bound.target.os_layer().map(|layer| layer.text)
    };
    frontend.idiv_overflow_traps = layer.and_then(llrm_target::os::divide_fault_error) == Some(11);
    Ok(Arguments { source, frontend, dump_hir, flags, dump, codegen, arm64, wasm32 })
}

/// Whether the object format is a host's: an ELF or Mach-O object, whatever
/// the target or its pointer width. An OMF object, or none, is the PC's.
fn is_hosted(object_format: &str) -> bool {
    object_format != "omf" && !object_format.is_empty()
}

pub fn main(argv: &[String]) -> i32 {
    let args = match llrm_codegen::support::debug::timed("parse args", || parse_args(argv)) {
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
        let mut program = llrm_codegen::support::debug::timed("frontend", || {
            parsed(&args.source, &args.frontend, args.dump_hir.as_deref())
        })
        .map_err(|error| error.0)?;
        // A host links the runtime as C: an ELF object calls it by the target's
        // C family where there is one (x86-32's default is Open
        // Watcom's registers, which no C compiler for Linux speaks).
        let mut codegen = args.codegen.clone();
        if codegen.abi.is_none()
            && codegen.object_format == "elf"
            && codegen.arch.calling().chosen(Some("sysv")).is_ok()
        {
            codegen.abi = Some("sysv".to_owned());
        }
        let args = Arguments { codegen, ..args };
        // a host (an ELF or Mach-O object) has no memory at the PC's low
        // addresses: the object format states it, whatever the pointer width
        program.promises.hosted = args.wasm32 || is_hosted(&args.codegen.object_format);
        // The Microsoft runtimes are segmented code with far pointers: on a
        // flat target the object only failed later, "overlapping or
        // pre-added relocations".
        if !program.runtime.calls_natively() && args.codegen.arch.object().code_model().flat() {
            return Err(format!(
                "the {} runtime is segmented 16-bit code: build for a 32-bit target with -fqb-runtime=llrm",
                args.frontend.runtime
            ));
        }
        if program.runtime.calls_natively() {
            // the `-mabi=` family's convention, else the target's default
            let native = args.codegen.arch.calling().chosen(args.codegen.abi.as_deref())?;
            driver::natively_called(&mut program, native.cc.as_deref());
        }
        if args.flags.assembly {
            let module = compile::assembled(&program, None, &args.codegen).map_err(|error| error.to_string())?;
            let output = args.flags.output.clone().unwrap_or_else(|| args.source.with_extension("asm"));
            llrm_codegen::driver::flags::write_output(&output, llrm_codegen::driver::basic::text(&module)?)
                .map_err(|error| error.to_string())?;
        } else if let (Some(platform), Some(output)) = (args.arm64, &args.flags.output) {
            let bytes = super::arm64::object_bytes(&program, &args.source, &args.codegen, platform)
                .map_err(|error| error.to_string())?;
            llrm_codegen::driver::flags::write_output(output, bytes).map_err(|error| error.to_string())?;
        } else if let (true, Some(output)) = (args.wasm32, &args.flags.output) {
            let bytes = super::wasm32::object_bytes(&program, &args.source, &args.codegen)
                .map_err(|error| error.to_string())?;
            llrm_codegen::driver::flags::write_output(output, bytes).map_err(|error| error.to_string())?;
        } else if let Some(output) = &args.flags.output {
            let bytes = compile::object_bytes(&program, &args.source, None, &args.codegen)
                .map_err(|error| error.to_string())?;
            llrm_codegen::support::debug::timed("write output", || {
                llrm_codegen::driver::flags::write_output(output, bytes)
            })
            .map_err(|error| error.to_string())?;
        } else if args.dump.is_none() {
            print!("{}", codec::encode(&program, None).map_err(|error| error.0)?);
        }
        llrm_codegen::support::debug::timed("drop program", || drop(program));
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

#[cfg(test)]
mod tests {
    use super::{is_hosted, parse_args};

    /// `--arm64` and a 64-bit pointer once decided hosting (`arm64.is_none()`,
    /// `bits == 64`): an arm64 Mach-O object ran the PC-memory path of a
    /// bare-metal target. The object format decides, and only it.
    #[test]
    fn an_elf_or_macho_object_is_hosted_and_an_omf_or_no_object_is_not() {
        assert!(is_hosted("elf64") && is_hosted("macho") && is_hosted("elf"));
        assert!(!is_hosted("omf") && !is_hosted(""));
    }

    fn runtime(argv: &[&str]) -> String {
        let argv: Vec<String> = argv.iter().map(|one| one.to_string()).collect();
        parse_args(&argv).expect("parses").frontend.runtime
    }

    fn float_stores(argv: &[&str]) -> (bool, bool) {
        let argv: Vec<String> = argv.iter().map(|one| one.to_string()).collect();
        let parsed = parse_args(&argv).expect("parses");
        (parsed.frontend.float_store, parsed.flags.float_store)
    }

    /// The default was BC's rounding on every store, which cost fpbench/bas
    /// 28% of its clocks against main; the default is C's fast mode.
    /// nbody_single/bas still prints BC's 86016 under it (bench.py's output
    /// check fails the row otherwise).
    #[test]
    fn a_single_assignment_is_not_rounded_unless_standard_excess_precision_is_asked() {
        assert_eq!(float_stores(&["p.bas"]), (false, false));
        assert_eq!(float_stores(&["p.bas", "-fexcess-precision=fast"]), (false, false));
        assert_eq!(float_stores(&["p.bas", "-fexcess-precision=standard"]), (true, true));
    }

    /// llrm-qb called the Microsoft VB-DOS runtime unless told otherwise; the
    /// default is llrm's.
    #[test]
    fn the_runtime_is_llrms_unless_a_microsoft_one_is_named() {
        assert_eq!(runtime(&["p.bas"]), "llrm");
        assert_eq!(runtime(&["p.bas", "--runtime", "qb45"]), "qb45");
        assert_eq!(runtime(&["p.bas", "-fqb-runtime=pds71"]), "pds71");
    }
}
