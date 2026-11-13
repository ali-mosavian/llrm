//! The Z80's action table: what the DAG legalizers do with each operation.
//!
//! `i8` and `i16` are register types (`GR8`, `GR16`). Byte add, subtract and
//! logic work on A, word add and subtract on HL; word logic is the two bytes'
//! ops joined. Shifts by a constant are instructions. There is no multiply,
//! divide, shift by a variable or `i32` instruction yet: those are refused by
//! the legalizer, not papered over.
//!
//! A comparison is `Z80ISD::CMP a, b`: the `i8` flags of `a - b`. `BRCOND`
//! reads a condition code (a constant, [`cond`]) and those flags. Only the
//! flags a `jp cc` tests are codes: Z (`EQ`, `NE`) and C (`CC`, `CS`). An
//! unsigned greater-than swaps the operands; a signed compare flips both sign
//! bits and compares unsigned. A word compare is `CMP` too: `sbc hl,rr` sets Z
//! over all sixteen bits and C as the borrow.

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::known_bits::masked_value_is_zero;
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::{TargetLowering, TargetLoweringInfo};

use super::generated::ids::isd as ids;

/// The Z80's own nodes, numbered by `own_ids.txt`.
pub mod z80isd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const CMPC: ISD = ISD::Target(ids::CMPC);
    pub const CMPS: ISD = ISD::Target(ids::CMPS);
    pub const ADDF: ISD = ISD::Target(ids::ADDF);
    pub const SUBF: ISD = ISD::Target(ids::SUBF);
    pub const ADCF: ISD = ISD::Target(ids::ADCF);
    pub const SBCF: ISD = ISD::Target(ids::SBCF);
    pub const WRAPPER: ISD = ISD::Target(ids::WRAPPER);
    pub const SUBZX: ISD = ISD::Target(ids::SUBZX);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const TSTS: ISD = ISD::Target(ids::TSTS);
    pub const BIT: ISD = ISD::Target(ids::BIT);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
}

/// Condition codes; a code and its opposite differ in bit 0.
pub mod cond {
    pub const NZ: u128 = 0;
    pub const Z: u128 = 1;
    pub const NC: u128 = 2;
    pub const C: u128 = 3;
}

/// `speed_routines`: the long divides and the variable 64-bit shifts keep the
/// speed routines in a function tuned for size too (`pipeline::SPEED_ROUTINES`,
/// the second selection `compile::decomposed_functions` prices).
pub fn target_lowering(speed_routines: bool) -> TargetLowering {
    use ISD::*;
    let mut tl = TargetLowering::empty(
        TargetLoweringInfo::new(&[MVT::I1, MVT::I8, MVT::I16], false).with_soft_float().with_narrow_library_shift(),
    );
    tl.init_actions();
    // SD241: the shift routine stays for a variable amount known small until an
    // inline policy is priced
    tl.set_libcall_for_small_variable_shift();
    for vt in [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        for ext in [LoadExt::SExtLoad, LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, MVT::I1, Action::Promote);
        }
    }
    // A sign-extending load is a load and a sign extension; a zero- or
    // any-extending one to a type that is not a register is not formed, to
    // be split again.
    for (vt, mem) in [(MVT::I16, MVT::I8), (MVT::I32, MVT::I8), (MVT::I32, MVT::I16), (MVT::I64, MVT::I8)]
        .into_iter()
        .chain([(MVT::I64, MVT::I16), (MVT::I64, MVT::I32)])
    {
        tl.set_load_ext_action(LoadExt::SExtLoad, vt, mem, Action::Expand);
    }
    for (vt, mem) in
        [(MVT::I32, MVT::I8), (MVT::I32, MVT::I16), (MVT::I64, MVT::I8), (MVT::I64, MVT::I16), (MVT::I64, MVT::I32)]
    {
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
    let mut set = |ops: &[ISD], vts: &[MVT], action: Action| {
        for &op in ops {
            for &vt in vts {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[Add, Sub, And, Or, Xor], &[MVT::I8], Action::Legal);
    set(&[Add, And, Or, Xor], &[MVT::I16], Action::Legal);
    // the difference of two bytes known to be widened (`lower_sub`)
    set(&[Sub], &[MVT::I16], Action::Custom);
    // The words of a long add or subtract: the carry is the flags of `add hl,
    // rr` and `adc hl, rr` (`lower_carry_op`), where the pin's
    // `ExpandIntRes_ADDSUB` keys on `UADDO_CARRY`/`USUBO_CARRY` being legal.
    set(&[UAddO, USubO, UAddOCarry, USubOCarry], &[MVT::I16], Action::Custom);
    // A shift by a constant is an instruction; by a variable it is a call
    // (`lower_shift`).
    set(&[Shl, Srl, Sra], &[MVT::I8, MVT::I16], Action::Custom);
    // `RTLIB::MUL_I16` and the others: libgcc's names (`RuntimeLibcalls.def`),
    // with the same family for bytes, which this runtime has because a byte
    // loop is half the word's.
    set(&[Mul, SDiv, UDiv, SRem, URem], &[MVT::I8, MVT::I16], Action::LibCall);
    set(&[ShlParts, SrlParts, SraParts], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[MulHS, MulHU, UMulLoHi, SMulLoHi], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[SDivRem, UDivRem], &[MVT::I8], Action::Expand);
    set(&[CtTz, CtLz, CtPop, BSwap], &[MVT::I8, MVT::I16], Action::Expand);
    // no rotate or funnel shift instruction: shifts and an or (`expandROT`,
    // `expandFunnelShift`)
    set(&[Rotl, Rotr], &[MVT::I8, MVT::I16], Action::Expand);
    set(&[FShl, FShr], &[MVT::I8], Action::Expand);
    // `MatchRotate` makes `fshl` of the two shifts of a long shifted by one; a
    // word funnel by 15 left or 1 right is `FSHR1`, the other amounts are
    // the shifts and an or again
    set(&[FShl, FShr], &[MVT::I16], Action::Custom);
    // the action is by the type extended from (`LegalizeDAG`): a word from a
    // byte is the byte sign-extended (`lower_sign_extend_inreg`)
    set(&[SignExtendInReg], &[MVT::I1, MVT::I16], Action::Expand);
    set(&[SignExtendInReg], &[MVT::I8], Action::Custom);
    set(&[Undef, Poison], &[MVT::I1, MVT::I8, MVT::I16], Action::Expand);
    set(&[ZeroExtend, AnyExtend, SignExtend], &[MVT::I8, MVT::I16], Action::Custom);
    set(&[SetCC, Select], &[MVT::I8, MVT::I16], Action::Custom);
    // a long's order is the chain of its words' compares (`lower_setcc_carry`)
    set(&[SetCCCarry], &[MVT::I1, MVT::I8, MVT::I16], Action::Custom);
    // i1 is a register type here, but its memory is a byte (`lower_bit_load`)
    set(&[Load, Store], &[MVT::I1], Action::Custom);
    // `va_start` stores the address of the first unnamed argument
    // (`lower_vastart_frame_index`); the rest of a `va_list` is a pointer
    // the generic expansions walk
    set(&[VAStart], &[MVT::Other], Action::Custom);
    set(&[VAEnd, VACopy], &[MVT::Other], Action::Expand);
    for (vt, names) in [
        (MVT::I8, ["__mulqi3", "__divqi3", "__udivqi3", "__modqi3", "__umodqi3"]),
        (MVT::I16, ["__mulhi3", "__divhi3", "__udivhi3", "__modhi3", "__umodhi3"]),
    ] {
        for (op, name) in [Mul, SDiv, UDiv, SRem, URem].into_iter().zip(names) {
            tl.set_libcall_name(op, vt, name);
        }
    }
    // the product of two zero-extended words is one call in a function tuned
    // for size, the whole 32-bit result in `hl` (low) and `de` (`expand_mul`):
    // 15 B for the product against 216 B of partial products. At -O2 the
    // partial products ran faster in div_constants_short and
    // compare_to_int64.
    tl.set_size_libcall_name(UMulLoHi, MVT::I16, "__umulhisi3");
    // the longs are expanded to words; a divide or a variable shift of one is
    // the call, not a word-wise expansion
    for (vt, names) in [
        (MVT::I32, ["__divsi3", "__udivsi3", "__modsi3", "__umodsi3", "__ashlsi3", "__lshrsi3", "__ashrsi3"]),
        (MVT::I64, ["__divdi3", "__udivdi3", "__moddi3", "__umoddi3", "__ashldi3", "__lshrdi3", "__ashrdi3"]),
    ] {
        for (op, name) in [SDiv, UDiv, SRem, URem, Shl, Srl, Sra].into_iter().zip(names) {
            tl.set_libcall_name(op, vt, name);
        }
    }
    // a long shifted by a constant is a call in a function tuned for size once
    // the bytes still shifted times the bits beyond whole bytes reach 10: each
    // is a 2 B shift instruction, the call is 5 B and the routine 15 once
    // (x << 5 is 60 B inline, x >> 29 only 17). The 64-bit shifts reach it
    // through their 32-bit halves.
    for (op, name) in [Shl, Srl, Sra].into_iter().zip(["__ashlsi3", "__lshrsi3", "__ashrsi3"]) {
        tl.set_size_libcall_name(op, MVT::I32, name);
    }
    // the 64-bit shifts have a smaller routine for a function tuned for size (a
    // bit at a time: 101 B against 189)
    for (op, name) in [Shl, Srl, Sra].into_iter().zip(["__s_ashldi3", "__s_lshrdi3", "__s_ashrdi3"]) {
        if !speed_routines {
            tl.set_size_variable_libcall_name(op, MVT::I64, name);
        }
    }
    tl.set_size_shift_call(8, 10);
    // the long divide has a smaller routine for a function tuned for size
    // (one step per dividend bit, registers alone: 76 B against 391)
    for (op, name) in
        [SDiv, UDiv, SRem, URem].into_iter().zip(["__s_divsi3", "__s_udivsi3", "__s_modsi3", "__s_umodsi3"])
    {
        if !speed_routines {
            tl.set_size_libcall_name(op, MVT::I32, name);
        }
    }
    // a word divide and remainder of the same operands are one call too
    // (`lower_divrem_word`)
    for (op, name) in [(SDivRem, "__divremhi4"), (UDivRem, "__udivhi3")] {
        tl.set_operation_action(op, MVT::I16, Action::Custom);
        tl.set_libcall_name(op, MVT::I16, name);
    }
    // a divide and a remainder of the same long operands are one call, the
    // remainder stored through an address (`ExpandDivRemLibCall`; the pin's
    // `useDivRem` pairs them where the type is Custom, `lower_divrem`); a long
    // long's pair comes back in one cell of sixteen bytes
    for (vt, names) in [
        (MVT::I32, [("__divremsi4", "__s_divremsi4"), ("__udivremsi4", "__s_udivremsi4")]),
        (MVT::I64, [("__divmddi4", "__s_divmddi4"), ("__udivmddi4", "__s_udivmddi4")]),
    ] {
        for (op, (name, small)) in [SDivRem, UDivRem].into_iter().zip(names) {
            tl.set_operation_action(op, vt, Action::Custom);
            tl.set_libcall_name(op, vt, name);
            if !speed_routines {
                tl.set_size_libcall_name(op, vt, small);
            }
        }
    }
    // the long long divide has a smaller routine (one step per dividend bit)
    // for a function tuned for size
    for (op, name) in
        [SDiv, UDiv, SRem, URem].into_iter().zip(["__s_divdi3", "__s_udivdi3", "__s_moddi3", "__s_umoddi3"])
    {
        if !speed_routines {
            tl.set_size_libcall_name(op, MVT::I64, name);
        }
    }
    // softened floats: compiler-rt's routines
    tl.set_soft_float_libcall_names();
    tl.set_operation_action(BrCond, MVT::Other, Action::Custom);
    // equality of a long is the bytes' xors or-ed in the accumulator
    tl.set_byte_equality(true);
    tl.set_assert_sext_keeps_high(true);
    tl.set_operation_action(GlobalAddress, MVT::I16, Action::Custom);
    // a jump table: the word at table + 2 * index, jumped to (`ExpandNode` of
    // BR_JT)
    tl.set_operation_action(BrJT, MVT::Other, Action::Expand);
    tl.set_operation_action(JumpTable, MVT::I16, Action::Custom);
    tl
}

/// The custom lowerings.
pub struct Z80Lowering {
    /// The function is tuned for size: the small routines
    /// (`set_size_libcall_name`).
    pub sized: bool,
}

impl Z80Lowering {
    /// A variable 64-bit shift as one loop over eight bytes in a frame cell, a
    /// bit a trip (`WSHIFT`): the value stored, shifted in place and loaded
    /// back. The module price decides against the routine's call
    /// (`INLINE_SHIFTS`, `compile::decomposed_functions`). The pin's
    /// counterpart is `ExpandIntRes_Shift`'s library call; MSP430
    /// (`EmitShiftInstr`) and AVR (`insertShift`) expand a variable shift into
    /// a loop block.
    fn lower_wide_shift(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        let (value, amount) = (dag.operand(node, 0), dag.operand(node, 1));
        if dag.constant(amount).is_some() {
            return None;
        }
        let (kind, to_high) = match dag.opcode(node) {
            ISD::Shl => (0, 0),
            ISD::Srl => (1, 7),
            _ => (2, 7),
        };
        let cell = dag.create_stack_temporary(8, 1);
        let info = MemInfo {
            mem_vt: EVT::I64,
            align: 1,
            volatile: false,
            base: MemBase::Stack(cell),
            ext: None,
            space: 0,
            ordering: None,
        };
        let address = dag.get_frame_index(EVT::I16, cell);
        let root = dag.entry_node();
        let stored = dag.get_store(root, value, address, info);
        // the loop starts at the byte the bits leave from: the low one for a
        // left shift, the high one for a right
        let pointer = if to_high == 0 {
            address
        } else {
            let seven = dag.get_constant(to_high, EVT::I16);
            dag.get_node(ISD::Add, EVT::I16, &[address, seven])
        };
        let count = dag.get_zext_or_trunc(amount, EVT::I16);
        let kind = dag.get_constant(kind, EVT::I8);
        let shifted = dag.get_node(super::call::WSHIFT, EVT::OTHER, &[stored, pointer, count, kind]);
        Some(dag.get_load(EVT::I64, shifted, address, info))
    }

    /// A shift by a constant is selected as it is; by a variable it is the call
    /// of `__ashl`, `__lshr` or `__ashr` (`RTLIB::SHL_I16`, ...), the
    /// amount a byte.
    fn lower_shift(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        let (value, amount) = (dag.operand(node, 0), dag.operand(node, 1));
        let vt = dag.value_type(node);
        if vt == EVT::I64 {
            return self.lower_wide_shift(dag, node);
        }
        if let Some(count) = dag.constant(amount) {
            // a word shifted right by a byte or more, keeping its sign, is its
            // high byte shifted by the rest and sign-extended: the bit by bit
            // form is 2 instructions a bit
            if dag.opcode(node) == ISD::Sra && vt == EVT::I16 && (8..16).contains(&count) {
                let (eight, rest) = (dag.get_constant(8, EVT::I8), dag.get_constant(count - 8, EVT::I8));
                let high = dag.get_node(ISD::Srl, EVT::I16, &[value, eight]);
                let high = dag.get_node(ISD::Truncate, EVT::I8, &[high]);
                let byte = if count == 8 { high } else { dag.get_node(ISD::Sra, EVT::I8, &[high, rest]) };
                return Some(dag.get_node(ISD::SignExtend, EVT::I16, &[byte]));
            }
            return Some(node);
        }
        let name = match (dag.opcode(node), vt) {
            (ISD::Shl, EVT::I8) => "__ashlqi3",
            (ISD::Srl, EVT::I8) => "__lshrqi3",
            (ISD::Sra, EVT::I8) => "__ashrqi3",
            (ISD::Shl, _) => "__ashlhi3",
            (ISD::Srl, _) => "__lshrhi3",
            _ => "__ashrhi3",
        };
        self.make_lib_call(dag, name, &[value, amount], vt, dag.opcode(node) == ISD::Sra)
    }
}

/// `ExpandDivRemLibCall` for a word: the routine returns the quotient in `de`
/// and the remainder in `hl`, the registers of a long result (low word `hl`,
/// high word `de`), so the remainder is the first part and the quotient the
/// second; the divide routine `__udivhi3` leaves both there already, the
/// signed one is `__divremhi4`.
fn lower_divrem_word(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    use super::call::{Ext, In, Out, lower_call};
    let signed = dag.opcode(node) == ISD::SDivRem;
    let name = if signed { "__divremhi4" } else { "__udivhi3" };
    let ext = if signed { Ext::Sign } else { Ext::Zero };
    let outs: Vec<Out> = (0..2)
        .map(|at| Out {
            value: dag.operand(node, at),
            ext,
            pointer: false,
            arg_vt: EVT::I16,
            split: false,
            variadic: false,
            sret: false,
            byval: None,
        })
        .collect();
    let ins =
        vec![
            In { vt: EVT::I16, pointer: false, arg_vt: EVT::I32, split: true, sret: false, ext: Ext::Any, byval: None };
            2
        ];
    let callee = dag.get_target_external_symbol(EVT::I16, name);
    let root = dag.entry_node();
    let (_chain, values) = lower_call(dag, root, callee, &outs, &ins).ok()?;
    let [remainder, quotient] = values[..] else { return None };
    Some(dag.get_node_multi(ISD::MergeValues, &[EVT::I16, EVT::I16], &[quotient, remainder]))
}

impl Z80Lowering {
    /// `ExpandDivRemLibCall`: one call to the routine that returns the
    /// quotient and stores the remainder through the address it is given, the
    /// last argument; the remainder is loaded back after the call.
    fn lower_divrem(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        use super::call::{Ext, In, Out, lower_call};
        let signed = dag.opcode(node) == ISD::SDivRem;
        let name = match (signed, self.sized) {
            (true, false) => "__divremsi4",
            (false, false) => "__udivremsi4",
            (true, true) => "__s_divremsi4",
            (false, true) => "__s_udivremsi4",
        };
        let ext = if signed { Ext::Sign } else { Ext::Zero };
        let cell = dag.create_stack_temporary(4, 1);
        let mut outs = Vec::new();
        for at in 0..2 {
            let value = dag.operand(node, at);
            for word in 0..2u128 {
                let part = if word == 0 {
                    value
                } else {
                    let by = dag.get_constant(16, EVT::I8);
                    dag.get_node(ISD::Srl, EVT::I32, &[value, by])
                };
                let part = dag.get_node(ISD::Truncate, EVT::I16, &[part]);
                outs.push(Out {
                    value: part,
                    ext,
                    pointer: false,
                    arg_vt: EVT::I32,
                    split: true,
                    variadic: false,
                    sret: false,
                    byval: None,
                });
            }
        }
        outs.push(Out {
            value: dag.get_frame_index(EVT::I16, cell),
            ext: Ext::Any,
            pointer: true,
            arg_vt: EVT::I16,
            split: false,
            variadic: false,
            sret: false,
            byval: None,
        });
        let ins = vec![
            In {
                vt: EVT::I16,
                pointer: false,
                arg_vt: EVT::I32,
                split: true,
                sret: false,
                ext: Ext::Any,
                byval: None
            };
            2
        ];
        let callee = dag.get_target_external_symbol(EVT::I16, name);
        let root = dag.entry_node();
        let (chain, values) = lower_call(dag, root, callee, &outs, &ins).ok()?;
        let [lo, hi] = values[..] else { return None };
        let quotient = dag.get_node(ISD::BuildPair, EVT::I32, &[lo, hi]);
        let address = dag.get_frame_index(EVT::I16, cell);
        let info = MemInfo {
            mem_vt: EVT::I32,
            align: 1,
            volatile: false,
            base: MemBase::Stack(cell),
            ext: None,
            space: 0,
            ordering: None,
        };
        let remainder = dag.get_load(EVT::I32, chain, address, info);
        Some(dag.get_node_multi(ISD::MergeValues, &[EVT::I32, EVT::I32], &[quotient, remainder]))
    }

    /// `ExpandDivRemLibCall` of a long long: the pair is one call to the
    /// routine that writes both through the hidden pointer (the `sret` of
    /// `__divdi3`), a cell of sixteen bytes, the quotient first; both are
    /// loaded back after the call.
    fn lower_divrem_wide(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        use super::call::{Ext, Out, lower_call};
        let signed = dag.opcode(node) == ISD::SDivRem;
        let name = match (signed, self.sized) {
            (true, false) => "__divmddi4",
            (false, false) => "__udivmddi4",
            (true, true) => "__s_divmddi4",
            (false, true) => "__s_udivmddi4",
        };
        let ext = if signed { Ext::Sign } else { Ext::Zero };
        let cell = dag.create_stack_temporary(16, 1);
        let mut outs = vec![Out {
            value: dag.get_frame_index(EVT::I16, cell),
            ext: Ext::Any,
            pointer: true,
            arg_vt: EVT::I16,
            split: false,
            variadic: false,
            sret: true,
            byval: None,
        }];
        for at in 0..2 {
            let value = dag.operand(node, at);
            for word in 0..4u128 {
                let part = if word == 0 {
                    value
                } else {
                    let by = dag.get_constant(16 * word, EVT::I8);
                    dag.get_node(ISD::Srl, EVT::I64, &[value, by])
                };
                let part = dag.get_node(ISD::Truncate, EVT::I16, &[part]);
                outs.push(Out {
                    value: part,
                    ext,
                    pointer: false,
                    arg_vt: EVT::I64,
                    split: true,
                    variadic: false,
                    sret: false,
                    byval: None,
                });
            }
        }
        let callee = dag.get_target_external_symbol(EVT::I16, name);
        let root = dag.entry_node();
        let (chain, _) = lower_call(dag, root, callee, &outs, &[]).ok()?;
        let base = dag.get_frame_index(EVT::I16, cell);
        let load = |dag: &mut SelectionDAG, at: i128| {
            let address = if at == 0 {
                base
            } else {
                let by = dag.get_constant(at as u128, EVT::I16);
                dag.get_node(ISD::Add, EVT::I16, &[base, by])
            };
            let info = MemInfo {
                mem_vt: EVT::I64,
                align: 1,
                volatile: false,
                base: MemBase::Stack(cell),
                ext: None,
                space: 0,
                ordering: None,
            };
            dag.get_load(EVT::I64, chain, address, info)
        };
        let quotient = load(dag, 0);
        let remainder = load(dag, 8);
        Some(dag.get_node_multi(ISD::MergeValues, &[EVT::I64, EVT::I64], &[quotient, remainder]))
    }
}

impl Hooks for Z80Lowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Shl | ISD::Srl | ISD::Sra => self.lower_shift(dag, node),
            ISD::FShr => (dag.constant(dag.operand(node, 2)) == Some(1)).then_some(node),
            ISD::FShl if dag.constant(dag.operand(node, 2)) == Some(15) => {
                let (high, low) = (dag.operand(node, 0), dag.operand(node, 1));
                let one = dag.get_constant(1, dag.value_type(dag.operand(node, 2)));
                Some(dag.get_node(ISD::FShr, EVT::I16, &[high, low, one]))
            }
            ISD::Sub => Some(lower_sub(dag, node)),
            ISD::UAddO | ISD::USubO | ISD::UAddOCarry | ISD::USubOCarry if dag.value_type(node) == EVT::I16 => {
                Some(lower_carry_op(dag, node))
            }
            ISD::SDivRem | ISD::UDivRem if dag.value_type(node) == EVT::I32 => self.lower_divrem(dag, node),
            ISD::SDivRem | ISD::UDivRem if dag.value_type(node) == EVT::I64 => self.lower_divrem_wide(dag, node),
            ISD::SDivRem | ISD::UDivRem if dag.value_type(node) == EVT::I16 => lower_divrem_word(dag, node),
            ISD::SetCC => lower_setcc(dag, node),
            ISD::SetCCCarry => lower_setcc_carry(dag, node),
            ISD::Select => Some(lower_select(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            ISD::ZeroExtend | ISD::AnyExtend | ISD::SignExtend => Some(lower_extend(dag, node)),
            ISD::SignExtendInReg => lower_sign_extend_inreg(dag, node),
            ISD::GlobalAddress => lower_global_address(dag, node),
            ISD::JumpTable => lower_jump_table(dag, node),
            ISD::VAStart => {
                llrm_codegen::backend::selectiondag::target_calling::lower_vastart_frame_index(dag, node, EVT::I16, 1)
            }
            ISD::Load if dag.value_type(node) == EVT::I1 => lower_bit_load(dag, node),
            ISD::Store if dag.value_type(dag.operand(node, 1)) == EVT::I1 => lower_bit_store(dag, node),
            _ => None,
        }
    }

    /// `makeLibCall` through `LowerCall`: the routine's address is a target
    /// external symbol, the chain is the entry node (the result is the
    /// value, nothing orders the call after a store).
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
        // `LowerCallTo` splits a type the target holds in no register: a long
        // travels as two words and a long long as four, low first
        let mut outs = Vec::new();
        // a result of no register is the pointer to its memory, the first
        // argument (`CanLowerReturn` false: the hidden `sret` pointer)
        let slot = (ret == EVT::I64).then(|| dag.create_stack_temporary(8, 1));
        if let Some(cell) = slot {
            outs.push(Out {
                value: dag.get_frame_index(EVT::I16, cell),
                ext: Ext::Any,
                pointer: true,
                arg_vt: EVT::I16,
                split: false,
                variadic: false,
                sret: true,
                byval: None,
            });
        }
        for &value in args {
            let vt = dag.value_type(value);
            let words = match vt {
                EVT::I32 => 2,
                EVT::I64 => 4,
                _ => 1,
            };
            let parts: Vec<SDValue> = if words == 1 {
                vec![value]
            } else {
                (0..words)
                    .map(|word| {
                        let by = dag.get_constant(16 * word as u128, EVT::I8);
                        let shifted = if word == 0 { value } else { dag.get_node(ISD::Srl, vt, &[value, by]) };
                        dag.get_node(ISD::Truncate, EVT::I16, &[shifted])
                    })
                    .collect()
            };
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
        let part_vt = if ret == EVT::I32 { EVT::I16 } else { ret };
        let ins = match ret {
            EVT::I64 => Vec::new(),
            _ => {
                vec![
                    In {
                        vt: part_vt,
                        pointer: false,
                        arg_vt: ret,
                        split: ret == EVT::I32,
                        sret: false,
                        ext: Ext::Any,
                        byval: None
                    };
                    if ret == EVT::I32 { 2 } else { 1 }
                ]
            }
        };
        let callee = dag.get_target_external_symbol(EVT::I16, name);
        let root = dag.entry_node();
        let (chain, values) = lower_call(dag, root, callee, &outs, &ins).ok()?;
        if let Some(cell) = slot {
            let address = dag.get_frame_index(EVT::I16, cell);
            let info = MemInfo {
                mem_vt: EVT::I64,
                align: 1,
                volatile: false,
                base: MemBase::Stack(cell),
                ext: None,
                space: 0,
                ordering: None,
            };
            return Some(dag.get_load(EVT::I64, chain, address, info));
        }
        match values[..] {
            [one] => Some(one),
            [lo, hi] => Some(dag.get_node(ISD::BuildPair, ret, &[lo, hi])),
            _ => None,
        }
    }
}

/// A bit in memory is a byte with the upper bits zero, as the pin's type
/// legalizer makes it where `i1` is not a register type (`PromoteIntRes_LOAD`,
/// `LegalizeDAG`'s truncating store of a non-byte width). Left alone, a loaded
/// bit tested by a branch and widened by a `select` was selected into a
/// compare of its own conditional move.
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

/// The low byte of a word sign-extended in place, `(sra (shl x, 8), 8)` as
/// the expansion has it (sixteen single-bit shifts), is the byte sign-extended
/// to a word: `ld e, a; rla; sbc a, a; ld d, a`. A sign-extending load of a
/// byte reaches here, as the legalizer makes it an extending load and this
/// (`LegalizeDAG`, `SEXTLOAD` expanded).
fn lower_sign_extend_inreg(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::ValueType(from) = dag.node(dag.operand(node, 1)).data else { return None };
    if from != EVT::I8 || dag.value_type(node) != EVT::I16 {
        return None;
    }
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[dag.operand(node, 0)]);
    Some(dag.get_node(ISD::SignExtend, EVT::I16, &[byte]))
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
    let vt = dag.value_type(node);
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
    // a sign-extended bit is all ones (`select c, 1, 2` reaches here as
    // `sext(c) + 2`)
    let width = vt.size_in_bits();
    let set = if dag.opcode(node) == ISD::SignExtend { (1u128 << width) - 1 } else { 1 };
    let (one, zero) = (dag.get_constant(set, vt), dag.get_constant(0, vt));
    let select = dag.get_node(ISD::Select, vt, &[condition, one, zero]);
    lower_select(dag, select)
}

/// A word difference whose operands both have a zero high byte is the
/// difference of two zero-extended bytes (`SUBZX8`), whether or not the
/// extension is in the DAG: a phi of a byte and a constant is a word register
/// the extension was made before.
fn lower_sub(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (left, right) = (dag.operand(node, 0), dag.operand(node, 1));
    let widened = |dag: &SelectionDAG, value| dag.opcode(value) == ISD::ZeroExtend;
    if dag.value_type(node) != EVT::I16
        || (widened(dag, left) && widened(dag, right))
        || !masked_value_is_zero(dag, left, 0xFF00)
        || !masked_value_is_zero(dag, right, 0xFF00)
    {
        return node;
    }
    let byte = |dag: &mut SelectionDAG, value| {
        if widened(dag, value) { dag.operand(value, 0) } else { dag.get_node(ISD::Truncate, EVT::I8, &[value]) }
    };
    let (left, right) = (byte(dag, left), byte(dag, right));
    dag.get_node(z80isd::SUBZX, EVT::I16, &[left, right])
}

/// `LowerGlobalAddress`: `WRAPPER` of the address, an immediate the linker
/// fills in.
fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Global { symbol, offset } = dag.node(node).data else { return None };
    let address = dag.get_target_global_address(EVT::I16, symbol, offset);
    Some(dag.get_node(z80isd::WRAPPER, EVT::I16, &[address]))
}

/// `LowerJumpTable`: `WRAPPER` of the table's address.
fn lower_jump_table(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::JumpTable(index) = dag.node(node).data else { return None };
    let table = dag.get_target_jump_table(EVT::I16, index);
    Some(dag.get_node(z80isd::WRAPPER, EVT::I16, &[table]))
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

/// The code a comparison of `op0` with `op1` branches on, and the operands to
/// compare (possibly swapped or sign-flipped). `x > C` is `x >= C + 1`, the
/// constant staying the right-hand side.
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    op0: SDValue,
    op1: SDValue,
) -> (u128, SDValue, SDValue) {
    use CondCode::*;
    let width = dag.value_type(op0).size_in_bits();
    if let (SetUGt | SetULe | SetGt | SetLe, Some(c)) = (cc, dag.constant(op1)) {
        let top = match cc {
            SetUGt | SetULe => (1u128 << width) - 1,
            _ => (1u128 << (width - 1)) - 1,
        };
        if c != top {
            let next = dag.get_constant((c + 1) & ((1u128 << width) - 1), dag.value_type(op0));
            let wider = match cc {
                SetUGt => SetUGe,
                SetGt => SetGe,
                SetULe => SetULt,
                _ => SetLt,
            };
            return translate_cc(dag, wider, op0, next);
        }
    }
    match cc {
        SetEq => (cond::Z, op0, op1),
        SetNe => (cond::NZ, op0, op1),
        SetULt => (cond::C, op0, op1),
        SetUGe => (cond::NC, op0, op1),
        SetUGt => (cond::C, op1, op0),
        SetULe => (cond::NC, op1, op0),
        SetLt | SetGe => {
            let (a, b) = (flip_sign(dag, op0), flip_sign(dag, op1));
            (if cc == SetLt { cond::C } else { cond::NC }, a, b)
        }
        SetGt | SetLe => {
            // a > b is b < a, a <= b is b >= a
            let (a, b) = (flip_sign(dag, op1), flip_sign(dag, op0));
            (if cc == SetGt { cond::C } else { cond::NC }, a, b)
        }
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// The byte and the bit of it that `value` is, as far as a compare with zero
/// can tell: `x & (1 << n)`, in a byte or a word, and the same after the
/// combiner narrowed it to the byte that holds the bit (`trunc (srl (and w, 1
/// << n), 8)`).
fn single_bit_of(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> Option<(SDValue, u32)> {
    let low_byte = |dag: &mut SelectionDAG, word: SDValue| match dag.value_type(word) {
        EVT::I8 => word,
        _ => dag.get_node(ISD::Truncate, EVT::I8, &[word]),
    };
    let high_byte = |dag: &mut SelectionDAG, word: SDValue| {
        let eight = dag.get_constant(8, EVT::I8);
        let high = dag.get_node(ISD::Srl, EVT::I16, &[word, eight]);
        dag.get_node(ISD::Truncate, EVT::I8, &[high])
    };
    let single = |dag: &SelectionDAG, and: SDValue| {
        (dag.opcode(and) == ISD::And)
            .then(|| dag.constant(dag.operand(and, 1)).filter(|bits| bits.is_power_of_two()))
            .flatten()
            .map(|bits| (dag.operand(and, 0), bits.trailing_zeros()))
    };
    match dag.opcode(value) {
        ISD::And if matches!(dag.value_type(value), EVT::I8 | EVT::I16) => {
            let (word, index) = single(dag, value)?;
            let byte = if index < 8 { low_byte(dag, word) } else { high_byte(dag, word) };
            Some((byte, index & 7))
        }
        ISD::ZeroExtend | ISD::AnyExtend if dag.value_type(dag.operand(value, 0)) == EVT::I8 => {
            single_bit_of(dag, dag.operand(value, 0))
        }
        ISD::Truncate => {
            let shifted = dag.operand(value, 0);
            let (inner, by) = (dag.operand(shifted, 0), dag.constant(dag.operand(shifted, 1))?);
            if dag.opcode(shifted) != ISD::Srl || dag.value_type(shifted) != EVT::I16 || by != 8 {
                return None;
            }
            let (word, index) = single(dag, inner)?;
            (index >= 8).then(|| (high_byte(dag, word), index - 8))
        }
        _ => None,
    }
}

/// The condition code and flags of `op0 cc op1`. A comparison of a signed value
/// with zero is the test of its sign bit (`bit 7`), not a compare of the value
/// with a flipped copy: `x < 0` is `NZ`, `x >= 0` is `Z`.
fn compare(
    dag: &mut SelectionDAG,
    mut cc: CondCode,
    mut op0: SDValue,
    mut op1: SDValue,
) -> (u128, SDValue) {
    use CondCode::*;
    let vt = dag.value_type(op0);
    let minus_one = (1u128 << vt.size_in_bits()) - 1;
    // x > -1 is x >= 0, x <= -1 is x < 0
    if let (SetGt | SetLe, Some(c)) = (cc, dag.constant(op1)) {
        if c == minus_one {
            cc = if cc == SetGt { SetGe } else { SetLt };
            op1 = dag.get_constant(0, vt);
        }
    }
    if matches!(cc, SetLt | SetGe) && dag.constant(op1) == Some(0) {
        let flags = dag.get_node(z80isd::TSTS, EVT::I8, &[op0]);
        return (if cc == SetLt { cond::NZ } else { cond::Z }, flags);
    }
    // `(x & (1 << n)) == 0` is `bit n, x`: the Z flag is set when the bit is
    // clear. X86's `LowerAndToBT`, on the byte of the word that holds the
    // bit (`bit` has no word form).
    if matches!(cc, SetEq | SetNe) && dag.constant(op1) == Some(0) {
        if let Some((byte, index)) = single_bit_of(dag, op0) {
            let bit = dag.get_constant(u128::from(index), EVT::I8);
            let flags = dag.get_node(z80isd::BIT, EVT::I8, &[byte, bit]);
            return (if cc == SetEq { cond::Z } else { cond::NZ }, flags);
        }
    }
    // `cp (hl)` takes the memory operand second: a loaded byte compared with a
    // register is compared the other way round, with the order of the
    // condition reversed
    if vt == EVT::I8 && dag.opcode(op0) == ISD::Load && dag.opcode(op1) != ISD::Load && dag.constant(op1).is_none() {
        cc = match cc {
            SetULt => SetUGt,
            SetUGt => SetULt,
            SetULe => SetUGe,
            SetUGe => SetULe,
            SetLt => SetGt,
            SetGt => SetLt,
            SetLe => SetGe,
            SetGe => SetLe,
            other => other,
        };
        std::mem::swap(&mut op0, &mut op1);
        let op1 = op1;
        let (code, a, b) = translate_cc(dag, cc, op0, op1);
        return (code, dag.get_node(z80isd::CMP, EVT::I8, &[a, b]));
    }
    // A signed order of two words that are not constants: the subtract's sign
    // corrected for overflow (6 B) where biasing both operands costs 8.
    // DEVIATE Z80-CMP16S-ROW (owner z80, exit 2026-11-15): compare_to_int64
    // +19 B / +416 T at -O1..-O3 (allocation after the one site; 917 B against
    // SDCC's 1589 B).
    if vt == EVT::I16 && dag.constant(op0).is_none() && dag.constant(op1).is_none() {
        let (code, a, b) = match cc {
            SetLt => (cond::C, op0, op1),
            SetGe => (cond::NC, op0, op1),
            SetGt => (cond::C, op1, op0),
            SetLe => (cond::NC, op1, op0),
            _ => {
                let (code, a, b) = translate_cc(dag, cc, op0, op1);
                return (code, dag.get_node(z80isd::CMP, EVT::I8, &[a, b]));
            }
        };
        return (code, dag.get_node(z80isd::CMPS, EVT::I8, &[a, b]));
    }
    let (code, a, b) = translate_cc(dag, cc, op0, op1);
    (code, dag.get_node(z80isd::CMP, EVT::I8, &[a, b]))
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
    let (code, flags) = compare(dag, cc, op0, op1);
    let code = cc_operand(dag, code);
    Some(dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]))
}

/// `UADDO`, `USUBO`, `UADDO_CARRY` and `USUBO_CARRY` on words: the sum
/// (difference) and the flags it leaves, the carry out being the flags' C.
fn lower_carry_op(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    super::pipeline::CARRY_OPS.with(|count| count.set(count.get() + 1));
    if let Some(generic) = generic_carry_op(dag, node) {
        return generic;
    }
    let pair = carry_pair(dag, node);
    let (sum, flags) = (SDValue { node: pair.node, res: 0 }, SDValue { node: pair.node, res: 1 });
    let code = cc_operand(dag, cond::C);
    let byte = dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]);
    let carry = dag.get_node(ISD::Truncate, EVT::I1, &[byte]);
    dag.get_node_multi(ISD::MergeValues, &[EVT::I16, EVT::I1], &[sum, carry])
}

/// A carry op left to the generic form (`add`, then the carry as a compare of
/// the sum), which the step passes read: a step by one (`inc rr` and a zero
/// test make its carry; `add hl, rr` does it dearer), and the high word of one,
/// which adds the step's carry, held in no flag, to a word.
fn generic_carry_op(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let op = dag.opcode(node);
    let add = matches!(op, ISD::UAddO | ISD::UAddOCarry);
    let (a, b) = step_operands(dag, node);
    let (sum_op, order) = if add { (ISD::Add, CondCode::SetULt) } else { (ISD::Sub, CondCode::SetUGt) };
    let (sum, carry) = if matches!(op, ISD::UAddO | ISD::USubO) {
        if !is_step_by_one(dag, node) {
            return None;
        }
        step_sum_and_carry(dag, node)
    } else {
        let carry_in = dag.operand(node, 2);
        if let Some(flags) = carry_flags_of(dag, carry_in) {
            // Nothing added, the carry out unread: a word stepped by the carry,
            // `jr nc` over `inc rr`, in place of `adc hl, rr` and
            // the zero it needs in a register. (A constant word has
            // no step to take: `adc hl, hl` or `sbc hl, hl` makes the bit a
            // mask.)
            if dag.constant(b) != Some(0)
                || dag.constant(a).is_some()
                || dag.has_users(SDValue { node: node.node, res: 1 })
            {
                return None;
            }
            let code = cc_operand(dag, cond::C);
            let byte = dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]);
            let step = dag.get_node(ISD::ZeroExtend, EVT::I16, &[byte]);
            let sum = dag.get_node(sum_op, EVT::I16, &[a, step]);
            let carry = word_compare(dag, sum, a, order);
            return Some(dag.get_node_multi(ISD::MergeValues, &[EVT::I16, EVT::I1], &[sum, carry]));
        }
        let step_op = SDValue { node: carry_in.node, res: 0 };
        let from_step = carry_in.res == 1 && is_step_by_one(dag, step_op);
        match dag.constant(b).map(|c| c as u16) {
            // The step's own carry, made again here (nodes are shared) so that
            // it stays the compare it is.
            Some(0) => {
                let carry_in = if from_step { step_sum_and_carry(dag, step_op).1 } else { carry_in };
                let step = bit_of(dag, carry_in);
                let sum = dag.get_node(sum_op, EVT::I16, &[a, step]);
                (sum, word_compare(dag, sum, a, order))
            }
            // x + 0xFFFF + c is x less the borrow of the step, `ll == 0`, as a
            // decrement's high word is.
            Some(0xFFFF)
                if add && from_step && dag.constant(dag.operand(step_op, 1)).map(|c| c as u16) == Some(0xFFFF) =>
            {
                let low = dag.operand(step_op, 0);
                let zero = dag.get_constant(0, EVT::I16);
                let borrow = word_compare(dag, low, zero, CondCode::SetEq);
                let borrow = bit_of(dag, borrow);
                let sum = dag.get_node(ISD::Sub, EVT::I16, &[a, borrow]);
                let (_, carry_in) = step_sum_and_carry(dag, step_op);
                let step = bit_of(dag, carry_in);
                let any = dag.get_node(ISD::Or, EVT::I16, &[a, step]);
                (sum, word_compare(dag, any, zero, CondCode::SetNe))
            }
            _ => return None,
        }
    };
    Some(dag.get_node_multi(ISD::MergeValues, &[EVT::I16, EVT::I1], &[sum, carry]))
}

/// The sum of a step by one and its carry out: the sum is zero (an increment)
/// or the operand is not (a decrement); a subtract borrows as it wraps.
fn step_sum_and_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> (SDValue, SDValue) {
    let add = dag.opcode(node) == ISD::UAddO;
    let (a, b) = step_operands(dag, node);
    let sum = dag.get_node(if add { ISD::Add } else { ISD::Sub }, EVT::I16, &[a, b]);
    let zero = dag.get_constant(0, EVT::I16);
    let carry = match dag.constant(b).map(|c| c as u16) {
        Some(1) if add => word_compare(dag, sum, zero, CondCode::SetEq),
        Some(0xFFFF) if add => word_compare(dag, a, zero, CondCode::SetNe),
        _ if add => word_compare(dag, sum, a, CondCode::SetULt),
        _ => word_compare(dag, a, b, CondCode::SetULt),
    };
    (sum, carry)
}

/// A compare of two words as the bit it sets, already in the form selection
/// takes.
fn word_compare(
    dag: &mut SelectionDAG,
    lhs: SDValue,
    rhs: SDValue,
    cc: CondCode,
) -> SDValue {
    let (code, flags) = compare(dag, cc, lhs, rhs);
    let code = cc_operand(dag, code);
    let byte = dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]);
    dag.get_node(ISD::Truncate, EVT::I1, &[byte])
}

/// A bit made by `word_compare` as a word of 0 or 1: its byte, zero-extended
/// (`zext`).
fn bit_of(
    dag: &mut SelectionDAG,
    bit: SDValue,
) -> SDValue {
    let byte = if dag.opcode(bit) == ISD::Truncate { dag.operand(bit, 0) } else { condition_byte(dag, bit) };
    dag.get_node(ISD::ZeroExtend, EVT::I16, &[byte])
}

/// The operands of a carry op, a constant of an add last.
fn step_operands(
    dag: &SelectionDAG,
    node: SDValue,
) -> (SDValue, SDValue) {
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    if matches!(dag.opcode(node), ISD::UAddO | ISD::UAddOCarry)
        && dag.constant(a).is_some()
        && dag.constant(b).is_none()
    {
        (b, a)
    } else {
        (a, b)
    }
}

/// A word op with a carry out that adds or subtracts one: `inc rr` / `dec rr`
/// and a zero test.
fn is_step_by_one(
    dag: &SelectionDAG,
    node: SDValue,
) -> bool {
    matches!(dag.opcode(node), ISD::UAddO | ISD::USubO)
        && matches!(
            dag.constant(step_operands(dag, node).1).map(|c| c as u16),
            Some(1 | 0xFFFF)
        )
}

/// The flags that hold `carry` in C when a carry op made it, else none.
fn carry_flags_of(
    dag: &mut SelectionDAG,
    carry: SDValue,
) -> Option<SDValue> {
    let value = if dag.opcode(carry) == ISD::MergeValues { dag.operand(carry, carry.res as usize) } else { carry };
    if let Some(flags) = carry_out_flags(dag, carry) {
        return Some(flags);
    }
    if dag.opcode(value) == ISD::Truncate {
        let byte = dag.operand(value, 0);
        if dag.opcode(byte) == z80isd::SETCC && dag.constant(dag.operand(byte, 0)) == Some(cond::C) {
            return Some(dag.operand(byte, 1));
        }
    }
    None
}

/// The `ADDF`/`ADCF`/`SUBF`/`SBCF` node (sum, flags) of a carry op. Nodes are
/// shared, so a carry in made from the op before it here is the very node that
/// op's own lowering makes.
fn carry_pair(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let op = dag.opcode(node);
    let subtract = matches!(op, ISD::USubO | ISD::USubOCarry);
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    if matches!(op, ISD::UAddOCarry | ISD::USubOCarry) {
        let flags = carry_flags(dag, dag.operand(node, 2));
        dag.get_node_multi(if subtract { z80isd::SBCF } else { z80isd::ADCF }, &[EVT::I16, EVT::I8], &[a, b, flags])
    } else {
        dag.get_node_multi(if subtract { z80isd::SUBF } else { z80isd::ADDF }, &[EVT::I16, EVT::I8], &[a, b])
    }
}

/// The flags whose C is the carry out of a word carry op still in its generic
/// form (`UADDO` and the others, result 1): the very
/// `ADDF`/`ADCF`/`SUBF`/`SBCF` node `lower_carry_op` makes of it.
fn carry_out_flags(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> Option<SDValue> {
    let op = dag.opcode(value);
    if value.res != 1
        || !matches!(
            op,
            ISD::UAddO | ISD::USubO | ISD::UAddOCarry | ISD::USubOCarry
        )
        || dag.value_type(SDValue { node: value.node, res: 0 }) != EVT::I16
        || is_step_by_one(dag, SDValue { node: value.node, res: 0 })
    {
        return None;
    }
    let pair = carry_pair(dag, SDValue { node: value.node, res: 0 });
    Some(SDValue { node: pair.node, res: 1 })
}

/// The flags that hold `carry` in C: those of the carry op before it (its own
/// node, or the lowered `Truncate(SETCC C, flags)`), else the flags of `0 -
/// byte`, whose C is set when the byte is not zero.
fn carry_flags(
    dag: &mut SelectionDAG,
    carry: SDValue,
) -> SDValue {
    if let Some(flags) = carry_flags_of(dag, carry) {
        return flags;
    }
    // The carry of a step by one is the compare it is made in (the step is not
    // a flag op).
    let step = SDValue { node: carry.node, res: 0 };
    let carry = if carry.res == 1 && is_step_by_one(dag, step) { step_sum_and_carry(dag, step).1 } else { carry };
    let byte = condition_byte(dag, carry);
    let zero = dag.get_constant(0, EVT::I8);
    dag.get_node(z80isd::CMP, EVT::I8, &[zero, byte])
}

/// `LowerSETCCCARRY`: the high words' compare takes the borrow of the low
/// words' (`cp16`, then `cp16c`), the order read from the carry. A signed order
/// has the carry made to hold the sign corrected for overflow.
fn lower_setcc_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (high, other, borrow) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    // The low words' subtract folded to no borrow (`x - 0` never borrows): the
    // high compare stands alone.
    if dag.constant(borrow) == Some(0) && matches!(dag.value_type(high), EVT::I8 | EVT::I16) {
        let (code, flags) = compare(dag, cond_of(dag, dag.operand(node, 3)), high, other);
        let code = cc_operand(dag, code);
        return Some(dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]));
    }
    if dag.opcode(borrow) != ISD::USubO || dag.value_type(high) != EVT::I16 {
        return None;
    }
    let (low, below) = (dag.operand(borrow, 0), dag.operand(borrow, 1));
    let cc = cond_of(dag, dag.operand(node, 3));
    let code = match cc {
        CondCode::SetULt | CondCode::SetLt => cond::C,
        CondCode::SetUGe | CondCode::SetGe => cond::NC,
        _ => return None,
    };
    let signed = dag.get_constant(matches!(cc, CondCode::SetLt | CondCode::SetGe) as u128, EVT::I8);
    let carry = dag.get_node(z80isd::CMP, EVT::I8, &[low, below]);
    let flags = dag.get_node(z80isd::CMPC, EVT::I8, &[high, other, carry, signed]);
    let code = cc_operand(dag, code);
    Some(dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags]))
}

/// The code and flags a condition value branches on: a lowered compare, or the
/// value tested against zero (`NE`).
fn condition(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> (SDValue, SDValue) {
    let value = match dag.opcode(value) {
        ISD::SetCC => lower_setcc(dag, value).unwrap_or(value),
        ISD::SetCCCarry => lower_setcc_carry(dag, value).unwrap_or(value),
        _ => value,
    };
    if dag.opcode(value) == z80isd::SETCC {
        return (dag.operand(value, 0), dag.operand(value, 1));
    }
    // the carry a carry op left (`lower_carry_op`): branch on its flags
    if let Some(flags) = carry_out_flags(dag, value) {
        return (cc_operand(dag, cond::C), flags);
    }
    let wrapped = if dag.opcode(value) == ISD::MergeValues { dag.operand(value, value.res as usize) } else { value };
    if dag.opcode(wrapped) == ISD::Truncate && dag.opcode(dag.operand(wrapped, 0)) == z80isd::SETCC {
        let setcc = dag.operand(wrapped, 0);
        return (dag.operand(setcc, 0), dag.operand(setcc, 1));
    }
    let byte = condition_byte(dag, value);
    let zero = dag.get_constant(0, EVT::I8);
    let flags = dag.get_node(z80isd::CMP, EVT::I8, &[byte, zero]);
    (cc_operand(dag, cond::NZ), flags)
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
        ISD::SetCCCarry => true,
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
            ISD::UAddO | ISD::USubO | ISD::UAddOCarry | ISD::USubOCarry if carry_out_flags(dag, value).is_some() => {
                let flags = carry_out_flags(dag, value).expect("a carry op");
                let code = cc_operand(dag, cond::C);
                dag.get_node(z80isd::SETCC, EVT::I8, &[code, flags])
            }
            ISD::Constant => {
                let bit = dag.constant(value).expect("a constant") & 1;
                dag.get_constant(bit, EVT::I8)
            }
            ISD::SetCC => match lower_setcc(dag, value) {
                Some(lowered) => lowered,
                None => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
            },
            ISD::SetCCCarry => match lower_setcc_carry(dag, value) {
                Some(lowered) => lowered,
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
            // a bit loaded and tested at once: the byte read, masked
            ISD::Load => match lower_bit_load(dag, value) {
                Some(lowered) => condition_byte(dag, lowered),
                None => dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]),
            },
            ISD::Freeze => {
                let bit = dag.operand(value, 0);
                condition_byte(dag, bit)
            }
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
        _ => value,
    }
}

/// `LowerSELECT`: `CMOV false, true, cc, flags`.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (test, on_true, on_false) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    let vt = dag.value_type(node);
    dag.get_node(z80isd::CMOV, vt, &[on_false, on_true, code, flags])
}

/// `LowerBRCOND`: `BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, test, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    dag.get_node(z80isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
