//! LLVM IR to selected machine nodes: clang's front end alone makes the IR, the
//! shared mid-end optimizes it, this backend selects it. Allocation and
//! emission are not here yet.

use std::rc::Rc;

pub use llrm_codegen::driver::flags::apply_pass_flag;
use llrm_mir::program::Program;
use llrm_target::Target;
use llrm_transforms::pipeline::{Applied, Options};

use crate::dagisel::function::{Selection, select_function};
use crate::dagisel::hooks::Cpu;
use crate::target::Z80;

/// How far the mid-end goes.
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
}

/// Everything a compile is told besides the IR.
pub struct Request {
    pub cpu: Cpu,
    pub level: Level,
    /// `-f[no-]PASS` switches, applied to the mid-end.
    pub passes: Vec<String>,
}

impl Request {
    pub fn new(level: Level) -> Self {
        Self { cpu: Cpu::Z80, level, passes: Vec::new() }
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
pub fn program(
    text: &str,
    request: &Request,
) -> Result<Program, String> {
    let mut module = llrm_mir::parse::module(&without_triple(text)).map_err(|error| error.to_string())?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let layout = Z80.layout().datalayout;
    match module.datalayout.as_deref() {
        Some(stated) if stated == layout => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not the Z80's `{layout}`")),
        None => return Err(format!("the module states no datalayout; the Z80's is `{layout}`")),
    }
    let mut program = Program::new(vec![module], machine())?;
    if request.level != Level::O0 {
        let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
        let mut options = request.level.options();
        // Bytes over speed on an 8-bit target: no unrolling (the 6502's
        // stand-in for the override table).
        options.unroll = false;
        options.loop_unrolling = false;
        for flag in &request.passes {
            apply_pass_flag(&mut options, flag);
        }
        llrm_transforms::pipeline::applied(&mut program, &Applied { options, only: None, dump })?;
        program.modules.iter_mut().try_for_each(llrm_transforms::memsetpattern::expanded)?;
    }
    Ok(program)
}

/// Every function `text` defines, selected: its name and the result, or why it
/// was refused.
pub fn selections(
    text: &str,
    request: &Request,
) -> Result<Vec<(String, Result<Selection, String>)>, String> {
    let mut program = program(text, request)?;
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
    let module = &program.modules[0];
    let names: Vec<String> = module
        .globals
        .iter()
        .filter(|global| global.function().is_some_and(|one| !one.is_declaration()))
        .filter_map(|global| global.name.as_deref().map(str::to_owned))
        .collect();
    Ok(names
        .into_iter()
        .map(|name| {
            let selected = select_function(module, &name, request.cpu);
            (name, selected)
        })
        .collect())
}

/// The cost model the mid-end reads: the pin's `opt` with no target, whose
/// `Basic` TargetTransformInfo prices the loop-idiom, inline and
/// strength-reduction decisions.
pub fn machine() -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Z80Machine)
}

struct Z80Machine;

impl llrm_mir::target::Machine for Z80Machine {
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }

    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        Some(Rc::new(crate::tti::Z80Tti::new()))
    }
}
