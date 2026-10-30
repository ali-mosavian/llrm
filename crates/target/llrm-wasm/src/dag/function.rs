//! The builder's view of wasm: the pointer is `i32` (`i64` for wasm64), `i32`
//! and `i64` are the legal integers, and a return is `WebAssemblyISD::RETURN`.

use std::collections::BTreeMap;

use llrm_codegen::backend::isel::Convention;
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_codegen::backend::selectiondag::{BooleanContents, DagConfig, MemInfo, SelectionDAG};
use llrm_lir::registers::{Info, RegId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_target::BuilderArch;

use super::call::{CALL, WasmCalling};
use super::pipeline::{Scheduled, select_block};
use crate::dagisel::generated::ids::isd;
use crate::dagisel::hooks::Cpu;
use crate::mir::ValType;
use crate::{REGISTER_INFO, Width};

pub(crate) fn pointer(width: Width) -> EVT {
    match width {
        Width::W32 => EVT::I32,
        Width::W64 => EVT::I64,
    }
}

/// The calling `TargetNodes` borrows for `'static`, one per width.
fn calling(width: Width) -> &'static WasmCalling {
    static W32: WasmCalling = WasmCalling { pointer: EVT::I32 };
    static W64: WasmCalling = WasmCalling { pointer: EVT::I64 };
    match width {
        Width::W32 => &W32,
        Width::W64 => &W64,
    }
}

/// `WebAssemblyTargetLowering::getScalarShiftAmountTy`: the type's width
/// rounded up to a power of two, at least 8 bits, and 32 bits where the shift
/// is a libcall.
fn shift_amount_of(vt: EVT) -> EVT {
    let mut bits = vt.size_in_bits().next_power_of_two();
    if bits > 1 && bits < 8 {
        bits = 8;
    }
    if bits > 64 {
        bits = 32;
    }
    EVT::integer(bits)
}

pub fn builder_target(width: Width) -> BuilderTarget {
    BuilderTarget {
        dag: DagConfig {
            pointer: pointer(width),
            offset_folding: true,
            boolean_contents: BooleanContents::ZeroOrOne,
            ..DagConfig::default()
        },
        shift_amount: pointer(width),
        shift_amount_of: Some(shift_amount_of),
        legal_ints: vec![32, 64],
        stack_align: Some(16),
        jump_tables: true,
        // WebAssemblyTargetLowering: setMinimumJumpTableEntries(2); the
        // pin's -O0 skips the table search beyond one whole-range table, bit
        // tests and peeling.
        min_jump_table_entries: 2,
        switch_optimize: false,
        target_nodes: Some(TargetNodes {
            call: ISD::Target(CALL),
            ret: ISD::Target(isd::RETURN),
            stack_pointer: if width == Width::W32 { "sp32" } else { "sp64" },
            big_endian: false,
            calling: Some(calling(width)),
        }),
        ..BuilderTarget::m64()
    }
}

/// What the builder asks of wasm.
pub struct WasmArch {
    pub width: Width,
}

impl BuilderArch for WasmArch {
    fn registers(&self) -> &'static Info {
        &REGISTER_INFO
    }

    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        (0..=u8::MAX)
            .map(RegId::from_raw)
            .find(|&id| REGISTER_INFO.name(id) == Some(name))
            .unwrap_or_else(|| panic!("no wasm register {name}"))
    }

    fn stack_slot_bytes(&self) -> i64 {
        8
    }

    fn stack_alignment(&self) -> i64 {
        16
    }

    fn roles(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::spaces::Spaces::default()
    }

    fn full(&self) -> Option<&dyn llrm_target::Target> {
        None
    }
}

/// One block, selected and scheduled.
pub struct Selected {
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
    /// The memory accesses of each machine node, as load and store node ids.
    pub mem_refs: BTreeMap<u32, Vec<u32>>,
    /// The load and store nodes as the builder made them, by node id.
    pub accesses: BTreeMap<u32, MemInfo>,
    /// This block's id: an extra one (a switch lowering made it) is numbered
    /// after the function's.
    pub id: u32,
    /// The IR block whose lowering made this one: itself, or the `switch`
    /// block.
    pub origin: u32,
    /// The machine blocks' ids in layout order: each IR block, then the blocks
    /// its lowering made.
    pub layout: Vec<u32>,
    /// What the phis of the successors take from this block's edges (an IR
    /// block's; the extra ones have none).
    pub phis: Vec<PhiCopy>,
    /// The (size, alignment) of each stack object, by frame index: the
    /// function's allocas, then what the builder made.
    pub frame: Vec<(u64, u64)>,
}

/// `PHI result, [incoming, from this block]` of the successor `to`: registers
/// by the builder's number.
#[derive(Clone, Copy, Debug)]
pub struct PhiCopy {
    pub to: u32,
    pub dst: u32,
    pub src: u32,
    pub ty: ValType,
}

/// Function `name` of `module`, block by block in layout order.
pub fn select_function(
    module: &Module,
    name: &str,
    width: Width,
) -> Result<Vec<Selected>, String> {
    let layout = DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    // The target's `TargetCalling` places every value; the builder reads none
    // of this.
    let convention = Convention {
        parameters: Vec::new(),
        returns: Vec::new(),
        float_result: None,
        popped: 0,
        saved: Vec::new(),
        stack_arguments: 0,
        first: 0,
    };
    let target = builder_target(width);
    let arch = WasmArch { width };
    let cpu = if width == Width::W32 { Cpu::Wasm32 } else { Cpu::Wasm64 };
    let mut info = FunctionLoweringInfo::new(function);
    let mut selected = Vec::new();
    let mut machine_layout = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        let extras = std::mem::take(&mut built.extra);
        let phis = built
            .phi_updates
            .iter()
            .map(|&(phi, src)| {
                let llrm_mir::module::ValueDef::Instruction(def) = function.value(phi).def else {
                    return Err("a phi that is no instruction".to_owned());
                };
                let to = function.parent(def).ok_or("a phi outside a block")?.0;
                let dst = *info.value_regs.get(&phi).ok_or("a phi with no register")?;
                Ok(PhiCopy { to, dst, src, ty: super::value_type(module, function.value(phi).ty, width)? })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut group = vec![(block, block, built, phis)];
        group.extend(extras.into_iter().map(|(id, one)| (id, block, one, Vec::new())));
        for (id, origin, mut one, phis) in group {
            let Scheduled { units, mem_refs, accesses } =
                select_block(&mut one.dag, cpu, &info.jump_tables).map_err(|why| format!("{why:?}"))?;
            machine_layout.push(id.0);
            selected.push(Selected {
                id: id.0,
                origin: origin.0,
                phis,
                dag: one.dag,
                units,
                mem_refs,
                accesses,
                layout: Vec::new(),
                frame: Vec::new(),
            });
        }
    }
    let mut frame = static_frame(module, function, &layout, &info)?;
    frame.extend(info.objects.iter().map(|&(size, align)| (u64::from(size), u64::from(align))));
    for block in &mut selected {
        block.layout.clone_from(&machine_layout);
        block.frame.clone_from(&frame);
    }
    Ok(selected)
}

/// The (size, alignment) of each static alloca, by the frame index
/// `FunctionLoweringInfo` gave it: the type's size times a constant count, at
/// its alignment (`FunctionLoweringInfo::set`).
fn static_frame(
    module: &Module,
    function: &llrm_mir::module::Function,
    layout: &DataLayout,
    info: &FunctionLoweringInfo,
) -> Result<Vec<(u64, u64)>, String> {
    use llrm_mir::context::ConstantKind;
    use llrm_mir::module::{Operand, ValueDef};
    use llrm_mir::opcode::Opcode;

    let mut objects = vec![(0, 1); info.static_allocas.iter().map(|(_, &index)| index as usize + 1).max().unwrap_or(0)];
    for (value, &index) in info.static_allocas.iter() {
        let ValueDef::Instruction(def) = function.value(value).def else {
            return Err("an alloca that is no instruction".into());
        };
        let instruction = function.instruction(def);
        let Opcode::Alloca { allocated, align, .. } = instruction.opcode else {
            return Err("an alloca that is none".into());
        };
        let count = match instruction.operands.first() {
            None => 1,
            Some(Operand::Constant(id)) => match module.context.get(*id).kind {
                ConstantKind::Int(count) => u64::try_from(count).map_err(|_| "a negative alloca count")?,
                _ => return Err("an alloca count that is no number".into()),
            },
            Some(_) => return Err("an alloca count that is no constant".into()),
        };
        let types = &module.context.types;
        let size = layout.alloc_size(types, allocated) * count;
        // `AI->getAlign()`; an alloca that states none has the type's preferred
        // one (the IR reader's default).
        let align = align.map_or_else(|| layout.pref_align(types, allocated), u64::from);
        objects[index as usize] = (size, align);
    }
    Ok(objects)
}
