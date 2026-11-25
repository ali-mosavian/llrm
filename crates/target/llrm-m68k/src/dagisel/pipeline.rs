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
use llrm_codegen::backend::selectiondag::dag_target::{DagTarget, FrameObjects, Phases, Select, select_units};
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::live_out;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::schedule::{Preference, SchedInfo, Unit};
use llrm_codegen::backend::selectiondag::{NodeData, SDValue};
use llrm_dagisel::interp::Selector;

use super::generated::ids::reg as pin;
use super::generated::instrinfo::flag;
use super::generated::{m68k_m68000, m68k_m68020};
use super::hooks::{Cpu, M68kHooks};
use super::lowering::{M68kLowering, target_lowering_with};
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
        super::local::desc(opcode as u16).map_or(0, |desc| usize::from(desc.num_defs))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        super::local::desc(opcode as u16).map_or(&[], |desc| desc.implicit_defs)
    }

    fn is_call(
        &self,
        opcode: u32,
    ) -> bool {
        super::local::desc(opcode as u16).is_some_and(|desc| desc.has(flag::CALL))
    }

    fn call_frame_setup(&self) -> Option<u32> {
        Some(u32::from(super::generated::ids::opcode::ADJCALLSTACKDOWN))
    }

    fn call_frame_destroy(&self) -> Option<u32> {
        Some(u32::from(super::generated::ids::opcode::ADJCALLSTACKUP))
    }

    /// A `move.l`, `clr.l` store of a call's argument: `(%sp)` or `(d,%sp)`.
    fn call_argument_slot(
        &self,
        dag: &SelectionDAG,
        node: u32,
    ) -> Option<(u32, i64)> {
        use super::generated::ids::opcode as op;
        let of = |at: u32| dag.node(SDValue { node: at, res: 0 });
        let this = of(node);
        let ISD::Machine(code) = this.opcode else { return None };
        // the displacement operand comes first in the `(d,An)` forms
        let (base, displacement) = match code as u16 {
            op::MOV32jr | op::MOV32ji | op::CLR32j => (this.ops.first()?, 0),
            op::MOV32pr | op::MOV32pi | op::CLR32p => {
                let NodeData::Const { bits, .. } = of(this.ops.first()?.node).data else { return None };
                (this.ops.get(1)?, bits as i64)
            }
            _ => return None,
        };
        let copy = of(base.node);
        let sp = copy.opcode == ISD::CopyFromReg
            && copy.ops.get(1).is_some_and(|reg| matches!(of(reg.node).data, NodeData::PhysReg(id) if id == reg::SP));
        let chain = this.ops.last()?;
        (sp && dag.value_type(*chain) == EVT::OTHER).then_some((chain.node, displacement))
    }

    fn copy_clobbers(&self) -> Option<u16> {
        Some(pin::CCR)
    }

    /// `M68kRegisterInfo::getCrossCopyRegClass` ([L101-L105][pin]) gives
    /// `CCR` the class `DR16`, so a flag live across a call cannot be copied
    /// within its class: the scheduler clones the definition
    /// (`CopyAndMoveSuccessors`) and the flags are recomputed after the call.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/M68k/M68kRegisterInfo.h#L101-L105
    fn has_cross_copy_class(
        &self,
        register: u16,
    ) -> bool {
        register == pin::CCR
    }

    /// Data registers 0-7 and address registers 0-5: the 14 a value can wait
    /// in.
    fn spill_threshold(&self) -> Option<u32> {
        Some(14)
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
    select_block_with(
        dag,
        cpu,
        false,
        false,
        false,
        false,
        true,
        Default::default(),
        Default::default(),
        &[],
        &mut Temporaries::new(0),
        None,
    )
}

static LOWERINGS_BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many lowering tables [`select_block_with`] has built: the pin builds
/// `M68kTargetLowering` once per subtarget, not once per block.
pub fn lowerings_built() -> usize {
    LOWERINGS_BUILT.load(std::sync::atomic::Ordering::Relaxed)
}

type Lowering = std::rc::Rc<llrm_codegen::backend::selectiondag::actions::TargetLowering>;

/// The lowering table of a CPU, the carry-chain setting, the module's library
/// calls and the size setting, built once per thread and not for every block
/// (4.9 M of grep's 132 M compile instructions at -O2).
fn lowering_for(
    cpu: Cpu,
    carry_chain: bool,
    libcalls: &[crate::libcalls::Kind],
    size: bool,
) -> Lowering {
    type Key = (Cpu, bool, Vec<crate::libcalls::Kind>, bool);
    thread_local! {
        static TABLES: std::cell::RefCell<Vec<(Key, Lowering)>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();
        if let Some((_, one)) = tables
            .iter()
            .find(|((c, cc, kinds, s), _)| (*c, *cc, kinds.as_slice(), *s) == (cpu, carry_chain, libcalls, size))
        {
            return one.clone();
        }
        let mut tli = target_lowering_with(cpu, carry_chain);
        crate::libcalls::apply(&mut tli, libcalls);
        // `DAG.shouldOptForSize()`: the routines tuned for size
        // (`set_size_libcall_name`)
        tli.set_opt_for_size(size);
        LOWERINGS_BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let made = Lowering::new(tli);
        tables.push(((cpu, carry_chain, libcalls.to_vec(), size), made.clone()));
        made
    })
}

/// [`select_block`], with the pin's source-order scheduler where
/// `source_order`, `(An)+` and `-(An)` accesses where `indexed`, and `addq`
/// and `subq` where `short`.
pub fn select_block_with(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    source_order: bool,
    indexed: bool,
    short: bool,
    absolute_data: bool,
    carry_chain: bool,
    price: crate::timing::Price,
    mul: super::mulprice::Chain,
    libcalls: &[crate::libcalls::Kind],
    frame: &mut Temporaries,
    live: Option<&mut FunctionLoweringInfo>,
) -> Result<Vec<Unit>, Refusal> {
    let tli = lowering_for(cpu, carry_chain, libcalls, price == crate::timing::Price::Size);
    let mut lowering = M68kLowering {
        indexed,
        short,
        absolute_data,
        half_mul: short && super::lowering::prefers_word_multiply(cpu),
        cpu,
        price,
        mul,
    };
    let selector = Selector::new(match cpu {
        Cpu::M68000 | Cpu::M68010 => m68k_m68000::TABLE,
        Cpu::M68020 | Cpu::M68030 | Cpu::M68040 | Cpu::M68060 => m68k_m68020::TABLE,
    });
    let mut target = M68kTarget {
        tli: &tli,
        lowering: &mut lowering,
        chooser: M68kChooser { selector: &selector, hooks: M68kHooks::new(cpu, false), free_any_extend: short },
        short,
        live,
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
    short: bool,
    /// What the blocks before this one said of the registers they copied out
    /// (`FunctionLoweringInfo::LiveOutRegInfo`).
    live: Option<&'a mut FunctionLoweringInfo>,
}

struct M68kChooser<'a> {
    selector: &'a Selector,
    hooks: M68kHooks,
    /// DEVIATE M68K-ANYEXT ([`super::anyext`]); false is the pin's `MOVZX`.
    free_any_extend: bool,
}

impl Select for M68kChooser<'_> {
    fn select(
        &mut self,
        adapter: &mut llrm_codegen::backend::selectiondag::dagisel_adapter::DagisAdapter<'_>,
        node: u32,
    ) -> llrm_dagisel::interp::Outcome {
        super::indexed::select(adapter, node)
            .or_else(|| super::bfffo::select(adapter, node))
            .or_else(|| if self.hooks.folds_memory() { super::rmw::select(adapter, node) } else { None })
            .or_else(|| if self.hooks.has_bit_field_extract() { super::bfextu::select(adapter, node) } else { None })
            .or_else(|| if self.free_any_extend { super::zextmask::select(adapter, node) } else { None })
            .or_else(|| if self.free_any_extend { super::anyext::select(adapter, node) } else { None })
            .unwrap_or_else(|| self.selector.select(adapter, &mut self.hooks, node))
    }
}

impl DagTarget for M68kTarget<'_> {
    type Lowering = M68kLowering;

    fn phases(&mut self) -> Phases<'_, M68kLowering> {
        Phases { tli: self.tli, lowering: &mut *self.lowering, arch: Some(&M68k), opt_none: false }
    }

    /// `getCopyFromRegs`: a register the earlier blocks said something about
    /// is read back asserted.
    fn prepare(
        &mut self,
        dag: &mut SelectionDAG,
    ) {
        if let Some(live) = &self.live {
            live_out::assert_live_out_copies(dag, live);
        }
    }

    /// `ComputeLiveOutVRegInfo`, after the last combine.
    fn preprocess(
        &mut self,
        dag: &mut SelectionDAG,
        _temporaries: &mut dyn FrameObjects,
    ) {
        if let Some(live) = &mut self.live {
            live_out::compute_live_out_vreg_info(dag, live);
        }
    }

    fn selection(&mut self) -> (&dyn PinRegisters, &mut dyn Select) {
        (&M68kPins, &mut self.chooser)
    }

    fn postprocess(
        &mut self,
        dag: &mut SelectionDAG,
    ) {
        super::btst::immediate_bit_numbers(dag);
        if self.short {
            super::short::narrow(dag);
        }
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

impl llrm_codegen::backend::selectiondag::dag_target::FrameObjects for Temporaries {
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
