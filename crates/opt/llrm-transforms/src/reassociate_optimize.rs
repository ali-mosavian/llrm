//! Reassociate's optimizers of a linearized expression: the identities of one
//! operation applied to its sorted leaves.
//!
//! Translation of `OptimizeExpression`, `OptimizeAndOrXor`, `OptimizeXor`,
//! `OptimizeAdd`, `OptimizeMul`, `RemoveFactorFromExpression`,
//! `buildMinimalMultiplyDAG` and their helpers
//! ([Reassociate.cpp:1166-2105](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp#L1166))
//! at d1106deb71cc.
//!
//! MIRROR: constants fold to one; `X & X`, `X ^ X`, `X & ~X`, `X + -X` go;
//! repeated addends become a multiply; a factor common to several products is
//! pulled out; a product of four or more repeated factors becomes a minimal
//! multiply DAG; `xor` operands over one value are merged by the four Xor
//! rules.
//!
//! DEVIATE 1: the `xor` rules work on integers up to 128 bits; a wider `xor`
//! is left as `OptimizeAndOrXor` leaves it.
//!
//! DEVIATE 2: `FindSingleUseMultiplyFactors` and `EmitAddTreeOfValues` are
//! loops, not recursion: a product or sum of a function's length would
//! otherwise overflow the stack. The order of the factors and of the adds made
//! is upstream's.
//!
//! DEVIATE 3: a floating constant is the nearest `f32` or `f64`; see
//! `reassociate_fp`.
//!
//! # How it works
//! Each optimizer works on `ops`, sorted by rank, high to low, so equal values
//! and a value and its negation are adjacent. `FindInOperandList` scans only
//! the run of equal rank. The caller recurses while the leaf count falls.
//!
//! # Used by
//! `crate::reassociate` (`ReassociateExpression`).
//!
//! # Example
//! `x + x + y` has leaves `x, x, y` and becomes `x*2 + y`.

use llrm_mir::constantfolding::constant_fold_binary_op_operands;
use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::reassociate::{OperandMap, Reassociator, ValueEntry};
use crate::reassociate_fp::{float_constant, float_value, fold_float};
use crate::reassociate_tree::OverflowTracking;

/// `reassociate::Factor`: a base raised to a power.
#[derive(Clone, Copy)]
struct Factor {
    base: Operand,
    power: u32,
}

/// `XorOpnd`: a non-constant operand of an `xor` seen as `symbolic op const`
/// with `op` an `or` or an `and`.
#[derive(Clone, Copy)]
struct XorOpnd {
    /// The operand as it stands in the tree; none once invalidated.
    value: Option<Operand>,
    symbolic: Operand,
    constant: u128,
    rank: u32,
    is_or: bool,
}

impl XorOpnd {
    fn is_invalid(&self) -> bool {
        self.value.is_none()
    }
}

impl Reassociator<'_> {
    /// `ConstantExpr::getBinOpIdentity(Opcode, Ty)`: `-0.0` for `fadd`.
    pub fn bin_op_identity(
        &self,
        op: BinaryOp,
        ty: llrm_mir::types::TypeId,
    ) -> Operand {
        match op {
            BinaryOp::FAdd => float_constant(self.context, ty, -0.0),
            BinaryOp::FMul => float_constant(self.context, ty, 1.0),
            BinaryOp::Mul => Operand::Constant(self.context.int(ty, 1)),
            BinaryOp::And => Operand::Constant(self.context.int(ty, -1)),
            _ => Operand::Constant(self.context.int(ty, 0)),
        }
    }

    /// `ConstantExpr::getBinOpAbsorber(Opcode, Ty)`.
    fn bin_op_absorber(
        &self,
        op: BinaryOp,
        ty: llrm_mir::types::TypeId,
    ) -> Option<Operand> {
        match op {
            BinaryOp::Mul | BinaryOp::And => Some(Operand::Constant(self.context.int(ty, 0))),
            BinaryOp::Or => Some(Operand::Constant(self.context.int(ty, -1))),
            _ => None,
        }
    }

    fn null_value(
        &self,
        ty: llrm_mir::types::TypeId,
    ) -> Operand {
        self.zero_of(ty)
    }

    fn all_ones(
        &self,
        ty: llrm_mir::types::TypeId,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, -1))
    }

    /// `Instruction::isIdenticalTo` of the instructions computing `a` and `b`.
    fn is_identical_to(
        &self,
        a: Operand,
        b: Operand,
    ) -> bool {
        let (Some(a), Some(b)) = (self.inst_of(a), self.inst_of(b)) else { return false };
        let (a, b) = (self.function.instruction(a), self.function.instruction(b));
        a.opcode == b.opcode && a.ty == b.ty && a.operands == b.operands && a.flags == b.flags
    }

    /// `FindInOperandList`: scan forwards and backwards among the values of
    /// the rank of element `i` for `x`; `i` if it is not there. Useful for `x`
    /// when `-x` is seen, as both get one rank.
    fn find_in_operand_list(
        &self,
        ops: &[ValueEntry],
        i: usize,
        x: Operand,
    ) -> usize {
        let rank = ops[i].rank;
        let same = |j: usize| ops[j].op == x || self.is_identical_to(ops[j].op, x);
        for j in i + 1..ops.len() {
            self.tick();
            if ops[j].rank != rank {
                break;
            }
            if same(j) {
                return j;
            }
        }
        for j in (0..i).rev() {
            if ops[j].rank != rank {
                break;
            }
            if same(j) {
                return j;
            }
        }
        i
    }

    /// `OptimizeExpression`: fold the constants of `ops`, then apply the
    /// identities of the operation. A single value if the whole expression
    /// reduces to one; otherwise `ops` is changed as needed.
    pub fn optimize_expression(
        &mut self,
        root: InstId,
        ops: &mut Vec<ValueEntry>,
    ) -> Option<Operand> {
        let opcode = self.binop(root).expect("a binary operator");
        let ty = self.function.instruction(root).ty;
        loop {
            // Fold any constants found.
            let mut cst: Option<Operand> = None;
            while let Some(&last) = ops.last() {
                let Operand::Constant(c) = last.op else { break };
                match cst {
                    None => {
                        ops.pop();
                        cst = Some(last.op);
                    }
                    Some(accumulated) => {
                        let Operand::Constant(accumulated_id) = accumulated else { break };
                        let folded = if self.is_float(ty) {
                            fold_float(self.context, opcode, last.op, accumulated)
                        } else {
                            constant_fold_binary_op_operands(self.context, self.env, opcode, c, accumulated_id)
                                .map(Operand::Constant)
                        };
                        match folded {
                            Some(res) => {
                                ops.pop();
                                cst = Some(res);
                            }
                            None => break,
                        }
                    }
                }
            }
            // Nothing but constants: done.
            if ops.is_empty() {
                return cst;
            }

            // Put the combined constant back at the end of the list, except
            // where there is no point: an add of 0 is dropped and a multiply by
            // zero is the whole expression.
            if let Some(cst) = cst
                && cst != self.bin_op_identity(opcode, ty)
            {
                if Some(cst) == self.bin_op_absorber(opcode, ty) {
                    return Some(cst);
                }
                ops.push(ValueEntry { rank: 0, op: cst });
            }

            if ops.len() == 1 {
                return Some(ops[0].op);
            }

            // Destructive annihilation due to identities between elements.
            let count = ops.len();
            let result = match opcode {
                BinaryOp::And | BinaryOp::Or => self.optimize_and_or_xor(opcode, ops),
                BinaryOp::Xor => self.optimize_xor(root, ops),
                BinaryOp::Add | BinaryOp::FAdd => self.optimize_add(root, ops),
                BinaryOp::Mul | BinaryOp::FMul => self.optimize_mul(root, ops),
                _ => None,
            };
            if result.is_some() {
                return result;
            }
            if ops.len() == count {
                return None;
            }
        }
    }

    /// `OptimizeAndOrXor`: identities of `and`, `or` and `xor`.
    fn optimize_and_or_xor(
        &mut self,
        opcode: BinaryOp,
        ops: &mut Vec<ValueEntry>,
    ) -> Option<Operand> {
        let mut i: isize = 0;
        let mut e = ops.len() as isize;
        while i != e {
            self.tick();
            let at = i as usize;
            // `X` and `~X`: `X & ~X` is 0, `X | ~X` is -1.
            if let Some(x) = self.match_not(ops[at].op) {
                let found = self.find_in_operand_list(ops, at, x);
                if found != at {
                    let ty = self.type_of(x);
                    match opcode {
                        BinaryOp::And => return Some(self.null_value(ty)),
                        BinaryOp::Or => return Some(self.all_ones(ty)),
                        _ => {}
                    }
                }
            }
            // Duplicate pairs sit next to each other, by the sort.
            if at + 1 != ops.len() && ops[at + 1].op == ops[at].op {
                if matches!(opcode, BinaryOp::And | BinaryOp::Or) {
                    ops.remove(at);
                    e -= 1;
                    continue;
                }
                if e == 2 {
                    return Some(self.null_value(self.type_of(ops[0].op)));
                }
                // `Y ^ X ^ X` is `Y`.
                ops.drain(at..at + 2);
                i -= 1;
                e -= 2;
            }
            i += 1;
        }
        None
    }

    /// `createAndInstr`: `opnd & c`; none for 0, `opnd` for all ones.
    fn create_and(
        &mut self,
        at: InstId,
        opnd: Operand,
        c: u128,
        width: u32,
    ) -> Option<Operand> {
        if c == 0 {
            return None;
        }
        if c == mask(width) {
            return Some(opnd);
        }
        let ty = self.type_of(opnd);
        let constant = Operand::Constant(self.context.int(ty, c as i128));
        let and = self.insert_before(at, Opcode::Binary(BinaryOp::And), ty, vec![opnd, constant], Flags::default());
        Some(self.result(and))
    }

    /// `XorOpnd::XorOpnd`.
    fn xor_opnd(
        &mut self,
        v: Operand,
        width: u32,
    ) -> XorOpnd {
        let mut opnd = XorOpnd { value: Some(v), symbolic: v, constant: 0, rank: 0, is_or: true };
        if let Some(inst) = self.inst_of(v)
            && let Some(kind @ (BinaryOp::Or | BinaryOp::And)) = self.binop(inst)
        {
            let (mut v0, mut v1) = (self.operand(inst, 0), self.operand(inst, 1));
            if self.int_constant(v0).is_some() {
                std::mem::swap(&mut v0, &mut v1);
            }
            if let Some((c, _)) = self.int_constant(v1) {
                opnd.constant = c & mask(width);
                opnd.symbolic = v0;
                opnd.is_or = kind == BinaryOp::Or;
            }
        }
        opnd.rank = self.rank(opnd.symbolic);
        opnd
    }

    /// `CombineXorOpnd(It, Opnd1, ConstOpnd, Res)`: `(x | c) ^ c` is `x & ~c`.
    /// The result of the combination, itself none for a zero, if it applies.
    fn combine_xor_opnd_const(
        &mut self,
        at: InstId,
        opnd: &XorOpnd,
        const_opnd: &mut u128,
        width: u32,
    ) -> Option<Option<Operand>> {
        if !opnd.is_or || opnd.constant == 0 {
            return None;
        }
        let value = opnd.value?;
        if !self.has_one_use(value) || opnd.constant != *const_opnd {
            return None;
        }
        let res = self.create_and(at, opnd.symbolic, !opnd.constant & mask(width), width);
        *const_opnd ^= opnd.constant;
        if let Some(inst) = self.inst_of(value) {
            self.redo.insert(inst);
        }
        Some(res)
    }

    /// `CombineXorOpnd(It, Opnd1, Opnd2, ConstOpnd, Res)`: two operands over
    /// one value, by Xor-Rules 2 to 4.
    fn combine_xor_opnds(
        &mut self,
        at: InstId,
        opnd1: &XorOpnd,
        opnd2: &XorOpnd,
        const_opnd: &mut u128,
        width: u32,
    ) -> Option<Option<Operand>> {
        let x = opnd1.symbolic;
        if x != opnd2.symbolic {
            return None;
        }
        let (v1, v2) = (opnd1.value?, opnd2.value?);
        // At least `Opnd1 ^ Opnd2` dies.
        let mut dead = 1;
        dead += i32::from(self.has_one_use(v1));
        dead += i32::from(self.has_one_use(v2));
        let all = mask(width);
        let res;
        if opnd1.is_or != opnd2.is_or {
            // Xor-Rule 2: `(x | c1) ^ (x & c2)` is `(x & c3) ^ c1`, `c3 = ~c1 ^
            // c2`.
            let (o1, o2) = if opnd2.is_or { (opnd2, opnd1) } else { (opnd1, opnd2) };
            let (c1, c2) = (o1.constant, o2.constant);
            let c3 = (!c1 & all) ^ c2;
            // Do not increase code size.
            if c3 != 0 && c3 != all {
                let new = if *const_opnd != 0 { 1 } else { 2 };
                if new > dead {
                    return None;
                }
            }
            res = self.create_and(at, x, c3, width);
            *const_opnd ^= c1;
        } else if opnd1.is_or {
            // Xor-Rule 3: `(x | c1) ^ (x | c2)` is `(x & c3) ^ c3`, `c3 = c1 ^
            // c2`.
            let c3 = opnd1.constant ^ opnd2.constant;
            if c3 != 0 && c3 != all {
                let new = if *const_opnd != 0 { 1 } else { 2 };
                if new > dead {
                    return None;
                }
            }
            res = self.create_and(at, x, c3, width);
            *const_opnd ^= c3;
        } else {
            // Xor-Rule 4: `(x & c1) ^ (x & c2)` is `x & (c1 ^ c2)`.
            res = self.create_and(at, x, opnd1.constant ^ opnd2.constant, width);
        }
        // Hope the originals are deleted as dead code.
        for v in [v1, v2] {
            if let Some(inst) = self.inst_of(v) {
                self.redo.insert(inst);
            }
        }
        Some(res)
    }

    /// `OptimizeXor`.
    fn optimize_xor(
        &mut self,
        root: InstId,
        ops: &mut Vec<ValueEntry>,
    ) -> Option<Operand> {
        if let Some(v) = self.optimize_and_or_xor(BinaryOp::Xor, ops) {
            return Some(v);
        }
        if ops.len() == 1 {
            return None;
        }
        let ty = self.type_of(ops[0].op);
        let width = self.context.types.int_bits(ty).expect("an integer xor");
        if width > 128 {
            return None;
        }
        let mut const_opnd: u128 = 0;

        // Step 1: ValueEntry to XorOpnd.
        let mut opnds: Vec<XorOpnd> = Vec::new();
        for entry in ops.clone() {
            match self.int_constant(entry.op) {
                Some((c, _)) => const_opnd ^= c,
                None => {
                    let opnd = self.xor_opnd(entry.op, width);
                    opnds.push(opnd);
                }
            }
        }

        // Step 2: sort so that operands over one symbolic value cluster, the
        // lower rank first.
        let mut order: Vec<usize> = (0..opnds.len()).collect();
        order.sort_by_key(|&i| opnds[i].rank);

        // Step 3: combine adjacent operands.
        let mut prev: Option<usize> = None;
        let mut changed = false;
        for &curr in &order {
            // Step 3.1: `curr ^ const`.
            if const_opnd != 0
                && let Some(cv) = self.combine_xor_opnd_const(root, &opnds[curr].clone(), &mut const_opnd, width)
            {
                changed = true;
                match cv {
                    Some(value) => opnds[curr] = self.xor_opnd(value, width),
                    None => {
                        opnds[curr].value = None;
                        continue;
                    }
                }
            }
            let Some(p) = prev.filter(|&p| opnds[p].symbolic == opnds[curr].symbolic) else {
                prev = Some(curr);
                continue;
            };
            // Step 3.2: same symbolic value, `prev ^ curr ^ const`.
            if let Some(cv) =
                self.combine_xor_opnds(root, &opnds[curr].clone(), &opnds[p].clone(), &mut const_opnd, width)
            {
                opnds[p].value = None;
                match cv {
                    Some(value) => {
                        opnds[curr] = self.xor_opnd(value, width);
                        prev = Some(curr);
                    }
                    None => {
                        opnds[curr].value = None;
                        prev = None;
                    }
                }
                changed = true;
            }
        }

        // Step 4: reassemble.
        if changed {
            ops.clear();
            for opnd in &opnds {
                if opnd.is_invalid() {
                    continue;
                }
                let value = opnd.value.expect("valid");
                let entry = self.entry(value);
                ops.push(entry);
            }
            if const_opnd != 0 {
                let c = Operand::Constant(self.context.int(ty, const_opnd as i128));
                let entry = self.entry(c);
                ops.push(entry);
            }
            match ops.len() {
                1 => return Some(ops.last().expect("one").op),
                0 => return Some(Operand::Constant(self.context.int(ty, const_opnd as i128))),
                _ => {}
            }
        }
        None
    }

    /// `FindSingleUseMultiplyFactors`: the factors of a tree of one-use
    /// multiplies, `op1` before `op0` as upstream's recursion visits them.
    fn single_use_multiply_factors(
        &self,
        v: Operand,
    ) -> Vec<Operand> {
        let mut factors = Vec::new();
        let mut stack = vec![v];
        while let Some(v) = stack.pop() {
            match self.reassociable(v, &[BinaryOp::Mul, BinaryOp::FMul]) {
                Some(bo) => {
                    stack.push(self.operand(bo, 0));
                    stack.push(self.operand(bo, 1));
                }
                None => factors.push(v),
            }
        }
        factors
    }

    /// `ConstantInt` value of `v` as a signed number of its width.
    fn signed_int(
        &self,
        v: Operand,
    ) -> Option<(i128, u32)> {
        let (bits, width) = self.int_constant(v)?;
        Some((llrm_mir::context::signed(bits, width), width))
    }

    /// `OptimizeAdd`.
    fn optimize_add(
        &mut self,
        root: InstId,
        ops: &mut Vec<ValueEntry>,
    ) -> Option<Operand> {
        let ty = self.function.instruction(root).ty;
        // Scan for `X` and `-X` pairs and duplicates: `Y+Y+Y+Z` is `3*Y+Z`.
        let mut i: isize = 0;
        let mut e = ops.len() as isize;
        while i != e {
            self.tick();
            let at = i as usize;
            let the_op = ops[at].op;
            // A value seen before: the instances are next to each other.
            if at + 1 != ops.len() && ops[at + 1].op == the_op {
                // Remove every instance of the operand in one move.
                let end = at + ops[at..].iter().take_while(|entry| entry.op == the_op).count();
                let found = (end - at) as u64;
                ops.drain(at..end);
                let the_ty = self.type_of(the_op);
                let count = if self.is_float(the_ty) {
                    float_constant(self.context, the_ty, found as f64)
                } else {
                    // Truncated if it overflows the type.
                    Operand::Constant(self.context.int(the_ty, found as i128))
                };
                let mul = self.create_mul(the_op, count, root, root);
                // Optimize the multiply too, so that `(X*2)*3` becomes `X*6`.
                self.redo.insert(mul);
                let mul_value = self.result(mul);
                if ops.is_empty() {
                    return Some(mul_value);
                }
                let entry = self.entry(mul_value);
                ops.insert(0, entry);
                i -= 1;
                e = ops.len() as isize;
                i += 1;
                continue;
            }

            // `X` and `-X`, or `X` and `~X`.
            let x = if let Some(x) = self.match_neg(the_op) {
                x
            } else if let Some(x) = self.match_not(the_op) {
                x
            } else if let Some(x) = self.match_fneg(the_op) {
                x
            } else {
                i += 1;
                continue;
            };
            let mut found = self.find_in_operand_list(ops, at, x);
            if found == at {
                i += 1;
                continue;
            }
            let is_not = self.match_not(the_op).is_some();
            if ops.len() == 2 && !is_not {
                return Some(self.null_value(self.type_of(x)));
            }
            if ops.len() == 2 && is_not {
                return Some(self.all_ones(self.type_of(x)));
            }
            ops.remove(at);
            if at < found {
                found -= 1;
            } else {
                i -= 1;
            }
            ops.remove(found);
            // Revisit the element; two are gone.
            i -= 1;
            e -= 2;
            // `X + ~X` appends -1.
            if is_not {
                let minus_one = self.all_ones(self.type_of(x));
                let entry = self.entry(minus_one);
                ops.push(entry);
                e += 1;
            }
            i += 1;
        }

        // Count the factors common to the multiplies among the addends:
        // `A*A + A*B*C + D` is `A*(A + B*C) + D`.
        let mut occurrences: OperandMap<u32> = OperandMap::new();
        let mut max_occ = 0u32;
        let mut max_occ_val: Option<Operand> = None;
        // Per product, the factors already counted (a stamp, not a set cleared
        // each time).
        let mut stamps: OperandMap<u32> = OperandMap::new();
        let mut stamp = 0u32;
        let mut fma_candidates: Vec<Operand> = Vec::new();

        let entries: Vec<Operand> = ops.iter().map(|entry| entry.op).collect();
        for entry in entries {
            if let Some(bop) = self.reassociable(entry, &[BinaryOp::Mul, BinaryOp::FMul]) {
                stamp += 1;
                self.count_factors(bop, &mut occurrences, &mut stamps, stamp, &mut max_occ, &mut max_occ_val);
                continue;
            }
            if let Some(bop) = self.is_fmul_add_candidate(entry)
                && self.has_fp_associative_flags(bop)
            {
                fma_candidates.push(entry);
                stamp += 1;
                self.count_factors(bop, &mut occurrences, &mut stamps, stamp, &mut max_occ, &mut max_occ_val);
            }
        }

        // The pairs kept together for an `fma` hide their multiplies: break
        // them up if a factor repeats, so that factoring applies.
        if max_occ > 1 {
            for v in fma_candidates {
                ops.retain(|entry| entry.op != v);
                let fadd = self.inst_of(v).expect("an fadd");
                for index in 0..2 {
                    let operand = self.operand(fadd, index);
                    let entry = self.entry(operand);
                    ops.push(entry);
                }
            }
        }

        if max_occ <= 1 {
            return None;
        }
        let max_occ_val = max_occ_val.expect("a factor");

        // A throwaway use of the factor twice: removing a factor from one
        // expression must not change what `RemoveFactorFromExpression` sees of
        // the next.
        let dummy_opcode = Opcode::Binary(if self.is_float(ty) { BinaryOp::FAdd } else { BinaryOp::Add });
        let dummy_ty = self.type_of(max_occ_val);
        let dummy = self
            .function
            .create_instruction(dummy_opcode, dummy_ty, vec![max_occ_val, max_occ_val], Flags::default(), None);

        let mut new_mul_ops: Vec<Operand> = Vec::new();
        let mut at = 0;
        while at < ops.len() {
            self.tick();
            if self.reassociable(ops[at].op, &[BinaryOp::Mul, BinaryOp::FMul]).is_none() {
                at += 1;
                continue;
            }
            if let Some(v) = self.remove_factor_from_expression(ops[at].op, max_occ_val) {
                // The factorized operand may occur several times: convert them
                // all at once. Element `at` goes too, so it is revisited.
                let this = ops[at].op;
                let mut j = ops.len();
                while j != at {
                    j -= 1;
                    if ops[j].op == this {
                        new_mul_ops.push(v);
                        ops.remove(j);
                    }
                }
            } else {
                at += 1;
            }
        }

        // No need for extra uses anymore.
        self.function.erase(dummy).expect("an unplaced instruction");

        // The add tree of the products, optimized in turn: `A*A*B + A*A*C`.
        let sum = self.emit_add_tree_of_values(root, &new_mul_ops);
        if let Some(inst) = self.inst_of(sum) {
            self.redo.insert(inst);
        }
        let product = self.create_mul(sum, max_occ_val, root, root);
        // Rerun on the multiply in case the inner expression became one.
        self.redo.insert(product);
        let product_value = self.result(product);
        if ops.is_empty() {
            return Some(product_value);
        }
        let entry = self.entry(product_value);
        ops.insert(0, entry);
        None
    }

    /// `CountFactors`: add one to the count of each factor of the product
    /// `bop`, the negation of a negative constant too.
    fn count_factors(
        &mut self,
        bop: InstId,
        occurrences: &mut OperandMap<u32>,
        stamps: &mut OperandMap<u32>,
        stamp: u32,
        max_occ: &mut u32,
        max_occ_val: &mut Option<Operand>,
    ) {
        let product = self.result(bop);
        let factors = self.single_use_multiply_factors(product);
        assert!(factors.len() > 1, "bad linearize");
        for factor in factors {
            if stamps.get(factor) == Some(&stamp) {
                continue;
            }
            stamps.insert(factor, stamp);
            self.count_factor(factor, occurrences, max_occ, max_occ_val);

            // A negative constant: the negated value is a factor too, the
            // negate percolates out. Not the minimum value, which cannot be
            // made positive.
            let negated = if let Some((value, width)) = self.signed_int(factor) {
                let minimum = if width >= 128 { i128::MIN } else { -(1_i128 << (width - 1)) };
                (value < 0 && value != minimum)
                    .then(|| Operand::Constant(self.context.int(self.type_of(factor), -value)))
            } else {
                float_value(self.context, factor)
                    .filter(|x| x.is_sign_negative())
                    .map(|x| float_constant(self.context, self.type_of(factor), -x))
            };
            if let Some(negated) = negated {
                if stamps.get(negated) == Some(&stamp) {
                    continue;
                }
                stamps.insert(negated, stamp);
                self.count_factor(negated, occurrences, max_occ, max_occ_val);
            }
        }
    }

    fn count_factor(
        &self,
        factor: Operand,
        occurrences: &mut OperandMap<u32>,
        max_occ: &mut u32,
        max_occ_val: &mut Option<Operand>,
    ) {
        let occ = {
            let slot = occurrences.get_or_insert(factor, 0);
            *slot += 1;
            *slot
        };
        // On a tie prefer a variable over a constant: factoring out X from
        // `X*C1 + X*C2` exposes folding, factoring out a constant does not.
        let better = occ > *max_occ
            || (occ == *max_occ
                && (self.inst_of(factor).is_some() || self.is_argument(factor))
                && max_occ_val.is_some_and(|m| self.is_constant(m) && !self.is_undef_operand(m)));
        if better {
            *max_occ = occ;
            *max_occ_val = Some(factor);
        }
    }

    /// `EmitAddTreeOfValues`: `((a + b) + c) + ...`, before `at`.
    fn emit_add_tree_of_values(
        &mut self,
        at: InstId,
        values: &[Operand],
    ) -> Operand {
        let mut sum = values[0];
        for &value in &values[1..] {
            let add = self.create_add(sum, value, at, at);
            sum = self.result(add);
        }
        sum
    }

    /// `RemoveFactorFromExpression`: `v`, a multiply tree with `factor` among
    /// its factors, without it; none if `v` has no such factor.
    fn remove_factor_from_expression(
        &mut self,
        v: Operand,
        factor: Operand,
    ) -> Option<Operand> {
        let bo = self.reassociable(v, &[BinaryOp::Mul, BinaryOp::FMul])?;
        let mut flags = OverflowTracking::default();
        let (tree, changed) = self.linearize_expr_tree(bo, &mut flags);
        self.changed |= changed;
        let mut factors: Vec<ValueEntry> = Vec::new();
        for (leaf, weight) in tree {
            let entry = self.entry(leaf);
            for _ in 0..weight {
                factors.push(entry);
            }
        }

        let mut found = false;
        let mut needs_negate = false;
        for i in 0..factors.len() {
            if factors[i].op == factor {
                found = true;
                factors.remove(i);
                break;
            }
            // The negative of this factor.
            if let (Some((f1, _)), Some((f2, _))) = (self.signed_int(factor), self.signed_int(factors[i].op)) {
                if f1 == f2.wrapping_neg() {
                    found = true;
                    needs_negate = true;
                    factors.remove(i);
                    break;
                }
            } else if let (Some(f1), Some(f2)) =
                (float_value(self.context, factor), float_value(self.context, factors[i].op))
                && f1 == -f2
            {
                found = true;
                needs_negate = true;
                factors.remove(i);
                break;
            }
        }

        if !found {
            // Put the operands back in the tree.
            self.rewrite_expr_tree(bo, &factors, flags);
            return None;
        }

        let after = llrm_mir::edit::Position::After(bo);
        let mut result;
        // A single multiply: remove it and return the one operand left.
        if factors.len() == 1 {
            self.redo.insert(bo);
            result = factors[0].op;
        } else {
            self.rewrite_expr_tree(bo, &factors, flags);
            result = self.result(bo);
        }
        if needs_negate {
            let neg = self.create_neg(result, after, bo);
            result = self.result(neg);
        }
        Some(result)
    }

    /// `collectMultiplyFactors`: the values that repeat, with their powers,
    /// removed from `ops` (an even number of each), highest power first. If
    /// the powers sum to four or more.
    fn collect_multiply_factors(
        ops: &mut Vec<ValueEntry>,
        factors: &mut Vec<Factor>,
    ) -> bool {
        // The sum of the powers of the simplifiable factors.
        let mut power_sum = 0u32;
        let size = ops.len();
        let mut idx = 1;
        while idx < size {
            let op = ops[idx - 1].op;
            let mut count = 1;
            while idx < size && ops[idx].op == op {
                count += 1;
                idx += 1;
            }
            if count > 1 {
                power_sum += count;
            }
            idx += 1;
        }
        // Simplify only if the sum is four or more: then there is always a
        // simplification, which keeps an already minimal form from being tried
        // again and again.
        if power_sum < 4 {
            return false;
        }

        let mut idx = 1;
        while idx < ops.len() {
            let op = ops[idx - 1].op;
            let mut count = 1u32;
            while idx < ops.len() && ops[idx].op == op {
                count += 1;
                idx += 1;
            }
            if count == 1 {
                idx += 1;
                continue;
            }
            // An even number of occurrences move to the factors.
            count &= !1;
            idx -= count as usize;
            factors.push(Factor { base: op, power: count });
            ops.drain(idx..idx + count as usize);
            idx += 1;
        }
        factors.sort_by(|a, b| b.power.cmp(&a.power));
        true
    }

    /// `buildMultiplyTree`: the product of `values`, before `at`.
    fn build_multiply_tree(
        &mut self,
        at: InstId,
        values: &mut Vec<Operand>,
    ) -> Operand {
        if values.len() == 1 {
            return values[0];
        }
        let mut lhs = values.pop().expect("a value");
        while let Some(rhs) = values.pop() {
            let mul = self.create_mul(lhs, rhs, at, at);
            lhs = self.result(mul);
        }
        lhs
    }

    /// `buildMinimalMultiplyDAG`: `(a^x)*(b^y)*...` with the powers sorted
    /// descending, in the fewest multiplies.
    fn build_minimal_multiply_dag(
        &mut self,
        at: InstId,
        factors: &mut Vec<Factor>,
    ) -> Operand {
        let mut outer: Vec<Operand> = Vec::new();
        let (mut last, mut idx) = (0, 1);
        while idx < factors.len() && factors[idx].power > 0 {
            if factors[idx].power != factors[last].power {
                last = idx;
                idx += 1;
                continue;
            }
            // Multiply across the factors of one power so that they are raised
            // as a single entity.
            let mut inner = vec![factors[last].base];
            loop {
                inner.push(factors[idx].base);
                idx += 1;
                if idx >= factors.len() || factors[idx].power != factors[last].power {
                    break;
                }
            }
            let product = self.build_multiply_tree(at, &mut inner);
            factors[last].base = product;
            if let Some(inst) = self.inst_of(product) {
                self.redo.insert(inst);
            }
            last = idx;
        }
        // Factors of equal power are folded into the first.
        factors.dedup_by(|b, a| a.power == b.power);

        // Collect the base of each odd power into the outer product and halve
        // each power, to square the expression.
        for factor in factors.iter_mut() {
            if factor.power & 1 == 1 {
                outer.push(factor.base);
            }
            factor.power >>= 1;
        }
        if factors[0].power != 0 {
            let square_root = self.build_minimal_multiply_dag(at, factors);
            outer.push(square_root);
            outer.push(square_root);
        }
        if outer.len() == 1 {
            return outer[0];
        }
        self.build_multiply_tree(at, &mut outer)
    }

    /// `OptimizeMul`: a product of four or more leaves with repeats becomes a
    /// minimal multiply DAG.
    fn optimize_mul(
        &mut self,
        root: InstId,
        ops: &mut Vec<ValueEntry>,
    ) -> Option<Operand> {
        // Only a chain of more than three, where a balanced tree might need
        // fewer multiplies.
        if ops.len() < 4 {
            return None;
        }
        let mut factors: Vec<Factor> = Vec::new();
        if !Self::collect_multiply_factors(ops, &mut factors) {
            return None;
        }
        let dag = self.build_minimal_multiply_dag(root, &mut factors);
        if ops.is_empty() {
            return Some(dag);
        }
        let entry = self.entry(dag);
        // `lower_bound` in the descending order.
        let at = ops.partition_point(|e| e.rank > entry.rank);
        ops.insert(at, entry);
        None
    }
}
