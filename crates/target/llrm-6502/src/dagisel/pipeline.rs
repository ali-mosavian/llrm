//! The shared DAG stages run for the 6502: the pin's `CodeGenAndEmitDAG` order
//! (combine, legalize types, combine, legalize the DAG, combine, select,
//! schedule) over a built block, with the 6502's tables and hooks. The same
//! stages as m68k's `pipeline.rs`; `DagTarget` will replace the pair.
//!
//! DEVIATE: no target combines and no `PreprocessISelDAG`. Reason: the slice
//! does not reach them. Exit: M7.

use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineLevel, DagCombiner};
use llrm_codegen::backend::selectiondag::dag_select::do_instruction_selection_with;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::interp::Selector;

use super::combine::{MosCombines, MosLateCombines};
use super::generated::ids::reg as pin;
use super::generated::instrinfo::{self, flag};
use super::generated::{mos_mos6502, mos_w65c02};
use super::hooks::{Cpu, MosHooks};
use super::lowering::{MosLowering, target_lowering};
use crate::reg;
use crate::target::Mos;

/// Every register the pin numbers (the toy `.td` has eight zero-page bytes),
/// with ours. `P` and `C` are both our `flags`; the first match names it.
const REGISTERS: [(u16, RegId); 17] = [
    (pin::A, reg::A),
    (pin::X, reg::X),
    (pin::Y, reg::Y),
    (pin::P, reg::FLAGS),
    (pin::C, reg::FLAGS),
    (pin::RC0, reg::RC0),
    (pin::RC1, reg::RC1),
    (pin::RC2, reg::RC2),
    (pin::RC3, reg::RC3),
    (pin::RC4, reg::RC4),
    (pin::RC5, reg::RC5),
    (pin::RC6, reg::RC6),
    (pin::RC7, reg::RC7),
    (pin::RS0, reg::RS0),
    (pin::RS1, reg::RS1),
    (pin::RS2, reg::RS2),
    (pin::RS3, reg::RS3),
];

/// The 6502's physical registers in the pin's numbering.
pub struct MosPins;

impl PinRegisters for MosPins {
    fn phys_reg(
        &self,
        register: u16,
    ) -> Option<RegId> {
        REGISTERS.iter().find(|(number, _)| *number == register).map(|(_, id)| *id)
    }

    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        REGISTERS.iter().find(|(_, one)| *one == id).map(|(number, _)| *number)
    }
}

/// What the scheduler reads of the instruction descriptions.
pub struct MosSched;

impl SchedInfo for MosSched {
    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        MosPins.pin_register(id)
    }

    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        instrinfo::desc(opcode as u16).map_or(0, |desc| usize::from(desc.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        instrinfo::desc(opcode as u16).map_or(&[], |desc| desc.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        instrinfo::desc(opcode as u16).is_some_and(|desc| desc.has(flag::CALL))
    }

    fn call_frame_setup(&self) -> Option<u32> {
        Some(u32::from(super::generated::ids::opcode::ADJCALLSTACKDOWN))
    }

    fn call_frame_destroy(&self) -> Option<u32> {
        Some(u32::from(super::generated::ids::opcode::ADJCALLSTACKUP))
    }

    fn source_order(&self) -> bool {
        true
    }
}

/// Why a block did not get through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    Legalize(String),
    CannotSelect(String),
}

/// `CodeGenAndEmitDAG` up to the end of scheduling: the units, in order.
pub fn select_block(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    sized: bool,
    synthesize: bool,
) -> Result<Vec<Unit>, Refusal> {
    let tli = target_lowering();
    let mut lowering = MosLowering {
        widen_products: match super::function::widen_products() {
            super::function::Widen::Off => false,
            super::function::Widen::Unsized => !sized,
            super::function::Widen::Always => true,
        },
    };
    let arch = Mos;
    DagCombiner::new(dag, &mut MosCombines { sized, synthesize }, CombineLevel::BeforeLegalizeTypes)
        .with_tli(&tli)
        .run();
    TypeLegalizer::new(dag, &tli)
        .with_target(&arch)
        .with_hooks(&mut lowering)
        .run()
        .map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut MosLateCombines, CombineLevel::AfterLegalizeTypes).with_tli(&tli).run();
    DagLegalizer::new(dag, &tli, &mut lowering).run().map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    // DEVIATE 6502-ASSERT (owner wasm-3, exit 2026-11-24): the pin keeps an
    // assertion to selection; one a truncation reads blocks the byte
    // narrowing of a loaded boolean (`lda g`), leaving `lda (zp),y`.
    drop_assertions_read_as_bytes(dag);
    DagCombiner::new(dag, &mut MosLateCombines, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();

    let selector = Selector::new(match cpu {
        Cpu::Mos6502 => mos_mos6502::TABLE,
        Cpu::W65c02 => mos_w65c02::TABLE,
    });
    let mut hooks = MosHooks::new(cpu, false);
    do_instruction_selection_with(dag, &MosPins, |adapter, node| selector.select(adapter, &mut hooks, node))
        .map_err(|why| Refusal::CannotSelect(format!("{why:?}")))?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &MosSched))
}

/// An `assertzext` or `assertsext` only truncations read: the byte they keep
/// is the operand's low byte, so the assertion tells nothing and blocks the
/// narrowing of the operand's load.
fn drop_assertions_read_as_bytes(dag: &mut SelectionDAG) {
    for node in dag.node_ids().collect::<Vec<_>>() {
        let value = SDValue { node, res: 0 };
        if matches!(dag.opcode(value), ISD::AssertSext | ISD::AssertZext)
            && dag.has_users(value)
            && dag.users(value).all(|user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::Truncate)
        {
            let operand = dag.operand(value, 0);
            dag.replace_all_uses_with(value, operand);
        }
    }
}
