//! `AArch64DAGToDAGISel::tryBitfieldInsertInZeroOp`: `(and (shl x, n), mask)` of one contiguous run
//! of bits is `UBFM` (`UBFIZ`).
//!
//! DEVIATE SD509: the `and (any_extend (shl ..))` form of an `i64` is not matched. Exit: the `Widen`
//! helper.
//!
//! # See also
//! `isBitfieldPositioningOp`, `AArch64ISelDAGToDAG.cpp`, pin d1106deb71cc.

use llrm_dagisel::dag::SDValue as PinValue;
use llrm_dagisel::generated::aarch64::ids::opcode as aarch64_ids;
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::Outcome;
use llrm_dagisel::x86_addr::AddrDag;

fn is_shifted_mask(x: u64) -> bool {
    x != 0 && {
        let filled = x | (x - 1);
        filled.wrapping_add(1) & filled == 0
    }
}

/// `tryBitfieldInsertInZeroOp`. `None` for any node it does not match.
pub fn select_bitfield_insert_in_zero(
    dag: &mut impl AddrDag,
    node: u32,
) -> Option<Outcome> {
    if dag.opcode(node) != isd::AND {
        return None;
    }
    let value = PinValue::new(node, 0);
    let vt = dag.value_type(value);
    let (bits, opcode) = match vt {
        mvt::i32 => (32u32, aarch64_ids::UBFMWri),
        mvt::i64 => (64, aarch64_ids::UBFMXri),
        _ => return None,
    };
    let all = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
    // `isBitfieldPositioningOp`: the bits not provably zero are one run.
    let non_zero = !dag.known_zero_bits(value) & all;
    if !is_shifted_mask(non_zero) {
        return None;
    }
    // `isBitfieldPositioningOpFromAnd`, for UBFIZ (`BiggerPattern` false).
    dag.constant(dag.operand(node, 1))?;
    let shl = dag.operand(node, 0);
    if dag.opcode(shl.node) != isd::SHL {
        return None;
    }
    let shl_imm = u64::try_from(dag.constant(dag.operand(shl.node, 1))?).ok()?;
    let source = dag.operand(shl.node, 0);
    if !dag.has_one_use(shl) {
        return None;
    }
    let dst_lsb = non_zero.trailing_zeros();
    let width = (non_zero >> dst_lsb).trailing_ones();
    if width >= bits || shl_imm != u64::from(dst_lsb) {
        return None;
    }
    let immr = (bits - dst_lsb) % bits;
    let imms = width - 1;
    let (immr, imms) = (dag.target_constant(i64::from(immr), vt), dag.target_constant(i64::from(imms), vt));
    dag.morph_node_to(node, opcode, &[vt], &[source, immr, imms]);
    Some(Outcome::Selected)
}

// ---- `tryBitfieldInsertOp` -------------------------------------------------------------------
//
// DEVIATE SD509: an `OR` none of whose bits are useful is not selected to `IMPLICIT_DEF`, and the
// `any_extend`/`truncate` forms of `isBitfieldExtractOpFromAnd` are not matched. Exit: the `Widen`
// helper.

use crate::mc::immediate::{decode_logical, logical};
use llrm_dagisel::dag::{Dag, SDValue as Value};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::generated::aarch64::ids::opcode as op;

/// `SelectionDAG::MaxRecursionDepth`.
const MAX_DEPTH: u32 = 6;

fn width_of(vt: Vt) -> Option<u32> {
    match vt {
        mvt::i32 => Some(32),
        mvt::i64 => Some(64),
        _ => None,
    }
}

fn all(bits: u32) -> u64 {
    if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

fn is_mask(x: u64) -> bool {
    x != 0 && x & x.wrapping_add(1) == 0
}

/// `isOpcWithIntImmediate`: `node` is `opcode` with a constant right operand.
fn imm_operand<D: Dag>(
    dag: &D,
    node: u32,
    opcode: u16,
) -> Option<u64> {
    if dag.opcode(node) != opcode || dag.num_operands(node) < 2 {
        return None;
    }
    let right = dag.operand(node, 1);
    (dag.opcode(right.node) == isd::Constant).then(|| dag.constant(right)).flatten().map(|c| c as u64)
}

/// A constant operand of a selected node (a target constant).
fn target_imm<D: Dag>(
    dag: &D,
    node: u32,
    index: usize,
) -> u64 {
    dag.constant(dag.operand(node, index)).unwrap_or(0) as u64
}

/// `getUsefulBits`: of the `bits` wide `value`, those a user reads. `useful` is the set asked about
/// (every bit at depth 0).
fn useful_bits<D: Dag>(
    dag: &D,
    value: Value,
    useful: u64,
    bits: u32,
    depth: u32,
) -> u64 {
    if depth >= MAX_DEPTH {
        return useful;
    }
    let mut users = 0u64;
    for (user, _) in dag.users(value) {
        users |= useful_bits_for_use(dag, user, useful, value, bits, depth);
    }
    useful & users
}

fn useful_bits_for_use<D: Dag>(
    dag: &D,
    user: u32,
    useful: u64,
    orig: Value,
    bits: u32,
    depth: u32,
) -> u64 {
    // Users should already be selected.
    if !dag.is_machine(user) {
        return useful;
    }
    let me = Value::new(user, 0);
    match dag.opcode(user) {
        op::ANDSWri | op::ANDSXri | op::ANDWri | op::ANDXri => {
            let imm = decode_logical(target_imm(dag, user, 1) as u32, bits);
            useful_bits(dag, me, useful & imm, bits, depth + 1)
        }
        op::UBFMWri | op::UBFMXri => {
            useful_from_bitfield_move(dag, me, useful, target_imm(dag, user, 1), target_imm(dag, user, 2), bits, depth)
        }
        op::ORRWrs | op::ORRXrs => {
            if dag.operand(user, 0) != orig && dag.operand(user, 1) == orig {
                useful_from_or_shifted(dag, me, useful, bits, depth)
            } else {
                useful
            }
        }
        op::BFMWri | op::BFMXri => useful_from_bfm(dag, me, orig, useful, bits, depth),
        op::STRBBui | op::STURBBi => {
            if dag.operand(user, 0) != orig { useful } else { useful & 0xff }
        }
        op::STRHHui | op::STURHHi => {
            if dag.operand(user, 0) != orig { useful } else { useful & 0xffff }
        }
        _ => useful,
    }
}

fn useful_from_bitfield_move<D: Dag>(
    dag: &D,
    me: Value,
    useful: u64,
    imm: u64,
    msb: u64,
    bits: u32,
    depth: u32,
) -> u64 {
    let full = all(bits);
    let op_useful = if msb >= imm {
        let mask = (1u64 << (msb - imm + 1)).wrapping_sub(1) & full;
        useful_bits(dag, me, mask, bits, depth + 1) << imm & full
    } else {
        let shift = u64::from(bits) - imm;
        let mask = ((1u64 << (msb + 1)).wrapping_sub(1) << shift) & full;
        useful_bits(dag, me, mask, bits, depth + 1) >> shift
    };
    useful & op_useful
}

fn useful_from_or_shifted<D: Dag>(
    dag: &D,
    me: Value,
    useful: u64,
    bits: u32,
    depth: u32,
) -> u64 {
    let full = all(bits);
    let encoded = target_imm(dag, me.node, 2);
    let (kind, amount) = ((encoded >> 6) & 7, encoded & 0x3f);
    let mask = match kind {
        0 => useful_bits(dag, me, full << amount & full, bits, depth + 1) >> amount,
        1 => useful_bits(dag, me, full >> amount, bits, depth + 1) << amount & full,
        _ => return useful,
    };
    useful & mask
}

fn useful_from_bfm<D: Dag>(
    dag: &D,
    me: Value,
    orig: Value,
    useful: u64,
    bits: u32,
    depth: u32,
) -> u64 {
    let full = all(bits);
    let (imm, msb) = (target_imm(dag, me.node, 2), target_imm(dag, me.node, 3));
    let result = useful_bits(dag, me, full, bits, depth + 1);
    let mut mask = 0u64;
    if msb >= imm {
        // BFXIL
        let op_useful = (1u64 << (msb - imm + 1)).wrapping_sub(1) & full;
        if dag.operand(me.node, 1) == orig {
            mask = (result & op_useful) << imm & full;
        }
        if dag.operand(me.node, 0) == orig {
            mask |= result & !op_useful & full;
        }
    } else {
        // BFI
        let lsb = u64::from(bits) - imm;
        let op_useful = ((1u64 << (msb + 1)).wrapping_sub(1) << lsb) & full;
        if dag.operand(me.node, 1) == orig {
            mask = (result & op_useful) >> lsb;
        }
        if dag.operand(me.node, 0) == orig {
            mask |= result & !op_useful & full;
        }
    }
    useful & mask
}

/// `getLeftShift`: `value` shifted left by `amount` (right when negative) as a `UBFM`.
fn left_shift<D: AddrDag>(
    dag: &mut D,
    value: Value,
    amount: i64,
) -> Value {
    if amount == 0 {
        return value;
    }
    let vt = dag.value_type(value);
    let bits = i64::from(width_of(vt).expect("i32 or i64"));
    let opcode = if bits == 32 { op::UBFMWri } else { op::UBFMXri };
    let (immr, imms) = if amount > 0 { (bits - amount, bits - 1 - amount) } else { (-amount, bits - 1) };
    let (immr, imms) = (dag.target_constant(immr, vt), dag.target_constant(imms, vt));
    Value::new(dag.machine_node(opcode, &[vt], &[value, immr, imms]), 0)
}

/// `isBitfieldPositioningOp` with `BiggerPattern` false: `(and (shl VAL, N), ShiftedMask)` or
/// `(shl VAL, N)`: the source, the destination bit and the width.
fn bitfield_positioning<D: AddrDag>(
    dag: &mut D,
    value: Value,
    bigger: bool,
) -> Option<(Value, u32, u32)> {
    let bits = width_of(dag.value_type(value))?;
    let non_zero = !dag.known_zero_bits(value) & all(bits);
    if !is_shifted_mask(non_zero) {
        return None;
    }
    let dst_lsb = non_zero.trailing_zeros();
    let width = (non_zero >> dst_lsb).trailing_ones();
    match dag.opcode(value.node) {
        isd::AND => {
            let and_imm = imm_operand(dag, value.node, isd::AND)?;
            let (and0, shl_imm) = {
                let and0 = dag.operand(value.node, 0);
                (and0, imm_operand(dag, and0.node, isd::SHL)?)
            };
            let shl0 = dag.operand(dag.operand(value.node, 0).node, 0);
            let _ = and_imm;
            if (!bigger && !dag.has_one_use(and0)) || width >= bits || (shl_imm != u64::from(dst_lsb) && !bigger) {
                return None;
            }
            Some((left_shift(dag, shl0, shl_imm as i64 - i64::from(dst_lsb)), dst_lsb, width))
        }
        isd::SHL => {
            let shl_imm = imm_operand(dag, value.node, isd::SHL)?;
            if !bigger && !dag.has_one_use(value) {
                return None;
            }
            // `isSeveralBitsPositioningOpFromShl`: `(shl (and val, mask), N)`.
            let inner = dag.operand(value.node, 0);
            if let Some(and_imm) = imm_operand(dag, inner.node, isd::AND) {
                let shifted = and_imm << shl_imm >> shl_imm;
                let shifted = shifted & all(bits);
                if is_mask(shifted) {
                    return Some((dag.operand(inner.node, 0), shl_imm as u32, shifted.trailing_ones()));
                }
            }
            if shl_imm != u64::from(dst_lsb) && !bigger {
                return None;
            }
            let src = dag.operand(value.node, 0);
            Some((left_shift(dag, src, shl_imm as i64 - i64::from(dst_lsb)), dst_lsb, width))
        }
        _ => None,
    }
}

/// `isBitfieldDstMask`.
fn bitfield_dst_mask(
    dst_mask: u64,
    to_insert: u64,
    ignored_high: u32,
    bits: u32,
) -> bool {
    let significant = all(bits - ignored_high);
    let (dst, insert) = (dst_mask & significant, to_insert & significant);
    dst & insert == 0 && (dst | insert) == significant
}

/// `isWorthFoldingIntoOrrWithShift`: the operand and the encoded shift of an `ORR` that takes `dst`
/// as a shifted register.
fn worth_folding_into_orr<D: AddrDag>(
    dag: &mut D,
    dst: Value,
) -> Option<(Value, u64)> {
    if !dag.has_one_use(dst) {
        return None;
    }
    let vt = dag.value_type(dst);
    let size = u64::from(width_of(vt)?);
    if let Some(and_imm) = imm_operand(dag, dst.node, isd::AND) {
        if !is_shifted_mask(and_imm & all(size as u32)) {
            return None;
        }
        let dst_op0 = dag.operand(dst.node, 0);
        if !dag.has_one_use(dst_op0) {
            return None;
        }
        let srl = imm_operand(dag, dst_op0.node, isd::SRL)?;
        let trailing = u64::from(and_imm.trailing_zeros());
        if srl + trailing >= size {
            return None;
        }
        let mask_width = u64::from((and_imm >> trailing).trailing_ones());
        let ubfm = if size == 32 { op::UBFMWri } else { op::UBFMXri };
        let source = dag.operand(dst_op0.node, 0);
        let (immr, imms) = (
            dag.target_constant((srl + trailing) as i64, vt),
            dag.target_constant((srl + trailing + mask_width - 1) as i64, vt),
        );
        let node = dag.machine_node(ubfm, &[vt], &[source, immr, imms]);
        return Some((Value::new(node, 0), trailing));
    }
    if let Some(shl) = imm_operand(dag, dst.node, isd::SHL) {
        return Some((dag.operand(dst.node, 0), shl));
    }
    if let Some(srl) = imm_operand(dag, dst.node, isd::SRL) {
        return Some((dag.operand(dst.node, 0), (1 << 6) | srl));
    }
    None
}

/// `tryOrrWithShift`.
fn orr_with_shift<D: AddrDag>(
    dag: &mut D,
    node: u32,
    opd0: Value,
    opd1: Value,
    src: Value,
    dst: Value,
    bigger: bool,
) -> Option<Outcome> {
    if opd1 != dst {
        return None;
    }
    let vt = dag.value_type(Value::new(node, 0));
    let orr = if width_of(vt)? == 32 { op::ORRWrs } else { op::ORRXrs };
    if bigger {
        // `opd0 = and Src, #Mask` leaves `Src` for the `ORR`, which also folds `dst`'s shift.
        let mask = imm_operand(dag, opd0.node, isd::AND)?;
        if !is_mask(mask) || dag.operand(opd0.node, 0) != src {
            return None;
        }
        let (shifted, encoded) = worth_folding_into_orr(dag, dst)?;
        let shifter = dag.target_constant(encoded as i64, vt);
        dag.morph_node_to(node, orr, &[vt], &[opd0, shifted, shifter]);
        return Some(Outcome::Selected);
    }
    let select = |dag: &mut D, left: Value, right: Value, kind: u64, amount: u64| {
        let shifter = dag.target_constant(((kind << 6) | amount) as i64, vt);
        dag.morph_node_to(node, orr, &[vt], &[left, right, shifter]);
        Some(Outcome::Selected)
    };
    if let Some(shl) = imm_operand(dag, opd0.node, isd::SHL) {
        let operand = dag.operand(opd0.node, 0);
        if operand == src && dag.has_one_use(opd0) {
            return select(dag, dst, src, 0, shl);
        }
        if operand == opd1 {
            return select(dag, opd1, opd1, 0, shl);
        }
    }
    if let Some(srl) = imm_operand(dag, opd0.node, isd::SRL) {
        if dag.operand(opd0.node, 0) == opd1 {
            return select(dag, opd1, opd1, 1, srl);
        }
    }
    None
}

/// `isBitfieldExtractOpFromAnd` for an `and` of a right shift (or, in the bigger pattern, of the
/// value itself): the source and the `UBFM` bit range (DEVIATE SD509 for the `any_extend`/`truncate`
/// forms).
fn bitfield_extract_from_and<D: AddrDag>(
    dag: &D,
    node: u32,
    ignored_low: u32,
    bigger: bool,
) -> Option<(Value, u32, u32)> {
    let bits = width_of(dag.value_type(Value::new(node, 0)))?;
    let mut and_imm = imm_operand(dag, node, isd::AND)?;
    and_imm |= all(ignored_low.min(64));
    if and_imm & and_imm.wrapping_add(1) != 0 {
        return None;
    }
    let op0 = dag.operand(node, 0);
    let (src, srl) = if let Some(srl) = imm_operand(dag, op0.node, isd::SRL) {
        (dag.operand(op0.node, 0), srl)
    } else if bigger {
        (op0, 0)
    } else {
        return None;
    };
    if !bigger && (srl == 0 || srl >= u64::from(bits)) {
        return None;
    }
    let ones = if bits == 32 { (and_imm as u32).trailing_ones() } else { and_imm.trailing_ones() };
    let mut msb = srl as u32 + ones - 1;
    if bits == 32 && imm_operand(dag, op0.node, isd::SRL).is_some() {
        msb = msb.min(31);
    }
    Some((src, srl as u32, msb))
}

/// `tryBitfieldInsertOpFromOr`.
fn bitfield_insert_from_or<D: AddrDag>(
    dag: &mut D,
    node: u32,
    useful: u64,
) -> Option<Outcome> {
    let result = Value::new(node, 0);
    let vt = dag.value_type(result);
    let bits = width_of(vt)?;
    let ignored_high = (useful << (64 - bits)).leading_zeros().min(bits);
    let ignored_low = useful.trailing_zeros().min(bits);
    for i in 0..4 {
        let bigger = i / 2 == 1;
        let opd0 = dag.operand(node, i % 2);
        let opd1 = dag.operand(node, (i + 1) % 2);
        let (src, dst_lsb, width, imm_r, imm_s);
        if dag.opcode(opd0.node) == isd::AND
            && let Some((extract_src, lsb, msb)) = bitfield_extract_from_and(dag, opd0.node, ignored_low, bigger)
        {
            // The range is a `UBFM` of the right type; `BFXIL` shares its `immr` and `imms`.
            if msb < lsb {
                continue;
            }
            (src, dst_lsb, width, imm_r, imm_s) = (extract_src, 0, msb - lsb + 1, lsb, msb);
        } else if let Some((positioned, lsb, w)) = bitfield_positioning(dag, opd0, bigger) {
            (src, dst_lsb, width, imm_r, imm_s) = (positioned, lsb, w, (bits - lsb) % bits, w - 1);
        } else {
            continue;
        }
        let known_zero_bits = dag.known_zero_bits(opd1);
        let to_insert = all(width) << dst_lsb;
        if to_insert & !known_zero_bits != 0 {
            continue;
        }
        let dst = match imm_operand(dag, opd1.node, isd::AND) {
            Some(mask) if bitfield_dst_mask(mask, to_insert, ignored_high, bits) => dag.operand(opd1.node, 0),
            _ => opd1,
        };
        if let Some(done) = orr_with_shift(dag, node, opd0, opd1, src, dst, bigger) {
            return Some(done);
        }
        let (imm_r, imm_s) = (dag.target_constant(i64::from(imm_r), vt), dag.target_constant(i64::from(imm_s), vt));
        let opcode = if bits == 32 { op::BFMWri } else { op::BFMXri };
        dag.morph_node_to(node, opcode, &[vt], &[dst, src, imm_r, imm_s]);
        return Some(Outcome::Selected);
    }
    // `or (and X, Mask0), (and Y, Mask1)` with the masks complements: a BFXIL.
    let (mut and0, mut and1) = (dag.operand(node, 0), dag.operand(node, 1));
    let (mask0, mask1) = (imm_operand(dag, and0.node, isd::AND)?, imm_operand(dag, and1.node, isd::AND)?);
    let full = all(bits);
    if !(dag.has_one_use(and0) && dag.has_one_use(and1) && mask0 & full == !mask1 & full) {
        return None;
    }
    let (mut mask0, mut mask1) = (mask0 & full, mask1 & full);
    if !(is_shifted_mask(mask0) || is_shifted_mask(mask1)) {
        return None;
    }
    if is_shifted_mask(mask0) {
        std::mem::swap(&mut and0, &mut and1);
        std::mem::swap(&mut mask0, &mut mask1);
    }
    let mut src = dag.operand(and1.node, 0);
    let dst = dag.operand(and0.node, 0);
    let lsb = mask1.trailing_zeros();
    let width = bits - mask0.count_ones();
    let mut lsr = u64::from(lsb);
    if dag.has_one_use(src) {
        if let Some(amount) = imm_operand(dag, src.node, isd::SRL) {
            if amount + u64::from(lsb) < u64::from(bits) {
                lsr = amount + u64::from(lsb);
                src = dag.operand(src.node, 0);
            }
        }
    }
    let shift = if bits == 32 { op::UBFMWri } else { op::UBFMXri };
    let (lsr_c, top) = (dag.target_constant(lsr as i64, vt), dag.target_constant(i64::from(bits) - 1, vt));
    let moved = Value::new(dag.machine_node(shift, &[vt], &[src, lsr_c, top]), 0);
    let imm_r = dag.target_constant(i64::from((bits - lsb) % bits), vt);
    let imm_s = dag.target_constant(i64::from(width) - 1, vt);
    let opcode = if bits == 32 { op::BFMWri } else { op::BFMXri };
    dag.morph_node_to(node, opcode, &[vt], &[dst, moved, imm_r, imm_s]);
    Some(Outcome::Selected)
}

/// `tryBitfieldInsertOpFromOrAndImm`: `or (and X, MaskImm), OrImm` that only sets bits the `and`
/// clears is a `BFI`/`BFXIL` of the constant.
fn bitfield_insert_from_or_and_imm<D: AddrDag>(
    dag: &mut D,
    node: u32,
) -> Option<Outcome> {
    let vt = dag.value_type(Value::new(node, 0));
    let bits = width_of(vt)?;
    let full = all(bits);
    let or_imm = imm_operand(dag, node, isd::OR)? & full;
    // The `ORR` immediate encodes: a BFI would only trade an `AND`+`ORR` for it.
    if logical(or_imm, bits).is_some() {
        return None;
    }
    let and = dag.operand(node, 0);
    if !dag.has_one_use(and) {
        return None;
    }
    imm_operand(dag, and.node, isd::AND)?;
    let not_known_zero = !dag.known_zero_bits(and) & full;
    let known_zero_bits = !not_known_zero & full;
    if !is_shifted_mask(known_zero_bits) || or_imm & not_known_zero != 0 {
        return None;
    }
    let lsb = not_known_zero.trailing_ones();
    let width = bits - not_known_zero.count_ones();
    let imm_r = (bits - lsb) % bits;
    let imm_s = width - 1;
    let is_bfi = lsb != 0;
    let bfi_imm = or_imm >> lsb;
    if is_bfi && logical(bfi_imm, bits).is_none() {
        let chunks = |x: u64| (0..bits).step_by(16).filter(|shift| (x >> shift) & 0xffff != 0).count();
        if chunks(bfi_imm) > chunks(or_imm) {
            return None;
        }
    }
    let movi = if bits == 32 { op::MOVi32imm } else { op::MOVi64imm };
    let constant = dag.target_constant(bfi_imm as i64, vt);
    let materialized = Value::new(dag.machine_node(movi, &[vt], &[constant]), 0);
    let (imm_r, imm_s) = (dag.target_constant(i64::from(imm_r), vt), dag.target_constant(i64::from(imm_s), vt));
    let opcode = if bits == 32 { op::BFMWri } else { op::BFMXri };
    let dst = dag.operand(and.node, 0);
    dag.morph_node_to(node, opcode, &[vt], &[dst, materialized, imm_r, imm_s]);
    Some(Outcome::Selected)
}

/// `tryBitfieldInsertOp`. `None` for any node it does not match.
pub fn select_bitfield_insert<D: AddrDag>(
    dag: &mut D,
    node: u32,
) -> Option<Outcome> {
    if dag.opcode(node) != isd::OR {
        return None;
    }
    let result = Value::new(node, 0);
    let bits = width_of(dag.value_type(result))?;
    let useful = useful_bits(dag, result, all(bits), bits, 0);
    if useful == 0 {
        return None;
    }
    bitfield_insert_from_or(dag, node, useful).or_else(|| bitfield_insert_from_or_and_imm(dag, node))
}
