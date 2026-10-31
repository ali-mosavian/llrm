//! `AArch64TargetLowering::PerformDAGCombine`: the scalar integer part.
//!
//! # How it works
//! `performMulCombine` rewrites `x * c` as shifts, adds and subtracts where
//! the pin does, so the selector sees `ADDXrs` and not a `MOVi32imm` and a
//! `MADD`.
//!
//! DEVIATE SD504: the `hasALULSLFast` decompositions are omitted (apple-m1 does not
//! have the feature); the vector, SVE and `rdsvl` parts do not apply to the
//! scalar table; `isOnlyUsedAsMemoryAddress` does not look at indexed
//! accesses. Exit: vectors; indexed addressing.
//!
//! # See also
//! `performMulCombine` in `AArch64ISelLowering.cpp`, pin d1106deb71cc.

use llrm_codegen::backend::selectiondag::dag_combiner::CombineLevel;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_dagisel::generated::aarch64::ids::isd as a64;

/// `isOnlyUsedAsMemoryAddress`: every use of the add/sub is the address of a load or store.
fn only_used_as_memory_address(
    dag: &SelectionDAG,
    node: SDValue,
) -> bool {
    dag.users(node).all(|user| {
        let ops = &dag.node(SDValue { node: user, res: 0 }).ops;
        match dag.opcode(SDValue { node: user, res: 0 }) {
            ISD::Load => ops.get(1) == Some(&node),
            ISD::Store => ops.get(2) == Some(&node),
            ISD::Target(op) if op == a64::PREFETCH => ops.get(2) == Some(&node),
            _ => false,
        }
    })
}

fn shl(
    dag: &mut SelectionDAG,
    x: SDValue,
    amount: u32,
) -> Option<SDValue> {
    let vt = dag.value_type(x);
    if amount >= vt.size_in_bits() {
        return None;
    }
    let by = dag.get_constant(u128::from(amount), EVT::I64);
    Some(dag.get_node(ISD::Shl, vt, &[x, by]))
}

fn binary(
    dag: &mut SelectionDAG,
    op: ISD,
    a: SDValue,
    b: SDValue,
) -> SDValue {
    let vt = dag.value_type(a);
    dag.get_node(op, vt, &[a, b])
}

/// `performMulCombine`, the scalar integer part.
pub fn combine_mul(
    dag: &mut SelectionDAG,
    node: SDValue,
    level: CombineLevel,
) -> Option<SDValue> {
    if level == CombineLevel::BeforeLegalizeTypes {
        return None;
    }
    let vt = dag.value_type(node);
    let width = vt.size_in_bits();
    let (n0, n1) = (dag.operand(node, 0), dag.operand(node, 1));

    // `X*(Y+1)` is `X*Y+X`, `X*(1-Y)` is `X-X*Y`: the MachineCombiner forms a madd.
    let add_sub_with_one = |dag: &SelectionDAG, v: SDValue| -> Option<(ISD, SDValue)> {
        let op = dag.opcode(v);
        if !matches!(op, ISD::Add | ISD::Sub) || !dag.has_one_use(v.node) {
            return None;
        }
        let (mut other, mut one) = (dag.operand(v, 0), dag.operand(v, 1));
        if op == ISD::Sub {
            std::mem::swap(&mut other, &mut one);
        }
        (dag.constant(one) == Some(1)).then_some((op, other))
    };
    if let Some((op, mul_oper)) = add_sub_with_one(dag, n0) {
        let product = binary(dag, ISD::Mul, n1, mul_oper);
        return Some(binary(dag, op, n1, product));
    }
    if let Some((op, mul_oper)) = add_sub_with_one(dag, n1) {
        let product = binary(dag, ISD::Mul, n0, mul_oper);
        return Some(binary(dag, op, n0, product));
    }

    let bits = dag.constant(n1)?;
    let shift_up = 128 - width;
    let c = ((bits << shift_up) as i128) >> shift_up;
    if c == 0 {
        return None;
    }
    let trailing = c.trailing_zeros();
    if trailing > 0 {
        // The multiply may fold into smull/umull, or an add/sub user into madd/msub.
        if dag.has_one_use(n0.node) && matches!(dag.opcode(n0), ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend) {
            return None;
        }
        if dag.has_one_use(node.node) {
            let user = dag.users(node).next().expect("a user");
            let user_value = SDValue { node: user, res: 0 };
            if matches!(dag.opcode(user_value), ISD::Add | ISD::Sub) && !only_used_as_memory_address(dag, user_value) {
                return None;
            }
        }
    }
    let shifted = c >> trailing;
    let is_pow2 = |v: i128| v > 0 && v & (v - 1) == 0;
    let log2 = |v: i128| v.trailing_zeros();
    if c >= 0 {
        if is_pow2(shifted - 1) {
            // (mul x, (2^N + 1) * 2^M) => (shl (add (shl x, N), x), M)
            let a = shl(dag, n0, log2(shifted - 1))?;
            let sum = binary(dag, ISD::Add, a, n0);
            return shl_or_same(dag, sum, trailing);
        } else if is_pow2(c + 1) {
            // (mul x, 2^N - 1) => (sub (shl x, N), x)
            let a = shl(dag, n0, log2(c + 1))?;
            return Some(binary(dag, ISD::Sub, a, n0));
        } else if is_pow2(shifted + 1) {
            // (mul x, (2^(N-M) - 1) * 2^M) => (sub (shl x, N), (shl x, M))
            let hi = shl(dag, n0, log2(shifted + 1) + trailing)?;
            let lo = shl(dag, n0, trailing)?;
            return Some(binary(dag, ISD::Sub, hi, lo));
        }
    } else {
        let shifted_neg_plus1 = -shifted + 1;
        if is_pow2(-c + 1) {
            // (mul x, -(2^N - 1)) => (sub x, (shl x, N))
            let a = shl(dag, n0, log2(-c + 1))?;
            return Some(binary(dag, ISD::Sub, n0, a));
        } else if is_pow2(-c - 1) {
            // (mul x, -(2^N + 1)) => - (add (shl x, N), x)
            let a = shl(dag, n0, log2(-c - 1))?;
            let sum = binary(dag, ISD::Add, a, n0);
            let zero = dag.get_constant(0, vt);
            return Some(binary(dag, ISD::Sub, zero, sum));
        } else if is_pow2(shifted_neg_plus1) {
            // (mul x, -(2^(N-M) - 1) * 2^M) => (sub (shl x, M), (shl x, N))
            let lo = shl(dag, n0, trailing)?;
            let hi = shl(dag, n0, log2(shifted_neg_plus1) + trailing)?;
            return Some(binary(dag, ISD::Sub, lo, hi));
        }
    }
    None
}

/// `Shl(value, amount)` where an amount of zero is the value itself, as the
/// pin's `getNode(SHL, x, 0)` folds.
fn shl_or_same(
    dag: &mut SelectionDAG,
    value: SDValue,
    amount: u32,
) -> Option<SDValue> {
    if amount == 0 { Some(value) } else { shl(dag, value, amount) }
}

/// `AArch64DAGToDAGISel::tryShiftAmountMod`, for the forms that need no new machine node: a
/// shift by `x +/- N` with `N` a multiple of the width, or by `x & mask` whose low bits are all
/// ones through `log2(width)`, shifts by `x`. The register shift takes the amount modulo the
/// width.
///
/// DEVIATE SD504: `N - x` (a `NEG`, a `NOT`) is not done. Exit: a pre-selection hook.
pub fn shift_amount_mod(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (size, bits) = match vt {
        EVT::I32 => (32u128, 5),
        EVT::I64 => (64, 6),
        _ => return None,
    };
    let amount = dag.operand(node, 1);
    let peeled = if matches!(dag.opcode(amount), ISD::ZeroExtend | ISD::AnyExtend) { dag.operand(amount, 0) } else { amount };
    let constant = |dag: &SelectionDAG, value: SDValue| dag.constant(value).map(|c| c & ((1u128 << dag.value_type(value).size_in_bits()) - 1));
    let inner = match dag.opcode(peeled) {
        ISD::Add | ISD::Sub => {
            let (first, second) = (dag.operand(peeled, 0), dag.operand(peeled, 1));
            if constant(dag, second).is_some_and(|c| c % size == 0) { first } else { return None }
        }
        ISD::And => {
            let (first, second) = (dag.operand(peeled, 0), dag.operand(peeled, 1));
            match constant(dag, second) {
                Some(mask) if mask.trailing_ones() >= bits => first,
                _ => return None,
            }
        }
        _ => return None,
    };
    let want = dag.value_type(amount);
    let have = dag.value_type(inner);
    let new_amount = match have.size_in_bits().cmp(&want.size_in_bits()) {
        std::cmp::Ordering::Equal => inner,
        std::cmp::Ordering::Less => dag.get_node(ISD::ZeroExtend, want, &[inner]),
        std::cmp::Ordering::Greater => dag.get_node(ISD::Truncate, want, &[inner]),
    };
    let first = dag.operand(node, 0);
    Some(dag.get_node(dag.opcode(node), vt, &[first, new_amount]))
}

/// `performConditionalSubCombine`: `x - csel(0, q, cc, flags)` where the flags are the `SUBS x, q`
/// is a `csel` between `x` and the difference the `SUBS` already computed.
pub fn conditional_sub(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I32 && vt != EVT::I64 {
        return None;
    }
    let (x, sel) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.opcode(sel) != ISD::Target(a64::CSEL) || !dag.has_one_use(sel.node) {
        return None;
    }
    let flags = dag.operand(sel, 3);
    if dag.opcode(flags) != ISD::Target(a64::SUBS) || dag.operand(flags, 0) != x {
        return None;
    }
    let q = dag.operand(flags, 1);
    let (first, second) = (dag.operand(sel, 0), dag.operand(sel, 1));
    let zero_on_true = dag.is_null_constant(first) && second == q;
    let zero_on_false = dag.is_null_constant(second) && first == q;
    if !zero_on_true && !zero_on_false {
        return None;
    }
    let diff = SDValue { node: flags.node, res: 0 };
    let cc = dag.operand(sel, 2);
    let (on_true, on_false) = if zero_on_true { (x, diff) } else { (diff, x) };
    Some(dag.get_node(ISD::Target(a64::CSEL), vt, &[on_true, on_false, cc, flags]))
}

fn is_shifted_mask(x: u64) -> bool {
    x != 0 && {
        let filled = x | (x - 1);
        filled.wrapping_add(1) & filled == 0
    }
}

/// `AArch64TargetLowering::targetShrinkDemandedConstant` and `optimizeLogicalImm`: the undemanded
/// bits of an `and`/`or`/`xor` mask are set so it is a bitmask immediate, and the node becomes the
/// `ANDWri`-family machine node, so no `MOVi32imm` is needed. `None` is "not handled".
pub fn shrink_demanded_constant(
    dag: &mut SelectionDAG,
    op: SDValue,
    demanded: u128,
    level: CombineLevel,
) -> Option<Option<SDValue>> {
    use llrm_dagisel::generated::aarch64::ids::opcode as machine;
    // Delay this optimization to as late as possible (`TLO.LegalOps`).
    if level < CombineLevel::AfterLegalizeDag {
        return None;
    }
    let vt = dag.value_type(op);
    let size = vt.size_in_bits();
    if (size != 32 && size != 64) || !vt.is_integer() {
        return None;
    }
    let orig_mask = if size == 64 { u64::MAX } else { (1u64 << size) - 1 };
    if demanded as u64 & orig_mask == orig_mask {
        return None;
    }
    let new_opcode = match (dag.opcode(op), size) {
        (ISD::And, 32) => machine::ANDWri,
        (ISD::And, _) => machine::ANDXri,
        (ISD::Or, 32) => machine::ORRWri,
        (ISD::Or, _) => machine::ORRXri,
        (ISD::Xor, 32) => machine::EORWri,
        (ISD::Xor, _) => machine::EORXri,
        _ => return None,
    };
    let old = dag.constant(dag.operand(op, 1))? as u64;
    let mut imm = old;
    let mut mask = orig_mask;
    if imm == 0 || imm == mask || crate::mc::immediate::logical(imm & mask, size).is_some() {
        return None;
    }
    let mut elt = size;
    let mut demanded = demanded as u64;
    // Clear bits that are not demanded.
    imm &= demanded;
    let mut new;
    loop {
        // The non-demanded bits take the value of the preceding demanded bits, so the immediate
        // switches between 0 and 1 as little as possible.
        let non_demanded = !demanded;
        let inverted = !imm & demanded;
        let rotated = ((inverted << 1) | ((inverted >> (elt - 1)) & 1)) & non_demanded;
        let sum = rotated.wrapping_add(non_demanded);
        let carry = non_demanded & !sum & (1u64 << (elt - 1)) != 0;
        let ones = sum.wrapping_add(u64::from(carry)) & non_demanded;
        new = (imm | ones) & mask;
        if is_shifted_mask(new) || is_shifted_mask(!(new | !mask)) {
            break;
        }
        if elt == 2 {
            return None;
        }
        elt /= 2;
        mask >>= elt;
        let hi = imm >> elt;
        let demanded_hi = demanded >> elt;
        if (imm ^ hi) & (demanded & demanded_hi) & mask != 0 {
            return None;
        }
        imm |= hi;
        demanded |= demanded_hi;
    }
    // Replicate the element across the register width.
    while elt < size {
        new |= new << elt;
        elt *= 2;
    }
    if new == old {
        return None;
    }
    let x = dag.operand(op, 0);
    // All zeros or all ones is left to the target-independent combines; otherwise a machine node
    // keeps them from undoing this.
    if new == 0 || new == orig_mask {
        let constant = dag.get_constant(u128::from(new), vt);
        return Some(Some(dag.get_node(dag.opcode(op), vt, &[x, constant])));
    }
    let encoded = crate::mc::immediate::logical(new, size)?;
    let encoded = dag.get_constant_with(u128::from(encoded), vt, true, false);
    Some(Some(dag.get_node(ISD::Machine(u32::from(new_opcode)), vt, &[x, encoded])))
}
