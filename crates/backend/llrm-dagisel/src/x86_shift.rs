//! `tryShiftAmountMod`: a shift count the hardware masks anyway.
//!
//! # How it works
//! x86 shifts use only the low 5 bits of the count. A count of `n + 32`, `n ^
//! 32`, `31 - n` (as `n ^ -1`) or `32 - n` (as `0 - n`) is the same as `n`,
//! `~n` or `-n` masked to 5 bits, and the table's `shiftMask*` patterns drop
//! that mask. This rewrites the count to `(newcount & 31)` and lets the table
//! select the shift. A translation of [L4664][fold] for 32-bit and narrower
//! values; the 64-bit branch (`Size == 64`) is not needed here (SD35).
//!
//! # Used by
//! The `X86DAGToDAGISel::Select` loop, for `SHL`, `SRL`, `SRA`, `ROTL` and
//! `ROTR` nodes.
//!
//! [fold]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L4664

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt};
use crate::x86_addr::AddrDag;

/// Rewrites the count of the shift `node`; the node now (it may have merged
/// with an equal one) when it did, `None` when the count is not one of the
/// shapes.
pub fn try_shift_amount_mod<D: AddrDag>(
    dag: &mut D,
    node: u32,
) -> Option<u32> {
    // the shift's width in bits: 64 for a quadword, else 32 (the pin's `Size`)
    let size: u64 = if dag.value_type(SDValue { node, res: 0 }) == mvt::i64 { 64 } else { 32 };
    let orig = dag.operand(node, 1);
    let mut amount = orig;
    if dag.opcode(amount.node) == isd::TRUNCATE {
        amount = dag.operand(amount.node, 0);
    }
    let opcode = dag.opcode(amount.node);
    if !matches!(opcode, isd::ADD | isd::SUB | isd::XOR) {
        return None;
    }
    let (add0, add1) = (dag.operand(amount.node, 0), dag.operand(amount.node, 1));
    let constant = |dag: &D, v: SDValue| (dag.opcode(v.node) == isd::Constant).then(|| dag.constant(v)).flatten();
    let (add0_c, add1_c) = (constant(dag, add0), constant(dag, add1));
    let urem = |c: i64| (c as u64) % size;

    let new_amount = if add1_c.is_some_and(|c| urem(c) == 0) {
        // `n + 32`, `n - 32`, `n ^ 32`: the same count modulo 32.
        add0
    } else if opcode != isd::ADD
        && dag.has_one_use(amount)
        && (add0_c.is_some_and(|c| urem(c) == size - 1) || add1_c.is_some_and(|c| urem(c) == size - 1))
    {
        // `31 - n` and `n ^ 31` are `~n` modulo 32.
        if opcode == isd::SUB && add0_c.is_none() {
            return None;
        }
        let vt = dag.value_type(amount);
        let all_ones = dag.get_constant(-1, vt);
        dag.get_node(isd::XOR, vt, &[if add0_c.is_none() { add0 } else { add1 }, all_ones])
    } else if opcode == isd::SUB && add0_c.is_some_and(|c| c as u64 != 0) {
        // `32 - n` is `-n` modulo 32.
        if urem(add0_c.unwrap()) != 0 {
            return None;
        }
        let vt = dag.value_type(amount);
        let zero = dag.get_constant(0, vt);
        dag.get_node(isd::SUB, vt, &[zero, add1])
    } else {
        return None;
    };

    let mut new_amount = new_amount;
    if dag.value_type(new_amount) != mvt::i8 {
        new_amount = dag.get_node(isd::TRUNCATE, mvt::i8, &[new_amount]);
    }
    let mask = dag.get_constant((size - 1) as i64, mvt::i8);
    new_amount = dag.get_node(isd::AND, mvt::i8, &[new_amount, mask]);

    let value = dag.operand(node, 0);
    let updated = dag.update_node_operands(node, &[value, new_amount]);
    if updated != node {
        dag.replace_node(node, updated);
    } else if dag.use_empty(orig.node) {
        dag.remove_dead_node(orig.node);
    }
    Some(updated)
}
