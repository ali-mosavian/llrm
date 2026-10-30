//! The shared DAG stages run for m68k: the pin's `CodeGenAndEmitDAG` order
//! (combine, legalize types, combine, legalize the DAG, combine, select,
//! schedule) over a built block, with m68k's tables and hooks.
//!
//! # How it works
//! The stages are the codegen crate's; what is m68k's is the action table and
//! lowering ([`super::lowering`]), the matcher table and hooks, the pin's
//! register numbers ([`M68kPins`]) and the instruction descriptions the
//! scheduler reads ([`M68kSched`]).
//!
//! DEVIATE: no target combines (`PerformDAGCombine` of M68k is not translated)
//! and no `PreprocessISelDAG`. Reason: the slice does not reach them. Exit: M4.
//!
//! # See also
//! `SelectionDAGISel::CodeGenAndEmitDAG`, pin d1106deb71cc, and
//! `dag_select.rs` in the codegen crate for the x86 run of the same stages.

use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use llrm_codegen::backend::selectiondag::dag_select::do_instruction_selection_with;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::schedule::{Preference, SchedInfo, Unit, schedule};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::interp::Selector;

use super::generated::ids::reg as pin;
use super::generated::instrinfo::{self, flag};
use super::generated::{m68k_m68000, m68k_m68020};
use super::hooks::{Cpu, M68kHooks};
use super::lowering::{M68kLowering, target_lowering};
use crate::reg;
use crate::target::M68k;

/// Every register the pin numbers, with ours.
const REGISTERS: [(u16, RegId); 41] = [
    (pin::CCR, reg::CCR),
    (pin::SP, reg::SP),
    (pin::WSP, reg::WSP),
    (pin::A0, reg::A0),
    (pin::A1, reg::A1),
    (pin::A2, reg::A2),
    (pin::A3, reg::A3),
    (pin::A4, reg::A4),
    (pin::A5, reg::A5),
    (pin::A6, reg::A6),
    (pin::BD0, reg::BD0),
    (pin::BD1, reg::BD1),
    (pin::BD2, reg::BD2),
    (pin::BD3, reg::BD3),
    (pin::BD4, reg::BD4),
    (pin::BD5, reg::BD5),
    (pin::BD6, reg::BD6),
    (pin::BD7, reg::BD7),
    (pin::D0, reg::D0),
    (pin::D1, reg::D1),
    (pin::D2, reg::D2),
    (pin::D3, reg::D3),
    (pin::D4, reg::D4),
    (pin::D5, reg::D5),
    (pin::D6, reg::D6),
    (pin::D7, reg::D7),
    (pin::WA0, reg::WA0),
    (pin::WA1, reg::WA1),
    (pin::WA2, reg::WA2),
    (pin::WA3, reg::WA3),
    (pin::WA4, reg::WA4),
    (pin::WA5, reg::WA5),
    (pin::WA6, reg::WA6),
    (pin::WD0, reg::WD0),
    (pin::WD1, reg::WD1),
    (pin::WD2, reg::WD2),
    (pin::WD3, reg::WD3),
    (pin::WD4, reg::WD4),
    (pin::WD5, reg::WD5),
    (pin::WD6, reg::WD6),
    (pin::WD7, reg::WD7),
];

/// m68k's physical registers in the pin's numbering.
pub struct M68kPins;

impl PinRegisters for M68kPins {
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

    /// `PC` has no register id here: the addressing-mode matchers still ask whether a base is it.
    fn is_fixed(
        &self,
        register: u16,
    ) -> bool {
        register == pin::PC
    }
}

/// What the scheduler reads of the instruction descriptions.
pub struct M68kSched;

impl SchedInfo for M68kSched {
    fn pin_register(
        &self,
        id: RegId,
    ) -> Option<u16> {
        M68kPins.pin_register(id)
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

    fn copy_clobbers(&self) -> Option<u16> {
        Some(pin::CCR)
    }

    /// M68k does not enable the machine scheduler: `createDefaultScheduler`
    /// gives it the ILP list scheduler.
    fn preference(&self) -> Preference {
        if std::env::var_os("LLRM_M68K_SOURCE_SCHED").is_some() { Preference::Source } else { Preference::Ilp }
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
    select_block_with(dag, cpu, false)
}

/// [`select_block`], with a compare's load put where a pattern folds it where
/// `load_as_source` (see `lowering::lower_setcc`).
pub fn select_block_with(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    load_as_source: bool,
) -> Result<Vec<Unit>, Refusal> {
    let tli = target_lowering(cpu);
    let mut lowering = M68kLowering { load_as_source };
    let arch = M68k;
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
        Cpu::M68000 => m68k_m68000::TABLE,
        Cpu::M68020 => m68k_m68020::TABLE,
    });
    let mut hooks = M68kHooks::new(cpu, false);
    do_instruction_selection_with(dag, &M68kPins, |adapter, node| selector.select(adapter, &mut hooks, node))
        .map_err(|why| Refusal::CannotSelect(format!("{why:?}")))?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &M68kSched))
}
