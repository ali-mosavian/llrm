//! LLVM IR to m68k assembly text: `llc`'s job over `opt`'s. Clang's front end
//! alone makes the IR (`-O2 -Xclang -disable-llvm-passes`); the shared mid-end
//! optimizes it; the m68k backend selects, allocates and prints.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_mir::target::Machine;
use llrm_transforms::pipeline::{Applied, Options as MidEnd};

use crate::compile::{Options, text_with};
use crate::dagisel::hooks::Cpu;
use crate::machine::M68kMachine;

/// How far the mid-end goes: gcc's and clang's `-O` levels, as `llc` takes
/// them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    O0,
    O1,
    O2,
    O3,
    Os,
    Oz,
}

impl Level {
    /// The level `-O<text>` names.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(match text {
            "0" => Self::O0,
            "" | "1" => Self::O1,
            "2" => Self::O2,
            "3" => Self::O3,
            "s" => Self::Os,
            "z" => Self::Oz,
            _ => return Err(format!("unknown optimization level -O{text}; choose -O0, -O1, -O2, -O3, -Os or -Oz")),
        })
    }

    fn mid_end(self) -> MidEnd {
        match self {
            Self::O0 => MidEnd::none(),
            Self::O1 => MidEnd::basic(),
            Self::O2 => MidEnd::default(),
            Self::O3 => MidEnd::speed(),
            Self::Os => MidEnd::size(),
            Self::Oz => MidEnd::min_size(),
        }
    }
}

/// The CPU `-mcpu=` names.
pub fn cpu_of(name: &str) -> Result<Cpu, String> {
    match name {
        "68000" | "m68000" => Ok(Cpu::M68000),
        "68010" | "m68010" => Ok(Cpu::M68010),
        "68020" | "m68020" => Ok(Cpu::M68020),
        "68030" | "m68030" => Ok(Cpu::M68030),
        "68040" | "m68040" => Ok(Cpu::M68040),
        "68060" | "m68060" => Ok(Cpu::M68060),
        _ => Err(format!("unknown m68k cpu `{name}`; choose 68000, 68010, 68020, 68030, 68040 or 68060")),
    }
}

/// The machine the mid-end is handed: its cost model prices the passes that
/// need one (loop strength reduction, the CRC recognizer).
pub fn machine() -> Rc<dyn Machine> {
    Rc::new(M68kMachine)
}

/// `text`, LLVM IR from clang, as a program: the lines and words MIR's reader
/// leaves out of LLVM dropped, the datalayout checked against the target's.
pub fn program(text: &str) -> Result<Program, String> {
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with("target triple") || line.starts_with(';') {
            continue;
        }
        kept.push_str(&line.replace(" dso_local", ""));
        kept.push('\n');
    }
    let mut module = llrm_mir::parse::module(&kept).map_err(|error| error.to_string())?;
    module.refuse_aliases("m68k")?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let expected = crate::kernels::DATALAYOUT;
    match module.datalayout.as_deref() {
        Some(stated) if stated == expected => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not m68k's `{expected}`")),
        None => return Err(format!("the module states no datalayout; m68k's is `{expected}`")),
    }
    Program::new(vec![module], machine())
}

/// `text` through the shared mid-end at `level` and the m68k backend, as
/// assembly: each defined function's label and instructions.
pub fn assembly(
    text: &str,
    cpu: Cpu,
    level: Level,
) -> Result<String, String> {
    assembly_with_debug(text, cpu, level, false)
}

/// [`assembly`], with each instruction's source place (`.loc`) where `debug`.
pub fn assembly_with_debug(
    text: &str,
    cpu: Cpu,
    level: Level,
    debug: bool,
) -> Result<String, String> {
    let dump = llrm_support::env_var("LLRM_MIR_STAGES").map(std::path::PathBuf::from);
    assembly_with(text, cpu, level, debug, dump.as_deref())
}

/// [`assembly`], writing the mid-end's stages to `dump` (`N/NNN-stage.ll`, as
/// every other target's `--dump DIR` and `LLRM_MIR_STAGES` do).
pub fn assembly_dumping(
    text: &str,
    cpu: Cpu,
    level: Level,
    dump: Option<&std::path::Path>,
) -> Result<String, String> {
    assembly_with(text, cpu, level, false, dump)
}

/// [`assembly`] with the debug lines and the stage dump chosen.
pub fn assembly_with(
    text: &str,
    cpu: Cpu,
    level: Level,
    debug: bool,
    dump: Option<&std::path::Path>,
) -> Result<String, String> {
    let mut program = program(text)?;
    let options = MidEnd { library: Vec::new(), ..level.mid_end() };
    if let Some(dump) = dump {
        std::fs::create_dir_all(dump).map_err(|error| format!("{}: {error}", dump.display()))?;
    }
    let applied = Applied { options, dump: dump.map(Into::into), ..Applied::default() };
    llrm_transforms::pipeline::applied(&mut program, &applied)?;
    program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    // `llvm.scmp` and `llvm.ucmp` have no m68k instruction: the generic
    // expansion
    program.modules.iter_mut().try_for_each(|module| llrm_codegen::backend::compares::expanded(module, |_| true))?;
    // The mid-end is the module's, run once: `text_with` runs it over the whole
    // module for each function it is given.
    let optimized = crate::compile::mid_end(&program.modules[0])?;
    let module = &optimized;
    let backend = Options { size: matches!(level, Level::Os | Level::Oz), debug, mid_end: false, ..Options::default() };
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().ok_or("an unnamed function")?;
        out.push_str(&text_with(module, &name, cpu, backend).map_err(|why| format!("@{name}: {why}"))?);
    }
    Ok(out)
}
