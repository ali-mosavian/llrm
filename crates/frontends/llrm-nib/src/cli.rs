//! `tools/modernexe.py`'s compile, to an object rather than a linked
//! executable, plus `tools/modernstages.py`'s `--dump DIR`.
//!
//! ```text
//! llrm-nib SOURCE [--entry ENTRY] [--dump DIR] [--procedure-segments] [--used-by OBJ]... [--unchecked-bounds] [OPTIONS]
//! ```
//!
//! OPTIONS are gcc's, as `llrm_core::driver::flags` takes them. Without
//! `-o`, the object, or with `-S` the assembly, goes beside the source
//! unless `--dump` is given.
//! `--procedure-segments` gives each procedure a code segment, all of one
//! name, for a linker that drops unreferenced ones (jwlink's `option
//! eliminate`); Microsoft LINK wants each name defined once. With
//! `--used-by`, only the exports those objects name stay exported, and the
//! rest is dropped with whatever only they call.

use std::path::PathBuf;

use llrm_core::backend::masm;
use llrm_core::backend::objbuild::CodeLayout;
use llrm_core::driver::{
    self as codegen,
    flags::{self, Flags},
};

use super::compile as nib;
use super::driver;
use super::nibstages;

fn usage() -> String {
    format!(
        "usage: llrm-nib [-h] [--entry ENTRY] [--dump DUMP] [--procedure-segments] [--used-by OBJ]... [--unchecked-bounds] {} source",
        flags::USAGE
    )
}

struct Arguments {
    source: PathBuf,
    flags: Flags,
    entry: String,
    dump: Option<PathBuf>,
    layout: CodeLayout,
    used_by: Vec<PathBuf>,
    frontend: super::Frontend,
    /// The target, the built-in DOS on the target's default CPU unless
    /// `--machine` names another, and the pipeline.
    codegen: codegen::Options,
    /// `--os-layer FIELD`: print a field of the target's OS layer instead of
    /// compiling.
    os_layer: Option<Result<String, String>>,
    /// `--declare h|bi|inc`: print the declarations of the program's exports
    /// instead of compiling.
    declare: Option<super::declarations::Language>,
}

fn parse_args(argv: &[String]) -> Result<Arguments, String> {
    let (mut source, mut flags, mut entry, mut dump) = (None, Flags::default(), "main".to_owned(), None);
    let mut layout = CodeLayout::OneSegment;
    let mut used_by = Vec::new();
    let mut os_layer = None;
    let mut declare = None;
    let (mut warn_target_width, mut unchecked_bounds) = (true, false);
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
            "--declare" => {
                declare = Some(
                    super::declarations::Language::named(&value("--declare")?).ok_or("--declare takes h, bi or inc")?,
                )
            }
            "--os-layer" => os_layer = Some(value("--os-layer")?),
            "-Wno-target-width" => warn_target_width = false,
            "--used-by" => used_by.push(PathBuf::from(value("--used-by")?)),
            "--unchecked-bounds" => unchecked_bounds = true,
            _ if flag.starts_with('-') && flag.len() > 1 => return Err(format!("unrecognized arguments: {argument}")),
            _ if source.is_none() => source = Some(PathBuf::from(argument)),
            _ => return Err(format!("unrecognized arguments: {argument}")),
        }
        at += 1;
    }
    let field = os_layer;
    let source = match (source, &field) {
        (Some(source), _) => source,
        (None, Some(_)) => PathBuf::new(),
        (None, None) => return Err("the following arguments are required: source".to_owned()),
    };
    let bound = llrm_driver::target(&flags, None)?;
    let frontend = super::Frontend {
        debug: flags.debug,
        checked_stack: flags.sanitize.stack,
        unchecked_bounds,
        warn_target_width,
        symbols: super::Frontend::symbols_for(&*bound.target, flags.format(&*bound.target)?.name()),
        native_name: flags.convention(&*bound.target)?.name.clone(),
        ..super::Frontend::for_target(&*bound.target)?
    };
    let codegen = bound.options(&flags, flags.machine(&*bound.target, nib::machine(&*bound.target, &frontend.os))?);
    let os_layer = field.map(|field| {
        bound
            .target
            .os_layer()
            .ok_or_else(|| "this target has no OS layer".to_owned())
            .and_then(
                |layer| layer.report(&bound.target.runtime("nib").ok_or("this target has no Nib runtime")?, &field),
            )
    });
    Ok(Arguments { source, flags, entry, dump, layout, used_by, frontend, codegen, os_layer, declare })
}

/// The frontend for the target `-m<N>` among `arguments` names, those arguments
/// taken out of them: for the tools that run or declare a program and take no
/// other target flag (`-mabi=` too: it says what `"c"` is).
pub fn frontend_with_mode(arguments: &mut Vec<String>) -> Result<super::Frontend, String> {
    let mut flags = llrm_core::driver::flags::Flags::default();
    let mut rest = Vec::new();
    for argument in std::mem::take(arguments) {
        if llrm_core::driver::flags::Flags::mode_flag(&argument).is_some() || argument.starts_with("-mabi=") {
            flags.take(&[argument], &mut 0)?;
        } else {
            rest.push(argument);
        }
    }
    *arguments = rest;
    let bound = llrm_driver::target(&flags, None)?;
    let format = flags.format(&*bound.target)?.name();
    Ok(super::Frontend {
        symbols: super::Frontend::symbols_for(&*bound.target, format),
        native_name: flags.convention(&*bound.target)?.name.clone(),
        ..super::Frontend::for_target(&*bound.target)?
    })
}

/// The symbols `objects` import.
fn used(objects: &[PathBuf]) -> Result<std::collections::BTreeSet<String>, String> {
    let mut names = std::collections::BTreeSet::new();
    for object in objects {
        let bytes = std::fs::read(object).map_err(|error| format!("{}: {error}", object.display()))?;
        let records =
            llrm_core::objectfile::omf::parse(&bytes).map_err(|error| format!("{}: {error:?}", object.display()))?;
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
    if let Some(report) = &args.os_layer {
        return match report {
            Ok(text) => {
                println!("{}", text.trim_end());
                0
            }
            Err(message) => {
                eprintln!("llrm-nib: error: {message}");
                2
            }
        };
    }
    if let Some(language) = args.declare {
        return match super::declare_file(&args.source, language, &args.frontend) {
            Ok(text) => {
                print!("{text}");
                0
            }
            Err((path, error)) => {
                eprintln!("{}", driver::refused(&path, &error).0);
                1
            }
        };
    }
    let result = (|| -> Result<(), String> {
        if let Some(dump) = &args.dump {
            nibstages::dumped(&args.source, dump, &args.frontend, &args.codegen, &args.entry)?;
        }
        let output = match (&args.flags.output, &args.dump) {
            (Some(output), _) => output.clone(),
            (None, None) => args.source.with_extension(if args.flags.assembly { "asm" } else { "obj" }),
            (None, Some(_)) => return Ok(()),
        };
        let mut program =
            llrm_core::support::debug::timed("frontend", || driver::parsed(&args.source, &args.frontend, None))
                .map_err(|error| error.0)?;
        for (file, warning) in args.frontend.reported.borrow().iter() {
            eprintln!("{}", driver::refused(file, warning).0);
        }
        if !args.used_by.is_empty() {
            nib::keep_exports(&mut program, &used(&args.used_by)?);
        }
        // A library cut to what some objects name has no entry to want, even
        // when the cut leaves it no export.
        let module = if args.used_by.is_empty() {
            nib::assembled(&program, &args.entry, &args.codegen, &args.frontend.os)?
        } else {
            nib::assembled_library(&program, &args.codegen, &args.frontend.os)?
        };
        let bytes = if args.flags.assembly {
            masm::text(&module).map_err(|error| error.to_string())?.into_bytes()
        } else {
            nib::object(&module, &args.source, args.layout, args.flags.format(&*args.codegen.arch)?)?
        };
        llrm_core::support::debug::timed("write output", || llrm_core::driver::flags::write_output(&output, &bytes))
            .map_err(|error| error.to_string())?;
        // Standard output is the output's, where it is asked for.
        if output != std::path::Path::new("-") {
            println!("{} ({} bytes)", output.display(), bytes.len());
        }
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

#[cfg(test)]
mod tests {
    use llrm_target::Target;

    /// M16 without its Nib runtime: what a target not yet given one is.
    struct Bare(llrm_x86_m16::M16);

    impl Target for Bare {
        fn name(&self) -> &'static str {
            self.0.name()
        }
        fn machine(&self) -> llrm_core::abi::machine::Machine {
            self.0.machine()
        }
        fn cpus(&self) -> &'static [&'static str] {
            self.0.cpus()
        }
        fn registers(&self) -> &'static llrm_core::backend::registerinfo::Info {
            self.0.registers()
        }
        fn march(
            &self,
            name: &str,
        ) -> Option<&'static str> {
            self.0.march(name)
        }
        fn marches(&self) -> Vec<&'static str> {
            self.0.marches()
        }
        fn layout(&self) -> llrm_target::layout::Layout {
            self.0.layout()
        }
        fn stack_slot_bytes(&self) -> i64 {
            self.0.stack_slot_bytes()
        }
        fn frame_register(&self) -> iced_x86::Register {
            self.0.frame_register()
        }
        fn first_argument_offset(
            &self,
            far: bool,
        ) -> i64 {
            self.0.first_argument_offset(far)
        }
        fn return_address_bytes(
            &self,
            far: bool,
        ) -> i64 {
            self.0.return_address_bytes(far)
        }
        fn results(
            &self,
            width: u32,
        ) -> Vec<iced_x86::Register> {
            self.0.results(width)
        }
        fn stack_pointer(&self) -> iced_x86::Register {
            self.0.stack_pointer()
        }
        fn callee_saved(&self) -> Vec<(iced_x86::Register, iced_x86::Register)> {
            self.0.callee_saved()
        }
        fn cpu_table(
            &self,
            name: &str,
        ) -> Option<llrm_target::timings::CpuTable> {
            self.0.cpu_table(name)
        }
        fn forms_text(&self) -> String {
            self.0.forms_text()
        }
        fn registers_text(&self) -> String {
            self.0.registers_text()
        }
        fn operand_bytes(&self) -> i64 {
            self.0.operand_bytes()
        }
        fn default_cpu(&self) -> &'static str {
            self.0.default_cpu()
        }
        fn operation_costs(
            &self,
            price: &dyn Fn(&str) -> i64,
            prefix: i64,
        ) -> llrm_mir::target::OperationCosts {
            self.0.operation_costs(price, prefix)
        }
        fn register_capacity(&self) -> i64 {
            self.0.register_capacity()
        }
        fn float_stack(&self) -> usize {
            self.0.float_stack()
        }
        fn address_forms(
            &self,
            costs: &llrm_mir::target::OperationCosts,
            address_stall: i64,
        ) -> Vec<llrm_mir::target::AddressForm> {
            self.0.address_forms(costs, address_stall)
        }
        fn cost_model(&self) -> llrm_target::CostModel {
            self.0.cost_model()
        }
        fn calling(&self) -> &'static llrm_target::calling::Calling {
            self.0.calling()
        }
        fn conventions(&self) -> &'static [&'static str] {
            self.0.conventions()
        }
        fn object(&self) -> llrm_target::object::ObjectFormat {
            self.0.object()
        }
    }

    /// `-m` was a list the frontend kept by hand; a target without a Nib
    /// runtime is refused by saying so, whatever else it is.
    #[test]
    fn a_target_without_a_nib_runtime_is_refused_by_that_message() {
        let error = super::super::Os::for_target(&Bare(llrm_x86_m16::M16)).expect_err("refused");
        assert_eq!(error, "target x86-m16 has no Nib runtime");
        assert!(super::super::Os::for_target(&llrm_x86_m16::M16).is_ok());
    }
}
