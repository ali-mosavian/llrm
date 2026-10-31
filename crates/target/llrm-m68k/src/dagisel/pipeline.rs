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
use llrm_codegen::backend::selectiondag::SelectionDAG;
use llrm_codegen::backend::selectiondag::dag_target::{DagTarget, Phases, Select, select_units};
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::schedule::{Preference, SchedInfo, Unit};
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

    /// `PC` has no register id here: the addressing-mode matchers still ask
    /// whether a base is it.
    fn is_fixed(
        &self,
        register: u16,
    ) -> bool {
        register == pin::PC
    }
}

/// What the scheduler reads of the instruction descriptions.
pub struct M68kSched {
    /// Keep the source order of the pin's `-pre-RA-sched=source` instead of the
    /// ILP list scheduler.
    pub source_order: bool,
}

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
        if self.source_order { Preference::Source } else { Preference::Ilp }
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
    select_block_with(dag, cpu, false, false, &mut Temporaries::new(0))
}

/// [`select_block`], with a compare's load put where a pattern folds it where
/// `load_as_source` (see `lowering::lower_setcc`).
pub fn select_block_with(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    load_as_source: bool,
    source_order: bool,
    frame: &mut Temporaries,
) -> Result<Vec<Unit>, Refusal> {
    let tli = target_lowering(cpu);
    let mut lowering = M68kLowering { load_as_source };
    let selector = Selector::new(match cpu {
        Cpu::M68000 => m68k_m68000::TABLE,
        Cpu::M68020 => m68k_m68020::TABLE,
    });
    let mut target = M68kTarget {
        tli: &tli,
        lowering: &mut lowering,
        chooser: M68kChooser { selector: &selector, hooks: M68kHooks::new(cpu, false) },
    };
    select_units(dag, &mut target, &M68kSched { source_order }, frame).map_err(|why| match why {
        llrm_codegen::backend::selectiondag::dag_select::Refusal::Legalize(why) => Refusal::Legalize(why),
        other => Refusal::CannotSelect(format!("{other:?}")),
    })
}

struct M68kTarget<'a> {
    tli: &'a llrm_codegen::backend::selectiondag::actions::TargetLowering,
    lowering: &'a mut M68kLowering,
    chooser: M68kChooser<'a>,
}

struct M68kChooser<'a> {
    selector: &'a Selector,
    hooks: M68kHooks,
}

impl Select for M68kChooser<'_> {
    fn select(
        &mut self,
        adapter: &mut llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter<'_>,
        node: u32,
    ) -> llrm_dagisel::interp::Outcome {
        self.selector.select(adapter, &mut self.hooks, node)
    }
}

impl DagTarget for M68kTarget<'_> {
    type Lowering = M68kLowering;

    fn phases(&mut self) -> Phases<'_, M68kLowering> {
        Phases { tli: self.tli, lowering: &mut *self.lowering, arch: Some(&M68k) }
    }

    fn selection(&mut self) -> (&dyn PinRegisters, &mut dyn Select) {
        (&M68kPins, &mut self.chooser)
    }
}

/// `CreateStackTemporary`: the scratch frame objects a block's legalization
/// asks for, numbered after the function's static allocas.
pub struct Temporaries {
    next: i32,
    /// `(frame index, size, alignment)` of each one made, as
    /// `Selected::locals`.
    pub made: Vec<(i32, i64, i64)>,
}

impl Temporaries {
    /// Frame indices from `first` on: the number of frame objects there are.
    pub fn new(first: i32) -> Temporaries {
        Temporaries { next: first, made: Vec::new() }
    }
}

impl llrm_codegen::backend::selectiondag::x86_preprocess::FrameObjects for Temporaries {
    fn objects(&self) -> i32 {
        self.next
    }

    fn create_stack_temporary(
        &mut self,
        size: u32,
        align: u32,
    ) -> i32 {
        let index = self.next;
        self.next += 1;
        self.made.push((index, i64::from(size), i64::from(align.max(1))));
        index
    }
}
