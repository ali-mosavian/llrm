//! LLVM IR to ca65 text: `llc`'s job over `opt`'s. Clang's front end alone
//! makes the IR; the shared mid-end optimizes it; this backend selects,
//! allocates and prints it. The runtime (`runtime/mos_rt.s`) is linked, not
//! printed.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_mir::target::Machine as _;
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
    let mut program = Program::new(vec![module], machine())?;
    if request.level != Level::O0 {
        let dump = std::env::var_os("LLRM_MIR_STAGES").filter(|one| !one.is_empty()).map(Into::into);
        let mut options = request.level.options();
        // Bytes over speed on an 8-bit target: no unrolling (the stand-in for the
        // target's override-table entry).
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
    let data = data_text(module, &program.layout)?;
    let mut selections = Vec::new();
    for name in &names {
        let mut selection = select_function(module, name, request.cpu)?;
        selection.zero_page = request.platform.zero_page;
        selection.tuning = request.level.tuning();
        selections.push(selection);
    }
    let parts: Vec<(&str, &_)> = names.iter().map(String::as_str).zip(&selections).collect();
    let mut text = module_text(module, &parts)?.concat();
    for (name, label) in &data.1 {
        text = text.replace(name.as_str(), label);
    }
    Ok(equates(&request.platform) + &text + &data.0)
}

fn llrm_6502_prepare(program: &mut Program) {
    program.modules.iter_mut().for_each(crate::prepare::split_phi_edges);
}

/// The cost model the mid-end reads: the pin's `opt` with no target, whose `Basic` TargetTransformInfo prices
/// the loop-idiom, inline and strength-reduction decisions. Without it (`tti() = None`) those passes price by
/// generic rules and the recognizer of CRC loops never runs. Everything else is `Neutral`'s: no register
/// pressure, unit prices.
pub fn machine() -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Mos6502Machine)
}

struct Mos6502Machine;

impl llrm_mir::target::Machine for Mos6502Machine {
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
        Some(Rc::new(llrm_mir::tti::Basic))
    }
}

/// The module's defined variables as ca65 data, and the renames of the symbols ca65 cannot spell (`.crctable`, the
/// table `LoopIdiomRecognize` makes) to the labels that stand for them in the code.
///
/// DEVIATE: integers, arrays, structs and zero fills only; a pointer or float initializer is refused. Reason:
/// nothing the corpus makes needs more. Exit: relocations, with the object writer.
fn data_text(
    module: &llrm_mir::module::Module,
    layout: &llrm_mir::datalayout::DataLayout,
) -> Result<(String, Vec<(String, String)>), String> {
    use llrm_mir::module::GlobalKind;
    let mut out = String::new();
    let mut renames = Vec::new();
    for global in &module.globals {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let Some(initializer) = variable.initializer else { continue };
        let name = global.name.owned().ok_or("an unnamed variable")?;
        let mut bytes = vec![0u8; layout.alloc_size(&module.context.types, variable.ty) as usize];
        write_constant(module, layout, initializer, variable.ty, &mut bytes, 0)?;
        let label = if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            name.clone()
        } else {
            let label: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            renames.push((name.clone(), format!("data{label}")));
            format!("data{label}")
        };
        out.push_str(&format!(".segment \"{}\"\n{label}:\n", if variable.constant { "RODATA" } else { "DATA" }));
        for chunk in bytes.chunks(16) {
            out.push_str(&format!("  .byte {}\n", chunk.iter().map(u8::to_string).collect::<Vec<_>>().join(",")));
        }
        out.push_str(".segment \"CODE\"\n");
    }
    renames.sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
    Ok((out, renames))
}

fn write_constant(
    module: &llrm_mir::module::Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: llrm_mir::ConstantId,
    ty: llrm_mir::TypeId,
    out: &mut [u8],
    at: usize,
) -> Result<(), String> {
    use llrm_mir::{ConstantKind, Type};
    let context = &module.context;
    let types = &context.types;
    match (&context.get(id).kind, types.get(ty)) {
        (ConstantKind::Zero | ConstantKind::Null | ConstantKind::Undef | ConstantKind::Poison, _) => Ok(()),
        (ConstantKind::Int(value), Type::Int(bits)) => {
            let bytes = (*bits as usize).div_ceil(8);
            out[at..at + bytes].copy_from_slice(&value.to_le_bytes()[..bytes]);
            Ok(())
        }
        (ConstantKind::Bytes(data), _) => {
            out[at..at + data.len()].copy_from_slice(data);
            Ok(())
        }
        (ConstantKind::Aggregate(members), Type::Array { element, .. }) => {
            let stride = layout.alloc_size(types, *element) as usize;
            members
                .iter()
                .enumerate()
                .try_for_each(|(index, member)| write_constant(module, layout, *member, *element, out, at + index * stride))
        }
        (ConstantKind::Aggregate(members), _) if types.fields(ty).is_some() => {
            let (_, offsets) = layout.struct_layout(types, ty);
            let fields = types.fields(ty).expect("a struct").to_vec();
            members.iter().enumerate().try_for_each(|(index, member)| {
                write_constant(module, layout, *member, fields[index], out, at + offsets[index] as usize)
            })
        }
        _ => Err("a global initializer that is not an integer, array or struct".into()),
    }
}
