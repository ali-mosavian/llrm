//! C through Open Watcom's front end and
//! the backend, to an object or jwasm source.
//!
//! ```text
//! llrm-c pal.cgs -o pal.obj [--dump DIR]
//! ```

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use llrm_codegen::backend::{masm, objbuild};
use llrm_codegen::driver::flags::{self, Flags};
use llrm_mir::types::FloatKind;

use super::{hir, stream, translate};

#[derive(Debug)]
pub enum CompileError {
    Unsupported(hir::Unsupported),
    Io(std::io::Error),
    Emission(objbuild::Error),
}

impl fmt::Display for CompileError {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
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
pub fn selected(
    text: &str,
    module: &str,
    dump: Option<&Path>,
    codegen: &llrm_codegen::driver::Options,
) -> Result<masm::Module, CompileError> {
    selected_checking(text, module, dump, codegen, None, false, false)
}

/// [`selected`], a module that defines the entry asking its linker for the
/// target's start-up (`-fentry-requests`).
pub fn selected_requesting(
    text: &str,
    module: &str,
    dump: Option<&Path>,
    codegen: &llrm_codegen::driver::Options,
) -> Result<masm::Module, CompileError> {
    selected_checking(text, module, dump, codegen, None, false, true)
}

/// What C's runtime says of its stack on `target`: the `stack.toml` its
/// description names, the OS layer's stack limit and Open Watcom's
/// `__STKOVERFLOW`, which a checked function compares with and enters.
pub fn stack_check(target: &dyn llrm_target::Target) -> llrm_codegen::hir::model::StackCheck {
    let description = target.runtime("c").expect("a C target has a C runtime");
    let name = description.string("stack").expect("c.toml names its stack check");
    stack_check_of(description.file(&name).expect("the stack check is shipped"))
}

fn stack_check_of(text: &str) -> llrm_codegen::hir::model::StackCheck {
    let row = toml::Value::Table(text.parse().expect("stack.toml parses"));
    llrm_codegen::hir::model::StackCheck::from_toml(&row).expect("stack.toml states a stack check")
}

/// The library routines C's runtime on `target` has (`libcalls` of its
/// `c.toml`), each with the calling convention (LLVM's number) a call of it is
/// made in under `abi`: the convention a routine that states none has.
fn libcalls(
    target: &dyn llrm_target::Target,
    abi: Option<&str>,
    format: &str,
) -> Result<Vec<(String, u32, String)>, String> {
    let description = target.runtime("c").ok_or("this target has no C runtime")?;
    libcalls_named(target, abi, format, libcall_names(&*description.table()?))
}

/// `libcalls` for `names`: what a runtime that named them would get.
fn libcalls_named(
    target: &dyn llrm_target::Target,
    abi: Option<&str>,
    format: &str,
    names: Vec<String>,
) -> Result<Vec<(String, u32, String)>, String> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let profile = Profile::for_abi(target, abi)?;
    // A routine the source declares and does not mark: in the default registers
    // where the ABI has them, else C's.
    let convention = if profile.flags.iter().any(|flag| flag == "-ecc") { &profile.cdecl } else { &profile.registers };
    let calling = target.calling();
    let one = calling.named(convention).ok_or_else(|| format!("calling.toml has no {convention}"))?;
    let cc = one.cc.clone().ok_or_else(|| format!("calling.toml has no {convention} with a cc"))?;
    let pattern = one.symbol.get(format).map_or("*", String::as_str);
    let number = match cc.as_str() {
        "cdecl" => 0,
        other => llrm_mir::opcode::CONVENTIONS
            .iter()
            .find(|(spelled, _)| spelled.strip_suffix("cc") == Some(other))
            .map(|(_, number)| *number)
            .ok_or_else(|| format!("no calling convention {other:?}"))?,
    };
    Ok(names
        .into_iter()
        .map(|name| {
            let symbol = pattern.replace('*', &name);
            (name, number, symbol)
        })
        .collect())
}

/// The routines a runtime description names as its own (`libcalls`): none
/// where it names none, which is how a target without them is told.
fn libcall_names(table: &toml::Table) -> Vec<String> {
    table
        .get("libcalls")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|one| one.as_str().map(str::to_owned))
        .collect()
}

/// The unit `text` records and the profile it is raised under, for `codegen`.
pub(crate) fn prepared(
    text: &str,
    codegen: &llrm_codegen::driver::Options,
    wrapv: bool,
    entry_requests: bool,
) -> Result<(hir::Unit, Profile), CompileError> {
    let mut unit = hir::unit(&stream::parse(text))?;
    unit.wrapv = wrapv;
    let mut profile = Profile::for_abi(&*codegen.arch, codegen.abi.as_deref()).map_err(hir::Unsupported)?;
    profile.lifetimes = codegen.pipeline.optimize;
    profile.expects = codegen.pipeline.optimize;
    // Only a program linked against the target's own start-up library asks
    // for it.
    if !entry_requests {
        profile.entry_requests.clear();
        profile.entry_argument_requests.clear();
    }
    let calling = codegen.arch.calling();
    let cc_of = |name: &str| {
        calling
            .named(name)
            .and_then(|one| one.cc.clone())
            .ok_or_else(|| hir::Unsupported(format!("calling.toml has no {name} with a cc")))
    };
    unit.cdecl_cc = Some(cc_of(&profile.cdecl)?).filter(|cc| cc != "cdecl");
    unit.registers_cc = Some(cc_of(&profile.registers)?).filter(|cc| cc != "cdecl");
    unit.entry = profile.entry.clone();
    unit.default_registers = profile.default_registers.clone();
    unit.hardware = profile.hardware.clone();
    unit.entry_cc = calling
        .named(&profile.entry_convention)
        .and_then(|one| one.cc.clone())
        .ok_or_else(|| hir::Unsupported(format!("calling.toml has no {} with a cc", profile.entry_convention)))?;
    unit.decorate(calling, codegen.object_format);
    // The front end was picked by the shim's flat flag; the target says
    // what flat is.
    if unit.flat != codegen.arch.layout().spaces.far_is_near() {
        return Err(hir::Unsupported(format!(
            "the front end is {} but target {} is {}",
            if unit.flat { "flat" } else { "segmented" },
            codegen.arch.name(),
            if unit.flat { "segmented" } else { "flat" }
        ))
        .into());
    }
    Ok((unit, profile))
}

/// [`selected`], each function checking its stack as `stack_check` says
/// (`-fsanitize=stack`).
pub fn selected_checking(
    text: &str,
    module: &str,
    dump: Option<&Path>,
    codegen: &llrm_codegen::driver::Options,
    stack_check: Option<llrm_codegen::hir::model::StackCheck>,
    wrapv: bool,
    entry_requests: bool,
) -> Result<masm::Module, CompileError> {
    let mut library = Vec::new();
    let program = llrm_codegen::support::debug::timed("frontend translate", || -> Result<_, CompileError> {
        let (unit, profile) = prepared(text, codegen, wrapv, entry_requests)?;
        library = library_routines(codegen, &profile);
        let program = translate::program(&unit, module, codegen.arch.calling(), &profile)?;
        for warning in unit.warnings.borrow().iter() {
            eprintln!("{module}: {warning}");
        }
        Ok(llrm_codegen::hir::model::Program { stack_check, ..program })
    })?;
    let mut options = codegen.clone();
    options.pipeline.library = library;
    if let Some(dump) = dump {
        fs::create_dir_all(dump)?;
        options.dump = Some(dump.to_path_buf());
    }
    Ok(llrm_codegen::driver::compiled(&program, &options)?.pop().expect("one module"))
}

/// The library routines the program is linked with under this compile's ABI
/// (the pin's `TargetLibraryInfo`, which reads them from the triple): `strlen`
/// where the C convention is the System V one of the host's libc, spelled as
/// the ABI decorates it. DEVIATE D11 (see `pipeline::Options::library`): no
/// other ABI states it, so no pass forms a call of it there (Open Watcom's
/// register convention and the 16-bit runtimes are not proven to link it).
fn library_routines(
    codegen: &llrm_codegen::driver::Options,
    profile: &Profile,
) -> Vec<llrm_mir::libfunc::Routine> {
    let calling = codegen.arch.calling();
    let Some(convention) = calling.named(&profile.cdecl).filter(|one| one.cc.as_deref() == Some("sysv")) else {
        return Vec::new();
    };
    let (Some(number), Some(pattern)) = (
        llrm_target::calling::Calling::number_of(convention.cc.as_deref()),
        convention.symbol.get(codegen.object_format),
    ) else {
        return Vec::new();
    };
    vec![llrm_mir::libfunc::Routine {
        standard: "strlen",
        symbol: pattern.replace('*', "strlen"),
        calling_convention: number,
    }]
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
    /// `--os-layer FIELD`: what the target's OS layer says of C's runtime,
    /// printed instead of compiling.
    os_layer: Option<Result<String, String>>,
    flags: Flags,
    dump: Option<PathBuf>,
    include: Vec<String>,
    /// The target, whose OS layer header a program that includes it is given
    /// (`os_header_directory`).
    target: std::rc::Rc<dyn llrm_target::Target>,
    /// Watcom switches the program opts into, passed to wccq: -oa, -on, and
    /// the preprocessor's -D and -U as its `-d` and `-u`.
    watcom: Vec<String>,
    /// `-fentry-requests`: the module that defines the entry asks its linker
    /// for the target's start-up (`entry_requests` of its description), as
    /// Open Watcom's compiler does for a program linked with its library.
    entry_requests: bool,
    /// The target, the built-in DOS on a 386 unless `--machine` names
    /// another, and the pipeline.
    codegen: llrm_codegen::driver::Options,
}

/// The code-generator stream wccq records for one C file; with `debug`,
/// its debug types and symbols too (-d2).
/// How the target asks Open Watcom's front end to record C: the `[frontend]` of
/// its C runtime description (`runtime/c/<target>/c.toml`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    /// The front end's tree: its CPU, `i86` or `386`.
    pub cpu: String,
    /// The convention whose contract a call to a routine that states none has
    /// under this ABI, by its name in `calling.toml`.
    pub convention: String,
    /// The routine the runtime's start calls, and the convention it calls it in
    /// by its name in `calling.toml`.
    pub entry: String,
    pub entry_convention: String,
    /// What it records of a function it passes in its default registers: its
    /// own list (`[ff:0]`), which differs by tree.
    pub default_registers: String,
    /// The ABI the front end is asked for: what it is run with, and what its
    /// records mean in `calling.toml`'s conventions: one it records as
    /// cdecl, and one it records in its default registers.
    pub flags: Vec<String>,
    /// A long double's size in memory, the target's
    /// (`Layout::long_double_bytes`): what `__SIZEOF_LONG_DOUBLE__` says.
    pub long_double_bytes: u64,
    pub cdecl: String,
    pub registers: String,
    /// The header it includes first, from the repository root.
    pub header: String,
    /// An interrupt handler's parameters, in order: the registers its frame
    /// holds, by name. None where the target has no interrupt handlers.
    pub interrupt_parameters: Vec<String>,
    /// Open Watcom's registers, for inline code that names them.
    pub hardware: crate::hwreg::Registers,
    /// Symbols a module that defines the entry asks its linker for, and, where
    /// the entry takes parameters, more: Open Watcom's `_cstart_` and `__argc`
    /// (its start-up, and what builds `argv`).
    pub entry_requests: Vec<String>,
    pub entry_argument_requests: Vec<String>,
    /// Segments the target's linker lays out between two markers, as
    /// `BEGIN:MIDDLE:END` (`XIB:XI:XIE`).
    pub bracketed_segments: Vec<String>,
    /// Whether a block's locals are marked with `llvm.lifetime` (the pipeline
    /// optimizes: clang's `ShouldEmitLifetimeMarkers`).
    pub lifetimes: bool,
    /// Whether `__builtin_expect` is the `llvm.expect` intrinsic, which
    /// `lower-expect` turns into branch weights: clang's at -O1 and up; below
    /// it is its argument.
    pub expects: bool,
}

impl Profile {
    /// The target's profile under its default ABI.
    pub fn of(target: &dyn llrm_target::Target) -> Result<Self, String> {
        Self::for_abi(target, None)
    }

    /// The profile under `-mabi=family`, the target's default without.
    pub fn for_abi(
        target: &dyn llrm_target::Target,
        family: Option<&str>,
    ) -> Result<Self, String> {
        let description = target.runtime("c").ok_or_else(|| format!("target {} has no C runtime", target.name()))?;
        let calling = target.calling();
        calling.chosen(family)?;
        let mut profile = Self::parse(&*description.table()?, family.unwrap_or(&calling.default))?;
        profile.long_double_bytes = target.layout().long_double_bytes();
        // A long double is x87's 80-bit type at every target whose layout has
        // one, whatever the ABI's other switches are: without `-fld`
        // the front end makes it a double.
        if target.layout().long_double_bytes() >= u64::from(FloatKind::X86Fp80.value_bytes())
            && !profile.flags.iter().any(|one| one == "-fld")
        {
            profile.flags.push("-fld".to_owned());
        }
        Ok(profile)
    }

    pub fn parse(
        table: &toml::Table,
        family: &str,
    ) -> Result<Self, String> {
        let frontend = table
            .get("frontend")
            .and_then(toml::Value::as_table)
            .ok_or("the C runtime description has no [frontend]")?;
        let text = |from: &toml::Table, key: &str| {
            from.get(key)
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("frontend.{key} is not a string"))
        };
        let abi = frontend
            .get("abi")
            .and_then(|one| one.get(family))
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("the C runtime description has no [frontend.abi.{family}]"))?;
        let flags = abi
            .get("flags")
            .and_then(toml::Value::as_array)
            .ok_or("frontend.abi.flags is not a list")?
            .iter()
            .map(|one| one.as_str().map(str::to_owned).ok_or("a frontend flag is not a string"))
            .collect::<Result<_, _>>()?;
        let interrupt_parameters = match frontend.get("interrupt_parameters") {
            None => Vec::new(),
            Some(list) => list
                .as_array()
                .ok_or("frontend.interrupt_parameters is not a list")?
                .iter()
                .map(|one| one.as_str().map(str::to_owned).ok_or("an interrupt parameter is not a string"))
                .collect::<Result<_, _>>()?,
        };
        let names = |key: &str| -> Result<Vec<String>, String> {
            abi.get(key)
                .map(|list| {
                    list.as_array()
                        .ok_or_else(|| format!("frontend.abi.{key} is not a list"))?
                        .iter()
                        .map(|one| {
                            one.as_str().map(str::to_owned).ok_or_else(|| format!("a {key} name is not a string"))
                        })
                        .collect()
                })
                .unwrap_or(Ok(Vec::new()))
        };
        Ok(Self {
            bracketed_segments: names("bracketed_segments")?,
            entry_requests: names("entry_requests")?,
            entry_argument_requests: names("entry_argument_requests")?,
            cpu: text(frontend, "watcom_cpu")?,
            convention: text(abi, "convention")?,
            entry: text(frontend, "entry")?,
            entry_convention: text(frontend, "entry_convention")?,
            default_registers: text(frontend, "default_registers")?,
            flags,
            long_double_bytes: 0,
            cdecl: text(abi, "cdecl")?,
            registers: text(abi, "registers")?,
            // A family may state its own, as Open Watcom's, which has nothing
            // of Borland's to undo.
            header: text(abi, "header").or_else(|_| text(frontend, "header"))?,
            interrupt_parameters,
            lifetimes: false,
            expects: false,
            // Only a target that has inline code naming registers states them.
            hardware: match frontend.get("registers").and_then(toml::Value::as_table) {
                Some(table) => crate::hwreg::Registers::parse(table)?,
                None => crate::hwreg::Registers::default(),
            },
        })
    }
}

/// `recorded` for real mode, the target the tests of the recorded streams are
/// for.
#[cfg(test)]
pub fn recorded(
    source: &Path,
    includes: &[String],
    debug: bool,
    watcom: &[&str],
) -> Result<String, hir::Unsupported> {
    recorded_for(
        source,
        includes,
        debug,
        watcom,
        &Profile::of(&llrm_x86_m16::M16).expect("real mode has a C front end profile"),
    )
}

/// The Open Watcom front end of each CPU, carried in the binary.
const WCCQ: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/wccq.rs"));

/// The headers the front end is forced to include, by file name.
const HEADERS: &[(&str, &str)] = &[
    ("borland.h", include_str!("borland.h")),
    ("flat.h", include_str!("flat.h")),
    ("watcom.h", include_str!("watcom.h")),
    ("owkeywords.h", include_str!("owkeywords.h")),
];

/// `run`, again while the image just written is held open for writing by a
/// forked child of another thread (ETXTBSY, until that child execs).
fn exec_busy(run: impl Fn() -> std::io::Result<std::process::Output>) -> std::io::Result<std::process::Output> {
    let mut tries = 0;
    loop {
        match run() {
            Err(error) if error.raw_os_error() == Some(26) && tries < 50 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

fn write_executable(
    path: &Path,
    image: &[u8],
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, image)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

/// `source` recorded by the front end of `profile`'s tree, asked as it says.
pub fn recorded_for(
    source: &Path,
    includes: &[String],
    debug: bool,
    watcom: &[&str],
    profile: &Profile,
) -> Result<String, hir::Unsupported> {
    let unbuilt = || hir::Unsupported("llrm was built without the toolchain feature".into());
    let image = WCCQ.iter().find(|(cpu, _)| *cpu == profile.cpu).map(|(_, image)| *image).ok_or_else(unbuilt)?;
    let failed = |detail: String| hir::Unsupported(format!("wccq failed on {}:\n{detail}", source.display()));
    let scratch = tempfile::tempdir().map_err(|error| failed(error.to_string()))?;
    for (name, text) in HEADERS {
        fs::write(scratch.path().join(name), text).map_err(|error| failed(error.to_string()))?;
    }
    let wccq = scratch.path().join("wccq");
    write_executable(&wccq, image).map_err(|error| failed(error.to_string()))?;
    let header = Path::new(&profile.header).file_name().and_then(|name| name.to_str()).unwrap_or_default();
    // One forced include (a second `-fi=` replaces the first): what GCC
    // predefines and programs test (`__INT_MAX__`, `__SIZE_TYPE__`,
    // `__BYTE_ORDER__`), as the target's sizes make them, and then the target's
    // own header.
    let forced = scratch.path().join("predefined.h");
    let text = format!(
        "{}#include \"{}\"\n",
        crate::predefined::header(profile.cpu == "386", profile.long_double_bytes),
        scratch.path().join(header).display()
    );
    fs::write(&forced, text).map_err(|error| failed(error.to_string()))?;
    let header = format!("-fi={}", forced.display());
    // `-funsigned-char` takes the ABI's `-j` (a signed char) away;
    // `-fsigned-char` is `-j`.
    let unsigned = watcom.contains(&"-funsigned-char");
    let watcom: Vec<&str> = watcom.iter().copied().filter(|one| *one != "-funsigned-char").collect();
    let flags: Vec<&str> = profile
        .flags
        .iter()
        .map(String::as_str)
        .filter(|one| !(unsigned && *one == "-j"))
        .chain([header.as_str()])
        .collect();
    let out = scratch.path().join("unit.cgs");
    let absolute = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let searched: Vec<String> =
        includes.iter().map(|one| format!("-I{}", absolute(Path::new(one)).display())).collect();
    // In the scratch directory, where wccq also leaves its .err file.
    let run = || {
        std::process::Command::new(&wccq)
            .args(&flags)
            .args(debug.then_some("-d2"))
            .args(&watcom)
            .args(&searched)
            .arg(format!("-fo={}/unit.obj", scratch.path().display()))
            .arg(absolute(source))
            .env("QBOPT_CG_STREAM", &out)
            .current_dir(scratch.path())
            .output()
    };
    let done = exec_busy(run).map_err(|error| failed(error.to_string()))?;
    if !done.status.success() || !out.exists() {
        let text = String::from_utf8_lossy(&done.stdout).into_owned() + &String::from_utf8_lossy(&done.stderr);
        return Err(failed(text));
    }
    fs::read_to_string(&out).map_err(|error| failed(error.to_string()))
}

fn usage() -> String {
    format!("usage: llrm-c [-h] [-I INCLUDE] [--dump DUMP] [--os-layer FIELD] {} source", flags::USAGE)
}

/// A directory holding `target`'s OS layer header as `llrm_os.h`.
fn os_header_directory(target: &dyn llrm_target::Target) -> Option<tempfile::TempDir> {
    let text = target.os_layer()?.report(&target.runtime("c")?, "header").ok()?;
    let directory = tempfile::tempdir().ok()?;
    fs::write(directory.path().join("llrm_os.h"), text + "\n").ok()?;
    Some(directory)
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let (mut source, mut flags, mut dump) = (None, Flags::default(), None);
    let (mut include, mut watcom) = (Vec::new(), Vec::new());
    let mut os_layer = None;
    let mut entry_requests = false;
    let mut at = 0;
    while at < argv.len() {
        // The language's, not the driver's: whether a plain char is signed.
        match argv[at].as_str() {
            "-fsigned-char" => {
                watcom.push("-j".to_owned());
                at += 1;
                continue;
            }
            "-funsigned-char" => {
                watcom.push("-funsigned-char".to_owned());
                at += 1;
                continue;
            }
            _ => {}
        }
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
            "-fentry-requests" => entry_requests = true,
            // Watcom's: relaxed alias checking; relaxed floating point.
            "-oa" => watcom.push("-oa".to_owned()),
            "-on" => watcom.push("-on".to_owned()),
            // The value is joined (`-DX=1`) or the next argument (`-D X=1`).
            "-D" | "-U" => {
                let text = value(argument)?;
                watcom.push(format!("-{}{text}", argument[1..].to_lowercase()));
            }
            // The language standard, as the front end's `-zastd=`.
            flag if flag.starts_with("-std=") => watcom.push(format!("-zastd={}", &flag[5..])),
            flag if flag.starts_with("-I") => include.push(flag[2..].to_owned()),
            flag if flag.starts_with("-D") || flag.starts_with("-U") => {
                watcom.push(format!("-{}{}", flag[1..2].to_lowercase(), &flag[2..]));
            }
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
    let os_layer =
        os_layer.map(|field| {
            bound
                .target
                .os_layer()
                .ok_or_else(|| "this target has no OS layer".to_owned())
                .and_then(
                    |layer| layer.report(&bound.target.runtime("c").ok_or("this target has no C runtime")?, &field),
                )
        });
    let machine = flags.machine(&*bound.target, bound.target.machine())?;
    let target = bound.target.clone();
    Ok(Args {
        source,
        os_layer,
        dump,
        include,
        target,
        watcom,
        entry_requests,
        codegen: {
            let options = bound.options(&flags, machine);
            let libcalls = libcalls(&*bound.target, options.abi.as_deref(), options.object_format)?;
            llrm_codegen::driver::Options { rounds_float_spills: true, libcalls, ..options }
        },
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
            llrm_codegen::support::debug::timed("frontend wccq", || {
                // The target's physical addresses reach the program as
                // `PHYSICAL_<NAME>`.
                let defines: Vec<String> = args
                    .codegen
                    .arch
                    .physical_addresses()
                    .iter()
                    .map(|(name, address)| format!("-dPHYSICAL_{}=0x{address:X}UL", name.to_uppercase()))
                    .collect();
                let switches: Vec<&str> =
                    args.watcom.iter().map(String::as_str).chain(defines.iter().map(String::as_str)).collect();
                let profile =
                    Profile::for_abi(&*args.codegen.arch, args.codegen.abi.as_deref()).map_err(hir::Unsupported)?;
                let record =
                    |include: &[String]| recorded_for(&args.source, include, args.flags.debug, &switches, &profile);
                let first = record(&args.include);
                // `#include "llrm_os.h"` needs no `-I`: a program that includes
                // it, itself or through another header, is
                // recorded again with the header's directory searched last. The
                // others pay nothing for it.
                match &first {
                    Err(error) if error.0.contains("llrm_os.h") => match os_header_directory(&*args.target) {
                        Some(directory) => {
                            let include = [&args.include[..], &[directory.path().display().to_string()]].concat();
                            record(&include)
                        }
                        None => first,
                    },
                    _ => first,
                }
            })?
        };
        let output = args.flags.output.clone().unwrap_or_else(|| args.source.with_extension("asm"));
        let module = args.source.file_stem().and_then(|one| one.to_str()).unwrap_or_default();
        let format = args.flags.format(&*args.codegen.arch)?;
        let spelled = llrm_codegen::driver::Options { object_format: format.name(), ..args.codegen.clone() };
        let built = selected_checking(
            &text,
            module,
            args.dump.as_deref(),
            &spelled,
            args.flags.sanitize.stack.then(|| stack_check(&*args.codegen.arch)),
            args.flags.wrapv,
            args.entry_requests,
        )?;
        let name = args.source.file_name().and_then(|one| one.to_str()).unwrap_or_default();
        if !args.flags.assembly
            && matches!(
                output.extension().and_then(|one| one.to_str()).map(str::to_lowercase).as_deref(),
                Some("obj" | "o")
            )
        {
            let bytes = objbuild::written_in(&built, name, objbuild::CodeLayout::OneSegment, format)?;
            llrm_codegen::support::debug::timed("write output", || {
                llrm_codegen::driver::flags::write_output(&output, bytes)
            })?;
        } else {
            llrm_codegen::driver::flags::write_output(&output, masm::text(&built)?)?;
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
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, fixture, None, &llrm_driver::m16_options_old(machine)).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
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

    /// `-fsanitize=stack` compares with the word and calls the routine
    /// `stack.toml` names, and the C start-up the tests link defines both:
    /// a description naming a word start-up never fills would compare with
    /// zero. The default build checks nothing.
    /// Embedding wccq made 13 parallel compiles fail with "Text file busy": a
    /// thread forked while another held the image open for writing. The run is
    /// asked again.
    #[test]
    fn test_a_run_refused_as_text_file_busy_is_asked_again() {
        let calls = std::cell::Cell::new(0);
        let run = || {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err(std::io::Error::from_raw_os_error(26))
            } else {
                std::process::Command::new("true").output()
            }
        };
        assert!(super::exec_busy(run).unwrap().status.success());
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn the_stack_check_names_what_the_c_runtime_defines() {
        let root = Path::new(env!("LLRM_ROOT"));
        let check = super::stack_check(&llrm_x86_m16::M16);
        // The assembly is laid out in columns: its words are what is compared.
        let runtime = |name: &str| {
            let text = std::fs::read_to_string(root.join("runtime").join(name)).unwrap();
            text.lines()
                .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            runtime("shared/dos/m16/os.asm").contains(&format!("public {}", check.limit))
                && runtime("shared/dos/m16/start.asm").contains(&format!("mov {}, ax", check.limit))
        );
        assert!(runtime("c/x86-m16/ext.asm").contains(&format!("public {}", check.handler)));
        let text = std::fs::read_to_string(root.join("tests/fixtures/c/anims.cgs")).unwrap();
        let options = llrm_driver::m16_options_old(llrm_codegen::abi::machine::Machine {
            cpu: "486".to_owned(),
            ..llrm_x86_m16::machine::BUILT_IN.clone()
        });
        let listing = |check| {
            llrm_codegen::backend::masm::text(
                &super::selected_checking(&text, "anims", None, &options, check, false, false).unwrap(),
            )
            .unwrap()
        };
        let named = llrm_codegen::hir::model::StackCheck { limit: "FOO".into(), handler: "BAR".into(), ..check };
        let checked = listing(Some(named));
        assert!(
            checked.contains("cmp sp, word ptr FOO")
                && checked.contains("call far ptr BAR")
                && !checked.contains("LL$STACK_LOW"),
            "{checked}"
        );
        assert!(!listing(None).contains("cmp sp"));
    }

    /// The first line of a loop in `body`: a label some later conditional jump
    /// returns to (the exit test is `jne`, `jb` or `jl` by what the loop
    /// counts).
    fn loop_start<S: AsRef<str>>(body: &[S]) -> Option<usize> {
        body.iter().position(|line| {
            let label = line.as_ref().trim_end_matches(':');
            line.as_ref().starts_with('L')
                && line.as_ref().ends_with(':')
                && body
                    .iter()
                    .skip_while(|one| one.as_ref() != line.as_ref())
                    .any(|one| one.as_ref().starts_with('j') && one.as_ref().ends_with(&format!(" {label}")))
        })
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

    /// Two arrays off one stepped offset beside the counter: two steps by
    /// constants, one by the element size (2) and one by one (the
    /// counter, `inc`, or counting to zero `add -1` with no compare), and at
    /// most one compare. A pointer per array is a third step.
    fn shares_one_stepped_offset(counting: &[String]) {
        let steps: Vec<&String> = counting.iter().filter(|one| !one.starts_with("cmp ")).collect();
        let compares = counting.len() - steps.len();
        let by = |one: &&String, amount: &str| {
            one.starts_with("inc ") && amount == "1"
                || one.starts_with("dec ") && amount == "-1"
                || (one.starts_with("add ") || one.starts_with("sub ")) && one.ends_with(&format!(", {amount}"))
        };
        let counter = steps.iter().any(|one| by(one, "1") || by(one, "-1"));
        let offset = steps.iter().any(|one| by(one, "2"));
        assert!(steps.len() == 2 && counter && offset && compares <= 1, "{counting:?}");
    }

    /// `dot` indexes `a[i]` and `b[i]`: both arrays share one stepped offset
    /// beside the counter, as `opt -passes=loop-reduce` + `llc -mcpu=i486`
    /// list it (`inc; add 2; cmp`), not a pointer per array.
    #[test]
    fn test_addresses_differing_by_base_share_one_stepped_offset() {
        shares_one_stepped_offset(&loop_counting("dot"));
    }

    /// `bytes` indexes by `i` itself, with a bound only known at run time: the
    /// counter is the offset and is compared with `n`, as llc lists it
    /// (`inc; cmp`).
    #[test]
    fn test_a_counter_read_only_as_offsets_counts_to_zero() {
        // The counter is the offset: one step by one and one compare with `n`
        // (a register or its stack slot), nothing else.
        let counting = loop_counting("bytes");
        assert!(
            counting.len() == 2 && counting[0].starts_with("inc ") && counting[1].starts_with("cmp "),
            "{counting:?}"
        );
    }

    /// `from1` reads `a[i]` and `b[i - 1]`: `(i - 1) * 2` folds into the
    /// displacement (`[bx+di-2]`), so both arrays index off `i * 2` beside
    /// one counter, as `opt` + `llc -mcpu=i486` list it.
    #[test]
    fn test_subscripts_of_one_stride_share_one_offset() {
        shares_one_stepped_offset(&loop_counting("from1"));
        let loop_ = innermost("from1");
        let two_registers = |one: &String| {
            let inside = one.split_once("ptr [").map_or("", |(_, inside)| inside).as_bytes();
            inside.len() > 4 && inside[2] == b'+' && inside[3].is_ascii_alphabetic()
        };
        assert!(
            loop_.iter().filter(|one| one.starts_with("movsx") && one.contains("ptr [")).all(two_registers),
            "{loop_:#?}"
        );
    }

    /// tests/fixtures/c/`path`.cgs's MIR as the front end emits it.
    fn emitted(path: &str) -> String {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{path}.cgs"))).unwrap();
        let program = crate::translate::program(
            &crate::hir::unit(&crate::stream::parse(&text)).unwrap(),
            "t",
            llrm_target::Target::calling(&llrm_x86_m16::M16),
            &crate::compile::Profile::of(&llrm_x86_m16::M16).unwrap(),
        )
        .unwrap();
        let options = llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone());
        let (mir, _) = llrm_codegen::driver::emitted(&program, &options).unwrap();
        llrm_mir::print::module(&mir.modules[0])
    }

    /// tests/fixtures/c/`path`.cgs compiled.
    fn compiles(path: &str) {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{path}.cgs"))).unwrap();
        let options = llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone());
        super::selected(&text, "t", None, &options).unwrap_or_else(|error| panic!("{path}: {error}"));
    }

    /// OW parsed restrict but discarded it before CG; the shim now records it.
    #[test]
    fn test_restrict_reaches_mir_as_distinct_noalias_roots() {
        let text = emitted("tests/test_restrict_reaches_mir_as_distinct_noalias_roots");
        let define = text.lines().find(|line| line.starts_with("define ")).expect("a definition");
        // The attribute order is the printer's; each pointer is also `noundef`
        // (CGCall.cpp `DetermineNoUndef`: a scalar parameter).
        let on_pointers = |attribute: &str| define.split(", ").filter(|one| one.contains(attribute)).count();
        assert_eq!((on_pointers("noalias"), on_pointers("noundef")), (3, 3), "{text}");
    }

    /// `ls_animate(&ls, 0.05f)` pushes the single's four bytes; CGFloat was
    /// refused.
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
    /// at `no scalar width for T51` instead of laying its 12 bytes on the
    /// stack.
    #[test]
    fn test_aggregate_argument_is_pushed_by_value() {
        compiles("tests/test_aggregate_argument_is_pushed_by_value");
    }

    /// `name`'s listing of `function`, compiled from
    /// tests/fixtures/c/`name`.cgs.
    fn listing_of(
        name: &str,
        function: &str,
    ) -> String {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{name}.cgs"))).unwrap();
        let options = llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone());
        let built = super::selected(&text, name, None, &options).unwrap_or_else(|error| panic!("{error:?}"));
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        let start = listing.find(&format!("{function} proc")).expect("the function");
        listing[start..start + listing[start..].find("endp").unwrap()].to_owned()
    }

    /// A table of function pointers initialised with integer constants
    /// (`SIG_IGN`, `SIG_DFL`), in a model with far code, had 2-byte entries
    /// where code pointers are 4: clib's signal table was half its size and
    /// `signal` answered the wrong previous handler.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_code_pointer_constant_in_data_is_as_wide_as_the_model_says() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("table.c");
        std::fs::write(
            &source,
            "typedef void (*handler)(int);\nhandler table[3] = { (handler)1, (handler)2, (handler)0 };\n",
        )
        .unwrap();
        let recorded = super::recorded(&source, &[], false, &[]).expect("wccq records table.c");
        let data: Vec<&str> = recorded.lines().filter(|line| line.contains("DGInteger")).collect();
        assert_eq!(data.len(), 3, "{recorded}");
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records table.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "table", None, &options).unwrap()).unwrap();
        let bytes: usize = asm
            .lines()
            .skip_while(|one| *one != "_table label byte")
            .skip(1)
            .take_while(|one| one.starts_with("db "))
            .map(|one| one[3..].split(',').count())
            .sum();
        assert_eq!(bytes, 12, "{asm}");
    }

    /// `-DNAME=value`, `-UNAME` and `-Idir` were refused as unrecognized
    /// arguments, so no source that selects its code by a macro the build
    /// sets (a library's `-D__DOS__`) could be compiled. The front end must
    /// see them.
    #[test]
    fn test_defines_and_undefines_reach_the_front_end() {
        let argv: Vec<String> =
            ["-DA=3", "-D", "B", "-UC", "-U", "D", "-Idir", "-I", "other", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        assert_eq!(args.watcom, ["-dA=3", "-dB", "-uC", "-uD"]);
        assert_eq!(args.include, ["dir", "other"]);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("d.c");
        std::fs::write(
            &source,
            "#ifndef WANTED\n#error WANTED is not defined\n#endif\nint f(void) { return WANTED; }\n",
        )
        .unwrap();
        let profile = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        assert!(super::recorded_for(&source, &[], false, &[], &profile).is_err(), "premise: -error without -D");
        let made = super::recorded_for(&source, &[], false, &["-dWANTED=7"], &profile);
        assert!(made.is_ok(), "{made:?}");
    }

    /// Open Watcom's own ABI read Borland's header: `__BORLANDC__` was defined,
    /// `__segment` renamed away, `__STDC__` undone. Open Watcom's `malloc.h`
    /// (`__segment __seg`) and every clib source with a `__BORLANDC__` branch
    /// failed or took the wrong one. `-std=c99` is the front end's
    /// `-zastd=c99`.
    #[test]
    fn test_the_watcom_abi_has_no_borland_names_and_takes_a_language_standard() {
        let argv: Vec<String> = ["-std=c99", "x.c"].map(str::to_owned).to_vec();
        assert_eq!(super::parse_args(&argv).unwrap().watcom, ["-zastd=c99"]);
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("w.c");
        std::fs::write(
            &source,
            "#ifdef __BORLANDC__\n#error Borland\n#endif\n#if __STDC_VERSION__ < 199901L\n#error not c99\n#endif\n\
             int f(__segment s) { return (int)s; }\n",
        )
        .unwrap();
        let watcom = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let made = super::recorded_for(&source, &[], false, &["-zastd=c99"], &watcom);
        assert!(made.is_ok(), "{made:?}");
        let borland = super::Profile::for_abi(&llrm_x86_m16::M16, Some("cdecl")).unwrap();
        assert!(
            super::recorded_for(&source, &[], false, &["-zastd=c99"], &borland).is_err(),
            "premise: Borland's keeps its names"
        );
    }

    /// The front end predefined the build host's OS: a real-mode program saw
    /// `__LINUX__` and no `__DOS__`, so every source that picks its system
    /// calls by them (clib's `handleio`, `file`) took the Linux branch.
    #[test]
    fn test_real_mode_sources_see_dos_and_not_the_build_host() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("os.c");
        std::fs::write(
            &source,
            "#ifndef __DOS__\n#error not DOS\n#endif\n#ifdef __LINUX__\n#error Linux\n#endif\nint x;\n",
        )
        .unwrap();
        for family in ["cdecl", "watcom", "ia16", "regparm3"] {
            let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some(family)).unwrap();
            let made = super::recorded_for(&source, &[], false, &[], &profile);
            assert!(made.is_ok(), "{family}: {made:?}");
        }
    }

    /// Inline code that says its registers (`#pragma aux f = "..." __parm
    /// [__dx]`) was laid down with its parameters dropped: Open Watcom's
    /// `cgets` ran `int 21h` function 0Ah on whatever `dx` held. Each
    /// parameter is in its register before the bytes.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_inline_code_gets_its_parameters_in_the_registers_it_names() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("regs.c");
        std::fs::write(
            &source,
            "extern void buffered(char *s);\n\
             #pragma aux buffered = \"mov ah,0ah\" \"int 21h\" __parm __caller [__dx] __value __modify __exact [__ah]\n\
             extern unsigned char add2(unsigned char a, unsigned char b);\n\
             #pragma aux add2 = \"add al,bl\" __parm [__al] [__bl] __value [__al] __modify __exact [__al]\n\
             char buf[10];\n\
             void fill(void) { buffered(buf); }\n\
             unsigned char sum(unsigned char x, unsigned char y) { return add2(x, y); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records regs.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let built = super::selected(&text, "regs", None, &options).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let before = |function: &str, bytes: &str, register: &str| {
            let from = asm.find(&format!("{function} proc")).expect("the function");
            let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
            let at = body.iter().position(|one| one.starts_with(bytes)).expect("the inline bytes");
            body[..at]
                .iter()
                .any(
                    |one| one.strip_prefix("mov ")
                        .or_else(|| one.strip_prefix("movzx "))
                        .is_some_and(|rest| rest.starts_with(register)),
                )
        };
        assert!(before("fill_", "db 0b4h,00ah", "dx,"), "{asm}");
        assert!(before("sum_", "db 002h,0c3h", "bx,"), "{asm}");
    }

    /// A Borland `__asm { fld x }` block naming frame places has a save set of
    /// its own, which took it for a pragma with registers: qcport's `_fsin`
    /// and `_ftol_short` did not compile ("F.0 is inline code with registers
    /// and a symbol reference"), found by CI, not by the committed streams.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_an_asm_block_naming_frame_places_is_not_a_pragma_with_registers() {
        let source = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/inlinecode.c");
        let profile = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records inlinecode.c");
        let options = llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone());
        super::selected(&text, "inlinecode", None, &options).expect("compiles");
    }

    /// Inline code that lowers SP (`alloca`) was refused ("modifies sp"),
    /// and once let through returned with SP where it ended: the
    /// saved registers popped from the wrong place. The return sets
    /// SP from the frame register.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_inline_code_that_lowers_sp_returns_with_the_frame_restored() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("grow.c");
        std::fs::write(
            &source,
            "extern void *grow(unsigned n);\n#pragma aux grow = \"sub sp,ax\" __parm __nomemory [__ax] __value [__sp] __modify __exact __nomemory [__sp]\nint use(unsigned n) { char *b = grow(n); b[0] = 1; return b[0]; }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records grow.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "grow", None, &options).unwrap()).unwrap();
        let (lea, pop) = (asm.find("lea sp").expect(&asm), asm.find("pop bp").expect(&asm));
        assert!(lea < pop, "{asm}");
    }

    /// A function with `alloca` returns by setting SP from the frame
    /// register and popping every register it kept, so all are
    /// pushed at the entry. The heap test's
    /// `Test_alloca_stackavail__memavl__memmax` pushed SI after the
    /// first allocation (shrink-wrapping) and popped it on every return:
    /// the program jumped to the stack.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_function_with_alloca_pushes_what_it_keeps_at_its_entry() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("shrink.c");
        std::fs::write(&source, include_str!("../../../../tests/fixtures/c/alloca_shrink.c")).unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records shrink.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm =
            llrm_codegen::backend::masm::text(&super::selected(&text, "shrink", None, &options).unwrap()).unwrap();
        let (entry, body) = asm.split_once("L0_0:").expect(&asm);
        // `pop es` after `pushw DGROUP` is the body's own, and merged returns
        // put the one epilogue after it.
        let kept = |text: &str, op: &str| {
            text.lines().filter(|one| one.trim().starts_with(op) && !one.contains("bp") && !one.contains("es")).count()
        };
        assert_eq!(kept(entry, "push"), kept(body.split("retf").next().unwrap(), "pop"), "{asm}");
    }

    /// Inline code that changes SI (`__modify [__si]`) in a function whose
    /// convention keeps SI for its caller: the function did not save it, so
    /// `_fstrrev` handed its caller back a corrupted SI.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_register_inline_code_changes_is_saved_by_the_function_that_keeps_it() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("keep.c");
        std::fs::write(
            &source,
            "extern void clobber(void);\n#pragma aux clobber = \"mov si,1\" __modify __exact [__si]\n\
             void run(void) { clobber(); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records keep.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "keep", None, &options).unwrap()).unwrap();
        assert!(asm.contains("push si") && asm.contains("pop si"), "{asm}");
    }

    /// Open Watcom's plain char is unsigned (its library is built with `-j`),
    /// Borland's signed: the Open Watcom family took Borland's, and clib's own
    /// tests compare `'\x81'` with an unsigned char and failed.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_plain_char_is_signed_or_not_as_the_abi_and_the_switch_say() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("sign.c");
        std::fs::write(&source, "int negative(void) { char c = '\\x81'; return c < 0; }\n").unwrap();
        let returns = |flags: &[&str]| {
            let mut argv: Vec<String> = flags.iter().map(|one| (*one).to_owned()).collect();
            argv.push("sign.c".to_owned());
            let args = super::parse_args(&argv).unwrap();
            let profile = super::Profile::for_abi(&*args.codegen.arch, args.codegen.abi.as_deref()).unwrap();
            let switches: Vec<&str> = args.watcom.iter().map(String::as_str).collect();
            let text = super::recorded_for(&source, &[], false, &switches, &profile).expect("wccq records sign.c");
            let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "sign", None, &args.codegen).unwrap())
                .unwrap();
            asm.contains("mov ax, 1")
        };
        assert!(!returns(&["-m16", "-mabi=watcom"]), "Open Watcom's char is unsigned");
        assert!(returns(&["-m16", "-mabi=watcom", "-fsigned-char"]));
        assert!(returns(&["-m16", "-mabi=cdecl"]), "Borland's char is signed");
        assert!(!returns(&["-m16", "-mabi=cdecl", "-funsigned-char"]));
    }

    /// Open Watcom's compiler leaves `_cstart_` in a module that defines `main`
    /// and `__argc` where `main` takes arguments: they pull in the start-up
    /// and the module that builds `argv`. Without them a program linked against
    /// the library had `argc` 0 and no `argv[0]`, and needed `_cstart_` named
    /// on the link line.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_module_with_main_asks_its_linker_for_the_start_up_and_for_argv() {
        let directory = tempfile::tempdir().unwrap();
        let listing = |name: &str, text: &str| {
            let source = directory.path().join(format!("{name}.c"));
            std::fs::write(&source, text).unwrap();
            let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
            let recorded = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records it");
            let options = llrm_codegen::driver::Options {
                abi: Some("watcom".to_owned()),
                ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
            };
            llrm_codegen::backend::masm::text(&super::selected_requesting(&recorded, name, None, &options).unwrap())
                .unwrap()
        };
        let argv = listing("withargs", "int main(int argc, char **argv) { return argc; }\n");
        assert!(argv.contains("extern _cstart_:near") && argv.contains("extern __argc:near"), "{argv}");
        let bare = listing("bare", "int main(void) { return 0; }\n");
        assert!(bare.contains("extern _cstart_:near") && !bare.contains("__argc"), "{bare}");
        // Not unless asked: a program linked with llrm's own runtime has no
        // `_cstart_`.
        let source = directory.path().join("plain.c");
        std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let recorded = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records it");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let plain =
            llrm_codegen::backend::masm::text(&super::selected(&recorded, "plain", None, &options).unwrap()).unwrap();
        assert!(!plain.contains("_cstart_"), "{plain}");
        let other = listing("other", "int helper(int a) { return a; }\n");
        assert!(!other.contains("_cstart_") && !other.contains("__argc"), "{other}");
    }

    /// A module with initializers in `XI` declared no `XIB` before it or `XIE`
    /// after, so the linker, meeting `XI` first, laid it out ahead of the
    /// begin marker: the start-up's range of initializers was empty and
    /// `argv` was never built.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_segment_between_markers_is_declared_with_them() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("brackets.c");
        std::fs::write(
            &source,
            "static void init(void) {}\n\
             static struct { char kind; void (*run)(void); } __based( __segname( \"XI\" ) ) entry = { 1, init };\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records brackets.c");
        let argv: Vec<String> = ["-m16", "-mabi=watcom", "brackets.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "brackets", None, &args.codegen).unwrap())
            .unwrap();
        let order: Vec<&str> = asm.lines().filter_map(|one| one.strip_suffix(" segment word public 'DATA'")).collect();
        assert_eq!(order, ["XIB", "XI", "XIE"], "{asm}");
    }

    /// The address of a function the unit only declares, taken in code and
    /// not in a table, stopped the compile ("@name undeclared"): the call
    /// declares its callee and the address did not.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_the_address_of_an_extern_function_taken_in_code_is_compiled() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("addr.c");
        std::fs::write(&source, "extern unsigned other(void);\nunsigned (*get(void))(void) { return other; }\n")
            .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records addr.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let built = super::selected(&text, "addr", None, &options);
        let asm = llrm_codegen::backend::masm::text(&built.unwrap()).unwrap();
        assert!(asm.contains("mov ax, offset other_") && asm.contains("extern other_:far"), "{asm}");
    }

    /// Inline code with registers that names a symbol (`"call __doserror1_"`,
    /// `"mov ax,offset table"`) was refused, which left clib's DOS calls to the
    /// reference compiler. A near call is spelled as the instruction, a far
    /// one and an offset as their words.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_inline_code_with_registers_may_name_symbols() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("sym.c");
        std::fs::write(
            &source,
            "extern void __near fixed(void);\nextern void __far reach(void);\nextern int table[4];\n\
             extern int step(int n);\n\
             #pragma aux step = \"int 21h\" \"call fixed\" \"call reach\" \"mov dx,offset table\" \
             __parm __caller [__ax] __value [__ax] __modify __exact [__ax __dx]\n\
             int run(int n) { return step(n); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records sym.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "sym", None, &options).unwrap()).unwrap();
        assert!(asm.contains("call near ptr fixed_"), "{asm}");
        assert!(asm.contains("dw offset reach_") && asm.contains("dw seg reach_"), "{asm}");
        assert!(asm.contains("dw offset _table"), "{asm}");
        assert!(asm.contains("extern fixed_:near") && asm.contains("extern reach_:far"), "{asm}");
    }

    /// Every module's code was in `<MODULE>_TEXT`, so a near call between
    /// modules (clib's `__set_errno_dos`, declared near) reached into another
    /// segment. `-mcode-segment=_TEXT` puts them all in one.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_code_segment_may_be_named_for_every_module() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("one.c");
        std::fs::write(&source, "int one(void) { return 1; }\n").unwrap();
        let argv: Vec<String> = ["-m16", "-mabi=watcom", "-mcode-segment=_TEXT", "one.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records one.c");
        let asm =
            llrm_codegen::backend::masm::text(&super::selected(&text, "one", None, &args.codegen).unwrap()).unwrap();
        assert!(asm.lines().any(|one| one == ".code _TEXT"), "{asm}");
    }

    /// A register list that spells the default convention's own assignment
    /// (`__parm [__ax] [__dx] [__bx]`, the heap's `__MemAllocator`) was refused
    /// as "has a register calling convention", and clib's malloc and free with
    /// it. A list that differs still is.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_register_list_that_is_the_default_assignment_is_the_default_convention() {
        let directory = tempfile::tempdir().unwrap();
        let build = |pragma: &str| {
            let source = directory.path().join("alloc.c");
            std::fs::write(
                &source,
                format!(
                    "extern void *allocator(unsigned a, void *b, unsigned c);\n{pragma}\n\
                     void *use(unsigned n) {{ return allocator(n, 0, 3); }}\n"
                ),
            )
            .unwrap();
            let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
            let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records alloc.c");
            let options = llrm_codegen::driver::Options {
                abi: Some("watcom".to_owned()),
                ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
            };
            super::selected(&text, "alloc", None, &options)
                .map(|built| llrm_codegen::backend::masm::text(&built).unwrap())
        };
        let same = build("#pragma aux allocator \"*\" __parm [__ax] [__dx] [__bx]").expect("the default's own list");
        assert!(same.contains("mov bx, 3") && same.contains("call far ptr allocator"), "{same}");
        let other = build("#pragma aux allocator \"*\" __parm [__dx] [__ax] [__bx]").expect_err("another list");
        assert!(format!("{other:?}").contains("register calling convention"), "{other:?}");
    }

    /// A branch on `a && b && c`, its compares of a pointer with the addresses
    /// of two functions, made each address in the block after the branch: the
    /// coalescer stopped on a value without a definition ("a unique value"),
    /// compiling clib's `sigsy`.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_branch_on_compares_with_two_function_addresses_makes_each_before_its_compare() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("cond.c");
        std::fs::write(
            &source,
            "extern int rwd;\nextern void (*exitp)(void);\nextern void nullexit(void);\nvoid restore(void) { }\n\
             extern void chain(void);\n\
             void h(void) { if (rwd == 0 && exitp != nullexit && exitp != restore) chain(); }\n",
        )
        .unwrap();
        let profile = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records cond.c");
        let options = llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone());
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "cond", None, &options).unwrap()).unwrap();
        assert!(asm.contains("call"), "{asm}");
    }

    /// Watcom's -oa lets a store through a pointer leave alone every object
    /// the unit never takes the address of: INIT `sw` said so, and llrm-c
    /// read only its debug bits, so `after_store` read `counter` and `seen`
    /// again after `*p = 5`. Without -oa they are read again, as C requires.
    #[test]
    fn test_relaxed_alias_checking_keeps_unaddressed_objects_across_a_store() {
        let reads =
            |listing: &str, name: &str| listing.lines().filter(|one| one.contains(&format!("ptr {name}"))).count();
        let strict = listing_of("strictalias", "_after_store");
        assert_eq!(
            (reads(&strict, "_counter"), reads(&strict, "_seen")),
            (2, 2),
            "premise: read again without -oa\n{strict}"
        );
        let relaxed = listing_of("relaxalias", "_after_store");
        assert_eq!((reads(&relaxed, "_counter"), reads(&relaxed, "_seen")), (1, 1), "{relaxed}");
    }

    /// A const object is read once across a call or a store: writing it is
    /// undefined, wherever its address went. An extern const was a plain
    /// global, and no analysis read `constant` as unwritable, so each was
    /// read again after `ext()`, `keep(tbl)` or `*p = 1`.
    #[test]
    fn test_a_const_object_is_read_once_across_a_call_or_a_store() {
        let reads = |function: &str, name: &str| {
            listing_of("constobj", function).lines().filter(|one| one.contains(&format!("ptr {name}"))).count()
        };
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
        match super::selected(
            &std::fs::read_to_string(path).unwrap(),
            "regs",
            None,
            &llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone()),
        ) {
            Err(CompileError::Unsupported(refused)) => {
                assert!(refused.to_string().contains("_twice has a register calling convention"), "{refused}");
            }
            _ => panic!("a register convention was compiled as a stack one"),
        }
    }

    /// toolchain/owshim/build.sh hardcoded macOS ARM64's defines and clang, so
    /// no wccq could be built on any other host and llrm-c refused every C
    /// file.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_wccq_built_here_records_the_committed_stream() {
        let root = Path::new(env!("LLRM_ROOT"));
        let source = root.join("tests/fixtures/c/halve.c");
        let without_path =
            |text: &str| text.lines().filter(|line| !line.contains("DBSrcFile")).collect::<Vec<_>>().join("\n");
        // The committed streams are Borland's, every function cdecl.
        let recorded = super::recorded_for(
            &source,
            &[],
            false,
            &[],
            &super::Profile::for_abi(&llrm_x86_m16::M16, Some("cdecl")).unwrap(),
        )
        .expect("wccq records halve.c");
        let committed = std::fs::read_to_string(root.join("tests/fixtures/c/halve.cgs")).unwrap();
        assert_eq!(without_path(&recorded), without_path(&committed));
    }

    /// `far_pointer == NULL`, where `NULL` is `((void *)0)`: the constant was
    /// converted from a near pointer, to the data segment's offset 0, so a far
    /// null pointer compared unequal to it (clib's `memtest` said so of
    /// `_fmemchr`; `signal`'s `SIG_DFL` likewise). A number used as a pointer
    /// keeps its bits, with segment 0.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_constant_number_used_as_a_far_pointer_has_segment_zero() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("null.c");
        std::fs::write(
            &source,
            "int isnull(void far *p) { return p == (void *)0; }\nint isdefault(void far *p) { return p == (void far *)2; }\n",
        )
        .unwrap();
        let profile = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records null.c");
        let options = llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone());
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "null", None, &options).unwrap()).unwrap();
        assert!(!asm.contains("DGROUP"), "{asm}");
    }

    /// Inline code taking a value in ES (`__parm [__es]`, a far pointer in
    /// `[__es __di]`) was refused: clib's DOS block calls (`_TinyFreeBlock`)
    /// and its far string functions (`_fmemmove`) were left to the reference
    /// compiler.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_inline_code_takes_and_answers_in_es() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("seg.c");
        std::fs::write(
            &source,
            "extern void setes(unsigned s);\n\
             #pragma aux setes = \"mov dl,1\" __parm [__es] __modify __exact [__dl]\n\
             extern void far *same(void far *p);\n\
             #pragma aux same = \"nop\" __parm [__es __di] __value [__es __di] __modify __exact []\n\
             void set(unsigned s) { setes(s); }\n\
             void far *pass(void far *p) { return same(p); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records seg.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "seg", None, &options).unwrap()).unwrap();
        let body = |function: &str| -> Vec<String> {
            let from = asm.find(&format!("{function} proc")).expect("the function");
            asm[from..].lines().map(|one| one.trim().to_owned()).take_while(|one| !one.ends_with("endp")).collect()
        };
        let set = body("set_");
        let at = set.iter().position(|one| one.starts_with("db 0b2h")).expect("the bytes");
        assert!(set[..at].iter().any(|one| one == "mov es, ax"), "{set:?}");
        let pass = body("pass_");
        let at = pass.iter().position(|one| one == "db 090h").expect("the bytes");
        assert!(pass[..at].iter().any(|one| one.starts_with("mov es, ")), "{pass:?}");
        assert!(pass[at..].iter().any(|one| one == "mov ax, es"), "{pass:?}");
    }

    /// `outp( int port, int value )` is `__parm [__dx] [__al]`: an int in a
    /// byte register, which clib's `outp` could not be built with.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_byte_register_takes_the_low_byte_of_an_int() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("port.c");
        std::fs::write(
            &source,
            "extern void out(int port, int value);\n\
             #pragma aux out = \"out dx,al\" __parm [__dx] [__al] __modify __exact []\n\
             void put(int port, int value) { out(port, value); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records port.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "port", None, &options).unwrap()).unwrap();
        let body: Vec<&str> = asm.lines().map(str::trim).skip_while(|one| *one != "put_ proc far").collect();
        let at = body.iter().position(|one| *one == "db 0eeh").expect("the out");
        assert!(body[..at].iter().any(|one| one.starts_with("mov dx, ")), "{body:?}");
        assert!(body[..at].iter().any(|one| one.starts_with("mov ax, ")), "{body:?}");
    }

    /// Inline code answering a small struct in registers (`div_t` in
    /// `[__dx __ax]`) was refused ("a 4-byte aggregate as a value"), and
    /// clib's `div`, `delay` and `__getctime` with it.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_inline_code_may_answer_a_small_struct() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("pair.c");
        std::fs::write(
            &source,
            "typedef struct { int quot; int rem; } pair;\nextern pair split(int a, int b);\n\
             #pragma aux split = \"cwd\" \"idiv cx\" __parm [__ax] [__cx] __value [__dx __ax] __modify __exact [__ax __dx]\n\
             pair run(int a, int b) { return split(a, b); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records pair.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "pair", None, &options).unwrap()).unwrap();
        assert!(asm.contains("db 099h,0f7h,0f9h") && asm.contains("retf"), "{asm}");
    }

    /// A 64-bit parameter in `[__ax __bx __cx __dx]` (the same set as the
    /// default list) was refused ("parameters in the default registers",
    /// "does not fit its registers"), as was the same set for the answer:
    /// `ulltoa` and `lltoa` did not compile. Open Watcom's compiler puts
    /// the top word in ax, the bottom in dx.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_quad_word_in_four_registers_has_its_top_word_in_ax() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("quad.c");
        std::fs::write(
            &source,
            "extern void put(unsigned long long);\n#pragma aux put = \"nop\" __parm [__ax __bx __cx __dx] __modify [__ax __bx __cx __dx]\n\
             extern unsigned long long get(void);\n#pragma aux get = \"nop\" __value [__ax __bx __cx __dx] __modify [__ax __bx __cx __dx]\n\
             unsigned long long hold;\nvoid use(void) { put(0x0001000200030004ULL); hold = get(); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records quad.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "quad", None, &options).unwrap()).unwrap();
        let loads = ["mov ax, 1", "mov bx, 2", "mov cx, 3", "mov dx, 4"].map(|one| asm.find(one).expect(&asm));
        assert!(loads.windows(2).all(|two| two[0] < two[1]), "{asm}");
    }

    /// `__parm __caller []` inline code (`EnterDebuggerWithMessage`) finds its
    /// argument on the stack and the caller takes it back. It was refused ("a
    /// parameter on the stack"): `assert`, `dbgrpt` and `enterdb` did not
    /// compile.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_inline_code_finds_its_argument_on_the_stack_and_the_caller_pops_it() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("brk.c");
        std::fs::write(
            &source,
            "extern void brk(const char __far *);\n#pragma aux brk __parm __caller [] = \"int 3\" \"jmp short L1\" 'W' 'V' \"L1:\"\n\
             void t(const char __far *m) { brk(m); }\n",
        )
        .unwrap();
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("watcom")).unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records brk.c");
        let options = llrm_codegen::driver::Options {
            abi: Some("watcom".to_owned()),
            ..llrm_driver::m16_options_old(llrm_x86_m16::machine::BUILT_IN.clone())
        };
        let asm = llrm_codegen::backend::masm::text(&super::selected(&text, "brk", None, &options).unwrap()).unwrap();
        let (push, code, pop) =
            (asm.find("push dx").expect(&asm), asm.find("db 0cch").expect(&asm), asm.find("add sp, 4").expect(&asm));
        assert!(push < code && code < pop, "{asm}");
    }

    /// A far pointer made from a long in memory had its segment as a word-wide
    /// view of the shifted dword, a second width of one value: the
    /// allocator loaded ES again on every iteration of the loop that stores
    /// through it.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_far_pointer_made_from_a_long_loads_its_segment_once() {
        let profile = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        let source = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/farcast.c");
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records farcast.c");
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "farcast", None, &llrm_driver::m16_options_old(machine)).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let name = llrm_target::Target::calling(&llrm_x86_m16::M16)
            .native()
            .decorated("omf", "fill")
            .expect("the default convention spells its symbols");
        let from = asm.find(&format!("{name} proc")).expect("the function");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let loop_at = loop_start(&body).expect("the loop");
        assert!(
            body.iter().filter(|line| line.starts_with("mov es,")).count() == 1
                && !body[loop_at..].iter().any(|line| line.starts_with("mov es,")),
            "{body:?}"
        );
    }

    /// A long double global got its initializer as a double, 8 bytes, while
    /// code loads it as 10 bytes: `gld` read 1.07e-49 and took two bytes of
    /// `after`.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_long_double_initializer_is_ten_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("probe.c");
        std::fs::write(&source, "long double gld = 3.5;\nshort after = 7;\nlong double get(void) { return gld; }\n")
            .unwrap();
        let recorded = super::recorded(&source, &[], false, &[]).expect("wccq records probe.c");
        let data: Vec<&str> = recorded.lines().filter(|line| line.contains("DGBytes")).collect();
        assert_eq!(data, ["- DGBytes 10 00000000000000e00040"], "{recorded}");
    }

    /// The flat target's long double was a double (8 bytes, no x87 tbyte code)
    /// while the real-mode target's was the x87 type: `-fld` was a switch
    /// in one target's file. The profile adds it where the layout has the x87
    /// type.
    /// A runtime that names no `libcalls` has none (`memcpy` stays `rep movs`
    /// there), a named one is a call: the m16 runtime names `memmove` and the
    /// m32 one `memcpy`, `memmove` and `memset`
    /// (runtime/c/x86-m32/ext.asm).
    #[test]
    fn test_a_runtime_names_its_libcalls_and_none_by_default() {
        let table: toml::Table = "libcalls = [\"memcpy\"]".parse().unwrap();
        assert_eq!(super::libcall_names(&table), ["memcpy"]);
        assert!(super::libcall_names(&toml::Table::new()).is_empty());
        let names =
            |target: &dyn llrm_target::Target| super::libcall_names(&target.runtime("c").unwrap().table().unwrap());
        assert_eq!(names(&llrm_x86_m16::M16), ["memmove"], "m16 names the routine it has");
        assert_eq!(names(&llrm_x86_m32::M32), ["memcpy", "memmove", "memset"]);
    }

    /// The call of `memcpy` named the routine `memcpy` in the stack convention:
    /// the m32 runtime has `memcpy_` (registers) and `_memcpy` (sysv), so the
    /// link failed with "memcpy is an undefined reference" once it was flipped
    /// on.
    #[test]
    fn test_the_libcall_is_the_symbol_a_declared_memcpy_would_have() {
        let m32: &dyn llrm_target::Target = &llrm_x86_m32::M32;
        let named = || ["memcpy", "memmove", "memset"].map(str::to_owned).to_vec();
        let watcom = super::libcalls_named(m32, Some("watcom"), "omf", named()).unwrap();
        let sysv = super::libcalls_named(m32, Some("sysv"), "omf", named()).unwrap();
        assert_eq!(watcom.iter().map(|one| one.2.as_str()).collect::<Vec<_>>(), ["memcpy_", "memmove_", "memset_"]);
        assert_eq!(sysv.iter().map(|one| one.2.as_str()).collect::<Vec<_>>(), ["_memcpy", "_memmove", "_memset"]);
    }

    /// A listed libcall is called by the symbol its ABI decorates, so a name
    /// the runtime does not define is an undefined reference at link: every
    /// (routine, ABI) of m32's `libcalls` is a `public` of ext.asm. Their
    /// results are tests/run/c/memcpy_runtime.c and memfuncs_runtime.c (n =
    /// 0..70000, both conventions).
    #[test]
    fn test_the_m32_runtime_defines_every_symbol_its_libcalls_name() {
        let m32: &dyn llrm_target::Target = &llrm_x86_m32::M32;
        let root = Path::new(env!("LLRM_ROOT"));
        let ext = std::fs::read_to_string(root.join("runtime/c/x86-m32/ext.asm")).unwrap()
            + &std::fs::read_to_string(root.join("runtime/shared/dos/m32/memfuncs.inc")).unwrap();
        let named = || ["memcpy", "memmove", "memset"].map(str::to_owned).to_vec();
        let defined: Vec<&str> = ext.lines().filter_map(|line| line.trim().strip_prefix("public ")).collect();
        for abi in ["watcom", "sysv"] {
            for (name, _, symbol) in super::libcalls_named(m32, Some(abi), "omf", named()).unwrap() {
                assert!(
                    defined.contains(&symbol.as_str()),
                    "{name} under {abi} is {symbol}, which ext.asm does not define (ext.asm, memfuncs.inc): {defined:?}"
                );
            }
        }
    }

    #[test]
    fn test_every_x86_target_asks_for_the_x87_long_double_once() {
        for profile in
            [super::Profile::of(&llrm_x86_m16::M16).unwrap(), super::Profile::of(&llrm_x86_m32::M32).unwrap()]
        {
            assert_eq!(profile.flags.iter().filter(|one| *one == "-fld").count(), 1, "{:?}", profile.flags);
        }
    }

    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_flat_long_double_is_recorded_as_the_x87_type() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("probe.c");
        std::fs::write(&source, "long double gld = 3.5;\nlong double get(void) { return gld; }\n").unwrap();
        let profile = super::Profile::of(&llrm_x86_m32::M32).unwrap();
        let recorded = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records probe.c");
        assert!(recorded.contains("TY_LONG_DOUBLE"), "{recorded}");
        assert!(recorded.lines().any(|line| line.contains("DGBytes 10 ")), "{recorded}");
        // the object is the description's 12 bytes: the image, then two of
        // padding
        assert!(recorded.lines().any(|line| line.trim() == "- DGUBytes 2"), "{recorded}");
    }

    /// The front end, its switches and its header were `if flat` here and a
    /// pair of env names in build.rs: a target now says them in its C
    /// runtime description, and a target with none is refused.
    #[test]
    fn test_a_target_asks_the_front_end_as_its_description_says() {
        let real = super::Profile::of(&llrm_x86_m16::M16).unwrap();
        let flat = super::Profile::of(&llrm_x86_m32::M32).unwrap();
        assert_eq!(
            (real.cpu.as_str(), real.header.as_str(), real.flags.contains(&"-mm".to_owned())),
            ("i86", "crates/frontends/llrm-c/src/borland.h", true)
        );
        assert_eq!(
            (flat.cpu.as_str(), flat.header.as_str(), flat.flags.contains(&"-zp4".to_owned())),
            ("386", "crates/frontends/llrm-c/src/flat.h", true)
        );
        let none: toml::Table = "stack = \"stack.toml\"\n".parse().unwrap();
        assert_eq!(super::Profile::parse(&none, "x").unwrap_err(), "the C runtime description has no [frontend]");
    }

    /// Where EBP is a value (no frame register kept), an `lea` through it is
    /// the value's address, not the frame's: masm read the register's name and
    /// the lack of a base value on an `lea`'s operand as a frame cell, and made
    /// `lea ebp,[ebp+ecx*2]` into `[esp+ecx*2]`.
    #[test]
    fn test_m32_an_lea_through_a_value_in_ebp_is_not_a_frame_cell() {
        let lines = flat_listing_with("sixptr", &["-O2", "-ftree-ch"]);
        let leas: Vec<&String> = lines.iter().filter(|line| line.starts_with("lea") && line.contains('*')).collect();
        assert!(!leas.iter().any(|line| line.contains("[esp+")), "{lines:#?}");
    }

    /// -O3 takes LLVM's copy budget (300, `UnrollThresholdAggressive`), not
    /// gcc's 200: at 150 the product's middle loop, 328 operations once its
    /// inner loop is copied out, stayed rolled and `b` was read back from the
    /// stack by 8 multiplies a trip, 10,399 model clocks against 1,189.
    #[test]
    fn test_m32_o3_copies_out_a_product_whose_operand_is_constant() {
        let lines = flat_listing_at("matmul", "-O3");
        let multiplies = lines.iter().filter(|line| line.starts_with("imul")).count();
        assert_eq!(multiplies, 0, "{lines:#?}");
    }

    /// A load whose only reader is an add is the add's memory operand. The
    /// return read the frame register as it does where one is kept, so a load
    /// into EBP, the last register given out where none is, looked live to
    /// the end: `mov ebp, [..]; add ax, bp` where the other two loads folded.
    #[test]
    fn test_m32_a_load_into_the_free_frame_register_is_folded_into_its_add() {
        let lines = flat_listing("sumthree");
        let body: Vec<&str> = lines
            .iter()
            .skip_while(|line| *line != "sum_three_ proc near")
            .take_while(|line| *line != "sum_three_ endp")
            .map(String::as_str)
            .collect();
        let folded = body.iter().filter(|line| line.starts_with("add ax, word ptr [")).count();
        assert_eq!(folded, 3, "{body:#?}");
    }

    /// The 386 front end records `sum.c` as the committed flat stream: `flat=1`
    /// in INIT, int and pointers 4 bytes. (Its source path is the
    /// machine's, so that line is not compared.)
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_the_386_wccq_records_the_committed_flat_stream() {
        let dir = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32");
        let recorded =
            super::recorded_for(&dir.join("sum.c"), &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
                .expect("the 386 wccq records sum.c");
        let committed = std::fs::read_to_string(dir.join("sum.cgs")).unwrap();
        let body =
            |text: &str| text.lines().filter(|line| !line.contains("DBSrcFile")).map(str::to_owned).collect::<Vec<_>>();
        assert!(recorded.starts_with("INIT ") && recorded.lines().next().unwrap().ends_with(" flat=1"), "{recorded}");
        assert_eq!(body(&recorded), body(&committed));
    }

    /// A second `-fi=` replaces the first: the predefined macros' header was
    /// forced after the target's, `far` was a far pointer again on the flat
    /// target, and DWARF refused every bench program that used one ("a far or
    /// huge pointer has no DWARF type"). The target's header and the
    /// predefined macros are both in the one forced file.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_the_targets_header_and_the_predefined_macros_are_both_forced() {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("far.c");
        std::fs::write(
            &source,
            "int far *cell;\nint widest = __INT_MAX__;\nint main(void) { return *cell + (widest != 2147483647); }\n",
        )
        .unwrap();
        let recorded = super::recorded_for(&source, &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
            .expect("records far.c");
        assert!(!recorded.contains("TY_LONG_POINTER"), "`far` is a far pointer on a flat target: {recorded}");
        assert!(recorded.contains("2147483647"), "__INT_MAX__ is defined: {recorded}");
    }

    /// Borland's dos.h names a parameter `__segment` (`peek( unsigned
    /// __segment, unsigned __offset )`), an Open Watcom keyword: on the
    /// flat target the declaration was E1060 "Invalid type" (QCport host/dbg
    /// and game/mdl_ai at m32). Both targets' headers rename it.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_flat_program_may_name_a_parameter_segment() {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("seg.c");
        std::fs::write(&source, "int peek( unsigned __segment, unsigned __offset );\nint peek( unsigned __segment, unsigned __offset ) { return __segment + __offset; }\n").unwrap();
        for target in [super::Profile::of(&llrm_x86_m16::M16).unwrap(), super::Profile::of(&llrm_x86_m32::M32).unwrap()]
        {
            super::recorded_for(&source, &[], false, &[], &target)
                .unwrap_or_else(|error| panic!("{}: {error:?}", target.cpu));
        }
    }

    /// The loop in `function` that reads `marker`, from its label to its
    /// backward branch, as the rich route selects it.
    fn selected_loop(
        fixture: &str,
        function: &str,
        marker: &str,
    ) -> Vec<String> {
        let path = Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs"));
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        // At -O2: a loop is copied out only where the code does not grow, so
        // the loop the test reads stays a loop.
        // With the allocation search and the spiller's route, which -Omax
        // alone makes: one allocation keeps a pointer of `strides` in the
        // frame (#1886).
        let options = llrm_codegen::driver::Options {
            pipeline: llrm_transforms::pipeline::Options {
                search: true,
                ..llrm_transforms::pipeline::Options::standard()
            },
            ..llrm_driver::m16_options_old(machine)
        };
        let built = super::selected(&std::fs::read_to_string(path).unwrap(), fixture, None, &options).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
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
    fn selected_body(
        fixture: &str,
        function: &str,
    ) -> Vec<String> {
        let path = Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs"));
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(
            &std::fs::read_to_string(path).unwrap(),
            fixture,
            None,
            &llrm_driver::m16_options_old(machine),
        )
        .unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        asm[from..]
            .lines()
            .skip(1)
            .map(str::trim)
            .take_while(|one| !one.ends_with("endp"))
            .filter(|one| !one.ends_with(':'))
            .map(str::to_owned)
            .collect()
    }

    /// `function` of tests/fixtures/c/regparm3.c under `-mabi=regparm3`: gcc's
    /// `-mregparm=3` by the size of the argument.
    fn regparm3(function: &str) -> Vec<String> {
        regparm3_at(function, llrm_codegen::driver::flags::Level::O2)
    }

    fn regparm3_at(
        function: &str,
        level: llrm_codegen::driver::flags::Level,
    ) -> Vec<String> {
        procedure_of(&regparm3_listing(level), &format!("{function}@3"))
    }

    /// The lines of `symbol`'s procedure in `asm`.
    fn procedure_of(
        asm: &str,
        symbol: &str,
    ) -> Vec<String> {
        let from = asm.find(&format!("{symbol} proc")).expect("the function");
        asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).map(str::to_owned).collect()
    }

    /// tests/fixtures/c/regparm3.c compiled under `-mabi=regparm3`.
    fn regparm3_listing(level: llrm_codegen::driver::flags::Level) -> String {
        regparm3_listing_with(level, |_| {})
    }

    /// `regparm3_listing`, the pipeline options adjusted by `tune`.
    fn regparm3_listing_with(
        level: llrm_codegen::driver::flags::Level,
        tune: impl FnOnce(&mut llrm_transforms::pipeline::Options),
    ) -> String {
        let profile = super::Profile::for_abi(&llrm_x86_m16::M16, Some("regparm3")).unwrap();
        let source = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/regparm3.c");
        let text = super::recorded_for(&source, &[], false, &[], &profile).expect("wccq records regparm3.c");
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let mut options = llrm_codegen::driver::Options {
            abi: Some("regparm3".to_owned()),
            pipeline: level.options(),
            ..llrm_driver::m16_options_old(machine)
        };
        tune(&mut options.pipeline);
        let built = super::selected(&text, "regparm3", None, &options).unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap()
    }

    /// A definition has the convention of its prior declaration, as with any C
    /// compiler: `declared` follows a `__cdecl` prototype and is
    /// `_declared`, cdecl's, under the regparm3 default, where `plain` after no
    /// prototype is `_plain@3`. The Borland routines a program replaces are
    /// defined so (QCport's strlib.c).
    #[test]
    fn test_m16_regparm3_a_definition_takes_the_convention_of_its_declaration() {
        let asm = regparm3_listing(llrm_codegen::driver::flags::Level::O2);
        assert!(asm.contains("public _declared\n") && asm.contains("_declared proc"), "{asm}");
        assert!(asm.contains("_plain@3 proc"), "{asm}");
        let body = procedure_of(&asm, "_declared");
        assert!(body.iter().any(|line| line.contains("[bp+6]")), "its arguments are on the stack: {body:?}");
    }

    /// A definition that contradicts its prototype's convention is an error, as
    /// Open Watcom says.
    #[test]
    fn test_a_definition_contradicting_its_prototype_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("clash.c");
        std::fs::write(&source, "int __cdecl f(int a, int b);\nint __pascal f(int a, int b) { return a - b; }\n")
            .unwrap();
        let error = super::recorded_for(
            &source,
            &[],
            false,
            &[],
            &super::Profile::for_abi(&llrm_x86_m16::M16, Some("regparm3")).unwrap(),
        )
        .unwrap_err();
        assert!(error.0.contains("Modifiers disagree"), "{}", error.0);
    }

    /// The first three integer arguments arrive in AL, DX and ECX, each the
    /// width of its value (a long in one register); the fourth is the
    /// stack's first word.
    #[test]
    fn test_m16_regparm3_the_first_three_arguments_arrive_by_their_size_and_the_fourth_on_the_stack() {
        let body = regparm3("_sized");
        assert!(
            body.contains(&"movsx ax, al".to_owned())
                && body.contains(&"add ax, dx".to_owned())
                && body.contains(&"add eax, ecx".to_owned())
                && body.contains(&"add eax, dword ptr [bp+6]".to_owned()),
            "{body:?}"
        );
    }

    /// A float and a double are on the stack and leave the registers to the
    /// integers after them: `skip(float, int b, double, int d, int e)` has
    /// b, d, e in AX, DX, CX. Open Watcom's convention stops at the first that
    /// fits none.
    #[test]
    fn test_m16_regparm3_a_float_and_a_double_do_not_stop_the_later_integers_taking_registers() {
        let body = regparm3("_skip");
        assert!(
            body.contains(&"fld dword ptr [bp+6]".to_owned()) && body.contains(&"fld qword ptr [bp+10]".to_owned()),
            "{body:?}"
        );
        assert!(!body.iter().any(|line| line.contains("[bp+18]") || line.contains("[bp+20]")), "{body:?}");
    }

    /// A call in a register convention changes FS and GS, as any call into code
    /// that may use them does: the description names no selector, and a far
    /// pointer's segment held in FS across `ext` was read again after it,
    /// whatever `ext` had put there (QCport drew 37697 polygons for
    /// Borland's 37637, its pool's self-test failing).
    #[test]
    fn test_m16_regparm3_a_call_does_not_keep_a_segment_in_fs_or_gs() {
        let body = regparm3("_keep_far");
        let mut loaded: Vec<&str> = Vec::new();
        for line in &body {
            if line.starts_with("call") {
                loaded.clear();
            }
            for register in ["fs", "gs"] {
                if line.starts_with(&format!("l{register} ")) || line.starts_with(&format!("mov {register},")) {
                    loaded.push(register);
                } else if line.contains(&format!("{register}:[")) && !loaded.contains(&register) {
                    panic!("{register} is read after a call without being loaded: {body:?}");
                }
            }
        }
    }

    /// A struct of 4 bytes passed by value is on the stack, whatever its size;
    /// Open Watcom's convention passes it as a long in registers.
    /// `take4(struct S4 s, int a)` reads s at [bp+6] and [bp+8], a is in AX.
    #[test]
    fn test_m16_regparm3_a_small_struct_argument_is_on_the_stack() {
        let body = regparm3("_take4");
        // Which register takes each word is the allocator's: the struct is read
        // from [bp+6] and [bp+8], and no register holds it.
        let reads = |cell: &str| body.iter().filter(|line| line.contains(&format!("word ptr [bp+{cell}]"))).count();
        assert!(reads("6") >= 1 && reads("8") >= 1, "{body:?}");
    }

    /// A copy and an add, `mov r,ax; add r,k`, are one `lea r,[eax+k]` where it
    /// costs no more: the 486's LEA is one clock (its table held the 386's
    /// two), so with the address-size prefix it is the two instructions' two
    /// clocks in a byte less. Two values off one argument took two
    /// instructions each.
    #[test]
    fn test_m16_regparm3_a_copy_and_an_add_are_one_lea_where_it_costs_no_more() {
        let body = regparm3("_leas");
        let leas = body.iter().filter(|line| line.starts_with("lea ") && line.contains("[eax+")).count();
        assert!(leas == 2 && !body.iter().any(|line| line.starts_with("mov ")), "{body:?}");
    }

    /// A copy and a unit add stay `mov r,ax; inc r`: the INC makes them three
    /// bytes, the LEA is four.
    #[test]
    fn test_m16_regparm3_a_copy_and_a_unit_add_stay_when_the_lea_is_longer_than_the_inc() {
        let body = regparm3_at("_incs", llrm_codegen::driver::flags::Level::Os);
        assert!(
            body.iter().any(|line| line.starts_with("inc ")) && !body.iter().any(|line| line.starts_with("lea ")),
            "{body:?}"
        );
    }

    /// `calleepop` serves a regparm3 function as it does a cdecl16 one:
    /// `many`'s fourth and fifth arguments are the only ones on the stack,
    /// and its callee removes them (`ret 4`), its callers nothing.
    #[test]
    fn test_m16_regparm3_calleepop_removes_the_stack_arguments_in_the_callee() {
        // `many` is static and small: inlined into `call_many` it has no
        // procedure. Kept a call, as the test means it.
        let asm = regparm3_listing_with(llrm_codegen::driver::flags::Level::O2, |pipeline| {
            pipeline.inline = llrm_transforms::inline::Threshold { limit: 0, last: false, ..pipeline.inline };
        });
        let body = procedure_of(&asm, "_many@3");
        assert_eq!(body.last().map(String::as_str), Some("ret 4"), "{body:?}");
        let caller = procedure_of(&asm, "_call_many@3");
        assert!(!caller.iter().any(|line| line.starts_with("add sp")), "{caller:?}");
    }

    /// A long on the stack is two words: `stack_long`'s fourth, fifth and sixth
    /// arguments (a byte, a long, a long) are at [bp+6], [bp+8] and
    /// [bp+12]. The long was a word's worth to the stack's layout, so the sixth
    /// was read at [bp+10], from the middle of the fifth.
    #[test]
    fn test_m16_regparm3_a_long_on_the_stack_takes_two_words() {
        let body = regparm3("_stack_long");
        assert!(
            body.iter().any(|line| line.ends_with("byte ptr [bp+6]"))
                && body.iter().any(|line| line.ends_with("dword ptr [bp+8]"))
                && body.iter().any(|line| line.ends_with("dword ptr [bp+12]")),
            "{body:?}"
        );
    }

    /// An i64 is four stack words, low half first, and the int after it takes
    /// AX.
    #[test]
    fn test_m16_regparm3_an_i64_is_on_the_stack_and_the_next_integer_takes_ax() {
        let body = regparm3("_wide");
        assert!(
            body.contains(&"mov eax, dword ptr [bp+6]".to_owned())
                || body.iter().any(|line| line.contains("dword ptr [bp+6]")),
            "{body:?}"
        );
        assert!(!body.iter().any(|line| line.contains("[bp+14]")), "{body:?}");
    }

    /// A long is returned in EAX: `long_result` ends with the product in EAX,
    /// the `shld edx, eax, 16` that made DX:AX gone.
    #[test]
    fn test_m16_regparm3_a_long_result_is_in_eax_with_no_dx_ax_repack() {
        let body = regparm3("_long_result");
        assert!(body.contains(&"imul eax, edx".to_owned()), "{body:?}");
        assert!(!body.iter().any(|line| line.starts_with("shld")), "{body:?}");
    }

    /// A far pointer comes back in EAX, the segment in the high word. Taken
    /// apart with the segment a dword's high half used as a word, the
    /// allocator did not see it as the one value and loaded ES again on every
    /// iteration of the loop: 10007 more instructions in bench
    /// textfill/nib.
    #[test]
    fn test_m16_regparm3_a_far_pointer_returned_in_eax_loads_es_once_before_the_loop() {
        let body = regparm3("_fill");
        assert_eq!(body.iter().filter(|line| line.starts_with("mov es,")).count(), 1, "{body:?}");
        let loop_start = loop_start(&body).expect("the loop");
        assert!(body[loop_start..].iter().all(|line| !line.starts_with("mov es,")), "{body:?}");
    }

    /// A constant far pointer is returned as the one dword it is: it was split
    /// to words and joined again (six instructions) in bench textfill's
    /// `os.text_screen`.
    #[test]
    fn test_m16_regparm3_a_constant_far_pointer_is_returned_as_one_move() {
        let body = regparm3("_const_far");
        assert_eq!(
            body.iter().filter(|line| !line.ends_with(':')).collect::<Vec<_>>(),
            ["mov eax, -1207959552", "retf"],
            "{body:?}"
        );
    }

    /// An argument register stored to its slot at entry is still the value: a
    /// later read of the slot is a read of the register. They were `mov
    /// [bp-2], dx` then `mov bx, [bp-2]`, and `add di, [bp-2]`: a memory
    /// operand more each (bench recchop/c -Os: +185).
    #[test]
    fn test_m16_regparm3_a_slot_a_register_still_holds_is_read_from_the_register() {
        // The premise moved with the allocator (`_chop` no longer stores dx at
        // entry), so the property is asked of every store of a register
        // to a frame slot in the listing: until the register is written, the
        // slot is not read.
        let listing = regparm3_listing(llrm_codegen::driver::flags::Level::Os);
        let lines: Vec<&str> = listing.lines().map(str::trim).collect();
        let written = |line: &str, register: &str| {
            line.split_once(' ')
                .is_some_and(|(_, rest)| rest.split(',').next().is_some_and(|dest| dest.trim() == register))
        };
        let mut stores = 0;
        for (at, line) in lines.iter().enumerate() {
            let Some((slot_part, register)) = line.strip_prefix("mov ").and_then(|rest| rest.split_once(", ")) else {
                continue;
            };
            let Some(slot) = slot_part.strip_prefix("word ptr [bp-").map(|rest| rest.trim_end_matches(']')) else {
                continue;
            };
            if !matches!(register, "ax" | "bx" | "cx" | "dx" | "si" | "di") {
                continue;
            }
            stores += 1;
            let reads = lines[at + 1..]
                .iter()
                .take_while(|next| !written(next, register) && !next.contains("call") && !next.ends_with("endp"))
                // An extend of the slot is a folded reload (the pin folds a
                // reload into `movsx`/`movzx` too): the allocator started from
                // the coalescer's intervals leaves `movsx esi, [bp-6]` where si
                // still holds the word (AM-IO, #4269).
                .filter(|next| {
                    !next.starts_with("mov word ptr [bp-")
                        && !next.starts_with("movsx ")
                        && !next.starts_with("movzx ")
                        && next.contains(&format!("[bp-{slot}]"))
                })
                .count();
            assert_eq!(
                reads,
                0,
                "{register} is read back from [bp-{slot}] while it holds the value: {:?}",
                &lines[at..at + 8.min(lines.len() - at)]
            );
        }
        assert!(stores > 0, "the listing stores no register to a slot: the property is vacuous");
    }

    /// A far pointer is returned as one dword, offset low: the callee joins the
    /// words and the caller splits them.
    #[test]
    fn test_m16_regparm3_a_far_pointer_result_is_one_dword_joined_and_split() {
        let body = regparm3("_far_result");
        // joined into eax: the words shifted together, not left as dx:ax. (Not
        // "no edx": which register holds the low word as a scratch is the
        // allocator's choice.)
        assert!(
            body.iter().any(|line| line.starts_with("shl eax, 16") || line.starts_with("shld eax,"))
                && body.iter().any(|line| line.starts_with("or eax,") || line.starts_with("shld eax,")),
            "{body:?}"
        );
        let body = regparm3("_caller");
        assert!(
            body.contains(&"shr eax, 16".to_owned()) && body.iter().any(|line| line.starts_with("mov es,")),
            "{body:?}"
        );
    }

    /// `function` of tests/fixtures/c/ia16.cgs under `-mabi=ia16`: gcc-ia16's
    /// convention.
    fn ia16(function: &str) -> Vec<String> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/ia16.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let options =
            llrm_codegen::driver::Options { abi: Some("ia16".to_owned()), ..llrm_driver::m16_options_old(machine) };
        let built = super::selected(&text, "ia16", None, &options).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).map(str::to_owned).collect()
    }

    /// `ia16-elf-gcc` writes a struct result larger than 4 bytes through a near
    /// pointer that is the first argument and returns it in AX, the caller
    /// removing all it pushed (6 bytes, the pointer too); Borland's was a far
    /// pointer (4 bytes) returned in DX:AX.
    #[test]
    fn test_m16_ia16_a_struct_result_is_a_near_first_argument_the_caller_removes() {
        let body = ia16("_fs");
        assert!(
            body.contains(&"mov bx, word ptr [bp+6]".to_owned()) && body.contains(&"mov ax, bx".to_owned()),
            "{body:?}"
        );
        assert_eq!(body.last().map(String::as_str), Some("retf"));
        let body = ia16("_callsr");
        assert!(body.contains(&"lea ax, [bp-6]".to_owned()) && body.contains(&"add sp, 6".to_owned()), "{body:?}");
    }

    /// A struct of 4 bytes is returned in DX:AX as a long is.
    #[test]
    fn test_m16_ia16_a_small_struct_result_is_in_registers() {
        let body = ia16("_fw");
        assert!(body.contains(&"shld edx, eax, 16".to_owned()), "{body:?}");
        assert!(!body.iter().any(|line| line.contains("[bp+10]")), "{body:?}");
    }

    /// `function` of tests/fixtures/c/watcall16.cgs under `-mabi=watcom`: Open
    /// Watcom's 16-bit register convention.
    fn watcall16(function: &str) -> Vec<String> {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/watcall16.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let options =
            llrm_codegen::driver::Options { abi: Some("watcom".to_owned()), ..llrm_driver::m16_options_old(machine) };
        let built = super::selected(&text, "watcall16", None, &options).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        asm[from..].lines().skip(1).map(str::trim).take_while(|one| !one.ends_with("endp")).map(str::to_owned).collect()
    }

    /// `wcc -ecw` took the first four words in AX, DX, BX and CX and the rest
    /// from the stack, lowest first, popped with `retf 4`; the stack
    /// convention read all six from [bp+6]... The sum is reassociated
    /// (`a + c*d - (b + e) + f`): the same ten instructions and two memory
    /// operands as the previous association, no `neg`. The pin has no Watcom
    /// convention to compare with.
    #[test]
    fn test_m16_watcom_words_arrive_in_ax_dx_bx_cx_and_the_callee_pops_the_rest() {
        let body = watcall16("six_");
        assert_eq!(
            body,
            [
                "push bp",
                "mov bp, sp",
                "L0_0:",
                "imul cx, bx",
                "add ax, cx",
                "add dx, word ptr [bp+6]",
                "sub ax, dx",
                "add ax, word ptr [bp+8]",
                "pop bp",
                "retf 4"
            ]
        );
    }

    /// A long or a far pointer takes a pair: `mixed(int a, long b, int c)` has
    /// a in AX, b in BX:CX and c in DX, as `wcc` has it (DX was passed over
    /// by the pair and stayed free); a far pointer is its offset in AX and its
    /// segment in DX.
    #[test]
    fn test_m16_watcom_a_long_and_a_far_pointer_take_a_pair_and_a_passed_over_register_stays_free() {
        let body = watcall16("mixed_");
        // The allocator names the registers the extensions land in; what is
        // read is the convention's: the pair in BX (low) and CX, `c` in
        // DX, `a` in AX.
        let reads = |op: &str, source: &str| {
            body.iter().any(|line| line.starts_with(&format!("{op} ")) && line.ends_with(&format!(", {source}")))
        };
        assert!(
            reads("movzx", "bx") && reads("movzx", "cx") && reads("movsx", "dx") && reads("movsx", "ax"),
            "{body:?}"
        );
        let body = watcall16("far_pointer_");
        assert!(
            body.contains(&"mov si, ax".to_owned())
                && body.contains(&"mov es, dx".to_owned())
                // `*p + a + b`: the load is the late operand, so a + b may be
                // made first (`add cx, bx`, b is dead after) or the load first
                // (`add ax, bx`): the sum is the same, and tests/run/c/
                // watcom_far_sum.c runs both orders.
                && (body.contains(&"add ax, bx".to_owned()) || body.contains(&"add cx, bx".to_owned()))
                && body.contains(&"add ax, cx".to_owned()),
            "{body:?}"
        );
    }

    /// A struct larger than a dword is written through the near address in SI,
    /// which comes back in AX.
    #[test]
    fn test_m16_watcom_a_struct_result_is_written_through_si_and_returned_in_ax() {
        let body = watcall16("result_");
        assert!(
            // The 6-byte result is written through SI (a dword and a word, or
            // words: the combiner and allocator choose) and SI is returned.
            body.iter().filter(|line| line.starts_with("mov ") && line.contains("ptr [si")).count() >= 2
                && body.contains(&"mov ax, si".to_owned()),
            "{body:?}"
        );
        assert_eq!(body.last().map(String::as_str), Some("retf"));
    }

    /// The call passes a long in DX:AX and an int in BX and leaves the callee's
    /// own `retf` to pop nothing.
    #[test]
    fn test_m16_watcom_a_call_passes_a_long_in_dx_ax_and_takes_the_result_in_dx_ax() {
        let body = watcall16("calls_");
        assert!(body.contains(&"mov bx, 5".to_owned()) && body.contains(&"call far ptr away_".to_owned()), "{body:?}");
        assert!(!body.iter().any(|line| line.starts_with("add sp")), "{body:?}");
    }

    /// The loop of `function` in `fixture`'s listing as `llrm-c -O2 -march=i486
    /// -S` writes it: from the label its backward branch takes to the
    /// branch.
    fn driven_loop(
        fixture: &str,
        function: &str,
    ) -> Vec<String> {
        let directory = tempfile::tempdir().unwrap();
        let (source, out) = (
            Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs")),
            directory.path().join("out.asm"),
        );
        let argv =
            ["-O2", "-march=i486", "-S", source.to_str().unwrap(), "-o", out.to_str().unwrap()].map(str::to_owned);
        assert_eq!(super::main(&argv), 0);
        let asm = std::fs::read_to_string(out).unwrap();
        let from = asm.find(&format!("{function} proc")).expect("the function");
        let lines: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let at =
            lines.iter().rposition(|one| one.starts_with('j') && !one.starts_with("jmp")).expect("a backward branch");
        let label = format!("{}:", lines[at].split_whitespace().nth(1).unwrap());
        let top = lines.iter().position(|one| *one == label).expect("its label");
        lines[top..=at].iter().map(|one| (*one).to_owned()).collect()
    }

    /// A volatile store to a far address may change `*ch` and `*at` as far as
    /// the AA stack of the pin can tell (`opt -passes='loop-mssa(licm)'` and
    /// `default<O2>` on this loop hoist neither): both cells are read every
    /// trip, at 0xB8000000 as at 0x1234. That video memory is no program data
    /// is target knowledge the pin has no place for (#3994: tuning, once the
    /// memory-space model states it from the target description).
    #[test]
    fn test_a_volatile_far_store_leaves_the_loop_the_cells_the_pin_leaves_it() {
        let loads = |body: &[String]| body.iter().filter(|one| one.contains("ptr [")).count();
        // the far pointers are in whichever segment registers the allocator
        // keeps them
        let stores =
            |body: &[String]| body.iter().filter(|one| one.contains("byte ptr ") && one.contains("s:[")).count();
        let video = driven_loop("farvolatile", "_video");
        assert_eq!((loads(&video), stores(&video)), (2, 2), "{video:#?}");
        let conventional = driven_loop("farvolatile", "_conventional");
        assert_eq!((loads(&conventional), stores(&conventional)), (2, 2), "{conventional:#?}");
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
    /// counter. Strength gave each stride its own pointer, and two of them
    /// lived in the frame: loaded for every access and stepped in memory.
    /// Indexes in registers are the cure: one per address width, as the
    /// byte array through 16-bit `si` saves its two address-size prefixes
    /// for one more add.
    #[test]
    fn test_arrays_of_several_strides_keep_their_indexes_in_registers() {
        for function in ["_bench_strides3", "_bench_strides4"] {
            let body = selected_loop("strides", function, "xor");
            // An access's address registers; a `lea` reaches no memory, its
            // operands are arithmetic.
            let registers: std::collections::BTreeSet<String> = body
                .iter()
                .filter(|one| !one.starts_with("lea "))
                .filter_map(|one| Some(one.split_once('[')?.1.split_once(']')?.0.to_owned()))
                .flat_map(|inside| inside.split(['+', '-', '*']).map(str::trim).map(str::to_owned).collect::<Vec<_>>())
                .filter(|part| {
                    part.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                        && !matches!(part.as_str(), "bp" | "ebp")
                })
                .filter(|part| ["ax", "bx", "cx", "dx", "si", "di"].iter().any(|name| part.ends_with(name)))
                .collect();
            // A pointer kept in the frame is stepped there: `add word ptr
            // [bp-76h], 1`.
            let stepped = body
                .iter()
                .any(
                    |one| ["add ", "sub ", "inc ", "dec "].iter().any(|op| {
                        one.strip_prefix(op).is_some_and(|rest| rest.split(',').next().unwrap_or("").contains("ptr ["))
                    }),
                );
            assert!(registers.len() <= 2 && !stepped, "{function}: {registers:?}\n{body:#?}");
        }
    }

    /// qcport's sys.c, item.c and mdl_ai.c keep near and far function
    /// pointers in static tables; the rich route refused all three with
    /// "a code address in data".
    #[test]
    fn test_code_addresses_in_data_are_called_through() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/codeptrs.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "codeptrs", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        // The near table is folded away with its call: `_twice` is inlined.
        assert!(lines.iter().any(|one| one.starts_with("dd _show")), "{asm}");
        let from = asm.find("_run proc").expect("_run");
        let run: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        // The far table's entry is read by the call itself (`call dword ptr
        // _notices`, or through an index register), not copied out first.
        assert!(run.iter().any(|one| one.starts_with("call dword ptr ")), "{run:#?}");
        // The near table call is folded to `_twice` and inlined (`add si, si`).
        assert!(run.iter().any(|one| *one == "add si, si"), "{run:#?}");
    }

    /// qcport's sc.c initializes `short links[5]` from a constant: an
    /// aggregate CGAssign, refused as "a 10-byte aggregate as a value".
    #[test]
    fn test_a_local_array_initializer_is_copied() {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/arrayinit.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "arrayinit", None, &llrm_driver::m16_options_old(machine));
        assert!(built.is_ok(), "{:?}", built.err());
    }

    /// qcport's dbg.c, sys_time.c, d_poly.c and ent.c use `_asm` and
    /// `__emit__`: "inline code F.0 is not raised to the rich MIR". The code
    /// is laid down with each frame place it names at that place's slot, and
    /// a value-less return after it answers what it left in dx:ax.
    #[test]
    fn test_inline_code_reads_and_writes_its_frame_places() {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/inlinecode.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built =
            super::selected(&text, "inlinecode", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let body = |name: &str| -> Vec<String> {
            let from = asm.find(&format!("{name} proc")).expect("the procedure");
            asm[from..]
                .lines()
                .skip(1)
                .map(str::trim)
                .take_while(|one| !one.ends_with("endp"))
                .map(str::to_owned)
                .collect()
        };
        // Each db line decoded: the code's own instructions, displacements
        // patched.
        let decoded = |lines: &[String]| -> Vec<String> {
            lines
                .iter()
                .map(|line| match line.strip_prefix("db ") {
                    Some(bytes) => {
                        let bytes: Vec<u8> = bytes
                            .split(',')
                            .map(|one| u8::from_str_radix(one.trim_end_matches('h'), 16).unwrap())
                            .collect();
                        let mut decoder = iced_x86::Decoder::new(16, &bytes, iced_x86::DecoderOptions::NONE);
                        let mut formatter = iced_x86::NasmFormatter::new();
                        iced_x86::Formatter::options_mut(&mut formatter).set_number_base(iced_x86::NumberBase::Decimal);
                        decoder
                            .iter()
                            .map(|one| {
                                let mut text = String::new();
                                iced_x86::Formatter::format(&mut formatter, &one, &mut text);
                                text
                            })
                            .collect::<Vec<_>>()
                            .join("; ")
                    }
                    None => line.clone(),
                })
                .collect()
        };
        let sine = decoded(&body("_sine"));
        let slot = |line: &str| {
            line.split_once('[').map(|(_, rest)| rest.trim_end_matches(']').replace(' ', "").to_lowercase())
        };
        let stored =
            sine.iter().find(|one| one.starts_with("fstp qword ptr")).and_then(|one| slot(one)).expect("rad's slot");
        let code = sine.iter().find(|one| one.contains("fsin")).expect("the code");
        let returned = sine
            .iter()
            .rev()
            .find(|one| one.starts_with("fld qword ptr"))
            .and_then(|one| slot(one))
            .expect("result's slot");
        assert!(
            code.starts_with(&format!("fld qword [{stored}]")) && code.ends_with(&format!("fstp qword [{returned}]")),
            "{sine:#?}"
        );
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
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "farconst", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_ticks proc").expect("_ticks");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        // The selector is pushed and popped into ES (same bytes as `mov es,
        // ax`).
        let segment =
            |one: &&str| one.ends_with(", 64") || one.ends_with(&format!(", {}", 0x0040_006C)) || *one == "pushw 64";
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
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "farnull", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
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
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/nearviads.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "nearviads", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_paint proc").expect("_paint");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let through_stack = |one: &&str| {
            one.contains("ss:[") || one.contains("mov ds,") || one.starts_with("lds ") || one.starts_with("pop ds")
        };
        assert!(!body.iter().any(through_stack), "{body:#?}");
    }

    /// qcport's savegame.c passes stack locals as far pointers. Their segment
    /// was DGROUP, wrong wherever the stack is not in it, as dsp.asm's IRQ
    /// stack is not.
    #[test]
    fn test_a_far_pointer_to_a_local_is_in_the_stack_segment() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/localfar.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "localfar", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_caller proc").expect("_caller");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(
            !body.iter().any(|one| one.contains("DGROUP")) && body.iter().any(|one| one.ends_with("ss")),
            "{body:#?}"
        );
    }

    /// A call's `add sp,2` is three bytes; BCC -Os pops the argument into CX
    /// in one. llrm-c -Os kept the add: 1.3K of QCport's surplus over BCC.
    /// On the 486 a pop is slower, so -O2 keeps it.
    #[test]
    fn test_arguments_are_popped_off_at_os_only() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/popargs.cgs")).unwrap();
        let cleanups = |level: Level| {
            let machine = llrm_codegen::abi::machine::Machine {
                cpu: "486".to_owned(),
                ..llrm_x86_m16::machine::BUILT_IN.clone()
            };
            let options =
                llrm_codegen::driver::Options { pipeline: level.options(), ..llrm_driver::m16_options_old(machine) };
            let built = super::selected(&text, "popargs", None, &options).expect("selects");
            let asm = llrm_codegen::backend::masm::text(&built).unwrap();
            let from = asm.find("_caller proc").expect("_caller");
            asm[from..]
                .lines()
                .map(str::trim)
                .take_while(|one| !one.ends_with("endp"))
                .filter(|one| one.starts_with("add sp") || *one == "pop cx")
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        use llrm_codegen::driver::flags::Level;
        assert_eq!(cleanups(Level::Os), ["pop cx", "pop cx", "pop cx"]);
        assert_eq!(cleanups(Level::O2), ["add sp, 2", "add sp, 4"]);
    }

    /// A C function that calls nothing came out of the compile with no word
    /// that nothing re-enters it: `norecurse` was inferred where no compiler
    /// ran.
    #[test]
    fn test_a_leaf_function_comes_out_of_the_compile_norecurse() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/halve.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let directory = std::env::temp_dir().join(format!("llrm-c-norecurse-{}", std::process::id()));
        super::selected(&text, "halve", Some(&directory), &llrm_driver::m16_options_old(machine)).expect("selects");
        let mut stages = std::fs::read_dir(&directory)
            .unwrap()
            .flatten()
            .map(|one| one.path())
            .filter(|one| one.extension().is_some_and(|ext| ext == "ll"))
            .collect::<Vec<_>>();
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
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "fabs", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_halfabs proc").expect("_halfabs");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(body.contains(&"fabs") && !body.iter().any(|one| one.starts_with("call")), "{body:#?}");
    }

    /// Spill slots went below the allocas: past a 200-byte array, each of
    /// their accesses took a two-byte displacement. QCport had 3,088 such.
    #[test]
    fn test_spill_slots_sit_above_a_big_frame_array() {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/spillnear.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "spillnear", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_spills proc").expect("_spills");
        let far: Vec<&str> = asm[from..]
            .lines()
            .map(str::trim)
            .take_while(|one| !one.ends_with("endp"))
            .filter(|one| !one.starts_with("lea"))
            .filter(|one| {
                one.split("[bp-")
                    .nth(1)
                    .and_then(|rest| rest.split(']').next()?.parse::<i64>().ok())
                    .is_some_and(|disp| disp > 128)
            })
            .collect();
        assert!(far.is_empty(), "{far:#?}");
    }

    /// A frame object passed to a call is read by it. DSE took it for
    /// private and dropped every store before the call that `test` did not
    /// read back itself: QCport's ls_selftest returned -2.
    #[test]
    fn test_stores_a_callee_reads_through_its_argument_stay() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/argread.cgs")).unwrap();
        let options = llrm_codegen::driver::Options {
            pipeline: llrm_codegen::driver::flags::Level::Os.options(),
            ..llrm_driver::m16_options_old(llrm_codegen::abi::machine::Machine {
                cpu: "486".to_owned(),
                ..llrm_x86_m16::machine::BUILT_IN.clone()
            })
        };
        let built = super::selected(&text, "argread", None, &options).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_test proc").expect("_test");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let calls: Vec<usize> =
            body.iter().enumerate().filter(|(_, one)| **one == "call far ptr _animate").map(|(at, _)| at).collect();
        // `s`, by the address passed to `animate`: `lea r, [bp-N]`, made once
        // for both calls.
        let base: i64 = body[..calls[1]]
            .iter()
            .rev()
            .find_map(|one| one.strip_prefix("lea ")?.split_once(", [bp-")?.1.strip_suffix(']')?.parse().ok())
            .expect("&s");
        // Every byte the stores between the calls write, by frame offset.
        let mut written = std::collections::BTreeSet::new();
        for one in &body[calls[0]..calls[1]] {
            let Some((width, rest)) = [("byte", 1), ("word", 2), ("dword", 4)]
                .iter()
                .find_map(|(name, width)| Some((*width, one.strip_prefix(&format!("mov {name} ptr [bp-"))?)))
            else {
                continue;
            };
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
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/indexread.cgs")).unwrap();
        let options = llrm_codegen::driver::Options {
            pipeline: llrm_codegen::driver::flags::Level::Os.options(),
            ..llrm_driver::m16_options_old(llrm_codegen::abi::machine::Machine {
                cpu: "486".to_owned(),
                ..llrm_x86_m16::machine::BUILT_IN.clone()
            })
        };
        let built = super::selected(&text, "indexread", None, &options).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_test proc").expect("_test");
        if std::env::var_os("SHOWASM").is_some() {
            eprintln!("{}", &asm[from..]);
        }
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        let stored: Vec<&&str> = body
            .iter()
            .filter(|one| one.starts_with("mov") && one.contains("ptr [bp-") && one.contains("],"))
            .collect();
        // Or the array is the constant table it was copied from, read where it
        // lies (`visitAllocaInst`, 77fa49769a): its bytes are the initializer.
        let table = asm.lines().any(|one| one.trim() == "db 001h,001h,001h,002h,001h,004h,007h,000h,001h,007h");
        let from_table = body.iter().any(|one| one.contains("word ptr G$"));
        assert!(stored.len() >= 3 || (table && from_table), "{body:#?}");
    }

    /// Each float-to-int conversion switched the rounding mode to chop and
    /// back: two conversions in a row loaded the control word four times.
    #[test]
    fn test_conversions_in_a_row_switch_the_rounding_mode_once() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/truncs.cgs")).unwrap();
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "truncs", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
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
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "diamond", None, &llrm_driver::m16_options_old(machine)).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let from = asm.find("_diamond proc").expect("_diamond");
        let body: Vec<&str> = asm[from..].lines().map(str::trim).take_while(|one| !one.ends_with("endp")).collect();
        assert!(body.last().is_some_and(|last| last.starts_with("ret")), "{body:#?}");
    }

    /// The listing of each function in `fixture`, `-Os` on a 486.
    fn listed(
        fixture: &str,
        functions: &[&str],
    ) -> Vec<Vec<String>> {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c/{fixture}.cgs")))
                .unwrap();
        let options = llrm_codegen::driver::Options {
            pipeline: llrm_codegen::driver::flags::Level::Os.options(),
            ..llrm_driver::m16_options_old(llrm_codegen::abi::machine::Machine {
                cpu: "486".to_owned(),
                ..llrm_x86_m16::machine::BUILT_IN.clone()
            })
        };
        let built = super::selected(&text, fixture, None, &options).expect("selects");
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
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
            assert!(
                !body.iter().any(|one| one.starts_with("shld") || one.starts_with("xor e"))
                    && body.iter().any(|one| one.starts_with("or ")),
                "{body:#?}"
            );
        }
    }

    /// A long returned in DX:AX and cast to a far pointer was joined into
    /// one register (`shl`, `shrd`) and split again (`shld`) to be stored.
    #[test]
    fn test_a_long_made_a_far_pointer_is_stored_as_its_words() {
        let [body] = listed("farwords", &["_keep"]).try_into().expect("one");
        assert!(
            !body.iter().any(|one| one.starts_with("shl") || one.starts_with("shrd") || one.starts_with("shld")),
            "{body:#?}"
        );
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
        let machine =
            llrm_codegen::abi::machine::Machine { cpu: "486".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
        let built = super::selected(&text, "x87crowd", None, &llrm_driver::m16_options_old(machine));
        assert!(built.is_ok(), "{:?}", built.err());
    }

    /// Watcom types a void function as an int whose returns give none;
    /// raised as `ret i16 poison`, isel refused all of qmove.
    #[test]
    fn test_a_void_function_is_selected() {
        let path = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/parity/qmove.cgs");
        let built = super::selected(
            &std::fs::read_to_string(path).unwrap(),
            "qmove",
            None,
            &llrm_driver::m16_options_old(llrm_codegen::abi::machine::Machine {
                cpu: "486".to_owned(),
                ..llrm_x86_m16::machine::BUILT_IN.clone()
            }),
        );
        assert!(built.is_ok(), "{:?}", built.err());
    }

    /// tests/fixtures/c32/`fixture`, raised under `-m32 level`, printed.
    fn flat_text(
        fixture: &str,
        level: &str,
    ) -> String {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32").join(fixture)).unwrap();
        let argv: Vec<String> = ["-m32", "--isel=old", level, "x.c"].map(str::to_owned).to_vec();
        let codegen = super::parse_args(&argv).unwrap().codegen;
        let (unit, profile) = super::prepared(&text, &codegen, false, false).unwrap();
        let program = super::translate::program(&unit, "test", codegen.arch.calling(), &profile).unwrap();
        let emitted = llrm_codegen::hir::mir::emit(&program, &llrm_x86_m32::layout()).swap_remove(0);
        assert_eq!(emitted.refused, Vec::<(String, String)>::new());
        llrm_mir::print::module(&emitted.module)
    }

    /// `total`, declared in a block of matmul's loop, had no `llvm.lifetime`
    /// calls, so no copy of the function carried a marker and the frame slots
    /// of locals in different blocks never shared (QCport's `_menu_draw`: 266
    /// to 3548 bytes inlined). Marked: one start and one end, none unmarked.
    #[test]
    fn a_local_in_a_loops_block_is_started_and_ended() {
        let text = flat_text("matmul.cgs", "-O1");
        assert_eq!(text.matches("call void @llvm.lifetime.start").count(), 1, "{text}");
        assert_eq!(text.matches("call void @llvm.lifetime.end").count(), 1, "{text}");
        assert!(!flat_text("matmul.cgs", "-O0").contains("lifetime"));
    }

    /// tests/fixtures/c32/`fixture`.cgs, as the 386 front end recorded it,
    /// selected for `-m32`.
    fn flat_listing(fixture: &str) -> Vec<String> {
        flat_listing_with(fixture, &["-O2"])
    }

    fn flat_listing_at(
        fixture: &str,
        level: &str,
    ) -> Vec<String> {
        flat_listing_with(fixture, &[level])
    }

    fn flat_listing_with(
        fixture: &str,
        flags: &[&str],
    ) -> Vec<String> {
        flat_listing_libcalls(fixture, flags, true)
    }

    /// The listing of a flat fixture where the runtime names `memcpy`
    /// (`libcalls`, as c.toml does) or does not.
    fn flat_listing_libcalls(
        fixture: &str,
        flags: &[&str],
        libcalls: bool,
    ) -> Vec<String> {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c32/{fixture}.cgs")))
                .unwrap();
        let argv: Vec<String> =
            ["-m32", "--isel=old"].into_iter().chain(flags.iter().copied()).chain(["x.c"]).map(str::to_owned).collect();
        let mut args = super::parse_args(&argv).unwrap();
        // As c.toml would name them, the runtime's own table naming none for
        // now.
        args.codegen.libcalls = if libcalls {
            super::libcalls_named(&llrm_x86_m32::M32, args.codegen.abi.as_deref(), "omf", vec!["memcpy".to_owned()])
                .unwrap()
        } else {
            Vec::new()
        };
        let built = super::selected(&text, fixture, None, &args.codegen).unwrap();
        llrm_codegen::backend::masm::text(&built).unwrap().lines().map(|line| line.trim().to_owned()).collect()
    }

    /// `j.a = 200` on a field that fills its byte was a read-modify-write of an
    /// unwritten byte: `load; and 0; or; store`. SROA reads an unwritten
    /// slot as poison, `and poison, 0` is poison, and bitfield_long_long
    /// printed 0 at -O2. A field filling its container is stored, as clang
    /// does.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_m32_a_field_filling_its_container_is_stored_not_merged() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("bf.c");
        std::fs::write(
            &source,
            "struct s { unsigned long long a:8, b:32; };\n\
             int __cdecl f(void) { struct s j; j.a = 200; j.b = 7; j.b += 5; return j.a * 1000 + j.b; }\n",
        )
        .unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
            .expect("wccq records bf.c");
        let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "x.c"].map(str::to_owned).to_vec();
        let built = super::selected(&text, "bf", None, &super::parse_args(&argv).unwrap().codegen).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(asm.contains("mov eax, 200012"), "{asm}");
    }

    /// A static function with a stride parameter, called with two strides,
    /// takes its word parameters in AX and DX and indexes with them in the 67h
    /// form: the entry marker defines them with no destination, and promote
    /// refused the function (x_dct `-m16 -Os -march=i386`: "value#1 has no
    /// word destination to promote").
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_m16_a_word_parameter_in_a_register_is_widened_for_the_67h_form() {
        let directory = tempfile::tempdir().unwrap();
        let (source, out) = (directory.path().join("d.c"), directory.path().join("d.asm"));
        std::fs::write(
            &source,
            "static int blk[8][8]; \n\
             static void dct8(int *d, int stride) \n\
             { \n\
                 int t0 = d[0] + d[7 * stride], t7 = d[0] - d[7 * stride], t1 = d[stride] + d[6 * stride], t6 = d[stride] - d[6 * stride]; \n\
                 int t2 = d[2 * stride] + d[5 * stride], t5 = d[2 * stride] - d[5 * stride], t3 = d[3 * stride] + d[4 * stride], t4 = d[3 * stride] - d[4 * stride]; \n\
                 int a = t0 + t3, b = t0 - t3, c = t1 + t2, e = t1 - t2; \n\
                 d[0] = a + c; d[4 * stride] = a - c; \n\
                 d[2 * stride] = (b * 5 + e * 2) >> 2; d[6 * stride] = (b * 2 - e * 5) >> 2; \n\
                 d[stride] = (t7 * 7 + t6 * 5 + t5 * 3 + t4) >> 3; d[3 * stride] = (t7 * 5 - t6 - t5 * 7 - t4 * 3) >> 3; \n\
                 d[5 * stride] = (t7 * 3 - t6 * 7 + t5 + t4 * 5) >> 3; d[7 * stride] = (t7 - t6 * 3 + t5 * 5 - t4 * 7) >> 3; \n\
             } \n\
             long f(int n) { int i; for (i = 0; i < n; ++i) dct8(&blk[i & 7][0], 1); dct8(&blk[0][n], 8); return blk[1][1]; }\n",
        )
        .unwrap();
        let argv =
            ["-m16", "--isel=old", "-Os", "-march=i386", "-S", source.to_str().unwrap(), "-o", out.to_str().unwrap()]
                .map(str::to_owned);
        assert_eq!(super::main(&argv), 0);
        let asm = std::fs::read_to_string(out).unwrap();
        assert!(asm.lines().any(|line| line.trim().starts_with("movzx e")), "{asm}");
    }

    /// A function that pushes an argument kept `ebp` from the allocator at -Os
    /// (framefree's "tuned for size" rule; the pin's `hasFP` has none): the
    /// frame register stayed a frame register, `mov ebp, esp` in the prologue.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_m32_os_frees_the_frame_register_of_a_function_that_pushes() {
        let directory = tempfile::tempdir().unwrap();
        let (source, out) = (directory.path().join("p.c"), directory.path().join("p.asm"));
        std::fs::write(
            &source,
            "extern int g(int, int); \n\
             int f(int *a) \n\
             { int p0 = a[0], p1 = a[1], p2 = a[2], p3 = a[3], p4 = a[4], p5 = a[5], p6 = a[6]; \n\
               int x = g(p0, p1); \n\
               return x + p0 * p1 + p2 * p3 + p4 * p5 + p6 * x + (p0 ^ p4) + (p1 ^ p5) + (p2 ^ p6) + (p3 ^ p0); }\n",
        )
        .unwrap();
        let argv = [
            "-m32",
            "-march=i486",
            "-mabi=sysv",
            "--isel=dag",
            "-Os",
            "-S",
            source.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]
        .map(str::to_owned);
        assert_eq!(super::main(&argv), 0);
        let asm = std::fs::read_to_string(out).unwrap();
        assert!(!asm.lines().any(|line| line.trim() == "mov ebp, esp"), "{asm}");
    }

    /// A byte stored to the absolute address 0xB8000 (text video memory) was
    /// `mov byte ptr [32768], 65`: the direct address was masked to 16 bits on
    /// every target (GORILLA/bload_bsave.bas printed the wrong cell at -O2).
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_m32_an_absolute_address_keeps_its_32_bits() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("vm.c");
        std::fs::write(
            &source,
            "void __cdecl f(void) { *(volatile char *)0xB8000 = 65; *(volatile char *)0xB8001 = 66; }\n",
        )
        .unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
            .expect("wccq records vm.c");
        let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "x.c"].map(str::to_owned).to_vec();
        let built = super::selected(&text, "vm", None, &super::parse_args(&argv).unwrap().codegen).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(!asm.contains("[32768]") && !asm.contains("[32769]"), "{asm}");
    }

    /// `(long long)a * (65500 + (i & 31))` is a dword times a dword: one `imul`
    /// into EDX:EAX. The old sign-bit count did not see that the divisor's
    /// range fits an i32, so x_fixmul widened both and multiplied in halves
    /// (`mul` plus two `imul` and an `add`; +111% clocks at -O1..-O3).
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_m32_a_product_of_two_i32_ranges_is_one_widening_imul() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("fm.c");
        std::fs::write(
            &source,
            "long fm(long a, int i) { return (long)(((long long)a * (65500 + (i & 31))) >> 16); }\n",
        )
        .unwrap();
        let text = super::recorded_for(&source, &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
            .expect("wccq records fm.c");
        let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "-march=i486", "x.c"].map(str::to_owned).to_vec();
        let built = super::selected(&text, "fm", None, &super::parse_args(&argv).unwrap().codegen).unwrap();
        let asm = llrm_codegen::backend::masm::text(&built).unwrap();
        let lines: Vec<&str> = asm.lines().map(str::trim).collect();
        assert!(lines.iter().any(|line| matches!(*line, "imul eax" | "imul edx" | "imul ecx" | "imul ebx")), "{asm}");
        assert!(!lines.iter().any(|line| line.starts_with("mul ")), "{asm}");
    }

    /// Ten floats live across a loop spill x87 values. Every spill cell was a
    /// qword, so a float reloaded from it kept double precision and
    /// nbody_single printed 86015, the pin 86016. The cell is the float's
    /// own dword, as the pin's RFP32 `fst dword` rounds; a double keeps its
    /// qword.
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_m32_a_float_spills_to_a_dword_cell_and_a_double_to_a_qword() {
        let listing = |kind: &str| {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("spill.c");
            std::fs::write(
                &source,
                format!(
                    "extern {kind} g[12];\n\
                     {kind} f(int n) {{ int i; {kind} a0 = g[0], a1 = g[1], a2 = g[2], a3 = g[3], a4 = g[4], \
                     a5 = g[5], a6 = g[6], a7 = g[7], a8 = g[8], a9 = g[9];\n\
                     for (i = 0; i < n; ++i) {{\n\
                     a0 = a0 * a1 + a9; a1 = a1 * a2 + a0; a2 = a2 * a3 + a1; a3 = a3 * a4 + a2; a4 = a4 * a5 + a3;\n\
                     a5 = a5 * a6 + a4; a6 = a6 * a7 + a5; a7 = a7 * a8 + a6; a8 = a8 * a9 + a7; a9 = a9 * a0 + a8; }}\n\
                     return a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8 + a9; }}\n"
                ),
            )
            .unwrap();
            let text = super::recorded_for(&source, &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
                .expect("wccq records spill.c");
            let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "-march=i486", "x.c"].map(str::to_owned).to_vec();
            let built = super::selected(&text, "spill", None, &super::parse_args(&argv).unwrap().codegen).unwrap();
            llrm_codegen::backend::masm::text(&built).unwrap()
        };
        let spills = |asm: &str, width: &str| {
            asm.lines()
                .filter(|line| line.trim().starts_with("fstp") && line.contains(&format!("{width} ptr [e")))
                .count()
        };
        let single = listing("float");
        assert_eq!(spills(&single, "qword"), 0, "{single}");
        assert!(spills(&single, "dword") > 0, "{single}");
        let double = listing("double");
        assert!(spills(&double, "qword") > 0, "{double}");
    }

    /// `int add(int, int)` as flat 32-bit code: cdecl32's arguments at [esp+4]
    /// and [esp+8] (EBP is no frame where nothing needs one), the result in
    /// EAX. It listed `bp`, `[bp+4]` and a DX:AX result, and never reached
    /// the allocator, before the target stated them. The operands are added
    /// highest rank first, as Reassociate has them: the pinned clang -m32
    /// -march=i486 -O2 lists `mov eax, [esp+8]; add eax, [esp+4]; ret`.
    #[test]
    fn test_m32_lists_add_as_flat_cdecl32() {
        let lines = flat_listing("add");
        assert_eq!(&lines[..2], [".386", ".model flat"]);
        let body: Vec<&str> = lines
            .iter()
            .skip_while(|line| *line != "_add proc near")
            .skip(1)
            .take_while(|line| *line != "_add endp")
            .map(String::as_str)
            .collect();
        assert_eq!(body, ["L0_0:", "mov eax, dword ptr [esp+8]", "add eax, dword ptr [esp+4]", "ret"]);
    }

    /// The body of `name`'s procedure in the flat listing of `regs.c`, as the
    /// default convention compiles it.
    fn regs_body(name: &str) -> Vec<String> {
        let lines = flat_listing("regs");
        lines
            .iter()
            .skip_while(|line| **line != format!("{name} proc near"))
            .skip(1)
            .take_while(|line| **line != format!("{name} endp"))
            .cloned()
            .collect()
    }

    /// `fixture`'s flat listing under `-mabi=sysv`: gcc's i386 convention.
    fn sysv_body(name: &str) -> Vec<String> {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32/sysv.cgs")).unwrap();
        let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "-mabi=sysv", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let built = super::selected(&text, "sysv", None, &args.codegen).unwrap();
        let lines: Vec<String> =
            llrm_codegen::backend::masm::text(&built).unwrap().lines().map(|line| line.trim().to_owned()).collect();
        lines
            .iter()
            .skip_while(|line| **line != format!("{name} proc near"))
            .skip(1)
            .take_while(|line| **line != format!("{name} endp"))
            .cloned()
            .collect()
    }

    /// gcc -m32 writes every struct result, a one-byte one too, through a
    /// pointer that is the first argument and pops it, `ret 4`: Open
    /// Watcom's rule (a 1, 2 or 4 byte struct in EAX, the address in ESI) was
    /// applied whatever the ABI.
    #[test]
    fn test_m32_sysv_writes_a_struct_result_through_the_first_argument_and_the_callee_pops_it() {
        assert_eq!(
            sysv_body("_r1"),
            ["L0_0:", "mov eax, dword ptr [esp+4]", "mov ecx, dword ptr [esp+8]", "mov byte ptr [eax], cl", "ret 4"]
        );
        assert_eq!(sysv_body("_r12").last().map(String::as_str), Some("ret 4"));
    }

    /// The caller of such a function removes the arguments but the address: gcc
    /// added 8 after pushing 12 bytes.
    #[test]
    fn test_m32_sysv_caller_does_not_remove_the_result_address_the_callee_popped() {
        let body = sysv_body("_calls");
        let after = body.iter().skip_while(|line| **line != "call _away").skip(1).cloned().collect::<Vec<_>>();
        assert!(after.iter().any(|line| line == "add esp, 8"), "{body:?}");
        assert!(!after.iter().any(|line| line == "add esp, 12"), "{body:?}");
    }

    /// An ELF object has no `_` before a C name and no `_` after a default one:
    /// `six_` and `_explicit_cdecl` were printed whatever object format was
    /// asked, where the convention's `symbol` table says `*` for elf.
    #[test]
    fn test_m32_an_elf_object_spells_its_symbols_without_decoration() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32/regs.cgs")).unwrap();
        let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "x.c"].map(str::to_owned).to_vec();
        let mut args = super::parse_args(&argv).unwrap();
        args.codegen.object_format = "elf";
        let built = super::selected(&text, "regs", None, &args.codegen).unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        for name in ["six", "result", "explicit_cdecl", "far_away"] {
            assert!(
                listing.contains(&format!("\n{name} proc near")) || listing.contains(&format!("extern {name}:near")),
                "{name} in {listing}"
            );
        }
        assert!(!listing.contains("six_") && !listing.contains("_explicit_cdecl"), "{listing}");
    }

    /// An unmarked C function takes Open Watcom's register convention: `six`
    /// read its fifth and sixth from the stack and popped them with `ret
    /// 8`, the first four being in EAX, EDX, EBX and ECX. It had refused such
    /// a function ("has a register calling convention"), compiled with -ecc.
    #[test]
    fn test_m32_an_unmarked_function_takes_registers_and_pops_its_stack_arguments() {
        let body = regs_body("six_");
        // The pin makes the sum of four registers a tree and splits its `lea`
        // into adds, so which register takes which add
        // is the allocator's: five adds, three of registers and two of the
        // stack words, no move, no `lea`.
        let adds = |from: &str| body.iter().filter(|line| line.starts_with("add ") && line.contains(from)).count();
        assert_eq!(body.len(), 7, "{body:?}");
        assert_eq!((body[0].as_str(), body[6].as_str()), ("L0_0:", "ret 8"), "{body:?}");
        assert_eq!((adds("esp"), adds("e")), (2, 5), "{body:?}");
        assert!(body[1..6].iter().all(|line| line.starts_with("add ")), "{body:?}");
    }

    /// A struct larger than a dword is written through the address in ESI,
    /// which comes back in EAX, as `wcc386` has it.
    #[test]
    fn test_m32_a_struct_result_is_written_through_esi_and_returned_in_eax() {
        let body = regs_body("result_");
        assert!(
            body.contains(&"mov dword ptr [esi], eax".to_owned()) && body.contains(&"mov eax, esi".to_owned()),
            "{body:?}"
        );
        assert_eq!(body.last().map(String::as_str), Some("ret"));
    }

    /// A one-byte struct argument travels in AL as an integer does; a
    /// twelve-byte one is in memory with `a` after it.
    #[test]
    fn test_m32_a_small_struct_argument_is_a_register_and_a_large_one_is_memory() {
        assert_eq!(regs_body("small_"), ["L4_0:", "movsx eax, al", "add eax, edx", "ret"]);
        let body = regs_body("by_value_");
        assert!(
            body.contains(&"mov eax, dword ptr [esp+12]".to_owned())
                && body.contains(&"add eax, dword ptr [esp+16]".to_owned()),
            "{body:?}"
        );
        assert_eq!(body.last().map(String::as_str), Some("ret 16"));
    }

    /// `__cdecl` names the stack convention and its `_name` symbol on a target
    /// whose default is registers.
    #[test]
    fn test_m32_an_explicit_cdecl_function_keeps_the_stack_and_its_caller_pops() {
        let body = regs_body("_explicit_cdecl");
        assert_eq!(body, ["L5_0:", "mov eax, dword ptr [esp+4]", "sub eax, dword ptr [esp+8]", "ret"]);
    }

    /// A narrow argument goes as a stack slot: `push ax` pushed two bytes, and
    /// cdecl32's next argument, and the callee's read of it, lay a dword
    /// apart.
    #[test]
    fn test_m32_pushes_narrow_arguments_as_dwords() {
        let lines = flat_listing("args");
        assert!(
            lines.iter().filter(|line| line.starts_with("push ")).all(|line| line
                .split_whitespace()
                .nth(1)
                .is_some_and(|operand| operand.starts_with('e') || operand.starts_with("offset"))),
            "{lines:#?}"
        );
        assert!(lines.contains(&"add esp, 8".to_owned()), "{lines:#?}");
    }

    /// Native 32-bit addressing reads whole registers: the pass that zeroed
    /// EBP's upper half for a cell read 32 bits wide (a 16-bit frame's
    /// `[bp]` under 32-bit addressing) wrote `movzx ebp, ebp` into a flat
    /// frame, and the reserve was 70 bytes, not a multiple of the dword stack.
    #[test]
    fn test_m32_keeps_the_dword_stack() {
        let lines = flat_listing("bytes");
        assert!(lines.iter().all(|line| !line.starts_with("movzx ebp")), "{lines:#?}");
        assert!(lines.contains(&"sub esp, 76".to_owned()), "{lines:#?}");
        // 70 under the frame register, 76 reserved with its cell: 2 above the
        // stack pointer.
        assert!(lines.contains(&"mov byte ptr [esp+eax+2], al".to_owned()), "{lines:#?}");
    }

    /// A struct wider than 16 bytes passed by value is one `byval` pointer and
    /// a copy, not a parameter and a store per word: a 4 KB one was 1,024
    /// parameters, and the backend quadratic in them (pr20621-1: a 64 KB one
    /// never finished compiling).
    // It records C through wccq, which only the toolchain feature builds.
    #[cfg(feature = "toolchain")]
    #[test]
    fn test_a_large_struct_passed_by_value_is_one_copy_not_a_word_each() {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("big.c");
        std::fs::write(&source, "struct big { int i[1024]; };\nint foo(struct big b, int x) { return b.i[x]; }\nint main(void) { struct big g; g.i[3] = 7; return foo(g, 3); }\n").unwrap();
        let argv: Vec<String> = ["-m32", "--isel=old", "-O0", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let recorded = super::recorded_for(&source, &[], false, &[], &super::Profile::of(&llrm_x86_m32::M32).unwrap())
            .expect("records big.c");
        let built = super::selected(&recorded, "big", None, &args.codegen).unwrap();
        let lines: Vec<String> =
            llrm_codegen::backend::masm::text(&built).unwrap().lines().map(|line| line.trim().to_owned()).collect();
        assert!(lines.len() < 80, "{} lines", lines.len());
        assert!(lines.contains(&"rep movsd".to_owned()) && lines.contains(&"ret 4100".to_owned()), "{lines:#?}");
    }

    /// `fixture`'s flat object, as records: (type, body).
    fn flat_object(fixture: &str) -> Vec<(u8, Vec<u8>)> {
        flat_object_with(fixture, &[])
    }

    fn flat_object_with(
        fixture: &str,
        flags: &[&str],
    ) -> Vec<(u8, Vec<u8>)> {
        let text =
            std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join(format!("tests/fixtures/c32/{fixture}.cgs")))
                .unwrap();
        let argv: Vec<String> =
            ["-m32", "--isel=old", "-O2"].iter().chain(flags).chain(&["x.c"]).map(|one| (*one).to_owned()).collect();
        let mut args = super::parse_args(&argv).unwrap();
        // The objects asserted here are the selector's own copies, `rep movs`
        // among them; the flip's are `flat_listing`'s.
        args.codegen.libcalls.clear();
        let built = super::selected(&text, fixture, None, &args.codegen).unwrap();
        let bytes = llrm_codegen::backend::objbuild::written(&built, "x.c").unwrap();
        let (mut at, mut out) = (0, Vec::new());
        while at < bytes.len() {
            let length = usize::from(u16::from_le_bytes([bytes[at + 1], bytes[at + 2]]));
            out.push((bytes[at], bytes[at + 3..at + 2 + length].to_vec()));
            at += 3 + length;
        }
        out
    }

    /// A flat object is USE32 and its records are the 32-bit ones: a segment
    /// with its 32-bit length, a public with a 32-bit offset, data at a
    /// 32-bit offset, no group; and `add`'s code is the 32-bit encoding of
    /// what its listing says. It was 16-bit records and `67 8b 46 08` (`mov
    /// eax, [bp+8]` under an address-size prefix) ending in `66 c3` (a word
    /// return).
    #[test]
    fn test_m32_writes_a_use32_object_with_its_own_encoding() {
        let records = flat_object("add");
        let kinds: Vec<u8> = records.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(
            kinds,
            [0x80, 0x96, 0x99, 0x99, 0x91, 0xA1, 0x8A],
            "THEADR LNAMES SEGDEF32 x2 PUBDEF32 LEDATA32 MODEND"
        );
        assert!(records[2].1[0] & 1 == 1 && records[3].1[0] & 1 == 1, "both segments are USE32: {records:?}");
        let code = &records[5].1;
        assert_eq!(hex(&code[5..]), "8b44240803442404c3", "mov eax,[esp+8]; add eax,[esp+4]; ret");
    }

    /// A near procedure that pops its own arguments returns `ret 8` (`c2
    /// 0800`): the word form, `66 c2 0800`, pops a 16-bit return address
    /// and sent the flat program into the vector table. A function of six
    /// arguments has two on the stack under Watcom's convention, which its
    /// callee pops.
    #[test]
    fn test_m32_returns_popping_arguments_with_a_dword_ret() {
        let object = flat_object_with("regs", &[]);
        let code = hex(&object.iter().find(|(kind, _)| *kind == 0xA1).expect("code").1);
        assert!(code.contains("c20800") && !code.contains("66c2"), "{code}");
    }

    /// A flat string operation takes its operands in ESI, EDI and ECX with no
    /// segment operand: it was `movs dword ptr es:[di], dword ptr ebx:[si]`
    /// with the source and count in whatever registers the allocator chose,
    /// and a refusal ("may be in no register") before that. A struct copy
    /// (strcopy.c) is the string operation: the old fixture, a copy loop
    /// between two frame arrays, is no `rep movsd` any more, the copy being
    /// dead.
    #[test]
    fn test_m32_lists_a_string_copy_through_esi_edi_ecx() {
        // Where the runtime names no `memcpy`: the copy stays the string
        // operation.
        let lines = flat_listing_libcalls("strcopy", &["-O2"], false);
        let at = lines.iter().position(|line| line == "rep movsd").unwrap_or_else(|| panic!("a rep movsd: {lines:#?}"));
        let before = &lines[..at];
        assert!(
            before.iter().any(|line| line.starts_with("mov esi") || line.starts_with("lea esi"))
                && before.iter().any(|line| line.starts_with("mov edi") || line.starts_with("lea edi")),
            "{before:#?}"
        );
        assert!(
            before.iter().rev().take(4).any(|line| line.starts_with("mov ecx") || line == "shr ecx, 2"),
            "{before:#?}"
        );
        assert!(!lines.iter().any(|line| line.contains("es:") || line.contains("ebx:")), "{lines:#?}");
        let object = flat_object("strcopy");
        let code = hex(&object.iter().find(|(kind, _)| *kind == 0xA1).expect("code").1);
        assert!(code.contains("f3a5") && !code.contains("66f3a5"), "rep movsd, not its word form: {code}");
    }

    /// The same copy where the m32 runtime names `memcpy` (c.toml `libcalls`):
    /// a call of `memcpy_`, the pointers' alignment being unknown, and no `rep
    /// movs`.
    #[test]
    fn test_m32_calls_memcpy_for_a_string_copy_the_runtime_has_a_memcpy_for() {
        let lines = flat_listing("strcopy");
        assert!(lines.iter().any(|line| line == "call memcpy_"), "a call of memcpy_: {lines:#?}");
        assert!(!lines.iter().any(|line| line.starts_with("rep movs")), "no rep movs: {lines:#?}");
    }

    /// `-fsanitize=stack` compares ESP with a dword limit and enters a near
    /// handler: `cmp sp, word ptr` and `call far ptr` were a 16-bit compare
    /// and a far call in a flat program.
    #[test]
    fn test_m32_checks_its_stack_against_a_dword_limit_and_a_near_handler() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c32/add.cgs")).unwrap();
        let argv: Vec<String> = ["-m32", "--isel=old", "-O2", "x.c"].map(str::to_owned).to_vec();
        let args = super::parse_args(&argv).unwrap();
        let built = super::selected_checking(
            &text,
            "add",
            None,
            &args.codegen,
            Some(super::stack_check(&llrm_x86_m32::M32)),
            false,
            false,
        )
        .unwrap();
        let listing = llrm_codegen::backend::masm::text(&built).unwrap();
        assert!(
            listing.contains("cmp esp, dword ptr LL$STACK_LOW") && listing.contains("call __STKOVERFLOW"),
            "{listing}"
        );
        assert!(!listing.contains("far ptr") && listing.contains("extern __STKOVERFLOW:near"), "{listing}");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The body of `name` in the flat listing of `fixture`.
    fn flat_body(
        fixture: &str,
        name: &str,
    ) -> Vec<String> {
        flat_body_with(fixture, name, &["-O2"])
    }

    fn flat_body_with(
        fixture: &str,
        name: &str,
        flags: &[&str],
    ) -> Vec<String> {
        let lines = flat_listing_with(fixture, flags);
        lines
            .iter()
            .skip_while(|line| **line != format!("_{name} proc near"))
            .skip(1)
            .take_while(|line| **line != format!("_{name} endp"))
            .cloned()
            .collect()
    }

    /// A short read through a pointer and widened is one `movsx`. The peephole
    /// asked the 16-bit encoder whether `movsx eax, word ptr [eax]`
    /// encodes, which it does not, so a flat compile kept `mov ax, word ptr
    /// [eax]; movsx eax, ax`.
    #[test]
    fn test_m32_widens_a_load_in_one_instruction() {
        let body = flat_body("extend", "extend");
        assert!(body.contains(&"movsx eax, word ptr [eax]".to_owned()), "{body:#?}");
    }

    /// Every address register holds a pointer loaded from the arguments. The
    /// peephole decoded the bytes of the flat code it was judging as 16-bit
    /// code, read the effects of `movsx eax, word ptr [ebx]` wrongly, and
    /// moved it above a read of EAX: `movsx eax, [ebx]; movsx ebx, [eax]`
    /// loaded through a value.
    #[test]
    fn test_m32_never_addresses_through_a_value() {
        let body = flat_body("extend", "extend2");
        let mut pointers: Vec<&str> = Vec::new();
        for line in &body {
            let Some((operation, operands)) = line.split_once(' ') else { continue };
            let Some((destination, rest)) = operands.split_once(", ") else { continue };
            if let Some(inside) =
                rest.split_once('[').and_then(|(_, tail)| tail.split_once(']')).map(|(inside, _)| inside)
            {
                for register in inside
                    .split(|one: char| !one.is_ascii_alphanumeric())
                    .filter(|one| one.len() == 3 && one.starts_with('e'))
                {
                    assert!(
                        register == "ebp" || register == "esp" || pointers.contains(&register),
                        "{register} is a value in `{line}`: {body:#?}"
                    );
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

    /// A word sum is the word `add`, not a dword one and the `movzx` of its low
    /// word: its upper half stays as it was, which is zero where the words
    /// were zero-extended.
    #[test]
    fn test_m32_adds_words_as_words() {
        let body = flat_body("narrow", "short_sum");
        assert!(
            body.iter().any(|line| line.starts_with("add ax, "))
                && !body.iter().any(|line| line.starts_with("add eax, ")),
            "{body:#?}"
        );
    }

    /// The same sum of dwords keeps the `movzx`: the word `add` would leave the
    /// upper half as it was, and the result is read as a dword. The combiner
    /// makes the extension `and eax, 65535`; the pin selects it back to `movzx`
    /// (X86InstrCompiler.td), which the 486 and the Pentium price at 3 clocks
    /// against 1: so for size (-Os), and `and` stays at -O2.
    #[test]
    fn test_m32_keeps_the_extension_where_the_upper_half_is_not_zero() {
        let small = flat_body_with("narrow", "int_sum", &["-Os"]);
        assert!(
            small.iter().any(|line| line.starts_with("add eax, ")) && small.iter().any(|line| line == "movzx eax, ax"),
            "{small:#?}"
        );
        let fast = flat_body("narrow", "int_sum");
        assert!(
            fast.iter().any(|line| line == "and eax, 65535") && !fast.iter().any(|line| line.starts_with("movzx")),
            "{fast:#?}"
        );
    }

    /// The sieve's inner loop indexes its array with a word counter, and the
    /// counter's register is the index: the zero extension of a word whose
    /// register is zero above it was a copy into another one that the store
    /// read, one more instruction a trip than m16's `[bp+si]`.
    #[test]
    fn test_m32_indexes_through_the_counter_not_a_copy_of_it() {
        let body = flat_body("sieve", "bench_sieve");
        // From the store the loop makes (the trip's extension is the first
        // thing after it).
        let from = body.iter().position(|line| line.starts_with("mov byte ptr [")).expect("the loop stores a byte");
        // To the last jump before the epilogue: the copied guard leaves the
        // function by more than one return, whose `movzx` is not a trip's.
        let ret = body[from..].iter().position(|line| line == "ret").expect("the function returns") + from;
        let last_jump = body[from..ret].iter().rposition(|line| line.starts_with('j')).expect("the loop jumps") + from;
        let loop_ = &body[from..=last_jump];
        let copies = |line: &&String| {
            line.starts_with("movzx e")
                || line.split_once(' ').is_some_and(|(operation, operands)| {
                    operation == "mov"
                        && operands.split_once(", ").is_some_and(|(to, from)| {
                            to.len() == 3 && from.len() == 3 && to.starts_with('e') && from.starts_with('e')
                        })
                })
        };
        // The counter is `(unsigned short)(multiple + i)` against an unknown
        // `limit`: the 16-bit add may wrap, so it cannot be widened,
        // and gcc zero-extends it every trip too (two instructions to ours
        // one). The zero copies llrm once had came from ECX's upper
        // half happening to be zero where the peephole could see it
        // (register-choice luck); with EBP a value register it is one.
        assert!(loop_.iter().filter(copies).count() <= 1, "{loop_:#?}");
    }

    /// `d = a + b` into a register that is neither is one `lea`: no flags to
    /// keep, three bytes for the four of `mov; add`, and a native form flat
    /// code has without a prefix. The target priced a flat dword's address
    /// as the real-mode prefixed one and had no price for the native form, so
    /// it never chose it.
    #[test]
    fn test_m32_adds_into_a_new_register_with_lea() {
        let body = flat_body("lea", "lea_sum");
        assert!(
            body.iter().any(|line| line.starts_with("lea e") && line.contains("[e") && line.contains("+e")),
            "{body:#?}"
        );
        assert!(
            !body.iter().any(|line| line != "mov ebp, esp"
                && line.starts_with("mov e")
                && line.split_once(", ").is_some_and(|(_, from)| from.starts_with('e') && from.len() == 3)),
            "{body:#?}"
        );
    }

    /// A loop over `int *`: the pointer, the index and the sum are dwords in
    /// 32-bit registers, addressed `[base+index]` with no segment, selector
    /// or 16-bit register.
    #[test]
    fn test_m32_lists_a_loop_over_int_pointers() {
        let lines = flat_listing("sum");
        let body: Vec<&str> = lines
            .iter()
            .skip_while(|line| *line != "sum_ proc near")
            .skip(1)
            .take_while(|line| *line != "sum_ endp")
            .map(String::as_str)
            .collect();
        // The load is the add's operand, addressed by a 32-bit register; the
        // pointer or the index steps by a constant and a conditional
        // jump ends the loop (llc lists `inc; cmp; jl` or, counting to zero,
        // `add -1; jne`).
        let at = body.iter().position(|line| line.starts_with("add e") && line.contains("dword ptr [e"));
        let at = at.unwrap_or_else(|| panic!("{body:#?}"));
        let jump = body[at..].iter().position(|line| line.starts_with('j') && !line.starts_with("jmp")).map(|k| at + k);
        let jump = jump.unwrap_or_else(|| panic!("{body:#?}"));
        let steps = body[at + 1..jump]
            .iter()
            .filter(|line| {
                line.starts_with("inc e")
                    || line.starts_with("dec e")
                    || ((line.starts_with("add e") || line.starts_with("sub e"))
                        && line.rsplit(", ").next().is_some_and(|last| last.parse::<i64>().is_ok()))
            })
            .count();
        assert!(steps >= 2, "the pointer or index and the counter step by constants: {body:#?}");
        assert!(
            body.iter().all(|line| !line.contains(" bp")
                && !line.contains("[bx")
                && !line.contains("es:")
                && !line.contains("far")),
            "{body:#?}"
        );
        assert!(
            matches!(body.last(), Some(&"ret") | Some(&"jmp L0_3")),
            "ends by a return or a jump to the shared one: {body:#?}"
        );
    }
}
