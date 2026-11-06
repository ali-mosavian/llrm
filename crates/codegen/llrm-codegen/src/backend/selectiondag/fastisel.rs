//! `FastISel` for x86 at -O0: the pin's `-fast-isel` path, which selects an
//! IR block straight to machine nodes without the combiner, the legalizers or
//! the matcher.
//!
//! Off unless `LLRM_FAST_ISEL=1`, and then only at -O0 (`!cpu.optimized`); -O1
//! and up never see it. A function it cannot take is selected by the DAG path,
//! whole (DEVIATE F2).
//!
//! # See also
//! `docs`: `/home/alim/scratch/handoff/o0-fastisel-poc-plan-2026-11-06.md`.

use std::cell::Cell;

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Function, Module, Operand};
use llrm_mir::opcode::Opcode;

use super::builder::{BlockDag, BuilderTarget, fresh_dag};
use super::dag::SDValue;
use super::mvt::EVT;
use super::function_lowering::FunctionLoweringInfo;
use crate::backend::isel::Convention;

thread_local! {
    static TOOK: Cell<bool> = const { Cell::new(false) };
}

/// Whether `LLRM_FAST_ISEL=1` asks for the fast path.
pub fn enabled() -> bool {
    llrm_support::env_var("LLRM_FAST_ISEL").is_some_and(|value| value == "1")
}

/// Records whether the function being selected goes through FastISel.
pub fn note(fast: bool) {
    TOOK.with(|took| took.set(fast));
}

/// The function just selected went through FastISel: read once by the caller
/// that records the selector.
pub fn take_took() -> bool {
    TOOK.with(|took| took.replace(false))
}

/// FastISel's blocks for every block of `function`, or none: one block it
/// cannot take sends the whole function to the DAG path.
// DEVIATE F1: hand-written subset; exit 2026-11-27 (a generator over
// X86GenFastISel.inc). DEVIATE F2: whole-function fallback; exit with integer +
// call coverage. DEVIATE F3: no FP, vector, atomics or intrinsics; exit after
// parity.
#[allow(clippy::too_many_arguments)]
pub fn build_all(
    module: &Module,
    function: &Function,
    target: &BuilderTarget,
    opcode: &dyn Fn(&str) -> Option<u16>,
    convention: &Convention,
    arch: &dyn llrm_target::Target,
    info: &mut FunctionLoweringInfo,
) -> Option<Vec<(BlockId, BlockDag)>> {
    let (mov, ret) = (opcode("MOV32ri")?, opcode("RET")?);
    if !function.parameters().is_empty() || module.signature(function.ty).2 || function.layout().len() != 1 {
        return None;
    }
    let block = function.layout()[0];
    let insts: Vec<_> = function.iter_block(block).collect();
    let [only] = insts[..] else { return None };
    let instruction = function.instruction(only);
    let (Opcode::Ret, [Operand::Constant(constant)]) = (&instruction.opcode, instruction.operands.as_slice()) else {
        return None;
    };
    let ConstantKind::Int(value) = module.context.get(*constant).kind else { return None };
    let ty = function.operand_type(&module.context, instruction.operands[0])?;
    if module.context.types.int_bits(ty)? != 32 || convention.returns.len() != 1 {
        return None;
    }
    let mut dag = fresh_dag(module, function, target, info, block, None);
    let vt = EVT::I32;
    let entry = dag.entry_node();
    let immediate = dag.get_constant_with(value as u128, vt, true, false);
    let moved = dag.intern(
        super::isd::ISD::Machine(u32::from(mov)),
        &[vt],
        &[immediate],
        Default::default(),
        super::dag::NodeData::None,
    );
    let eax = arch.registers().named(convention.returns[0], 4);
    let copy = dag.get_copy_to_phys_reg(entry, eax, moved, None);
    let zero = dag.get_constant_with(0, vt, true, false);
    let register = dag.get_phys_register(vt, eax);
    let glue = SDValue { node: copy.node, res: 1 };
    let ret = dag.intern(
        super::isd::ISD::Machine(u32::from(ret)),
        &[EVT::OTHER],
        &[zero, register, copy, glue],
        Default::default(),
        super::dag::NodeData::None,
    );
    dag.set_root(ret);
    Some(vec![(
        block,
        BlockDag {
            dag,
            phi_updates: Vec::new(),
            live_ins: Vec::new(),
            extra: Vec::new(),
            tail_calls: Vec::new(),
            values: Vec::new(),
        },
    )])
}
