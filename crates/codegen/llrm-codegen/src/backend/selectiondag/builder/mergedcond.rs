//! `visitCondBr`'s merged conditions (DEVIATE SD15 is its registry row): a
//! branch on `a || b` or `a && b` of compares is a chain of branches, not a
//! `setcc` of each, an `or` and one branch. `FindMergedConditions` walks the
//! tree of logical operations and makes a `CaseBlock` for each leaf; the first
//! goes in the current block, the others in blocks the walk placed after it,
//! built once the layout is final (`FinishBasicBlock`, as the switch lowering's
//! are).
//!
//! Whether to split is `shouldKeepJumpConditionsTogether`: the cost of what the
//! right operand needs beyond the left's, by the target's latencies, against
//! X86's threshold (`getJumpConditionMergingParams`).
//!
//! Edge probabilities are not kept by the DAG builder (nor by `visit_br`
//! before this); the pin's `TProb`/`FProb` bookkeeping is not translated (the
//! row's exit: probabilities kept by the builder). Owner sdag-a (w9).

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, FloatPredicate, Opcode};

use super::super::isd::{CondCode, ISD};
use super::super::mvt::EVT;
use super::{Builder, Refused};
use crate::backend::selectiondag::switchcg::BranchProbability;

/// A logical `and` or `or`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Logic {
    And,
    Or,
}

/// What a case compares its left operand with.
#[derive(Clone, Copy, Debug)]
pub(super) enum Rhs {
    Operand(Operand),
    /// `i1 true`: a condition tested as it is.
    True,
}

/// `CaseBlock` of a merged condition: `lhs cc rhs` to `true_bb`, else
/// `false_bb`, in `this_bb`.
#[derive(Clone, Copy, Debug)]
pub(super) struct MergedCase {
    cc: CondCode,
    lhs: Operand,
    rhs: Rhs,
    true_bb: BlockId,
    false_bb: BlockId,
    this_bb: BlockId,
    /// `CaseBlock::TrueProb` and `FalseProb`.
    t_prob: BranchProbability,
    f_prob: BranchProbability,
}

impl MergedCase {
    pub(super) fn block(&self) -> BlockId {
        self.this_bb
    }
}

/// `m_LogicalAnd` / `m_LogicalOr` on `value`: the operation and its operands,
/// of an `and`/`or` or of a `select` with a constant arm.
fn logical(
    function: &Function,
    context: &llrm_mir::context::Context,
    value: Operand,
) -> Option<(InstId, Logic, Operand, Operand)> {
    let Operand::Value(value) = value else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    let one = function.instruction(inst);
    let constant = |operand: Operand, want: u128| match operand {
        Operand::Constant(id) => {
            matches!(context.get(id).kind, ConstantKind::Int(bits) if bits == want)
                || (want == 0 && matches!(context.get(id).kind, ConstantKind::Zero))
        }
        _ => false,
    };
    match &one.opcode {
        Opcode::Binary(BinaryOp::And) => Some((inst, Logic::And, one.operands[0], one.operands[1])),
        Opcode::Binary(BinaryOp::Or) => Some((inst, Logic::Or, one.operands[0], one.operands[1])),
        Opcode::Select if constant(one.operands[2], 0) => Some((inst, Logic::And, one.operands[0], one.operands[1])),
        Opcode::Select if constant(one.operands[1], 1) => Some((inst, Logic::Or, one.operands[0], one.operands[2])),
        _ => None,
    }
}

impl Builder<'_> {
    /// The instruction defining `operand`, if it is one.
    fn defining(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = operand else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    /// `InBlock`: not an instruction, or one of the current IR block.
    fn in_block(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_none_or(|inst| self.function.parent(inst) == Some(self.block))
    }

    /// `isExportableFromCurrentBlock`.
    fn exportable(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Value(value) = operand else { return true };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => {
                self.function.parent(inst) == Some(self.block) || self.info.value_regs.contains_key(&value)
            }
            ValueDef::Argument(_) => {
                self.function.entry() == Some(self.block) || self.info.value_regs.contains_key(&value)
            }
        }
    }

    /// `visitCondBr`'s branch on a tree of logical operations. Whether it took
    /// the branch: false leaves `visit_br` to make the plain one.
    pub(super) fn visit_merged_branch(
        &mut self,
        instruction: &Instruction,
        on_true: BlockId,
        on_false: BlockId,
    ) -> Result<bool, Refused> {
        let cond = instruction.operands[0];
        if instruction.metadata.iter().any(|(name, _)| name == "unpredictable") {
            return Ok(false);
        }
        let Operand::Value(value) = cond else { return Ok(false) };
        if !self.function.users(value).has_one_use() {
            return Ok(false);
        }
        let Some((cond_inst, logic, lhs, rhs)) = logical(self.function, &self.module.context, cond) else {
            return Ok(false);
        };
        if self.keep_conditions_together(cond_inst, logic, lhs, rhs, on_true, on_false) {
            return Ok(false);
        }
        let (order, next, names) = (self.order.clone(), self.next_synthetic, self.dag.synthetic_names.clone());
        let mut cases = Vec::new();
        let (t_prob, f_prob) =
            (self.edge_probability(self.block, on_true), self.edge_probability(self.block, on_false));
        self.find_merged_conditions(
            cond,
            on_true,
            on_false,
            self.block,
            self.block,
            logic,
            (t_prob, f_prob),
            false,
            &mut cases,
        )?;
        // A runtime routine's flags answer lives only until the next call: a
        // case block would export it, so the compares stay in this block.
        let flags_read = cases
            .iter()
            .any(
                |case| matches!(
                    case.lhs,
                    Operand::Value(value) if self.node_map.get(&value).is_some_and(|one| self.dag.opcode(*one) == super::super::x86isd::LLRM_CALL_FLAGS)
                ),
            );
        if flags_read || !self.should_emit_as_branches(&cases) {
            // remove the blocks the walk made
            self.order = order;
            self.next_synthetic = next;
            self.dag.synthetic_names = names;
            return Ok(false);
        }
        for case in &cases[1..] {
            self.export_condition(case.lhs)?;
            if let Rhs::Operand(rhs) = case.rhs {
                self.export_condition(rhs)?;
            }
        }
        let mut cases = cases.into_iter();
        let first = cases.next().expect("a merged condition has a case");
        self.deferred.extend(cases.map(super::switch::Deferred::Merged));
        self.emit_merged_case(&first)?;
        Ok(true)
    }

    /// `shouldKeepJumpConditionsTogether` with X86's
    /// `getJumpConditionMergingParams`: whether the compares stay one
    /// condition.
    fn keep_conditions_together(
        &mut self,
        cond_inst: InstId,
        logic: Logic,
        lhs: Operand,
        rhs: Operand,
        on_true: BlockId,
        on_false: BlockId,
    ) -> bool {
        // DEVIATE SD103: at optsize two compares stay two branches; merged they
        // cost a `setcc` pair and a logic op, and byte registers are scarce.
        if self.has_opt_size() {
            return false;
        }
        let Some(tti) = self.target.tti.clone() else { return true };
        let tti = tti.0;
        let shared = |logic: Logic| match logic {
            Logic::And => llrm_mir::jumpmerge::Logic::And,
            Logic::Or => llrm_mir::jumpmerge::Logic::Or,
        };
        let logic = shared(logic);
        let params = tti.jump_condition_merging_params(&llrm_mir::jumpmerge::JumpMergeQuery {
            context: &self.module.context,
            function: self.function,
            logic,
            lhs,
            rhs,
        });
        let optimizes = self.target_optimizes();
        let (function, block) = (self.function, self.block);
        let (module, layout) = (self.module, self.layout);
        let bpi = &mut self.info.bpi;
        let mut likely = || {
            if !optimizes {
                return None;
            }
            let bpi = bpi.get_or_insert_with(|| {
                llrm_analysis::bpi::BranchProbabilityInfo::of(
                    llrm_analysis::bpi::Module {
                        context: &module.context,
                        declarations: &module.globals,
                        metadata: &module.metadata,
                        layout,
                    },
                    function,
                )
            });
            // the pin names the successors the other way round
            // (`IfFalse = getSuccessor(0)`): the edge to the false destination
            // is the one it calls true
            if bpi.is_edge_hot(function, block, on_false) {
                Some(true)
            } else if bpi.is_edge_hot(function, block, on_true) {
                Some(false)
            } else {
                None
            }
        };
        llrm_mir::jumpmerge::should_keep_together(
            &*tti,
            &module.context,
            layout,
            &module.globals,
            function,
            cond_inst,
            logic,
            (lhs, rhs),
            params,
            &mut likely,
        )
    }

    /// `FindMergedConditions`.
    #[allow(clippy::too_many_arguments)]
    fn find_merged_conditions(
        &mut self,
        cond: Operand,
        true_bb: BlockId,
        false_bb: BlockId,
        cur_bb: BlockId,
        switch_bb: BlockId,
        opc: Logic,
        (t_prob, f_prob): (BranchProbability, BranchProbability),
        invert: bool,
        cases: &mut Vec<MergedCase>,
    ) -> Result<(), Refused> {
        // skip over a `not` that is not part of the tree, inverting op and
        // operands
        if let Some(inst) = self.defining(cond)
            && let Operand::Value(result) = cond
            && self.function.users(result).has_one_use()
            && let Some(inner) = self.not_operand(inst)
            && self.in_block(inner)
        {
            return self.find_merged_conditions(
                inner,
                true_bb,
                false_bb,
                cur_bb,
                switch_bb,
                opc,
                (t_prob, f_prob),
                !invert,
                cases,
            );
        }
        let tree = logical(self.function, &self.module.context, cond).map(|(inst, mut op, a, b)| {
            if invert {
                op = if op == Logic::And { Logic::Or } else { Logic::And };
            }
            (inst, op, a, b)
        });
        let in_tree = tree.filter(|(inst, op, a, b)| {
            *op == opc
                && cond_has_one_use(self.function, cond)
                && self.function.parent(*inst) == Some(self.block)
                && self.in_block(*a)
                && self.in_block(*b)
        });
        let Some((_, _, op0, op1)) = in_tree else {
            return self.emit_branch_for_merged_condition(
                cond,
                true_bb,
                false_bb,
                cur_bb,
                switch_bb,
                (t_prob, f_prob),
                invert,
                cases,
            );
        };
        // a block after the current one
        let at = self.order.iter().position(|one| *one == cur_bb).map_or(self.order.len(), |at| at + 1);
        let mut place = at;
        let tmp = self.new_block(&mut place);
        match opc {
            Logic::Or => {
                // X | Y: jmp_if_X TBB, jmp TmpBB; TmpBB: jmp_if_Y TBB, jmp FBB
                // The probability of the first is TProb/2 to TBB and the rest
                // to TmpBB; the second gets A/2 and B normalized.
                let first = (t_prob.div(2), t_prob.div(2).saturating_add(f_prob));
                self.find_merged_conditions(op0, true_bb, tmp, cur_bb, switch_bb, opc, first, invert, cases)?;
                let second = normalize(t_prob.div(2), f_prob);
                self.find_merged_conditions(op1, true_bb, false_bb, tmp, switch_bb, opc, second, invert, cases)?;
            }
            Logic::And => {
                // X & Y: jmp_if_X TmpBB, jmp FBB; TmpBB: jmp_if_Y TBB, jmp FBB
                // TrueProb for BB1 + FalseProb for TmpBB = FalseProb for the
                // original block.
                let first = (t_prob.saturating_add(f_prob.div(2)), f_prob.div(2));
                self.find_merged_conditions(op0, tmp, false_bb, cur_bb, switch_bb, opc, first, invert, cases)?;
                let second = normalize(t_prob, f_prob.div(2));
                self.find_merged_conditions(op1, true_bb, false_bb, tmp, switch_bb, opc, second, invert, cases)?;
            }
        }
        Ok(())
    }

    /// `m_Not(x)` of an `i1`: `xor x, true`.
    fn not_operand(
        &self,
        inst: InstId,
    ) -> Option<Operand> {
        let one = self.function.instruction(inst);
        if one.opcode != Opcode::Binary(BinaryOp::Xor) {
            return None;
        }
        let ones = |operand: Operand| match operand {
            Operand::Constant(id) => matches!(self.module.context.get(id).kind, ConstantKind::Int(1)),
            _ => false,
        };
        if ones(one.operands[1]) {
            Some(one.operands[0])
        } else if ones(one.operands[0]) {
            Some(one.operands[1])
        } else {
            None
        }
    }

    /// `EmitBranchForMergedCondition`: a leaf of the tree.
    #[allow(clippy::too_many_arguments)]
    fn emit_branch_for_merged_condition(
        &mut self,
        cond: Operand,
        true_bb: BlockId,
        false_bb: BlockId,
        cur_bb: BlockId,
        switch_bb: BlockId,
        (t_prob, f_prob): (BranchProbability, BranchProbability),
        invert: bool,
        cases: &mut Vec<MergedCase>,
    ) -> Result<(), Refused> {
        if let Some(inst) = self.defining(cond) {
            let compare = self.function.instruction(inst);
            let operands = match &compare.opcode {
                Opcode::ICmp(_) | Opcode::FCmp(_) => Some((compare.operands[0], compare.operands[1])),
                _ => None,
            };
            if let Some((lhs, rhs)) = operands
                && (cur_bb == switch_bb || (self.exportable(lhs) && self.exportable(rhs)))
            {
                let cc = match &compare.opcode {
                    Opcode::ICmp(predicate) => super::cond_code(if invert { predicate.inverse() } else { *predicate }),
                    Opcode::FCmp(predicate) => {
                        let predicate: FloatPredicate = if invert { predicate.inverse() } else { *predicate };
                        let cc = super::float_cond_code(predicate);
                        if compare.flags.contains(llrm_mir::opcode::Flags::NNAN) { without_nan(cc) } else { cc }
                    }
                    _ => unreachable!("a compare"),
                };
                cases.push(MergedCase {
                    cc,
                    lhs,
                    rhs: Rhs::Operand(rhs),
                    true_bb,
                    false_bb,
                    this_bb: cur_bb,
                    t_prob,
                    f_prob,
                });
                return Ok(());
            }
        }
        let cc = if invert { CondCode::SetNe } else { CondCode::SetEq };
        cases.push(MergedCase { cc, lhs: cond, rhs: Rhs::True, true_bb, false_bb, this_bb: cur_bb, t_prob, f_prob });
        Ok(())
    }

    /// `ShouldEmitAsBranches`.
    fn should_emit_as_branches(
        &self,
        cases: &[MergedCase],
    ) -> bool {
        let [first, second] = cases else { return true };
        let (Rhs::Operand(a_rhs), Rhs::Operand(b_rhs)) = (first.rhs, second.rhs) else { return true };
        // two compares of the same values fold into one
        if (first.lhs == second.lhs && a_rhs == b_rhs) || (a_rhs == second.lhs && first.lhs == b_rhs) {
            return false;
        }
        // (X != null) | (Y != null) --> (X|Y) != 0, and the same of ==/&
        let null = matches!(
            a_rhs,
            Operand::Constant(id) if matches!(self.module.context.get(id).kind, ConstantKind::Null | ConstantKind::Zero | ConstantKind::Int(0))
        );
        if a_rhs == b_rhs && first.cc == second.cc && null {
            if first.cc == CondCode::SetEq && first.true_bb == second.this_bb {
                return false;
            }
            if first.cc == CondCode::SetNe && first.false_bb == second.this_bb {
                return false;
            }
        }
        true
    }

    /// `visitSwitchCase` for a merged case.
    pub(super) fn emit_merged_case(
        &mut self,
        case: &MergedCase,
    ) -> Result<(), Refused> {
        let lhs = self.get_value(case.lhs)?;
        self.add_successor_with_prob(case.this_bb, case.true_bb, case.t_prob);
        self.add_successor_with_prob(case.this_bb, case.false_bb, case.f_prob);
        let test = match case.rhs {
            // fold `(X == true)` to X
            Rhs::True if case.cc == CondCode::SetEq => lhs,
            Rhs::True => {
                let one = self.dag.get_constant(1, self.dag.value_type(lhs));
                self.dag.get_setcc(EVT::I1, lhs, one, case.cc)
            }
            Rhs::Operand(rhs) => {
                let rhs = self.get_value(rhs)?;
                self.dag.get_setcc(EVT::I1, lhs, rhs, case.cc)
            }
        };
        let (mut true_bb, mut false_bb, mut test) = (case.true_bb, case.false_bb, test);
        if Some(true_bb) == self.layout_next(case.this_bb) {
            std::mem::swap(&mut true_bb, &mut false_bb);
            let one = self.dag.get_constant(1, self.dag.value_type(test));
            test = self.dag.get_node(ISD::Xor, self.dag.value_type(test), &[test, one]);
        }
        let root = self.control_root();
        let dest = self.dag.get_basic_block(true_bb.0);
        let brcond = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, test, dest]);
        let fall = self.dag.get_basic_block(false_bb.0);
        let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[brcond, fall]);
        self.dag.set_root(node);
        Ok(())
    }
}

/// `BranchProbability::normalizeProbabilities` of two: the pair scaled to sum
/// to one, equal when both are zero.
fn normalize(
    a: BranchProbability,
    b: BranchProbability,
) -> (BranchProbability, BranchProbability) {
    let sum = u64::from(a.0) + u64::from(b.0);
    if sum == 0 {
        let half = BranchProbability::new(1, 2);
        return (half, half.compl());
    }
    let first = BranchProbability((u64::from(a.0) * (1 << 31) / sum) as u32);
    (first, first.compl())
}

/// `value` has one use.
fn cond_has_one_use(
    function: &Function,
    value: Operand,
) -> bool {
    matches!(value, Operand::Value(v) if function.users(v).has_one_use())
}

/// `getFCmpCodeWithoutNaN`: the code of a compare that cannot see a NaN.
fn without_nan(cc: CondCode) -> CondCode {
    match cc {
        CondCode::SetOEq | CondCode::SetUEq => CondCode::SetEq,
        CondCode::SetOGt | CondCode::SetUGt => CondCode::SetGt,
        CondCode::SetOGe | CondCode::SetUGe => CondCode::SetGe,
        CondCode::SetOLt | CondCode::SetULt => CondCode::SetLt,
        CondCode::SetOLe | CondCode::SetULe => CondCode::SetLe,
        CondCode::SetONe | CondCode::SetUNe => CondCode::SetNe,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::datalayout::DataLayout;

    use super::super::{BuilderTarget, build_block};
    use crate::backend::selectiondag::function_lowering::FunctionLoweringInfo;

    /// How many blocks the branch of `f` was split into beyond its own.
    fn split_blocks(
        body: &str,
        tti: bool,
    ) -> usize {
        let text = format!(
            "target datalayout = \"e-p:32:32-n8:16:32\"\ndefine i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, ptr %p, i8 %e) {{\nentry:\n  %x = icmp slt i32 %a, %b\n{body}\n  br i1 %o, label %t, label %e2\nt:\n  ret i32 1\ne2:\n  ret i32 2\n}}\n"
        );
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let (id, _, function) =
            module.functions().find(|(_, g, f)| !f.is_declaration() && g.name.as_deref() == Some("f")).unwrap();
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        let mut target = BuilderTarget::m32();
        if tti {
            target = target.with_tti(Some(std::rc::Rc::new(llrm_x86::tti::X86::of_cpu("i486").unwrap())));
        }
        build_block(
            &module,
            function,
            &layout,
            &target,
            &llrm_x86_m32::M32,
            &convention,
            &mut info,
            function.entry().unwrap(),
        )
        .unwrap_or_else(|why| panic!("{}", why.0))
        .extra
        .len()
    }

    /// The probabilities the branch was lowered with: (from, to, 2^31sts).
    fn edges_of(body: &str) -> Vec<(u32, u32, u32)> {
        let text = format!(
            "target datalayout = \"e-p:32:32-n8:16:32\"\ndefine i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, ptr %p, i8 %e) {{\nentry:\n  %x = icmp slt i32 %a, %b\n{body}\n  br i1 %o, label %t, label %e2, !prof !0\nt:\n  ret i32 1\ne2:\n  ret i32 2\n}}\n!0 = !{{!\"branch_weights\", i32 3, i32 1}}\n"
        );
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let (id, _, function) =
            module.functions().find(|(_, g, f)| !f.is_declaration() && g.name.as_deref() == Some("f")).unwrap();
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        let target = BuilderTarget::m32().with_tti(Some(std::rc::Rc::new(llrm_x86::tti::X86::of_cpu("i486").unwrap())));
        build_block(
            &module,
            function,
            &layout,
            &target,
            &llrm_x86_m32::M32,
            &convention,
            &mut info,
            function.entry().unwrap(),
        )
        .unwrap_or_else(|why| panic!("{}", why.0));
        info.edge_probs.iter().map(|(from, to, p)| (from.0, to.0, *p)).collect()
    }

    /// A branch lowered as two (`a < b || c * d > 7`) was 50/50 on each edge in
    /// the machine CFG, whatever the IR said; the pin's `FindMergedConditions`
    /// shares the branch's probabilities out so that leaving by each target
    /// has the probability the branch gave it (`TProb/2` to the first
    /// compare's true edge, the rest on to the second).
    #[test]
    fn the_two_branches_of_a_merged_or_leave_by_each_target_with_the_branchs_probability() {
        let edges = edges_of(MUL);
        let total = 2_147_483_648.0_f64;
        let of = |from: u32, to: u32| {
            edges.iter().filter(|e| e.0 == from && e.1 == to).map(|e| f64::from(e.2) / total).sum::<f64>()
        };
        // blocks: entry, t, e2 (ids 0, 1, 2); the merged compare's block
        // follows
        let tmp = edges.iter().map(|e| e.0).find(|from| *from != 0).expect("a block of the second compare");
        let to_t = of(0, 1) + of(0, tmp) * of(tmp, 1);
        let to_e2 = of(0, 2) + of(0, tmp) * of(tmp, 2);
        assert!((to_t - 0.75).abs() < 1e-3, "{edges:?}: leaves by t with {to_t}, the branch gave 3/4");
        assert!((to_e2 - 0.25).abs() < 1e-3, "{edges:?}: leaves by e2 with {to_e2}, the branch gave 1/4");
    }

    const CHEAP: &str = "  %l = add i32 %c, 1\n  %y = icmp sgt i32 %l, 7\n  %o = or i1 %x, %y";
    const DEEP: &str = "  %l = add i32 %c, 1\n  %m = add i32 %l, %d\n  %y = icmp sgt i32 %m, 7\n  %o = or i1 %x, %y";
    const LOAD: &str = "  %l = load i32, ptr %p\n  %y = icmp sgt i32 %l, 7\n  %o = or i1 %x, %y";
    const MUL: &str = "  %l = mul i32 %c, %d\n  %y = icmp sgt i32 %l, 7\n  %o = or i1 %x, %y";
    const SHIFT: &str = "  %l = shl i32 %c, 2\n  %y = icmp sgt i32 %l, 7\n  %o = or i1 %x, %y";

    /// The pin's llc (i486) keeps `a < b || c + 1 > 7` as one condition and
    /// splits a right side costing more than the threshold of 2 latency
    /// units: `c + 1 + d`, a load, a multiply. Compared by its asm.
    #[test]
    fn a_right_side_over_the_threshold_is_a_branch_of_its_own() {
        assert_eq!(split_blocks(CHEAP, true), 0);
        assert_eq!(split_blocks(SHIFT, true), 0);
        for costly in [DEEP, LOAD, MUL] {
            assert_eq!(split_blocks(costly, true), 1, "{costly}");
        }
    }

    /// A right side whose dependencies reach an `alloca` five levels down is an
    /// incomplete count (the `alloca`'s array-size operand is the sixth level),
    /// so the pin splits it however cheap it is. Nib's `main` kept a nine-term
    /// `||` as `setcc`/`or` (+13 `test`/`or` in jpeg_fixed) because the count
    /// stopped short of the `alloca` behind a phi.
    #[test]
    fn a_right_side_five_levels_above_an_alloca_is_incomplete_and_splits() {
        let chain = |gep: usize| {
            let mut text = String::from(
                "  %a0 = alloca i32
  store i32 0, ptr %a0
",
            );
            let mut last = "%a0".to_owned();
            for at in 1..=gep {
                text += &format!(
                    "  %g{at} = getelementptr i8, ptr {last}, i32 0
  store i32 0, ptr %g{at}
"
                );
                last = format!("%g{at}");
            }
            text + &format!(
                "  %pi = ptrtoint ptr {last} to i32
  %y = icmp sgt i32 %pi, 7
  %o = or i1 %x, %y"
            )
        };
        assert_eq!(split_blocks(&chain(3), true), 1, "alloca at the sixth level");
        assert_eq!(split_blocks(&chain(2), true), 0, "alloca one level up");
    }

    /// Without a cost model the conditions stay together, as before.
    #[test]
    fn without_a_cost_model_conditions_stay_together() {
        assert_eq!(split_blocks(DEEP, false), 0);
    }

    /// An `or` of two `==` splits whatever it costs (no CCMP here), and a
    /// `&&` of three compares is two blocks past the first.
    #[test]
    fn an_or_of_equalities_splits() {
        let eq = "  %y = icmp eq i32 %c, %d\n  %z = icmp eq i32 %a, %b\n  %o = or i1 %z, %y";
        assert_eq!(split_blocks(eq, true), 1);
    }

    /// Queens' test, `d == 0 || d == row - r || d == r - row`, reaches the
    /// builder as a chain of `select c, true, x` (`m_LogicalOr` takes it), and
    /// a `select c, x, false` is `m_LogicalAnd`: a chain whose right sides cost
    /// more than the threshold is a chain of branches, two blocks past the
    /// first, not `setcc`s and `or`s. (Queens' own chain splits by the
    /// profile: its early exit is hot.)
    #[test]
    fn a_chain_of_selects_is_a_chain_of_branches() {
        let or = "  %l1 = load i32, ptr %p
  %m1 = mul i32 %l1, %d
  %e1 = icmp sgt i32 %m1, 7
  %c0 = select i1 %x, i1 true, i1 %e1
  %l2 = load i32, ptr %p
  %m2 = mul i32 %l2, %c
  %e2 = icmp sgt i32 %m2, 9
  %o = select i1 %c0, i1 true, i1 %e2";
        assert_eq!(split_blocks(or, true), 2);
        let and = "  %l1 = load i32, ptr %p
  %m1 = mul i32 %l1, %d
  %e1 = icmp sgt i32 %m1, 7
  %c0 = select i1 %x, i1 %e1, i1 false
  %l2 = load i32, ptr %p
  %m2 = mul i32 %l2, %c
  %e2 = icmp sgt i32 %m2, 9
  %o = select i1 %c0, i1 %e2, i1 false";
        assert_eq!(split_blocks(and, true), 2);
    }
}
