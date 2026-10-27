//! The arithmetic folds: `getAddExpr`, `getMulExpr`, their helpers and the
//! flag strengthening they share.

use std::borrow::Cow;
use std::collections::BTreeMap;

use llrm_mir::hash::HashMap;
use llrm_mir::hash::HashSet;
use llrm_mir::opcode::BinaryOp;

use super::ap::Ap;
use super::expr::{Kind, Scev, Ty, Type, Use, flags};
use super::{Cx, Flags, Key, KeyLike, ScalarEvolution};

/// `scev-mulops-inline-threshold`.
const MUL_OPS_INLINE_THRESHOLD: usize = 32;
/// `scev-addops-inline-threshold`.
const ADD_OPS_INLINE_THRESHOLD: usize = 500;
/// `scalar-evolution-max-arith-depth`.
const MAX_ARITH_DEPTH: u32 = 32;
/// `scalar-evolution-max-add-rec-size`.
const MAX_ADD_REC_SIZE: usize = 8;
/// `scalar-evolution-huge-expr-threshold`.
const HUGE_EXPR_THRESHOLD: u16 = 4096;

/// `ScalarEvolution::hasFlags`.
fn has_flags(
    f: u8,
    test: u8,
) -> bool {
    test == f & test
}

/// `umul_ov` of the file: a product of `u64`s, noting overflow.
fn umul_ov(
    i: u64,
    j: u64,
    overflow: &mut bool,
) -> u64 {
    let k = i.wrapping_mul(j);
    if j > 1 && k / j != i {
        *overflow = true;
    }
    k
}

/// `Choose`: the binomial coefficient "n choose k". If an intermediate
/// computation overflows, `overflow` is set and the return is garbage.
fn choose(
    n: u64,
    mut k: u64,
    overflow: &mut bool,
) -> u64 {
    // We use the multiplicative formula:
    //     n(n-1)(n-2)...(n-(k-1)) / k(k-1)(k-2)...1 .
    // At each iteration, we take the n-th term of the numeral and divide by the
    // (k-n)th term of the denominator. This division will always produce an
    // integral result, and helps reduce the chance of overflow in the
    // intermediate computations. However, we can still overflow even when the
    // final result would fit.
    if n == 0 || n == k {
        return 1;
    }
    if k > n {
        return 0;
    }

    if k > n / 2 {
        k = n - k;
    }

    let mut r = 1u64;
    for i in 1..=k {
        r = umul_ov(r, n - (i - 1), overflow);
        r /= i;
    }
    r
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L814
    /// `hasHugeExpression`: a node with at least `HugeExprThreshold` nodes
    /// below.
    fn has_huge_expression(
        &self,
        ops: &[Use],
    ) -> bool {
        ops.iter().any(|op| self.expression_size(op.scev) >= HUGE_EXPR_THRESHOLD)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L828
    /// `constantFoldAndGroupOps`: fold the constants of `ops` with `fold`,
    /// drop an identity, return an absorber, and sort the rest by complexity.
    /// The whole expression if it reduces to a single operand.
    pub(super) fn constant_fold_and_group_ops(
        &mut self,
        cx: &Cx,
        ops: &mut Vec<Use>,
        fold: &dyn Fn(Ap, Ap) -> Ap,
        is_identity: &dyn Fn(Ap) -> bool,
        is_absorber: &dyn Fn(Ap) -> bool,
    ) -> Option<Scev> {
        let mut folded: Option<Ap> = None;
        let mut idx = 0;
        while idx < ops.len() {
            if let Kind::Constant(c) = self.node(ops[idx].scev).kind {
                folded = Some(match folded {
                    None => c,
                    Some(previous) => fold(previous, c),
                });
                ops.remove(idx);
                continue;
            }
            idx += 1;
        }

        if ops.is_empty() {
            let folded = folded.expect("Must have folded value");
            return Some(self.get_constant(folded));
        }

        if let Some(f) = folded
            && is_absorber(f)
        {
            return Some(self.get_constant(f));
        }

        self.group_by_complexity(cx, ops);
        if let Some(f) = folded
            && !is_identity(f)
        {
            let c = self.get_constant(f);
            ops.insert(0, Use::of(c));
        }

        (ops.len() == 1).then(|| ops[0].scev)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2400
    /// `StrengthenNoWrapFlags`: infer a more aggressive set of can't-overflow
    /// flags for the operation of kind `tag` on `ops` if possible.
    pub(super) fn strengthen_no_wrap_flags(
        &mut self,
        cx: &Cx,
        tag: Type,
        ops: &[Use],
        mut f: u8,
    ) -> u8 {
        debug_assert!(matches!(tag, Type::Add | Type::AddRec | Type::Mul), "don't call from other places!");

        let sign_or_unsign_mask = flags::NUW | flags::NSW;
        let mut sign_or_unsign_wrap = f & sign_or_unsign_mask;

        // If FlagNSW is true and all the operands are non-negative, infer
        // FlagNUW.
        if sign_or_unsign_wrap == flags::NSW && ops.iter().all(|op| self.is_known_non_negative(cx, op.scev)) {
            f |= sign_or_unsign_mask;
        }

        sign_or_unsign_wrap = f & sign_or_unsign_mask;

        if sign_or_unsign_wrap != sign_or_unsign_mask
            && matches!(tag, Type::Add | Type::Mul)
            && ops.len() == 2
            && self.as_constant(ops[0].scev).is_some()
        {
            let opcode = match tag {
                Type::Add => BinaryOp::Add,
                Type::Mul => BinaryOp::Mul,
                _ => unreachable!("Unexpected SCEV op."),
            };

            let c = self.as_constant(ops[0].scev).expect("a constant");

            // (A <opcode> C) --> (A <opcode> C)<nsw> if the op doesn't sign
            // overflow.
            if sign_or_unsign_wrap & flags::NSW == 0 {
                let nsw_region = llrm_mir::constantrange::ConstantRange::make_guaranteed_no_wrap_region(
                    opcode,
                    &llrm_mir::constantrange::ConstantRange::single(c.width, c.bits),
                    llrm_mir::constantrange::NO_SIGNED_WRAP,
                );
                if nsw_region.contains_range(&self.get_signed_range(cx, ops[1].scev)) {
                    f |= flags::NSW;
                }
            }

            // (A <opcode> C) --> (A <opcode> C)<nuw> if the op doesn't unsign
            // overflow.
            if sign_or_unsign_wrap & flags::NUW == 0 {
                let nuw_region = llrm_mir::constantrange::ConstantRange::make_guaranteed_no_wrap_region(
                    opcode,
                    &llrm_mir::constantrange::ConstantRange::single(c.width, c.bits),
                    llrm_mir::constantrange::NO_UNSIGNED_WRAP,
                );
                if nuw_region.contains_range(&self.get_unsigned_range(cx, ops[1].scev)) {
                    f |= flags::NUW;
                }
            }
        }

        // <0,+,nonnegative><nw> is also nuw
        // TODO: Add corresponding nsw case
        if tag == Type::AddRec
            && has_flags(f, flags::NW)
            && !has_flags(f, flags::NUW)
            && ops.len() == 2
            && self.match_zero(ops[0].scev)
            && self.is_known_non_negative(cx, ops[1].scev)
        {
            f |= flags::NUW;
        }

        // both (udiv X, Y) * Y and Y * (udiv X, Y) are always NUW
        if tag == Type::Mul && !has_flags(f, flags::NUW) && ops.len() == 2 {
            if let Kind::UDiv(_, divisor) = self.node(ops[0].scev).kind
                && divisor.scev == ops[1].scev
            {
                f |= flags::NUW;
            }
            if let Kind::UDiv(_, divisor) = self.node(ops[1].scev).kind
                && divisor.scev == ops[0].scev
            {
                f |= flags::NUW;
            }
        }

        f
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3873
    /// `findExistingSCEVInCache`.
    fn find_existing_scev_in_cache(
        &self,
        tag: Type,
        ops: &[Use],
    ) -> Option<Scev> {
        let key = Key { tag, ops: Cow::Borrowed(ops), ty: None, constant: None, value: None, header: None };
        self.unique.get(&key as &dyn KeyLike).copied()
    }

    /// The node `new (SCEVAllocator) SCEV{Add,Mul}Expr` makes, found or made,
    /// with `f` set on it: `getOrCreateAddExpr`/`getOrCreateMulExpr`.
    fn get_or_create_commutative(
        &mut self,
        tag: Type,
        ops: &[Use],
        f: u8,
    ) -> Scev {
        let key = Key { tag, ops: Cow::Borrowed(ops), ty: None, constant: None, value: None, header: None };
        let ty = match tag {
            // The type of an add is that of its first pointer operand, else of
            // its first operand.
            Type::Add => ops
                .iter()
                .map(|op| self.type_of(op.scev))
                .find(|ty| ty.is_pointer())
                .unwrap_or_else(|| self.type_of(ops[0].scev)),
            _ => self.type_of(ops[0].scev),
        };
        let kind = || match tag {
            Type::Add => Kind::Add(ops.to_vec()),
            Type::Mul => Kind::Mul(ops.to_vec()),
            _ => unreachable!("an add or a multiply"),
        };
        let size = self.compute_expression_size(ops);
        let s = self.intern(key, kind, Some(ty), size);
        self.nodes[s.0 as usize].flags |= f;
        s
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2995
    /// `getOrCreateAddExpr`.
    fn get_or_create_add_expr(
        &mut self,
        ops: &[Use],
        f: u8,
    ) -> Scev {
        self.get_or_create_commutative(Type::Add, ops, f)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3041
    /// `getOrCreateMulExpr`.
    fn get_or_create_mul_expr(
        &mut self,
        ops: &[Use],
        f: u8,
    ) -> Scev {
        self.get_or_create_commutative(Type::Mul, ops, f)
    }

    /// `SCEVUse{S, UseFlags}`.
    pub(super) fn with_flags(
        &self,
        s: Scev,
        use_flags: u8,
    ) -> Use {
        self.with_use_flags(s, use_flags)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2194
    /// `CollectAddOperandsWithScales`: process the `ops` list, which is a list
    /// of operands to be added under the given scale, and update the map. See
    /// the C++ comment for the example it works.
    ///
    /// Returns whether it appears that any interesting folding opportunities
    /// may be exposed.
    fn collect_add_operands_with_scales(
        &mut self,
        cx: &Cx,
        m: &mut HashMap<Use, Ap>,
        new_ops: &mut Vec<Use>,
        accumulated_constant: &mut Ap,
        ops: &[Use],
        scale: Ap,
    ) -> bool {
        let mut interesting = false;

        // Iterate over the add operands. They are sorted, with constants first.
        let mut i = 0;
        while let Some(c) = self.as_constant(ops[i].scev) {
            i += 1;
            // Pull a buried constant out to the outside.
            if scale != Ap::one(scale.width) || !accumulated_constant.is_zero() || c.is_zero() {
                interesting = true;
            }
            *accumulated_constant = accumulated_constant.add(scale.mul(c));
        }

        // Next comes everything else. We're especially interested in
        // multiplies here, but they're in the middle, so just visit the rest
        // with one loop.
        while i != ops.len() {
            let op = ops[i];
            let mul_operands = matches!(self.node(op.scev).kind, Kind::Mul(_)).then(|| self.operands(op.scev).to_vec());
            if let Some(mul_operands) = mul_operands
                && let Some(first) = self.as_constant(mul_operands[0].scev)
            {
                let new_scale = scale.mul(first);
                if mul_operands.len() == 2 && matches!(self.node(mul_operands[1].scev).kind, Kind::Add(_)) {
                    // A multiplication of a constant with another add; recurse.
                    let add_operands = self.operands(mul_operands[1].scev).to_vec();
                    interesting |= self.collect_add_operands_with_scales(
                        cx,
                        m,
                        new_ops,
                        accumulated_constant,
                        &add_operands,
                        new_scale,
                    );
                } else {
                    // A multiplication of a constant with some other value.
                    // Update the map.
                    let mul_ops: Vec<Use> = mul_operands[1..].to_vec();
                    let key = Use::of(self.get_mul_expr(cx, mul_ops, Flags::default(), 0).scev);
                    match m.get_mut(&key) {
                        None => {
                            m.insert(key, new_scale);
                            new_ops.push(key);
                        }
                        Some(entry) => {
                            *entry = entry.add(new_scale);
                            // The map already had an entry for this value,
                            // which may indicate a folding opportunity.
                            interesting = true;
                        }
                    }
                }
            } else {
                // An ordinary operand. Update the map.
                match m.get_mut(&op) {
                    None => {
                        m.insert(op, scale);
                        new_ops.push(op);
                    }
                    Some(entry) => {
                        *entry = entry.add(scale);
                        // The map already had an entry for this value, which
                        // may indicate a folding opportunity.
                        interesting = true;
                    }
                }
            }
            i += 1;
        }

        interesting
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2517
    /// `ScalarEvolution::getAddExpr`: a canonical add expression, or something
    /// simpler if possible.
    pub fn get_add_expr(
        &mut self,
        cx: &Cx,
        mut ops: Vec<Use>,
        f: Flags,
        depth: u32,
    ) -> Use {
        let orig_flags = f.expr;
        let use_flags = f.use_;
        debug_assert!(orig_flags & !(flags::NUW | flags::NSW) == 0, "only nuw or nsw allowed");
        debug_assert!(use_flags & !(flags::NUW | flags::NSW) == 0, "only nuw or nsw allowed");
        debug_assert!(!ops.is_empty(), "Cannot get empty add!");
        if ops.len() == 1 {
            return ops[0];
        }
        #[cfg(debug_assertions)]
        {
            let ety = self.get_effective_scev_type(cx, self.type_of(ops[0].scev));
            for op in &ops[1..] {
                debug_assert!(
                    self.get_effective_scev_type(cx, self.type_of(op.scev)) == ety,
                    "SCEVAddExpr operand types don't match!"
                );
            }
            let num_ptrs = ops.iter().filter(|op| self.type_of(op.scev).is_pointer()).count();
            debug_assert!(num_ptrs <= 1, "add has at most one pointer operand");
        }

        if let Some(folded) = self.constant_fold_and_group_ops(
            cx,
            &mut ops,
            &|c1, c2| c1.add(c2),
            &|c| c.is_zero(), // identity
            &|_| false,       // absorber
        ) {
            return Use::of(folded);
        }

        // Keep track of operands after constant folding, for verification when
        // adding use-specific flags.
        #[cfg(debug_assertions)]
        let orig_ops = ops.clone();

        let mut idx = usize::from(self.as_constant(ops[0].scev).is_some());

        // Limit recursion calls depth.
        if depth > MAX_ARITH_DEPTH || self.has_huge_expression(&ops) {
            let computed = self.strengthen_no_wrap_flags(cx, Type::Add, &ops, orig_flags);
            let s = self.get_or_create_add_expr(&ops, computed);
            return self.with_flags(s, use_flags);
        }

        if let Some(s) = self.find_existing_scev_in_cache(Type::Add, &ops) {
            // Don't strengthen flags if we have no new information.
            if self.get_no_wrap_flags(s, orig_flags) != orig_flags {
                let computed = self.strengthen_no_wrap_flags(cx, Type::Add, &ops, orig_flags);
                self.nodes[s.0 as usize].flags |= computed;
            }
            return self.with_flags(s, use_flags);
        }

        // Okay, check to see if the same value occurs in the operand list more
        // than once. If so, merge them together into an multiply expression.
        // Since we sorted the list, these values are required to be adjacent.
        let ty = self.type_of(ops[0].scev);
        let mut found_match = false;
        let mut i = 0usize;
        let mut e = ops.len();
        while i + 1 != e {
            if ops[i] == ops[i + 1] {
                // X + Y + Y  -->  X + Y*2
                // Scan ahead to count how many equal operands there are.
                let mut count = 2;
                while i + count != e && ops[i + count] == ops[i] {
                    count += 1;
                }
                // Merge the values into a multiply.
                let scale = Use::of(self.get_constant_of_type(cx, ty, count as i128));
                // `const SCEV *Mul`: the result's use flags are dropped.
                let mul = Use::of(self.get_mul_expr(cx, vec![scale, ops[i]], Flags::default(), depth + 1).scev);
                if ops.len() == count {
                    return mul;
                }
                ops[i] = mul;
                ops.drain(i + 1..i + count);
                // --i; e -= Count - 1;
                e -= count - 1;
                found_match = true;
                if i == 0 {
                    // `--i` on an unsigned zero wraps; the `++i` of the loop
                    // brings it back to 0.
                    continue;
                }
                i -= 1;
            }
            i += 1;
        }
        if found_match {
            return self.get_add_expr(cx, ops, Flags::expr(orig_flags), depth + 1);
        }

        // Check for truncates. If all the operands are truncated from the same
        // type, see if factoring out the truncate would permit the result to be
        // folded. eg., n*trunc(x) + m*trunc(y) --> trunc(trunc(m)*x +
        // trunc(n)*y) if the contents of the resulting outer trunc fold to
        // something simple.
        let find_trunc_src_type = |se: &Self| -> Option<Ty> {
            // We're ultimately looking to fold an addrec of truncs and muls of
            // only constants and truncs, so if we find any other types of SCEV
            // as operands of the addrec then we bail and return nullptr here.
            // Otherwise, we return the type of the operand of a trunc that we
            // find.
            let at = ops.get(idx)?;
            if let Kind::Truncate(inner) = se.node(at.scev).kind {
                return Some(se.type_of(inner.scev));
            }
            if matches!(se.node(at.scev).kind, Kind::Mul(_)) {
                let last_op = *se.operands(at.scev).last().expect("a multiply has operands");
                if let Kind::Truncate(inner) = se.node(last_op.scev).kind {
                    return Some(se.type_of(inner.scev));
                }
            }
            None
        };
        if let Some(src_type) = find_trunc_src_type(self) {
            let mut large_ops: Vec<Use> = Vec::new();
            let mut ok = true;
            // Check all the operands to see if they can be represented in the
            // source type of the truncate.
            for op in ops.clone() {
                match self.node(op.scev).kind.clone() {
                    Kind::Truncate(inner) => {
                        if self.type_of(inner.scev) != src_type {
                            ok = false;
                            break;
                        }
                        large_ops.push(inner);
                    }
                    Kind::Constant(_) => {
                        large_ops.push(Use::of(self.get_any_extend_expr(cx, op, src_type)));
                    }
                    Kind::Mul(_) => {
                        let mut large_mul_ops: Vec<Use> = Vec::new();
                        for m_op in self.operands(op.scev).to_vec() {
                            if !ok {
                                break;
                            }
                            match self.node(m_op.scev).kind.clone() {
                                Kind::Truncate(inner) => {
                                    if self.type_of(inner.scev) != src_type {
                                        ok = false;
                                        break;
                                    }
                                    large_mul_ops.push(inner);
                                }
                                Kind::Constant(_) => {
                                    large_mul_ops.push(Use::of(self.get_any_extend_expr(cx, m_op, src_type)));
                                }
                                _ => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if ok {
                            large_ops.push(self.get_mul_expr(cx, large_mul_ops, Flags::default(), depth + 1));
                        }
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                // Evaluate the expression in the larger type.
                let fold = self.get_add_expr(cx, large_ops, Flags::default(), depth + 1).scev;
                // If it folds to something simple, use it. Otherwise, don't.
                if matches!(self.node(fold).kind, Kind::Constant(_) | Kind::Unknown(_)) {
                    return Use::of(self.get_truncate_expr(cx, Use::of(fold), ty, 0));
                }
            }
        }

        if ops.len() == 2 {
            // Check if we have an expression of the form ((X + C1) - C2), where
            // C1 and C2 can be folded in a way that allows retaining wrapping
            // flags of (X + C1).
            let (a, b) = (ops[0], ops[1]);
            let add_expr = matches!(self.node(b.scev).kind, Kind::Add(_)).then_some(b.scev);
            let c = self.as_constant(a.scev);
            if let (Some(add_expr), Some(c)) = (add_expr, c)
                && let Some(c1) = self.as_constant(self.operands(add_expr)[0].scev)
            {
                let c2 = c;
                let mut preserved_flags = flags::NONE;

                let const_add = c1.add(c2);
                let add_flags = self.get_no_wrap_flags(add_expr, flags::MASK);
                // Adding a smaller constant is NUW if the original AddExpr was
                // NUW.
                if has_flags(add_flags, flags::NUW) && const_add.ule(c1) {
                    preserved_flags |= flags::NUW;
                }

                // Adding a constant with the same sign and small magnitude is
                // NSW, if the original AddExpr was NSW.
                if has_flags(add_flags, flags::NSW)
                    && c1.is_negative() == const_add.is_negative()
                    && abs_ap(const_add).ule(abs_ap(c1))
                {
                    preserved_flags |= flags::NSW;
                }

                if preserved_flags != flags::NONE {
                    let mut new_ops: Vec<Use> = self.operands(add_expr).to_vec();
                    new_ops[0] = Use::of(self.get_constant(const_add));
                    return self.get_add_expr(cx, new_ops, Flags::expr(preserved_flags), 0);
                }
            }

            // Try to push the constant operand into a ZExt: A + zext (-A + B)
            // -> zext (B), if trunc (A) + -A + B does not unsigned-wrap.
            if let Some(inner_add) = self.match_unary(b.scev, Type::ZeroExtend)
                && matches!(self.node(inner_add).kind, Kind::Add(_))
            {
                let narrow_a = self.get_truncate_expr(cx, a, self.type_of(inner_add), 0);
                let first = self.operands(inner_add)[0].scev;
                let neg_first = self.get_negative_scev(cx, first, 0);
                if narrow_a == neg_first
                    && self.get_zero_extend_expr(cx, Use::of(narrow_a), self.type_of(b.scev), 0) == a.scev
                    && has_flags(
                        self.strengthen_no_wrap_flags(
                            cx,
                            Type::Add,
                            &[Use::of(narrow_a), Use::of(inner_add)],
                            flags::NONE,
                        ),
                        flags::NUW,
                    )
                {
                    let sum =
                        self.get_add_expr(cx, vec![Use::of(narrow_a), Use::of(inner_add)], Flags::default(), 0).scev;
                    return Use::of(self.get_zero_extend_expr(cx, Use::of(sum), self.type_of(b.scev), 0));
                }
            }
        }

        // Canonicalize (-1 * urem X, Y) + X --> (Y * X/Y)
        if ops.len() == 2
            && let Some((all_ones, urem)) = self.match_binary(ops[0].scev, Type::Mul, 0)
            && self.match_all_ones(all_ones)
            && let Some((x, y)) = self.match_urem(cx, urem)
            && x == ops[1].scev
        {
            let quotient = self.get_udiv_expr(cx, ops[1], Use::of(y));
            return self.get_mul_expr(cx, vec![Use::of(y), Use::of(quotient)], Flags::default(), 0);
        }

        // Skip past any other cast SCEVs.
        while idx < ops.len() && (self.scev_type(ops[idx].scev) as u8) < (Type::Add as u8) {
            idx += 1;
        }

        // If there are add operands they would be next.
        if idx < ops.len() {
            let mut deleted_add = false;
            // If the original flags and all inlined SCEVAddExprs are NUW, use
            // the common NUW flag for expression after inlining. Other flags
            // cannot be preserved, because they may depend on the original
            // order of operations.
            let mut common_flags = orig_flags & flags::NUW;
            while matches!(self.node(ops[idx].scev).kind, Kind::Add(_)) {
                let add = ops[idx].scev;
                if ops.len() > ADD_OPS_INLINE_THRESHOLD || self.operands(add).len() > ADD_OPS_INLINE_THRESHOLD {
                    break;
                }
                // If we have an add, expand the add operands onto the end of
                // the operands list.
                ops.remove(idx);
                ops.extend_from_slice(&self.operands(add).to_vec());
                deleted_add = true;
                common_flags &= self.get_no_wrap_flags(add, flags::MASK);
                if idx >= ops.len() {
                    break;
                }
            }

            // If we deleted at least one add, we added operands to the end of
            // the list, and they are not necessarily sorted. Recurse to resort
            // and resimplify any operands we just acquired.
            if deleted_add {
                return self.get_add_expr(cx, ops, Flags::expr(common_flags), depth + 1);
            }
        }

        // Skip over the add expression until we get to a multiply.
        while idx < ops.len() && (self.scev_type(ops[idx].scev) as u8) < (Type::Mul as u8) {
            idx += 1;
        }

        // Check to see if there are any folding opportunities present with
        // operands multiplied by constant values.
        if idx < ops.len() && matches!(self.node(ops[idx].scev).kind, Kind::Mul(_)) {
            let bit_width = self.get_type_size_in_bits(cx, ty);
            let mut m: HashMap<Use, Ap> = HashMap::default();
            let mut new_ops: Vec<Use> = Vec::new();
            let mut accumulated_constant = Ap::zero(bit_width);
            if self.collect_add_operands_with_scales(
                cx,
                &mut m,
                &mut new_ops,
                &mut accumulated_constant,
                &ops.clone(),
                Ap::one(bit_width),
            ) {
                // Some interesting folding opportunity is present, so its
                // worthwhile to re-generate the operands list. Group the
                // operands by constant scale, to avoid multiplying by the same
                // constant scale multiple times.
                let mut mul_op_lists: BTreeMap<u128, Vec<Use>> = BTreeMap::new();
                for new_op in &new_ops {
                    mul_op_lists.entry(m[new_op].bits).or_default().push(*new_op);
                }
                // Re-generate the operands list.
                ops.clear();
                if !accumulated_constant.is_zero() {
                    ops.push(Use::of(self.get_constant(accumulated_constant)));
                }
                for (scale, list) in mul_op_lists {
                    if scale == 1 {
                        ops.push(self.get_add_expr(cx, list, Flags::default(), depth + 1));
                    } else if scale != 0 {
                        let sum = self.get_add_expr(cx, list, Flags::default(), depth + 1);
                        let c = Use::of(self.get_constant(Ap::new(bit_width, scale)));
                        ops.push(self.get_mul_expr(cx, vec![c, sum], Flags::default(), depth + 1));
                    }
                }
                if ops.is_empty() {
                    return Use::of(self.get_zero(cx, ty));
                }
                if ops.len() == 1 {
                    return ops[0];
                }
                return self.get_add_expr(cx, ops, Flags::default(), depth + 1);
            }
        }

        // If we are adding something to a multiply expression, make sure the
        // something is not already an operand of the multiply. If so, merge it
        // into the multiply.
        while idx < ops.len() && matches!(self.node(ops[idx].scev).kind, Kind::Mul(_)) {
            let mul = ops[idx].scev;
            let mul_operands = self.operands(mul).to_vec();
            for (mul_op, mul_op_use) in mul_operands.iter().enumerate() {
                // Scan all terms to find every occurrence of common factor
                // MulOpSCEV and fold them in one shot:
                //   A1*X + A2*X + ... + An*X  -->  X * (A1 + A2 + ... + An)
                let mul_op_scev = mul_op_use.scev;
                if self.as_constant(mul_op_scev).is_some() {
                    continue;
                }

                // Cofactors: 1 for bare addends matching MulOpSCEV, or the
                // remaining product for multiply terms containing MulOpSCEV.
                let mut cofactors: Vec<Use> = Vec::new();
                let mut dead_indices: Vec<usize> = Vec::new();
                for add_op in 0..ops.len() {
                    if mul_op_scev == ops[add_op].scev {
                        // W + X + (X * Y * Z)  -->  W + (X * ((Y*Z)+1))
                        cofactors.push(Use::of(self.get_one(cx, ty)));
                        dead_indices.push(add_op);
                        continue;
                    }

                    if add_op <= idx || !matches!(self.node(ops[add_op].scev).kind, Kind::Mul(_)) {
                        continue;
                    }

                    let other_mul = ops[add_op].scev;
                    let other_operands = self.operands(other_mul).to_vec();
                    for (o_mul_op, other) in other_operands.iter().enumerate() {
                        if other.scev == mul_op_scev {
                            // (A*B*C) + (A*D*E)  -->  A * (B*C + D*E)
                            cofactors.push(self.strip_factor(cx, other_mul, o_mul_op, depth));
                            dead_indices.push(add_op);
                            break;
                        }
                    }
                }

                // Fold all collected cofactors with the anchor multiply's
                // cofactor:
                //   MulOpSCEV * (Cofactor_1 + ... + Cofactor_n +
                // AnchorCofactor)
                if !cofactors.is_empty() {
                    cofactors.push(self.strip_factor(cx, mul, mul_op, depth));

                    let inner_sum = self.get_add_expr(cx, cofactors, Flags::default(), depth + 1);
                    let outer_mul =
                        self.get_mul_expr(cx, vec![Use::of(mul_op_scev), inner_sum], Flags::default(), depth + 1);

                    // DeadIndices does not include Idx (the anchor), hence +1.
                    if ops.len() == dead_indices.len() + 1 {
                        return outer_mul;
                    }

                    // Erase Ops[Idx] first, then erase DeadIndices in reverse
                    // order. The -1 adjustment accounts for the shift from
                    // removing Idx; reverse order means each erasure only
                    // shifts later positions, which have already been
                    // processed.
                    ops.remove(idx);
                    for dead in dead_indices.iter().rev() {
                        ops.remove(if *dead > idx { dead - 1 } else { *dead });
                    }

                    ops.push(outer_mul);
                    return self.get_add_expr(cx, ops, Flags::default(), depth + 1);
                }
            }
            idx += 1;
        }

        // If there are any add recurrences in the operands list, see if any
        // other added values are loop invariant. If so, we can fold them into
        // the recurrence.
        while idx < ops.len() && (self.scev_type(ops[idx].scev) as u8) < (Type::AddRec as u8) {
            idx += 1;
        }

        // Scan over all recurrences, trying to fold loop invariants into them.
        while idx < ops.len() && matches!(self.node(ops[idx].scev).kind, Kind::AddRec(..)) {
            // Scan all of the other operands to this add and add them to the
            // vector if they are loop invariant w.r.t. the recurrence.
            let mut li_ops: Vec<Use> = Vec::new();
            let add_rec = ops[idx].scev;
            let Kind::AddRec(_, add_rec_loop) = self.node(add_rec).kind else { unreachable!() };
            let mut i = 0usize;
            while i < ops.len() {
                if self.is_available_at_loop_entry(cx, ops[i].scev, add_rec_loop) {
                    li_ops.push(ops[i]);
                    ops.remove(i);
                } else {
                    i += 1;
                }
            }

            // If we found some loop invariants, fold them into the recurrence.
            if !li_ops.is_empty() {
                // Compute nowrap flags for the addition of the loop-invariant
                // ops and the addrec. Temporarily push it as an operand for
                // that purpose. These flags are valid in the scope of the
                // addrec only.
                li_ops.push(Use::of(add_rec));
                let mut computed = self.strengthen_no_wrap_flags(cx, Type::Add, &li_ops, orig_flags);
                li_ops.pop();

                //  NLI + LI + {Start,+,Step}  -->  NLI + {LI+Start,+,Step}
                let start = self.addrec_start(add_rec);
                li_ops.push(Use::of(start));

                let mut add_rec_ops: Vec<Use> = self.operands(add_rec).to_vec();

                // It is not in general safe to propagate flags valid on an add
                // within the addrec scope to one outside it. We must prove that
                // the inner scope is guaranteed to execute if the outer one
                // does to be able to safely propagate. We know the program is
                // undefined if poison is produced on the inner scoped addrec.
                // We also know that *for this use* the outer scoped add can't
                // overflow (because of the flags we just computed for the inner
                // scoped add) without the program being undefined. Proving that
                // entry to the outer scope neccesitates entry to the inner
                // scope, thus proves the program undefined if the flags would
                // be violated in the outer scope.
                let mut add_flags = computed;
                if add_flags != flags::NONE
                    && !self.is_guaranteed_to_transfer_execution_to_loop_header(cx, &li_ops, add_rec_loop)
                {
                    add_flags = flags::NONE;
                }
                add_rec_ops[0] = self.get_add_expr(cx, li_ops, Flags::expr(add_flags), depth + 1);

                // Build the new addrec. Propagate the NUW and NSW flags if both
                // the outer add and the inner addrec are guaranteed to have no
                // overflow. Always propagate NW.
                computed = self.get_no_wrap_flags(add_rec, computed | flags::NW);
                let new_rec = self.get_add_rec_expr(cx, add_rec_ops, add_rec_loop, Flags::expr(computed)).scev;

                // If all of the other operands were loop invariant, we are
                // done.
                if ops.len() == 1 {
                    return Use::of(new_rec);
                }

                // Otherwise, add the folded AddRec by the non-invariant parts.
                // DEVIATE 8 (permanent): LLVM compares `Ops[i] == AddRec` with
                // the use bits, so a recurrence held with a use
                // flag is never found and the loop runs off the
                // array. Compare the node.
                // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L2939
                let at = ops.iter().position(|op| op.scev == add_rec).expect("the add recurrence is still there");
                ops[at] = Use::of(new_rec);
                return self.get_add_expr(cx, ops, Flags::default(), depth + 1);
            }

            // Okay, if there weren't any loop invariants to be folded, check to
            // see if there are multiple AddRec's with the same loop induction
            // variable being added together. If so, we can fold them.
            let mut other_idx = idx + 1;
            while other_idx < ops.len() && matches!(self.node(ops[other_idx].scev).kind, Kind::AddRec(..)) {
                // We expect the AddRecExpr's to be sorted in reverse dominance
                // order, so that the 1st found AddRecExpr is dominated by all
                // others.
                let Kind::AddRec(_, other_loop) = self.node(ops[other_idx].scev).kind else { unreachable!() };
                debug_assert!(
                    cx.tree.dominates(other_loop, add_rec_loop),
                    "AddRecExprs are not sorted in reverse dominance order?"
                );
                if add_rec_loop == other_loop {
                    // Other + {A,+,B}<L> + {C,+,D}<L>  -->  Other +
                    // {A+C,+,B+D}<L>
                    let mut add_rec_ops: Vec<Use> = self.operands(add_rec).to_vec();
                    while other_idx != ops.len() && matches!(self.node(ops[other_idx].scev).kind, Kind::AddRec(..)) {
                        let other_add_rec = ops[other_idx].scev;
                        let Kind::AddRec(_, this_loop) = self.node(other_add_rec).kind else { unreachable!() };
                        if this_loop == add_rec_loop {
                            let other_operands = self.operands(other_add_rec).to_vec();
                            for (i, other_op) in other_operands.iter().enumerate() {
                                if i >= add_rec_ops.len() {
                                    add_rec_ops.extend_from_slice(&other_operands[i..]);
                                    break;
                                }
                                add_rec_ops[i] =
                                    self.get_add_expr(cx, vec![add_rec_ops[i], *other_op], Flags::default(), depth + 1);
                            }
                            ops.remove(other_idx);
                            // --OtherIdx; then the loop's ++OtherIdx below.
                            other_idx = other_idx.wrapping_sub(1);
                        }
                        other_idx = other_idx.wrapping_add(1);
                    }
                    // Step size has changed, so we cannot guarantee no
                    // self-wraparound.
                    ops[idx] = self.get_add_rec_expr(cx, add_rec_ops, add_rec_loop, Flags::default());
                    return self.get_add_expr(cx, ops, Flags::default(), depth + 1);
                }
                other_idx += 1;
            }

            // Otherwise couldn't fold anything into this recurrence. Move onto
            // the next one.
            idx += 1;
        }

        // Okay, it looks like we really DO need an add expr. Check to see if we
        // already have one, otherwise create a new one.
        #[cfg(debug_assertions)]
        debug_assert!(use_flags == flags::NONE || orig_ops == ops, "Tried to add SCEVUse flags after operands changed");
        let computed = self.strengthen_no_wrap_flags(cx, Type::Add, &ops, orig_flags);
        let s = self.get_or_create_add_expr(&ops, computed);
        self.with_flags(s, use_flags)
    }

    /// `StripFactor` of `getAddExpr`: the product of all operands of `m`
    /// except the one at `op_idx`.
    fn strip_factor(
        &mut self,
        cx: &Cx,
        m: Scev,
        op_idx: usize,
        depth: u32,
    ) -> Use {
        let operands = self.operands(m).to_vec();
        if operands.len() == 2 {
            return operands[usize::from(op_idx == 0)];
        }
        let mut remaining: Vec<Use> = operands[..op_idx].to_vec();
        remaining.extend_from_slice(&operands[op_idx + 1..]);
        self.get_mul_expr(cx, remaining, Flags::default(), depth + 1)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3112
    /// `containsConstantInAddMulChain`: whether any of the operands of
    /// `start_expr` is a constant, or any of the add or multiply expressions
    /// in it contains one.
    fn contains_constant_in_add_mul_chain(
        &self,
        start_expr: Scev,
    ) -> bool {
        let mut found = false;
        let mut visited = HashSet::default();
        let mut worklist = Vec::new();
        let mut push = |s: Scev, found: &mut bool, worklist: &mut Vec<Scev>, se: &Self| {
            if visited.insert(s) {
                // follow
                *found |= matches!(se.node(s).kind, Kind::Constant(_));
                if matches!(se.node(s).kind, Kind::Add(_) | Kind::Mul(_)) {
                    worklist.push(s);
                }
            }
        };
        push(start_expr, &mut found, &mut worklist, self);
        while let Some(s) = worklist.pop() {
            if found {
                break;
            }
            for op in self.operands_of(s) {
                push(op.scev, &mut found, &mut worklist, self);
                if found {
                    break;
                }
            }
        }
        found
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3133
    /// `ScalarEvolution::getMulExpr`: a canonical multiply expression, or
    /// something simpler if possible.
    pub fn get_mul_expr(
        &mut self,
        cx: &Cx,
        mut ops: Vec<Use>,
        f: Flags,
        depth: u32,
    ) -> Use {
        let orig_flags = f.expr;
        let use_flags = f.use_;
        debug_assert!(orig_flags == orig_flags & (flags::NUW | flags::NSW), "only nuw or nsw allowed");
        debug_assert!(use_flags == use_flags & (flags::NUW | flags::NSW), "only nuw or nsw allowed");
        debug_assert!(!ops.is_empty(), "Cannot get empty mul!");
        if ops.len() == 1 {
            return ops[0];
        }
        #[cfg(debug_assertions)]
        {
            let ety = self.type_of(ops[0].scev);
            debug_assert!(!ety.is_pointer());
            for op in &ops[1..] {
                debug_assert!(self.type_of(op.scev) == ety, "SCEVMulExpr operand types don't match!");
            }
        }

        if let Some(folded) = self.constant_fold_and_group_ops(
            cx,
            &mut ops,
            &|c1, c2| c1.mul(c2),
            &|c| c.is_one(),  // identity
            &|c| c.is_zero(), // absorber
        ) {
            return Use::of(folded);
        }

        #[cfg(debug_assertions)]
        let orig_ops = ops.clone();

        // Limit recursion calls depth.
        if depth > MAX_ARITH_DEPTH || self.has_huge_expression(&ops) {
            let computed = self.strengthen_no_wrap_flags(cx, Type::Mul, &ops, orig_flags);
            let s = self.get_or_create_mul_expr(&ops, computed);
            return self.with_flags(s, use_flags);
        }

        if let Some(s) = self.find_existing_scev_in_cache(Type::Mul, &ops) {
            // Don't strengthen flags if we have no new information.
            if self.get_no_wrap_flags(s, orig_flags) != orig_flags {
                let computed = self.strengthen_no_wrap_flags(cx, Type::Mul, &ops, orig_flags);
                self.nodes[s.0 as usize].flags |= computed;
            }
            return self.with_flags(s, use_flags);
        }

        if let Some(lhsc) = self.as_constant(ops[0].scev)
            && ops.len() == 2
        {
            let lhsc_scev = ops[0].scev;
            // C1*(C2+V) -> C1*C2 + C1*V
            // If any of Add's ops are Adds or Muls with a constant, apply this
            // transformation as well.
            //
            // TODO: There are some cases where this transformation is not
            // profitable; for example, Add = (C0 + X) * Y + Z. Maybe the scope
            // of this transformation should be narrowed down.
            if let Some((op0, op1)) = self.match_binary(ops[1].scev, Type::Add, 0)
                && self.contains_constant_in_add_mul_chain(ops[1].scev)
            {
                let lhs = self.get_mul_expr(cx, vec![Use::of(lhsc_scev), Use::of(op0)], Flags::default(), depth + 1);
                let rhs = self.get_mul_expr(cx, vec![Use::of(lhsc_scev), Use::of(op1)], Flags::default(), depth + 1);
                return self.get_add_expr(cx, vec![Use::of(lhs.scev), Use::of(rhs.scev)], Flags::default(), depth + 1);
            }

            if self.match_all_ones(ops[0].scev) {
                // If we have a mul by -1 of an add, try distributing the -1
                // among the add operands.
                if matches!(self.node(ops[1].scev).kind, Kind::Add(_)) {
                    let add = ops[1].scev;
                    let mut new_ops: Vec<Use> = Vec::new();
                    let mut any_folded = false;
                    for add_op in self.operands(add).to_vec() {
                        let mul =
                            self.get_mul_expr(cx, vec![ops[0], Use::of(add_op.scev)], Flags::default(), depth + 1).scev;
                        if !matches!(self.node(mul).kind, Kind::Mul(_)) {
                            any_folded = true;
                        }
                        new_ops.push(Use::of(mul));
                    }
                    if any_folded {
                        return self.get_add_expr(cx, new_ops, Flags::default(), depth + 1);
                    }
                } else if let Kind::AddRec(_, add_rec_loop) = self.node(ops[1].scev).kind {
                    let add_rec = ops[1].scev;
                    // Negation preserves a recurrence's no self-wrap property.
                    let mut operands: Vec<Use> = Vec::new();
                    for add_rec_op in self.operands(add_rec).to_vec() {
                        operands.push(self.get_mul_expr(
                            cx,
                            vec![ops[0], Use::of(add_rec_op.scev)],
                            Flags::default(),
                            depth + 1,
                        ));
                    }
                    // Let M be the minimum representable signed value. AddRec
                    // with nsw multiplied by -1 can have signed overflow if and
                    // only if it takes a value of M: M * (-1) would stay M and
                    // (M + 1) * (-1) would be the maximum signed value. In all
                    // other cases signed overflow is impossible.
                    let mut flags_mask = flags::NW;
                    if self.get_no_wrap_flags(add_rec, flags::NSW) != 0 {
                        let bits = self.get_type_size_in_bits(cx, self.type_of(add_rec));
                        let min_int = Ap::signed_min(bits);
                        let range_min = self.get_signed_range(cx, add_rec).get_signed_min();
                        if Ap::new(bits, range_min) != min_int {
                            flags_mask |= flags::NSW;
                        }
                    }
                    let ar_flags = self.get_no_wrap_flags(add_rec, flags_mask);
                    return self.get_add_rec_expr(cx, operands, add_rec_loop, Flags::expr(ar_flags));
                }
            }

            // Try to push the constant operand into a ZExt: C * zext (A + B) ->
            // zext (C*A + C*B) if trunc (C) * (A + B) does not unsigned-wrap.
            if let Some(inner_add) = self.match_unary(ops[1].scev, Type::ZeroExtend)
                && matches!(self.node(inner_add).kind, Kind::Add(_))
            {
                let narrow_c = self.get_truncate_expr(cx, Use::of(lhsc_scev), self.type_of(inner_add), 0);
                let first = self.operands(inner_add)[0].scev;
                if self.as_constant(first).is_some()
                    && self.get_zero_extend_expr(cx, Use::of(narrow_c), self.type_of(ops[1].scev), 0) == lhsc_scev
                    && has_flags(
                        self.strengthen_no_wrap_flags(
                            cx,
                            Type::Mul,
                            &[Use::of(narrow_c), Use::of(inner_add)],
                            flags::NONE,
                        ),
                        flags::NUW,
                    )
                {
                    let res = self
                        .get_mul_expr(
                            cx,
                            vec![Use::of(narrow_c), Use::of(inner_add)],
                            Flags::expr(flags::NUW),
                            depth + 1,
                        )
                        .scev;
                    return Use::of(self.get_zero_extend_expr(cx, Use::of(res), self.type_of(ops[1].scev), depth + 1));
                }
            }

            // Try to fold (C1 * D /u C2) -> C1/C2 * D, if C1 and C2 are
            // powers-of-2, D is a multiple of C2, and C1 is a multiple of C2.
            // If C2 is a multiple of C1, fold to (D /u (C2 /u C1)).
            let mut c1v = lhsc;
            // (C1 * D /u C2) == -1 * -C1 * D /u C2 when C1 != INT_MIN. Don't
            // treat -1 as -1 * 1, as it won't enable additional folds.
            if c1v.is_negative() && !c1v.is_signed_min() && !c1v.is_all_ones() {
                c1v = abs_ap(c1v);
            }
            if c1v.is_power_of_2()
                && let Some((d, c2_scev)) = self.match_binary(ops[1].scev, Type::UDiv, 0)
                && let Some(c2) = self.as_constant(c2_scev)
                && c2.is_power_of_2()
                && c1v.countr_zero() <= self.get_min_trailing_zeros(cx, d, None)
            {
                let mut new_mul: Option<Scev> = None;
                if !c1v.ult(c2) {
                    let quotient = self.get_constant(c1v);
                    let udiv = self.get_udiv_expr(cx, Use::of(quotient), Use::of(c2_scev));
                    new_mul = Some(self.get_mul_expr(cx, vec![Use::of(udiv), Use::of(d)], Flags::default(), 0).scev);
                } else if c2.countr_zero() <= self.get_min_trailing_zeros(cx, d, None) {
                    debug_assert!(!c1v.is_one() && !c1v.is_zero(), "C1 <= 1 should have been folded earlier");
                    let c1_scev = self.get_constant(c1v);
                    let ratio = self.get_udiv_expr(cx, Use::of(c2_scev), Use::of(c1_scev));
                    new_mul = Some(self.get_udiv_expr(cx, Use::of(d), Use::of(ratio)));
                }
                if let Some(new_mul) = new_mul {
                    return Use::of(if c1v == lhsc { new_mul } else { self.get_negative_scev(cx, new_mul, 0) });
                }
            }
        }

        // Skip over the add expression until we get to a multiply.
        let mut idx = 0usize;
        while idx < ops.len() && (self.scev_type(ops[idx].scev) as u8) < (Type::Mul as u8) {
            idx += 1;
        }

        // If there are mul operands inline them all into this expression.
        if idx < ops.len() {
            let mut deleted_mul = false;
            while idx < ops.len() && matches!(self.node(ops[idx].scev).kind, Kind::Mul(_)) {
                let mul = ops[idx].scev;
                if ops.len() > MUL_OPS_INLINE_THRESHOLD {
                    break;
                }
                // If we have an mul, expand the mul operands onto the end of
                // the operands list.
                ops.remove(idx);
                ops.extend_from_slice(&self.operands(mul).to_vec());
                deleted_mul = true;
            }

            // If we deleted at least one mul, we added operands to the end of
            // the list, and they are not necessarily sorted. Recurse to resort
            // and resimplify any operands we just acquired.
            if deleted_mul {
                return self.get_mul_expr(cx, ops, Flags::default(), depth + 1);
            }
        }

        // If there are any add recurrences in the operands list, see if any
        // other added values are loop invariant. If so, we can fold them into
        // the recurrence.
        while idx < ops.len() && (self.scev_type(ops[idx].scev) as u8) < (Type::AddRec as u8) {
            idx += 1;
        }

        // Scan over all recurrences, trying to fold loop invariants into them.
        while idx < ops.len() && matches!(self.node(ops[idx].scev).kind, Kind::AddRec(..)) {
            // Scan all of the other operands to this mul and add them to the
            // vector if they are loop invariant w.r.t. the recurrence.
            let mut li_ops: Vec<Use> = Vec::new();
            let mut add_rec = ops[idx].scev;
            let Kind::AddRec(_, add_rec_loop) = self.node(add_rec).kind else { unreachable!() };
            let mut i = 0usize;
            while i < ops.len() {
                if self.is_available_at_loop_entry(cx, ops[i].scev, add_rec_loop) {
                    li_ops.push(ops[i]);
                    ops.remove(i);
                } else {
                    i += 1;
                }
            }

            // If we found some loop invariants, fold them into the recurrence.
            if !li_ops.is_empty() {
                //  NLI * LI * {Start,+,Step}  -->  NLI * {LI*Start,+,LI*Step}
                let mut new_ops: Vec<Use> = Vec::new();
                // `const SCEV *Scale`: the result's use flags are dropped.
                let scale = Use::of(self.get_mul_expr(cx, li_ops, Flags::default(), depth + 1).scev);

                // If both the mul and addrec are nuw, we can preserve nuw. If
                // both the mul and addrec are nsw, we can only preserve nsw if
                // either a) they are also nuw, or b) all multiplications of
                // addrec operands with scale are nsw.
                let computed = self.strengthen_no_wrap_flags(cx, Type::Mul, &[scale, Use::of(add_rec)], orig_flags);
                let mut fl = self.get_no_wrap_flags(add_rec, computed);

                let add_rec_operands = self.operands(add_rec).to_vec();
                for add_rec_op in &add_rec_operands {
                    new_ops.push(self.get_mul_expr(cx, vec![scale, *add_rec_op], Flags::default(), depth + 1));

                    if has_flags(fl, flags::NSW) && !has_flags(fl, flags::NUW) {
                        let scale_range = self.get_signed_range(cx, scale.scev);
                        let nsw_region = llrm_mir::constantrange::ConstantRange::make_guaranteed_no_wrap_region(
                            BinaryOp::Mul,
                            &scale_range,
                            llrm_mir::constantrange::NO_SIGNED_WRAP,
                        );
                        if !nsw_region.contains_range(&self.get_signed_range(cx, add_rec_op.scev)) {
                            fl &= !flags::NSW;
                        }
                    }
                }

                let new_rec = self.get_add_rec_expr(cx, new_ops, add_rec_loop, Flags::expr(fl)).scev;

                // If all of the other operands were loop invariant, we are
                // done.
                if ops.len() == 1 {
                    return Use::of(new_rec);
                }

                // Otherwise, multiply the folded AddRec by the non-invariant
                // parts.
                // DEVIATE 8 (permanent): LLVM compares `Ops[i] == AddRec` with
                // the use bits, so a recurrence held with a use
                // flag is never found and the loop runs off the
                // array. Compare the node.
                // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3357
                let at = ops.iter().position(|op| op.scev == add_rec).expect("the add recurrence is still there");
                ops[at] = Use::of(new_rec);
                return self.get_mul_expr(cx, ops, Flags::default(), depth + 1);
            }

            // Okay, if there weren't any loop invariants to be folded, check to
            // see if there are multiple AddRec's with the same loop induction
            // variable being multiplied together. If so, we can fold them.
            //
            // {A1,+,A2,+,...,+,An}<L> * {B1,+,B2,+,...,+,Bn}<L>
            // = {x=1 in [ sum y=x..2x [ sum z=max(y-x, y-n)..min(x,n) [
            //       choose(x, 2x)*choose(2x-y, x-z)*A_{y-z}*B_z
            //   ]]],+,...up to x=2n}.
            // Note that the arguments to choose() are always integers with
            // values known at compile time, never SCEV objects.
            //
            // The implementation avoids pointless extra computations when the
            // two addrec's are of different length (mathematically, it's
            // equivalent to an infinite stream of zeros on the right).
            let mut ops_modified = false;
            let mut other_idx = idx + 1;
            while other_idx != ops.len() && matches!(self.node(ops[other_idx].scev).kind, Kind::AddRec(..)) {
                let other_add_rec = ops[other_idx].scev;
                let Kind::AddRec(_, other_loop) = self.node(other_add_rec).kind else { unreachable!() };
                if other_loop != add_rec_loop {
                    other_idx += 1;
                    continue;
                }

                // Limit max number of arguments to avoid creation of
                // unreasonably big SCEVAddRecs with very complex operands.
                let (ar_ops, other_ops) = (self.operands(add_rec).to_vec(), self.operands(other_add_rec).to_vec());
                if ar_ops.len() + other_ops.len() - 1 > MAX_ADD_REC_SIZE
                    || self.has_huge_expression(&[Use::of(add_rec), Use::of(other_add_rec)])
                {
                    other_idx += 1;
                    continue;
                }

                let mut overflow = false;
                let ty = self.type_of(add_rec);
                let larger_than_64_bits = self.get_type_size_in_bits(cx, ty) > 64;
                let mut add_rec_ops: Vec<Use> = Vec::new();
                let xe = (ar_ops.len() + other_ops.len() - 1) as i64;
                let mut x = 0i64;
                while x != xe && !overflow {
                    let mut sum_ops: Vec<Use> = Vec::new();
                    let mut y = x;
                    let ye = 2 * x + 1;
                    while y != ye && !overflow {
                        let coeff1 = choose(x as u64, (2 * x - y) as u64, &mut overflow);
                        let mut z = (y - x).max(y - ar_ops.len() as i64 + 1);
                        let ze = (x + 1).min(other_ops.len() as i64);
                        while z < ze && !overflow {
                            let coeff2 = choose((2 * x - y) as u64, (x - z) as u64, &mut overflow);
                            let coeff = if larger_than_64_bits {
                                umul_ov(coeff1, coeff2, &mut overflow)
                            } else {
                                coeff1.wrapping_mul(coeff2)
                            };
                            let coeff_term = self.get_constant_of_type(cx, ty, coeff as i128);
                            let term1 = ar_ops[(y - z) as usize];
                            let term2 = other_ops[z as usize];
                            sum_ops.push(self.get_mul_expr(
                                cx,
                                vec![Use::of(coeff_term), term1, term2],
                                Flags::default(),
                                depth + 1,
                            ));
                            z += 1;
                        }
                        y += 1;
                    }
                    if sum_ops.is_empty() {
                        sum_ops.push(Use::of(self.get_zero(cx, ty)));
                    }
                    add_rec_ops.push(self.get_add_expr(cx, sum_ops, Flags::default(), depth + 1));
                    x += 1;
                }
                if !overflow {
                    let new_add_rec = self.get_add_rec_expr(cx, add_rec_ops, add_rec_loop, Flags::default()).scev;
                    if ops.len() == 2 {
                        return Use::of(new_add_rec);
                    }
                    ops[idx] = Use::of(new_add_rec);
                    ops.remove(other_idx);
                    other_idx = other_idx.wrapping_sub(1);
                    ops_modified = true;
                    if !matches!(self.node(new_add_rec).kind, Kind::AddRec(..)) {
                        break;
                    }
                    add_rec = new_add_rec;
                }
                other_idx = other_idx.wrapping_add(1);
            }
            if ops_modified {
                return self.get_mul_expr(cx, ops, Flags::default(), depth + 1);
            }

            // Otherwise couldn't fold anything into this recurrence. Move onto
            // the next one.
            idx += 1;
        }

        // Okay, it looks like we really DO need an mul expr. Check to see if we
        // already have one, otherwise create a new one.
        #[cfg(debug_assertions)]
        debug_assert!(use_flags == flags::NONE || orig_ops == ops, "Tried to add SCEVUse flags after operands changed");
        let computed = self.strengthen_no_wrap_flags(cx, Type::Mul, &ops, orig_flags);
        let s = self.get_or_create_mul_expr(&ops, computed);
        self.with_flags(s, use_flags)
    }
}

/// `APInt::abs`.
fn abs_ap(c: Ap) -> Ap {
    if c.is_negative() { c.neg() } else { c }
}
