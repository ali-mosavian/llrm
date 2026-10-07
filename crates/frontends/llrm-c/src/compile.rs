//! Port of `qbopt/cfront/compile.py`: C through Open Watcom's front end and
//! the backend, to an object or jwasm source.
//!
//! ```text
//! llrm-c pal.cgs -o pal.obj [--dump DIR]
//! ```

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use super::{hir, stream, translate};

use llrm_core::backend::{
    masm, objbuild,
};
use llrm_core::driver::flags::{self, Flags};

#[derive(Debug)]
pub enum CompileError {
    Unsupported(hir::Unsupported),
    Io(std::io::Error),
    Emission(objbuild::Error),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Emission(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<hir::Unsupported> for CompileError {
    fn from(error: hir::Unsupported) -> Self {
        Self::Unsupported(error)
    }
}

impl From<String> for CompileError {
    fn from(error: String) -> Self {
        Self::Unsupported(hir::Unsupported(error))
    }
}

impl From<objbuild::Error> for CompileError {
    fn from(error: objbuild::Error) -> Self {
        Self::Emission(error)
    }
}

impl From<masm::Unprintable> for CompileError {
    fn from(error: masm::Unprintable) -> Self {
        Self::Emission(objbuild::Error::Unprintable(error))
    }
}

impl From<std::io::Error> for CompileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// C through the rich MIR: translated to HIR, then compiled by the driver,
/// which writes each stage to `dump` or where `LLRM_MIR_STAGES` names.
pub fn selected(text: &str, module: &str, dump: Option<&Path>, codegen: &llrm_core::driver::Options) -> Result<masm::Module, CompileError> {
    selected_checking(text, module, dump, codegen, None)
}

/// What C's runtime says of its stack on `target`: the `stack.toml` its description names, the OS layer's
/// stack limit and Open Watcom's `__STKOVERFLOW`, which a checked function compares with and enters.
pub fn stack_check(target: &dyn llrm_target::Target) -> llrm_core::hir::model::StackCheck {
    let description = target.runtime("c").expect("a C target has a C runtime");
    let name = description.string("stack").expect("c.toml names its stack check");
    stack_check_of(description.file(&name).expect("the stack check is shipped"))
}

fn stack_check_of(text: &str) -> llrm_core::hir::model::StackCheck {
    let row = toml::Value::Table(text.parse().expect("stack.toml parses"));
    llrm_core::hir::model::StackCheck::from_toml(&row).expect("stack.toml states a stack check")
}

/// [`selected`], each function checking its stack as `stack_check` says (`-fsanitize=stack`).
pub fn selected_checking(text: &str, module: &str, dump: Option<&Path>, codegen: &llrm_core::driver::Options, stack_check: Option<llrm_core::hir::model::StackCheck>) -> Result<masm::Module, CompileError> {
    let program = llrm_core::support::debug::timed("frontend translate", || -> Result<_, CompileError> {
        let mut unit = hir::unit(&stream::parse(text))?;
        let profile = Profile::for_abi(&*codegen.arch, codegen.abi.as_deref()).map_err(hir::Unsupported)?;
        let calling = codegen.arch.calling();
        let cc_of = |name: &str| calling.named(name).and_then(|one| one.cc.clone()).ok_or_else(|| hir::Unsupported(format!("calling.toml has no {name} with a cc")));
        unit.cdecl_cc = Some(cc_of(&profile.cdecl)?).filter(|cc| cc != "cdecl");
        unit.registers_cc = Some(cc_of(&profile.registers)?).filter(|cc| cc != "cdecl");
        unit.entry = profile.entry.clone();
        unit.default_registers = profile.default_registers.clone();
        unit.entry_cc = calling.named(&profile.entry_convention).and_then(|one| one.cc.clone()).ok_or_else(|| hir::Unsupported(format!("calling.toml has no {} with a cc", profile.entry_convention)))?;
        unit.decorate(calling, codegen.object_format);
        // The front end was picked by the shim's flat flag; the target says what flat is.
        if unit.flat != codegen.arch.layout().spaces.far_is_near() {
            return Err(hir::Unsupported(format!("the front end is {} but target {} is {}", if unit.flat { "flat" } else { "segmented" }, codegen.arch.name(), if unit.flat { "segmented" } else { "flat" })).into());
        }
        let program = translate::program(&unit, module, codegen.arch.calling(), &profile)?;
        for warning in unit.warnings.borrow().iter() {
            eprintln!("{module}: {warning}");
        }
        Ok(llrm_core::hir::model::Program { stack_check, ..program })
    })?;
    let mut options = codegen.clone();
    if let Some(dump) = dump {
        fs::create_dir_all(dump)?;
        options.dump = Some(dump.to_path_buf());
    }
    Ok(llrm_core::driver::compiled(&program, &options)?.pop().expect("one module"))
}

/// `bytes.fromhex(raw)`.
pub(crate) fn hex_bytes(raw: &str) -> Vec<u8> {
    let digits: Vec<char> = raw.chars().filter(|one| !one.is_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let text: String = pair.iter().collect();
            u8::from_str_radix(&text, 16)
                .unwrap_or_else(|_| panic!("ValueError: non-hexadecimal number found in fromhex() arg"))
        })
        .collect()
}

struct Args {
    source: PathBuf,
    /// `--os-layer FIELD`: what the target's OS layer says of C's runtime, printed instead of compiling.
    os_layer: Option<Result<String, String>>,
    flags: Flags,
    dump: Option<PathBuf>,
    include: Vec<String>,
    /// Watcom switches the program opts into, passed to wccq: -oa, -on.
    watcom: Vec<&'static str>,
    /// The target, the built-in DOS on a 386 unless `--machine` names
    /// another, and the pipeline.
    codegen: llrm_core::driver::Options,
}

/// The code-generator stream wccq records for one C file; with `debug`,
/// its debug types and symbols too (-d2).
/// How the target asks Open Watcom's front end to record C: the `[frontend]` of its C runtime
/// description (`runtime/c/<target>/c.toml`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    /// The front end's tree: its CPU, `i86` or `386`.
    pub cpu: String,
    /// The convention whose contract a call to a routine that states none has, by its name in `calling.toml`.
    pub convention: String,
    /// The routine the runtime's start calls, and the convention it calls it in by its name in `calling.toml`.
    pub entry: String,
    pub entry_convention: String,
    /// What it records of a function it passes in its default registers: its own list (`[ff:0]`), which differs by tree.
    pub default_registers: String,
    /// The ABI the front end is asked for: what it is run with, and what its records mean in `calling.toml`'s conventions: one it
    /// records as cdecl, and one it records in its default registers.
    pub flags: Vec<String>,
    pub cdecl: String,
    pub registers: String,
    /// The header it includes first, from the repository root.
    pub header: String,
    /// An interrupt handler's parameters, in order: the registers its frame holds, by name. None where the target has no
    /// interrupt handlers.
    pub interrupt_parameters: Vec<String>,
}

impl Profile {
    /// The target's profile under its default ABI.
    pub fn of(target: &dyn llrm_target::Target) -> Result<Self, String> {
        Self::for_abi(target, None)
    }

    /// The profile under `-mabi=family`, the target's default without.
    pub fn for_abi(target: &dyn llrm_target::Target, family: Option<&str>) -> Result<Self, String> {
        let description = target.runtime("c").ok_or_else(|| format!("target {} has no C runtime", target.name()))?;
        let calling = target.calling();
        calling.chosen(family)?;
        Self::parse(&description.table()?, family.unwrap_or(&calling.default))
    }

    pub fn parse(table: &toml::Table, family: &str) -> Result<Self, String> {
        let frontend = table.get("frontend").and_then(toml::Value::as_table).ok_or("the C runtime description has no [frontend]")?;
        let text = |from: &toml::Table, key: &str| from.get(key).and_then(toml::Value::as_str).map(str::to_owned).ok_or_else(|| format!("frontend.{key} is not a string"));
        let abi = frontend.get("abi").and_then(|one| one.get(family)).and_then(toml::Value::as_table).ok_or_else(|| format!("the C runtime description has no [frontend.abi.{family}]"))?;
        let flags = abi.get("flags").and_then(toml::Value::as_array).ok_or("frontend.abi.flags is not a list")?.iter().map(|one| one.as_str().map(str::to_owned).ok_or("a frontend flag is not a string")).collect::<Result<_, _>>()?;
        let interrupt_parameters = match frontend.get("interrupt_parameters") {
            None => Vec::new(),
            Some(list) => list.as_array().ok_or("frontend.interrupt_parameters is not a list")?.iter().map(|one| one.as_str().map(str::to_owned).ok_or("an interrupt parameter is not a string")).collect::<Result<_, _>>()?,
        };
        Ok(Self { cpu: text(frontend, "watcom_cpu")?, convention: text(frontend, "convention")?, entry: text(frontend, "entry")?, entry_convention: text(frontend, "entry_convention")?, default_registers: text(frontend, "default_registers")?, flags, cdecl: text(abi, "cdecl")?, registers: text(abi, "registers")?, header: text(frontend, "header")?, interrupt_parameters })
    }
}

/// `recorded` for real mode, the target the tests of the recorded streams are for.
#[cfg(test)]
pub fn recorded(source: &Path, includes: &[String], debug: bool, watcom: &[&str]) -> Result<String, hir::Unsupported> {
    recorded_for(source, includes, debug, watcom, &Profile::of(&llrm_x86_m16::M16).expect("real mode has a C front end profile"))
}

/// `source` recorded by the front end of `profile`'s tree, asked as it says.
pub fn recorded_for(source: &Path, includes: &[String], debug: bool, watcom: &[&str], profile: &Profile) -> Result<String, hir::Unsupported> {
    let root = Path::new(env!("LLRM_ROOT"));
    let unbuilt = || hir::Unsupported("llrm was built without the toolchain feature".into());
    let wccq = Path::new(option_env!("LLRM_WCCQ_DIR").ok_or_else(unbuilt)?).join(&profile.cpu).join("wccq");
    let header = format!("-fi={}", root.join(&profile.header).display());
    let failed = |detail: String| hir::Unsupported(format!("wccq failed on {}:\n{detail}", source.display()));
    let scratch = tempfile::tempdir().map_err(|error| failed(error.to_string()))?;
    // What GCC predefines and programs test (`__INT_MAX__`, `__SIZE_TYPE__`, `__BYTE_ORDER__`), as the target's sizes make them.
    let defined = scratch.path().join("predefined.h");
    fs::write(&defined, crate::predefined::header(profile.cpu == "386")).map_err(|error| failed(error.to_string()))?;
    let predefined = format!("-fi={}", defined.display());
    let flags: Vec<&str> = profile.flags.iter().map(String::as_str).chain([header.as_str(), predefined.as_str()]).collect();
    let out = scratch.path().join("unit.cgs");
    let absolute = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let searched = includes.iter().map(|one| format!("-I{}", absolute(Path::new(one)).display()));
    // In the scratch directory, where wccq also leaves its .err file.
    let done = std::process::Command::new(&wccq)
        .args(flags)
        .args(debug.then_some("-d2"))
        .args(watcom)
        .args(searched)
        .arg(format!("-fo={}/unit.obj", scratch.path().display()))
        .arg(absolute(source))
        .env("QBOPT_CG_STREAM", &out)
        .current_dir(scratch.path())
        .output()
        .map_err(|error| failed(error.to_string()))?;
    if !done.status.success() || !out.exists() {
        let text = String::from_utf8_lossy(&done.stdout).into_owned() + &String::from_utf8_lossy(&done.stderr);
        return Err(failed(text));
    }
    fs::read_to_string(&out).map_err(|error| failed(error.to_string()))
}

fn usage() -> String {
    format!("usage: llrm-c [-h] [-I INCLUDE] [--dump DUMP] [--os-layer FIELD] {} source", flags::USAGE)
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let (mut source, mut flags, mut dump) = (None, Flags::default(), None);
    let (mut include, mut watcom) = (Vec::new(), Vec::new());
    let mut os_layer = None;
    let mut at = 0;
    while at < argv.len() {
        if flags.take(argv, &mut at)? {
            at += 1;
            continue;
        }
        let argument = argv[at].as_str();
        let mut value = |name: &str| {
            at += 1;
            argv.get(at).cloned().ok_or(format!("argument {name}: expected one argument"))
        };
        match argument {
            "-I" | "--include" => include.push(value("-I/--include")?),
            "--dump" => dump = Some(PathBuf::from(value("--dump")?)),
            "--os-layer" => os_layer = Some(value("--os-layer")?),
            // Watcom's: relaxed alias checking; relaxed floating point.
            "-oa" => watcom.push("-oa"),
            "-on" => watcom.push("-on"),
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unrecognized arguments: {flag}"));
            }
            path if source.is_none() => source = Some(PathBuf::from(path)),
            extra => return Err(format!("unrecognized arguments: {extra}")),
        }
        at += 1;
    }
    let source = match (source, &os_layer) {
        (Some(source), _) => source,
        (None, Some(_)) => PathBuf::new(),
        (None, None) => return Err("the following arguments are required: source".to_owned()),
    };
    let bound = llrm_driver::target(&flags, Some(&["x86-m16", "x86-m32"]))?;
    let os_layer = os_layer.map(|field| bound.target.os_layer().ok_or_else(|| "this target has no OS layer".to_owned()).and_then(|layer| layer.report(&bound.target.runtime("c").ok_or("this target has no C runtime")?, &field)));
    let machine = flags.machine(&*bound.target, bound.target.machine())?;
    Ok(Args {
        source,
        os_layer,
        dump,
        include,
        watcom,
        codegen: bound.options(&flags, machine),
        flags,
    })
}

/// `main`: exit status 0 on success, 1 on a refusal, 2 on bad arguments.
pub fn main(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{}\nllrm-c: error: {message}", usage());
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
                eprintln!("llrm-c: error: {message}");
                2
            }
        };
    }
    let result = (|| -> Result<(), CompileError> {
        let text = if args.source.extension().and_then(|one| one.to_str()) == Some("cgs") {
            fs::read_to_string(&args.source)?
        } else {
            llrm_core::support::debug::timed("frontend wccq", || {
                // The target's physical addresses reach the program as `PHYSICAL_<NAME>`.
                let defines: Vec<String> = args.codegen.arch.physical_addresses().iter().map(|(name, address)| format!("-dPHYSICAL_{}=0x{address:X}UL", name.to_uppercase())).collect();
                let switches: Vec<&str> = args.watcom.iter().copied().chain(defines.iter().map(String::as_str)).collect();
                recorded_for(&args.source, &args.include, args.flags.debug, &switches, &Profile::for_abi(&*args.codegen.arch, args.codegen.abi.as_deref()).map_err(hir::Unsupported)?)
            })?
        };
        let output = args.flags.output.clone().unwrap_or_else(|| args.source.with_extension("asm"));
        let module = args
            .source
            .file_stem()
            .and_then(|one| one.to_str())
            .unwrap_or_default();
        let format = args.flags.format(&*args.codegen.arch)?;
        let spelled = llrm_core::driver::Options { object_format: format.name(), ..args.codegen.clone() };
        let built = selected_checking(&text, module, args.dump.as_deref(), &spelled, args.flags.sanitize.stack.then(|| stack_check(&*args.codegen.arch)))?;
        let name = args.source.file_name().and_then(|one| one.to_str()).unwrap_or_default();
        if !args.flags.assembly && matches!(output.extension().and_then(|one| one.to_str()).map(str::to_lowercase).as_deref(), Some("obj" | "o")) {
            let bytes = objbuild::written_in(&built, name, objbuild::CodeLayout::OneSegment, format)?;
            llrm_core::support::debug::timed("write output", || fs::write(&output, bytes))?;
        } else {
            fs::write(&output, masm::text(&built)?)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("llrm-c: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::CompileError;

    /// The innermost loop's lines, from its label to its backward branch.
    fn innermost(fixture: &str) -> Vec<String> {
        let path = Path::new(env!("LLRM_ROOT")).join(format!("{}/tests/fixtures/c/{fixture}.cgs", env!("LLRM_ROOT")));
        let text = std::fs::read_to_string(path).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, fixture, None, &llrm_driver::m16_options(machine)).unwrap();
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        let (top, back) = lines
            .iter()
            .enumerate()
            .filter(|(_, one)| one.starts_with('j') && !one.starts_with("jmp"))
            .find_map(|(at, one)| {
                let label = format!("{}:", one.split_whitespace().nth(1)?);
                Some((lines[..at].iter().position(|line| *line == label)?, at))
            })
            .expect("a loop");
        lines[top..back].iter().map(|one| (*one).to_owned()).collect()
    }

    /// `-fsanitize=stack` compares with the word and calls the routine `stack.toml` names, and the C
    /// start-up the tests link defines both: a description naming a word start-up never fills would
    /// compare with zero. The default build checks nothing.
    #[test]
    fn the_stack_check_names_what_the_c_runtime_defines() {
        let root = Path::new(env!("LLRM_ROOT"));
        let check = super::stack_check(&llrm_x86_m16::M16);
        let runtime = |name: &str| std::fs::read_to_string(root.join("runtime").join(name)).unwrap();
        assert!(runtime("shared/dos/m16/os.asm").contains(&format!("public {}", check.limit)) && runtime("shared/dos/m16/start.asm").contains(&format!("mov {}, ax", check.limit)));
        assert!(runtime("c/x86-m16/ext.asm").contains(&format!("public {}", check.handler)));
        let text = std::fs::read_to_string(root.join("tests/fixtures/c/anims.cgs")).unwrap();
        let options = llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() });
        let listing = |check| llrm_core::backend::masm::text(&super::selected_checking(&text, "anims", None, &options, check).unwrap()).unwrap();
        let named = llrm_core::hir::model::StackCheck { limit: "FOO".into(), handler: "BAR".into(), ..check };
        let checked = listing(Some(named));
        assert!(checked.contains("cmp sp, word ptr FOO") && checked.contains("call far ptr BAR") && !checked.contains("_llrm_os_stack_low"), "{checked}");
        assert!(!listing(None).contains("cmp sp"));
    }

    /// The innermost loop's counting: its steps by a constant and its compares.
    fn loop_counting(fixture: &str) -> Vec<String> {
        let constant = |one: &str| one.rsplit(", ").next().is_some_and(|last| last.parse::<i64>().is_ok());
        innermost(fixture)
            .into_iter()
            .filter(|one| {
                one.starts_with("inc ")
                    || one.starts_with("dec ")
                    || one.starts_with("cmp ")
                    || (one.starts_with("add ") || one.starts_with("sub ")) && constant(one)
            })
            .collect()
    }

    /// `dot` indexes `a[i]` and `b[i]`: before strength waited for the other passes to
    /// settle, it kept two pointers and a counter, three steps per iteration.
    #[test]
    fn test_addresses_differing_by_base_share_one_stepped_offset() {
        assert_eq!(loop_counting("dot"), ["add bx, 2"]);
    }

    /// `bytes` indexes by `i` itself, with a bound only known at run time: the
    /// counter never counted to zero, so each iteration compared it with `n` in memory.
    #[test]
    fn test_a_counter_read_only_as_offsets_counts_to_zero() {
        assert_eq!(loop_counting("bytes"), ["inc bx"]);
    }

    /// `from1` reads `a[i]` and `b[i - 1]`: `(i - 1) * 2` was a second root beside
    /// `i * 2`, so each array stepped its own pointer beside a counter.
    #[test]
    fn test_subscripts_of_one_stride_share_one_offset() {
        assert_eq!(loop_counting("from1"), ["add bx, 2"]);
        let loop_ = innermost("from1");
        let two_registers = |one: &String| {
            let inside = one.split_once("ptr [").map_or("", |(_, inside)| inside).as_bytes();
            inside.len() > 4 && inside[2] == b'+' && inside[3].is_ascii_alphabetic()
        };
        assert!(loop_.iter().filter(|one| one.contains("ptr [")).all(two_registers), "{loop_:#?}");
    }

    /// tests/fixtures/c/`path`.cgs's MIR as the front end emits it.
    fn emitted(path: &str) -> String {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{path}.cgs"))).unwrap();
        let program = crate::translate::program(&crate::hir::unit(&crate::stream::parse(&text)).unwrap(), "t", llrm_target::Target::calling(&llrm_x86_m16::M16), &crate::compile::Profile::of(&llrm_x86_m16::M16).unwrap()).unwrap();
        let options = llrm_driver::m16_options(llrm_x86_m16::machine::BUILT_IN.clone());
        let (mir, _) = llrm_core::driver::emitted(&program, &options).unwrap();
        llrm_mir::print::module(&mir.modules[0])
    }

    /// tests/fixtures/c/`path`.cgs compiled.
    fn compiles(path: &str) {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{path}.cgs"))).unwrap();
        let options = llrm_driver::m16_options(llrm_x86_m16::machine::BUILT_IN.clone());
        super::selected(&text, "t", None, &options).unwrap_or_else(|error| panic!("{path}: {error}"));
    }

    /// OW parsed restrict but discarded it before CG; the shim now records it.
    #[test]
    fn test_restrict_reaches_mir_as_distinct_noalias_roots() {
        let text = emitted("tests/test_restrict_reaches_mir_as_distinct_noalias_roots");
        let define = text.lines().find(|line| line.starts_with("define ")).expect("a definition");
        assert_eq!(define.matches("ptr noalias").count(), 3, "{text}");
    }

    /// `ls_animate(&ls, 0.05f)` pushes the single's four bytes; CGFloat was refused.
    #[test]
    fn test_float_moves_as_its_bits() {
        compiles("ls");
    }

    /// `raw` ends in inline code and `return;`-less: its MIR returned nothing,
    /// and DX:AX reached the caller only because nothing was emitted after.
    #[test]
    fn test_value_less_return_returns_what_the_code_left() {
        compiles("inline");
    }

    /// qcport's combat_brush_points stopped at `no scalar width for T51`
    /// while compiling `*target = *center`.
    #[test]
    fn test_aggregate_copy_through_pointers_reaches_mir() {
        compiles("tests/test_aggregate_copy_through_pointers_reaches_mir");
    }

    /// qcport's combat_radius passes a BspVec3 by value; the frontend stopped
    /// at `no scalar width for T51` instead of laying its 12 bytes on the stack.
    #[test]
    fn test_aggregate_argument_is_pushed_by_value() {
        compiles("tests/test_aggregate_argument_is_pushed_by_value");
    }

    /// `name`'s listing of `function`, compiled from tests/fixtures/c/`name`.cgs.
    fn listing_of(name: &str, function: &str) -> String {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{name}.cgs"))).unwrap();
        let options = llrm_driver::m16_options(llrm_x86_m16::machine::BUILT_IN.clone());
        let built = super::selected(&text, name, None, &options).unwrap_or_else(|error| panic!("{error:?}"));
        let listing = llrm_core::backend::masm::text(&built).unwrap();
        let start = listing.find(&format!("{function} proc")).expect("the function");
        listing[start..start + listing[start..].find("endp").unwrap()].to_owned()
    }

    /// Watcom's -oa lets a store through a pointer leave alone every object
    /// the unit never takes the address of: INIT `sw` said so, and llrm-c
    /// read only its debug bits, so `after_store` read `counter` and `seen`
    /// again after `*p = 5`. Without -oa they are read again, as C requires.
    #[test]
    fn test_relaxed_alias_checking_keeps_unaddressed_objects_across_a_store() {
        let reads = |listing: &str, name: &str| listing.lines().filter(|one| one.contains(&format!("ptr {name}"))).count();
        let strict = listing_of("strictalias", "_after_store");
        assert_eq!((reads(&strict, "_counter"), reads(&strict, "_seen")), (2, 2), "premise: read again without -oa\n{strict}");
        let relaxed = listing_of("relaxalias", "_after_store");
        assert_eq!((reads(&relaxed, "_counter"), reads(&relaxed, "_seen")), (1, 1), "{relaxed}");
    }

    /// A const object is read once across a call or a store: writing it is
    /// undefined, wherever its address went. An extern const was a plain
    /// global, and no analysis read `constant` as unwritable, so each was
    /// read again after `ext()`, `keep(tbl)` or `*p = 1`.
    #[test]
    fn test_a_const_object_is_read_once_across_a_call_or_a_store() {
        let reads = |function: &str, name: &str| listing_of("constobj", function).lines().filter(|one| one.contains(&format!("ptr {name}"))).count();
        assert_eq!(reads("_ext_plain", "_ev"), 2, "premise: a plain extern is read again after a call");
        assert_eq!(reads("_ext_const", "_ek"), 1);
        assert_eq!(reads("_table", "_tbl"), 1);
        assert_eq!(reads("_escaped", "_tbl"), 1);
        assert_eq!(reads("_stored", "_tbl"), 1);
    }

    /// A callee taking arguments in registers: the raise pushed them anyway,
    /// and ls linked against `strlen_`, Watcom's register-convention strlen.
    #[test]
    fn test_register_convention_is_refused() {
        let path = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/regs.cgs");
        match super::selected(&std::fs::read_to_string(path).unwrap(), "regs", None, &llrm_driver::m16_options(llrm_x86_m16::machine::BUILT_IN.clone())) {
            Err(CompileError::Unsupported(refused)) => {
                assert!(refused.to_string().contains("_twice has a register calling convention"), "{refused}");
            }
            _ => panic!("a register convention was compiled as a stack one"),
        }
    }

    /// toolchain/owshim/build.sh hardcoded macOS ARM64's defines and clang, so no wccq
    /// could be built on any other host and llrm-c refused every C file.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_wccq_built_here_records_the_committed_stream() {
        let root = Path::new(env!("LLRM_ROOT"));
        let source = root.join("tests/fixtures/c/halve.c");
        let without_path = |text: &str| text.lines().filter(|line| !line.contains("DBSrcFile")).collect::<Vec<_>>().join("\n");
        let recorded = super::recorded(&source, &[], false, &[]).expect("wccq records halve.c");
        let committed = std::fs::read_to_string(root.join("tests/fixtures/c/halve.cgs")).unwrap();
        assert_eq!(without_path(&recorded), without_path(&committed));
    }

    /// A long double global got its initializer as a double, 8 bytes, while
    /// code loads it as 10 bytes: `gld` read 1.07e-49 and took two bytes of `after`.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_long_double_initializer_is_ten_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("probe.c");
        std::fs::write(&source, "long double gld = 3.5;\nshort after = 7;\nlong double get(void) { return gld; }\n").unwrap();
        let recorded = super::recorded(&source, &[], false, &[]).expect("wccq records probe.c");
        let data: Vec<&str> = recorded.lines().filter(|line| line.contains("DGBytes")).collect();
        assert_eq!(data, ["- DGBytes 10 00000000000000e00040"], "{recorded}");
    }

    /// The front end, its switches and its header were `if flat` here and a pair of env names in build.rs: a
    /// target now says them in its C runtime description, and a target with none is refused.
    #[test]
    fn test_a_target_asks_the_front_end_as_its_description_says() {
        let real = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        let flat = super::Profile::of(&llrm_x86_m32::M32).unwrap();
        assert_eq!((real.cpu.as_str(), real.header.as_str(), real.flags.contains(&"-mm".to_owned())), ("i86", "crates/frontends/llrm-c/src/borland.h", true));
        assert_eq!((flat.cpu.as_str(), flat.header.as_str(), flat.flags.contains(&"-zp4".to_owned())), ("386", "crates/frontends/llrm-c/src/flat.h", true));
        let none: toml::Table = "stack = \"stack.toml\"\n".parse().unwrap();
        assert_eq!(super::Profile::parse(&none, "x").unwrap_err(), "the C runtime description has no [frontend]");
    }

    /// The 386 front end records `sum.c` as the committed flat stream: `flat=1` in INIT, int and
    /// pointers 4 bytes. (Its source path is the machine's, so that line is not compared.)
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_the_386_wccq_records_the_committed_flat_stream() {
        let dir = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32");
        let recorded = super::recorded_for(&dir.join("sum.c"), &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap()).expect("the 386 wccq records sum.c");
        let committed = std::fs::read_to_string(dir.join("sum.cgs")).unwrap();
        let body = |text: &str| text.lines().filter(|line| !line.contains("DBSrcFile")).map(str::to_owned).collect::<Vec<_>>();
        assert!(recorded.starts_with("INIT ") && recorded.lines().next().unwrap().ends_with(" flat=1"), "{recorded}");
        assert_eq!(body(&recorded), body(&committed));
    }

    /// The loop in `function` that reads `marker`, from its label to its backward branch, as the rich route selects it.
    fn selected_loop(fixture: &str, function: &str, marker: &str) -> Vec<String> {
        let path = Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs"));
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&std::fs::read_to_string(path).unwrap(), fixture, None, &llrm_driver::m16_options(machine)).unwrap();
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        let lines: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        lines
            .iter()
            .enumerate()
            .filter(|(_, one)| one.starts_with('j') && !one.starts_with("jmp"))
            .filter_map(|(at, one)| {
                let label = format!("{}:", one.split_whitespace().nth(1)?);
                let top = lines[..at].iter().position(|line| *line == label)?;
                Some(lines[top..=at].iter().map(|one| (*one).to_owned()).collect::<Vec<_>>())
            })
            .find(|body| body.iter().any(|one| one.starts_with(marker)))
            .expect("the loop")
    }

    /// The instructions of `function` as the rich route selects them.
    fn selected_body(fixture: &str, function: &str) -> Vec<String> {
        let path = Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs"));
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&std::fs::read_to_string(path).unwrap(), fixture, None, &llrm_driver::m16_options(machine)).unwrap();
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).filter(|one| !one.ends_with(':')).map(str::to_owned).collect()
    }

    /// `function` of tests/fixtures/c/watcall16.cgs under `-mabi=watcom`: Open Watcom's 16-bit register convention.
    fn watcall16(function: &str) -> Vec<String> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/watcall16.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let options = llrm_core::driver::Options { abi: Some("watcom".to_owned()), ..llrm_driver::m16_options(machine) };
        let built = super::selected(&text, "watcall16", None, &options).unwrap();
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).map(str::to_owned).collect()
    }

    /// `wcc -ecw` took the first four words in AX, DX, BX and CX and the rest from the stack, lowest first, popped with
    /// `retf 4`; the stack convention read all six from [bp+6]...
    #[test]
    fn test_m16_watcom_words_arrive_in_ax_dx_bx_cx_and_the_callee_pops_the_rest() {
        let body = watcall16("six_");
        assert_eq!(body, ["push bp", "mov bp, sp", "L0_0:", "sub ax, dx", "imul bx, cx", "add ax, bx", "sub ax, word ptr [bp+6]", "add ax, word ptr [bp+8]", "pop bp", "retf 4"]);
    }

    /// A long or a far pointer takes a pair: `mixed(int a, long b, int c)` has a in AX, b in BX:CX and c in DX, as `wcc` has it
    /// (DX was passed over by the pair and stayed free); a far pointer is its offset in AX and its segment in DX.
    #[test]
    fn test_m16_watcom_a_long_and_a_far_pointer_take_a_pair_and_a_passed_over_register_stays_free() {
        let body = watcall16("mixed_");
        assert!(body.contains(&"movzx esi, bx".to_owned()) && body.contains(&"movzx ebx, cx".to_owned()) && body.contains(&"movsx ebx, dx".to_owned()), "{body:?}");
        let body = watcall16("far_pointer_");
        assert!(body.contains(&"mov si, ax".to_owned()) && body.contains(&"mov es, dx".to_owned()) && body.contains(&"add ax, bx".to_owned()) && body.contains(&"add ax, cx".to_owned()), "{body:?}");
    }

    /// A struct larger than a dword is written through the near address in SI, which comes back in AX.
    #[test]
    fn test_m16_watcom_a_struct_result_is_written_through_si_and_returned_in_ax() {
        let body = watcall16("result_");
        assert!(body.contains(&"mov dword ptr [si], ebx".to_owned()) && body.contains(&"mov ax, si".to_owned()), "{body:?}");
        assert_eq!(body.last().map(String::as_str), Some("retf"));
    }

    /// The call passes a long in DX:AX and an int in BX and leaves the callee's own `retf` to pop nothing.
    #[test]
    fn test_m16_watcom_a_call_passes_a_long_in_dx_ax_and_takes_the_result_in_dx_ax() {
        let body = watcall16("calls_");
        assert!(body.contains(&"mov bx, 5".to_owned()) && body.contains(&"call far ptr away_".to_owned()), "{body:?}");
        assert!(!body.iter().any(|line| line.starts_with("add sp")), "{body:?}");
    }

    /// The loop of `function` in `fixture`'s listing as `llrm-c -O2 -march=i486 -S`
    /// writes it: from the label its backward branch takes to the branch.
    fn driven_loop(fixture: &str, function: &str) -> Vec<String> {
        let directory = tempfile::tempdir().unwrap();
        let (source, out) = (Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs")), directory.path().join("out.asm"));
        let argv = ["-O2", "-march=i486", "-S", source.to_str().unwrap(), "-o", out.to_str().unwrap()].map(str::to_owned);
        assert_eq!(super::main(&argv), 0);
        let asm = std::fs::read_to_string(out).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        let lines: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let at = lines.iter().rposition(|one| one.starts_with('j') && !one.starts_with("jmp")).expect("a backward branch");
        let label = format!("{}:", lines[at].split_whitespace().nth(1).unwrap());
        let top = lines.iter().position(|one| *one == label).expect("its label");
        lines[top..=at].iter().map(|one| (*one).to_owned()).collect()
    }

    /// A volatile store writes only the bytes it addresses: to video memory
    /// it cannot change `*ch` or `*at`. Volatile was a barrier, and
    /// `inttoptr 0xB8000000` was no known segment, so `video` read both
    /// pointers and both cells again every trip, 4 loads (#257's regression).
    /// At 0x1234, which may be the program's own data, they stay.
    #[test]
    fn test_a_volatile_far_store_to_video_memory_leaves_the_loop_its_cells() {
        let loads = |body: &[String]| body.iter().filter(|one| one.contains("ptr [")).count();
        let stores = |body: &[String]| body.iter().filter(|one| one.contains("byte ptr es:[")).count();
        let video = driven_loop("farvolatile", "_video");
        assert_eq!((loads(&video), stores(&video)), (0, 2), "{video:#?}");
        let conventional = driven_loop("farvolatile", "_conventional");
        assert_eq!((loads(&conventional), stores(&conventional)), (4, 2), "{conventional:#?}");
    }

    /// `die` and `quit` do not return: nothing follows their calls. Without
    /// the call class stated, `f` kept a return after both, 11 instructions
    /// to 8.
    #[test]
    fn test_nothing_follows_a_call_that_does_not_return() {
        let body = selected_body("tests/test_noreturn_and_aborts_are_stated_of_the_callee", "_f");
        assert_eq!(body.len(), 8, "{body:?}");
        assert!(body.iter().all(|one| !one.starts_with("retf")), "{body:?}");
    }

    /// `strides` walks frame arrays of 1-, 2-, 4- and 8-byte elements with one
    /// counter. Strength gave each stride its own pointer, and two of them lived
    /// in the frame: loaded for every access and stepped in memory. Indexes in
    /// registers are the cure: one per address width, as the byte array through
    /// 16-bit `si` saves its two address-size prefixes for one more add.
    #[test]
    fn test_arrays_of_several_strides_keep_their_indexes_in_registers() {
        for function in ["_bench_strides3", "_bench_strides4"] {
            let body = selected_loop("strides", function, "xor");
            let registers: std::collections::BTreeSet<String> = body
                .iter()
                .filter_map(|one| Some(one.split_once('[')?.1.split_once(']')?.0.to_owned()))
                .flat_map(|inside| inside.split(['+', '-', '*']).map(str::trim).map(str::to_owned).collect::<Vec<_>>())
                .filter(|part| part.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && !matches!(part.as_str(), "bp" | "ebp"))
                .filter(|part| ["ax", "bx", "cx", "dx", "si", "di"].iter().any(|name| part.ends_with(name)))
                .collect();
            // A pointer kept in the frame is stepped there: `add word ptr [bp-76h], 1`.
            let stepped = body.iter().any(|one| ["add ", "sub ", "inc ", "dec "].iter().any(|op| one.strip_prefix(op).is_some_and(|rest| rest.split(',').next().unwrap_or("").contains("ptr ["))));
            assert!(registers.len() <= 2 && !stepped, "{function}: {registers:?}\n{body:#?}");
        }
    }

    /// qcport's sys.c, item.c and mdl_ai.c keep near and far function
    /// pointers in static tables; the rich route refused all three with
    /// "a code address in data".
    #[test]
    fn test_code_addresses_in_data_are_called_through() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/codeptrs.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "codeptrs", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        assert!(lines.contains(&"dw _twice") && lines.iter().any(|one| one.starts_with("dd _show")), "{asm}");
        let from = asm.find("_run proc").expect("_run");
        let run: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(run.iter().any(|one| one.starts_with("call dword ptr [")), "{run:#?}");
        assert!(run.iter().any(|one| one.split_whitespace().collect::<Vec<_>>() == ["call", "ax"] || one.starts_with("call word ptr [")), "{run:#?}");
    }

    /// qcport's sc.c initializes `short links[5]` from a constant: an
    /// aggregate CGAssign, refused as "a 10-byte aggregate as a value".
    #[test]
    fn test_a_local_array_initializer_is_copied() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/arrayinit.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "arrayinit", None, &llrm_driver::m16_options(machine));
        assert!(built.is_ok(), "{:?}", built.err());
    }

    /// qcport's dbg.c, sys_time.c, d_poly.c and ent.c use `_asm` and
    /// `__emit__`: "inline code F.0 is not raised to the rich MIR". The code
    /// is laid down with each frame place it names at that place's slot, and
    /// a value-less return after it answers what it left in dx:ax.
    #[test]
    fn test_inline_code_reads_and_writes_its_frame_places() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/inlinecode.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "inlinecode", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let body = |name: &str| -> Vec<String> {
            let from = asm.find(&format!("{name} proc")).expect("the procedure");
            asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).map(str::to_owned).collect()
        };
        // Each db line decoded: the code's own instructions, displacements patched.
        let decoded = |lines: &[String]| -> Vec<String> {
            lines
                .iter()
                .map(|line| match line.strip_prefix("db ") {
                    Some(bytes) => {
                        let bytes: Vec<u8> = bytes.split(',').map(|one| u8::from_str_radix(one.trim_end_matches('h'), 16).unwrap()).collect();
                        let mut decoder = iced_x86::Decoder::new(16, &bytes, iced_x86::DecoderOptions::NONE);
                        let mut formatter = iced_x86::NasmFormatter::new();
                        iced_x86::Formatter::options_mut(&mut formatter).set_number_base(iced_x86::NumberBase::Decimal);
                        decoder.iter().map(|one| {
                            let mut text = String::new();
                            iced_x86::Formatter::format(&mut formatter, &one, &mut text);
                            text
                        }).collect::<Vec<_>>().join("; ")
                    }
                    None => line.clone(),
                })
                .collect()
        };
        let sine = decoded(&body("_sine"));
        let slot = |line: &str| line.split_once('[').map(|(_, rest)| rest.trim_end_matches(']').replace(' ', "").to_lowercase());
        let stored = sine.iter().find(|one| one.starts_with("fstp qword ptr")).and_then(|one| slot(one)).expect("rad's slot");
        let code = sine.iter().find(|one| one.contains("fsin")).expect("the code");
        let returned = sine.iter().rev().find(|one| one.starts_with("fld qword ptr")).and_then(|one| slot(one)).expect("result's slot");
        assert!(code.starts_with(&format!("fld qword [{stored}]")) && code.ends_with(&format!("fstp qword [{returned}]")), "{sine:#?}");
        let ticks = body("_ticks");
        assert!(ticks.contains(&"db 00fh,031h".to_owned()), "{ticks:#?}");
    }

    /// qcport's sys_time.c reads the BIOS tick count through the constant
    /// far pointer 0040:006Ch. The constant was made a word, its segment
    /// lost, and the read went to DGROUP:006Ch: the tick never changed and
    /// calibration spun forever.
    #[test]
    fn test_a_far_pointer_constant_keeps_its_segment() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/farconst.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "farconst", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_ticks proc").expect("_ticks");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let segment = |one: &&str| one.ends_with(", 64") || one.ends_with(&format!(", {}", 0x0040_006C));
        assert!(!body.iter().any(|one| one.contains("DGROUP")) && body.iter().any(segment), "{body:#?}");
    }

    /// qcport's d_alias.c tests a far LeafCache pointer against 0. The 0
    /// became a near pointer made far, DGROUP:0, so a null cache passed the
    /// test, its stores landed in the interrupt table, and the next timer
    /// tick jumped to garbage. An integer made a far pointer is its own
    /// segment:offset, as Borland converts it.
    #[test]
    fn test_an_integer_made_a_far_pointer_is_not_in_dgroup() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/farnull.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "farnull", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_put proc").expect("_put");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(!body.iter().any(|one| one.contains("DGROUP")), "{body:#?}");
    }

    /// qcport's snd_mix_paint runs from dsp.asm's IRQ on its own stack, DS
    /// loaded with DGROUP and SS not. It read the mixer's fields through ss:
    /// and set DS from SS, painted through a pointer read from the IRQ's
    /// stack segment, and wrote the VGA BIOS ROM 107k times.
    #[test]
    fn test_near_data_is_reached_through_ds_alone() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/nearviads.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "nearviads", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_paint proc").expect("_paint");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let through_stack = |one: &&str| one.contains("ss:[") || one.contains("mov ds,") || one.starts_with("lds ") || one.starts_with("pop ds");
        assert!(!body.iter().any(through_stack), "{body:#?}");
    }

    /// qcport's savegame.c passes stack locals as far pointers. Their segment
    /// was DGROUP, wrong wherever the stack is not in it, as dsp.asm's IRQ
    /// stack is not.
    #[test]
    fn test_a_far_pointer_to_a_local_is_in_the_stack_segment() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/localfar.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "localfar", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_caller proc").expect("_caller");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(!body.iter().any(|one| one.contains("DGROUP")) && body.iter().any(|one| one.ends_with("ss")), "{body:#?}");
    }

    /// A call's `add sp,2` is three bytes; BCC -Os pops the argument into CX
    /// in one. llrm-c -Os kept the add: 1.3K of QCport's surplus over BCC.
    /// On the 486 a pop is slower, so -O2 keeps it.
    #[test]
    fn test_arguments_are_popped_off_at_os_only() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/popargs.cgs")).unwrap();
        let cleanups = |level: Level| {
            let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
            let options = llrm_core::driver::Options { pipeline: level.options(), ..llrm_driver::m16_options(machine) };
            let built = super::selected(&text, "popargs", None, &options).expect("selects");
            let asm = llrm_core::backend::masm::text(&built).unwrap();
            let from = asm.find("_caller proc").expect("_caller");
            asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).filter(|one| one.starts_with("add sp") || *one == "pop cx").map(str::to_owned).collect::<Vec<_>>()
        };
        use llrm_core::driver::flags::Level;
        assert_eq!(cleanups(Level::Os), ["pop cx", "pop cx", "pop cx"]);
        assert_eq!(cleanups(Level::O2), ["add sp, 2", "add sp, 4"]);
    }

    /// A C function that calls nothing came out of the compile with no word
    /// that nothing re-enters it: `norecurse` was inferred where no compiler ran.
    #[test]
    fn test_a_leaf_function_comes_out_of_the_compile_norecurse() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/halve.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let directory = std::env::temp_dir().join(format!("llrm-c-norecurse-{}", std::process::id()));
        super::selected(&text, "halve", Some(&directory), &llrm_driver::m16_options(machine)).expect("selects");
        let mut stages = std::fs::read_dir(&directory).unwrap().flatten().map(|one| one.path()).filter(|one| one.extension().is_some_and(|ext| ext == "ll")).collect::<Vec<_>>();
        stages.sort_by_key(|one| one.file_name().map(std::ffi::OsStr::to_owned));
        let last = std::fs::read_to_string(stages.last().expect("a stage")).unwrap();
        std::fs::remove_dir_all(&directory).ok();
        let defined = last.lines().filter(|line| line.starts_with("define")).collect::<Vec<_>>();
        assert!(!defined.is_empty() && defined.iter().all(|line| line.contains("norecurse")), "{defined:?}");
    }

    /// bcc -O makes fabs the x87 instruction; llrm-c called the library's:
    /// a double pushed, a far call, eight bytes cleaned, 29 times in QCport.
    #[test]
    fn test_fabs_is_the_instruction() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/fabs.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "fabs", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_halfabs proc").expect("_halfabs");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(body.contains(&"fabs") && !body.iter().any(|one| one.starts_with("call")), "{body:#?}");
    }

    /// Spill slots went below the allocas: past a 200-byte array, each of
    /// their accesses took a two-byte displacement. QCport had 3,088 such.
    #[test]
    fn test_spill_slots_sit_above_a_big_frame_array() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/spillnear.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "spillnear", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_spills proc").expect("_spills");
        let far: Vec<&str> = asm[from..]
            .lines()
            .map(str::trim)
            .take_while(|one| !one.ends_with("endp"))
            .filter(|one| !one.starts_with("lea"))
            .filter(|one| one.split("[bp-").nth(1).and_then(|rest| rest.split(']').next()?.parse::<i64>().ok()).is_some_and(|disp| disp > 128))
            .collect();
        assert!(far.is_empty(), "{far:#?}");
    }

    /// A frame object passed to a call is read by it. DSE took it for
    /// private and dropped every store before the call that `test` did not
    /// read back itself: QCport's ls_selftest returned -2.
    #[test]
    fn test_stores_a_callee_reads_through_its_argument_stay() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/argread.cgs")).unwrap();
        let options = llrm_core::driver::Options { pipeline: llrm_core::driver::flags::Level::Os.options(), ..llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() }) };
        let built = super::selected(&text, "argread", None, &options).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_test proc").expect("_test");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let calls: Vec<usize> = body.iter().enumerate().filter(|(_, one)| **one == "call far ptr _animate").map(|(at, _)| at).collect();
        // `s`, by the address passed to `animate`: `lea ax, [bp-N]`.
        let base: i64 = body[..calls[1]].iter().rev().find_map(|one| one.strip_prefix("lea ax, [bp-")?.strip_suffix(']')?.parse().ok()).expect("&s");
        // Every byte the stores between the calls write, by frame offset.
        let mut written = std::collections::BTreeSet::new();
        for one in &body[calls[0]..calls[1]] {
            let Some((width, rest)) = [("byte", 1), ("word", 2), ("dword", 4)].iter().find_map(|(name, width)| Some((*width, one.strip_prefix(&format!("mov {name} ptr [bp-"))?))) else { continue };
            let at: i64 = rest.split(']').next().and_then(|one| one.parse().ok()).expect("an offset");
            written.extend(-at..-at + width);
        }
        // `animate` reads tab[2] (bytes 16..24 of `s`) and `last` (32..36).
        let read: std::collections::BTreeSet<i64> = (16..24).chain(32..36).map(|one| one - base).collect();
        assert!(read.is_subset(&written), "missing {:?}\n{body:#?}", read.difference(&written).collect::<Vec<_>>());
    }

    /// A local array read only by a variable index: DSE dropped its
    /// initializing stores. QCport's sc_selftest returned -109.
    #[test]
    fn test_an_array_read_by_index_keeps_its_initializer() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/indexread.cgs")).unwrap();
        let options = llrm_core::driver::Options { pipeline: llrm_core::driver::flags::Level::Os.options(), ..llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() }) };
        let built = super::selected(&text, "indexread", None, &options).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_test proc").expect("_test");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let stored: Vec<&&str> = body.iter().filter(|one| one.starts_with("mov") && one.contains("ptr [bp-") && one.contains("],")).collect();
        assert!(stored.len() >= 3, "{body:#?}");
    }

    /// Each float-to-int conversion switched the rounding mode to chop and
    /// back: two conversions in a row loaded the control word four times.
    #[test]
    fn test_conversions_in_a_row_switch_the_rounding_mode_once() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/truncs.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "truncs", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_truncs proc").expect("_truncs");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert_eq!(body.iter().filter(|one| one.starts_with("fldcw")).count(), 2, "{body:#?}");
    }

    /// An if/else's other arm was placed after the return: in a large
    /// function its branch and its jump back were both near, 4 and 3 bytes.
    /// QCport had 1,829 near branches past a return.
    #[test]
    fn test_both_arms_of_an_if_else_come_before_the_return() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/diamond.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "diamond", None, &llrm_driver::m16_options(machine)).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_diamond proc").expect("_diamond");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(body.last().is_some_and(|last| last.starts_with("ret")), "{body:#?}");
    }

    /// A float argument read across a call went to a fresh 8-byte slot:
    /// loaded and stored at entry, reloaded from the copy. Its own cell,
    /// which nothing writes, is read again instead.
    #[test]
    fn test_a_float_argument_is_read_from_its_own_cell() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/paramremat.cgs")).unwrap();
        let options = llrm_core::driver::Options { pipeline: llrm_core::driver::flags::Level::Os.options(), ..llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() }) };
        let built = super::selected(&text, "paramremat", None, &options).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        let from = asm.find("_summed proc").expect("_summed");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(body.contains(&"fadd dword ptr [bp+6]") && body.contains(&"sub sp, 8"), "{body:#?}");
    }

    /// The listing of each function in `fixture`, `-Os` on a 486.
    fn listed(fixture: &str, functions: &[&str]) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs"))).unwrap();
        let options = llrm_core::driver::Options { pipeline: llrm_core::driver::flags::Level::Os.options(), ..llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() }) };
        let built = super::selected(&text, fixture, None, &options).expect("selects");
        let asm = llrm_core::backend::masm::text(&built).unwrap();
        functions
            .iter()
            .map(|function| {
                let from = asm.find(&format!("{function} proc")).expect("the function");
                asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).map(str::to_owned).collect()
            })
            .collect()
    }

    /// `p == 0` for a far `p` compared it with `ptrtoint (inttoptr 0)`:
    /// the null's words were computed at run time, `xor ebx,ebx; shld
    /// ecx,ebx,16`, and compared with p's instead of `or`ing p's.
    #[test]
    fn test_a_far_null_test_ors_the_words() {
        for body in listed("farwords", &["_load", "_take"]) {
            assert!(!body.iter().any(|one| one.starts_with("shld") || one.starts_with("xor e")) && body.iter().any(|one| one.starts_with("or ")), "{body:#?}");
        }
    }

    /// A long returned in DX:AX and cast to a far pointer was joined into
    /// one register (`shl`, `shrd`) and split again (`shld`) to be stored.
    #[test]
    fn test_a_long_made_a_far_pointer_is_stored_as_its_words() {
        let [body] = listed("farwords", &["_keep"]).try_into().expect("one");
        assert!(!body.iter().any(|one| one.starts_with("shl") || one.starts_with("shrd") || one.starts_with("shld")), "{body:#?}");
        // Taken as they were, the words kept the call's dword width: a
        // selector went to EAX, `byte ptr eax:[...]`, and d_sky.c crashed.
        let [body] = listed("farwords", &["_copyrows"]).try_into().expect("one");
        assert!(!body.iter().any(|one| one.contains("ptr e") && one.contains("x:")), "{body:#?}");
    }

    /// A crowded x87 compare shares its source position with its branches;
    /// the spill victim was looked for at the last branch, where nothing is
    /// live, and qb-qrender's d_faces.c was refused.
    #[test]
    fn test_floats_crowding_a_compare_before_its_branches_spill() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/x87crowd.cgs")).unwrap();
        let machine = llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "x87crowd", None, &llrm_driver::m16_options(machine));
        assert!(built.is_ok(), "{:?}", built.err());
    }

    /// Watcom types a void function as an int whose returns give none;
    /// raised as `ret i16 poison`, isel refused all of qmove.
    #[test]
    fn test_a_void_function_is_selected() {
        let path = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/parity/qmove.cgs");
        let built = super::selected(&std::fs::read_to_string(path).unwrap(), "qmove", None, &llrm_driver::m16_options(llrm_core::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() }));
        assert!(built.is_ok(), "{:?}", built.err());
    }

    /// tests/fixtures/c32/`fixture`.cgs, as the 386 front end recorded it, selected for `-m32`.
    fn flat_listing(fixture: &str) -> Vec<String> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c32/{fixture}.cgs"))).unwrap();
        let argv: Vec<String> = ["-m32", "-O2", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let built = super::selected(&text, fixture, None, &args.codegen).unwrap();
        llrm_core::backend::masm::text(&built).unwrap().lines().map(|line| line.trim().to_owned()).collect()
    }

    /// `int add(int, int)` as flat 32-bit code: cdecl32's arguments at [esp+4] and [esp+8] (EBP is
    /// no frame where nothing needs one), the result in EAX. It listed `bp`, `[bp+4]` and a DX:AX
    /// result, and never reached the allocator, before the target stated them.
    #[test]
    fn test_m32_lists_add_as_flat_cdecl32() {
        let lines = flat_listing("add");
        assert_eq!(&lines[..2], [".386", ".model flat"]);
        let body: Vec<&str> = lines.iter().skip_while(|line| *line != "_add proc near").skip(1).take_while(|line| *line != "_add endp").map(String::as_str).collect();
        assert_eq!(body, ["L0_0:", "mov eax, dword ptr [esp+4]", "add eax, dword ptr [esp+8]", "ret"]);
    }

    /// The body of `name`'s procedure in the flat listing of `regs.c`, as the default convention compiles it.
    fn regs_body(name: &str) -> Vec<String> {
        let lines = flat_listing("regs");
        lines.iter().skip_while(|line| **line != format!("{name} proc near")).skip(1).take_while(|line| **line != format!("{name} endp")).cloned().collect()
    }

    /// `fixture`'s flat listing under `-mabi=sysv`: gcc's i386 convention.
    fn sysv_body(name: &str) -> Vec<String> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32/sysv.cgs")).unwrap();
        let argv: Vec<String> = ["-m32", "-O2", "-mabi=sysv", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let built = super::selected(&text, "sysv", None, &args.codegen).unwrap();
        let lines: Vec<String> = llrm_core::backend::masm::text(&built).unwrap().lines().map(|line| line.trim().to_owned()).collect();
        lines.iter().skip_while(|line| **line != format!("{name} proc near")).skip(1).take_while(|line| **line != format!("{name} endp")).cloned().collect()
    }

    /// gcc -m32 writes every struct result, a one-byte one too, through a pointer that is the first argument and pops it,
    /// `ret 4`: Open Watcom's rule (a 1, 2 or 4 byte struct in EAX, the address in ESI) was applied whatever the ABI.
    #[test]
    fn test_m32_sysv_writes_a_struct_result_through_the_first_argument_and_the_callee_pops_it() {
        assert_eq!(sysv_body("_r1"), ["L0_0:", "mov eax, dword ptr [esp+4]", "mov ecx, dword ptr [esp+8]", "mov byte ptr [eax], cl", "ret 4"]);
        assert_eq!(sysv_body("_r12").last().map(String::as_str), Some("ret 4"));
    }

    /// The caller of such a function removes the arguments but the address: gcc added 8 after pushing 12 bytes.
    #[test]
    fn test_m32_sysv_caller_does_not_remove_the_result_address_the_callee_popped() {
        let body = sysv_body("_calls");
        let after = body.iter().skip_while(|line| **line != "call _away").skip(1).cloned().collect::<Vec<_>>();
        assert!(after.iter().any(|line| line == "add esp, 8"), "{body:?}");
        assert!(!after.iter().any(|line| line == "add esp, 12"), "{body:?}");
    }

    /// An ELF object has no `_` before a C name and no `_` after a default one: `six_` and `_explicit_cdecl` were
    /// printed whatever object format was asked, where the convention's `symbol` table says `*` for elf.
    #[test]
    fn test_m32_an_elf_object_spells_its_symbols_without_decoration() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32/regs.cgs")).unwrap();
        let argv: Vec<String> = ["-m32", "-O2", "x.c"].map(str::to_owned).to_vec();
        let mut args = super::parse_args(&argv).unwrap();
        args.codegen.object_format = "elf";
        let built = super::selected(&text, "regs", None, &args.codegen).unwrap();
        let listing = llrm_core::backend::masm::text(&built).unwrap();
        for name in ["six", "result", "explicit_cdecl", "far_away"] {
            assert!(listing.contains(&format!("\n{name} proc near")) || listing.contains(&format!("extern {name}:near")), "{name} in {listing}");
        }
        assert!(!listing.contains("six_") && !listing.contains("_explicit_cdecl"), "{listing}");
    }

    /// An unmarked C function takes Open Watcom's register convention: `six` read its fifth and sixth from the
    /// stack and popped them with `ret 8`, the first four being in EAX, EDX, EBX and ECX. It had refused such
    /// a function ("has a register calling convention"), compiled with -ecc.
    #[test]
    fn test_m32_an_unmarked_function_takes_registers_and_pops_its_stack_arguments() {
        let body = regs_body("six_");
        assert_eq!(body, ["L0_0:", "add eax, edx", "add eax, ebx", "add eax, ecx", "add eax, dword ptr [esp+4]", "add eax, dword ptr [esp+8]", "ret 8"]);
    }

    /// A struct larger than a dword is written through the address in ESI, which comes back in EAX, as `wcc386` has it.
    #[test]
    fn test_m32_a_struct_result_is_written_through_esi_and_returned_in_eax() {
        let body = regs_body("result_");
        assert!(body.contains(&"mov dword ptr [esi], eax".to_owned()) && body.contains(&"mov eax, esi".to_owned()), "{body:?}");
        assert_eq!(body.last().map(String::as_str), Some("ret"));
    }

    /// A one-byte struct argument travels in AL as an integer does; a twelve-byte one is in memory with `a` after it.
    #[test]
    fn test_m32_a_small_struct_argument_is_a_register_and_a_large_one_is_memory() {
        assert_eq!(regs_body("small_"), ["L4_0:", "movsx eax, al", "add eax, edx", "ret"]);
        let body = regs_body("by_value_");
        assert!(body.contains(&"mov eax, dword ptr [esp+4]".to_owned()) && body.contains(&"add eax, dword ptr [esp+16]".to_owned()), "{body:?}");
        assert_eq!(body.last().map(String::as_str), Some("ret 16"));
    }

    /// `__cdecl` names the stack convention and its `_name` symbol on a target whose default is registers.
    #[test]
    fn test_m32_an_explicit_cdecl_function_keeps_the_stack_and_its_caller_pops() {
        let body = regs_body("_explicit_cdecl");
        assert_eq!(body, ["L5_0:", "mov eax, dword ptr [esp+4]", "sub eax, dword ptr [esp+8]", "ret"]);
    }

    /// A narrow argument goes as a stack slot: `push ax` pushed two bytes, and cdecl32's next
    /// argument, and the callee's read of it, lay a dword apart.
    #[test]
    fn test_m32_pushes_narrow_arguments_as_dwords() {
        let lines = flat_listing("args");
        assert!(lines.iter().filter(|line| line.starts_with("push ")).all(|line| line.split_whitespace().nth(1).is_some_and(|operand| operand.starts_with('e') || operand.starts_with("offset"))), "{lines:#?}");
        assert!(lines.contains(&"add esp, 8".to_owned()), "{lines:#?}");
    }

    /// Native 32-bit addressing reads whole registers: the pass that zeroed EBP's upper half for a
    /// cell read 32 bits wide (a 16-bit frame's `[bp]` under 32-bit addressing) wrote `movzx ebp, ebp`
    /// into a flat frame, and the reserve was 70 bytes, not a multiple of the dword stack.
    #[test]
    fn test_m32_keeps_the_dword_stack() {
        let lines = flat_listing("bytes");
        assert!(lines.iter().all(|line| !line.starts_with("movzx ebp")), "{lines:#?}");
        assert!(lines.contains(&"sub esp, 76".to_owned()), "{lines:#?}");
        // 70 under the frame register, 76 reserved with its cell: 2 above the stack pointer.
        assert!(lines.contains(&"mov byte ptr [esp+eax+2], al".to_owned()), "{lines:#?}");
    }

    /// `fixture`'s flat object, as records: (type, body).
    fn flat_object(fixture: &str) -> Vec<(u8, Vec<u8>)> {
        flat_object_with(fixture, &[])
    }

    fn flat_object_with(fixture: &str, flags: &[&str]) -> Vec<(u8, Vec<u8>)> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c32/{fixture}.cgs"))).unwrap();
        let argv: Vec<String> = ["-m32", "-O2"].iter().chain(flags).chain(&["x.c"]).map(|one| (*one).to_owned()).collect();
        let args = super::parse_args(&argv).unwrap();
        let built = super::selected(&text, fixture, None, &args.codegen).unwrap();
        let bytes = llrm_core::backend::objbuild::written(&built, "x.c").unwrap();
        let (mut at, mut out) = (0, Vec::new());
        while at < bytes.len() {
            let length = usize::from(u16::from_le_bytes([bytes[at + 1], bytes[at + 2]]));
            out.push((bytes[at], bytes[at + 3..at + 2 + length].to_vec()));
            at += 3 + length;
        }
        out
    }

    /// A flat object is USE32 and its records are the 32-bit ones: a segment with its 32-bit length,
    /// a public with a 32-bit offset, data at a 32-bit offset, no group; and `add`'s code is the
    /// 32-bit encoding of what its listing says. It was 16-bit records and `67 8b 46 08` (`mov
    /// eax, [bp+8]` under an address-size prefix) ending in `66 c3` (a word return).
    #[test]
    fn test_m32_writes_a_use32_object_with_its_own_encoding() {
        let records = flat_object("add");
        let kinds: Vec<u8> = records.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(kinds, [0x80, 0x96, 0x99, 0x99, 0x91, 0xA1, 0x8A], "THEADR LNAMES SEGDEF32 x2 PUBDEF32 LEDATA32 MODEND");
        assert!(records[2].1[0] & 1 == 1 && records[3].1[0] & 1 == 1, "both segments are USE32: {records:?}");
        let code = &records[5].1;
        assert_eq!(hex(&code[5..]), "8b44240403442408c3", "mov eax,[esp+4]; add eax,[esp+8]; ret");
    }

    /// A near procedure that pops its own arguments returns `ret 4` (`c2 0400`): the word form,
    /// `66 c2 0400`, pops a 16-bit return address and sent the flat program into the vector table.
    #[test]
    fn test_m32_returns_popping_arguments_with_a_dword_ret() {
        // The function that pops is called once: kept as one.
        let object = flat_object_with("pop", &["-fno-inline-functions-called-once"]);
        let code = hex(&object.iter().find(|(kind, _)| *kind == 0xA1).expect("code").1);
        assert!(code.contains("c20400") && !code.contains("66c2"), "{code}");
    }

    /// A flat string operation takes its operands in ESI, EDI and ECX with no segment operand: it was
    /// `movs dword ptr es:[di], dword ptr ebx:[si]` with the source and count in whatever registers
    /// the allocator chose, and a refusal ("may be in no register") before that.
    #[test]
    fn test_m32_lists_a_string_copy_through_esi_edi_ecx() {
        let lines = flat_listing("strings");
        let at = lines.iter().position(|line| line == "rep movsd").expect("a rep movsd");
        let before = &lines[..at];
        assert!(before.iter().any(|line| line.starts_with("lea esi, [esp")) && before.iter().any(|line| line.starts_with("lea edi, [esp")), "{before:#?}");
        assert!(before.iter().rev().take(4).any(|line| line == "shr ecx, 2" || line.starts_with("mov ecx")), "{before:#?}");
        let object = flat_object("strings");
        let code = hex(&object.iter().find(|(kind, _)| *kind == 0xA1).expect("code").1);
        assert!(code.contains("f3a5") && !code.contains("66f3a5"), "rep movsd, not its word form: {code}");
    }

    /// `-fsanitize=stack` compares ESP with a dword limit and enters a near handler: `cmp sp, word ptr`
    /// and `call far ptr` were a 16-bit compare and a far call in a flat program.
    #[test]
    fn test_m32_checks_its_stack_against_a_dword_limit_and_a_near_handler() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32/add.cgs")).unwrap();
        let argv: Vec<String> = ["-m32", "-O2", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let built = super::selected_checking(&text, "add", None, &args.codegen, Some(super::stack_check(&llrm_x86_m32::M32))).unwrap();
        let listing = llrm_core::backend::masm::text(&built).unwrap();
        assert!(listing.contains("cmp esp, dword ptr _llrm_os_stack_low") && listing.contains("call __STKOVERFLOW"), "{listing}");
        assert!(!listing.contains("far ptr") && listing.contains("extern __STKOVERFLOW:near"), "{listing}");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The body of `name` in the flat listing of `fixture`.
    fn flat_body(fixture: &str, name: &str) -> Vec<String> {
        let lines = flat_listing(fixture);
        lines.iter().skip_while(|line| **line != format!("_{name} proc near")).skip(1).take_while(|line| **line != format!("_{name} endp")).cloned().collect()
    }

    /// A short read through a pointer and widened is one `movsx`. The peephole asked the 16-bit
    /// encoder whether `movsx eax, word ptr [eax]` encodes, which it does not, so a flat compile kept
    /// `mov ax, word ptr [eax]; movsx eax, ax`.
    #[test]
    fn test_m32_widens_a_load_in_one_instruction() {
        let body = flat_body("extend", "extend");
        assert!(body.contains(&"movsx eax, word ptr [eax]".to_owned()), "{body:#?}");
    }

    /// Every address register holds a pointer loaded from the arguments. The peephole decoded the
    /// bytes of the flat code it was judging as 16-bit code, read the effects of `movsx eax, word ptr
    /// [ebx]` wrongly, and moved it above a read of EAX: `movsx eax, [ebx]; movsx ebx, [eax]`
    /// loaded through a value.
    #[test]
    fn test_m32_never_addresses_through_a_value() {
        let body = flat_body("extend", "extend2");
        let mut pointers: Vec<&str> = Vec::new();
        for line in &body {
            let Some((operation, operands)) = line.split_once(' ') else { continue };
            let Some((destination, rest)) = operands.split_once(", ") else { continue };
            if let Some(inside) = rest.split_once('[').and_then(|(_, tail)| tail.split_once(']')).map(|(inside, _)| inside) {
                for register in inside.split(|one: char| !one.is_ascii_alphanumeric()).filter(|one| one.len() == 3 && one.starts_with('e')) {
                    assert!(register == "ebp" || register == "esp" || pointers.contains(&register), "{register} is a value in `{line}`: {body:#?}");
                }
            }
            let destination = destination.trim();
            if destination.len() == 3 && destination.starts_with('e') && operation.starts_with("mov") {
                // A pointer is what an argument load puts in a register.
                if operation == "mov" && (rest.starts_with("dword ptr [ebp+") || rest.starts_with("dword ptr [esp+")) {
                    if !pointers.contains(&destination) {
                        pointers.push(destination);
                    }
                } else {
                    pointers.retain(|one| *one != destination);
                }
            }
        }
    }

    /// A word sum is the word `add`, not a dword one and the `movzx` of its low word: its upper
    /// half stays as it was, which is zero where the words were zero-extended.
    #[test]
    fn test_m32_adds_words_as_words() {
        let body = flat_body("narrow", "short_sum");
        assert!(body.iter().any(|line| line.starts_with("add ax, ")) && !body.iter().any(|line| line.starts_with("add eax, ")), "{body:#?}");
    }

    /// The same sum of dwords keeps the `movzx`: the word `add` would leave the upper half as it was,
    /// and the result is read as a dword.
    #[test]
    fn test_m32_keeps_the_extension_where_the_upper_half_is_not_zero() {
        let body = flat_body("narrow", "int_sum");
        assert!(body.iter().any(|line| line.starts_with("add eax, ")) && body.iter().any(|line| line == "movzx eax, ax"), "{body:#?}");
    }

    /// The sieve's inner loop indexes its array with a word counter, and the counter's register is the
    /// index: the zero extension of a word whose register is zero above it was a copy into another one
    /// that the store read, one more instruction a trip than m16's `[bp+si]`.
    #[test]
    fn test_m32_indexes_through_the_counter_not_a_copy_of_it() {
        let body = flat_body("sieve", "bench_sieve");
        let from = body.iter().position(|line| line.starts_with("imul ")).expect("the counter starts as i*i");
        let loop_ = &body[from..];
        let copies = |line: &&String| line.starts_with("movzx e") || line.split_once(' ').is_some_and(|(operation, operands)| operation == "mov" && operands.split_once(", ").is_some_and(|(to, from)| to.len() == 3 && from.len() == 3 && to.starts_with('e') && from.starts_with('e')));
        assert!(loop_.iter().filter(copies).count() == 0, "{loop_:#?}");
    }

    /// `d = a + b` into a register that is neither is one `lea`: no flags to keep, three bytes for the four of
    /// `mov; add`, and a native form flat code has without a prefix. The target priced a flat dword's address
    /// as the real-mode prefixed one and had no price for the native form, so it never chose it.
    #[test]
    fn test_m32_adds_into_a_new_register_with_lea() {
        let body = flat_body("lea", "lea_sum");
        assert!(body.iter().any(|line| line.starts_with("lea e") && line.contains("[e") && line.contains("+e")), "{body:#?}");
        assert!(!body.iter().any(|line| line != "mov ebp, esp" && line.starts_with("mov e") && line.split_once(", ").is_some_and(|(_, from)| from.starts_with('e') && from.len() == 3)), "{body:#?}");
    }

    /// A loop over `int *`: the pointer, the index and the sum are dwords in 32-bit registers,
    /// addressed `[base+index]` with no segment, selector or 16-bit register.
    #[test]
    fn test_m32_lists_a_loop_over_int_pointers() {
        let lines = flat_listing("sum");
        let body: Vec<&str> = lines.iter().skip_while(|line| *line != "sum_ proc near").skip(1).take_while(|line| *line != "sum_ endp").map(String::as_str).collect();
        // The load is the add's operand, and the add of the stride sets the flags the branch reads.
        assert!(body.iter().any(|line| line.starts_with("add e") && line.contains("dword ptr [e") && line.contains("+e")), "{body:#?}");
        assert!(body.iter().any(|line| line.starts_with("add e") && line.ends_with(", 4")), "{body:#?}");
        assert!(body.iter().any(|line| line.starts_with("jne ")) && !body.iter().any(|line| line.starts_with("cmp ") && line.ends_with(", 0")), "{body:#?}");
        assert!(body.iter().all(|line| !line.contains(" bp") && !line.contains("[bx") && !line.contains("es:") && !line.contains("far")), "{body:#?}");
        assert_eq!(body.last(), Some(&"ret"));
    }
}
