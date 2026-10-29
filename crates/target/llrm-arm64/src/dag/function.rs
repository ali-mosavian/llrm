//! The builder's view of arm64: 64-bit pointers, `i32` and `i64` in registers,
//! `RET_GLUE` for a return.

use std::sync::LazyLock;

use llrm_codegen::backend::isel::Convention;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_codegen::backend::selectiondag::{BooleanContents, DagConfig};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_lir::registers::{Info, RegId};
use llrm_target::BuilderArch;

use super::call::{Aarch64Calling, RET_GLUE};
use super::globals::Globals;
use super::pipeline::select_block;
use crate::machine::Platform;
use crate::REGISTER_INFO;

static CALLING: LazyLock<Aarch64Calling> = LazyLock::new(Aarch64Calling::default);

pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        dag: DagConfig {
            pointer: EVT::I64,
            offset_folding: true,
            // DEVIATE: `isOffsetFoldingLegal` is false for a GOT symbol; here its offset
            // folds too and `LowerGlobalAddress` splits it again. Exit: a per-symbol hook.
            dso_local_symbols: true,
            boolean_contents: BooleanContents::ZeroOrOne,
            ..DagConfig::default()
        },
        shift_amount: EVT::I64,
        legal_ints: vec![32, 64],
        stack_align: Some(16),
        jump_tables: false,
        target_nodes: Some(TargetNodes {
            call: ISD::Target(0),
            ret: ISD::Target(RET_GLUE),
            stack_pointer: "sp",
            big_endian: false,
            calling: Some(&*CALLING),
        }),
        ..BuilderTarget::m64()
    }
}

/// What the builder asks of arm64.
pub struct Aarch64Arch;

impl BuilderArch for Aarch64Arch {
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
            .unwrap_or_else(|| panic!("no arm64 register {name}"))
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
    /// The argument registers copied to virtual registers on entry.
    pub live_ins: Vec<(RegId, u32)>,
    /// The (size, alignment) of each frame object, by frame index: the
    /// function's allocas, then what the builder made.
    pub frame: Vec<(u64, u64)>,
    pub globals: Globals,
    /// The function's block ids in layout order: a branch names a block by id.
    pub layout: Vec<u32>,
    /// The successors of this block (ids), in the terminator's order.
    pub successors: Vec<u32>,
    /// What the phis of the function take from this block's edges.
    pub phis: Vec<PhiCopy>,
}

/// `PHI result, [incoming, from this block]` of the successor `to`: registers by the
/// builder's number.
#[derive(Clone, Copy, Debug)]
pub struct PhiCopy {
    pub to: u32,
    pub dst: u32,
    pub src: u32,
    /// The phi is a float: its registers are of the float class.
    pub float: bool,
}

/// Function `name` of `module`, block by block in layout order.
pub fn select_function(
    module: &Module,
    name: &str,
) -> Result<Vec<Selected>, String> {
    let layout = DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    select_function_in(module, name, &layout, Platform::Darwin)
}

/// [`select_function`] under the data layout the program was compiled with.
pub fn select_function_in(
    module: &Module,
    name: &str,
    layout: &DataLayout,
    platform: Platform,
) -> Result<Vec<Selected>, String> {
    let globals = Globals::of(module, layout, platform);
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    // The target's `TargetCalling` places every value; the builder reads none of this.
    let convention = Convention {
        parameters: Vec::new(),
        returns: Vec::new(),
        float_result: None,
        popped: 0,
        saved: Vec::new(),
        stack_arguments: 0,
        first: 0,
    };
    let target = builder_target();
    let mut info = FunctionLoweringInfo::new(function);
    let mut frame = static_frame(module, function, layout, &info)?;
    let mut selected = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, layout, &target, &Aarch64Arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        if !built.extra.is_empty() {
            return Err("a switch lowering made blocks".to_owned());
        }
        let units = select_block(&mut built.dag, &globals).map_err(|why| format!("{why:?}"))?;
        let phis = built
            .phi_updates
            .iter()
            .map(|&(phi, src)| {
                let llrm_mir::module::ValueDef::Instruction(def) = function.value(phi).def else {
                    return Err("a phi that is no instruction".to_owned());
                };
                let to = function.parent(def).ok_or("a phi outside a block")?.0;
                let dst = *info.value_regs.get(&phi).ok_or("a phi with no register")?;
                let float = matches!(module.context.types.get(function.value(phi).ty), llrm_mir::types::Type::Float(_));
                Ok(PhiCopy { to, dst, src, float })
            })
            .collect::<Result<Vec<_>, String>>()?;
        selected.push(Selected {
            dag: built.dag,
            units,
            live_ins: built.live_ins,
            frame: Vec::new(),
            globals: globals.clone(),
            layout: function.layout().iter().map(|b| b.0).collect(),
            successors: function.successors(block).into_iter().map(|b| b.0).collect(),
            phis,
        });
    }
    frame.extend(info.objects.iter().map(|&(size, align)| (u64::from(size), u64::from(align))));
    for block in &mut selected {
        block.frame.clone_from(&frame);
    }
    Ok(selected)
}

/// The (size, alignment) of each static alloca, by the frame index
/// `FunctionLoweringInfo` gave it: the type's size times a constant count.
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
        let ValueDef::Instruction(def) = function.value(value).def else { return Err("an alloca that is no instruction".into()) };
        let instruction = function.instruction(def);
        let Opcode::Alloca { allocated, align, .. } = instruction.opcode else { return Err("an alloca that is none".into()) };
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
        let align = layout.pref_align(types, allocated).max(u64::from(align.unwrap_or(1)));
        objects[index as usize] = (size, align);
    }
    Ok(objects)
}
