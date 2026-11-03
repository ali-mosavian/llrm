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
    /// `-g`: SDCC's `C$file$line$..` labels ahead of the instructions.
    pub debug: bool,
}

impl Request {
    pub fn new(level: Level) -> Self {
        Self { cpu: Cpu::Z80, level, passes: Vec::new(), debug: false }
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
    module.refuse_aliases("the z80")?;
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let layout = Z80.layout().datalayout;
    match module.datalayout.as_deref() {
        Some(stated) if stated == layout => {}
        Some(stated) => return Err(format!("the module's datalayout `{stated}` is not the Z80's `{layout}`")),
        None => return Err(format!("the module states no datalayout; the Z80's is `{layout}`")),
    }
    if request.debug {
        llrm_mir::dbgimport::types_and_globals(&mut module);
    }
    let mut program = Program::new(vec![module], machine())?;
    if request.level != Level::O0 {
        let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
        let mut options = request.level.options();
        // Bytes over speed on an 8-bit target: no unrolling (the 6502's
        // stand-in for the override table).
        options.unroll = false;
        options.loop_unrolling = false;
        // No zero- or sign-extending load: the compare of a loaded byte stays a
        // byte compare (`cp #255`), not a word compare of its extension.
        options.extload = false;
        // The runtime's `strlen` (`cpir`): LoopIdiomRecognize may form a call.
        options.library = vec![llrm_mir::libfunc::Routine {
            standard: "strlen",
            symbol: "_strlen".to_owned(),
            calling_convention: 0,
        }];
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

/// The source file of the unit's `!DICompileUnit` and the line function `name`
/// starts at (its `!DISubprogram`'s `scopeLine`), if `text` has them.
pub fn source_of(
    text: &str,
    name: &str,
) -> Option<(String, Option<u32>)> {
    let module = llrm_mir::parse::module(&without_triple(text)).ok()?;
    let (file, _) = llrm_mir::dbgimport::unit_file(&module)?;
    let line = module.named(name).and_then(|id| module.subprogram_line(module.global(id).function()?));
    Some((file, line))
}

/// SDCC's size and spelling of a debug type, as the `.adb` writes it: the class
/// and then the sign (`SI:S` int, `SI:U` unsigned int, `SC:U` char, `DG,SC:U`
/// pointer to char); none for one it has no spelling for.
pub(crate) fn sdcc_type(
    module: &llrm_mir::module::Module,
    id: llrm_mir::module::MetadataId,
) -> Option<(i64, String)> {
    use llrm_mir::debuginfo::{Kind, Scalar, read_type};
    let one = read_type(module, id)?;
    match one.kind {
        Kind::Scalar => {
            let (size, class, sign) = match Scalar::from_value(&one.name)? {
                Scalar::Void => (0, "SV", "S"),
                // DW_ATE_signed_char says signed, whatever SDCC's plain char is
                Scalar::Char | Scalar::Int8 => (1, "SC", "S"),
                Scalar::UInt8 => (1, "SC", "U"),
                Scalar::Int16 => (2, "SI", "S"),
                Scalar::UInt16 => (2, "SI", "U"),
                Scalar::Int32 => (4, "SL", "S"),
                Scalar::UInt32 => (4, "SL", "U"),
                Scalar::Float32 => (4, "SF", "S"),
                _ => return None,
            };
            Some((size, format!("{class}:{sign}")))
        }
        Kind::Pointer => Some((2, format!("DG,{}", sdcc_type(module, one.target?)?.1))),
        _ => None,
    }
}

/// The module's `.adb` (ASxxxx debug records, which sdld merges into the
/// `.cdb`): the module, then each defined function with its result type.
/// None without debug info. The lines come from the `C$` symbols.
pub fn adb(
    text: &str,
    request: &Request,
) -> Option<String> {
    use llrm_spike_mf::VariablePlace;
    let mut program = program(text, request).ok()?;
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
    let module = &program.modules[0];
    let (file, _) = llrm_mir::dbgimport::unit_file(module)?;
    let stem = file.rsplit_once('.').map_or(file.as_str(), |(stem, _)| stem);
    let mut out = format!("M:{stem}\n");
    for one in llrm_mir::debuginfo::functions(module) {
        let procedure = llrm_mir::debuginfo::read_type(module, one.r#type)?;
        let result = match procedure.target {
            Some(result) => sdcc_type(module, result)?.1,
            None => "SV:S".to_owned(),
        };
        let size = 2;
        out.push_str(&format!("F:G${}$0_0$0({{{size}}}DF,{result}),C,0,0,0,0,0\n", one.function));
        out.push_str(&format!("S:G${}$0_0$0({{{size}}}DF,{result}),C,0,0\n", one.function));
        let Ok(selection) = select_function(module, &one.function, request.cpu) else { continue };
        // Frame objects sit below IX, the locals first (`expand::layout`).
        let mut below = 0;
        let ends: Vec<i64> = selection
            .locals
            .iter()
            .map(|&bytes| {
                below += bytes;
                below
            })
            .collect();
        for (at, variable) in selection.variables.iter().enumerate() {
            let Some((size, spelling)) = sdcc_type(module, variable.r#type) else { continue };
            let place = match variable.place {
                VariablePlace::Register(register) => format!("R,0,0,{}", crate::expand::register_list(register)),
                VariablePlace::Frame { index, offset } => {
                    format!("B,1,{}", offset - ends.get(index as usize).copied().unwrap_or(0))
                }
            };
            out.push_str(&format!(
                "S:L{stem}.{}${}$1_0${at}({{{size}}}{spelling}),{place}\n",
                one.function, variable.name
            ));
        }
    }
    // A function the module calls but does not define: a symbol of its own.
    for one in llrm_mir::debuginfo::declarations(module) {
        let procedure = llrm_mir::debuginfo::read_type(module, one.r#type)?;
        let result = match procedure.target {
            Some(result) => sdcc_type(module, result)?.1,
            None => "SV:S".to_owned(),
        };
        out.push_str(&format!("S:G${}$0_0$0({{2}}DF,{result}),C,0,0\n", one.function));
    }
    Some(out)
}
