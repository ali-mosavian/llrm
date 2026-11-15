//! The builder's view of wasm: the pointer is `i32` (`i64` for wasm64), `i32`
//! and `i64` are the legal integers, and a return is `WebAssemblyISD::RETURN`.

use std::collections::BTreeMap;

use llrm_codegen::backend::isel::Convention;
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::live_out;
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
use crate::features::Features;
use crate::mir::ValType;
use crate::{REGISTER_INFO, Width};

pub(crate) fn pointer(width: Width) -> EVT {
    match width {
        Width::W32 => EVT::I32,
        Width::W64 => EVT::I64,
    }
}

/// The calling `TargetNodes` borrows for `'static`, one per width.
fn calling(
    width: Width,
    features: Features,
) -> &'static WasmCalling {
    static W32: WasmCalling = WasmCalling { pointer: EVT::I32, bulk_memory_opt: true };
    static W32_NO_BULK: WasmCalling = WasmCalling { pointer: EVT::I32, bulk_memory_opt: false };
    static W64: WasmCalling = WasmCalling { pointer: EVT::I64, bulk_memory_opt: true };
    static W64_NO_BULK: WasmCalling = WasmCalling { pointer: EVT::I64, bulk_memory_opt: false };
    match (width, features.has("bulk-memory-opt")) {
        (Width::W32, true) => &W32,
        (Width::W32, false) => &W32_NO_BULK,
        (Width::W64, true) => &W64,
        (Width::W64, false) => &W64_NO_BULK,
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

pub fn builder_target(
    width: Width,
    features: Features,
) -> BuilderTarget {
    BuilderTarget {
        dag: DagConfig {
            pointer: pointer(width),
            offset_folding: true,
            global_addresses_constant: true,
            // `shouldAssumeDSOLocal` on wasm is the IR's: local linkage or a
            // visibility other than default (clang marks every definition
            // `hidden`; a declaration of default visibility is preemptible).
            dso_local_symbols: false,
            boolean_contents: BooleanContents::ZeroOrOne,
            // The pin keeps `i32.div_s` for `sdiv x, 4` at every
            // level: `isIntDivCheap`.
            int_div_cheap: true,
            sdiv_pow2_stays: true,
            add_to_or_disjoint: true,
            // `WebAssemblySubtarget::useAA`
            use_aa: true,
            fp_stack: false,
            ..DagConfig::default()
        },
        tuning: crate::machine::TUNING,
        shift_amount: pointer(width),
        shift_amount_of: Some(shift_amount_of),
        legal_ints: vec![32, 64],
        soft_float: false,
        stack_align: Some(16),
        jump_tables: true,
        // WebAssemblyTargetLowering: setMinimumJumpTableEntries(2); the
        // pin's -O0 skips the table search beyond one whole-range table, bit
        // tests and peeling.
        min_jump_table_entries: 2,
        switch_optimize: false,
        // `WebAssemblyTargetMachine`: `TrapUnreachable`.
        trap_unreachable: true,
        target_nodes: Some(TargetNodes {
            call: ISD::Target(CALL),
            ret: ISD::Target(isd::RETURN),
            stack_pointer: if width == Width::W32 { "sp32" } else { "sp64" },
            big_endian: false,
            calling: Some(calling(width, features)),
        }),
        ..BuilderTarget::for_pointer_bits(64)
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
    /// The unwind destination of the `invoke` ending this block (an EH
    /// pad's block id), when the model keeps unwinding edges.
    pub unwind: Option<u32>,
    /// The block starts with a `landingpad`: an EH pad.
    pub landing: bool,
    /// The machine blocks' ids in layout order: each IR block, then the blocks
    /// its lowering made.
    pub layout: Vec<u32>,
    /// What the phis of the successors take from this block's edges (an IR
    /// block's; the extra ones have none).
    pub phis: Vec<PhiCopy>,
    /// The probability of each edge to a successor block, as a numerator over
    /// 2^31 (`BranchProbabilityInfo`); the extra blocks have none.
    pub probs: Vec<(u32, u32)>,
    /// The (size, alignment) of each stack object, by frame index: the
    /// function's allocas, then what the builder made.
    pub frame: Vec<(u64, u64)>,
    /// `MFI.hasVarSizedObjects()`, the same on every block.
    pub has_var_sized: bool,
    /// `-g`: the metadata id of each variable declared by a `#dbg_declare` of
    /// a static alloca, and the frame index of that alloca. The same on every
    /// block.
    pub declared: Vec<(u32, i32)>,
    /// `-g` at `-O1` and up: the metadata id of each variable that one
    /// `#dbg_value` says is an argument, and the argument's number.
    pub argument_variables: Vec<(u32, u32)>,
    /// StackColoring: the frame object each of the function's allocas moves to
    /// once selection is done (`frame`'s first entries are the allocas').
    pub stack_colors: Vec<i32>,
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
    unwinding: bool,
    optimize: bool,
    facts: &super::ModuleFacts,
) -> Result<Vec<Selected>, String> {
    let layout = DataLayout::parse(module.datalayout.as_deref().ok_or("a module with no datalayout")?)?;
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    let features = Features::of(function).map_err(|why| format!("@{name}: {why}"))?;
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
    // `getJumpConditionMergingParams`: the cost model splits every merged
    // condition into branches.
    let mut target = builder_target(width, features).with_tti(Some(std::rc::Rc::new(crate::machine::Wasm)));
    // `getOptLevel() != None`: clusters, jump tables, bit tests and peeling.
    target.switch_optimize = optimize;
    let arch = WasmArch { width };
    let cpu = if width == Width::W32 { Cpu::Wasm32 } else { Cpu::Wasm64 };
    let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
        llrm_analysis::bpi::Module {
            context: &module.context,
            declarations: &module.globals,
            metadata: &module.metadata,
            layout: &layout,
        },
        function,
    );
    let mut info = FunctionLoweringInfo::new(function);
    info.with_module_tables(facts.tables(module, target.dag.dso_local_symbols));
    let (statics, stack_colors) = static_frame(module, function, &layout, &info)?;
    let alignments: Vec<u64> = statics.iter().map(|&(_, align)| align).collect();
    let alias = super::alias::IrAlias::new(module, function, &layout);
    let mut selected = Vec::new();
    let mut machine_layout = Vec::new();
    let mut visited = llrm_support::hash::HashSet::default();
    // `SelectAllBasicBlocks`: reverse post-order, so a block is built after
    // the blocks that dominate it and reads what their phis know. A block no
    // path reaches follows, in layout order.
    let at: std::collections::HashMap<_, _> =
        function.layout().iter().enumerate().map(|(at, &block)| (block, at)).collect();
    let mut order = llrm_mir::blockorder::reverse_post_order_traversal(function);
    let reached: llrm_support::hash::HashSet<_> = order.iter().copied().collect();
    order.extend(function.layout().iter().copied().filter(|block| !reached.contains(block)));
    for block in order {
        if optimize {
            // `SelectAllBasicBlocks`: a phi knows what its incoming values
            // do once all its predecessors are selected.
            let ready = function.predecessors(block).iter().all(|pred| visited.contains(pred));
            let phis: Vec<_> = function
                .iter_block(block)
                .take_while(|&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi)
                .collect();
            for phi in phis {
                if ready {
                    let bits = |bits: u32| (bits <= 64 && bits != 0).then(|| if bits <= 32 { 32 } else { 64 });
                    live_out::compute_phi_live_out_reg_info(module, function, phi, &bits, &mut info);
                } else if let Some(reg) = function.instruction(phi).result.and_then(|r| info.value_regs.get(&r)) {
                    info.live_out.remove(reg);
                }
            }
            visited.insert(block);
        }
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        let extras = std::mem::take(&mut built.extra);
        // A phi of two registers (an `i128`, an `f128`) is updated part by part: the k-th source goes to the k-th register of the phi
        let mut seen: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
        let mut phis = Vec::new();
        for &(phi, src) in &built.phi_updates {
            let llrm_mir::module::ValueDef::Instruction(def) = function.value(phi).def else {
                return Err("a phi that is no instruction".to_owned());
            };
            let to = function.parent(def).ok_or("a phi outside a block")?.0;
            let base = *info.value_regs.get(&phi).ok_or("a phi with no register")?;
            let part = seen.entry(phi).or_insert(0);
            let dst = *info.part_regs(base, *part + 1).last().expect("a register");
            *part += 1;
            let phi_type = function.value(phi).ty;
            let ty = if super::is_wide(module.context.types.get(phi_type)) {
                ValType::I64
            } else {
                super::value_type(module, phi_type, width)?
            };
            phis.push(PhiCopy { to, dst, src, ty });
        }
        let probs = llrm_mir::loopinfo::children(function, block)
            .enumerate()
            .map(|(at, to)| (to.0, bpi.get_edge_probability(function, block, at).get_numerator()))
            .collect::<Vec<_>>();
        let mut group = vec![(block, block, built, phis)];
        group.extend(extras.into_iter().map(|(id, one)| (id, block, one, Vec::new())));
        for (n, (id, origin, mut one, phis)) in group.into_iter().enumerate() {
            // The objects a call made while the block was built (a variadic
            // call's buffer) are the frame's too:
            // `computeKnownBitsForFrameIndex` reads each object's alignment.
            one.dag.frame_alignments.clone_from(&alignments);
            one.dag.frame_alignments.extend(info.objects.iter().map(|&(_, align)| u64::from(align)));
            let Scheduled { units, mem_refs, accesses } =
                select_block(&mut one.dag, cpu, features, &mut info, &alias, optimize)
                    .map_err(|why| format!("{why:?}"))?;
            machine_layout.push((at[&block], id.0));
            // `addSuccessorWithProb`: the edges the builder made of a switch
            // are the machine blocks' own; the IR's apply to a block it left
            // alone.
            let made: Vec<(u32, u32)> = info
                .edge_probs
                .iter()
                .filter(|(from, ..)| *from == id)
                .map(|&(_, to, probability)| (to.0, probability))
                .collect();
            selected.push(Selected {
                id: id.0,
                origin: origin.0,
                unwind: if unwinding { unwind_of(function, origin) } else { None },
                landing: unwinding && id == origin && starts_with_landing_pad(function, origin),
                phis,
                probs: if !made.is_empty() {
                    made
                } else if n == 0 {
                    probs.clone()
                } else {
                    Vec::new()
                },
                dag: one.dag,
                units,
                mem_refs,
                accesses,
                layout: Vec::new(),
                frame: Vec::new(),
                has_var_sized: false,
                declared: Vec::new(),
                argument_variables: Vec::new(),
                stack_colors: Vec::new(),
            });
        }
    }
    // Selected in reverse post-order, emitted in layout order; the sort is
    // stable, so the blocks a switch was split into follow their block.
    machine_layout.sort_by_key(|&(at, _)| at);
    selected.sort_by_key(|block| at[&llrm_mir::module::BlockId(block.origin)]);
    let machine_layout: Vec<u32> = machine_layout.into_iter().map(|(_, id)| id).collect();
    let mut frame = statics;
    frame.extend(info.objects.iter().map(|&(size, align)| (u64::from(size), u64::from(align))));
    let declared: Vec<(u32, i32)> = function
        .debug_records()
        .iter()
        .filter_map(|record| {
            let llrm_mir::DebugWhat::Declare(llrm_mir::Operand::Value(address)) = record.what else { return None };
            let variable = llrm_mir::debuginfo::read_variable(module, record.variable)?;
            let index = *info.static_allocas.get(&address)?;
            (variable.scope == name).then_some((record.variable.0, index))
        })
        .collect();
    // A variable said once, to an argument: its home is the argument's local
    // for as long as the code leaves it there.
    let mut argument_variables: Vec<(u32, u32)> = Vec::new();
    let mut said: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for record in function.debug_records() {
        *said.entry(record.variable.0).or_default() += 1;
        let llrm_mir::DebugWhat::Value(llrm_mir::Operand::Value(value)) = record.what else { continue };
        let llrm_mir::module::ValueDef::Argument(number) = function.value(value).def else { continue };
        if llrm_mir::debuginfo::read_variable(module, record.variable).is_some_and(|one| one.scope == name) {
            argument_variables.push((record.variable.0, number));
        }
    }
    argument_variables.retain(|(variable, _)| said[variable] == 1);
    for block in &mut selected {
        block.layout.clone_from(&machine_layout);
        block.frame.clone_from(&frame);
        block.has_var_sized = info.has_var_sized_objects;
        block.declared.clone_from(&declared);
        block.argument_variables.clone_from(&argument_variables);
        block.stack_colors.clone_from(&stack_colors);
    }
    Ok(selected)
}

/// The pad an `invoke` ending `block` unwinds to.
fn unwind_of(
    function: &llrm_mir::module::Function,
    block: llrm_mir::module::BlockId,
) -> Option<u32> {
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::Opcode;

    let last = function.iter_block(block).last()?;
    let inst = function.instruction(last);
    match (&inst.opcode, inst.operands.as_slice()) {
        (Opcode::Invoke(_), [.., Operand::Block(_), Operand::Block(pad), _]) => Some(pad.0),
        _ => None,
    }
}

/// Whether the first instruction of `block` after its phis is a `landingpad`.
fn starts_with_landing_pad(
    function: &llrm_mir::module::Function,
    block: llrm_mir::module::BlockId,
) -> bool {
    use llrm_mir::opcode::Opcode;

    function
        .iter_block(block)
        .map(|inst| &function.instruction(inst).opcode)
        .find(|opcode| !matches!(opcode, Opcode::Phi))
        .is_some_and(|opcode| matches!(opcode, Opcode::LandingPad { .. }))
}

/// The (size, alignment) of each static alloca, by the frame index
/// `FunctionLoweringInfo` gave it: the type's size times a constant count, at
/// its alignment (`FunctionLoweringInfo::set`); and what StackColoring makes of
/// them.
fn static_frame(
    module: &Module,
    function: &llrm_mir::module::Function,
    layout: &DataLayout,
    info: &FunctionLoweringInfo,
) -> Result<(Vec<(u64, u64)>, Vec<i32>), String> {
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
        // `AI->getAlign()`; an alloca that states none has the type's preferred
        // one (the IR reader's default).
        let align = align.map_or_else(|| layout.pref_align(types, allocated), u64::from);
        sizes.push((value, size as i64, align as i64));
    }
    sizes.sort_by_key(|(value, _, _)| info.static_allocas[value]);
    let colors = llrm_codegen::backend::stackcoloring::color_frame(module, function, &sizes);
    Ok((sizes.iter().map(|&(_, size, align)| (size as u64, align as u64)).collect(), colors.colors))
}

#[cfg(test)]
mod stack_natural_alignment_tests {
    use super::*;

    /// Every target's datalayout names the stack alignment its frames keep, or the passes that raise an alloca to its type's preferred
    /// alignment have no bound. wasm declares S128 (the pin's string) and keeps 16.
    #[test]
    fn the_datalayout_states_the_stack_alignment_the_frames_keep() {
        // The pin clang's strings (d1106deb71cc), not ours read back.
        for (width, pin) in [
            (Width::W32, "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"),
            (Width::W64, "e-m:e-p:64:64-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"),
        ] {
            llrm_target::testing::assert_stack_facts(
                "wasm",
                &crate::layout(width).datalayout,
                WasmArch { width }.stack_alignment() as u64,
                Some(pin),
            );
        }
    }
}
