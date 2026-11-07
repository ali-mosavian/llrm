//! The shared DAG stages run for the Z80: the pin's `CodeGenAndEmitDAG` order
//! (combine, legalize types, combine, legalize the DAG, combine, select,
//! schedule) over a built block, with the Z80's tables and hooks. The same
//! stages as m68k's `pipeline.rs`; `DagTarget` will replace the pair.
//!
//! DEVIATE: no target combines beyond [`Z80Combines`] and no
//! `PreprocessISelDAG`. Reason: the slice does not reach them. Exit: M7.

use llrm_codegen::backend::registerinfo::RegId;
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use llrm_codegen::backend::selectiondag::dag_select::do_instruction_selection_with;
use llrm_codegen::backend::selectiondag::dagisel_adapter::PinRegisters;
use llrm_codegen::backend::selectiondag::function_lowering::FunctionLoweringInfo;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::live_out;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::schedule::{Preference, SchedInfo, Unit, schedule};
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

thread_local! {
    /// Set while a function is selected the second time, under `ILP`
    /// (DEVIATE Z80S1).
    pub static ILP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while a function that calls `__mulsi3` for its long multiplies is
    /// selected the second time, with the shifts and adds of `x * (2^k +- 1)`
    /// kept (`driver::selections_with_alternatives`).
    pub static DECOMPOSE_MULS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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

    fn call_frame_setup(&self) -> Option<u32> {
        Some(u32::from(super::generated::ids::opcode::ADJCALLSTACKDOWN))
    }

    fn call_frame_destroy(&self) -> Option<u32> {
        Some(u32::from(super::generated::ids::opcode::ADJCALLSTACKUP))
    }

    /// A flags result is the register `f`.
    fn fixed_register_def(
        &self,
        opcode: u32,
        result: usize,
    ) -> Option<u16> {
        let class = instrinfo::desc(opcode as u16)?.operands.get(result)?.reg_class;
        (u16::try_from(class) == Ok(pin::FlagsRegClassID)).then_some(pin::F)
    }

    fn source_order(&self) -> bool {
        true
    }

    /// DEVIATE Z80S1: the pin gives a target one DAG scheduler; the Z80 runs
    /// two and keeps the cheaper function (`compile::lines`). `Source` is what
    /// `createDefaultScheduler` gives a target that enables the machine
    /// scheduler; `ILP` is what it gives one that does not (M68k), and it
    /// fixed lz4's copy loops (`p + 1` ahead of the store through `p` kept five
    /// pairs live) but spills minmax and others. Reason: neither wins every
    /// function. Exit: the shared pre-RA machine scheduler runs on the Z80,
    /// then `ILP` and the second pass go. The pressure limits stay the pin's 0:
    /// the allocatable registers (7 bytes, 3 pairs) put 10 of the 60 cells
    /// above the tip, against 8; no limit scanned (pairs 0..3, bytes 0..7)
    /// got below 8.
    fn preference(&self) -> Preference {
        if ILP.with(std::cell::Cell::get) { Preference::Ilp } else { Preference::Source }
    }
}

/// Why a block did not get through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    Legalize(String),
    CannotSelect(String),
}

/// The Z80's combiner hooks.
struct Z80Combines {
    /// The size routine `__mulsi3` is this function's long multiply.
    long_multiply: bool,
}

impl CombineHooks for Z80Combines {
    /// DEVIATE Z80-SHL-COMMUTE: `(x + c) << n` stays. Commuted, the folded
    /// constant `c << n` is a `ld bc, #k` and an `add hl, bc` where the add of
    /// a small `c` was `inc hl`s (bintree -O2 +14 B). The pin's default is
    /// true. Owner: llrm-target-z80. Exit: 2026-11-20, a price per constant.
    fn is_desirable_to_commute_with_shift(
        &self,
        _dag: &SelectionDAG,
        _shift: SDValue,
        _level: CombineLevel,
    ) -> bool {
        false
    }

    /// A byte or word multiply by a constant is a library call (`__mulhi3`, a
    /// loop over the constant's bits, tens of T-states a bit); `2^n +- 1` as
    /// shifts and one add is a `add hl, hl` and a copy per shifted bit.
    ///
    /// A long multiplied by `2^k +- 1` (k up to 8) is `k` long shifts and an
    /// add (each a `add hl, hl` / `adc` pair); the multiply expands through
    /// byte lanes instead: 49 instructions for `x * 3`, 58 for `x * 7`. Where
    /// the function calls `__mulsi3` for its long multiplies (tuned for size,
    /// a module with enough of them) the call, about 14 B, is the smaller
    /// form: the decomposition left alone grew 12 -Os cells.
    fn decompose_mul_by_constant(
        &self,
        vt: EVT,
        c: u128,
    ) -> bool {
        let near_power = |n: u128| n.is_power_of_two() && n <= 256;
        match vt {
            EVT::I8 | EVT::I16 => true,
            EVT::I32 => !self.long_multiply && c >= 3 && (near_power(c - 1) || near_power(c + 1)),
            _ => false,
        }
    }

    /// The byte-lane multiply of a long whose three high bytes are zero is 15
    /// instructions for `x * 3` where the shifts and the add are 21 (a
    /// zero-extended byte, byte_multiply -O2 +36 B and +38,149 T); a
    /// zero-extended word keeps the decomposition (23 against 44).
    fn decompose_mul_by_constant_of(
        &self,
        dag: &SelectionDAG,
        x: SDValue,
        vt: EVT,
        c: u128,
    ) -> bool {
        self.decompose_mul_by_constant(vt, c)
            && !(vt == EVT::I32 && llrm_codegen::backend::selectiondag::known_bits::masked_value_is_zero(dag, x, 0xFFFF_FF00))
    }

    /// The pin's default: a compare of a masked or zero-extended value against
    /// a constant is made in the narrower type (`(and x, 7) == 3` is a byte
    /// `cp`), legal or not; the type legalizer promotes it to a byte. Without
    /// it the word compare kept a `sub a, #0` for the high byte the mask had
    /// zeroed (div_constants_short, 36 sites).
    fn is_setcc_type_desirable(
        &self,
        _narrow: EVT,
        _legal: bool,
    ) -> bool {
        true
    }

    /// A selector masked to three bits and compared with 0..7 in a chain is
    /// one `and` and a byte `cp` each, not a word compare with `sub a, #0` for
    /// the zeroed high byte (div_constants_short, 36 sites).
    fn narrow_setcc_with_other_uses(&self) -> bool {
        true
    }

    /// X86InstrCompiler.td: `shl x, 1` is `add x, x`. A long shifted by one is
    /// then a long add, whose carry is `adc hl, rr` (`carry.rs`); expanded as a
    /// shift it was the funnel of two words, 24 instructions.
    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        op == ISD::Shl
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        level: CombineLevel,
    ) -> Option<SDValue> {
        // Also called for every target node, whatever its operands.
        if dag.opcode(node) != ISD::Shl {
            return None;
        }
        let (value, amount) = (dag.operand(node, 0), dag.operand(node, 1));
        (level == CombineLevel::BeforeLegalizeTypes
            && dag.value_type(node) == EVT::I32
            && dag.constant(amount) == Some(1))
        .then(|| dag.get_node(ISD::Add, EVT::I32, &[value, value]))
    }

    /// A word compare against a constant is two byte compares and a branch
    /// between them; the compare of the loaded byte is one `cp`.
    ///
    /// DEVIATE: the pin widens (x86 and ARM compare words as cheaply as
    /// bytes). Exit: a cost query for the wider compare.
    fn extend_setcc_uses(
        &self,
        _wide: EVT,
    ) -> bool {
        false
    }
}

/// `CodeGenAndEmitDAG` up to the end of scheduling: the units, in order.
pub fn select_block(
    dag: &mut SelectionDAG,
    cpu: Cpu,
    info: &mut FunctionLoweringInfo,
    optimize: bool,
    (sized, long_multiply): (bool, bool),
) -> Result<Vec<Unit>, Refusal> {
    let mut tli = target_lowering();
    tli.set_opt_for_size(sized);
    // a long multiply is a call in a function tuned for size, in a module with
    // enough of them to pay for the routine (`prepare::mark_long_multiplies`)
    if long_multiply {
        tli.set_size_libcall_name(ISD::Mul, MVT::I32, "__mulsi3");
    }
    let mut lowering = Z80Lowering { sized };
    let mut combines = Z80Combines { long_multiply: sized && long_multiply };
    let arch = Z80;
    if optimize {
        live_out::assert_live_out_copies(dag, info);
    }
    DagCombiner::new(dag, &mut combines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    TypeLegalizer::new(dag, &tli)
        .with_target(&arch)
        .with_hooks(&mut lowering)
        .run()
        .map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut combines, CombineLevel::AfterLegalizeTypes).with_tli(&tli).run();
    DagLegalizer::new(dag, &tli, &mut lowering).run().map_err(|why| Refusal::Legalize(format!("{why:?}")))?;
    DagCombiner::new(dag, &mut combines, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();
    if optimize {
        live_out::compute_live_out_vreg_info(dag, info);
    }

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
