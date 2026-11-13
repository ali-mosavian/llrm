//! `visitSDIV`, `visitUDIV` and `visitREM` with the division-by-constant
//! logic: `BuildSDIV` and `BuildUDIV` (multiply-high and shifts, magic
//! numbers from [`super::div_const`]) and the power-of-two shifts.
//!
//! DEVIATE SD206: integer types the target holds in a register only (no
//! promoted-type multiply, no `BuildExactSDIV`/`BuildExactUDIV` for `exact`
//! divides), no `DIVREM` merging, and
//! `BuildSDIVPow2` is the generic shift sequence except where the target's
//! `sdiv_pow2_with_cmov` makes the pin's `cmov` form (x86 with `cmov`, divisor
//! not 2 or -2). `isIntDivCheap` is the
//! configured `int_div_cheap` (the pin's `minsize`; the `-Os` rule of the
//! target is not wired). Exit: the promoted and exact forms, the `-Os` rule.
//! Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::visitSDIV`
//! ([L5294](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5294)),
//! `visitUDIV`
//! ([L5452](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5452)),
//! `TargetLowering::BuildSDIV`
//! ([L6749](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L6749)),
//! `BuildUDIV`
//! ([L6938](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L6938)).

use super::dag::{SDNodeFlags, SDValue};
use super::dag_combiner::{CombineHooks, CombineLevel, DagCombiner};
use super::div_const::{signed_magic, unsigned_magic};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

/// One unit's prices for the divide-or-sequence choice (clocks or bytes).
struct DivSet {
    prefix: i64,
    add: i64,
    shift: i64,
    extend: i64,
    multiply: i64,
    multiply_long: i64,
    multiply_high: i64,
    divide: i64,
    divide_long: i64,
}

impl<H: CombineHooks> DagCombiner<'_, H> {
    /// `isOperationLegalOrCustom(op, vt, IsAfterLegalization)`, and the type
    /// legal; false without a table.
    fn can_do(
        &self,
        op: ISD,
        vt: EVT,
    ) -> bool {
        let Some(tli) = self.tli else { return false };
        if self.level >= CombineLevel::AfterLegalizeVectorOps {
            tli.is_operation_legal(op, vt)
        } else {
            tli.is_operation_legal_or_custom(op, vt)
        }
    }

    pub(super) fn shift_amount(
        &mut self,
        shifted: EVT,
        count: u32,
    ) -> SDValue {
        let ty = self.hooks.shift_amount_type(shifted);
        self.dag.get_constant(u128::from(count), ty)
    }

    /// The high half of `x * y` (`GetMULHS`/`GetMULHU`): the node, or the
    /// second result of the double-width multiply.
    fn multiply_high(
        &mut self,
        signed: bool,
        vt: EVT,
        x: SDValue,
        y: SDValue,
    ) -> Option<SDValue> {
        let (high, lohi) = if signed { (ISD::MulHS, ISD::SMulLoHi) } else { (ISD::MulHU, ISD::UMulLoHi) };
        let (x, y) = if self.hooks.multiply_high_magic_first(&*self.dag, signed, x, y) { (y, x) } else { (x, y) };
        if self.can_do(high, vt) {
            return Some(self.dag.get_node(high, vt, &[x, y]));
        }
        if self.can_do(lohi, vt) {
            let both = self.dag.get_node_multi(lohi, &[vt, vt], &[x, y]);
            return Some(SDValue { node: both.node, res: 1 });
        }
        // Neither: "if the type twice as wide is legal, widen and use a mul
        // plus a shift" (`MulVT = WideVT`, [L6798][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L6798
        let bits = vt.size_in_bits();
        let wide = EVT::integer(bits * 2);
        if (bits <= 16 || self.hooks.widen_multiply_high_any_width()) && self.can_do(ISD::Mul, wide) {
            let extend = if signed { ISD::SignExtend } else { ISD::ZeroExtend };
            let (x, y) = (self.dag.get_node(extend, wide, &[x]), self.dag.get_node(extend, wide, &[y]));
            let product = self.dag.get_node(ISD::Mul, wide, &[x, y]);
            let amount = self.shift_amount(wide, bits);
            let high = self.dag.get_node(ISD::Srl, wide, &[product, amount]);
            return Some(self.dag.get_node(ISD::Truncate, vt, &[high]));
        }
        None
    }

    fn is_signed_min(
        value: u128,
        bits: u32,
    ) -> bool {
        value == 1u128 << (bits - 1)
    }

    /// `visitSDIV`.
    pub(super) fn visit_sdiv(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_some() {
            return Some(self.dag.get_node(ISD::SDiv, vt, &[n0, n1]));
        }
        let c1 = self.dag.constant(n1);
        if c1 == Some(super::fold::mask(bits)) {
            return Some(self.dag.get_negative(n0, vt));
        }
        if c1.is_some_and(|c| Self::is_signed_min(c, bits)) {
            let test = self.dag.get_setcc(EVT::I8, n0, n1, CondCode::SetEq);
            let (one, zero) = (self.dag.get_constant(1, vt), self.dag.get_constant(0, vt));
            return Some(self.dag.get_select(vt, test, one, zero));
        }
        if self.sign_bit_is_zero(n1) && self.sign_bit_is_zero(n0) {
            return Some(self.dag.get_node(ISD::UDiv, vt, &[n0, n1]));
        }
        if let Some(narrow) = self.dag.config().narrow_divrem
            && let Some(narrowed) = super::narrow_divrem::narrow_sdivrem(self.dag, node, narrow)
        {
            return Some(narrowed);
        }
        self.visit_sdiv_like(n0, n1, node).or_else(|| self.divrem_of_div(node, n1))
    }

    fn sign_bit_is_zero(
        &self,
        value: SDValue,
    ) -> bool {
        let bits = self.dag.value_type(value).size_in_bits();
        super::known_bits::known_bits(self.dag, value).zero >> (bits - 1) & 1 == 1
    }

    /// `isIntDivCheap(VT)`: the configured flag, for every type (`i8` selects
    /// `DIV8r`/`IDIV8r`, SD302).
    fn int_div_cheap(
        &self,
        _vt: EVT,
    ) -> bool {
        self.dag.config().int_div_cheap
    }

    /// DEVIATE DIV-LIVE: the divide stays (`isIntDivCheap`) unless a narrow
    /// unsigned one has more values live across it than the registers it leaves
    /// and its reciprocal is the plain multiply and shift: the `div` pins eax,
    /// edx and a divisor register, the sequence none.
    fn divide_stays(
        &self,
        vt: EVT,
        n0: SDValue,
        n1: SDValue,
        signed: bool,
    ) -> bool {
        let config = self.dag.config();
        if !self.int_div_cheap(vt) {
            return false;
        }
        let bits = vt.size_in_bits();
        if config.div_live_limit == 0 || bits > config.div_live_max_bits || config.div_live <= config.div_live_limit {
            return true;
        }
        let Some(d) = self.dag.constant(n1).map(|c| c & super::fold::mask(bits)) else { return true };
        if d <= 1 || signed && d == super::fold::mask(bits) {
            return true;
        }
        // The plain unsigned reciprocal: one widening multiply and a shift,
        // no add fixup and no pre-shift (the sheet's byte prices overstate
        // the longer sequences, and a signed one carries a sign fixup).
        if signed {
            return true;
        }
        let known = super::known_bits::known_bits(self.dag, n0);
        let zeros = (0..bits).rev().take_while(|&bit| known.zero >> bit & 1 == 1).count() as u32;
        let found = unsigned_magic(d, bits, zeros.min(bits - (128 - d.leading_zeros())), true, false);
        found.is_add || found.pre_shift != 0
    }

    /// Whether a live remainder of the same operands sits beside the divide
    /// `node` would expand: `x / c` and `x % c` share one quotient, so the
    /// pair is priced as the remainder's sequence (quotient, multiply,
    /// subtract) against the one `div` that leaves both, and decided once.
    /// DEVIATE SD445 priced each node alone: the divide took the multiply-high
    /// sequence and the remainder, dearer than its `div`, kept the `div`, so
    /// both ran.
    fn has_sibling_rem(
        &self,
        n0: SDValue,
        n1: SDValue,
        signed: bool,
    ) -> bool {
        let remainder = if signed { ISD::SRem } else { ISD::URem };
        self.dag
            .users(n0)
            .any(
                |user| {
                    let at = SDValue { node: user, res: 0 };
                    self.dag.opcode(at) == remainder
                        && self.dag.operand(at, 0) == n0
                        && self.dag.operand(at, 1) == n1
                        && self.dag.has_users(at)
                },
            )
    }

    /// `isIntDivCheap(VT)` priced per divisor: whether the target's
    /// `div`/`idiv` of a `vt` value costs fewer clocks than `BuildSDIV`/
    /// `BuildUDIV`'s multiply-high sequence for the constant `n1` (and, for a
    /// remainder, the multiply and subtract that follow it). The sequence's
    /// parts are counted from the same magic numbers the build uses, each
    /// priced as the sheet prices it, with the operand-size prefix where the
    /// width is not the target's own. O(1) per node: the magic numbers take
    /// O(bits). DEVIATE SD445: the pin's hook is `minsize` alone. The target's
    /// `divide_margin` keeps the sequence where the prices differ by less.
    fn hardware_divide_wins(
        &self,
        vt: EVT,
        n0: SDValue,
        n1: SDValue,
        signed: bool,
        rem: bool,
    ) -> bool {
        let Some(p) = self.dag.config().div_prices.clone() else { return false };
        let bits = vt.size_in_bits();
        let width = i64::from(bits / 8);
        // A byte divide has no selected form, and the double word of a pair is
        // a call.
        if !(2..=4).contains(&width) || bits % 8 != 0 {
            return false;
        }
        let Some(d) = self.dag.constant(n1).map(|c| c & super::fold::mask(bits)) else { return false };
        if d <= 1 || signed && d == super::fold::mask(bits) {
            return false;
        }
        let known = super::known_bits::known_bits(self.dag, n0);
        let zeros = (0..bits).rev().take_while(|&bit| known.zero >> bit & 1 == 1).count() as u32;
        let clocks = Self::sequence_price(
            &DivSet {
                prefix: p.prefix,
                add: p.add,
                shift: p.shift,
                extend: p.extend,
                multiply: p.multiply,
                multiply_long: p.multiply_long,
                multiply_high: p.multiply_high,
                divide: p.divide,
                divide_long: p.divide_long,
            },
            p.operand,
            p.product.as_ref().map(|price| (price.0)(d as i64)),
            (bits, d, signed, rem, zeros),
        );
        let Some(b) = &p.bytes else { return clocks.0 + clocks.0 * p.margin / 100 < clocks.1 };
        // DEVIATE SD446: at speed on a target that trades bytes for clocks the
        // choice is clocks plus bytes at the rate, the margin on the clocks
        // alone (their spread; bytes are exact). The pin's `isIntDivCheap` is
        // `minsize` alone.
        let bytes = Self::sequence_price(
            &DivSet {
                prefix: b.prefix,
                add: b.add,
                shift: b.shift,
                extend: b.extend,
                multiply: b.multiply,
                multiply_long: b.multiply_long,
                multiply_high: b.multiply_high,
                divide: b.divide,
                divide_long: b.divide_long,
            },
            p.operand,
            None,
            (bits, d, signed, rem, zeros),
        );
        let rate = b.milliclocks_per_byte;
        // The clocks recur with the block, the bytes are paid once.
        let times = ((b.frequency * 1000.0).round() as i64).max(1);
        (clocks.0 + clocks.0 * p.margin / 100) * times + bytes.0 * rate < clocks.1 * times + bytes.1 * rate
    }

    /// The divide's price and the multiply-high sequence's, in `set`'s unit:
    /// `(hardware, sequence)`. `immediate` prices the `imul` by the divisor a
    /// remainder subtracts (`None`: the multiply or a shift-add chain, the
    /// cheaper).
    fn sequence_price(
        set: &DivSet,
        operand: i64,
        immediate: Option<i64>,
        (bits, d, signed, rem, zeros): (u32, u128, bool, bool, u32),
    ) -> (i64, i64) {
        let width = i64::from(bits / 8);
        let sized = |price: i64| price + if width != operand { set.prefix } else { 0 };
        let wide = |price: i64| price + if 4 != operand { set.prefix } else { 0 };
        let hardware =
            sized(if width == 4 { set.divide_long } else { set.divide }) + if signed { set.extend } else { set.add };
        // A multiply-high of a narrower value is the widened product, its high
        // half taken.
        let high = wide(set.multiply_high) + if width < 4 { set.extend + set.shift } else { 0 };
        let (add, shift) = (sized(set.add), sized(set.shift));
        let magic = if signed {
            let found = signed_magic(d, bits);
            let top = 1u128 << (bits - 1);
            let factor = (d & top != 0) != (found.magic & top != 0);
            high + if factor { add } else { 0 } + if found.shift != 0 { shift } else { 0 } + shift + add
        } else {
            let found = unsigned_magic(d, bits, zeros.min(bits - (128 - d.leading_zeros())), true, false);
            (if found.pre_shift != 0 { shift } else { 0 })
                + high
                + if found.is_add { 2 * add + shift } else { 0 }
                + if found.post_shift != 0 { shift } else { 0 }
        };
        // The remainder is `x - q * d`: the product by a constant, the `imul`
        // the selector emits for it.
        let product = immediate.map_or_else(
            || sized(if width == 4 { set.multiply_long } else { set.multiply }).min(2 * shift + 2 * add),
            sized,
        );
        (hardware, magic + if rem { product + add } else { 0 })
    }

    /// `visitSDIVLike`.
    pub(super) fn visit_sdiv_like(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        let exact = self.dag.node(node).flags.0 & SDNodeFlags::EXACT.0 != 0;
        let divisor = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1));
        if let Some(c) = divisor.filter(|&c| c != 0 && !exact) {
            let negated = c.wrapping_neg() & super::fold::mask(bits);
            if c.is_power_of_two() || negated.is_power_of_two() {
                // `BuildSDIVPow2` of the default hook: `SDValue(N, 0)`, the
                // divide as it is.
                if self.dag.config().sdiv_pow2_stays {
                    return Some(node);
                }
                // `X86TargetLowering::BuildSDIVPow2` ->
                // `buildSDIVPow2WithCMov` (TargetLowering.cpp#L6710): a
                // divisor of 2, -2, 1 or -1 keeps the default expansion.
                let log = c.trailing_zeros();
                if self.tli.is_some_and(|tli| tli.sdiv_pow2_with_cmov(vt)) && c != 1 && negated != 1 && log > 1 {
                    // `(x < 0 ? x + (2^log - 1) : x) >> log`, negated for a
                    // negative divisor.
                    let zero = self.dag.get_constant(0, vt);
                    let low = self.dag.get_constant((1u128 << log) - 1, vt);
                    let negative_x = self.dag.get_setcc(EVT::I8, n0, zero, CondCode::SetLt);
                    let biased = self.dag.get_node(ISD::Add, vt, &[n0, low]);
                    let chosen = self.dag.get_select(vt, negative_x, biased, n0);
                    let by = self.shift_amount(vt, log);
                    let quotient = self.dag.get_node(ISD::Sra, vt, &[chosen, by]);
                    let negative_divisor = c >> (bits - 1) & 1 == 1;
                    return Some(if negative_divisor { self.dag.get_negative(quotient, vt) } else { quotient });
                }
                // `Sign = x >> (bits-1)`, add `Sign >>> (bits - log2 |d|)`,
                // shift right by `log2 |d|`; a negative divisor
                // negates.
                let (top, inexact, by) =
                    (self.shift_amount(vt, bits - 1), self.shift_amount(vt, bits - log), self.shift_amount(vt, log));
                let sign = self.dag.get_node(ISD::Sra, vt, &[n0, top]);
                let srl = self.dag.get_node(ISD::Srl, vt, &[sign, inexact]);
                let sum = self.dag.get_node(ISD::Add, vt, &[n0, srl]);
                let quotient = if c == 1 || negated == 1 { n0 } else { self.dag.get_node(ISD::Sra, vt, &[sum, by]) };
                let negative = c >> (bits - 1) & 1 == 1;
                return Some(if negative { self.dag.get_negative(quotient, vt) } else { quotient });
            }
        }
        if self.dag.constant(n1).is_some()
            && !self.divide_stays(vt, n0, n1, true)
            && !self.hardware_divide_wins(vt, n0, n1, true, self.has_sibling_rem(n0, n1, true))
        {
            // DEVIATE SD-DIVMAG: by a negative divisor the quotient is the
            // negation of the one by its magnitude, which the remainder
            // (`x % -c` is `x % c`) already multiplies by: one multiply-high.
            if self.dag.config().sdiv_by_magnitude
                && let Some(c) = self.dag.constant(n1)
                && c >> (bits - 1) & 1 == 1
                && !Self::is_signed_min(c, bits)
                && !self.dag.is_opaque(n1)
            {
                let magnitude = self.dag.get_constant(c.wrapping_neg() & super::fold::mask(bits), vt);
                let quotient = self.dag.get_node(ISD::SDiv, vt, &[n0, magnitude]);
                return Some(self.dag.get_negative(quotient, vt));
            }
            return self.build_sdiv(n0, n1, node);
        }
        None
    }

    /// `TargetLowering::BuildSDIV`, for a type the target holds.
    fn build_sdiv(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if self.dag.node(node).flags.0 & SDNodeFlags::EXACT.0 != 0 || !self.tli?.types.is_type_legal(vt) {
            return None;
        }
        let mask = super::fold::mask(bits);
        let d = self.dag.constant(n1)? & mask;
        if d == 0 {
            return None;
        }
        let (magic, factor, shift, sign_mask) = if d == 1 || d == mask {
            (0, d, 0, 0)
        } else {
            let found = signed_magic(d, bits);
            let top = 1u128 << (bits - 1);
            let factor = match (d & top != 0, found.magic & top != 0) {
                (false, true) => 1,
                (true, false) => mask,
                _ => 0,
            };
            (found.magic, factor, found.shift, mask)
        };
        let magic = self.dag.get_constant(magic, vt);
        let high = self.multiply_high(true, vt, n0, magic)?;
        let factor = self.dag.get_constant(factor, vt);
        let scaled = self.dag.get_node(ISD::Mul, vt, &[n0, factor]);
        let sum = self.dag.get_node(ISD::Add, vt, &[high, scaled]);
        let by = self.shift_amount(vt, shift);
        let quotient = self.dag.get_node(ISD::Sra, vt, &[sum, by]);
        let top = self.shift_amount(vt, bits - 1);
        let sign = self.dag.get_node(ISD::Srl, vt, &[quotient, top]);
        let sign_mask = self.dag.get_constant(sign_mask, vt);
        let sign = self.dag.get_node(ISD::And, vt, &[sign, sign_mask]);
        Some(self.dag.get_node(ISD::Add, vt, &[quotient, sign]))
    }

    /// `visitUDIV`.
    pub(super) fn visit_udiv(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_some() {
            return Some(self.dag.get_node(ISD::UDiv, vt, &[n0, n1]));
        }
        if self.dag.constant(n1) == Some(super::fold::mask(vt.size_in_bits())) {
            let test = self.dag.get_setcc(EVT::I8, n0, n1, CondCode::SetEq);
            let (one, zero) = (self.dag.get_constant(1, vt), self.dag.get_constant(0, vt));
            return Some(self.dag.get_select(vt, test, one, zero));
        }
        if let Some(narrow) = self.dag.config().narrow_divrem
            && let Some(narrowed) = super::narrow_divrem::narrow_udivrem(self.dag, node, narrow)
        {
            return Some(narrowed);
        }
        self.visit_udiv_like(n0, n1, node).or_else(|| self.divrem_of_div(node, n1))
    }

    /// `visitUDIVLike`.
    pub(super) fn visit_udiv_like(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if let Some(c) = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1)) {
            if c.is_power_of_two() {
                let by = self.shift_amount(vt, c.trailing_zeros());
                return Some(self.dag.get_node(ISD::Srl, vt, &[n0, by]));
            }
        }
        // `udiv x, (shl c, y)` with a power of two `c`: `x >>> (log2 c + y)`.
        if self.dag.opcode(n1) == ISD::Shl {
            let (c, y) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
            if let Some(c) = self.dag.constant(c).filter(|&c| c.is_power_of_two() && !self.dag.is_opaque(n1)) {
                let amount_vt = self.dag.value_type(y);
                let log = self.dag.get_constant(u128::from(c.trailing_zeros()), amount_vt);
                let total = self.dag.get_node(ISD::Add, amount_vt, &[y, log]);
                return Some(self.dag.get_node(ISD::Srl, vt, &[n0, total]));
            }
        }
        if self.dag.constant(n1).is_some()
            && !self.divide_stays(vt, n0, n1, false)
            && !self.hardware_divide_wins(vt, n0, n1, false, self.has_sibling_rem(n0, n1, false))
        {
            return self.build_udiv(n0, n1, node);
        }
        None
    }

    /// `TargetLowering::BuildUDIV`, for a type the target holds.
    fn build_udiv(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if self.dag.node(node).flags.0 & SDNodeFlags::EXACT.0 != 0 || !self.tli?.types.is_type_legal(vt) {
            return None;
        }
        let d = self.dag.constant(n1)? & super::fold::mask(bits);
        if d <= 1 {
            return None;
        }
        let known = super::known_bits::known_bits(self.dag, n0);
        let known_zeros = (0..bits).rev().take_while(|&bit| known.zero >> bit & 1 == 1).count() as u32;
        let leading = known_zeros.min(bits - (128 - d.leading_zeros()));
        // [L7031][pin]: an `i32` divisor with no cheap magic is widened
        // to `i64` when the wide high multiply exists. The even-divisor rewrite
        // is kept unless the zero extension is free.
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L7031
        let widened = self.hooks.widened_unsigned_divide_type(vt);
        let wide = widened.unwrap_or(vt);
        let has_wide_mulhu = widened.is_some() && self.can_do(ISD::MulHU, wide);
        let has_wide_lohi = widened.is_some() && self.can_do(ISD::UMulLoHi, wide);
        let allow_widen = has_wide_mulhu || has_wide_lohi;
        let allow_even = !(allow_widen && self.hooks.zero_extend_i32_to_i64_free());
        let found = unsigned_magic(d, bits, leading, allow_even, allow_widen);
        if found.widen {
            let extended = self.dag.get_node(ISD::ZeroExtend, wide, &[n0]);
            let magic = self.dag.get_constant(found.magic, wide);
            let high = if has_wide_mulhu {
                self.dag.get_node(ISD::MulHU, wide, &[extended, magic])
            } else {
                let both = self.dag.get_node_multi(ISD::UMulLoHi, &[wide, wide], &[extended, magic]);
                SDValue { node: both.node, res: 1 }
            };
            return Some(self.dag.get_node(ISD::Truncate, vt, &[high]));
        }
        let mut quotient = n0;
        if found.pre_shift != 0 {
            let by = self.shift_amount(vt, found.pre_shift);
            quotient = self.dag.get_node(ISD::Srl, vt, &[quotient, by]);
        }
        let magic = self.dag.get_constant(found.magic, vt);
        quotient = self.multiply_high(false, vt, quotient, magic)?;
        if found.is_add {
            let difference = self.dag.get_node(ISD::Sub, vt, &[n0, quotient]);
            let one = self.shift_amount(vt, 1);
            let half = self.dag.get_node(ISD::Srl, vt, &[difference, one]);
            quotient = self.dag.get_node(ISD::Add, vt, &[half, quotient]);
        }
        if found.post_shift != 0 {
            let by = self.shift_amount(vt, found.post_shift);
            quotient = self.dag.get_node(ISD::Srl, vt, &[quotient, by]);
        }
        Some(quotient)
    }

    /// `visitREM`: `x % c` is `x - x / c * c` when the quotient has a cheap
    /// form.
    pub(super) fn visit_rem(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let signed = op == ISD::SRem;
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_some() {
            return Some(self.dag.get_node(op, vt, &[n0, n1]));
        }
        if signed {
            if self.sign_bit_is_zero(n1) && self.sign_bit_is_zero(n0) {
                return Some(self.dag.get_node(ISD::URem, vt, &[n0, n1]));
            }
            if let Some(narrow) = self.dag.config().narrow_divrem
                && let Some(narrowed) = super::narrow_divrem::narrow_sdivrem(self.dag, node, narrow)
            {
                return Some(narrowed);
            }
        } else if self.dag.constant(n1).is_some_and(|c| c.is_power_of_two()) {
            let ones = self.dag.get_all_ones_constant(vt);
            let below = self.dag.get_node(ISD::Add, vt, &[n1, ones]);
            return Some(self.dag.get_node(ISD::And, vt, &[n0, below]));
        } else if let Some(narrow) = self.dag.config().narrow_divrem
            && let Some(narrowed) = super::narrow_divrem::narrow_udivrem(self.dag, node, narrow)
        {
            return Some(narrowed);
        }
        if self.dag.constant(n1).is_none_or(|c| c == 0) {
            return None;
        }
        // "Checking cheapness here makes sense since the simplification
        // results in fatter code."
        if self.divide_stays(vt, n0, n1, signed) || self.hardware_divide_wins(vt, n0, n1, signed, true) {
            return None;
        }
        let quotient = if signed { self.visit_sdiv_like(n0, n1, node) } else { self.visit_udiv_like(n0, n1, node) };
        let Some(quotient) = quotient else {
            // sdiv, srem -> sdivrem
            return self.use_div_rem(node).map(|both| SDValue { node: both.node, res: 1 });
        };
        let product = self.dag.get_node(ISD::Mul, vt, &[quotient, n1]);
        Some(self.dag.get_node(ISD::Sub, vt, &[n0, product]))
    }

    /// The end of `visitSDIV` and `visitUDIV`: "If the divisor is constant,
    /// then return DIVREM only if isIntDivCheap() is true. Otherwise, we break
    /// the simplification logic in visitREM()."
    fn divrem_of_div(
        &mut self,
        node: SDValue,
        divisor: SDValue,
    ) -> Option<SDValue> {
        if self.dag.constant(divisor).is_some() && !self.int_div_cheap(self.dag.value_type(node)) {
            return None;
        }
        self.use_div_rem(node)
    }

    /// `DAGCombiner::useDivRem`
    /// ([L5181](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5181)):
    /// a divide and a remainder of the same operands are one `DIVREM` node,
    /// when the target has it (legal or custom) or a library call that makes
    /// both (`isDivRemLibcallAvailable`: the target names a routine for it).
    /// Every divide and remainder of the pair is rewritten to read it.
    pub(super) fn use_div_rem(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        use super::actions::Action;
        let op = self.dag.opcode(node);
        let signed = matches!(op, ISD::SDiv | ISD::SRem);
        let divrem = if signed { ISD::SDivRem } else { ISD::UDivRem };
        let vt = self.dag.value_type(node);
        if !vt.is_integer() || !self.dag.has_users(node) {
            return None;
        }
        let tli = self.tli?;
        if !tli.types.is_type_legal(vt) && tli.operation_action(divrem, vt) != Action::Custom {
            return None;
        }
        if !tli.is_operation_legal_or_custom(divrem, vt) && tli.libcall_name(divrem, vt).is_none() {
            return None;
        }
        // "If div is legal, it's better to do the normal expansion"
        let (quotient_op, remainder_op) = if signed { (ISD::SDiv, ISD::SRem) } else { (ISD::UDiv, ISD::URem) };
        let other = if op == quotient_op { remainder_op } else { quotient_op };
        let checked = if op == quotient_op { op } else { other };
        if tli.is_operation_legal_or_custom(checked, vt) {
            return None;
        }
        let (op0, op1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let users: Vec<u32> = self.dag.users(op0).collect();
        let mut combined: Option<SDValue> = None;
        for user in users {
            let at = SDValue { node: user, res: 0 };
            let used = (0..self.dag.node(at).values.len())
                .any(|res| self.dag.has_users(SDValue { node: user, res: res as u8 }));
            if user == node.node || !used {
                continue;
            }
            let user_op = self.dag.opcode(at);
            if !(user_op == op || user_op == other || user_op == divrem)
                || self.dag.operand(at, 0) != op0
                || self.dag.operand(at, 1) != op1
            {
                continue;
            }
            if combined.is_none() {
                if user_op == other {
                    combined = Some(self.dag.get_node_multi(divrem, &[vt, vt], &[op0, op1]));
                } else if user_op == divrem {
                    combined = Some(at);
                } else {
                    continue;
                }
            }
            let both = combined?;
            if user_op == quotient_op {
                self.dag.combine_to(at, SDValue { node: both.node, res: 0 });
            } else if user_op == remainder_op {
                self.dag.combine_to(at, SDValue { node: both.node, res: 1 });
            }
        }
        combined
    }
}
