//! A function from IR to scheduled machine nodes: the shared builder, then
//! [`super::pipeline`]'s stages, block by block.
//!
//! DEVIATE: a phi is a copy at the end of each predecessor, not a parallel copy per edge: wrong
//! when the phi's old value is still read on the edge that does not take it. Reason: the spike has
//! no critical-edge splitting. Exit: the shared machine function.
//!
//! DEVIATE: no static allocas, jump tables, switches or live-in registers: the
//! builder makes the frame objects and the C convention passes what the registers do not hold on
//! the stack, so the slice (arithmetic, branches, calls) needs none. Reason:
//! frame objects wait for the 6502 machine function. Exit: M4.

use llrm_codegen::backend::isel;
use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::builder::{BuilderTarget, TargetNodes, build_block};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::Unit;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Module};

use super::hooks::Cpu;
use super::call::{CALL, RET_GLUE};
use super::pipeline::select_block;
use crate::target::Mos;

/// The builder's view of the 6502: 16-bit pointers, `i8` and `i16` in registers, a byte shift
/// count, and the `CALL`/`RET_GLUE` nodes of `own_ids.txt`.
pub fn builder_target() -> BuilderTarget {
    BuilderTarget {
        shift_amount: EVT::I8,
        legal_ints: vec![8, 16],
        jump_tables: false,
        target_nodes: Some(TargetNodes { call: CALL, ret: RET_GLUE, stack_pointer: "rs0", big_endian: false }),
        ..BuilderTarget::m16()
    }
}

/// One block, selected and scheduled.
pub struct Selected {
    pub block: BlockId,
    pub dag: SelectionDAG,
    pub units: Vec<Unit>,
    /// The registers the block reads on arrival, with the virtual register each is copied to.
    pub live_ins: Vec<(RegId, u32)>,
    /// The phi inputs this block exports: the register its value was copied to, and the phi's own.
    pub phi_copies: Vec<(u32, u32)>,
}

/// Function `name` of `module`, block by block in layout order.
pub fn select_function(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<Selected>, String> {
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
    let mut selected = Vec::new();
    for &block in function.layout() {
        let mut built = build_block(module, function, &layout, &target, &arch, &convention, &mut info, block)
            .map_err(|why| why.0.to_owned())?;
        if !built.extra.is_empty() {
            return Err("a switch lowering made blocks".to_owned());
        }
        let units = select_block(&mut built.dag, cpu).map_err(|why| format!("{why:?}"))?;
        let mut phi_copies = Vec::new();
        for (at, (phi, input)) in built.phi_updates.iter().enumerate() {
            let own = *info.value_regs.get(phi).ok_or("a phi with no register")?;
            // A value of several registers has one entry per part, in order.
            let part = built.phi_updates[..at].iter().filter(|(one, _)| one == phi).count();
            phi_copies.push((*input, if part == 0 { own } else { info.part_regs(own, part + 1)[part] }));
        }
        selected.push(Selected { block, dag: built.dag, units, live_ins: built.live_ins, phi_copies });
    }
    Ok(selected)
}
