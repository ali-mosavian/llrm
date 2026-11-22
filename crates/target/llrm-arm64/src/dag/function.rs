//! The builder's view of arm64: 64-bit pointers, `i32` and `i64` in registers,
//! `RET_GLUE` for a return.

use std::sync::LazyLock;

use llrm_codegen::backend::isel::Convention;
use llrm_codegen::backend::selectiondag::SDValue;
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::known_bits::KnownBits;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_codegen::backend::selectiondag::{BooleanContents, DagConfig};
use llrm_codegen::backend::selectiondag::{NodeData, SelectionDAG};
use llrm_lir::registers::{Info, RegId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_target::BuilderArch;

use super::call::{Aarch64Calling, RET_GLUE};
use super::globals::Globals;
use super::pipeline::select_block;
use crate::REGISTER_INFO;
use crate::machine::Platform;

static DARWIN: LazyLock<Aarch64Calling> = LazyLock::new(Aarch64Calling::default);
static LINUX: LazyLock<Aarch64Calling> = LazyLock::new(|| Aarch64Calling { linux: true });

pub fn builder_target(platform: Platform) -> BuilderTarget {
    BuilderTarget {
        dag: DagConfig {
            pointer: EVT::I64,
            // `isOffsetFoldingLegal` is false: `performGlobalAddressCombine`
            // picks the offset.
            dso_local_symbols: true,
            // `AArch64TargetLowering::aggressivelyPreferBuildVectorSources`.
            prefers_build_vector_sources: true,
            boolean_contents: BooleanContents::ZeroOrOne,
            // `setBooleanVectorContents(ZeroOrNegativeOneBooleanContent)`.
            vector_boolean_contents: Some(BooleanContents::ZeroOrNegativeOne),
            known_bits_for_target_node: Some(known_bits_for_target_node),
            fma_faster: true,
            vectors: crate::machine::vectors_enabled(),
            ..DagConfig::default()
        },
        shift_amount: EVT::I64,
        legal_ints: vec![32, 64],
        legal_vectors: llrm_sdag::lowering::TargetLoweringInfo::aarch64().legal_vectors().to_vec(),
        vector_registers: llrm_sdag::lowering::TargetLoweringInfo::aarch64().vector_registers(),
        soft_float: false,
        stack_align: Some(16),
        max_alloca_align: Some(16),
        // The inline expansion has no `q` type (SD13): the calling hook makes
        // every constant copy and fill within the pin's limits.
        tuning: llrm_mir::target::Tuning {
            max_stores_per_memset: 0,
            max_stores_per_memset_opt_size: 0,
            max_stores_per_memcpy: 0,
            max_stores_per_memcpy_opt_size: 0,
            max_stores_per_memmove: 0,
            max_stores_per_memmove_opt_size: 0,
            ..llrm_mir::target::Tuning::PIN
        },
        native_atomics: true,
        jump_tables: true,
        // AArch64Subtarget.cpp#L345: 10 (4 at minsize, not modelled).
        min_jump_table_entries: 10,
        target_nodes: Some(TargetNodes {
            call: ISD::Target(0),
            ret: ISD::Target(RET_GLUE),
            stack_pointer: "sp",
            big_endian: false,
            calling: Some(if platform == Platform::Linux { &*LINUX } else { &*DARWIN }),
        }),
        legal_f128: true,
        ..BuilderTarget::for_pointer_bits(64)
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
    /// The ids of the blocks each of the function's jump tables goes to.
    pub tables: Vec<Vec<u32>>,
    /// The blocks whose address is taken, each with the symbol that names it.
    pub address_taken: Vec<(u32, String)>,
    /// The frame object each source variable lives in (`llvm.dbg.declare` of a
    /// static `alloca`), by the variable's metadata node.
    pub declared: Vec<(llrm_mir::MetadataId, u32)>,
    /// The value each `llvm.dbg.value` of this block names, as the selected
    /// nodes left it (`transferDbgValues`), in source order.
    pub dbg_values: Vec<(llrm_mir::MetadataId, Option<SDValue>)>,
    /// The probabilities the builder kept for the edges of merged conditions.
    pub edge_probs: Vec<(u32, u32, u32)>,
}

/// `PHI result, [incoming, from this block]` of the successor `to`: registers
/// by the builder's number.
#[derive(Clone, Copy, Debug)]
pub struct PhiCopy {
    pub to: u32,
    pub dst: u32,
    pub src: u32,
    /// The phi is a float or a vector: its registers are of the float class.
    pub float: bool,
    /// The phi holds 128 bits (a `q` vector): the allocator spills and copies
    /// the register whole.
    pub quad: bool,
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
    let function_id = id.0;
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
    let tti = crate::tti::AArch64::of_cpu(platform.cpu())
        .map(|one| std::rc::Rc::new(one) as std::rc::Rc<dyn llrm_mir::tti::Tti>);
    // `isIntDivCheap` keeps a divide by a constant a divide: the pin answers
    // `minsize` only
    // ([L33827](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ISelLowering.cpp#L33827)).
    // DEVIATE SD535 (`crate::trial`): the compile that keeps the divides at
    // `-Os` (`optsize`) is the trial's, kept where the expansion's bytes are
    // not repaid.
    let flagged = |names: &[&str]| {
        function
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    llrm_mir::opcode::Attribute::Flag(name) if names.contains(&name.as_str())
                ),
            )
    };
    let opt_for_size = flagged(&["optsize", "minsize"]);
    let divide_cheap = flagged(&["minsize"]) || (flagged(&["optsize"]) && crate::trial::divides_kept());
    let mut target = builder_target(platform).with_int_div_cheap(divide_cheap);
    // `AArch64TargetLowering::BuildSDIVPow2`: where a divide is cheap, a divide
    // by a power of two stays one.
    target.dag.sdiv_pow2_stays = divide_cheap;
    let target = target
        .with_tti(tti)
        .with_tail_calls(llrm_support::env_var("LLRM_TAIL_CALLS").is_none_or(|value| value != "0"))
        .with_zext_free(is_zext_free);
    let mut info = FunctionLoweringInfo::new(function);
    let mut frame = static_frame(module, function, layout, &mut info)?;
    let mut selected = Vec::new();
    let declared: Vec<(llrm_mir::MetadataId, u32)> = function
        .debug_records()
        .iter()
        .filter_map(|record| {
            let llrm_mir::DebugWhat::Declare(llrm_mir::Operand::Value(address)) = record.what else { return None };
            Some((record.variable, u32::try_from(*info.static_allocas.get(&address)?).ok()?))
        })
        .collect();
    // Each block is built and selected in turn, as the pin does, so a block
    // reads what the blocks before it knew of a register. The blocks a
    // lowering makes (the case blocks of a split `br (or a, b)`) follow the
    // block that made them in the layout.
    let mut work = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, layout, &target, &Aarch64Arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        let extras = std::mem::take(&mut built.extra);
        let made = !extras.is_empty();
        let parent_updates = built.phi_updates.clone();
        // The branches are read before selection turns them into machine nodes.
        let exits = (branch_targets(&built.dag, &info.jump_tables), ends_in_jump(&built.dag));
        let units = select_block(&mut built.dag, &globals, &mut info, opt_for_size, platform == Platform::Linux)
            .map_err(|why| format!("{why:?}"))?;
        work.push((block.0, made, built, units, exits));
        for (id, mut extra) in extras {
            // `FinishBasicBlock`: a block the lowering made is a predecessor
            // of the successors too, so each takes the phi values the block
            // that made it does.
            extra.phi_updates = parent_updates.clone();
            let exits = (branch_targets(&extra.dag, &info.jump_tables), ends_in_jump(&extra.dag));
            let units = select_block(&mut extra.dag, &globals, &mut info, opt_for_size, platform == Platform::Linux)
                .map_err(|why| format!("{why:?}"))?;
            work.push((id.0, true, extra, units, exits));
        }
    }
    let order: Vec<u32> = work.iter().map(|(id, ..)| *id).collect();
    for (at, (id, made, built, units, (targets, jumps))) in work.into_iter().enumerate() {
        let successors = if made {
            let mut successors = targets;
            if !jumps {
                successors.extend(order.get(at + 1).copied());
            }
            successors
        } else {
            function.successors(llrm_mir::module::BlockId(id)).into_iter().map(|b| b.0).collect()
        };
        let updates = &built.phi_updates;
        let phis = updates
            .iter()
            .enumerate()
            .map(|(at, &(phi, src))| {
                let llrm_mir::module::ValueDef::Instruction(def) = function.value(phi).def else {
                    return Err("a phi that is no instruction".to_owned());
                };
                let to = function.parent(def).ok_or("a phi outside a block")?.0;
                let reg = *info.value_regs.get(&phi).ok_or("a phi with no register")?;
                // A value of several registers (an aggregate, an `i128`) has
                // one update for each part, in order: the k-th is the phi's
                // k-th register.
                let part = updates[..at].iter().filter(|(one, _)| *one == phi).count();
                let dst = if part == 0 { reg } else { info.part_regs(reg, part + 1)[part] };
                let ty = function.value(phi).ty;
                let vector = matches!(
                    module.context.types.get(ty),
                    llrm_mir::types::Type::Vector { .. }
                );
                let float = vector || matches!(module.context.types.get(ty), llrm_mir::types::Type::Float(_));
                let quad = vector && layout.size_bits(&module.context.types, ty) == 128;
                Ok(PhiCopy { to, dst, src, float, quad })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let in_block: llrm_mir::dense::IdSet<llrm_mir::module::InstId> =
            built.dag.inst_order.iter().map(|&(inst, _)| inst).collect();
        // A note with no node to follow (the value is in another block) or no
        // value at all (`undef`, a constant, a piece, an assignment) is kept as
        // a note of nothing: the variable then says nothing, rather than
        // leaving an earlier note standing over code it no longer holds.
        let dbg_values = function
            .debug_records()
            .iter()
            .filter_map(|record| match record.what {
                llrm_mir::DebugWhat::Declare(_) => None,
                llrm_mir::DebugWhat::Value(llrm_mir::Operand::Value(value)) => {
                    if !in_block.contains(&record.before) {
                        return None;
                    }
                    let node = built.values.iter().find(|(one, _)| *one == value).map(|(_, node)| *node);
                    Some((record.variable, node.map(|node| built.dag.resolve(node))))
                }
                _ => (at == 0).then_some((record.variable, None)),
            })
            .collect();
        selected.push(Selected {
            dag: built.dag,
            units,
            live_ins: built.live_ins,
            frame: Vec::new(),
            globals: globals.clone(),
            layout: order.clone(),
            successors,
            phis,
            tables: info.jump_tables.clone(),
            // A block that replaced another answers to both addresses: one
            // symbol.
            address_taken: function
                .address_taken_blocks()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .map(|block| (block.0, super::globals::block_symbol(globals.local_prefix(), function_id, block.0)))
                .collect(),
            declared: declared.clone(),
            dbg_values,
            edge_probs: info.edge_probs.iter().map(|(from, to, p)| (from.0, to.0, *p)).collect(),
        });
    }
    frame.extend(info.objects.iter().map(|&(size, align)| (u64::from(size), u64::from(align))));
    for block in &mut selected {
        block.frame.clone_from(&frame);
    }
    Ok(selected)
}

/// The blocks the branches of a built block name, in the order they execute.
/// The chain from the root passes the nodes between the branches (the copies
/// that export a value to another block). A table jump goes to each entry of
/// its table.
fn branch_targets(
    dag: &SelectionDAG,
    tables: &[Vec<u32>],
) -> Vec<u32> {
    let mut targets = Vec::new();
    let mut at = dag.get_root();
    loop {
        let dest = match dag.opcode(at) {
            ISD::Br => Some(1),
            ISD::BrCond => Some(2),
            _ => None,
        };
        if let Some(dest) = dest
            && let NodeData::Block(block) = dag.node(dag.operand(at, dest)).data
        {
            targets.push(block);
        }
        if dag.opcode(at) == ISD::BrJT
            && let NodeData::JumpTable(table) = dag.node(dag.operand(at, 1)).data
        {
            for block in tables.get(table as usize).into_iter().flatten().rev() {
                if !targets.contains(block) {
                    targets.push(*block);
                }
            }
        }
        match dag.node(at).ops.first() {
            Some(&chain) if dag.value_type(chain) == EVT::OTHER => at = chain,
            _ => break,
        }
    }
    targets.reverse();
    targets
}

/// Whether the last thing a built block does is an unconditional branch or a
/// table jump (else it falls through).
fn ends_in_jump(dag: &SelectionDAG) -> bool {
    matches!(dag.opcode(dag.get_root()), ISD::Br | ISD::BrJT)
}

/// The (size, alignment) of each frame object of the static allocas, by the
/// index `FunctionLoweringInfo` gave it: the type's size times a constant
/// count, allocas whose lifetimes never overlap sharing one object
/// (`StackColoring`, `llrm_codegen::backend::stackcoloring`).
fn static_frame(
    module: &Module,
    function: &llrm_mir::module::Function,
    layout: &DataLayout,
    info: &mut FunctionLoweringInfo,
) -> Result<Vec<(u64, u64)>, String> {
    use llrm_mir::context::ConstantKind;
    use llrm_mir::module::{Operand, ValueDef};
    use llrm_mir::opcode::Opcode;

    let mut sizes = Vec::new();
    for (value, _) in info.static_allocas.iter() {
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
        let align = layout.pref_align(types, allocated).max(u64::from(align.unwrap_or(1)));
        sizes.push((value, size as i64, align as i64));
    }
    sizes.sort_by_key(|(value, _, _)| info.static_allocas[value]);
    let shared = llrm_codegen::backend::stackcoloring::share_frame(module, function, info, &sizes);
    Ok(shared.into_iter().map(|(size, align)| (size as u64, align as u64)).collect())
}

/// `AArch64TargetLowering::isZExtFree(SDValue, EVT)`: a 32 to 64 bit extension,
/// or a load of at most 32 bits, which the load instructions zero extend.
fn is_zext_free(
    dag: &SelectionDAG,
    value: SDValue,
    to: EVT,
) -> bool {
    let from = dag.value_type(value);
    if !from.is_integer() || !to.is_integer() {
        return false;
    }
    (from.size_in_bits() == 32 && to.size_in_bits() == 64)
        || (dag.opcode(value) == ISD::Load && from.size_in_bits() <= 32)
}

/// `AArch64TargetLowering::computeKnownBitsForTargetNode`, `AArch64ISD::CSEL`:
/// what both arms share.
fn known_bits_for_target_node(
    dag: &SelectionDAG,
    value: SDValue,
    operand: &dyn Fn(usize) -> KnownBits,
) -> Option<KnownBits> {
    (dag.opcode(value) == ISD::Target(llrm_dagisel::generated::aarch64::ids::isd::CSEL) && value.res == 0)
        .then(|| operand(0).intersect(operand(1)))
}
