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
use llrm_codegen::backend::selectiondag::known_bits::masked_value_is_zero;
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
    /// Every long multiply of an unsized function is the `__mulsi3` call, the near-power constants included
    /// (the form `DECOMPOSE_MULS` is the inline counterpart of, for a sized one).
    pub static CALL_MULS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while a function with a constant-count `memset` is selected the second time, each such fill an
    /// inline `ldir` fill (`Z80Calling::emit_memset`) where the first selection calls `__memseta`; the caller
    /// prices the two with the routine linked once (`compile::decomposed_functions`).
    pub static INLINE_FILLS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while a function is selected at a level that allows no trade between bytes and clocks (-O1, -Os, -Oz).
    pub static STRICT_LEVEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while a function with a variable 64-bit shift is selected the second
    /// time, each such shift an in-place loop over eight bytes of the frame
    /// (`Z80Lowering::lower_wide_shift`) where the first selection calls the
    /// shift routine; the caller prices the two with the routine linked once.
    pub static INLINE_SHIFTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while a function with a long divide is
    /// selected the second time, a function tuned for size calling the speed
    /// routines (`target_lowering(true)`); priced against the size routines by
    /// the smaller relative regret (`compile::decomposed_functions`).
    pub static SPEED_ROUTINES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Set while a function is selected with the words of its long adds and subtracts left to the generic
    /// expansion (an add and a compare of the sum), not the carry ops
    /// (`lower_carry_op`); the caller prices the two forms
    /// (`compile::lines_of`).
    pub static GENERIC_CARRY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The carry ops `lower_carry_op` made since the count was last taken: a function with none has no
    /// generic form to price.
    pub static CARRY_OPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Set while the generic form of a selection is emitted and allocated (`Selection::generic`).
    pub static GENERIC_FORM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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
    /// Selected at a level that allows no trade between bytes and clocks (`STRICT_LEVEL`).
    strict: bool,
    /// The module's word multiplies are all constants that chain (`prepare::mark_word_chains`): the routine goes.
    chain_credit: bool,
}

/// The bytes and T-states of a chain of shifts and adds on a word in `hl`, the source copied to `bc` when a step adds it.
fn word_chain_price(chain: &[(&'static str, i64)]) -> (i64, i64) {
    let (mut bytes, mut clocks) = (0, 0);
    let mut seeded = false;
    for &(step, m) in chain {
        let (b, t) = match step {
            "shl" => (m, 11 * m),
            "add" => (1, 11),
            "sub" => (3, 19),
            "fadd" => (3 + m, 19 + 11 * m),
            _ => (5 + m, 27 + 11 * m),
        };
        seeded |= matches!(step, "add" | "sub");
        bytes += b;
        clocks += t;
    }
    if seeded {
        bytes += 2;
        clocks += 8;
    }
    (bytes, clocks)
}

/// A word multiplied by a constant as the shifts and adds of `chain_search` (Bernstein 1986), where the pin leaves
/// every constant but `2^k +- 1` to the `__mulhi3` call: `ld de, #c; call` is 6 B and a loop of about 65 T a bit of
/// the constant. Taken where the chain is no larger and no slower (a strict level), or by the rate 1 B = 3 T.
///
/// DEVIATE Z80-MUL-CHAIN (owner z80, exit 2026-11-24): the pin has no such search; AArch64's `performMulCombine`
/// and X86's `combineMul` are the table-driven analogues.
/// The chain of shifts and adds for a word times `c`, with its bytes and T-states, and the `ld de, #c; call` it
/// replaces; `None` where the call is no dearer. `forced` takes the chain whatever its bytes (the routine is paid for).
pub(crate) fn word_chain(
    c: i64,
    strict: bool,
    forced: bool,
) -> Option<(Vec<(&'static str, i64)>, (i64, i64), (i64, i64))> {
    if c < 3 {
        return None;
    }
    let weight = if strict { 40 } else { 3 };
    let call = (6, 40 + 65 * i64::from(16 - (c as u16).leading_zeros()));
    let limit = if forced { i64::MAX / 8 } else { weight * call.0 + call.1 };
    let (chain, _) = llrm_codegen::backend::arithmetic::synthesize_chain(
        c,
        limit,
        &|m| m * (weight + 11),
        weight + 11,
        2 * weight + 8,
    )?;
    let price = word_chain_price(&chain);
    let wins = if forced {
        price.1 <= call.1
    } else if strict {
        price.0 <= call.0 && price.1 <= call.1
    } else {
        weight * price.0 + price.1 < limit
    };
    wins.then_some((chain, price, call))
}

/// Whether the pin turns a word times `c` into shifts and one add (`2^k +- 1`, times a power of two): no call.
pub(crate) fn pin_decomposes(c: u64) -> bool {
    let odd = c >> c.trailing_zeros().min(63);
    c == 0 || odd == 1 || (odd - 1).is_power_of_two() || (odd + 1).is_power_of_two()
}

fn word_product_chain(
    dag: &mut SelectionDAG,
    node: SDValue,
    strict: bool,
    credit: bool,
) -> Option<SDValue> {
    let (x, bits) = (dag.operand(node, 0), dag.constant(dag.operand(node, 1))?);
    if dag.constant(x).is_some() {
        return None;
    }
    let (chain, _, _) = word_chain(i64::from(bits as u16), strict, credit)?;
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: i64| {
        let by = dag.get_constant(by as u128, EVT::I8);
        dag.get_node(ISD::Shl, EVT::I16, &[value, by])
    };
    let mut current = x;
    for (step, m) in chain {
        current = match step {
            "shl" => shift(dag, current, m),
            "add" => dag.get_node(ISD::Add, EVT::I16, &[current, x]),
            "sub" => dag.get_node(ISD::Sub, EVT::I16, &[current, x]),
            "fadd" => {
                let moved = shift(dag, current, m);
                dag.get_node(ISD::Add, EVT::I16, &[current, moved])
            }
            _ => {
                let moved = shift(dag, current, m);
                dag.get_node(ISD::Sub, EVT::I16, &[moved, current])
            }
        };
    }
    Some(current)
}

/// `2^k +- 1` (k up to 8): shifts and one add (`decompose_mul_by_constant`).
fn near_power_factor(c: u128) -> bool {
    let near_power = |n: u128| n.is_power_of_two() && n <= 256;
    c >= 3 && (near_power(c - 1) || near_power(c + 1))
}

impl CombineHooks for Z80Combines {
    /// `isTruncateFree` and `isZExtFree` for a pair or a long and its low
    /// byte or word: the narrow value is a lane of the wide one, and an
    /// any-extension leaves the high lanes unwritten. `ShrinkDemandedOp` then
    /// does an add or a subtract whose result is truncated (a shift count
    /// `iv + off` read as a byte) on the byte, not on the word with its high
    /// half dropped after.
    fn free_narrowing(
        &self,
        wide: EVT,
        narrow: EVT,
    ) -> bool {
        matches!((wide, narrow), (EVT::I16 | EVT::I32, EVT::I8) | (EVT::I32, EVT::I16))
    }

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

    /// A shift is one `add hl, hl` or `sla`/`rl` pair a bit: `x * 36` is
    /// `((x << 3) + x) << 2`, 5 shifted bits, not `(x << 5) + (x << 2)`, 7.
    fn factor_trailing_zeros_of_mul(&self) -> bool {
        true
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
        match vt {
            EVT::I8 | EVT::I16 => true,
            EVT::I32 => !self.long_multiply && near_power_factor(c),
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
        matches!(op, ISD::Shl | ISD::Srl | ISD::And | ISD::Mul)
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        level: CombineLevel,
    ) -> Option<SDValue> {
        // Also called for every target node, whatever its operands.
        if dag.opcode(node) == ISD::Mul {
            let word = dag.value_type(node) == EVT::I16;
            return (word && level == CombineLevel::BeforeLegalizeTypes)
                .then(|| word_product_chain(dag, node, self.strict, self.chain_credit))
                .flatten();
        }
        if dag.opcode(node) == ISD::And {
            // A mask that fits the byte a word was extended from (zero or any: the high byte is masked off) is a byte `and`, extended after: no pair is made
            // for the extension and read back. The pin leaves it to isel.
            // DEVIATE Z80-BYTE-SHIFT (owner z80, exit 2026-12-15): with the byte shift, huffman_decode `getbit`.
            let (value, mask) = (dag.operand(node, 0), dag.operand(node, 1));
            let byte = (dag.value_type(node) == EVT::I16 && matches!(dag.opcode(value), ISD::ZeroExtend | ISD::AnyExtend))
                .then(|| dag.operand(value, 0))
                .filter(|byte| dag.value_type(*byte) == EVT::I8);
            return match (byte, dag.constant(mask)) {
                (Some(byte), Some(bits)) if bits <= 0xFF => {
                    let mask = dag.get_constant(bits, EVT::I8);
                    let masked = dag.get_node(ISD::And, EVT::I8, &[byte, mask]);
                    Some(dag.get_node(ISD::ZeroExtend, EVT::I16, &[masked]))
                }
                _ => None,
            };
        }
        if !matches!(dag.opcode(node), ISD::Shl | ISD::Srl) {
            return None;
        }
        let (value, amount) = (dag.operand(node, 0), dag.operand(node, 1));
        // A word whose high byte is zero, shifted right by less than 8 (C's promotion of a char): the shift runs on the
        // byte, one instruction a step against two, and the extension is what the demanded bits then fold away. The
        // pin narrows only a known amount (`SimplifyDemandedBits`, `SRL`).
        // DEVIATE Z80-BYTE-SHIFT (owner z80, exit 2026-12-15): huffman_decode `getbit`, `stream[i] >> (7 - n)`.
        if dag.opcode(node) == ISD::Srl {
            let width = dag.value_type(amount).size_in_bits();
            if level == CombineLevel::BeforeLegalizeTypes
                && dag.value_type(node) == EVT::I16
                && dag.constant(amount).is_none()
                && width >= 8
                && masked_value_is_zero(dag, value, 0xFF00)
                && masked_value_is_zero(dag, amount, ((1u128 << width) - 1) & !7)
            {
                let byte = dag.get_node(ISD::Truncate, EVT::I8, &[value]);
                let count = if width == 8 { amount } else { dag.get_node(ISD::Truncate, EVT::I8, &[amount]) };
                let shifted = dag.get_node(ISD::Srl, EVT::I8, &[byte, count]);
                return Some(dag.get_node(ISD::ZeroExtend, EVT::I16, &[shifted]));
            }
            return None;
        }
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
    (sized, long_multiply, cold, chain_credit): (bool, bool, bool, bool),
) -> Result<Vec<Unit>, Refusal> {
    let speed_routines = SPEED_ROUTINES.with(std::cell::Cell::get);
    let mut tli = target_lowering(speed_routines);
    if GENERIC_CARRY.with(std::cell::Cell::get) {
        for op in [ISD::UAddO, ISD::USubO, ISD::UAddOCarry, ISD::USubOCarry] {
            tli.set_operation_action(op, MVT::I16, llrm_codegen::backend::selectiondag::Action::Expand);
        }
    }
    if INLINE_SHIFTS.with(std::cell::Cell::get) {
        for op in [ISD::Shl, ISD::Srl, ISD::Sra] {
            tli.set_operation_action(op, MVT::I64, llrm_codegen::backend::selectiondag::Action::Custom);
        }
    }
    tli.set_opt_for_size(sized);
    // a long multiply is a call (`RTLIB::MUL_I32`) in a module with enough of
    // them to pay for the routine (`prepare::mark_long_multiplies`), at every
    // level: four word multiplies inline cost more bytes and more T-states
    let all_calls = CALL_MULS.with(std::cell::Cell::get);
    tli.set_force_shift_calls(all_calls && !sized);
    if long_multiply {
        tli.set_size_libcall_name(ISD::Mul, MVT::I32, "__mulsi3");
        if !sized {
            tli.set_libcall_name(ISD::Mul, MVT::I32, "__mulsi3");
            if !all_calls {
                tli.set_inline_constant_mul(near_power_factor);
            }
        }
    }
    // the product of two zero-extended words is a call in a block that runs
    // once (`block_frequencies` at most 1): 15 B against 216 B of partial
    // products for a few hundred T-states more; in a loop the products stay
    // inline (DEVIATE Z80-COLD-LIBCALL: the pin sizes per function only). Above -O1, where the module links
    // `__mulsi3` for its long products, the call is `__umulhisi3` for two words too: the same call, half the steps.
    if (cold || (long_multiply && !STRICT_LEVEL.with(std::cell::Cell::get))) && !sized {
        tli.set_libcall_name(ISD::UMulLoHi, MVT::I16, "__umulhisi3");
    }
    let mut lowering = Z80Lowering { sized: sized && !speed_routines };
    let mut combines = Z80Combines {
        long_multiply: (sized || all_calls) && long_multiply,
        strict: STRICT_LEVEL.with(std::cell::Cell::get),
        chain_credit,
    };
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
    // the users of each constant before any is selected: a selected store no longer names it
    let mut uses: llrm_support::hash::HashMap<u32, usize> = llrm_support::hash::HashMap::default();
    for (id, node) in dag.nodes().iter().enumerate() {
        if dag.is_deleted(id as u32) {
            continue;
        }
        let mut seen: Vec<u32> = Vec::new();
        for op in &node.ops {
            if !seen.contains(&op.node) && dag.nodes()[op.node as usize].opcode == ISD::Constant {
                seen.push(op.node);
                *uses.entry(op.node).or_default() += 1;
            }
        }
    }
    let mut hooks = Z80Hooks::new(cpu, false).with_uses(uses);
    do_instruction_selection_with(dag, &Z80Pins, |adapter, node| selector.select(adapter, &mut hooks, node))
        .map_err(|why| Refusal::CannotSelect(format!("{why:?}")))?;
    let root: SDValue = dag.get_root();
    Ok(schedule(dag, root, &Z80Sched))
}
