//! Reassociate's rewrites into forms that join a tree: a shift by a constant
//! as a multiply, an `or` of disjoint bits as an add, a product of a sum as a
//! sum of products.
//!
//! Translation of `ConvertShiftToMul`, `shouldConvertOrWithNoCommonBitsToAdd`,
//! `isLoadCombineCandidate`, `convertOrWithNoCommonBitsToAdd`,
//! `ShouldBreakUpDistribution` and `BreakUpDistribute`
//! ([Reassociate.cpp:927-1161](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L927))
//! at d1106deb71cc.
//!
//! MIRROR: each applies only where it joins a tree of the operation it turns
//! into; an `or` that looks like a load widening is left alone; `(X+Y)*C` is
//! distributed only where a sibling `X*C'` or `Y*C'` will factor again.
//!
//! # How it works
//! Each is a local rewrite of one instruction and its immediate users and
//! operands; the walk in `isLoadCombineCandidate` visits each instruction once.
//!
//! # Used by
//! `crate::reassociate_inst` (`OptimizeInst`).
//!
//! # Example
//! `(x << 3) * y` becomes `(x * 8) * y`; `x | y` with no common bits, added
//! to `z`, becomes `x + y + z`.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::known::have_no_common_bits_set;
use llrm_mir::valuetracking::pattern;

use crate::reassociate::Reassociator;

impl Reassociator<'_> {
    /// `ConvertShiftToMul`: `shl x, c` is `mul x, 1 << c`.
    pub fn convert_shift_to_mul(
        &mut self,
        shl: InstId,
    ) -> InstId {
        let ty = self.function.instruction(shl).ty;
        let (amount, width) = self.int_constant(self.operand(shl, 1)).expect("a constant shift amount");
        let one = self.context.int(ty, 1);
        let Operand::Constant(amount_constant) = self.operand(shl, 1) else { unreachable!("a constant") };
        let multiplier = llrm_mir::constfold::binary(self.context, BinaryOp::Shl, one, amount_constant)
            .map_or_else(|| self.poison_of(ty), Operand::Constant);
        let mul = self.create_mul(self.operand(shl, 0), multiplier, shl, shl);
        let poison = self.poison_of(ty);
        self.function.set_operand(shl, 0, poison);
        let (from, to) = (
            self.function.instruction(shl).result.expect("a value"),
            self.function.instruction(mul).result.expect("a value"),
        );
        self.function.take_name(from, to);
        self.replace_all_uses(shl, Operand::Value(to));

        // `nuw` holds in all cases, and so does `nuw nsw`. `nsw` alone holds
        // unless the shift is by the width minus one.
        let flags = self.flags(shl);
        let (nsw, nuw) = (flags.contains(Flags::NSW), flags.contains(Flags::NUW));
        let mut kept = Flags::default();
        if nsw && (nuw || amount < u128::from(width - 1)) {
            kept.insert(Flags::NSW);
        }
        if nuw {
            kept.insert(Flags::NUW);
        }
        self.function.set_flags(mul, kept);
        mul
    }

    /// `shouldConvertOrWithNoCommonBitsToAdd`: an operand, or the one user, is
    /// an add, subtract, multiply or shift tree node. A compile-time saving,
    /// not needed for correctness.
    fn should_convert_or_to_add(
        &self,
        or: InstId,
    ) -> bool {
        let interesting = |v: Operand| {
            [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Shl]
                .iter()
                .any(|&op| self.reassociable(v, &[op]).is_some())
        };
        let value = self.result(or);
        (0..2).any(|index| interesting(self.operand(or, index))) || self.only_user_value(value).is_some_and(interesting)
    }

    /// `isLoadCombineCandidate`: `or` of `shl`, `zext` and `load` nodes only.
    /// It looks like a load widening, not that it is one.
    fn is_load_combine_candidate(
        &self,
        or: InstId,
    ) -> bool {
        let mut visited = IdSet::<InstId>::new();
        let mut worklist: Vec<InstId> = vec![or];
        visited.insert(or);
        while let Some(inst) = worklist.pop() {
            let operands: &[usize] = match self.function.instruction(inst).opcode {
                Opcode::Binary(BinaryOp::Or) => &[0, 1],
                Opcode::Binary(BinaryOp::Shl) | Opcode::Cast(CastOp::ZExt) => &[0],
                // The edge of the graph.
                Opcode::Load { .. } => continue,
                _ => return false,
            };
            for &index in operands {
                // Each node must be an instruction.
                let Some(node) = self.inst_of(self.operand(inst, index)) else { return false };
                if visited.insert(node) {
                    worklist.push(node);
                }
            }
        }
        true
    }

    /// Whether `or` should become `add`: it joins a tree, is not a load
    /// widening, and its operands share no bit.
    pub fn should_convert_or_with_no_common_bits_to_add(
        &self,
        or: InstId,
    ) -> bool {
        self.should_convert_or_to_add(or)
            && !self.is_load_combine_candidate(or)
            && (self.flags(or).contains(Flags::DISJOINT) || {
                let query = Query { cxt: Some(or), ..Query::bare(self.context, self.function, self.env.layout) };
                have_no_common_bits_set(&query, self.operand(or, 0), self.operand(or, 1))
            })
    }

    /// `convertOrWithNoCommonBitsToAdd`: `x | y` is `add nuw nsw x, y`.
    pub fn convert_or_with_no_common_bits_to_add(
        &mut self,
        or: InstId,
    ) -> InstId {
        let new = self.create_add(self.operand(or, 0), self.operand(or, 1), or, or);
        let mut flags = self.flags(new);
        flags.insert(Flags::NSW);
        flags.insert(Flags::NUW);
        self.function.set_flags(new, flags);
        let (from, to) = (
            self.function.instruction(or).result.expect("a value"),
            self.function.instruction(new).result.expect("a value"),
        );
        self.function.take_name(from, to);
        self.replace_all_uses(or, Operand::Value(to));
        new
    }

    /// `ShouldBreakUpDistribution`: `mul` is `(X+Y)*C` or `(X-Y)*C`, one-use
    /// all the way, used by an add or subtract with a sibling `X*C'` or
    /// `Y*C'`: distributing, then factoring, saves an instruction.
    pub fn should_break_up_distribution(
        &self,
        mul: InstId,
    ) -> bool {
        let value = self.result(mul);
        if !self.has_one_use(value) {
            return false;
        }
        let sum = self.operand(mul, 0);
        let Some(add_sub) =
            self.reassociable(sum, &[BinaryOp::Add]).or_else(|| self.reassociable(sum, &[BinaryOp::Sub]))
        else {
            return false;
        };
        let query = self.query();
        if !pattern::imm_constant(&query, self.operand(mul, 1)) {
            return false;
        }
        let (a, b) = (self.operand(add_sub, 0), self.operand(add_sub, 1));
        // The parent must be an add or a subtract, so the tree is flattened.
        let Some(user) = self.only_user(value) else { return false };
        if !matches!(self.binop(user), Some(BinaryOp::Add | BinaryOp::Sub)) {
            return false;
        }
        (0..2)
            .map(|index| self.operand(user, index))
            .any(
                |sibling| {
                    if sibling == value || !self.has_one_use(sibling) {
                        return false;
                    }
                    // The sibling must be `NonConst * C'`.
                    let Some(sib) = self.inst_of(sibling).filter(|&s| self.binop(s) == Some(BinaryOp::Mul)) else {
                        return false;
                    };
                    let non_constant = self.operand(sib, 0);
                    pattern::imm_constant(&query, self.operand(sib, 1))
                        && (non_constant == a || non_constant == b)
                        && !self.is_constant(non_constant)
                },
            )
    }

    /// `BreakUpDistribute`: `(X+Y)*C` is `X*C + Y*C`, and `(X-Y)*C` is
    /// `X*C + Y*-C`, so no negate is made.
    pub fn break_up_distribute(
        &mut self,
        mul: InstId,
    ) -> InstId {
        let add_sub = self.inst_of(self.operand(mul, 0)).expect("an add or subtract");
        let c = self.operand(mul, 1);
        let c2 = if self.binop(add_sub) == Some(BinaryOp::Sub) {
            let ty = self.type_of(c);
            let (bits, _) = self.int_constant(c).expect("an integer constant");
            Operand::Constant(self.context.int(ty, (bits as i128).wrapping_neg()))
        } else {
            c
        };
        let m1 = self.create_mul(self.operand(add_sub, 0), c, mul, mul);
        let m2 = self.create_mul(self.operand(add_sub, 1), c2, mul, mul);
        let (r1, r2) = (self.result(m1), self.result(m2));
        let result = self.create_add(r1, r2, mul, mul);
        let value = self.result(result);
        self.replace_all_uses(mul, value);
        for inst in [m1, m2, result] {
            self.redo.insert(inst);
        }
        result
    }
}
