//! Ranges and no-wrap flag inference (S2): `getRangeRef` and what it is made
//! of, `getConstantMultiple`, `setNoWrapFlags`, `inferNoWrapViaConstantRanges`.

use llrm_mir::constantrange::{ConstantRange, PreferredRangeType, RangeIntrinsic};
use llrm_mir::hash::HashSet;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::known::{compute_known_bits, pointer_alignment, pointer_dereferenceable_bytes};
use llrm_mir::valuetracking::nonzero::is_known_non_zero;
use llrm_mir::valuetracking::signbits::compute_num_sign_bits;

use super::ap::Ap;
use super::backedge::ExitCountKind;
use super::expr::{Kind, Scev, Ty, Type, Use, flags};
use super::{Cx, ScalarEvolution};

/// `scev-range-iter-threshold`.
const RANGE_ITER_THRESHOLD: u32 = 32;

/// `ScalarEvolution::RangeSignHint`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeSignHint {
    Unsigned,
    Signed,
}

fn ap_of(
    width: u32,
    bits: u128,
) -> Ap {
    Ap::new(width, bits)
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6307
    /// `getConstantMultipleImpl`.
    fn get_constant_multiple_impl(
        &mut self,
        cx: &Cx,
        s: Scev,
        mut ctx_i: Option<InstId>,
    ) -> Ap {
        let bit_width = self.get_type_size_in_bits(cx, self.type_of(s));
        let shifted_by_zeros = |trailing_zeros: u32| -> Ap {
            if trailing_zeros >= bit_width { Ap::zero(bit_width) } else { Ap::one(bit_width).shl(trailing_zeros) }
        };

        match self.node(s).kind.clone() {
            Kind::Constant(value) => value,
            Kind::PtrToAddr(op) => self.get_constant_multiple(cx, op.scev, None),
            Kind::UDiv(..) => Ap::one(bit_width),
            Kind::Truncate(op) => {
                // Only multiples that are a power of 2 will hold after
                // truncation.
                let tz = self.get_min_trailing_zeros(cx, op.scev, ctx_i);
                shifted_by_zeros(tz)
            }
            Kind::ZeroExtend(op) => self.get_constant_multiple(cx, op.scev, ctx_i).zext(bit_width),
            Kind::SignExtend(op) => {
                // Only multiples that are a power of 2 will hold after sext.
                let tz = self.get_min_trailing_zeros(cx, op.scev, ctx_i);
                shifted_by_zeros(tz)
            }
            Kind::Mul(ops) => {
                if self.get_no_wrap_flags(s, flags::NUW) != 0 {
                    // The result is the product of all operand results.
                    let mut res = self.get_constant_multiple(cx, ops[0].scev, ctx_i);
                    for operand in &ops[1..] {
                        res = res.mul(self.get_constant_multiple(cx, operand.scev, ctx_i));
                    }
                    return res;
                }

                // If there are no wrap guarentees, find the trailing zeros,
                // which is the sum of trailing zeros for all its operands.
                let mut tz = 0u32;
                for operand in &ops {
                    tz += self.get_min_trailing_zeros(cx, operand.scev, ctx_i);
                }
                shifted_by_zeros(tz)
            }
            Kind::Add(ops) | Kind::AddRec(ops, _) => {
                if self.get_no_wrap_flags(s, flags::NUW) != 0 {
                    return self.gcd_multiple(cx, &ops, ctx_i);
                }
                // Find the trailing bits, which is the minimum of its operands.
                let mut tz = self.get_min_trailing_zeros(cx, ops[0].scev, ctx_i);
                for operand in &ops[1..] {
                    tz = tz.min(self.get_min_trailing_zeros(cx, operand.scev, ctx_i));
                }
                shifted_by_zeros(tz)
            }
            Kind::UMax(ops) | Kind::SMax(ops) | Kind::UMin(ops) | Kind::SMin(ops) | Kind::SequentialUMin(ops) => {
                self.gcd_multiple(cx, &ops, ctx_i)
            }
            Kind::Unknown(value) => {
                // Ask ValueTracking for known bits. SCEVUnknown only become
                // available at the point their underlying IR instruction has
                // been defined. If CtxI was not provided, use:
                // * the first instruction in the entry block if it is an
                //   argument
                // * the instruction itself otherwise.
                if ctx_i.is_none() {
                    if let Operand::Value(id) = value {
                        match cx.function.value(id).def {
                            ValueDef::Argument(_) => {
                                let entry = cx.function.entry().expect("a function with a body");
                                ctx_i = cx.function.block(entry).first();
                            }
                            ValueDef::Instruction(inst) => ctx_i = Some(inst),
                        }
                    }
                }
                let q = cx.query(ctx_i);
                let known = compute_known_bits(&q, value, 0);
                shifted_by_zeros(known.count_min_trailing_zeros())
            }
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    /// `GetGCDMultiple`: the GCD of the operands' multiples.
    fn gcd_multiple(
        &mut self,
        cx: &Cx,
        ops: &[Use],
        ctx_i: Option<InstId>,
    ) -> Ap {
        let mut res = self.get_constant_multiple(cx, ops[0].scev, ctx_i);
        for operand in &ops[1..] {
            if res.is_one() {
                break;
            }
            let other = self.get_constant_multiple(cx, operand.scev, ctx_i);
            let (mut x, mut y) = (res.bits, other.bits);
            while y != 0 {
                (x, y) = (y, x % y);
            }
            res = ap_of(res.width, x);
        }
        res
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6408
    /// `getConstantMultiple`: the largest known divisor of `s` (0 for the
    /// zero constant).
    pub fn get_constant_multiple(
        &mut self,
        cx: &Cx,
        s: Scev,
        ctx_i: Option<InstId>,
    ) -> Ap {
        // Skip looking up and updating the cache if there is a context
        // instruction, as the result will only be valid in the specified
        // context.
        if ctx_i.is_some() {
            return self.get_constant_multiple_impl(cx, s, ctx_i);
        }

        if let Some(&found) = self.constant_multiple_cache.get(&s) {
            return found;
        }

        let result = self.get_constant_multiple_impl(cx, s, ctx_i);
        self.constant_multiple_cache.insert(s, result);
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6425
    /// `getNonZeroConstantMultiple`.
    pub fn get_non_zero_constant_multiple(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> Ap {
        let multiple = self.get_constant_multiple(cx, s, None);
        if multiple.is_zero() { Ap::one(multiple.width) } else { multiple }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6430
    /// `getMinTrailingZeros`.
    pub fn get_min_trailing_zeros(
        &mut self,
        cx: &Cx,
        s: Scev,
        ctx_i: Option<InstId>,
    ) -> u32 {
        let multiple = self.get_constant_multiple(cx, s, ctx_i);
        multiple.countr_zero().min(self.get_type_size_in_bits(cx, self.type_of(s)))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6452
    /// `ScalarEvolution::setNoWrapFlags(SCEVAddRecExpr *, Flags)`: add the
    /// flags to the recurrence and forget what was computed without them.
    pub fn set_no_wrap_flags(
        &mut self,
        s: Scev,
        f: u8,
    ) {
        if self.get_no_wrap_flags(s, f) != f {
            self.node_set_no_wrap_flags(s, f);
            self.unsigned_ranges.remove(&s);
            self.signed_ranges.remove(&s);
            self.constant_multiple_cache.remove(&s);
        }
    }

    /// `SCEVAddRecExpr::setNoWrapFlags`: `NW` follows `NUW` and `NSW`.
    pub(super) fn node_set_no_wrap_flags(
        &mut self,
        s: Scev,
        mut f: u8,
    ) {
        if matches!(self.node(s).kind, Kind::AddRec(..)) && f & (flags::NUW | flags::NSW) != 0 {
            f |= flags::NW;
        }
        self.nodes[s.0 as usize].flags |= f;
    }

    /// `getUnsignedRange`.
    pub fn get_unsigned_range(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> ConstantRange {
        self.get_range_ref(cx, s, RangeSignHint::Unsigned, 0)
    }

    /// `getSignedRange`.
    pub fn get_signed_range(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> ConstantRange {
        self.get_range_ref(cx, s, RangeSignHint::Signed, 0)
    }

    /// `getUnsignedRangeMin` as an `APInt` of the expression's width.
    pub fn get_unsigned_range_min(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> Ap {
        let bits = self.get_type_size_in_bits(cx, self.type_of(s));
        Ap::new(bits, self.get_unsigned_range(cx, s).get_unsigned_min())
    }

    /// `getUnsignedRangeMax`.
    pub fn get_unsigned_range_max(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> Ap {
        let bits = self.get_type_size_in_bits(cx, self.type_of(s));
        Ap::new(bits, self.get_unsigned_range(cx, s).get_unsigned_max())
    }

    /// `getSignedRangeMin`.
    pub fn get_signed_range_min(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> Ap {
        let bits = self.get_type_size_in_bits(cx, self.type_of(s));
        Ap::new(bits, self.get_signed_range(cx, s).get_signed_min())
    }

    /// `getSignedRangeMax`.
    pub fn get_signed_range_max(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> Ap {
        let bits = self.get_type_size_in_bits(cx, self.type_of(s));
        Ap::new(bits, self.get_signed_range(cx, s).get_signed_max())
    }

    /// `setRange`.
    fn set_range(
        &mut self,
        s: Scev,
        hint: RangeSignHint,
        cr: ConstantRange,
    ) -> ConstantRange {
        let cache = match hint {
            RangeSignHint::Unsigned => &mut self.unsigned_ranges,
            RangeSignHint::Signed => &mut self.signed_ranges,
        };
        cache.insert(s, cr);
        cr
    }

    /// `Cache.find(S)`.
    fn cached_range(
        &self,
        s: Scev,
        hint: RangeSignHint,
    ) -> Option<ConstantRange> {
        match hint {
            RangeSignHint::Unsigned => self.unsigned_ranges.get(&s).copied(),
            RangeSignHint::Signed => self.signed_ranges.get(&s).copied(),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6586
    /// `getRangeRefIter`: the range of `s` computed iteratively, operands
    /// first, so that a deeply nested expression does not overflow the stack.
    fn get_range_ref_iter(
        &mut self,
        cx: &Cx,
        s: Scev,
        hint: RangeSignHint,
    ) -> ConstantRange {
        let mut work_list: Vec<Scev> = Vec::new();
        let mut seen: HashSet<Scev> = HashSet::default();

        // Add Expr to the worklist, if Expr is either an N-ary expression or a
        // SCEVUnknown PHI node.
        let add_to_worklist = |se: &Self, work_list: &mut Vec<Scev>, seen: &mut HashSet<Scev>, expr: Scev| {
            if !seen.insert(expr) {
                return;
            }
            if se.cached_range(expr, hint).is_some() {
                return;
            }
            match se.node(expr).kind {
                Kind::Unknown(Operand::Value(id)) => {
                    // Only a PHI node is followed.
                    if let ValueDef::Instruction(inst) = cx.function.value(id).def
                        && cx.function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi
                    {
                        work_list.push(expr);
                    }
                }
                Kind::Unknown(_) => {}
                Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
                _ => work_list.push(expr),
            }
        };
        add_to_worklist(self, &mut work_list, &mut seen, s);

        // Build worklist by queuing operands of N-ary expressions and phi
        // nodes.
        let mut i = 0;
        while i != work_list.len() {
            let p = work_list[i];
            i += 1;
            // If it is not a `SCEVUnknown`, just recurse into operands.
            if !matches!(self.node(p).kind, Kind::Unknown(_)) {
                for op in self.operands_of(p) {
                    add_to_worklist(self, &mut work_list, &mut seen, op.scev);
                }
                continue;
            }
            // `SCEVUnknown`'s require special treatment: the operands of a PHI
            // (SEAM S3: `getSCEV` of them is an unknown until `createSCEV`).
        }

        if work_list.len() > 1 {
            // Use getRangeRef to compute ranges for items in the worklist in
            // reverse order. This will force ranges for earlier operands to be
            // computed before their users in most cases.
            for p in work_list[1..].iter().rev().copied().collect::<Vec<_>>() {
                self.get_range_ref(cx, p, hint, 0);
            }
        }

        self.get_range_ref(cx, s, hint, 0)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6689
    /// `getRangeRef`: the range for a particular SCEV. If `hint` is
    /// `Unsigned` (resp. `Signed`) then it prefers ranges with a "cleaner"
    /// unsigned (resp. signed) representation.
    pub fn get_range_ref(
        &mut self,
        cx: &Cx,
        s: Scev,
        hint: RangeSignHint,
        depth: u32,
    ) -> ConstantRange {
        let range_type = match hint {
            RangeSignHint::Unsigned => PreferredRangeType::Unsigned,
            RangeSignHint::Signed => PreferredRangeType::Signed,
        };

        // See if we've computed this range already.
        if let Some(found) = self.cached_range(s, hint) {
            return found;
        }

        if let Kind::Constant(c) = self.node(s).kind {
            return self.set_range(s, hint, ConstantRange::single(c.width, c.bits));
        }

        // Switch to iteratively computing the range for S, if it is part of a
        // deeply nested expression.
        if depth > RANGE_ITER_THRESHOLD {
            return self.get_range_ref_iter(cx, s, hint);
        }

        let bit_width = self.get_type_size_in_bits(cx, self.type_of(s));
        let mut conservative_result = ConstantRange::full(bit_width);

        // If the value has known zeros, the maximum value will have those known
        // zeros as well.
        if hint == RangeSignHint::Unsigned {
            let multiple = self.get_non_zero_constant_multiple(cx, s);
            let max = Ap::all_ones(bit_width);
            let remainder = max.urem(multiple);
            if !remainder.is_zero() {
                conservative_result = ConstantRange::new(bit_width, 0, max.sub(remainder).add(Ap::one(bit_width)).bits);
            }
        } else {
            let tz = self.get_min_trailing_zeros(cx, s, None);
            if tz != 0 {
                let upper = Ap::signed_max(bit_width).ashr(tz).shl(tz).add(Ap::one(bit_width));
                conservative_result = ConstantRange::new(bit_width, Ap::signed_min(bit_width).bits, upper.bits);
            }
        }

        match self.node(s).kind.clone() {
            Kind::Constant(_) => unreachable!("Already handled above."),
            Kind::Truncate(op) => {
                let x = self.get_range_ref(cx, op.scev, hint, depth + 1);
                self.set_range(s, hint, conservative_result.intersect_with(&x.truncate(bit_width, 0), range_type))
            }
            Kind::ZeroExtend(op) => {
                let x = self.get_range_ref(cx, op.scev, hint, depth + 1);
                self.set_range(s, hint, conservative_result.intersect_with(&x.zero_extend(bit_width), range_type))
            }
            Kind::SignExtend(op) => {
                let x = self.get_range_ref(cx, op.scev, hint, depth + 1);
                self.set_range(s, hint, conservative_result.intersect_with(&x.sign_extend(bit_width), range_type))
            }
            Kind::PtrToAddr(op) => {
                let x = self.get_range_ref(cx, op.scev, hint, depth + 1);
                self.set_range(s, hint, x)
            }
            Kind::Add(ops) => {
                // Check if this is a URem pattern: A - (A / B) * B, which is
                // always < B.
                if hint == RangeSignHint::Unsigned
                    && let Some((urem_lhs, urem_rhs)) = self.match_urem(cx, s)
                {
                    let lhs_range = self.get_range_ref(cx, urem_lhs, hint, depth + 1);
                    let rhs_range = self.get_range_ref(cx, urem_rhs, hint, depth + 1);
                    conservative_result = conservative_result.intersect_with(&lhs_range.urem(&rhs_range), range_type);
                }
                let mut x = self.get_range_ref(cx, ops[0].scev, hint, depth + 1);
                let mut wrap_type = 0u32;
                if self.get_no_wrap_flags(s, flags::NSW) != 0 {
                    wrap_type |= llrm_mir::constantrange::NO_SIGNED_WRAP;
                }
                if self.get_no_wrap_flags(s, flags::NUW) != 0 {
                    wrap_type |= llrm_mir::constantrange::NO_UNSIGNED_WRAP;
                }
                for op in &ops[1..] {
                    let y = self.get_range_ref(cx, op.scev, hint, depth + 1);
                    x = x.add_with_no_wrap(&y, wrap_type, range_type);
                }
                self.set_range(s, hint, conservative_result.intersect_with(&x, range_type))
            }
            Kind::Mul(ops) => {
                let mut x = self.get_range_ref(cx, ops[0].scev, hint, depth + 1);
                for op in &ops[1..] {
                    let y = self.get_range_ref(cx, op.scev, hint, depth + 1);
                    x = x.multiply(&y, 0);
                }
                self.set_range(s, hint, conservative_result.intersect_with(&x, range_type))
            }
            Kind::UDiv(lhs, rhs) => {
                let x = self.get_range_ref(cx, lhs.scev, hint, depth + 1);
                let y = self.get_range_ref(cx, rhs.scev, hint, depth + 1);
                self.set_range(s, hint, conservative_result.intersect_with(&x.udiv(&y), range_type))
            }
            Kind::AddRec(ops, header) => {
                let add_rec = s;
                // If there's no unsigned wrap, the value will never be less
                // than its initial value.
                if self.get_no_wrap_flags(add_rec, flags::NUW) != 0 {
                    let start = self.addrec_start(add_rec);
                    let unsigned_min_value = self.get_unsigned_range_min(cx, start);
                    if !unsigned_min_value.is_zero() {
                        conservative_result = conservative_result
                            .intersect_with(&ConstantRange::new(bit_width, unsigned_min_value.bits, 0), range_type);
                    }
                }

                // If there's no signed wrap, and all the operands except
                // initial value have the same sign or zero, the value won't
                // ever be:
                // 1: smaller than initial value if operands are non negative,
                // 2: bigger than initial value if operands are non positive.
                // For both cases, value can not cross signed min/max boundary.
                if self.get_no_wrap_flags(add_rec, flags::NSW) != 0 {
                    let mut all_non_neg = true;
                    let mut all_non_pos = true;
                    for op in &ops[1..] {
                        if !self.is_known_non_negative(cx, op.scev) {
                            all_non_neg = false;
                        }
                        if !self.is_known_non_positive(cx, op.scev) {
                            all_non_pos = false;
                        }
                    }
                    let start = self.addrec_start(add_rec);
                    if all_non_neg {
                        let min = self.get_signed_range_min(cx, start);
                        conservative_result = conservative_result.intersect_with(
                            &ConstantRange::non_empty(bit_width, min.bits, Ap::signed_min(bit_width).bits),
                            range_type,
                        );
                    } else if all_non_pos {
                        let max = self.get_signed_range_max(cx, start);
                        conservative_result = conservative_result.intersect_with(
                            &ConstantRange::non_empty(
                                bit_width,
                                Ap::signed_min(bit_width).bits,
                                max.add(Ap::one(bit_width)).bits,
                            ),
                            range_type,
                        );
                    }
                }

                // TODO: non-affine addrec
                if ops.len() == 2 {
                    let max_be_scev = self.get_backedge_taken_count(cx, header, ExitCountKind::ConstantMaximum);
                    if let Some(mut max_be_count) = self.as_constant(max_be_scev) {
                        // Adjust MaxBECount to the same bitwidth as AddRec. We
                        // can truncate if MaxBECount's active bits are all <=
                        // AddRec's bit width.
                        if max_be_count.width > bit_width && max_be_count.active_bits() <= bit_width {
                            max_be_count = max_be_count.trunc(bit_width);
                        } else if max_be_count.width < bit_width {
                            max_be_count = max_be_count.zext(bit_width);
                        }

                        if max_be_count.width == bit_width {
                            let start = self.addrec_start(add_rec);
                            let step = self.get_step_recurrence(cx, add_rec);
                            let (range_from_affine, f) = self.get_range_for_affine_ar(cx, start, step, max_be_count);
                            conservative_result = conservative_result.intersect_with(&range_from_affine, range_type);
                            self.node_set_no_wrap_flags(add_rec, f);

                            let range_from_factoring = self.get_range_via_factoring(cx, start, step, max_be_count);
                            conservative_result = conservative_result.intersect_with(&range_from_factoring, range_type);
                        }
                    }

                    // Now try symbolic BE count and more powerful methods.
                    // (`UseExpensiveRangeSharpening` is off by default.)
                }

                self.set_range(s, hint, conservative_result)
            }
            Kind::UMax(ops) | Kind::SMax(ops) | Kind::UMin(ops) | Kind::SMin(ops) | Kind::SequentialUMin(ops) => {
                let intrinsic = match self.node(s).kind {
                    Kind::UMax(_) => RangeIntrinsic::UMax,
                    Kind::SMax(_) => RangeIntrinsic::SMax,
                    Kind::UMin(_) | Kind::SequentialUMin(_) => RangeIntrinsic::UMin,
                    Kind::SMin(_) => RangeIntrinsic::SMin,
                    _ => unreachable!("Unknown SCEVMinMaxExpr/SCEVSequentialMinMaxExpr."),
                };
                let mut x = self.get_range_ref(cx, ops[0].scev, hint, depth + 1);
                for op in &ops[1..] {
                    let y = self.get_range_ref(cx, op.scev, hint, depth + 1);
                    x = ConstantRange::intrinsic(intrinsic, &[x, y]);
                }
                self.set_range(s, hint, conservative_result.intersect_with(&x, range_type))
            }
            Kind::Unknown(value) => {
                let q = cx.query(None);

                // Check if the IR explicitly contains !range metadata.
                if let Some(md_range) = self.get_range_from_metadata(cx, value) {
                    conservative_result = conservative_result.intersect_with(&md_range, range_type);
                }

                // Use facts about recurrences in the underlying IR. Note that
                // add recurrences are AddRecExprs and thus don't hit this path.
                // This primarily handles shift recurrences.
                let cr = self.get_range_for_unknown_recurrence(cx, s);
                conservative_result = conservative_result.intersect_with(&cr, PreferredRangeType::Smallest);

                // See if ValueTracking can give us a useful range.
                let mut known = compute_known_bits(&q, value, 0);
                if known.width != bit_width {
                    known = known.zext_or_trunc(bit_width);
                }

                // ValueTracking may be able to compute a tighter result for the
                // number of sign bits than for the value of those sign bits.
                let mut ns = compute_num_sign_bits(&q, value, 0);
                if self.type_of(s).is_pointer() {
                    // NS counts the sign bits of the whole pointer; drop those
                    // above the index bits.
                    let Ty::Ptr(space) = self.type_of(s) else { unreachable!() };
                    let ptr_idx_diff = cx.layout.pointer(space).bits - bit_width;
                    ns = if ns > ptr_idx_diff { ns - ptr_idx_diff } else { 1 };
                }

                if ns > 1 {
                    // If we know any of the sign bits, we know all of the sign
                    // bits.
                    let high = high_bits_mask(bit_width, ns);
                    if known.zero & high != 0 {
                        known.zero |= high;
                    }
                    if known.one & high != 0 {
                        known.one |= high;
                    }
                }

                let (known_min, known_max) = (known.min_value(), known.max_value());
                if known_min != known_max.wrapping_add(1) & llrm_mir::context::mask(bit_width) {
                    conservative_result = conservative_result.intersect_with(
                        &ConstantRange::new(bit_width, known_min, known_max.wrapping_add(1)),
                        range_type,
                    );
                }
                if ns > 1 {
                    let lower = Ap::signed_min(bit_width).ashr(ns - 1);
                    let upper = Ap::signed_max(bit_width).ashr(ns - 1).add(Ap::one(bit_width));
                    conservative_result = conservative_result
                        .intersect_with(&ConstantRange::new(bit_width, lower.bits, upper.bits), range_type);
                }

                if self.type_of(s).is_pointer() && hint == RangeSignHint::Unsigned {
                    // Strengthen the range if the underlying IR value is a
                    // global/alloca/heap allocation using the size of the
                    // object.
                    let (deref_bytes, _can_be_null) = pointer_dereferenceable_bytes(&q, value);
                    if deref_bytes > 1 && (bit_width >= 64 || deref_bytes < (1u64 << bit_width)) {
                        // The highest address the object can start is
                        // DerefBytes bytes before the end (unsigned max
                        // value). If this value is not a multiple of the
                        // alignment, the last possible start value is the next
                        // lowest multiple of the alignment. Note: The
                        // computations below cannot overflow, because if they
                        // would there's no possible start address for the
                        // object.
                        let max_val = Ap::all_ones(bit_width).sub(Ap::new(bit_width, u128::from(deref_bytes)));
                        let align = pointer_alignment(&q, value);
                        let rem = max_val.zext_value() % u128::from(align);
                        let max_val = max_val.sub(Ap::new(bit_width, rem));
                        let mut min_val = Ap::zero(bit_width);
                        if is_known_non_zero(&q, value, 0) {
                            min_val = Ap::new(bit_width, u128::from(align));
                        }
                        conservative_result = conservative_result.intersect_with(
                            &ConstantRange::non_empty(bit_width, min_val.bits, max_val.add(Ap::one(bit_width)).bits),
                            range_type,
                        );
                    }
                }

                // A range of Phi is a subset of union of all ranges of its
                // input.
                if let Operand::Value(id) = value
                    && let ValueDef::Instruction(phi) = cx.function.value(id).def
                    && cx.function.instruction(phi).opcode == Opcode::Phi
                {
                    // SCEVExpander sometimes creates SCEVUnknowns that are
                    // secretly AddRecs; return the range for the corresponding
                    // AddRec.
                    let phi_scev = self.get_scev(cx, value);
                    if matches!(self.node(phi_scev).kind, Kind::AddRec(..)) {
                        return self.get_range_ref(cx, phi_scev, hint, depth + 1);
                    }

                    // Make sure that we do not run over cycled Phis.
                    if self.range_ref_phi_allowed_operands(cx, phi) {
                        let mut range_from_ops = ConstantRange::empty(bit_width);

                        let operands = cx.function.instruction(phi).operands.clone();
                        for op in operands {
                            if matches!(op, Operand::Block(_)) {
                                continue;
                            }
                            let op_scev = self.get_scev(cx, op);
                            let op_range = self.get_range_ref(cx, op_scev, hint, depth + 1);
                            range_from_ops = range_from_ops.union_with(&op_range, PreferredRangeType::Smallest);
                            // No point to continue if we already have a full
                            // set.
                            if range_from_ops.is_full_set() {
                                break;
                            }
                        }
                        conservative_result = conservative_result.intersect_with(&range_from_ops, range_type);
                    }
                }

                self.set_range(s, hint, conservative_result)
            }
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    /// `GetRangeFromMetadata`
    /// ([ScalarEvolution.cpp#L6437](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6437)):
    /// the `!range` of an instruction, else the `range` return attribute of a
    /// call, else the `range` attribute of a parameter.
    fn get_range_from_metadata(
        &self,
        cx: &Cx,
        value: Operand,
    ) -> Option<ConstantRange> {
        let Operand::Value(id) = value else { return None };
        let q = cx.query(None);
        let inst = match cx.function.value(id).def {
            ValueDef::Instruction(inst) => inst,
            ValueDef::Argument(at) => return llrm_mir::valuetracking::pattern::argument_range(&q, at),
        };
        let Some(node) = q.metadata_node(inst, "range") else {
            return llrm_mir::valuetracking::pattern::call_range(&q, cx.function.instruction(inst));
        };
        let mut range: Option<ConstantRange> = None;
        for pair in node.operands.chunks(2) {
            let (
                llrm_mir::module::MetadataOperand::Constant(lower),
                llrm_mir::module::MetadataOperand::Constant(upper),
            ) = (&pair[0], &pair.get(1)?)
            else {
                return None;
            };
            let ((lower, width), (upper, _)) =
                (q.int_constant(Operand::Constant(*lower))?, q.int_constant(Operand::Constant(*upper))?);
            let one = ConstantRange::new(width, lower, upper);
            range = Some(match range {
                None => one,
                Some(previous) => previous.union_with(&one, PreferredRangeType::Smallest),
            });
        }
        range
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6463
    /// `getRangeForUnknownRecurrence`: a shift recurrence `<start, shift,
    /// step>` over a loop with a small constant maximum trip count.
    fn get_range_for_unknown_recurrence(
        &mut self,
        cx: &Cx,
        u: Scev,
    ) -> ConstantRange {
        use llrm_mir::knownbits::KnownBits;
        use llrm_mir::opcode::BinaryOp;
        use llrm_mir::valuetracking::pattern::match_simple_recurrence;

        let bit_width = self.get_type_size_in_bits(cx, self.type_of(u));
        let full = ConstantRange::full(bit_width);

        // Match a simple recurrence of the form: <start, ShiftOp, Step>, and
        // then use information about the trip count to improve our available
        // range. Step may be loop varying here.
        let Kind::Unknown(Operand::Value(phi_value)) = self.node(u).kind.clone() else { return full };
        let ValueDef::Instruction(p) = cx.function.value(phi_value).def else { return full };
        if cx.function.instruction(p).opcode != Opcode::Phi {
            return full;
        }
        let Some(header) = cx.function.parent(p) else { return full };

        // No phi input may come from an unreachable block.
        if cx.function.predecessors(header).iter().any(|&pred| !cx.tree.is_reachable(pred)) {
            return full;
        }

        let q = cx.query(None);
        let Some((bo, start, step)) = match_simple_recurrence(&q, p) else { return full };

        // A recurrence in reachable code is in a loop; BO may be in a subloop.
        let Some(lp) = cx.loops.get_loop_for(header) else { return full };
        let Some(bo_block) = cx.function.parent(bo) else { return full };
        if !lp.contains(bo_block) {
            return full;
        }

        let Opcode::Binary(op) = cx.function.instruction(bo).opcode else { return full };
        if !matches!(op, BinaryOp::AShr | BinaryOp::LShr | BinaryOp::Shl) {
            return full;
        }
        if cx.function.instruction(bo).operands[0] != Operand::Value(phi_value) {
            return full;
        }

        let tc = self.get_small_constant_max_trip_count(cx, header);
        if tc == 0 || tc >= bit_width {
            return full;
        }

        let known_start = compute_known_bits(&q, start, 0);
        let known_step = compute_known_bits(&q, step, 0);

        // The total shift amount, careful of overflow.
        let max_shift = known_step.max_value();
        let mask = llrm_mir::context::mask(bit_width);
        let Some(total) = max_shift.checked_mul(u128::from(tc - 1)).filter(|total| *total <= mask) else {
            return full;
        };
        let total_shift = KnownBits::constant(bit_width, total);

        match op {
            BinaryOp::AShr => {
                // The end value is closer to zero than the start.
                let known_end = KnownBits::ashr(&known_start, &total_shift, false, false);
                if known_start.is_non_negative() {
                    return ConstantRange::non_empty(
                        bit_width,
                        known_end.min_value(),
                        known_start.max_value().wrapping_add(1),
                    );
                }
                if known_start.is_negative() {
                    return ConstantRange::non_empty(
                        bit_width,
                        known_start.min_value(),
                        known_end.max_value().wrapping_add(1),
                    );
                }
            }
            BinaryOp::LShr => {
                // The low end of the unsigned range is the last value produced.
                let known_end = KnownBits::lshr(&known_start, &total_shift, false, false);
                return ConstantRange::non_empty(
                    bit_width,
                    known_end.min_value(),
                    known_start.max_value().wrapping_add(1),
                );
            }
            BinaryOp::Shl => {
                // Iff no bits are shifted out, the value increases on every
                // shift.
                let known_end = KnownBits::shl(&known_start, &total_shift, false, false, false);
                if total < u128::from(known_start.count_min_leading_zeros()) {
                    return ConstantRange::new(
                        bit_width,
                        known_start.min_value(),
                        known_end.max_value().wrapping_add(1),
                    );
                }
            }
            _ => unreachable!("filtered out above"),
        }
        full
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6836
    /// `getRangeForAffineARHelper`: the range of an expression that starts in
    /// `start_range` and changes by `step` up to `max_be_count` times; and
    /// whether no wrapping occurred.
    fn get_range_for_affine_ar_helper(
        step: Ap,
        start_range: &ConstantRange,
        max_be_count: Ap,
        signed: bool,
    ) -> (ConstantRange, bool) {
        let bit_width = step.width;
        debug_assert!(bit_width == start_range.width && bit_width == max_be_count.width, "mismatched bit widths");
        // If either Step or MaxBECount is 0, then the expression won't change,
        // and we just need to return the initial range.
        if step.is_zero() || max_be_count.is_zero() {
            return (*start_range, true);
        }

        // If we don't know anything about the initial value (i.e. StartRange
        // is FullRange), then we don't know anything about the final range
        // either. Return FullRange.
        if start_range.is_full_set() {
            return (ConstantRange::full(bit_width), false);
        }

        // If Step is signed and negative, then we use its absolute value, but
        // we also note that we're moving in the opposite direction.
        let descending = signed && step.is_negative();

        let mut step = step;
        if signed {
            // This is correct even for INT_SMIN. Let's look at i8 to
            // illustrate this:
            // abs(INT_SMIN) = abs(-128) = abs(0x80) = -0x80 = 0x80 = 128.
            // This equations hold true due to the well-defined wrap-around
            // behavior of APInt.
            if step.is_negative() {
                step = step.neg();
            }
        }

        // Check if Offset is more than full span of BitWidth. If it is, the
        // expression is guaranteed to overflow.
        if Ap::all_ones(bit_width).udiv(step).ult(max_be_count) {
            return (ConstantRange::full(bit_width), false);
        }

        // Offset is by how much the expression can change. Checks above
        // guarantee no overflow here.
        let offset = step.mul(max_be_count);

        // Minimum value of the final range will match the minimal value of
        // StartRange if the expression is increasing and will be decreased by
        // Offset otherwise. Maximum value of the final range will match the
        // maximal value of StartRange if the expression is decreasing and will
        // be increased by Offset otherwise.
        let start_lower = Ap::new(bit_width, start_range.lower);
        let start_upper = Ap::new(bit_width, start_range.upper).sub(Ap::one(bit_width));
        let overflow;
        let moved_boundary;
        if signed {
            // This does not use sadd_ov, as we want to check overflow for a
            // signed start with an unsigned offset.
            if descending {
                moved_boundary = start_lower.sub(offset);
                overflow = start_lower.slt(moved_boundary) || start_range.is_sign_wrapped_set();
            } else {
                moved_boundary = start_upper.add(offset);
                overflow = moved_boundary.slt(start_upper) || start_range.is_sign_wrapped_set();
            }
        } else {
            let (sum, over) = start_upper.uadd_ov(offset);
            moved_boundary = sum;
            overflow = over || start_range.is_wrapped_set();
        }

        // It's possible that the new minimum/maximum value will fall into the
        // initial range (due to wrap around). This means that the expression
        // can take any value in this bitwidth, and we have to return full
        // range.
        if start_range.contains(moved_boundary.bits) {
            return (ConstantRange::full(bit_width), false);
        }

        let new_lower = if descending { moved_boundary } else { start_lower };
        let new_upper = (if descending { start_upper } else { moved_boundary }).add(Ap::one(bit_width));

        // No overflow detected, return [StartLower, StartUpper + Offset + 1)
        // range.
        (ConstantRange::non_empty(bit_width, new_lower.bits, new_upper.bits), !overflow)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6906
    /// `getRangeForAffineAR`: the range of `{start,+,step}` over `max_be_count`
    /// iterations, and the no-wrap flags that shows.
    fn get_range_for_affine_ar(
        &mut self,
        cx: &Cx,
        start: Scev,
        step: Scev,
        max_be_count: Ap,
    ) -> (ConstantRange, u8) {
        debug_assert!(
            self.get_type_size_in_bits(cx, self.type_of(start)) == self.get_type_size_in_bits(cx, self.type_of(step))
                && self.get_type_size_in_bits(cx, self.type_of(start)) == max_be_count.width,
            "mismatched bit widths"
        );
        let bits = max_be_count.width;

        // First, consider step signed.
        let start_s_range = self.get_signed_range(cx, start);
        let step_s_range = self.get_signed_range(cx, step);

        // If Step can be both positive and negative, we need to find ranges for
        // the maximum absolute step values in both directions and union them.
        let (sr1, nsw1) = Self::get_range_for_affine_ar_helper(
            Ap::new(bits, step_s_range.get_signed_min()),
            &start_s_range,
            max_be_count,
            true,
        );
        let (sr2, nsw2) = Self::get_range_for_affine_ar_helper(
            Ap::new(bits, step_s_range.get_signed_max()),
            &start_s_range,
            max_be_count,
            true,
        );
        let sr = sr1.union_with(&sr2, PreferredRangeType::Smallest);

        // Next, consider step unsigned.
        let step_max = self.get_unsigned_range_max(cx, step);
        let start_u_range = self.get_unsigned_range(cx, start);
        let (ur, nuw) = Self::get_range_for_affine_ar_helper(step_max, &start_u_range, max_be_count, false);

        let mut f = flags::NONE;
        if nuw {
            f |= flags::NUW;
        }
        if nsw1 && nsw2 {
            f |= flags::NSW;
        }

        // Finally, intersect signed and unsigned ranges.
        (sr.intersect_with(&ur, PreferredRangeType::Smallest), f)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7221
    /// `getRangeViaFactoring`: `{C?A:B,+,C?P:Q}` is the union of the ranges of
    /// `{A,+,P}` and `{B,+,Q}`.
    fn get_range_via_factoring(
        &mut self,
        cx: &Cx,
        start: Scev,
        step: Scev,
        max_be_count: Ap,
    ) -> ConstantRange {
        let bit_width = max_be_count.width;
        let full = ConstantRange::full(bit_width);
        let Some((start_cond, start_true, start_false)) = self.select_pattern(cx, bit_width, start) else {
            return full;
        };
        let Some((step_cond, step_true, step_false)) = self.select_pattern(cx, bit_width, step) else {
            return full;
        };
        if start_cond != step_cond {
            // Four possibilities would be needed; getRange already does about
            // as well.
            return full;
        }
        let true_start = self.get_constant(start_true);
        let true_step = self.get_constant(step_true);
        let false_start = self.get_constant(start_false);
        let false_step = self.get_constant(step_false);
        let true_range = self.get_range_for_affine_ar(cx, true_start, true_step, max_be_count).0;
        let false_range = self.get_range_for_affine_ar(cx, false_start, false_step, max_be_count).0;
        true_range.union_with(&false_range, PreferredRangeType::Smallest)
    }

    /// `getRangeViaFactoring`'s `SelectPattern`: `s` as `select C, T, F` of
    /// constants behind an integral cast and a constant offset; the condition
    /// and the two values.
    fn select_pattern(
        &mut self,
        cx: &Cx,
        bit_width: u32,
        s: Scev,
    ) -> Option<(Operand, Ap, Ap)> {
        use llrm_mir::context::ConstantKind;
        let mut s = s;
        let mut offset = Ap::new(bit_width, 0);
        // Peel off a constant offset.
        if let Kind::Add(ops) = &self.node(s).kind
            && ops.len() == 2
            && let Kind::Constant(off) = self.node(ops[0].scev).kind
        {
            offset = off;
            s = ops[1].scev;
        }
        // Peel off a cast operation.
        let mut cast = None;
        match self.node(s).kind {
            Kind::Truncate(op) => {
                cast = Some(0);
                s = op.scev;
            }
            Kind::ZeroExtend(op) => {
                cast = Some(1);
                s = op.scev;
            }
            Kind::SignExtend(op) => {
                cast = Some(2);
                s = op.scev;
            }
            _ => {}
        }
        let Kind::Unknown(Operand::Value(value)) = self.node(s).kind.clone() else { return None };
        let ValueDef::Instruction(select) = cx.function.value(value).def else { return None };
        let instruction = cx.function.instruction(select);
        if instruction.opcode != Opcode::Select {
            return None;
        }
        let constant = |operand: Operand| -> Option<Ap> {
            let Operand::Constant(id) = operand else { return None };
            let found = cx.context.get(id);
            let bits = match found.kind {
                ConstantKind::Int(bits) => bits,
                ConstantKind::Zero => 0,
                _ => return None,
            };
            Some(Ap::new(cx.context.types.int_bits(found.ty)?, bits))
        };
        let (condition, true_value, false_value) =
            (instruction.operands[0], constant(instruction.operands[1])?, constant(instruction.operands[2])?);
        // Re-apply the cast peeled off earlier.
        let recast = |value: Ap| match cast {
            Some(0) => value.trunc(bit_width),
            Some(1) => value.zext(bit_width),
            Some(_) => value.sext(bit_width),
            None => value,
        };
        // Re-apply the constant offset peeled off earlier.
        Some((condition, recast(true_value).add(offset), recast(false_value).add(offset)))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5063
    /// `inferNoWrapViaConstantRanges`: force the ranges of an affine
    /// recurrence, which sets the flags they show.
    pub fn infer_no_wrap_via_constant_ranges(
        &mut self,
        cx: &Cx,
        ar: Scev,
    ) {
        let Kind::AddRec(ops, header) = self.node(ar).kind.clone() else { return };
        if ops.len() != 2 {
            return;
        }

        // Force computation of ranges, which will also perform range-based flag
        // inference.
        if self.get_no_wrap_flags(ar, flags::NSW) == 0 {
            self.get_signed_range(cx, ar);
        }

        if self.get_no_wrap_flags(ar, flags::NUW) == 0 {
            self.get_unsigned_range(cx, ar);
        }

        if self.get_no_wrap_flags(ar, flags::NW) == 0 {
            let be_count = self.get_backedge_taken_count(cx, header, ExitCountKind::ConstantMaximum);
            if let Some(be_count_ap) = self.as_constant(be_count) {
                let step = self.get_step_recurrence(cx, ar);
                let step_cr = self.get_signed_range(cx, step);
                let no_overflow_bit_width = be_count_ap.active_bits() + step_cr.get_min_signed_bits();
                if no_overflow_bit_width <= self.get_type_size_in_bits(cx, self.type_of(ar)) {
                    self.node_set_no_wrap_flags(ar, flags::NW);
                }
            }
        }
    }

    /// `isKnownNonPositive`.
    pub fn is_known_non_positive(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> bool {
        let max = self.get_signed_range_max(cx, s);
        !max.is_strictly_positive()
    }

    /// The block `getRangeRef` takes the loop of an instruction from.
    pub fn header_of_loop_containing(
        &self,
        cx: &Cx,
        block: BlockId,
    ) -> Option<BlockId> {
        cx.loops.get_loop_for(block).map(|one| one.get_header())
    }
}

/// `APInt::getHighBitsSet(width, bits)`.
fn high_bits_mask(
    width: u32,
    bits: u32,
) -> u128 {
    let all = llrm_mir::context::mask(width);
    all & !(llrm_mir::context::mask(width - bits.min(width)))
}

/// Unused marker so the `Type` import stays meaningful in this file.
#[allow(dead_code)]
fn _kind_tag(k: &Kind) -> Type {
    k.tag()
}
