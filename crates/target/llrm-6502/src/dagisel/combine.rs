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

/// `sized`: the function is tuned for size (`optsize`, `minsize`).
pub struct MosCombines {
    pub sized: bool,
}

/// The combines after legalization: only the shared-shift answer, which every
/// run of the combiner must give alike (a fold the first run declined would
/// be made by the next).
pub struct MosLateCombines;

impl CombineHooks for MosLateCombines {
    fn fold_shift_of_shared_shift(
        &self,
        vt: EVT,
        inner: u32,
    ) -> bool {
        folds_shared_shift(vt, inner)
    }
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
    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        matches!(op, ISD::Mul | ISD::Add | ISD::Sub | ISD::Srl | ISD::Truncate)
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
            ISD::Add => borrow_of_ult(dag, node).or_else(|| widened_add_with_carry(dag, node)),
            ISD::Sub => borrow_of_ult(dag, node),
            ISD::Srl | ISD::Truncate => half_of_pair(dag, node),
            _ => decompose_mul(dag, node, self.sized),
        }
    }
}

/// `x + sext(a <u b)` and `x - zext(a <u b)` in a word of two or more bytes: the
/// borrow of `a - b`, subtracted from `x` by a subtract with borrow. A split
/// 64-bit subtract is `lo = a - b`, `hi = (ah - bh) + sext(a <u b)`; the
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
        ISD::Sub if dag.opcode(dag.operand(node, 1)) == ISD::ZeroExtend => {
            (dag.operand(node, 0), dag.operand(node, 1))
        }
        _ => return None,
    };
    let compare = dag.operand(extended, 0);
    if dag.opcode(compare) != ISD::SetCC
        || !matches!(dag.node(dag.operand(compare, 2)).data, NodeData::CondCode(CondCode::SetULt))
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

/// `x * c` as `(x << n) +- x`, shifted by the factors of two in `c` and negated
/// for a negative `c`.
fn decompose_mul(
    dag: &mut SelectionDAG,
    node: SDValue,
    sized: bool,
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
        _ => return None,
    };
    let amount = bits.trailing_zeros() + zeros;
    if amount >= width {
        return None;
    }
    // A long multiply is a call of about 59 bytes with its arguments. In a
    // function tuned for size the expansion has to be no longer, and measured
    // (x * C at -Os) it is only where a shift is whole bytes (`2^8 +- 1`,
    // `2^16 +- 1`, `2^24 +- 1`, also `2^9 +- 1`); `x * 3` is 74 bytes. At
    // speed it saves ~2200 clocks a multiply for ~15 bytes.
    if vt == EVT::I32 && sized && !(amount >= 8 && amount % 8 <= 1 && zeros % 8 == 0) {
        return None;
    }
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u32| {
        let by = dag.get_constant(u128::from(by), EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let high = shift(dag, x, amount);
    let low = if zeros == 0 { x } else { shift(dag, x, zeros) };
    let mut result = dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[high, low]);
    if signed < 0 {
        let zero = dag.get_constant(0, vt);
        result = dag.get_node(ISD::Sub, vt, &[zero, result]);
    }
    Some(result)
}
