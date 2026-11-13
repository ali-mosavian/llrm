//! The 6502's action table: what the DAG legalizers do with each operation.
//!
//! `i8` and `i16` are register types (`Imag8`, `Imag16`: the pair is what a
//! pointer lives in). Byte and word add, subtract and logic are instructions;
//! the carry forms (`UADDO`, `UADDO_CARRY`, `USUBO`, `USUBO_CARRY`, the carry
//! an `i1` in the `C` register) are too, so the type legalizer expands an `i32`
//! into two words joined by the carry, an `i64` into four. There is no
//! multiply, divide or shift by a variable: those are library calls.
//!
//! A comparison is `MOSISD::CMP a, b`: the `i8` flags of `a - b`. `SETCC`,
//! `CMOV` and `BRCOND` read a condition code (a constant, [`cond`]) and those
//! flags. Only the flags the 6502 branches on are codes: Z (`EQ`, `NE`), C
//! (`CC`, `CS`) and N (`MI`, `PL`). An unsigned greater-than swaps the
//! operands; a signed compare flips both sign bits and compares unsigned,
//! except against zero, where N is the sign.
//!
//! A word compare is `CMP` too (C and N come out of the high byte's
//! subtract-with-borrow); equality is `CMPZ`, whose Z covers both bytes.

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::known_bits::{masked_value_is_zero, num_sign_bits};
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{
    Action, LoadExt, MemBase, MemInfo, NodeData, SDNodeFlags, SDValue, SelectionDAG,
};
use llrm_codegen::backend::selectiondag::{TargetLowering, TargetLoweringInfo};

use super::generated::ids::isd as ids;

/// The 6502's own nodes, numbered by `own_ids.txt`.
pub mod mosisd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const WRAPPER: ISD = ISD::Target(ids::WRAPPER);
    pub const MUL32: ISD = ISD::Target(ids::MUL32);
    pub const UMULW16: ISD = ISD::Target(ids::UMULW16);
    pub const SMULW16: ISD = ISD::Target(ids::SMULW16);
    pub const UDIV32: ISD = ISD::Target(ids::UDIV32);
    pub const SDIV32: ISD = ISD::Target(ids::SDIV32);
    pub const UREM32: ISD = ISD::Target(ids::UREM32);
    pub const SREM32: ISD = ISD::Target(ids::SREM32);
    pub const SHL32: ISD = ISD::Target(ids::SHL32);
    pub const SRL32: ISD = ISD::Target(ids::SRL32);
    pub const SRA32: ISD = ISD::Target(ids::SRA32);
    pub const FSHL1: ISD = ISD::Target(ids::FSHL1);
    pub const FSHR1: ISD = ISD::Target(ids::FSHR1);
    pub const SRL1C: ISD = ISD::Target(ids::SRL1C);
    pub const CMPZ: ISD = ISD::Target(ids::CMPZ);
    pub const CMP32: ISD = ISD::Target(ids::CMP32);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
}

/// Condition codes; a code and its opposite differ in bit 0.
pub mod cond {
    pub const EQ: u128 = 0;
    pub const NE: u128 = 1;
    pub const CC: u128 = 2;
    pub const CS: u128 = 3;
    pub const MI: u128 = 4;
    pub const PL: u128 = 5;
}

pub fn target_lowering() -> TargetLowering {
    use ISD::*;
    let mut tl = TargetLowering::empty(
        TargetLoweringInfo::new(&[MVT::I1, MVT::I8, MVT::I16], false).with_soft_float().with_int_size(16),
    );
    tl.init_actions();
    // SD241: the shift routine stays for a variable amount known small until an inline policy is priced
    tl.set_libcall_for_small_variable_shift();
    for vt in [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        for ext in [LoadExt::SExtLoad, LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, MVT::I1, Action::Promote);
        }
    }
    // a sign-extending load is a load and a sign extension: no instruction
    // reads one
    for (vt, mem) in [(MVT::I16, MVT::I8), (MVT::I32, MVT::I8), (MVT::I32, MVT::I16), (MVT::I64, MVT::I8)]
        .into_iter()
        .chain([(MVT::I64, MVT::I16), (MVT::I64, MVT::I32)])
    {
        tl.set_load_ext_action(LoadExt::SExtLoad, vt, mem, Action::Expand);
    }
    // and a zero- or any-extending one is not selected from a global's address
    // (a pointer pair and `lda (zp),y` where `lda abs` is one instruction),
    // or at all to a type that is not a register: split again
    for (vt, mem) in [
        (MVT::I16, MVT::I8),
        (MVT::I32, MVT::I8),
        (MVT::I32, MVT::I16),
        (MVT::I64, MVT::I8),
        (MVT::I64, MVT::I16),
        (MVT::I64, MVT::I32),
    ] {
        for ext in [LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, mem, Action::Expand);
        }
    }
    for (vt, mem) in [(MVT::I64, MVT::I32), (MVT::I64, MVT::I16), (MVT::I64, MVT::I8), (MVT::I32, MVT::I16)]
        .into_iter()
        .chain([(MVT::I32, MVT::I8), (MVT::I16, MVT::I8)])
    {
        tl.set_trunc_store_action(vt, mem, Action::Expand);
    }
    let mut set = |ops: &[ISD], action: Action| {
        for &op in ops {
            for vt in [MVT::I8, MVT::I16] {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[Add, Sub, And, Or, Xor, UAddO, USubO, UAddOCarry, USubOCarry], Action::Legal);
    // Multiply, divide, remainder and every shift are instructions; the
    // variable ones expand to runtime calls where they are emitted.
    set(&[Mul, SDiv, SRem, Shl, Srl, Sra], Action::Legal);
    set(&[SDivRem, MulHS, MulHU, UMulLoHi, SMulLoHi], Action::Expand);
    set(&[UMulO], Action::Expand);
    // an unsigned divide and remainder are the one runtime call that makes
    // both: the legalizer expands each to the `UDIVREM` node, and the pair
    // of one dividend and divisor is one node
    set(&[UDiv, URem], Action::Expand);
    set(&[UDivRem], Action::Legal);
    set(&[CtTz, CtLz, CtPop, BSwap], Action::Expand);
    // rotates are shifts and an or, built where they are lowered
    set(&[Rotl, Rotr], Action::Custom);
    set(&[ZeroExtend, AnyExtend], Action::Custom);
    set(&[SignExtend], Action::Custom);
    // a sign extension in a register is a pair of shifts (a negated bit for a
    // bit)
    set(&[SignExtendInReg], Action::Expand);
    // an undefined value is a zero of the type (the legalizer builds it) until
    // the MIR has `undef`
    set(&[Undef, Poison], Action::Expand);
    // `CLC; ADC; ROR` keeps the carry out of a byte add: the floor and ceiling
    // averages of two bytes
    for op in [AvgFloorU, AvgCeilU] {
        tl.set_operation_action(op, MVT::I8, Action::Legal);
    }
    tl.set_operation_action(Undef, MVT::I1, Action::Expand);
    // `va_start` stores the address of the first unnamed argument (`lower_vastart_frame_index`, as llvm-mos's
    // `legalizeVAStart` stores `VarArgsStackIndex`); the rest of a `va_list` is a pointer the generic expansions walk
    tl.set_operation_action(VAStart, MVT::Other, Action::Custom);
    for op in [VAEnd, VACopy] {
        tl.set_operation_action(op, MVT::Other, Action::Expand);
    }
    tl.set_operation_action(Poison, MVT::I1, Action::Expand);
    // its action is read at the type it extends from, a bit among them
    tl.set_operation_action(SignExtendInReg, MVT::I1, Action::Expand);
    // A long multiply, divide or shift by a variable is one node of two words
    // (`lower_long`).
    for op in [Mul, SDiv, UDiv, SRem, URem, Shl, Srl, Sra, Rotl, Rotr] {
        tl.set_operation_action(op, MVT::I32, Action::Custom);
    }
    // i1 is a register type here, but its memory is a byte (`lower_bit_load`)
    for op in [Load, Store] {
        tl.set_operation_action(op, MVT::I1, Action::Custom);
    }
    // a word copied between two objects is bytes in turn (`lower_word_copy`)
    tl.set_operation_action(Store, MVT::I16, Action::Custom);
    tl.set_operation_action(BrCond, MVT::Other, Action::Custom);
    tl.set_operation_action(GlobalAddress, MVT::I16, Action::Custom);
    for op in [SetCC, Select] {
        for vt in [MVT::I8, MVT::I16] {
            tl.set_operation_action(op, vt, Action::Custom);
        }
    }
    // a compare of two words is the borrow of the low words into the high
    // words' subtract (`IntegerExpandSetCCOperands`)
    for vt in [MVT::I1, MVT::I8, MVT::I16] {
        tl.set_operation_action(SetCCCarry, vt, Action::Custom);
    }
    // a compare of two longs is one borrow chain over every byte: the node on
    // the halves is expanded into the chain (DEVIATE SC1)
    tl.set_operation_action(SetCCCarry, MVT::I32, Action::Custom);
    // softened floats: compiler-rt's routines
    tl.set_soft_float_libcall_names();
    tl
}

/// The custom lowerings.
pub struct MosLowering {
    /// A product of two words is a widening multiply (not in a function tuned
    /// for size: the two routines add bytes).
    pub widen_products: bool,
}

impl Hooks for MosLowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Rotl | ISD::Rotr => Some(lower_rotate(dag, node)),
            ISD::SetCC => lower_setcc(dag, node),
            ISD::SetCCCarry => lower_setcc_carry(dag, node),
            ISD::Select => Some(lower_select(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            ISD::VAStart => {
                llrm_codegen::backend::selectiondag::target_calling::lower_vastart_frame_index(dag, node, EVT::I16, 1)
            }
            ISD::Load if dag.value_type(node) == EVT::I1 => lower_bit_load(dag, node),
            ISD::Store if dag.value_type(dag.operand(node, 1)) == EVT::I1 => lower_bit_store(dag, node),
            ISD::Store if dag.value_type(dag.operand(node, 1)) == EVT::I16 => Some(lower_word_copy(dag, node)),
            ISD::ZeroExtend | ISD::AnyExtend | ISD::SignExtend => Some(lower_extend(dag, node)),
            ISD::Mul | ISD::SDiv | ISD::UDiv | ISD::SRem | ISD::URem | ISD::Shl | ISD::Srl | ISD::Sra
                if dag.value_type(node) == EVT::I32 =>
            {
                lower_long(dag, node, self.widen_products)
            }
            _ => None,
        }
    }

    /// `makeLibCall` through `LowerCall`: the routine's address is a target
    /// external symbol; a long or a long long travels as words, low first.
    ///
    /// DEVIATE: a shift count goes as a word. Reason: the routines are libgcc's
    /// `__ashldi3 (u64, int)` and `int` is a word here, the pin passes the count
    /// at the target's shift type (a byte). Exit: none.
    fn make_lib_call(
        &mut self,
        dag: &mut SelectionDAG,
        name: &'static str,
        args: &[SDValue],
        ret: EVT,
        signed: bool,
    ) -> Option<SDValue> {
        use super::call::{Ext, In, Out, lower_call};
        let ext = if signed { Ext::Sign } else { Ext::Zero };
        let words = |dag: &mut SelectionDAG, value: SDValue| -> Vec<SDValue> {
            let vt = dag.value_type(value);
            if vt.size_in_bits() <= 16 {
                return vec![value];
            }
            (0..vt.size_in_bits() / 16)
                .map(|at| {
                    let moved = if at == 0 {
                        value
                    } else {
                        let amount = dag.get_constant(u128::from(16 * at), EVT::I8);
                        dag.get_node(ISD::Srl, vt, &[value, amount])
                    };
                    dag.get_node(ISD::Truncate, EVT::I16, &[moved])
                })
                .collect()
        };
        let shift = name.starts_with("__ashl") || name.starts_with("__lshr") || name.starts_with("__ashr");
        let mut outs = Vec::new();
        for (at, &value) in args.iter().enumerate() {
            let value = if shift && at == 1 && dag.value_type(value).size_in_bits() < 16 {
                dag.get_node(ISD::ZeroExtend, EVT::I16, &[value])
            } else {
                value
            };
            let vt = dag.value_type(value);
            let parts = words(dag, value);
            let split = parts.len() > 1;
            outs.extend(parts.into_iter().map(|part| Out {
                value: part,
                ext,
                pointer: false,
                arg_vt: vt,
                split,
                variadic: false,
                sret: false,
                byval: None,
            }));
        }
        let count = if ret.size_in_bits() > 16 { ret.size_in_bits() / 16 } else { 1 };
        let part_vt = if count > 1 { EVT::I16 } else { ret };
        let ins = vec![
            In { vt: part_vt, pointer: false, arg_vt: ret, split: count > 1, sret: false, ext: Ext::Any, byval: None };
            count as usize
        ];
        let callee = dag.get_target_external_symbol(EVT::I16, name);
        let root = dag.entry_node();
        let (_chain, values) = lower_call(dag, root, callee, &outs, &ins).ok()?;
        join_words(dag, &values, ret)
    }
}

/// The words of a long or a long long, low first, as the one value.
fn join_words(
    dag: &mut SelectionDAG,
    words: &[SDValue],
    vt: EVT,
) -> Option<SDValue> {
    match words {
        [one] => Some(*one),
        [lo, hi] => Some(dag.get_node(ISD::BuildPair, vt, &[*lo, *hi])),
        [a, b, c, d] => {
            let half = EVT::integer(vt.size_in_bits() / 2);
            let low = dag.get_node(ISD::BuildPair, half, &[*a, *b]);
            let high = dag.get_node(ISD::BuildPair, half, &[*c, *d]);
            Some(dag.get_node(ISD::BuildPair, vt, &[low, high]))
        }
        _ => None,
    }
}

/// `expandROT`: `rotl x, c` is `(x << c) | (x >> (w - c))`, the amounts taken
/// modulo the width. A constant amount is two shifts by constants (the first is
/// a rotate by nothing when the amount is 0).
fn lower_rotate(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (x, amount) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let width = vt.size_in_bits() as u128;
    let (left, right) = if dag.opcode(node) == ISD::Rotl { (ISD::Shl, ISD::Srl) } else { (ISD::Srl, ISD::Shl) };
    let shift_vt = dag.value_type(amount);
    if let Some(c) = dag.constant(amount) {
        let c = c % width;
        if c == 0 {
            return x;
        }
        let (by, back) = (dag.get_constant(c, shift_vt), dag.get_constant(width - c, shift_vt));
        let (high, low) = (dag.get_node(left, vt, &[x, by]), dag.get_node(right, vt, &[x, back]));
        return dag.get_node(ISD::Or, vt, &[high, low]);
    }
    let mask = dag.get_constant(width - 1, shift_vt);
    let by = dag.get_node(ISD::And, shift_vt, &[amount, mask]);
    let zero = dag.get_constant(0, shift_vt);
    let negated = dag.get_node(ISD::Sub, shift_vt, &[zero, amount]);
    let back = dag.get_node(ISD::And, shift_vt, &[negated, mask]);
    let (high, low) = (dag.get_node(left, vt, &[x, by]), dag.get_node(right, vt, &[x, back]));
    dag.get_node(ISD::Or, vt, &[high, low])
}

/// A long (`i32`) multiply, divide, remainder or shift by a variable:
/// `MOSISD::<op>32` of the operands' halves, two words joined. A shift by a
/// constant is left to the type legalizer.
fn lower_long(
    dag: &mut SelectionDAG,
    node: SDValue,
    widen_products: bool,
) -> Option<SDValue> {
    let opcode = match dag.opcode(node) {
        ISD::Mul => mosisd::MUL32,
        ISD::UDiv => mosisd::UDIV32,
        ISD::SDiv => mosisd::SDIV32,
        ISD::URem => mosisd::UREM32,
        ISD::SRem => mosisd::SREM32,
        ISD::Shl => mosisd::SHL32,
        ISD::Srl => mosisd::SRL32,
        _ => mosisd::SRA32,
    };
    if opcode == mosisd::MUL32
        && widen_products
        && let Some(widened) = lower_widening_multiply(dag, node)
    {
        return Some(widened);
    }
    let shift = matches!(opcode, mosisd::SHL32 | mosisd::SRL32 | mosisd::SRA32);
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    if shift && dag.constant(b).is_some() {
        return None;
    }
    let sixteen = dag.get_constant(16, EVT::I8);
    let mut operands = Vec::new();
    for (at, value) in [a, b].into_iter().enumerate() {
        if shift && at == 1 {
            operands.push(dag.get_node(ISD::Truncate, EVT::I8, &[value]));
            continue;
        }
        let high = dag.get_node(ISD::Srl, EVT::I32, &[value, sixteen]);
        operands.push(dag.get_node(ISD::Truncate, EVT::I16, &[value]));
        operands.push(dag.get_node(ISD::Truncate, EVT::I16, &[high]));
    }
    let pair = dag.get_node_multi(opcode, &[EVT::I16, EVT::I16], &operands);
    Some(dag.get_node(
        ISD::BuildPair,
        EVT::I32,
        &[SDValue { node: pair.node, res: 0 }, SDValue { node: pair.node, res: 1 }],
    ))
}

/// Whether a 32-bit value is a sign extension of its low word: 17 sign bits,
/// or a zero extension that the IR says is not negative (`zext nneg`, which
/// leaves the word's top bit zero).
fn fits_signed_word(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    num_sign_bits(dag, value) >= 17
        || (dag.opcode(value) == ISD::ZeroExtend
            && dag.node(value).flags.has(SDNodeFlags::NON_NEG)
            && masked_value_is_zero(dag, value, 0xFFFF_0000))
}

/// A product of two operands that each fit in 16 bits is one 16 x 16 -> 32
/// multiply of their low words, whatever 32-bit multiplier they went through:
/// GCC's widening multiply (`convert_mult_to_widen`,
/// <https://github.com/gcc-mirror/gcc/blob/releases/gcc-15.2.0/gcc/tree-ssa-math-opts.cc>,
/// the `umulhisi3`/`mulhisi3` optabs) and LLVM's `SMUL_LOHI`/`UMUL_LOHI` of
/// the halves. Unsigned when both upper words are known zero, signed when both
/// have 17 sign bits; a mix of the two is a long multiply.
fn lower_widening_multiply(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    // a constant factor is the combiner's to shift and add, or a call kept for size
    if dag.constant(a).is_some() || dag.constant(b).is_some() {
        return None;
    }
    // the factor with a zero high byte walks the fewer bits: it is the multiplier
    let narrow = |dag: &SelectionDAG, v| masked_value_is_zero(dag, v, 0xFF00);
    let (a, b) = if narrow(dag, a) && !narrow(dag, b) { (b, a) } else { (a, b) };
    let upper = 0xFFFF_0000;
    let opcode = if masked_value_is_zero(dag, a, upper) && masked_value_is_zero(dag, b, upper) {
        mosisd::UMULW16
    } else if fits_signed_word(dag, a) && fits_signed_word(dag, b) {
        mosisd::SMULW16
    } else {
        return None;
    };
    let low = |dag: &mut SelectionDAG, value| dag.get_node(ISD::Truncate, EVT::I16, &[value]);
    let operands = [low(dag, a), low(dag, b)];
    let pair = dag.get_node_multi(opcode, &[EVT::I16, EVT::I16], &operands);
    Some(dag.get_node(
        ISD::BuildPair,
        EVT::I32,
        &[SDValue { node: pair.node, res: 0 }, SDValue { node: pair.node, res: 1 }],
    ))
}

/// A bit in memory is a byte with the upper bits zero, as the pin's type
/// legalizer makes it where `i1` is not a register type (`PromoteIntRes_LOAD`,
/// `LegalizeDAG`'s truncating store of a non-byte width). Left alone, a loaded
/// bit that a `select` tests was selected into a compare of its own zero
/// extension, forever. An extending load from it is promoted to a byte load by
/// the legalizer (`promote_sub_byte_ext_load`).
fn lower_bit_load(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Mem(info) = dag.node(node).data else { return None };
    if info.ext.is_some() {
        return None;
    }
    let (chain, ptr) = (dag.operand(node, 0), dag.operand(node, 1));
    let byte = dag.get_load(EVT::I8, chain, ptr, MemInfo { mem_vt: EVT::I8, ..info });
    let one = dag.get_constant(1, EVT::I8);
    let masked = dag.get_node(ISD::And, EVT::I8, &[byte, one]);
    let bit = dag.get_node(ISD::Truncate, EVT::I1, &[masked]);
    Some(dag.get_node_multi(ISD::MergeValues, &[EVT::I1, EVT::OTHER], &[bit, SDValue { node: byte.node, res: 1 }]))
}

/// Whether two accesses are to different known objects.
fn distinct_objects(
    a: MemBase,
    b: MemBase,
) -> bool {
    matches!((a, b), (MemBase::Global(_) | MemBase::Stack(_), MemBase::Global(_) | MemBase::Stack(_))) && a != b
}

/// A word loaded only to be stored to another object is copied a byte at a
/// time: low byte loaded and stored, then the high. Both loads first would
/// hold the low byte in a register across the high load (`tax`/`txa`); the
/// stores cannot touch what the second load reads when the objects differ.
/// Any other word store is left as it is, a word.
///
/// DEVIATE 6502-COPY-PAIRS (owner w9, exit 2026-12-05): the pin legalizes
/// i16 to bytes, which gives this and costs the word forms the selector folds
/// into an add or a compare. Exit: those folds match byte pairs.
fn lower_word_copy(
    dag: &mut SelectionDAG,
    store: SDValue,
) -> SDValue {
    let (chain, value, to) = (dag.operand(store, 0), dag.operand(store, 1), dag.operand(store, 2));
    let (NodeData::Mem(out), NodeData::Mem(inn)) = (dag.node(store).data, dag.node(value).data) else { return store };
    if dag.opcode(value) != ISD::Load
        || value.res != 0
        || inn.mem_vt != EVT::I16
        || out.mem_vt != EVT::I16
        || inn.ext.is_some()
        || inn.volatile
        || out.volatile
        || inn.ordering.is_some()
        || out.ordering.is_some()
        || dag.users(value).any(|user| user != store.node)
        || !distinct_objects(inn.base, out.base)
    {
        return store;
    }
    let (load_chain, from) = (dag.operand(value, 0), dag.operand(value, 1));
    let (load_info, store_info) = (MemInfo { mem_vt: EVT::I8, ..inn }, MemInfo { mem_vt: EVT::I8, ..out });
    let (high_from, high_to) = (dag.get_object_ptr_offset(from, 1), dag.get_object_ptr_offset(to, 1));
    let low = dag.get_load(EVT::I8, load_chain, from, load_info);
    let low_chain = SDValue { node: low.node, res: 1 };
    let before_store = if chain == (SDValue { node: value.node, res: 1 }) { low_chain } else { dag.get_token_factor(&[low_chain, chain]) };
    let low_store = dag.get_store(before_store, low, to, store_info);
    let high = dag.get_load(EVT::I8, low_store, high_from, load_info);
    let high_store = dag.get_store(SDValue { node: high.node, res: 1 }, high, high_to, store_info);
    // whatever waited for the word load waits for the whole copy
    dag.replace_all_uses_with(SDValue { node: value.node, res: 1 }, high_store);
    high_store
}

/// The store of a bit: the byte of 0 or 1 (`lower_bit_load`).
fn lower_bit_store(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Mem(info) = dag.node(node).data else { return None };
    let (chain, value, ptr) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let byte = condition_byte(dag, value);
    Some(dag.get_store(chain, byte, ptr, MemInfo { mem_vt: EVT::I8, ..info }))
}

/// A condition widened to a byte or a word is `select cond, 1, 0`; any other
/// extension is selected as it is.
fn lower_extend(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let condition = dag.operand(node, 0);
    if dag.value_type(condition) != EVT::I1 {
        return node;
    }
    let condition = if dag.opcode(condition) == ISD::SetCCCarry {
        lower_setcc_carry(dag, condition).unwrap_or(condition)
    } else {
        condition
    };
    // the flag of an overflow op is not a condition yet: its expansion makes
    // one, and the extension is lowered when that is done (`condition_byte` has
    // no byte for it and would widen the flag with this very node)
    if matches!(dag.opcode(condition), ISD::SAddO | ISD::SSubO | ISD::SMulO | ISD::UMulO) {
        return node;
    }
    let vt = dag.value_type(node);
    // the carry an add or subtract leaves is `0 + 0 + C`, not a select on it
    if dag.opcode(node) != ISD::SignExtend
        && condition.res == 1
        && matches!(
            dag.opcode(condition),
            ISD::UAddO | ISD::UAddOCarry | ISD::USubO | ISD::USubOCarry
        )
    {
        let byte = condition_byte(dag, condition);
        return if vt == EVT::I8 { byte } else { dag.get_node(ISD::ZeroExtend, vt, &[byte]) };
    }
    // a logic op over compares is a byte of 0 or 1 already (`condition_byte`):
    // widened, it is that byte, not a select that tests the byte it was
    // just made into
    if dag.opcode(node) != ISD::SignExtend
        && matches!(dag.opcode(condition), ISD::And | ISD::Or | ISD::Xor)
        && is_condition_tree(dag, condition)
    {
        let byte = condition_byte(dag, condition);
        return if vt == EVT::I8 { byte } else { dag.get_node(ISD::ZeroExtend, vt, &[byte]) };
    }
    // `zext (trunc x)` is `x` resized where the truncate drops only zero bits
    // (`isTruncateOf` in `visitZERO_EXTEND`,
    // <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L16421>):
    // the bit `lower_setcc_carry` leaves is a byte of 0 or 1 already. The
    // combiner has this fold, but the truncate is made here, after it ran.
    if dag.opcode(node) != ISD::SignExtend && dag.opcode(condition) == ISD::Truncate {
        let byte = dag.operand(condition, 0);
        let above = u128::MAX >> (128 - dag.value_type(byte).size_in_bits()) & !1;
        if masked_value_is_zero(dag, byte, above) {
            return dag.get_zext_or_trunc(byte, vt);
        }
    }
    // a sign-extended bit is all ones (`select c, 1, 2` reaches here as
    // `sext(c) + 2`)
    let width = vt.size_in_bits();
    let set = if dag.opcode(node) == ISD::SignExtend { (1u128 << width) - 1 } else { 1 };
    let (one, zero) = (dag.get_constant(set, vt), dag.get_constant(0, vt));
    let select = dag.get_node(ISD::Select, vt, &[condition, one, zero]);
    lower_select(dag, select)
}

/// `LowerGlobalAddress`: `WRAPPER` of the address, an immediate the linker
/// fills in.
fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Global { symbol, offset } = dag.node(node).data else { return None };
    let address = dag.get_target_global_address(EVT::I16, symbol, offset);
    Some(dag.get_node(mosisd::WRAPPER, EVT::I16, &[address]))
}

fn cc_operand(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant(code, EVT::I8)
}

fn cond_of(
    dag: &SelectionDAG,
    setcc: SDValue,
) -> CondCode {
    match dag.node(setcc).data {
        NodeData::CondCode(cc) => cc,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

/// `x ^ sign bit`: so a signed compare is an unsigned one. A constant folds.
fn flip_sign(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> SDValue {
    let vt = dag.value_type(value);
    let sign = 1u128 << (vt.size_in_bits() - 1);
    match dag.constant(value) {
        Some(bits) => dag.get_constant(bits ^ sign, vt),
        None => {
            let sign = dag.get_constant(sign, vt);
            dag.get_node(ISD::Xor, vt, &[value, sign])
        }
    }
}

/// Equality is symmetric: a value computed in this block goes on the left (it
/// is in `A` when the compare comes), an incoming register (an argument, a
/// value live in from another block) on the right, where `cmp` reads it
/// from memory. InstCombine orders a commutative operation's operands the same
/// way (arguments before instructions in its complexity order), but LSR
/// rewrites a loop's exit test with the limit on the left.
fn equality_operands(
    dag: &SelectionDAG,
    op0: SDValue,
    op1: SDValue,
) -> (SDValue, SDValue) {
    let incoming = |value: SDValue| dag.opcode(value) == ISD::CopyFromReg;
    let loaded = |value: SDValue| dag.opcode(value) == ISD::Load;
    if incoming(op0) && !incoming(op1) && !loaded(op1) && dag.constant(op1).is_none() { (op1, op0) } else { (op0, op1) }
}

/// A load nothing else reads, not extending, not volatile: `cmp g` and
/// `cmp g,x` take it from memory.
fn foldable_load(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.opcode(value) == ISD::Load
        && value.res == 0
        && matches!(dag.node(value).data, NodeData::Mem(info) if info.ext.is_none() && !info.volatile && info.ordering.is_none())
        && dag.has_one_use(value.node)
}

/// The code a comparison of `op0` with `op1` branches on, and the operands to
/// compare (possibly swapped or sign-flipped).
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    op0: SDValue,
    op1: SDValue,
) -> (u128, SDValue, SDValue) {
    use CondCode::*;
    // `TranslateX86CC`: a foldable load on the left and none on the right
    // flips the condition, so the load is the memory operand of the compare
    let (cc, op0, op1) =
        if foldable_load(dag, op0) && !foldable_load(dag, op1) && dag.constant(op1).is_none() {
            (cc.swapped_operands(), op1, op0)
        } else {
            (cc, op0, op1)
        };
    let against_zero = dag.is_null_constant(op1);
    // `x > C` is `x >= C + 1`: the constant stays the right-hand side, an
    // immediate of the compare (AArch64's `getAArch64Cmp` adjusts the
    // immediate the same way)
    let width = dag.value_type(op0).size_in_bits();
    if let (SetUGt | SetULe | SetGt | SetLe, Some(c)) = (cc, dag.constant(op1)) {
        let top = match cc {
            SetUGt | SetULe => (1u128 << width) - 1,
            _ => (1u128 << (width - 1)) - 1,
        };
        if c != top {
            let next = dag.get_constant((c + 1) & ((1u128 << width) - 1), dag.value_type(op0));
            let wider = if matches!(cc, SetUGt | SetGt) {
                if cc == SetUGt { SetUGe } else { SetGe }
            } else if cc == SetULe {
                SetULt
            } else {
                SetLt
            };
            return translate_cc(dag, wider, op0, next);
        }
    }
    match cc {
        SetEq | SetNe => {
            let (a, b) = equality_operands(dag, op0, op1);
            (if cc == SetEq { cond::EQ } else { cond::NE }, a, b)
        }
        SetULt => (cond::CC, op0, op1),
        SetUGe => (cond::CS, op0, op1),
        SetUGt => (cond::CC, op1, op0),
        SetULe => (cond::CS, op1, op0),
        SetLt if against_zero => (cond::MI, op0, op1),
        SetGe if against_zero => (cond::PL, op0, op1),
        SetLt | SetGe => {
            let (a, b) = (flip_sign(dag, op0), flip_sign(dag, op1));
            (if cc == SetLt { cond::CC } else { cond::CS }, a, b)
        }
        SetGt | SetLe => {
            // a > b is b < a, a <= b is b >= a
            let (a, b) = (flip_sign(dag, op1), flip_sign(dag, op0));
            (if cc == SetGt { cond::CC } else { cond::CS }, a, b)
        }
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// `LowerSETCC`, integer part: `SETCC cc, (CMP a, b)`.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (mut op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let mut cc = cond_of(dag, dag.operand(node, 2));
    if dag.value_type(op0) == EVT::I1 {
        // Compare the bits as bytes of 0 or 1. A signed order is the unsigned
        // one reversed, since a set bit is -1.
        (op0, op1) = (condition_byte(dag, op0), condition_byte(dag, op1));
        cc = match cc {
            CondCode::SetLt => CondCode::SetUGt,
            CondCode::SetGt => CondCode::SetULt,
            CondCode::SetLe => CondCode::SetUGe,
            CondCode::SetGe => CondCode::SetULe,
            other => other,
        };
    }
    if !matches!(dag.value_type(op0), EVT::I8 | EVT::I16) {
        return None;
    }
    let (code, a, b) = translate_cc(dag, cc, op0, op1);
    // Equality on a word needs Z over both bytes.
    let compare =
        if dag.value_type(a) == EVT::I16 && matches!(code, cond::EQ | cond::NE) { mosisd::CMPZ } else { mosisd::CMP };
    let flags = dag.get_node(compare, EVT::I8, &[a, b]);
    let code = cc_operand(dag, code);
    Some(dag.get_node(mosisd::SETCC, EVT::I8, &[code, flags]))
}

/// `x >= 0`, `x < 0` and `-1 >= x` (`x <= -1`) of a long are the sign of its
/// high word: the pin's `IntegerExpandSetCCOperands` takes that for `x < 0`
/// and `x > -1` where the compare reaches it so, and the others reach the
/// chain as a compare against a constant of zero or of all ones (`x < 0` too,
/// when a combine has made `x >= 0` its inverse before the expansion; the
/// branch then tests the inverse). The word's compare, as a `SETCC` of the
/// high word.
fn sign_compare(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (high, other, borrow) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let cc = cond_of(dag, dag.operand(node, 3));
    if dag.opcode(borrow) != ISD::USubO || borrow.res != 1 || !matches!(cc, CondCode::SetGe | CondCode::SetLt) {
        return None;
    }
    let (low, below) = (dag.operand(borrow, 0), dag.operand(borrow, 1));
    let zero = dag.get_constant(0, EVT::I16);
    if dag.constant(other) == Some(0) && dag.constant(below) == Some(0) {
        // x >= 0, x < 0
        let sign = dag.get_cond_code(cc);
        return Some(dag.get_node(ISD::SetCC, EVT::I1, &[high, zero, sign]));
    }
    if cc == CondCode::SetGe && dag.constant(high) == Some(0xFFFF) && dag.constant(low) == Some(0xFFFF) {
        // -1 >= x
        let lt = dag.get_cond_code(CondCode::SetLt);
        return Some(dag.get_node(ISD::SetCC, EVT::I1, &[other, zero, lt]));
    }
    None
}

/// `LowerSETCCCARRY`: the high words' subtract takes the borrow of the low
/// words', and the order is the borrow it leaves. A signed order compares the
/// high words with their sign bits flipped.
///
/// The pin's `IntegerExpandSetCCOperands` makes the node when the target has
/// it ([LegalizeIntegerTypes.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp));
/// ours is `legalize_types.rs`. Without it a compare of two words was a select
/// of the compares of its halves, each a chain.
fn lower_setcc_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (high, other, borrow) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.value_type(high) != EVT::I16
        || !matches!(dag.opcode(borrow), ISD::USubO | ISD::USubOCarry)
        || borrow.res != 1
    {
        return None;
    }
    if let Some(sign) = sign_compare(dag, node) {
        let vt = dag.value_type(node);
        return Some(if vt == EVT::I1 { sign } else { dag.get_node(ISD::ZeroExtend, vt, &[sign]) });
    }
    let cc = cond_of(dag, dag.operand(node, 3));
    let (high, other) = match cc {
        CondCode::SetULt | CondCode::SetUGe => (high, other),
        CondCode::SetLt | CondCode::SetGe => (flip_sign(dag, high), flip_sign(dag, other)),
        _ => return None,
    };
    // the flags are bits, as a subtract of words leaves them: `condition` reads
    // the carry of the node itself, not of a truncation of it
    let (low, below) = (dag.operand(borrow, 0), dag.operand(borrow, 1));
    let low_borrow = if dag.opcode(borrow) == ISD::USubOCarry {
        // the chain's earlier borrow stays the bit it was
        let earlier = dag.operand(borrow, 2);
        dag.get_node_multi(ISD::USubOCarry, &[EVT::I16, EVT::I1], &[low, below, earlier])
    } else {
        dag.get_node_multi(ISD::USubO, &[EVT::I16, EVT::I1], &[low, below])
    };
    let borrow = SDValue { node: low_borrow.node, res: 1 };
    let pair = dag.get_node_multi(ISD::USubOCarry, &[EVT::I16, EVT::I1], &[high, other, borrow]);
    let below = SDValue { node: pair.node, res: 1 };
    let vt = dag.value_type(node);
    match cc {
        CondCode::SetULt | CondCode::SetLt if vt == EVT::I1 => Some(below),
        CondCode::SetULt | CondCode::SetLt => Some(dag.get_node(ISD::ZeroExtend, vt, &[below])),
        _ => {
            // "not below" is the borrow's byte flipped: bytes only, no `xor` of
            // the bit, whose select tests itself
            let one = dag.get_constant(1, EVT::I8);
            let byte = carry_byte(dag, below);
            let byte = dag.get_node(ISD::Xor, EVT::I8, &[byte, one]);
            Some(if vt == EVT::I8 { byte } else { dag.get_node(ISD::Truncate, vt, &[byte]) })
        }
    }
}

/// `lower_setcc_carry` as the flags of the whole subtract: `CMP32` of the low
/// words and of the high words (their sign bits flipped for a signed order),
/// read with `CC` for below and `CS` for not below.
fn lower_setcc_carry_flags(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<(SDValue, SDValue)> {
    let (high, other, borrow) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.value_type(high) != EVT::I16 || dag.opcode(borrow) != ISD::USubO || borrow.res != 1 {
        return None;
    }
    if let Some(sign) = sign_compare(dag, node) {
        let lowered = lower_setcc(dag, sign)?;
        return Some((dag.operand(lowered, 0), dag.operand(lowered, 1)));
    }
    // Flags are made again where they are read: a chain that two orders share
    // (`a < b` and `a >= b`) is made once as a borrow and kept in a byte.
    if !dag.has_one_use(borrow.node) {
        return None;
    }
    let cc = cond_of(dag, dag.operand(node, 3));
    let (high, other) = match cc {
        CondCode::SetULt | CondCode::SetUGe => (high, other),
        CondCode::SetLt | CondCode::SetGe => (flip_sign(dag, high), flip_sign(dag, other)),
        _ => return None,
    };
    let (low, below) = (dag.operand(borrow, 0), dag.operand(borrow, 1));
    let flags = dag.get_node(mosisd::CMP32, EVT::I8, &[low, below, high, other]);
    let code = if matches!(cc, CondCode::SetULt | CondCode::SetLt) { cond::CC } else { cond::CS };
    Some((cc_operand(dag, code), flags))
}

/// The code and flags a condition value branches on: a lowered compare, or the
/// value tested against zero (`NE`).
fn condition(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> (SDValue, SDValue) {
    // A branch or a select on the order of two longs reads the flags the
    // chain leaves: `bcc` after the last `sbc`, not the borrow made into a byte
    // and tested again.
    if dag.opcode(value) == ISD::SetCCCarry
        && let Some(flags) = lower_setcc_carry_flags(dag, value)
    {
        return flags;
    }
    let value = match dag.opcode(value) {
        ISD::SetCC => lower_setcc(dag, value).unwrap_or(value),
        ISD::SetCCCarry => lower_setcc_carry(dag, value).unwrap_or(value),
        _ => value,
    };
    if dag.opcode(value) == mosisd::SETCC {
        return (dag.operand(value, 0), dag.operand(value, 1));
    }
    let byte = condition_byte(dag, value);
    let zero = dag.get_constant(0, EVT::I8);
    let flags = dag.get_node(mosisd::CMP, EVT::I8, &[byte, zero]);
    (cc_operand(dag, cond::NE), flags)
}

/// Whether `condition_byte` of `value` is made of lowered compares and
/// constants alone, with no fallback zero extension (which would come back
/// here).
fn is_condition_tree(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    if dag.value_type(value) != EVT::I1 {
        return false;
    }
    let operand = |i: usize| dag.operand(value, i);
    let tree = |i: usize| is_condition_tree(dag, operand(i));
    match dag.opcode(value) {
        ISD::Constant => true,
        ISD::SetCC => matches!(dag.value_type(operand(0)), EVT::I8 | EVT::I16) || (tree(0) && tree(1)),
        ISD::Truncate => matches!(dag.value_type(operand(0)), EVT::I8 | EVT::I16),
        ISD::Freeze => tree(0),
        ISD::Select => tree(1) && tree(2),
        ISD::And | ISD::Or | ISD::Xor => tree(0) && tree(1),
        _ => false,
    }
}

/// A condition value as a byte of 0 or 1. A logic op over conditions stays
/// in bytes: zero-extending it legalizes back into a `select` on itself, whose
/// lowering is `CMOV` on a compare of that `select`.
fn condition_byte(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> SDValue {
    match dag.value_type(value) {
        EVT::I1 => match dag.opcode(value) {
            ISD::Constant => {
                let bit = dag.constant(value).expect("a constant") & 1;
                dag.get_constant(bit, EVT::I8)
            }
            ISD::SetCC => match lower_setcc(dag, value) {
                Some(lowered) => lowered,
                None => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
            },
            ISD::SetCCCarry => match lower_setcc_carry(dag, value) {
                Some(lowered) => condition_byte(dag, lowered),
                None => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
            },
            // a multi-result lowering's wrapper (`lower_bit_load`): the bit it
            // holds
            ISD::MergeValues => {
                let bit = dag.operand(value, value.res as usize);
                condition_byte(dag, bit)
            }
            ISD::Truncate if matches!(dag.value_type(dag.operand(value, 0)), EVT::I8 | EVT::I16) => {
                let wide = dag.operand(value, 0);
                let byte =
                    if dag.value_type(wide) == EVT::I8 { wide } else { dag.get_node(ISD::Truncate, EVT::I8, &[wide]) };
                let one = dag.get_constant(1, EVT::I8);
                dag.get_node(ISD::And, EVT::I8, &[byte, one])
            }
            ISD::Freeze => {
                let bit = dag.operand(value, 0);
                condition_byte(dag, bit)
            }
            // the carry an add or subtract leaves in C: `0 + 0 + C`, and for
            // a borrow `0 - 0 - C` (all ones) masked to the bit
            ISD::UAddO | ISD::UAddOCarry | ISD::USubO | ISD::USubOCarry if value.res == 1 => carry_byte(dag, value),
            ISD::Select => {
                let (test, a, b) = (dag.operand(value, 0), dag.operand(value, 1), dag.operand(value, 2));
                let (a, b) = (condition_byte(dag, a), condition_byte(dag, b));
                dag.get_node(ISD::Select, EVT::I8, &[test, a, b])
            }
            op @ (ISD::And | ISD::Or | ISD::Xor) => {
                let (a, b) = (dag.operand(value, 0), dag.operand(value, 1));
                let (a, b) = (condition_byte(dag, a), condition_byte(dag, b));
                dag.get_node(op, EVT::I8, &[a, b])
            }
            _ => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
        },
        // the boolean of a saturating subtract's expansion is a byte
        // (`getSetCCResultType`): the same borrow, which read as C is inverted
        EVT::I8 if value.res == 1 && matches!(dag.opcode(value), ISD::USubO | ISD::USubOCarry) => {
            carry_byte(dag, value)
        }
        _ => value,
    }
}

/// The carry an add or subtract leaves in C as a byte of 0 or 1: `0 + 0 + C`,
/// and for a borrow `0 - 0 - C` (all ones) masked to the bit.
fn carry_byte(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> SDValue {
    let zero = dag.get_constant(0, EVT::I8);
    let adds = matches!(dag.opcode(value), ISD::UAddO | ISD::UAddOCarry);
    let widen = if adds { ISD::UAddOCarry } else { ISD::USubOCarry };
    let pair = dag.get_node_multi(widen, &[EVT::I8, EVT::I1], &[zero, zero, value]);
    let byte = SDValue { node: pair.node, res: 0 };
    let one = dag.get_constant(1, EVT::I8);
    if adds { byte } else { dag.get_node(ISD::And, EVT::I8, &[byte, one]) }
}

/// `LowerSELECT`: `CMOV false, true, cc, flags`.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (test, on_true, on_false) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    let vt = dag.value_type(node);
    dag.get_node(mosisd::CMOV, vt, &[on_false, on_true, code, flags])
}

/// `LowerBRCOND`: `BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, test, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    dag.get_node(mosisd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
