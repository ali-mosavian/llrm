use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use qbfront::Dialect;

fn main() -> ExitCode {
    let mut flags = llrm_core::driver::flags::Flags::default();
    let argv: Vec<String> = env::args().skip(1).collect();
    let mut taken = 0;
    let mut rest = Vec::new();
    while taken < argv.len() {
        // The target flags are the shared parser's: -m16, -m32, -m64.
        if llrm_core::driver::flags::Flags::mode_flag(&argv[taken]).is_some() {
            if let Err(why) = flags.take(&argv, &mut taken) {
                eprintln!("qbfront: {why}");
                return ExitCode::from(2);
            }
        } else {
            rest.push(argv[taken].clone());
        }
        taken += 1;
    }
    let segment_bytes = match llrm_driver::planned(
        &flags,
        Some(&["x86-m16"]),
        &[("x86-m32", "https://github.com/ali-mosavian/llrm/issues/1160")],
    ) {
        Ok(bound) => bound.target.layout().segment_bytes(),
        Err(why) => {
            eprintln!("qbfront: {why}");
            return ExitCode::from(2);
        }
    };
    let mut arguments = rest.into_iter();
    let mut dialect = Dialect::VbDos;
    let mut runtime = "vbdos".to_string();
    let mut row_major = false;
    let mut huge_arrays = false;
    let mut checked_arrays = false;
    let mut checked_division = false;
    let mut debug = false;
    let mut checked_overflow = false;
    let mut mbf = false;
    let mut alternate_math = false;
    let mut whole_program = false;
    let mut array_merging = false;
    let mut runtime_frames = false;
    let mut error_lines = false;
    let mut syntax = false;
    let mut include_dirs = Vec::new();
    let mut dump_source = None;
    let mut input = None;
    while let Some(argument) = arguments.next() {
        if argument == "--dialect" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --dialect requires a value");
                return ExitCode::from(2);
            };
            let Some(found) = Dialect::parse(&value) else {
                eprintln!("qbfront: unknown dialect {value:?}");
                return ExitCode::from(2);
            };
            dialect = found;
        } else if argument == "--runtime" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --runtime requires a value");
                return ExitCode::from(2);
            };
            if !matches!(value.as_str(), "qb45" | "pds71" | "vbdos") {
                eprintln!("qbfront: unknown runtime {value:?}");
                return ExitCode::from(2);
            }
            runtime = value;
        } else if argument == "--syntax" {
            syntax = true;
        } else if argument == "--huge-arrays" {
            huge_arrays = true;
        } else if argument == "--checked-arrays" {
            checked_arrays = true;
        } else if argument == "-g" {
            debug = true;
        } else if argument == "--checked-division" {
            checked_division = true;
        } else if argument == "--checked-overflow" {
            checked_overflow = true;
        } else if argument == "--mbf" {
            mbf = true;
        } else if argument == "--alternate-math" {
            alternate_math = true;
        } else if argument == "--whole-program" {
            whole_program = true;
        } else if argument == "--array-merging" {
            array_merging = true;
        } else if argument == "--own-frames" {
            // Accepted: own frames are the default.
        } else if argument == "--runtime-frames" {
            runtime_frames = true;
        } else if argument == "--error-lines" {
            error_lines = true;
        } else if argument == "--array-order" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --array-order requires column-major or row-major");
                return ExitCode::from(2);
            };
            match value.as_str() {
                "column-major" => row_major = false,
                "row-major" => row_major = true,
                _ => {
                    eprintln!("qbfront: unknown array order {value:?}");
                    return ExitCode::from(2);
                }
            }
        } else if argument == "--include" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --include requires a directory");
                return ExitCode::from(2);
            };
            include_dirs.push(PathBuf::from(value));
        } else if argument == "--dump-source" {
            let Some(value) = arguments.next() else {
                eprintln!("qbfront: --dump-source requires a file");
                return ExitCode::from(2);
            };
            dump_source = Some(PathBuf::from(value));
        } else if input.replace(argument).is_some() {
            eprintln!("qbfront: expected one input file");
            return ExitCode::from(2);
        }
    }
    let Some(input) = input else {
        eprintln!(
            "usage: qbfront [-m16] [--dialect PROFILE] [--runtime PROFILE] [--array-order column-major|row-major] [--huge-arrays] [--checked-arrays] [--checked-division] [--checked-overflow] [--whole-program] [--array-merging] [--runtime-frames] [--error-lines] [-g] [--include DIR] [--syntax] FILE"
        );
        return ExitCode::from(2);
    };
    let args = qbfront::driver::Args {
        dialect,
        runtime,
        options: qbfront::semantic::Options {
            row_major,
            huge_arrays,
            checked_arrays,
            checked_division,
            checked_overflow,
            mbf,
            alternate_math,
            whole_program,
            array_merging,
            runtime_frames,
            error_lines,
            segment_bytes,
        },
        debug,
        syntax,
        include_dirs,
        dump_source,
        input: PathBuf::from(input),
    };
    match qbfront::driver::compile(&args) {
        Ok(compiled) => {
            for warning in &compiled.warnings {
                eprintln!("{warning}");
            }
            print!("{}", compiled.text);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
