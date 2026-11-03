//! The shared DAG stages run for the Z80: the pin's `CodeGenAndEmitDAG` order
//! (combine, legalize types, combine, legalize the DAG, combine, select,
//! schedule) over a built block, with the Z80's tables and hooks. The same
//! stages as m68k's `pipeline.rs`; `DagTarget` will replace the pair.
//!
//! DEVIATE: no target combines and no `PreprocessISelDAG`. Reason: the slice
//! does not reach them. Exit: M7.

use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use llrm_codegen::backend::selectiondag::dag_select::do_instruction_selection_with;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::schedule::{SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::interp::Selector;

use super::generated::ids::reg as pin;
use super::generated::instrinfo::{self, flag};
use super::generated::{z80_z80, z80_z180};
use super::hooks::{Cpu, Z80Hooks};
use super::lowering::{Z80Lowering, target_lowering};
use crate::reg;
use crate::target::Z80;

/// Every register the pin numbers, with ours.
const REGISTERS: [(u16, RegId); 14] = [
    (pin::A, reg::A),
    (pin::B, reg::B),
    (pin::C, reg::C),
    (pin::D, reg::D),
    (pin::E, reg::E),
    (pin::H, reg::H),
    (pin::L, reg::L),
    (pin::BC, reg::BC),
    (pin::DE, reg::DE),
    (pin::HL, reg::HL),
    (pin::IX, reg::IX),
    (pin::IY, reg::IY),
    (pin::SP, reg::SP),
    (pin::F, reg::FLAGS),
];

/// The Z80's physical registers in the pin's numbering.
pub struct Z80Pins;

impl PinRegisters for Z80Pins {
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
pub struct Z80Sched;

impl SchedInfo for Z80Sched {
    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        Z80Pins.pin_register(id)
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
) -> Result<Vec<Unit>, Refusal> {
    let tli = target_lowering();
    let mut lowering = Z80Lowering;
    let arch = Z80;
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    TypeLegalizer::new(dag, &tli)
        .with_target(&arch)
        .with_hooks(&mut lowering)
        .run()
        .map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::AfterLegalizeTypes).with_tli(&tli).run();
    DagLegalizer::new(dag, &tli, &mut lowering).run().map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut NoCombines, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();

    let selector = Selector::new(match cpu {
        Cpu::Z80 => z80_z80::TABLE,
        Cpu::Z180 => z80_z180::TABLE,
    });
    let mut hooks = Z80Hooks::new(cpu, false);
    do_instruction_selection_with(dag, &Z80Pins, |adapter, node| selector.select(adapter, &mut hooks, node))
        .map_err(|why| Refusal::CannotSelect(format!("{why:?}")))?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &Z80Sched))
}
