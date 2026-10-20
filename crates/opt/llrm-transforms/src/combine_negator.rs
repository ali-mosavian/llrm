//! `Negator`: sinks a negation into an expression tree when that does not add
//! an instruction.
//!
//! Translation of
//! [`InstCombineNegator.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineNegator.cpp)
//! (`Negator::Negate` :547, `run` :526, `negate` :485, `visitImpl` :114).
//!
//! MIRROR: the C++ order of cases and the cache of negations tried. The
//! Builder is `TargetFolder`'s: an operation on constants folds, any other is
//! inserted before the instruction being negated. A failed negation erases
//! what it created, last first.
//!
//! DEVIATE 1: no vectors: the `shufflevector`, `extractelement` and
//! `insertelement` cases do not exist in MIR.
//!
//! DEVIATE 2: the `scmp`/`ucmp` case (`CmpIntrinsic`, :201) is not
//! translated: it builds an intrinsic call (`combine_build` DEVIATE 2).
//!
//! DEVIATE 3: no statistics, `DebugCounter` or `-instcombine-negator-enabled`
//! (always on). `NegatorDefaultMaxDepth` is `~0U` at the pin: the recursion
//! is as deep as the tree of one-use operands, as in LLVM.
//!
//! DEVIATE 4: constant expressions are not negated (MIR keeps them for
//! global initializers only).
//!
//! # How it works
//! `negate` asks the cache, else `visit`; `visit` answers the cases that need
//! no recursion first, then those that need one use, then recurses into the
//! operands. New instructions are queued on the worklist at insert, in the
//! order `Negate` hands them to `addToWorklist`.
//!
//! # Used by
//! `combine_addsub::visit_sub`, `combine_muldivrem`.
//!
//! # Example
//! `0 - (x + 1)` is `~x`; `0 - (a - b)` is `b - a`.

use llrm_mir::context::{ConstantKind, mask};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::types::TypeId;
use llrm_mir::valuetracking::range::{SelectOperand, is_known_negation};
use llrm_mir::valuetracking::{Query, pattern};
use llrm_support::hash::HashMap;

use crate::combine::Combiner;
use crate::combine_shared::{
    binop, drop_poison_generating_flags, fold_constants, get_complexity, has_one_use, inst_def, is_commutative, query,
    width,
};

impl Combiner<'_> {
    /// `Negator::Negate(LHSIsZero, IsNSW, Root, IC)`: `-root` with the
    /// negation sunk into `root`'s tree, else nothing and no change.
    pub fn negate(
        &mut self,
        lhs_is_zero: bool,
        is_nsw: bool,
        root: Operand,
    ) -> Option<Operand> {
        let mut negator = Negator { c: self, truly: lhs_is_zero, cache: HashMap::default(), created: Vec::new() };
        let negated = negator.negate(root, is_nsw, 0);
        if negated.is_none() {
            // Erase what was made, to avoid endless combine looping.
            for inst in std::mem::take(&mut negator.created).into_iter().rev() {
                negator.c.erase(inst);
            }
        }
        negated
    }
}

struct Negator<'c, 'a> {
    c: &'c mut Combiner<'a>,
    /// `IsTrulyNegation`: the root is `sub 0, x`.
    truly: bool,
    /// `NegationsCache`.
    cache: HashMap<Operand, Option<Operand>>,
    /// `NewInstructions`.
    created: Vec<InstId>,
}

impl Negator<'_, '_> {
    /// The value of an inserted instruction, recorded as created.
    fn make(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
    ) -> Operand {
        let new = self.c.insert_before(at, opcode, ty, operands, flags);
        self.created.push(new);
        Operand::Value(self.c.function.instruction(new).result.expect("a value"))
    }

    /// `Builder.CreateBinOp` over `TargetFolder`: constants fold.
    fn binop(
        &mut self,
        at: InstId,
        op: BinaryOp,
        a: Operand,
        b: Operand,
        flags: Flags,
    ) -> Operand {
        if let Some(folded) = fold_constants(self.c, op, a, b) {
            return folded;
        }
        let ty = self.c.type_of(a);
        self.make(at, Opcode::Binary(op), ty, vec![a, b], flags)
    }

    /// `Builder.CreateNot`.
    fn not(
        &mut self,
        at: InstId,
        v: Operand,
    ) -> Operand {
        let ones = self.c.all_ones(self.c.type_of(v));
        self.binop(at, BinaryOp::Xor, v, ones, Flags::default())
    }

    /// `Builder.CreateAdd(v, 1)`.
    fn increment(
        &mut self,
        at: InstId,
        v: Operand,
    ) -> Operand {
        let one = self.c.int(self.c.type_of(v), 1);
        self.binop(at, BinaryOp::Add, v, one, Flags::default())
    }

    /// `getSortedOperandsOfBinOp`: a commutative operation's operands, the
    /// simpler last.
    fn sorted_operands(
        &self,
        inst: InstId,
    ) -> [Operand; 2] {
        let instruction = self.c.function.instruction(inst);
        let (a, b) = (instruction.operands[0], instruction.operands[1]);
        let q = query(self.c, None);
        let commutative = matches!(instruction.opcode, Opcode::Binary(op) if is_commutative(op));
        if commutative && get_complexity(&q, a) < get_complexity(&q, b) { [b, a] } else { [a, b] }
    }

    /// `Negator::negate`: the cache, else `visit`.
    fn negate(
        &mut self,
        v: Operand,
        is_nsw: bool,
        depth: u32,
    ) -> Option<Operand> {
        if let Some(&known) = self.cache.get(&v) {
            return known;
        }
        let negated = self.visit(v, is_nsw, depth);
        self.cache.insert(v, negated);
        negated
    }

    /// `Negator::visitImpl`.
    fn visit(
        &mut self,
        v: Operand,
        is_nsw: bool,
        depth: u32,
    ) -> Option<Operand> {
        let ty = self.c.type_of(v);
        let bits = width(self.c, v);
        // -(undef) -> undef.
        if let Operand::Constant(k) = v
            && matches!(self.c.context.get(k).kind, ConstantKind::Undef)
        {
            return Some(v);
        }
        // In i1, negation can simply be ignored.
        if bits == 1 {
            return Some(v);
        }
        let (neg_of, constant) = {
            let q = query(self.c, None);
            (pattern::neg(&q, v).map(|(_, x, _)| x), pattern::apint(&q, v))
        };
        // -(-(X)) -> X.
        if let Some(x) = neg_of {
            return Some(x);
        }
        // Integral constants can be freely negated.
        if let Some((k, w)) = constant {
            return Some(self.c.int(ty, k.wrapping_neg() & mask(w)));
        }
        // If we have a non-instruction, then give up.
        let inst = inst_def(self.c, v)?;
        // A true negation can negate an instruction with other uses when it
        // needs no recursive reasoning, without increasing the count.
        if !has_one_use(self.c, v) && !self.truly {
            return None;
        }
        let instruction = self.c.function.instruction(inst);
        let (opcode, operands, flags) = (instruction.opcode.clone(), instruction.operands.to_vec(), instruction.flags);

        // In some cases we can give the answer without further recursion.
        match &opcode {
            Opcode::Binary(BinaryOp::Add) => {
                let ops = self.sorted_operands(inst);
                // `inc` is always negatible.
                if pattern::one(&query(self.c, None), ops[1]) {
                    return Some(self.not(inst, ops[0]));
                }
            }
            Opcode::Binary(BinaryOp::Xor) => {
                // `not` is always negatible.
                let x = pattern::not(&query(self.c, None), v);
                if let Some(x) = x {
                    return Some(self.increment(inst, x));
                }
            }
            Opcode::Binary(shift @ (BinaryOp::AShr | BinaryOp::LShr)) => {
                // Right-shift sign bit smear is negatible.
                let full =
                    pattern::apint(&query(self.c, None), operands[1]).is_some_and(|(k, _)| k == (bits - 1) as u128);
                if full {
                    let opposite = if *shift == BinaryOp::AShr { BinaryOp::LShr } else { BinaryOp::AShr };
                    return Some(self.make(inst, Opcode::Binary(opposite), ty, vec![operands[0], operands[1]], flags));
                }
                // An exact ashr could be an sdiv, which is much worse.
            }
            Opcode::Cast(extension @ (CastOp::SExt | CastOp::ZExt)) => {
                // `*ext` of i1 is always negatible.
                if width(self.c, operands[0]) == 1 {
                    let opposite = if *extension == CastOp::SExt { CastOp::ZExt } else { CastOp::SExt };
                    return Some(self.make(inst, Opcode::Cast(opposite), ty, vec![operands[0]], Flags::default()));
                }
            }
            Opcode::Select => {
                // If both arms are constants, there is no recursion and no
                // limit by uses.
                let arms = {
                    let q = query(self.c, None);
                    pattern::apint(&q, operands[1]).zip(pattern::apint(&q, operands[2]))
                };
                if let Some(((t, tw), (f, fw))) = arms {
                    let neg_t = self.c.int(ty, t.wrapping_neg() & mask(tw));
                    let neg_f = self.c.int(ty, f.wrapping_neg() & mask(fw));
                    return Some(self.make(inst, Opcode::Select, ty, vec![operands[0], neg_t, neg_f], flags));
                }
            }
            _ => {}
        }

        if opcode == Opcode::Binary(BinaryOp::Sub) && (has_one_use(self.c, v) || is_int_constant(self.c, operands[0])) {
            // `sub` is always negatible, if the old one dies or it
            // subtracted from a constant.
            let nsw = is_nsw && flags.contains(Flags::NSW);
            let flags = if nsw { Flags::NSW } else { Flags::default() };
            return Some(self.binop(inst, BinaryOp::Sub, operands[1], operands[0], flags));
        }

        // Some other cases, while still not recursive, are for one use.
        if !has_one_use(self.c, v) {
            return None;
        }
        match &opcode {
            Opcode::Cast(CastOp::ZExt) => {
                // 0 - (zext (i8 X u>> 7) to iN) --> sext (i8 X s>> 7) to iN
                let src = operands[0];
                let src_width = width(self.c, src);
                let splat = {
                    let q = query(self.c, None);
                    binop(&q, src, BinaryOp::LShr).filter(|&(_, amount, _)| {
                        pattern::apint(&q, amount).is_some_and(|(k, _)| k == (src_width - 1) as u128)
                    })
                };
                if self.truly
                    && let Some((x, amount, _)) = splat
                {
                    let ashr = self.binop(inst, BinaryOp::AShr, x, amount, Flags::default());
                    return Some(self.make(inst, Opcode::Cast(CastOp::SExt), ty, vec![ashr], Flags::default()));
                }
            }
            Opcode::Binary(BinaryOp::And) => {
                // sub(0, and(lshr(x, C), 1)) --> ashr(shl(x, (BW-1)-C), BW-1),
                // only for a true negation (the general sub does not shrink).
                let found = {
                    let q = query(self.c, None);
                    let shifted = |v: Operand| {
                        binop(&q, v, BinaryOp::LShr)
                            .filter(|&(_, amount, _)| pattern::apint(&q, amount).is_some())
                            .map(|(x, amount, _)| (x, amount))
                    };
                    let trunc_or_self = |v: Operand| match pattern::cast(&q, v, CastOp::Trunc) {
                        Some((_, _, inner)) => shifted(inner),
                        None => shifted(v),
                    };
                    (pattern::one(&q, operands[1]) && has_one_use(self.c, operands[0]))
                        .then(|| trunc_or_self(operands[0]))
                        .flatten()
                };
                if self.truly
                    && let Some((x, amount)) = found
                {
                    let x_ty = self.c.type_of(x);
                    let x_bits = width(self.c, x);
                    let bw_minus_one = self.c.int(x_ty, (x_bits - 1) as u128);
                    let distance = self.binop(inst, BinaryOp::Sub, bw_minus_one, amount, Flags::default());
                    let shl = self.binop(inst, BinaryOp::Shl, x, distance, Flags::default());
                    let ashr = self.binop(inst, BinaryOp::AShr, shl, bw_minus_one, Flags::default());
                    if x_ty == ty {
                        return Some(ashr);
                    }
                    return Some(self.make(inst, Opcode::Cast(CastOp::Trunc), ty, vec![ashr], Flags::default()));
                }
            }
            Opcode::Binary(BinaryOp::SDiv) => {
                // `sdiv` is negatible if the divisor is not undef/INT_MIN/1.
                let divisor = pattern::apint(&query(self.c, None), operands[1]);
                if let Some((k, w)) = divisor
                    && k != 1u128 << (w - 1)
                    && k != 1
                {
                    let neg = self.c.int(ty, k.wrapping_neg() & mask(w));
                    let exact = flags.intersect(Flags::EXACT);
                    return Some(self.make(inst, Opcode::Binary(BinaryOp::SDiv), ty, vec![operands[0], neg], exact));
                }
            }
            _ => {}
        }

        // The rest of the logic is recursive.
        self.visit_operands(inst, &opcode, &operands, flags, is_nsw, depth)
    }

    /// The recursive half of `visitImpl`.
    fn visit_operands(
        &mut self,
        inst: InstId,
        opcode: &Opcode,
        operands: &[Operand],
        flags: Flags,
        is_nsw: bool,
        depth: u32,
    ) -> Option<Operand> {
        let ty = self.c.function.instruction(inst).ty;
        match opcode {
            Opcode::Freeze => {
                // `freeze` is negatible if its operand is.
                let negated = self.negate(operands[0], is_nsw, depth + 1)?;
                Some(self.make(inst, Opcode::Freeze, ty, vec![negated], Flags::default()))
            }
            Opcode::Phi => self.visit_phi(inst, operands, is_nsw, depth),
            Opcode::Select => {
                let known = {
                    let q = query(self.c, None);
                    is_known_negation(
                        &q,
                        SelectOperand::from(operands[1]),
                        SelectOperand::from(operands[2]),
                        false,
                        false,
                    )
                };
                if known {
                    // One arm is the negation of the other: swap them.
                    let swapped =
                        self.make(inst, Opcode::Select, ty, vec![operands[0], operands[2], operands[1]], flags);
                    // Poison-generating flags are dropped.
                    let (tv, fv) = (operands[2], operands[1]);
                    let q = query(self.c, None);
                    let tv_negates_fv = pattern::neg(&q, tv).is_some_and(|(_, x, _)| x == fv);
                    let fv_negates_tv = pattern::neg(&q, fv).is_some_and(|(_, x, _)| x == tv);
                    let drops: Vec<Operand> = if tv_negates_fv {
                        vec![tv]
                    } else if fv_negates_tv {
                        vec![fv]
                    } else {
                        vec![tv, fv]
                    };
                    for one in drops {
                        if let Some(def) = inst_def(self.c, one) {
                            drop_poison_generating_flags(self.c, def);
                        }
                    }
                    return Some(swapped);
                }
                // `select` is negatible if both hands are.
                let neg1 = self.negate(operands[1], is_nsw, depth + 1)?;
                let neg2 = self.negate(operands[2], is_nsw, depth + 1)?;
                Some(self.make(inst, Opcode::Select, ty, vec![operands[0], neg1, neg2], flags))
            }
            Opcode::Cast(CastOp::Trunc) => {
                // `trunc` is negatible if its operand is.
                let negated = self.negate(operands[0], false, depth + 1)?;
                Some(self.make(inst, Opcode::Cast(CastOp::Trunc), ty, vec![negated], Flags::default()))
            }
            Opcode::Binary(BinaryOp::Shl) => {
                // `shl` is negatible if the first operand is.
                let nsw = is_nsw && flags.contains(Flags::NSW);
                let nsw_flags = if nsw { Flags::NSW } else { Flags::default() };
                if let Some(negated) = self.negate(operands[0], nsw, depth + 1) {
                    return Some(self.binop(inst, BinaryOp::Shl, negated, operands[1], nsw_flags));
                }
                // Otherwise, `shl %x, C` is `mul %x, 1<<C`.
                if !is_int_constant(self.c, operands[1]) || !self.truly {
                    return None;
                }
                let ones = self.c.all_ones(self.c.type_of(operands[1]));
                let factor = self.binop(inst, BinaryOp::Shl, ones, operands[1], Flags::default());
                Some(self.binop(inst, BinaryOp::Mul, operands[0], factor, nsw_flags))
            }
            Opcode::Binary(BinaryOp::Or) if !flags.contains(Flags::DISJOINT) => None,
            Opcode::Binary(op @ (BinaryOp::Or | BinaryOp::Add)) => {
                if *op == BinaryOp::Or {
                    // `or`/`add` are interchangeable when no bits are shared.
                    // `inc` is always negatible.
                    let ops = self.sorted_operands(inst);
                    if pattern::one(&query(self.c, None), ops[1]) {
                        return Some(self.not(inst, ops[0]));
                    }
                }
                self.visit_add_operands(inst, operands, depth)
            }
            Opcode::Binary(BinaryOp::Xor) => {
                let ops = self.sorted_operands(inst);
                // `xor` is negatible if one of its operands is invertible.
                let constant = pattern::apint(&query(self.c, None), ops[1]);
                match constant {
                    Some((k, w)) if self.truly => {
                        let inverted = self.c.int(ty, !k & mask(w));
                        let xor = self.binop(inst, BinaryOp::Xor, ops[0], inverted, Flags::default());
                        Some(self.increment(inst, xor))
                    }
                    _ => None,
                }
            }
            Opcode::Binary(BinaryOp::Mul) => {
                let ops = self.sorted_operands(inst);
                // `mul` is negatible if one of its operands is. The second
                // first: a constant is best inverted.
                let (negated, other) = match self.negate(ops[1], false, depth + 1) {
                    Some(negated) => (negated, ops[0]),
                    None => (self.negate(ops[0], false, depth + 1)?, ops[1]),
                };
                let nsw = is_nsw && flags.contains(Flags::NSW);
                Some(self.binop(inst, BinaryOp::Mul, negated, other, if nsw { Flags::NSW } else { Flags::default() }))
            }
            _ => None,
        }
    }

    /// The `phi` case: negatible if every incoming value is.
    fn visit_phi(
        &mut self,
        inst: InstId,
        operands: &[Operand],
        is_nsw: bool,
        depth: u32,
    ) -> Option<Operand> {
        let ty = self.c.function.instruction(inst).ty;
        let block = self.c.function.parent(inst)?;
        let mut negated = Vec::with_capacity(operands.len());
        for pair in operands.chunks(2) {
            let Operand::Block(from) = pair[1] else { return None };
            // Don't negate indvars to avoid infinite loops.
            if self.c.env.analyses.dominator_tree().dominates(block, from) {
                return None;
            }
            negated.push(self.negate(pair[0], is_nsw, depth + 1)?);
            negated.push(pair[1]);
        }
        Some(self.make(inst, Opcode::Phi, ty, negated, Flags::default()))
    }

    /// The `add` case: negatible if both operands are, or, for a true
    /// negation, one: `0-(a+b) --> (-a)-b`.
    fn visit_add_operands(
        &mut self,
        inst: InstId,
        operands: &[Operand],
        depth: u32,
    ) -> Option<Operand> {
        let mut negated = Vec::new();
        let mut kept = Vec::new();
        for &op in operands {
            // Can we sink the negation into this operand?
            match self.negate(op, false, depth + 1) {
                Some(neg) => negated.push(neg),
                None if self.truly => kept.push(op),
                None => return None,
            }
        }
        match (negated.len(), kept.len()) {
            // Then we get to keep the `add`.
            (2, _) => Some(self.binop(inst, BinaryOp::Add, negated[0], negated[1], Flags::default())),
            // Completely failed to sink the negation.
            (_, 2) => None,
            _ => Some(self.binop(inst, BinaryOp::Sub, negated[0], kept[0], Flags::default())),
        }
    }
}

/// `m_ImmConstant` of an integer.
fn is_int_constant(
    c: &Combiner,
    v: Operand,
) -> bool {
    let q: Query = query(c, None);
    pattern::apint(&q, v).is_some()
}

#[cfg(test)]
#[path = "combine_negator_tests.rs"]
mod tests;
