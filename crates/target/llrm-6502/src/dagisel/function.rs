//! A function from IR to scheduled machine nodes: the shared builder, then
//! [`super::pipeline`]'s stages, block by block.
//!
//! DEVIATE: the phi copies of a block that branches to several blocks are per
//! edge (an edge block the emitter makes), not placed by a machine-function phi
//! elimination; a `switch` to a block with phis is refused. Reason: no shared
//! machine function. Exit: the shared machine function.
//!
//! DEVIATE: no static allocas or jump tables: the
//! builder makes the frame objects and the C convention passes what the
//! registers do not hold on the stack, so the slice (arithmetic, branches,
//! calls) needs none. Reason: frame objects wait for the 6502 machine function.
//! Exit: M4.

use std::collections::HashMap;

use llrm_codegen::backend::isel;
use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::builder::{BlockDag, BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_mir::context::ConstantKind;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

use super::call::{CALL, MosCalling, RET_GLUE};
use super::hooks::Cpu;
use super::pipeline::select_block;
use crate::target::Mos;

/// The builder's view of the 6502: 16-bit pointers, `i8` and `i16` in
/// registers, a byte shift count, and the `CALL`/`RET_GLUE` nodes of
/// `own_ids.txt`.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        shift_amount: EVT::I8,
        legal_ints: vec![8, 16],
        jump_tables: false,
        target_nodes: Some(TargetNodes {
            call: CALL,
            ret: RET_GLUE,
            stack_pointer: "rs0",
            big_endian: false,
            calling: Some(&MosCalling),
        }),
        ..BuilderTarget::m16()
    }
}

/// One block, selected and scheduled.
pub struct Selected {
    pub block: BlockId,
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
    /// The registers the block reads on arrival, with the virtual register each
    /// is copied to.
    pub live_ins: Vec<(RegId, u32)>,
    /// The phi inputs this block exports, by successor: the register the value
    /// was copied to, and the phi's own. One entry with no successor when
    /// they all go together (a single successor, or a `switch`).
    pub edges: Vec<Edge>,
}

/// The phi copies on the edge to `to` (a parallel copy), or on every edge when
/// `to` is `None`.
pub struct Edge {
    pub to: Option<BlockId>,
    pub copies: Vec<(u32, u32)>,
}

/// A selected function: its blocks and the sizes of its local frame objects, by
/// frame index.
pub struct Selection {
    pub blocks: Vec<Selected>,
    pub locals: Vec<i64>,
}

/// Function `name` of `module`, block by block in layout order.
///
/// DEVIATE: static allocas share no frame object (no stack coloring) and none
/// is aligned: the 6502 has no alignment. Reason: nothing here needs the
/// saving. Exit: with the shared frame code.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Selection, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module
        .global(id)
        .function()
        .filter(|one| !one.is_declaration())
        .ok_or_else(|| format!("@{name} has no body"))?;
    let text = module.datalayout.as_deref().ok_or("a module with no datalayout")?;
    let layout = DataLayout::parse(text)?;
    let arch = Mos;
    let convention = isel::convention(module, &layout, id, &arch).map_err(|why| why.0)?;
    let target = builder_target();
    let mut info = FunctionLoweringInfo::new(function);
    let mut locals = alloca_sizes(module, function, &layout, &info)?;
    let mut selected = Vec::new();
    for &block in function.layout() {
        let built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        select_built(Some((function, block)), block, built, cpu, &mut info, &mut selected)?;
    }
    // The stack objects the builder made follow the static allocas.
    locals.extend(info.objects.iter().map(|(size, _)| i64::from(*size)));
    Ok(Selection { blocks: selected, locals })
}

/// The size of each static alloca, by the frame index `FunctionLoweringInfo`
/// gave it.
fn alloca_sizes(
    module: &Module,
    function: &Function,
    layout: &DataLayout,
    info: &FunctionLoweringInfo,
) -> Result<Vec<i64>, String> {
    let mut sizes = vec![0; info.static_allocas.iter().count()];
    for (value, index) in info.static_allocas.iter() {
        let ValueDef::Instruction(def) = function.value(value).def else {
            return Err("an alloca that is not an instruction".into());
        };
        let instruction = function.instruction(def);
        let Opcode::Alloca { allocated, .. } = instruction.opcode else {
            return Err("an alloca that is not an alloca".into());
        };
        let count = match instruction.operands.first() {
            None => 1,
            Some(Operand::Constant(id)) => match module.context.get(*id).kind {
                ConstantKind::Int(count) => count as i64,
                _ => return Err("an alloca whose count is not a number".into()),
            },
            Some(_) => return Err("an alloca with a count that is not a constant".into()),
        };
        sizes[*index as usize] = layout.alloc_size(&module.context.types, allocated) as i64 * count;
    }
    Ok(sizes)
}

/// One built block selected and scheduled, then the blocks its `switch`
/// lowering made. Every branch is to a label, so their position in the text
/// does not matter.
fn select_built(
    top: Option<(&Function, BlockId)>,
    block: BlockId,
    mut built: BlockDag,
    cpu: Cpu,
    info: &mut FunctionLoweringInfo,
    selected: &mut Vec<Selected>,
) -> Result<(), String> {
    let units = select_block(&mut built.dag, cpu).map_err(|why| format!("{why:?}"))?;
    // DEVIATE: refused. A `switch` made of several blocks puts one copy list on
    // all its edges, and a phi read on another edge is lost. Reason: the
    // edges leave blocks the builder made. Exit: split the critical edges
    // of the IR first (`llrm-transforms` `split_critical_edge`).
    if top.is_some() && !built.extra.is_empty() && !built.phi_updates.is_empty() {
        return Err("a switch to a block with phis: split its critical edges first".into());
    }
    // The block each phi is in, to split the copies by successor.
    let mut home: HashMap<ValueId, BlockId> = HashMap::new();
    let mut successors = Vec::new();
    if let Some((function, block)) = top.filter(|_| built.extra.is_empty()) {
        successors = function.successors(block).iter().copied().collect::<Vec<_>>();
        for &successor in &successors {
            for phi in
                function.iter_block(successor).take_while(|inst| function.instruction(*inst).opcode == Opcode::Phi)
            {
                home.extend(function.instruction(phi).result.map(|result| (result, successor)));
            }
        }
    }
    let mut edges: Vec<Edge> = Vec::new();
    for (at, (phi, input)) in built.phi_updates.iter().enumerate() {
        let own = *info.value_regs.get(phi).ok_or("a phi with no register")?;
        // A value of several registers has one entry per part, in order.
        let part = built.phi_updates[..at].iter().filter(|(one, _)| one == phi).count();
        let copy = (*input, if part == 0 { own } else { info.part_regs(own, part + 1)[part] });
        let to = if successors.len() > 1 { home.get(phi).copied() } else { None };
        match edges.iter_mut().find(|edge| edge.to == to) {
            Some(edge) => edge.copies.push(copy),
            None => edges.push(Edge { to, copies: vec![copy] }),
        }
    }
    selected.push(Selected { block, dag: built.dag, units, live_ins: built.live_ins, edges });
    for (extra, made) in built.extra {
        select_built(None, extra, made, cpu, info, selected)?;
    }
    Ok(())
}
