//! Target DAG combines: a multiply by a constant of the form `2^N +- 1` (times
//! a power of two) is a shift and an add or subtract, not a call to the
//! multiply routine.
//!
//! DEVIATE: the pin does this in `visitMUL` when `TLI.decomposeMulByConstant`
//! says so
//! ([L4908](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4908));
//! the shared combiner has no such hook, so it is a target combine on `Mul`
//! that runs when the generic `visitMUL` finds nothing. Only the one-shift
//! forms of the pin's block: no `2^N +- 2^M`. Exit: the hook in the shared
//! `visit_mul`.

use llrm_codegen::backend::selectiondag::dag_combiner::{CombineHooks, CombineLevel};
use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::known_bits::masked_value_is_zero;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{LoadExt, NodeData, SDValue, SelectionDAG};

use super::lowering::mosisd;

/// `sized`: the function is tuned for size (`optsize`, `minsize`).
/// `synthesize`: the module-level re-selection of `mulimage` is running, which
/// may expand a multiply by three signed digits.
pub struct MosCombines {
    pub sized: bool,
    pub synthesize: bool,
}

/// The combines after legalization: only the shared-shift answer, which every
/// run of the combiner must give alike (a fold the first run declined would
/// be made by the next).
pub struct MosLateCombines;

impl CombineHooks for MosLateCombines {
    /// `isTruncateFree` and `isZExtFree` for a word and its low byte: the
    /// byte is a lane of the pair, and an any-extension leaves the high one
    /// unwritten. `ShrinkDemandedOp` then does an add or a subtract whose
    /// result is truncated (`p - s` returned as a byte) on the byte, not on the
    /// word with its high half dropped after (`schr` -O2 47 B).
    fn free_narrowing(
        &self,
        wide: EVT,
        narrow: EVT,
    ) -> bool {
        wide == EVT::I16 && narrow == EVT::I8
    }

    /// DEVIATE 6502-PAIR-SHIFT: see [`keeps_pair_shift_whole`].
    fn wants_shift_of_shifted_logic(
        &self,
        dag: &SelectionDAG,
        shift: SDValue,
    ) -> bool {
        !keeps_pair_shift_whole(dag, shift)
    }

    /// DEVIATE SD257: a word `or` of two bytes read as a long is a byte `or`
    /// (`logical_value` 410 -> 402 B).
    fn known_zero_narrowing(&self) -> bool {
        true
    }

    /// DEVIATE 6502-SHL-COMMUTE: as [`MosCombines`]; every run of the
    /// combiner must give alike.
    fn is_desirable_to_commute_with_shift(
        &self,
        _dag: &SelectionDAG,
        _shift: SDValue,
        _level: CombineLevel,
    ) -> bool {
        false
    }

    fn fold_shift_of_shared_shift(
        &self,
        vt: EVT,
        inner: u32,
    ) -> bool {
        folds_shared_shift(vt, inner)
    }

    /// The pieces of a wide shift by one are made by type legalization, after
    /// the first run: [`funnel_by_one`] reads them here.
    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        matches!(op, ISD::Or | ISD::Add)
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        _level: CombineLevel,
    ) -> Option<SDValue> {
        funnel_by_one(dag, node)
    }
}

/// A word shift by a constant of two bytes joined into the word (`lo | hi <<
/// 8`, the form a `BuildPair` of an A:X argument or result takes): the pin's
/// `combineShiftOfShiftedLogic` splits it into a shift of each byte and an
/// `or` (`x << 1` of an argument: 22 B against 8), where the word shift is one
/// chain of `asl; rol` through A.
///
/// DEVIATE 6502-PAIR-SHIFT (owner llrm-target-6502, exit 2026-11-24): the pin
/// legalizes the word into byte lanes before this fold sees it.
fn keeps_pair_shift_whole(
    dag: &SelectionDAG,
    shift: SDValue,
) -> bool {
    let logic = dag.operand(shift, 0);
    if dag.value_type(shift) != EVT::I16 || dag.opcode(logic) != ISD::Or {
        return false;
    }
    // exactly the legalizer's `BuildPair`: `zext lo | (anyext hi << 8)`, both pieces bytes
    let byte_extended = |value: SDValue, ops: &[ISD]| {
        ops.contains(&dag.opcode(value)) && dag.value_type(dag.operand(value, 0)) == EVT::I8
    };
    let high = |value: SDValue| {
        dag.opcode(value) == ISD::Shl
            && dag.constant(dag.operand(value, 1)) == Some(8)
            && byte_extended(dag.operand(value, 0), &[ISD::AnyExtend, ISD::ZeroExtend])
    };
    let (a, b) = (dag.operand(logic, 0), dag.operand(logic, 1));
    (byte_extended(a, &[ISD::ZeroExtend]) && high(b)) || (byte_extended(b, &[ISD::ZeroExtend]) && high(a))
}

/// A word shift by 1 to 4 is a chain of that many steps for both bytes, and
/// the other half reads it; from 5 a byte shift or a bit trick is cheaper
/// than the chain whoever else reads it.
fn folds_shared_shift(
    vt: EVT,
    inner: u32,
) -> bool {
    vt != EVT::I16 || inner >= 5
}

impl CombineHooks for MosCombines {
    /// `isTruncateFree` and `isZExtFree` for a word and its low byte: the
    /// byte is a lane of the pair, and an any-extension leaves the high one
    /// unwritten. `ShrinkDemandedOp` then does an add or a subtract whose
    /// result is truncated (`p - s` returned as a byte) on the byte, not on the
    /// word with its high half dropped after (`schr` -O2 47 B).
    fn free_narrowing(
        &self,
        wide: EVT,
        narrow: EVT,
    ) -> bool {
        wide == EVT::I16 && narrow == EVT::I8
    }

    /// DEVIATE 6502-PAIR-SHIFT: see [`keeps_pair_shift_whole`].
    fn wants_shift_of_shifted_logic(
        &self,
        dag: &SelectionDAG,
        shift: SDValue,
    ) -> bool {
        !keeps_pair_shift_whole(dag, shift)
    }

    /// DEVIATE SD257: a word `or` of two bytes read as a long is a byte `or`
    /// (`logical_value` 410 -> 402 B).
    fn known_zero_narrowing(&self) -> bool {
        true
    }

    /// DEVIATE 6502-SHL-COMMUTE: `(x + c) << n` stays. Commuted, the shift
    /// runs on the zero-page cells (`asl rc17; rol rc18`) where the add went
    /// through Y and X (bintree -O2 +20 B). The pin's default is true
    /// (@ d1106deb71). Owner: llrm-target-6502. Exit: 2026-11-14, when the
    /// select-shift fold re-fires here.
    fn is_desirable_to_commute_with_shift(
        &self,
        _dag: &SelectionDAG,
        _shift: SDValue,
        _level: CombineLevel,
    ) -> bool {
        false
    }

    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        matches!(
            op,
            ISD::Mul | ISD::Add | ISD::Sub | ISD::Srl | ISD::Shl | ISD::Truncate | ISD::Or | ISD::SetCC
        )
    }

    /// DEVIATE 6502-SHARED-SHIFT: a word shift is one instruction whose two
    /// bytes both come from one chain, so the high half of `x >> 3` reads that
    /// shift, not a byte shift of its own (`x >> 11`). Exit: none; the pin's
    /// pieces are shared by node CSE.
    fn fold_shift_of_shared_shift(
        &self,
        vt: EVT,
        inner: u32,
    ) -> bool {
        folds_shared_shift(vt, inner)
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        _level: CombineLevel,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Add => borrow_of_ult(dag, node)
                .or_else(|| widened_add_with_carry(dag, node))
                .or_else(|| word_with_loaded_byte(dag, node))
                .or_else(|| funnel_by_one(dag, node))
                .or_else(|| low_bit_in_carry(dag, node)),
            ISD::Sub => borrow_of_ult(dag, node).or_else(|| word_with_loaded_byte(dag, node)),
            ISD::Srl | ISD::Truncate => half_of_pair(dag, node),
            ISD::Shl => shifted_byte_in_pieces(dag, node),
            ISD::Or => funnel_by_one(dag, node),
            ISD::SetCC => range_on_low_byte(dag, node),
            ISD::Mul => decompose_mul(dag, node, self.sized, self.synthesize),
            // the combiner offers every target node to the hook
            _ => None,
        }
    }
}

/// `(x - c) <u n` of a word, with `0 < c` and `c + n <= 256`: `x` lies in
/// `[c, c + n)`, below 256, so its high byte is zero and the low byte moves to
/// `lo - c`. The word is compared as `(lo(x) - c, hi(x)) <u n`: a byte subtract
/// where the word subtract made both bytes (`isspace`'s `c >= 9 && c <= 13`,
/// `adc #255` on the high byte and a borrow chain into the compare).
///
/// DEVIATE SD258 (owner llrm-target-6502, exit 2026-12-05): the pin keeps the
/// word range check (`foldRangeTest`'s result) for the target to lower; a
/// target with byte lanes gains the narrower subtract. Not a pin combine.
fn range_on_low_byte(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (sum, bound) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.value_type(sum) != EVT::I16
        || dag.opcode(sum) != ISD::Add
        || !matches!(
            dag.node(dag.operand(node, 2)).data,
            NodeData::CondCode(CondCode::SetULt)
        )
    {
        return None;
    }
    let (x, offset) = (dag.operand(sum, 0), dag.constant(dag.operand(sum, 1))?);
    let (n, c) = (dag.constant(bound)?, (0x10000 - offset) & 0xFFFF);
    if c == 0 || c > 255 || n == 0 || c + n > 256 {
        return None;
    }
    let vt = dag.value_type(node);
    let low = dag.get_node(ISD::Truncate, EVT::I8, &[x]);
    let eight = dag.get_constant(8, EVT::I16);
    let shifted = dag.get_node(ISD::Srl, EVT::I16, &[x, eight]);
    let high = dag.get_node(ISD::Truncate, EVT::I8, &[shifted]);
    let c_byte = dag.get_constant(c, EVT::I8);
    let diff = dag.get_node(ISD::Sub, EVT::I8, &[low, c_byte]);
    let pair = dag.get_node(ISD::BuildPair, EVT::I16, &[diff, high]);
    Some(dag.get_setcc(vt, pair, bound, CondCode::SetULt))
}

/// `(a << 1) | (b >> 15)` and `(a << 15) | (b >> 1)` of a word: one bit crosses
/// from `b` (`a`) to the other word, `FSHL1` and `FSHR1`. A wide shift by one
/// bit is split into words that way (`ExpandShiftByConstant`); it was a bit
/// made into a byte (`asl a; lda #0; rol a`) and `ora`ed in.
///
/// The pin's analogue is `MatchRotate`'s funnel shift
/// ([DAGCombiner.cpp#L9464](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L9464)),
/// which asks the type for a legal or custom `FSHL`: that makes one for every
/// amount, and its expansion hides the shifts from the folds that ran on them.
fn funnel_by_one(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    if !matches!(dag.opcode(node), ISD::Or | ISD::Add) || dag.value_type(node) != EVT::I16 {
        return None;
    }
    let shift = |dag: &SelectionDAG, v: SDValue, op: ISD, by: u128| {
        (dag.opcode(v) == op && dag.constant(dag.operand(v, 1)) == Some(by)).then(|| dag.operand(v, 0))
    };
    let (x, y) = (dag.operand(node, 0), dag.operand(node, 1));
    for (left, right) in [(x, y), (y, x)] {
        if let (Some(a), Some(b)) = (shift(dag, left, ISD::Shl, 1), shift(dag, right, ISD::Srl, 15)) {
            return Some(dag.get_node(mosisd::FSHL1, EVT::I16, &[a, b]));
        }
        if let (Some(a), Some(b)) = (shift(dag, left, ISD::Shl, 15), shift(dag, right, ISD::Srl, 1)) {
            return Some(dag.get_node(mosisd::FSHR1, EVT::I16, &[a, b]));
        }
    }
    None
}

/// `x + (y & 1)` of a byte: `y >> 1` and the bit that leaves, the carry of one
/// `lsr`, added to `x` by an add with carry. A loop that counts the bits of a
/// byte (`c += x & 1; x >>= 1`) was `txa; and #1; clc; adc c; sta c; txa; lsr
/// a; tax`: the shift and the bit were two instructions on `x`. The plain shift
/// of `y` by one, if the function has one, is the value of the same node.
///
/// The pin's analogue is the target's `combineAddOrSubToADCOrSBB`
/// ([X86ISelLowering.cpp#L53776](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L53776),
/// `X + (Y & 1)` to `ADC X, 0` on the flags `LowerAndToBT`
/// ([L24380](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24380))
/// makes of the bit). DEVIATE 6502-SRL1C: there the bit test is its own
/// instruction (`BT`); here it is made by the shift, a node of two results
/// merged with the shift that uses the same operand, as the pin merges a divide
/// and a remainder into `UDIVREM` (`useDivRem`,
/// [DAGCombiner.cpp#L5181](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5112)).
/// Bytes only. Owner: llrm-6502-a. Exit: 2026-12-01, when a word shift by one
/// reads the carry the same way (`FSHR1`).
fn low_bit_in_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    if dag.value_type(node) != EVT::I8 {
        return None;
    }
    let low_bit = |dag: &SelectionDAG, v: SDValue| {
        (dag.opcode(v) == ISD::And && dag.has_one_use(v.node))
            .then(|| {
                let (a, b) = (dag.operand(v, 0), dag.operand(v, 1));
                if dag.constant(b) == Some(1) {
                    Some(a)
                } else if dag.constant(a) == Some(1) {
                    Some(b)
                } else {
                    None
                }
            })
            .flatten()
    };
    let (x, y) = [(0, 1), (1, 0)]
        .into_iter()
        .find_map(|(x, b)| low_bit(dag, dag.operand(node, b)).map(|y| (dag.operand(node, x), y)))?;
    if dag.value_type(y) != EVT::I8 {
        return None;
    }
    let shifted = dag.get_node_multi(mosisd::SRL1C, &[EVT::I8, EVT::I1], &[y]);
    let plain: Vec<u32> = dag
        .users(y)
        .filter(|&user| {
            let v = SDValue { node: user, res: 0 };
            dag.opcode(v) == ISD::Srl && dag.operand(v, 0) == y && dag.constant(dag.operand(v, 1)) == Some(1)
        })
        .collect();
    for user in plain {
        dag.replace_all_uses_with(SDValue { node: user, res: 0 }, SDValue { node: shifted.node, res: 0 });
    }
    let zero = dag.get_constant(0, EVT::I8);
    let carry = SDValue { node: shifted.node, res: 1 };
    let sum = dag.get_node_multi(ISD::UAddOCarry, &[EVT::I8, EVT::I1], &[x, zero, carry]);
    Some(SDValue { node: sum.node, res: 0 })
}

/// `x + sext(a <u b)` and `x - zext(a <u b)` in a word of two or more bytes:
/// the borrow of `a - b`, subtracted from `x` by a subtract with borrow. A
/// split 64-bit subtract is `lo = a - b`, `hi = (ah - bh) + sext(a <u b)`; the
/// compare was a chain of its own and its result a byte.
///
/// The pin's analogue is the target's `combineAddOrSubToADCOrSBB`
/// ([X86ISelLowering.cpp#L53776](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L53776),
/// `X +- setcc(B)` to `ADC`/`SBB`), the compare and the subtract sharing the
/// flags by CSE. The generic combiner only reads a carry off an existing
/// `usubo` (`getAsCarry`,
/// [DAGCombiner.cpp#L3404](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3404)).
/// Here the `usubo` replaces the subtract `a - b` and gives the borrow.
fn borrow_of_ult(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !matches!(vt, EVT::I32 | EVT::I64) {
        return None;
    }
    let (x, extended) = match dag.opcode(node) {
        ISD::Add => [(0, 1), (1, 0)]
            .into_iter()
            .map(|(x, e)| (dag.operand(node, x), dag.operand(node, e)))
            .find(|&(_, e)| dag.opcode(e) == ISD::SignExtend)?,
        ISD::Sub if dag.opcode(dag.operand(node, 1)) == ISD::ZeroExtend => (dag.operand(node, 0), dag.operand(node, 1)),
        _ => return None,
    };
    let compare = dag.operand(extended, 0);
    if dag.opcode(compare) != ISD::SetCC
        || !matches!(
            dag.node(dag.operand(compare, 2)).data,
            NodeData::CondCode(CondCode::SetULt)
        )
    {
        return None;
    }
    let (a, b) = (dag.operand(compare, 0), dag.operand(compare, 1));
    if dag.value_type(a) != vt {
        return None;
    }
    let difference = dag.get_node_multi(ISD::USubO, &[vt, EVT::I1], &[a, b]);
    // the subtract of the same operands is the difference of the `usubo`
    let plain: Vec<u32> = dag.users(a).filter(|&user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::Sub).collect();
    for user in plain {
        if dag.operand(SDValue { node: user, res: 0 }, 1) == b && dag.operand(SDValue { node: user, res: 0 }, 0) == a {
            dag.replace_all_uses_with(SDValue { node: user, res: 0 }, SDValue { node: difference.node, res: 0 });
        }
    }
    let borrow = SDValue { node: difference.node, res: 1 };
    let (p, q) = if dag.opcode(x) == ISD::Sub && dag.has_one_use(x.node) {
        (dag.operand(x, 0), dag.operand(x, 1))
    } else {
        (x, dag.get_constant(0, vt))
    };
    let result = dag.get_node_multi(ISD::USubOCarry, &[vt, EVT::I1], &[p, q, borrow]);
    Some(SDValue { node: result.node, res: 0 })
}

/// `zext a + zext b + c` in a word, `c` a bit (the carry of the byte add
/// before): `adc` on the bytes, its carry the high byte. The word add was
/// split into two byte adds and a third for the bit, each carry a byte of its
/// own, 20 instructions where `lda; adc; sta` is 3.
///
/// DEVIATE 6502-9: the pin has no fold from a word sum of bytes to the byte
/// add with carry (an 8-bit target splits the word and the pieces stay). Exit:
/// an `ADDCARRY` fold in the shared combiner's `visitADD`.
fn widened_add_with_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (low, carry) = byte_sum_with_carry(dag, node)?;
    let high = dag.get_node(ISD::ZeroExtend, EVT::I8, &[carry]);
    Some(dag.get_node(ISD::BuildPair, EVT::I16, &[low, high]))
}

/// The low byte and the carry of the byte add that a word sum of that shape
/// is.
fn byte_sum_with_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<(SDValue, SDValue)> {
    if dag.value_type(node) != EVT::I16 || dag.opcode(node) != ISD::Add {
        return None;
    }
    let mut leaves = Vec::new();
    sum_terms(dag, node, &mut leaves);
    // the carry of a sum like this one, still a word sum: the sum it is, so
    // that the bit is known to be one
    for leaf in &mut leaves {
        if dag.opcode(*leaf) == ISD::Srl && dag.constant(dag.operand(*leaf, 1)) == Some(8) {
            if let Some((_, carry)) = byte_sum_with_carry(dag, dag.operand(*leaf, 0)) {
                *leaf = dag.get_node(ISD::ZeroExtend, EVT::I16, &[carry]);
            }
        }
    }
    let [x, y, z] = leaves[..] else { return None };
    let bit = |dag: &SelectionDAG, term: SDValue| masked_value_is_zero(dag, term, 0xFFFE);
    let byte =
        |dag: &SelectionDAG, term: SDValue| masked_value_is_zero(dag, term, 0xFF00) || is_any_extended_byte(dag, term);
    let (a, b, c) =
        [(x, y, z), (x, z, y), (y, z, x)].into_iter().find(|&(a, b, c)| bit(dag, c) && byte(dag, a) && byte(dag, b))?;
    let low = |dag: &mut SelectionDAG, term: SDValue| match dag.opcode(term) {
        ISD::ZeroExtend if dag.value_type(dag.operand(term, 0)) == EVT::I8 => dag.operand(term, 0),
        _ => dag.get_node(ISD::Truncate, EVT::I8, &[term]),
    };
    let (a, b) = (low(dag, a), low(dag, b));
    let carry = carry_in(dag, c);
    let sum = dag.get_node_multi(ISD::UAddOCarry, &[EVT::I8, EVT::I1], &[a, b, carry]);
    Some((SDValue { node: sum.node, res: 0 }, SDValue { node: sum.node, res: 1 }))
}

/// A byte widened with undefined high bits (`any_extend`, or a load that
/// extends that way): zero is one of the values those bits may have, so it is
/// a byte as `zext` is. `SimplifyDemandedBits` turns a `zext` into one
/// wherever the sum's high byte is not read.
fn is_any_extended_byte(
    dag: &SelectionDAG,
    term: SDValue,
) -> bool {
    match dag.opcode(term) {
        ISD::AnyExtend => dag.value_type(dag.operand(term, 0)) == EVT::I8,
        ISD::Load => {
            term.res == 0
                && matches!(
                    dag.node(term).data,
                    NodeData::Mem(info) if info.ext == Some(LoadExt::ExtLoad) && info.mem_vt == EVT::I8
                )
        }
        _ => false,
    }
}

/// `x + zext(*p)` and `x - zext(*p)` in a word, the byte loaded by nothing
/// else: the low bytes add or subtract with the load as the operand, the high
/// byte takes the carry (`adc #0`), as the pin's legalizer expands the word of
/// an 8-bit target (`ExpandIntRes_ADDSUB`,
/// [LegalizeIntegerTypes.cpp#L3725](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeIntegerTypes.cpp#L3725),
/// the operand's half a zero). Left a word op, the byte was loaded into a cell
/// for the word's second operand (`a - *p`: `lda (p); sta t; sec; lda a; sbc t`
/// where `sec; lda a; sbc (p)` is the byte op: 17 B, 13 B).
///
/// DEVIATE 6502-ZBYTE: the word is legal here (a pair of byte cells), so the
/// pin's legalizer never splits it. An address (`p + zext(*q)` read by a load
/// or a store) stays a word add, which `MOSIdx` matches as `(p),y`. Owner:
/// llrm-6502-a. Exit: 2026-12-01, when the word add and subtract expand as the
/// pin's do.
fn word_with_loaded_byte(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let sub = dag.opcode(node) == ISD::Sub;
    if dag.value_type(node) != EVT::I16 || !matches!(dag.opcode(node), ISD::Add | ISD::Sub) {
        return None;
    }
    let loaded = |dag: &SelectionDAG, term: SDValue| {
        term.res == 0
            && dag.opcode(term) == ISD::Load
            && dag.users(term).take(2).count() == 1
            && matches!(
                dag.node(term).data,
                NodeData::Mem(info) if info.ext == Some(LoadExt::ZExtLoad) && info.mem_vt == EVT::I8
            )
    };
    let (x, b) = match (dag.operand(node, 0), dag.operand(node, 1)) {
        (x, b) if loaded(dag, b) => (x, b),
        (b, x) if !sub && loaded(dag, b) => (x, b),
        _ => return None,
    };
    // `p + zext(*q)` that is a pointer stays a word: `(p),y` matches it
    let is_address = |dag: &SelectionDAG, user: u32| {
        let user = SDValue { node: user, res: 0 };
        match dag.opcode(user) {
            ISD::Load => dag.operand(user, 1) == node,
            ISD::Store => dag.operand(user, 2) == node,
            _ => false,
        }
    };
    if dag.users(node).any(|user| is_address(dag, user)) {
        return None;
    }
    let eight = dag.get_constant(8, EVT::I16);
    let high = dag.get_node(ISD::Srl, EVT::I16, &[x, eight]);
    let (x_low, x_high) = (dag.get_node(ISD::Truncate, EVT::I8, &[x]), dag.get_node(ISD::Truncate, EVT::I8, &[high]));
    let low_byte = dag.get_node(ISD::Truncate, EVT::I8, &[b]);
    let zero = dag.get_constant(0, EVT::I8);
    let (first, next) = if sub { (ISD::USubO, ISD::USubOCarry) } else { (ISD::UAddO, ISD::UAddOCarry) };
    let low = dag.get_node_multi(first, &[EVT::I8, EVT::I1], &[x_low, low_byte]);
    let carry = SDValue { node: low.node, res: 1 };
    let high = dag.get_node_multi(next, &[EVT::I8, EVT::I1], &[x_high, zero, carry]);
    Some(dag.get_node(
        ISD::BuildPair,
        EVT::I16,
        &[SDValue { node: low.node, res: 0 }, SDValue { node: high.node, res: 0 }],
    ))
}

/// `(zext x) << c` of a byte `x` to a word, `c` of 6 or 7: the pieces are
/// `x << c` and `x >> (8 - c)`, eight single-bit steps in all (and for 7 the
/// `lowbit`/`topbit` forms), where a word shift is `c` passes over two bytes.
/// `y << 7` for a row of 128 bytes was a loop of seven `asl; rol`.
///
/// DEVIATE 6502-SHL-PIECES (owner llrm-target-6502, exit 2026-12-15): the pin
/// leaves a word shift to the legalizer, whose steps are the word's; it has no
/// byte lanes to split into.
fn shifted_byte_in_pieces(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (extended, amount) = (dag.operand(node, 0), dag.constant(dag.operand(node, 1))?);
    if dag.value_type(node) != EVT::I16
        || dag.opcode(extended) != ISD::ZeroExtend
        || dag.value_type(dag.operand(extended, 0)) != EVT::I8
        || !(6..=7).contains(&amount)
    {
        return None;
    }
    let x = dag.operand(extended, 0);
    let (up, down) = (dag.get_constant(amount, EVT::I8), dag.get_constant(8 - amount, EVT::I8));
    let low = dag.get_node(ISD::Shl, EVT::I8, &[x, up]);
    let high = dag.get_node(ISD::Srl, EVT::I8, &[x, down]);
    let low = dag.get_node(ISD::ZeroExtend, EVT::I16, &[low]);
    let high = dag.get_node(ISD::ZeroExtend, EVT::I16, &[high]);
    let eight = dag.get_constant(8, EVT::I8);
    let high = dag.get_node(ISD::Shl, EVT::I16, &[high, eight]);
    Some(dag.get_node(ISD::Or, EVT::I16, &[low, high]))
}

/// One half of a `BuildPair`: `trunc` of it is the low piece, and `srl` of it
/// by the width of a piece is the high piece, extended.
fn half_of_pair(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let pair = dag.operand(node, 0);
    if dag.opcode(pair) != ISD::BuildPair {
        return None;
    }
    let (low, high) = (dag.operand(pair, 0), dag.operand(pair, 1));
    let piece = dag.value_type(low);
    let vt = dag.value_type(node);
    match dag.opcode(node) {
        ISD::Truncate if vt == piece => Some(low),
        ISD::Srl if dag.constant(dag.operand(node, 1)) == Some(u128::from(piece.size_in_bits())) => {
            Some(dag.get_zext_or_trunc(high, vt))
        }
        _ => None,
    }
}

/// The leaves of a tree of word adds, an inner add only where nothing else
/// reads it.
fn sum_terms(
    dag: &SelectionDAG,
    node: SDValue,
    leaves: &mut Vec<SDValue>,
) {
    for at in 0..2 {
        let term = dag.operand(node, at);
        if dag.opcode(term) == ISD::Add && dag.value_type(term) == EVT::I16 && dag.has_one_use(term.node) {
            sum_terms(dag, term, leaves);
        } else {
            leaves.push(term);
        }
    }
}

/// A word known to be 0 or 1 as the carry going into an add: the carry of an
/// earlier add it was extended from, or any other bit as the carry of adding
/// 255 to its byte (a bit is a carry only where an add made it).
fn carry_in(
    dag: &mut SelectionDAG,
    term: SDValue,
) -> SDValue {
    let mut bit = term;
    if dag.opcode(bit) == ISD::Srl
        && dag.constant(dag.operand(bit, 1)) == Some(8)
        && dag.opcode(dag.operand(bit, 0)) == ISD::BuildPair
    {
        bit = dag.operand(dag.operand(bit, 0), 1);
    }
    while dag.opcode(bit) == ISD::ZeroExtend {
        bit = dag.operand(bit, 0);
    }
    let made_by_add = bit.res == 1
        && dag.value_type(bit) == EVT::I1
        && matches!(
            dag.opcode(bit),
            ISD::UAddO | ISD::UAddOCarry | ISD::USubO | ISD::USubOCarry
        );
    if made_by_add {
        return bit;
    }
    let byte = dag.get_zext_or_trunc(bit, EVT::I8);
    let all = dag.get_constant(255, EVT::I8);
    let sum = dag.get_node_multi(ISD::UAddO, &[EVT::I8, EVT::I1], &[byte, all]);
    SDValue { node: sum.node, res: 1 }
}

/// The signed binary digits of an odd `m` with no two neighbours set (the
/// non-adjacent form), lowest first: `(exponent, +1 or -1)`.
fn non_adjacent_form(mut m: u128) -> Vec<(u32, bool)> {
    let mut digits = Vec::new();
    let mut exponent = 0;
    while m != 0 {
        if m & 1 == 1 {
            let plus = m & 3 == 1;
            digits.push((exponent, plus));
            m = if plus { m - 1 } else { m + 1 };
        }
        m >>= 1;
        exponent += 1;
    }
    digits
}

/// `x * m` for an odd `m` of three signed digits, as two adds in Horner form:
/// `(((x << a) +- x) << b) +- x`, then `<< zeros`, negated for a negative
/// constant. Fewer shifted bits of the two forms (binary `8 + 2 + 1`, or
/// non-adjacent `16 - 4 - 1`).
///
/// DEVIATE 6502-MUL-SYNTH: the pin decomposes `2^N +- 2^M` only and leaves the
/// general case as a TODO
/// ([DAGCombiner.cpp#L5024](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5024):
/// "a target set limit on number of ops and a callback to determine that
/// sequence"). Here the limit is two adds, on a 32-bit value (`__mulsi3`, 2,200
/// clocks a call; a byte or word multiply is a call of a few bytes and the
/// steps cost 7 to 38). Source: gcc's `synth_mult` (expmed.cc), bounded. It
/// runs only in `mulimage`'s re-selection, which keeps it when no multiply call
/// is left in the module and the bytes and clocks of the level pay. Owner:
/// llrm-target-6502. Exit: 2026-12-01, when the shared combine reads the
/// sequence from a target hook.
fn synthesized_mul(
    dag: &mut SelectionDAG,
    x: SDValue,
    magnitude: u128,
    zeros: u32,
    negative: bool,
    vt: EVT,
    synthesize: bool,
) -> Option<SDValue> {
    if !synthesize || vt != EVT::I32 {
        return None;
    }
    let binary: Vec<(u32, bool)> = (0..128).filter(|bit| magnitude >> bit & 1 == 1).map(|bit| (bit, true)).collect();
    let signed_digits = non_adjacent_form(magnitude);
    let mut digits = match (binary.len() == 3, signed_digits.len() == 3) {
        (true, true) if binary[2].0 <= signed_digits[2].0 => binary,
        (_, true) => signed_digits,
        (true, false) => binary,
        _ => return None,
    };
    digits.reverse(); // highest exponent first
    let width = vt.size_in_bits();
    if digits[0].0 + zeros >= width {
        return None;
    }
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u32| {
        let by = dag.get_constant(u128::from(by), EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let op = |plus: bool| if plus { ISD::Add } else { ISD::Sub };
    let first = shift(dag, x, digits[0].0 - digits[1].0);
    let mut result = dag.get_node(op(digits[1].1), vt, &[first, x]);
    result = shift(dag, result, digits[1].0 - digits[2].0);
    result = dag.get_node(op(digits[2].1), vt, &[result, x]);
    if zeros != 0 {
        result = shift(dag, result, zeros);
    }
    if negative {
        let zero = dag.get_constant(0, vt);
        result = dag.get_node(ISD::Sub, vt, &[zero, result]);
    }
    Some(result)
}

/// Whether `x * c` of `width` bits is expanded in line by `decompose_mul`
/// (a shift, or `(2^k +- 1) * 2^z`) and not left as a `__mulhi3` call. The
/// synthesis of `mulimage` (32 bits, a re-selection) is not counted.
pub fn mul_by_constant_stays_in_line(
    width: u32,
    c: i128,
    sized: bool,
) -> bool {
    let mut magnitude = c.unsigned_abs();
    if magnitude <= 1 || magnitude.is_power_of_two() {
        return true;
    }
    let zeros = magnitude.trailing_zeros();
    magnitude >>= zeros;
    let bits = match (magnitude - 1, magnitude + 1) {
        (below, _) if below.is_power_of_two() => below,
        (_, above) if above.is_power_of_two() => above,
        _ => return false,
    };
    let k = bits.trailing_zeros();
    if k + zeros >= width {
        return false;
    }
    !(width == 32 && sized && !(k + zeros >= 8 && (k + zeros) % 8 <= 1 && zeros % 8 == 0))
}

/// `x * c` for `c = (2^k +- 1) * 2^z` as the pin's two runs of shift steps,
/// `(x << k+z) +- (x << z)`, or as one run and a shift of the sum,
/// `((x << k) +- x) << z`, whichever takes fewer steps; negated for a negative
/// `c`. Every step of a wide shift is a pass over each byte of the word, so
/// the steps are the cost; a shift of `x` the function already has is free
/// (the pin's runs share it by CSE: `b[y * 20 + x]` beside `a[y * 16 + x]`).
///
/// DEVIATE 6502-MUL-FACTOR: the pin has only the first form
/// ([DAGCombiner.cpp#L5024](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5024)).
/// Owner: llrm-target-6502. Exit: 2026-11-21, when the shared combine reads the
/// form from a target hook. The constants covered are the pin's.
fn decompose_mul(
    dag: &mut SelectionDAG,
    node: SDValue,
    sized: bool,
    synthesize: bool,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I8 && vt != EVT::I16 && vt != EVT::I32 {
        return None;
    }
    let (x, c) = (dag.operand(node, 0), dag.operand(node, 1));
    let raw = dag.constant(c).filter(|_| !dag.is_opaque(c))?;
    let width = vt.size_in_bits();
    let signed = if raw >> (width - 1) & 1 == 1 { raw as i128 - (1i128 << width) } else { raw as i128 };
    let mut magnitude = signed.unsigned_abs();
    // a power of two is a shift, which `visitMUL` has already made
    if magnitude.is_power_of_two() {
        return None;
    }
    let zeros = magnitude.trailing_zeros();
    magnitude >>= zeros;
    let (add, bits) = match (magnitude - 1, magnitude + 1) {
        (below, _) if below.is_power_of_two() => (true, below),
        (_, above) if above.is_power_of_two() => (false, above),
        _ => return synthesized_mul(dag, x, magnitude, zeros, signed < 0, vt, synthesize),
    };
    let k = bits.trailing_zeros();
    if k + zeros >= width {
        return None;
    }
    // A long multiply is a call of about 59 bytes with its arguments. In a
    // function tuned for size the expansion has to be no longer, and measured
    // (x * C at -Os) it is only where a shift is whole bytes (`2^8 +- 1`,
    // `2^16 +- 1`, `2^24 +- 1`, also `2^9 +- 1`); `x * 3` is 74 bytes. At
    // speed it saves ~2200 clocks a multiply for ~15 bytes.
    if vt == EVT::I32 && sized && !(k + zeros >= 8 && (k + zeros) % 8 <= 1 && zeros % 8 == 0) {
        return None;
    }
    let has = |dag: &SelectionDAG, by: u32| {
        by == 0
            || dag.users(x).any(|user| {
                let user = SDValue { node: user, res: 0 };
                dag.opcode(user) == ISD::Shl
                    && dag.operand(user, 0) == x
                    && dag.constant(dag.operand(user, 1)) == Some(u128::from(by))
            })
    };
    let steps = |dag: &SelectionDAG, by: u32| if has(dag, by) { 0 } else { by };
    let two_runs = steps(dag, k + zeros) + steps(dag, zeros);
    let one_run = steps(dag, k) + zeros;
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u32| {
        let by = dag.get_constant(u128::from(by), EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let op = if add { ISD::Add } else { ISD::Sub };
    let mut result = if one_run < two_runs {
        let sum = {
            let high = shift(dag, x, k);
            dag.get_node(op, vt, &[high, x])
        };
        if zeros == 0 { sum } else { shift(dag, sum, zeros) }
    } else {
        let high = shift(dag, x, k + zeros);
        let low = if zeros == 0 { x } else { shift(dag, x, zeros) };
        dag.get_node(op, vt, &[high, low])
    };
    if signed < 0 {
        let zero = dag.get_constant(0, vt);
        result = dag.get_node(ISD::Sub, vt, &[zero, result]);
    }
    Some(result)
}
