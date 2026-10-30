//! LLVM IR to ca65 text: `llc`'s job over `opt`'s. Clang's front end alone
//! makes the IR; the shared mid-end optimizes it; this backend selects,
//! allocates and prints it. The runtime (`runtime/mos_rt.s`) is linked, not
//! printed.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_target::Target;
use llrm_transforms::pipeline::{Applied, Options};

use crate::alloc::Tuning;
use crate::dagisel::function::select_function;
use crate::dagisel::hooks::Cpu;
use crate::emit::equates;
use crate::platform::{Platform, SIM65};
use crate::select::module_text;
use crate::target::Mos;

pub use llrm_codegen::driver::flags::apply_pass_flag;

/// How far the mid-end goes; `-Os` and `-Oz` also price a byte at the size
/// level's clocks (`Tuning::SIZE`).
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

    fn options(self) -> Options {
        match self {
            Self::O0 => Options::none(),
            Self::O1 => Options::basic(),
            Self::O2 => Options::default(),
            Self::O3 => Options::speed(),
            Self::Os => Options::size(),
            Self::Oz => Options::min_size(),
        }
    }

    fn tuning(self) -> Tuning {
        match self {
            Self::Os | Self::Oz => Tuning::SIZE,
            _ => Tuning::SPEED,
        }
    }
}

/// The platform a row of `ROWS` names, or sim65.
pub fn platform_named(name: &str) -> Result<Platform, String> {
    let rows = std::iter::once(SIM65).chain(crate::platform::ROWS);
    rows.filter_map(|text| Platform::parse(text).ok()).find(|one| one.name == name).ok_or_else(|| format!("no platform {name}"))
}

/// Everything a compile is told besides the IR.
pub struct Request {
    pub platform: Platform,
    pub cpu: Cpu,
    pub level: Level,
    /// `-f[no-]PASS` switches, applied to the mid-end.
    pub passes: Vec<String>,
}

impl Request {
    pub fn new(level: Level) -> Self {
        Self { platform: Platform::parse(SIM65).expect("sim65 parses"), cpu: Cpu::Mos6502, level, passes: Vec::new() }
    }
}

/// `text` without the lines MIR's reader leaves out of LLVM (`target triple`,
/// comments) and `dso_local`.
fn without_triple(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("target triple") && !line.starts_with(';'))
        .map(|line| line.replace(" dso_local", ""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text` through the mid-end at the request's level.
fn program(
    text: &str,
    request: &Request,
) -> Result<Program, String> {
    let mut module = llrm_mir::parse::module(&without_triple(text)).map_err(|error| error.to_string())?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let layout = Mos.layout().datalayout;
    match module.datalayout.as_deref() {
        Some(stated) if stated == layout => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not the 6502's `{layout}`")),
        None => return Err(format!("the module states no datalayout; the 6502's is `{layout}`")),
    }
    let mut program = Program::new(vec![module], Rc::new(llrm_mir::target::Neutral))?;
    if request.level != Level::O0 {
        let mut options = request.level.options();
        // Bytes over speed on an 8-bit target: no unrolling (the stand-in for the
        // target's override-table entry).
        options.unroll = false;
        options.loop_unrolling = false;
        for flag in &request.passes {
            apply_pass_flag(&mut options, flag);
        }
        llrm_transforms::pipeline::applied(&mut program, &Applied { options, only: None, dump: None })?;
        program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    }
    Ok(program)
}

/// The assembly of every function `text` defines: the platform's register
/// equates, then each function in order.
pub fn assembly(
    text: &str,
    request: &Request,
) -> Result<String, String> {
    let mut program = program(text, request)?;
    llrm_6502_prepare(&mut program);
    let module = &program.modules[0];
    let names: Vec<String> = module
        .globals
        .iter()
        .filter(|global| global.function().is_some_and(|one| !one.is_declaration()))
        .filter_map(|global| global.name.as_deref().map(str::to_owned))
        .collect();
    if module.globals.iter().any(|global| global.function().is_none() && llrm_mir::program::defines(global)) {
        return Err("a module with data is not supported yet".into());
    }
    let mut selections = Vec::new();
    for name in &names {
        let mut selection = select_function(module, name, request.cpu)?;
        selection.zero_page = request.platform.zero_page;
        selection.tuning = request.level.tuning();
        selections.push(selection);
    }
    let parts: Vec<(&str, &_)> = names.iter().map(String::as_str).zip(&selections).collect();
    Ok(equates(&request.platform) + &module_text(module, &parts)?.concat())
}

fn llrm_6502_prepare(program: &mut Program) {
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
}
