//! `SCEVExpander`'s cost queries (`ScalarEvolutionExpander.cpp`): whether
//! writing a SCEV out costs more than a budget of instructions.
//!
//! # How it works
//! `is_high_cost_expansion` walks the expression with a worklist, as the
//! header's inline body does. Each node costs what the instructions its
//! expansion makes cost ([`ExpansionTti`]), counted once however often it is
//! reached, and costs nothing where an equal value is already at the insertion
//! point (`has_related_existing_expansion`: a compare operand of a loop exit,
//! or a value `ExprValueMap` names that dominates it).
//!
//! # Example
//! `(%a + %b + 1)` is two adds; with a budget of one it is high cost, unless
//! an instruction that computes it is already before the insertion point.
//!
//! # Used by
//! `rewriteLoopExitValues` (`exitvalues`), and IndVarSimplify's exit-count
//! rewrite.
//!
//! # See also
//! `ScalarEvolutionExpander.h`.

use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::Type as MirType;
use llrm_support::hash::HashSet;

use super::ap::Ap;
use super::expander::Expander;
use super::expr::{Kind, Scev, Ty, Use};
use super::{Cx, Flags, ScalarEvolution};

/// The instruction kinds the expander's cost model names (`Instruction::`
/// opcodes), and the none of the root of a walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opc {
    None,
    Add,
    Sub,
    Mul,
    Shl,
    LShr,
    UDiv,
    Or,
    ICmp,
    Select,
    Trunc,
    ZExt,
    SExt,
    PtrToAddr,
    Phi,
}

/// The `TargetTransformInfo` queries the expansion cost asks, named as
/// `docs/architecture/tti-plan.md` names them. The costs are in
/// `TCC_Basic` units (`TCC_Basic` is 1).
pub trait ExpansionTti {
    /// `hasMinSize` of the function: the cost kind is code size, not
    /// reciprocal throughput.
    fn code_size(&self) -> bool;
    /// `getArithmeticInstrCost(Opcode, Ty, CostKind)`.
    fn get_arithmetic_instr_cost(
        &self,
        opcode: Opc,
        ty: Ty,
    ) -> i64;
    /// `getCastInstrCost(Opcode, Dst, Src, CastContextHint::None, CostKind)`.
    fn get_cast_instr_cost(
        &self,
        opcode: Opc,
        to: Ty,
        from: Ty,
    ) -> i64;
    /// `getCmpSelInstrCost(Opcode, OpTy, ResultTy, BAD_ICMP_PREDICATE,
    /// CostKind)`.
    fn get_cmp_sel_instr_cost(
        &self,
        opcode: Opc,
        ty: Ty,
    ) -> i64;
    /// `getCFInstrCost(Instruction::PHI, CostKind)`.
    fn get_cf_instr_cost_phi(&self) -> i64;
    /// `getIntImmCostInst(Opcode, Idx, Imm, Ty, CostKind)`.
    fn get_int_imm_cost_inst(
        &self,
        opcode: Opc,
        index: i32,
        imm: &Ap,
        ty: Ty,
    ) -> i64;
}

/// One entry of `isHighCostExpansion`'s worklist: `SCEVOperand`.
struct Operand_ {
    parent_opcode: Opc,
    operand_index: i32,
    s: Scev,
}

/// An instruction the expansion makes, and the operands it reads:
/// `OperationIndices`.
struct Operation {
    opcode: Opc,
    min_index: usize,
    max_index: usize,
}

impl Expander {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L258
    /// `isHighCostExpansion(Exprs, L, Budget, TTI, At)`: `l` is the header of
    /// the loop.
    #[allow(clippy::too_many_arguments)]
    pub fn is_high_cost_expansion(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        exprs: &[Scev],
        l: BlockId,
        budget: u32,
        tti: &dyn ExpansionTti,
        at: InstId,
    ) -> bool {
        let mut worklist: Vec<Operand_> =
            exprs.iter().map(|&s| Operand_ { parent_opcode: Opc::None, operand_index: -1, s }).collect();
        let mut processed: HashSet<Scev> = HashSet::default();
        let mut cost = 0i64;
        // `TCC_Basic` is 1.
        let scaled_budget = i64::from(budget);
        while let Some(item) = worklist.pop() {
            if self.is_high_cost_expansion_helper(
                cx,
                se,
                &item,
                l,
                at,
                &mut cost,
                scaled_budget,
                tti,
                &mut processed,
                &mut worklist,
            ) {
                return true;
            }
        }
        debug_assert!(cost <= scaled_budget, "Should have returned from inner loop.");
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2187
    /// `isHighCostExpansionHelper`.
    #[allow(clippy::too_many_arguments)]
    fn is_high_cost_expansion_helper(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        item: &Operand_,
        l: BlockId,
        at: InstId,
        cost: &mut i64,
        budget: i64,
        tti: &dyn ExpansionTti,
        processed: &mut HashSet<Scev>,
        worklist: &mut Vec<Operand_>,
    ) -> bool {
        if *cost > budget {
            return true; // Already run out of budget, give up.
        }
        let s = item.s;
        // Was the cost of expansion of this expression already accounted for?
        let is_constant = matches!(se.node(s).kind, Kind::Constant(_));
        if !is_constant && !processed.insert(s) {
            return false; // We have already accounted for this expression.
        }
        // If we can find an existing value for this scev available at the
        // point "At" then consider the expression cheap.
        if self.has_related_existing_expansion(cx, se, s, at, l) {
            return false; // Consider the expression to be free.
        }
        let code_size = tti.code_size();
        match se.node(s).kind.clone() {
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
            // Assume to be zero-cost.
            Kind::Unknown(_) => false,
            Kind::Constant(imm) => {
                // Only evalulate the costs of constants when optimizing for
                // size.
                if !code_size {
                    return false;
                }
                let ty = se.node(s).ty.expect("a constant has a type");
                *cost += tti.get_int_imm_cost_inst(item.parent_opcode, item.operand_index, &imm, ty);
                *cost > budget
            }
            Kind::Truncate(_) | Kind::PtrToAddr(_) | Kind::ZeroExtend(_) | Kind::SignExtend(_) => {
                *cost += cost_and_collect_operands(se, s, tti, worklist);
                false // Will answer upon next entry into this function.
            }
            Kind::UDiv(..) => {
                // UDivExpr is very likely a UDiv that ScalarEvolution's
                // HowFarToZero or HowManyLessThans produced to compute a
                // precise expression, rather than a UDiv from the user's code.
                // If we can't find a UDiv in the code with some simple
                // searching, we need to account for it's cost.
                //
                // At the beginning of this function we already tried to find
                // existing value for plain 'S'. Now try to lookup 'S + 1'
                // since it is common pattern involving division. This is just
                // a simple search heuristic.
                let ty = se.node(s).ty.expect("a udiv has a type");
                let one = se.get_constant_of_type(cx, ty, 1);
                let plus_one = se.get_add_expr(cx, vec![Use::of(s), Use::of(one)], Flags::default(), 0).scev;
                if self.has_related_existing_expansion(cx, se, plus_one, at, l) {
                    return false; // Consider it to be free.
                }
                *cost += cost_and_collect_operands(se, s, tti, worklist);
                false // Will answer upon next entry into this function.
            }
            Kind::Add(_)
            | Kind::Mul(_)
            | Kind::UMax(_)
            | Kind::SMax(_)
            | Kind::UMin(_)
            | Kind::SMin(_)
            | Kind::SequentialUMin(_) => {
                debug_assert!(se.operands_of(s).len() > 1, "Nary expr should have more than 1 operand.");
                // The simple nary expr will require one less op (or pair of
                // ops) than the number of it's terms.
                *cost += cost_and_collect_operands(se, s, tti, worklist);
                *cost > budget
            }
            Kind::AddRec(..) => {
                debug_assert!(se.operands_of(s).len() >= 2, "Polynomial should be at least linear");
                *cost += cost_and_collect_operands(se, s, tti, worklist);
                *cost > budget
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2005
    /// `hasRelatedExistingExpansion(S, At, L)`: `l` is the header of the
    /// loop.
    pub fn has_related_existing_expansion(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        s: Scev,
        at: InstId,
        l: BlockId,
    ) -> bool {
        // Look for suitable value in simple conditions at the loop exits.
        let exiting = cx.loops.get_loop_for(l).map(|found| found.get_exiting_blocks(cx.function)).unwrap_or_default();
        for block in exiting {
            let Some(terminator) = cx.function.terminator(block) else { continue };
            let branch = cx.function.instruction(terminator);
            let [Operand::Value(condition), Operand::Block(_), Operand::Block(_)] = branch.operands[..] else {
                continue;
            };
            if branch.opcode != Opcode::Br {
                continue;
            }
            let ValueDef::Instruction(compare) = cx.function.value(condition).def else { continue };
            let compare = cx.function.instruction(compare);
            let (Opcode::ICmp(_), [left, right]) = (&compare.opcode, &compare.operands[..]) else { continue };
            for side in [left, right] {
                // `m_Instruction`: an operand that is an instruction.
                let Operand::Value(value) = *side else { continue };
                let ValueDef::Instruction(def) = cx.function.value(value).def else { continue };
                if se.get_scev(cx, *side) == s && cx.tree.instruction_dominates(cx.function, def, at) {
                    return true;
                }
            }
        }
        // Use expand's logic which is used for reusing a previous Value in
        // ExprValueMap. Note that we don't currently model the cost of
        // needing to drop poison generating flags on the instruction if we
        // want to reuse it. We effectively assume that has zero cost.
        self.find_value_in_expr_value_map(cx, se, s, at).is_some()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1637
    /// `FindValueInExprValueMap(S, InsertPt, DropPoisonGeneratingInsts)`,
    /// without the poison check (DEVIATE D12: `canReuseInstruction` is not
    /// translated; its only effect here is to answer `false` for a value
    /// whose poison-generating flags cannot be dropped, which costs the
    /// caller a reuse it would otherwise price at zero).
    pub fn find_value_in_expr_value_map(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        s: Scev,
        insert_point: InstId,
    ) -> Option<Operand> {
        // If S is a constant or unknown, it may be worse to reuse an existing
        // Value.
        if matches!(se.node(s).kind, Kind::Constant(_) | Kind::Unknown(_)) {
            return None;
        }
        let ty = se.node(s).ty?;
        let insert_block = cx.function.parent(insert_point)?;
        for v in se.get_scev_values(s) {
            let Operand::Value(value) = v else { continue };
            let ValueDef::Instruction(def) = cx.function.value(value).def else { continue };
            // Choose a Value from the set which dominates the InsertPt.
            // InsertPt should be inside the Value's parent loop so as not to
            // break the LCSSA form.
            let same_type = match (cx.context.types.get(cx.function.value(value).ty), ty) {
                (MirType::Int(bits), Ty::Int(want)) => *bits == want,
                (MirType::Pointer(space), Ty::Ptr(want)) => *space == want,
                _ => false,
            };
            if !same_type || !cx.tree.instruction_dominates(cx.function, def, insert_point) {
                continue;
            }
            let def_block = cx.function.parent(def)?;
            let inside = match cx.loops.get_loop_for(def_block) {
                None => true,
                Some(found) => found.contains(insert_block),
            };
            if inside {
                return Some(v);
            }
        }
        None
    }
}

/// `costAndCollectOperands<T>`: the cost of the instructions `s` expands to,
/// and its operands queued with the instruction that reads each.
fn cost_and_collect_operands(
    se: &ScalarEvolution,
    s: Scev,
    tti: &dyn ExpansionTti,
    worklist: &mut Vec<Operand_>,
) -> i64 {
    let ty = se.node(s).ty.expect("a typed expression");
    let operands: Vec<Scev> = se.operands_of(s).iter().map(|op| op.scev).collect();
    let count = operands.len();
    let mut cost = 0i64;
    // Collect the operations of all the instructions that will be needed to
    // expand the SCEVExpr. This is so that when we come to cost the operands,
    // we know what the generated user(s) will be.
    let mut operations: Vec<Operation> = Vec::new();
    let cast = |opcode: Opc, operations: &mut Vec<Operation>| {
        operations.push(Operation { opcode, min_index: 0, max_index: 0 });
        let from = se.node(operands[0]).ty.expect("a typed operand");
        tti.get_cast_instr_cost(opcode, ty, from)
    };
    let arith = |opcode: Opc, required: usize, min: usize, max: usize, operations: &mut Vec<Operation>| {
        operations.push(Operation { opcode, min_index: min, max_index: max });
        required as i64 * tti.get_arithmetic_instr_cost(opcode, ty)
    };
    let cmp_sel = |opcode: Opc, required: usize, min: usize, max: usize, operations: &mut Vec<Operation>| {
        operations.push(Operation { opcode, min_index: min, max_index: max });
        required as i64 * tti.get_cmp_sel_instr_cost(opcode, ty)
    };
    match &se.node(s).kind {
        Kind::Unknown(_) | Kind::Constant(_) => return 0,
        Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        Kind::PtrToAddr(_) => cost = cast(Opc::PtrToAddr, &mut operations),
        Kind::Truncate(_) => cost = cast(Opc::Trunc, &mut operations),
        Kind::ZeroExtend(_) => cost = cast(Opc::ZExt, &mut operations),
        Kind::SignExtend(_) => cost = cast(Opc::SExt, &mut operations),
        Kind::UDiv(..) => {
            let mut opcode = Opc::UDiv;
            if let Kind::Constant(divisor) = &se.node(operands[1]).kind {
                if divisor.is_power_of_2() {
                    opcode = Opc::LShr;
                }
            }
            cost = arith(opcode, 1, 0, 1, &mut operations);
        }
        Kind::Add(_) => cost = arith(Opc::Add, count - 1, 0, 1, &mut operations),
        Kind::Mul(_) => {
            // Match the actual expansion in visitMulExpr: multiply by -1 is
            // expanded as a negate (sub 0, x), and multiply by a power of 2 is
            // expanded as a shift. Only handle the common two-operand case
            // with a constant LHS; for everything else fall back to the
            // pessimistic all-multiplies estimate.
            let mut opcode = Opc::Mul;
            if count == 2 {
                if let Kind::Constant(c) = &se.node(operands[0]).kind {
                    if c.is_all_ones() {
                        opcode = Opc::Sub;
                    } else if c.is_power_of_2() {
                        opcode = Opc::Shl;
                    }
                }
            }
            cost = arith(opcode, count - 1, 0, 1, &mut operations);
        }
        Kind::SMax(_) | Kind::UMax(_) | Kind::SMin(_) | Kind::UMin(_) | Kind::SequentialUMin(_) => {
            // The reduction tree.
            cost += cmp_sel(Opc::ICmp, count - 1, 0, 1, &mut operations);
            cost += cmp_sel(Opc::Select, count - 1, 0, 2, &mut operations);
            if matches!(se.node(s).kind, Kind::SequentialUMin(_)) {
                // The safety net against poison.
                cost += cmp_sel(Opc::ICmp, count - 1, 0, 0, &mut operations);
                cost += arith(Opc::Or, if count > 2 { count - 2 } else { 0 }, 0, 1, &mut operations);
                cost += cmp_sel(Opc::Select, 1, 0, 1, &mut operations);
            }
        }
        Kind::AddRec(..) => {
            // Addrec expands to a phi and add per recurrence.
            let recurrences = count as i64 - 1;
            cost += tti.get_cf_instr_cost_phi() * recurrences;
            cost += tti.get_arithmetic_instr_cost(Opc::Add, ty) * recurrences;
            // AR start is used in phi.
            worklist.push(Operand_ { parent_opcode: Opc::Phi, operand_index: 0, s: operands[0] });
            // Other operands are used in add.
            for &op in &operands[1..] {
                worklist.push(Operand_ { parent_opcode: Opc::Add, operand_index: 1, s: op });
            }
        }
    }
    for operation in &operations {
        for (index, &op) in operands.iter().enumerate() {
            // Clamp the index to account for multiple IR operations being
            // chained.
            let min = index.max(operation.min_index);
            let at = min.min(operation.max_index);
            worklist.push(Operand_ { parent_opcode: operation.opcode, operand_index: at as i32, s: op });
        }
    }
    cost
}

/// The costs of [`ExpansionTti`] before the X86 target's lands (#2213): every
/// instruction `TCC_Basic`, as `TargetTransformInfoImplBase` prices a scalar
/// integer instruction (DEVIATE D13: X86's `getArithmeticInstrCost` prices a
/// scalar divide above that).
pub struct UnitCosts {
    pub minimum_size: bool,
}

impl ExpansionTti for UnitCosts {
    fn code_size(&self) -> bool {
        self.minimum_size
    }

    fn get_arithmetic_instr_cost(
        &self,
        _opcode: Opc,
        _ty: Ty,
    ) -> i64 {
        1
    }

    fn get_cast_instr_cost(
        &self,
        _opcode: Opc,
        _to: Ty,
        _from: Ty,
    ) -> i64 {
        1
    }

    fn get_cmp_sel_instr_cost(
        &self,
        _opcode: Opc,
        _ty: Ty,
    ) -> i64 {
        1
    }

    fn get_cf_instr_cost_phi(&self) -> i64 {
        0
    }

    fn get_int_imm_cost_inst(
        &self,
        _opcode: Opc,
        _index: i32,
        _imm: &Ap,
        _ty: Ty,
    ) -> i64 {
        1
    }
}

/// The target's cost model (`Machine::tti`) as the expansion cost asks it:
/// `getArithmeticInstrCost` and the rest, in the cost kind of the function
/// (`hasMinSize`: code size, else reciprocal throughput). A target that
/// states no cost model prices every instruction `TCC_Basic` ([`UnitCosts`]).
pub struct MachineCosts {
    pub tti: Option<std::rc::Rc<dyn llrm_mir::tti::Tti>>,
    pub minimum_size: bool,
}

impl MachineCosts {
    fn kind(&self) -> llrm_mir::tti::CostKind {
        if self.minimum_size { llrm_mir::tti::CostKind::CodeSize } else { llrm_mir::tti::CostKind::RecipThroughput }
    }

    fn ty(ty: Ty) -> llrm_mir::tti::Ty {
        match ty {
            Ty::Int(bits) => llrm_mir::tti::Ty::Int(bits),
            Ty::Ptr(_) => llrm_mir::tti::Ty::Pointer,
        }
    }
}

impl ExpansionTti for MachineCosts {
    fn code_size(&self) -> bool {
        self.minimum_size
    }

    fn get_arithmetic_instr_cost(
        &self,
        opcode: Opc,
        ty: Ty,
    ) -> i64 {
        use llrm_mir::tti::{ArithOp, OperandInfo};
        let Some(tti) = &self.tti else { return 1 };
        let op = match opcode {
            Opc::Add => ArithOp::Add,
            Opc::Sub => ArithOp::Sub,
            Opc::Mul => ArithOp::Mul,
            Opc::Shl => ArithOp::Shl,
            Opc::LShr => ArithOp::LShr,
            Opc::UDiv => ArithOp::UDiv,
            Opc::Or => ArithOp::Or,
            _ => return 1,
        };
        tti.arithmetic_cost(op, Self::ty(ty), self.kind(), OperandInfo::ANY, OperandInfo::ANY).value
    }

    fn get_cast_instr_cost(
        &self,
        opcode: Opc,
        to: Ty,
        from: Ty,
    ) -> i64 {
        use llrm_mir::tti::CastKind;
        let Some(tti) = &self.tti else { return 1 };
        let op = match opcode {
            Opc::Trunc => CastKind::Trunc,
            Opc::ZExt => CastKind::ZExt,
            Opc::SExt => CastKind::SExt,
            Opc::PtrToAddr => CastKind::PtrToInt,
            _ => return 1,
        };
        tti.cast_cost(op, Self::ty(to), Self::ty(from), self.kind()).value
    }

    fn get_cmp_sel_instr_cost(
        &self,
        opcode: Opc,
        ty: Ty,
    ) -> i64 {
        use llrm_mir::tti::CmpSel;
        let Some(tti) = &self.tti else { return 1 };
        let op = match opcode {
            Opc::ICmp => CmpSel::ICmp,
            Opc::Select => CmpSel::Select,
            _ => return 1,
        };
        tti.cmp_sel_cost(op, Self::ty(ty), self.kind()).value
    }

    fn get_cf_instr_cost_phi(&self) -> i64 {
        match &self.tti {
            Some(tti) => tti.cf_instr_cost(true, self.kind()).value,
            None => 0,
        }
    }

    fn get_int_imm_cost_inst(
        &self,
        opcode: Opc,
        index: i32,
        imm: &Ap,
        _ty: Ty,
    ) -> i64 {
        use llrm_mir::tti::ImmUser;
        let Some(tti) = &self.tti else { return 1 };
        let user = match opcode {
            Opc::Add => ImmUser::Add,
            Opc::Sub => ImmUser::Sub,
            Opc::Mul => ImmUser::Mul,
            Opc::Or => ImmUser::Or,
            Opc::UDiv => ImmUser::Divide,
            Opc::Shl | Opc::LShr => ImmUser::Shift,
            Opc::ICmp => ImmUser::ICmp,
            Opc::None => ImmUser::Unknown,
            _ => ImmUser::Other,
        };
        tti.int_imm_cost_inst(user, index.max(0) as u32, imm.sext_value(), imm.width).value
    }
}
