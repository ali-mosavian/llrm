//! `visitSwitch`'s work list: the clusters of a `switch` (see [`switchcg`])
//! split in halves and lowered to compare-and-branch blocks.
//!
//! The switch block is built as the pin builds it; every other block the
//! lowering makes (`CreateMachineBasicBlock`) is a `CaseBlock` held back
//! until the layout is final, then built as a block of its own
//! (`FinishBasicBlock`).

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, Instruction, Operand};
use llrm_mir::opcode::Opcode;

use super::super::dag::SDValue;
use super::super::isd::{CondCode, ISD};
use super::super::mvt::EVT;
use super::super::switchcg::{self, BranchProbability, CaseCluster, ClusterKind, SwitchTarget};
use super::{Builder, Refused};

/// A value a case block compares: the switch condition or a case constant.
#[derive(Clone, Copy, Debug)]
pub(super) enum CaseValue {
    Condition,
    Constant(i128),
}

/// `CaseBlock`: `lhs cc rhs` (or `low <= mhs <= high`) to `true_bb`, else
/// `false_bb`, in `this_bb`.
#[derive(Clone, Copy, Debug)]
pub(super) struct CaseBlock {
    cc: CondCode,
    lhs: CaseValue,
    rhs: CaseValue,
    mhs: Option<CaseValue>,
    true_bb: BlockId,
    false_bb: BlockId,
    this_bb: BlockId,
    /// `TrueProb` and `FalseProb`: what leaves by each edge.
    true_prob: BranchProbability,
    false_prob: BranchProbability,
}

/// What a block made by the lowering holds, built once the layout is final.
#[derive(Clone, Debug)]
pub(super) enum Deferred {
    /// A compare and branch.
    Case(CaseBlock, Operand),
    /// `visitJumpTableHeader` in a block other than the switch's.
    JumpTableHeader(JumpTableHeader),
    /// `visitJumpTable`: the indirect branch through the table.
    JumpTable { block: BlockId, reg: u32, index: u32, dests: Vec<(u32, BranchProbability)> },
    /// `visitBitTestHeader` in a block other than the switch's.
    BitTestHeader(BitTestHeader),
    /// A block `FindMergedConditions` placed: one compare and branch.
    Merged(super::mergedcond::MergedCase),
    /// `visitBitTestCase`: one mask test.
    BitTestCase { block: BlockId, header: BitTestHeader, case: usize, next: BlockId },
}

impl Deferred {
    pub(super) fn block(&self) -> BlockId {
        match self {
            Deferred::Case(case, _) => case.this_bb,
            Deferred::JumpTableHeader(header) => header.block,
            Deferred::JumpTable { block, .. } | Deferred::BitTestCase { block, .. } => *block,
            Deferred::BitTestHeader(header) => header.block,
            Deferred::Merged(case) => case.block(),
        }
    }
}

/// `JumpTableHeader` with the `JumpTable` it heads.
#[derive(Clone, Debug)]
pub(super) struct JumpTableHeader {
    block: BlockId,
    cond: Operand,
    first: i128,
    last: i128,
    reg: u32,
    table_block: BlockId,
    default: BlockId,
    unreachable: bool,
    /// The edge to the table and the edge to the default.
    table_prob: BranchProbability,
    default_prob: BranchProbability,
}

/// `BitTestBlock` as the header and the case blocks read it.
#[derive(Clone, Debug)]
pub(super) struct BitTestHeader {
    block: BlockId,
    cond: Operand,
    low_bound: i128,
    range: i128,
    reg: u32,
    case_blocks: Vec<BlockId>,
    masks: Vec<(u64, BlockId)>,
    default: BlockId,
    unreachable: bool,
}

/// `SwitchWorkListItem`: clusters `first..=last` to lower in `block`, which
/// are known to lie in `ge <= value < lt`.
struct WorkItem {
    block: BlockId,
    first: usize,
    last: usize,
    ge: Option<i128>,
    lt: Option<i128>,
    default_prob: BranchProbability,
}

impl Builder<'_> {
    /// The block after `block` in the layout so far.
    pub(super) fn layout_next(
        &self,
        block: BlockId,
    ) -> Option<BlockId> {
        let at = self.order.iter().position(|one| *one == block)?;
        self.order.get(at + 1).copied()
    }

    /// A new block, placed at `*at` in the layout (and `*at` moves past it),
    /// named as the switch's block.
    pub(super) fn new_block(
        &mut self,
        at: &mut usize,
    ) -> BlockId {
        let id = BlockId(self.next_synthetic);
        self.next_synthetic += 1;
        self.order.insert(*at, id);
        *at += 1;
        let name = self.dag.block_name(self.block.0).to_owned();
        self.dag.set_block_name(id.0, name);
        id
    }

    /// `ExportFromCurrentBlock(Cond)`.
    pub(super) fn export_condition(
        &mut self,
        cond: Operand,
    ) -> Result<(), Refused> {
        let Operand::Value(value) = cond else { return Ok(()) };
        if self.info.value_regs.contains_key(&value) {
            return Ok(());
        }
        let reg = self.info.create_reg(value);
        self.copy_value_to_register(cond, reg, ISD::AnyExtend)
    }

    pub(super) fn visit_switch(
        &mut self,
        instruction: &Instruction,
    ) -> Result<(), Refused> {
        let cond = instruction.operands[0];
        let cond_type = self.operand_type(cond)?;
        let width = cond_type.size_in_bits();
        let Operand::Block(default) = instruction.operands[1] else { return Err(Refused("switch default")) };
        let bpi = self
            .info
            .bpi
            .get_or_insert_with(
                || llrm_analysis::bpi::BranchProbabilityInfo::of(
                    llrm_analysis::bpi::Module {
                        context: &self.module.context,
                        declarations: &self.module.globals,
                        metadata: &self.module.metadata,
                        layout: self.layout,
                    },
                    self.function,
                ),
            );
        let edge = |index: usize| {
            BranchProbability(bpi.get_edge_probability(self.function, self.block, index).get_numerator())
        };
        let default_edge =
            BranchProbability(bpi.get_edge_probability_to(self.function, self.block, default).get_numerator());
        let mut cases = Vec::new();
        for (at, pair) in instruction.operands[2..].chunks(2).enumerate() {
            let (Operand::Constant(value), Operand::Block(dest)) = (pair[0], pair[1]) else {
                return Err(Refused("switch case"));
            };
            let ConstantKind::Int(bits) = self.module.context.get(value).kind else {
                return Err(Refused("switch case value"));
            };
            let shift = 128 - width;
            cases.push((((bits << shift) as i128) >> shift, dest.0, edge(at + 1)));
        }
        // The target prices a jump table at 0 where it has none to select (the
        // MIR pass reads the same cost).
        let target = SwitchTarget {
            jump_tables: self.target.jump_tables,
            opt_size: self.target.opt_size || self.function_has("optsize") || self.function_has("minsize"),
            ..SwitchTarget::x86(self.target.dag.pointer.size_in_bits())
        };
        let plan = switchcg::plan_with(&cases, default.0, &target);
        if plan.clusters.is_empty() {
            let next = self.layout_next(self.block);
            if Some(default) != next {
                let root = self.control_root();
                let dest = self.dag.get_basic_block(default.0);
                let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
                self.dag.set_root(node);
            }
            return Ok(());
        }
        let clusters = plan.clusters.clone();
        let peeled = plan.peeled.clone();
        self.plan = Some(plan);
        let unreachable_default = self
            .function
            .iter_block(default)
            .next()
            .is_some_and(|inst| self.function.instruction(inst).opcode == Opcode::Unreachable);
        // `peelDominantCaseCluster`: the cluster most of the switch goes to is
        // tested first, in this block, and falls through to a block after it
        // where the rest of the switch is lowered.
        let (start, default_prob) = match peeled {
            Some((cluster, prob)) => {
                let mut at = self.order.iter().position(|one| *one == self.block).expect("a placed block") + 1;
                let rest = self.new_block(&mut at);
                self.export_condition(cond)?;
                let mut one = [cluster];
                let item =
                    WorkItem { block: self.block, first: 0, last: 0, ge: None, lt: None, default_prob: prob.compl() };
                self.lower_work_item(&item, &mut one, cond, rest, false)?;
                (rest, switchcg::scale_case_probability(default_edge, prob))
            }
            None => (self.block, default_edge),
        };
        let mut work =
            vec![WorkItem { block: start, first: 0, last: clusters.len() - 1, ge: None, lt: None, default_prob }];
        let mut clusters = clusters;
        while let Some(item) = work.pop() {
            if item.last - item.first + 1 > 3 && self.target_optimizes() {
                self.split_work_item(&mut work, &item, &clusters, cond)?;
                continue;
            }
            self.lower_work_item(&item, &mut clusters, cond, default, unreachable_default)?;
        }
        Ok(())
    }

    pub(super) fn visit_deferred(
        &mut self,
        item: Deferred,
    ) -> Result<(), Refused> {
        match item {
            Deferred::Case(case, cond) => self.visit_switch_case(case, cond),
            Deferred::Merged(case) => self.emit_merged_case(&case),
            Deferred::JumpTableHeader(header) => self.emit_jump_table_header(&header),
            Deferred::JumpTable { block, reg, index, dests } => {
                for (dest, probability) in dests {
                    self.add_successor_with_prob(block, BlockId(dest), probability);
                }
                // visitJumpTable: `br_jt (CopyFromReg reg):1, JumpTable, index`
                let pointer = self.target.dag.pointer;
                let root = self.control_root();
                let copy = self.dag.get_copy_from_reg(root, reg, pointer);
                let table = self.dag.get_jump_table(pointer, index);
                let chain = SDValue { node: copy.node, res: 1 };
                let node = self.dag.get_node(ISD::BrJT, EVT::OTHER, &[chain, table, copy]);
                self.dag.set_root(node);
                Ok(())
            }
            Deferred::BitTestHeader(header) => self.emit_bit_test_header(&header),
            Deferred::BitTestCase { block, header, case, next } => self.emit_bit_test_case(block, &header, case, next),
        }
    }

    /// `visitJumpTableHeader`.
    fn emit_jump_table_header(
        &mut self,
        header: &JumpTableHeader,
    ) -> Result<(), Refused> {
        let pointer = self.target.dag.pointer;
        let switch_op = self.get_value(header.cond)?;
        let vt = self.dag.value_type(switch_op);
        let first = self.dag.get_constant(header.first as u128, vt);
        let sub = self.dag.get_node(ISD::Sub, vt, &[switch_op, first]);
        let index = self.dag.get_zext_or_trunc(sub, pointer);
        let root = self.control_root();
        let copy_to = self.dag.get_copy_to_reg(root, header.reg, index);
        let next = self.layout_next(header.block);
        let mut root = copy_to;
        self.add_successor_with_prob(header.block, header.table_block, header.table_prob);
        if !header.unreachable {
            self.add_successor_with_prob(header.block, header.default, header.default_prob);
            let span = self.dag.get_constant((header.last - header.first) as u128, vt);
            let cmp = self.dag.get_setcc(EVT::I8, sub, span, CondCode::SetUGt);
            let default = self.dag.get_basic_block(header.default.0);
            root = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, cmp, default]);
        }
        if Some(header.table_block) != next {
            let dest = self.dag.get_basic_block(header.table_block.0);
            root = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
        }
        self.dag.set_root(root);
        Ok(())
    }

    /// `visitBitTestHeader`.
    fn emit_bit_test_header(
        &mut self,
        header: &BitTestHeader,
    ) -> Result<(), Refused> {
        let switch_op = self.get_value(header.cond)?;
        let vt = self.dag.value_type(switch_op);
        let low = self.dag.get_constant(header.low_bound as u128, vt);
        let range_sub = self.dag.get_node(ISD::Sub, vt, &[switch_op, low]);
        let reg_vt = self.bit_test_type(vt, header);
        let sub = if reg_vt == vt { range_sub } else { self.dag.get_zext_or_trunc(range_sub, reg_vt) };
        let root = self.control_root();
        let copy_to = self.dag.get_copy_to_reg(root, header.reg, sub);
        let mut root = copy_to;
        if !header.unreachable {
            let range = self.dag.get_constant(header.range as u128, vt);
            let cmp = self.dag.get_setcc(EVT::I8, range_sub, range, CondCode::SetUGt);
            let default = self.dag.get_basic_block(header.default.0);
            root = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, cmp, default]);
        }
        let first = header.case_blocks[0];
        if Some(first) != self.layout_next(header.block) {
            let dest = self.dag.get_basic_block(first.0);
            root = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
        }
        self.dag.set_root(root);
        Ok(())
    }

    /// The type the bit-test index lives in: the condition's, or a pointer
    /// when a mask does not fit it.
    fn bit_test_type(
        &self,
        vt: EVT,
        header: &BitTestHeader,
    ) -> EVT {
        let bits = vt.size_in_bits();
        let fits = bits >= 64 || header.masks.iter().all(|(mask, _)| mask >> bits == 0);
        if fits { vt } else { self.target.dag.pointer }
    }

    /// `visitBitTestCase`.
    fn emit_bit_test_case(
        &mut self,
        block: BlockId,
        header: &BitTestHeader,
        case: usize,
        next: BlockId,
    ) -> Result<(), Refused> {
        let (mask, target) = header.masks[case];
        let cond_type = self.operand_type(header.cond)?;
        let vt = self.bit_test_type(cond_type, header);
        let root = self.control_root();
        let shift_op = self.dag.get_copy_from_reg(root, header.reg, vt);
        let pop = mask.count_ones();
        let cmp = if pop == 1 {
            let bit = self.dag.get_constant(u128::from(mask.trailing_zeros()), vt);
            self.dag.get_setcc(EVT::I8, shift_op, bit, CondCode::SetEq)
        } else if i128::from(pop) == header.range {
            let ones = self.dag.get_constant(u128::from(mask.trailing_ones()), vt);
            self.dag.get_setcc(EVT::I8, shift_op, ones, CondCode::SetNe)
        } else {
            let one = self.dag.get_constant(1, vt);
            let shifted = self.dag.get_node(ISD::Shl, vt, &[one, shift_op]);
            let m = self.dag.get_constant(u128::from(mask), vt);
            let and = self.dag.get_node(ISD::And, vt, &[shifted, m]);
            let zero = self.dag.get_constant(0, vt);
            self.dag.get_setcc(EVT::I8, and, zero, CondCode::SetNe)
        };
        let root = self.control_root();
        let dest = self.dag.get_basic_block(target.0);
        let mut node = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, cmp, dest]);
        if Some(next) != self.layout_next(block) {
            let fall = self.dag.get_basic_block(next.0);
            node = self.dag.get_node(ISD::Br, EVT::OTHER, &[node, fall]);
        }
        self.dag.set_root(node);
        Ok(())
    }

    pub(super) fn target_optimizes(&self) -> bool {
        true
    }

    /// `splitWorkItem`: a compare against the pivot, the halves new work.
    fn split_work_item(
        &mut self,
        work: &mut Vec<WorkItem>,
        item: &WorkItem,
        clusters: &[CaseCluster],
        cond: Operand,
    ) -> Result<(), Refused> {
        let (last_left, first_right, left_prob, right_prob) = compute_split(item, clusters);
        let pivot = clusters[first_right].low;
        let mut at = self.order.iter().position(|one| *one == item.block).expect("a placed block") + 1;
        let left = if last_left == item.first
            && clusters[item.first].kind == ClusterKind::Range
            && Some(clusters[item.first].low) == item.ge
            && clusters[item.first].high + 1 == pivot
        {
            BlockId(clusters[item.first].dest)
        } else {
            let block = self.new_block(&mut at);
            work.push(WorkItem {
                block,
                first: item.first,
                last: last_left,
                ge: item.ge,
                lt: Some(pivot),
                default_prob: item.default_prob.div(2),
            });
            self.export_condition(cond)?;
            block
        };
        let right = if first_right == item.last
            && clusters[item.last].kind == ClusterKind::Range
            && item.lt.is_some_and(|lt| clusters[item.last].high + 1 == lt)
        {
            BlockId(clusters[item.last].dest)
        } else {
            let block = self.new_block(&mut at);
            work.push(WorkItem {
                block,
                first: first_right,
                last: item.last,
                ge: Some(pivot),
                lt: item.lt,
                default_prob: item.default_prob.div(2),
            });
            self.export_condition(cond)?;
            block
        };
        let case = CaseBlock {
            cc: CondCode::SetLt,
            lhs: CaseValue::Condition,
            rhs: CaseValue::Constant(pivot),
            mhs: None,
            true_bb: left,
            false_bb: right,
            this_bb: item.block,
            true_prob: left_prob,
            false_prob: right_prob,
        };
        self.emit_or_defer(case, cond)
    }

    /// `lowerWorkItem` for range clusters: a compare per cluster, each in a
    /// block of its own after the first.
    fn lower_work_item(
        &mut self,
        item: &WorkItem,
        clusters: &mut [CaseCluster],
        cond: Operand,
        default: BlockId,
        unreachable_default: bool,
    ) -> Result<(), Refused> {
        let item_pos = self.order.iter().position(|one| *one == item.block).expect("a placed block");
        let next = self.order.get(item_pos + 1).copied();
        let size = item.last - item.first + 1;
        if size == 2 && item.block == self.block {
            let (small, big) = (clusters[item.first], clusters[item.last]);
            if small.low == small.high && big.low == big.high && small.dest == big.dest {
                let common = big.low ^ small.low;
                if common > 0 && common & (common - 1) == 0 {
                    let lhs = self.get_value(cond)?;
                    let vt = self.dag.value_type(lhs);
                    let bit = self.dag.get_constant(common as u128, vt);
                    let or = self.dag.get_node(ISD::Or, vt, &[lhs, bit]);
                    let want = self.dag.get_constant((big.low | small.low) as u128, vt);
                    let test = self.dag.get_setcc(EVT::I1, or, want, CondCode::SetEq);
                    let root = self.control_root();
                    let yes = self.dag.get_basic_block(small.dest);
                    let brcond = self.dag.get_node(ISD::BrCond, EVT::OTHER, &[root, test, yes]);
                    let no = self.dag.get_basic_block(default.0);
                    let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[brcond, no]);
                    self.dag.set_root(node);
                    self.add_successor_with_prob(item.block, BlockId(small.dest), small.prob.saturating_add(big.prob));
                    self.add_successor_with_prob(item.block, default, item.default_prob);
                    return Ok(());
                }
            }
        }
        // Most probable first (all equal here, so by value), then a cluster
        // whose block is next moves last so it falls through.
        clusters[item.first..=item.last].sort_by(|a, b| b.prob.cmp(&a.prob).then(a.low.cmp(&b.low)));
        let last_prob = clusters[item.last].prob;
        for at in (item.first..item.last).rev() {
            if clusters[at].prob > last_prob {
                break;
            }
            if clusters[at].kind == ClusterKind::Range && Some(BlockId(clusters[at].dest)) == next {
                clusters.swap(at, item.last);
                break;
            }
        }
        let mut insert_at = item_pos + 1;
        let mut current = item.block;
        // `UnhandledProbs`: the default's and every cluster's still to be
        // tested.
        let mut unhandled = clusters[item.first..=item.last]
            .iter()
            .fold(item.default_prob, |sum, cluster| sum.saturating_add(cluster.prob));
        for at in item.first..=item.last {
            let cluster = clusters[at];
            unhandled = unhandled.saturating_sub(cluster.prob);
            let (fallthrough, unreachable) = if at == item.last {
                (default, unreachable_default)
            } else {
                let block = self.new_block(&mut insert_at);
                self.export_condition(cond)?;
                (block, false)
            };
            match cluster.kind {
                ClusterKind::JumpTable(index) => {
                    let table = self.plan.as_ref().expect("a plan").jump_tables[index].clone();
                    let table_block = self.new_block(&mut insert_at);
                    let jt_index = self.info.new_jump_table(table.table.clone());
                    let reg = self.info.new_reg();
                    let header = JumpTableHeader {
                        block: current,
                        cond,
                        first: table.low,
                        last: table.high,
                        reg,
                        table_block,
                        default: fallthrough,
                        unreachable,
                        table_prob: cluster.prob,
                        default_prob: unhandled,
                    };
                    self.deferred.push(Deferred::JumpTable {
                        block: table_block,
                        reg,
                        index: jt_index,
                        dests: table.dests.clone(),
                    });
                    if current == self.block {
                        self.emit_jump_table_header(&header)?;
                    } else {
                        self.deferred.push(Deferred::JumpTableHeader(header));
                    }
                }
                ClusterKind::BitTests(index) => {
                    let bits = self.plan.as_ref().expect("a plan").bit_tests[index].clone();
                    let case_blocks: Vec<BlockId> = bits.cases.iter().map(|_| self.new_block(&mut insert_at)).collect();
                    let reg = self.info.new_reg();
                    let header = BitTestHeader {
                        block: current,
                        cond,
                        low_bound: bits.low_bound,
                        range: bits.range,
                        reg,
                        masks: bits
                            .cases
                            .iter()
                            .zip(&case_blocks)
                            .map(|(case, _)| (case.mask, BlockId(case.dest)))
                            .collect(),
                        case_blocks: case_blocks.clone(),
                        default: fallthrough,
                        unreachable,
                    };
                    for (at, &block) in case_blocks.iter().enumerate() {
                        let next = case_blocks.get(at + 1).copied().unwrap_or(fallthrough);
                        self.deferred.push(Deferred::BitTestCase { block, header: header.clone(), case: at, next });
                    }
                    if current == self.block {
                        self.emit_bit_test_header(&header)?;
                    } else {
                        self.deferred.push(Deferred::BitTestHeader(header));
                    }
                }
                ClusterKind::Range => {
                    let mut case = if cluster.low == cluster.high {
                        CaseBlock {
                            cc: CondCode::SetEq,
                            lhs: CaseValue::Condition,
                            rhs: CaseValue::Constant(cluster.low),
                            mhs: None,
                            true_bb: BlockId(cluster.dest),
                            false_bb: fallthrough,
                            this_bb: current,
                            true_prob: cluster.prob,
                            false_prob: unhandled,
                        }
                    } else {
                        CaseBlock {
                            cc: CondCode::SetLe,
                            lhs: CaseValue::Constant(cluster.low),
                            rhs: CaseValue::Constant(cluster.high),
                            mhs: Some(CaseValue::Condition),
                            true_bb: BlockId(cluster.dest),
                            false_bb: fallthrough,
                            this_bb: current,
                            true_prob: cluster.prob,
                            false_prob: unhandled,
                        }
                    };
                    if unreachable {
                        case.cc = CondCode::SetTrue;
                    }
                    self.emit_or_defer(case, cond)?;
                }
            }
            current = fallthrough;
        }
        Ok(())
    }

    fn emit_or_defer(
        &mut self,
        case: CaseBlock,
        cond: Operand,
    ) -> Result<(), Refused> {
        if case.this_bb == self.block {
            self.visit_switch_case(case, cond)
        } else {
            self.deferred.push(Deferred::Case(case, cond));
            Ok(())
        }
    }

    fn case_constant(
        &mut self,
        value: CaseValue,
        cond: Operand,
    ) -> Result<SDValue, Refused> {
        match value {
            CaseValue::Condition => self.get_value(cond),
            CaseValue::Constant(bits) => {
                let vt = self.operand_type(cond)?;
                Ok(self.dag.get_constant(bits as u128, vt))
            }
        }
    }

    /// `visitSwitchCase`.
    pub(super) fn visit_switch_case(
        &mut self,
        case: CaseBlock,
        cond: Operand,
    ) -> Result<(), Refused> {
        let cond_lhs = self.case_constant(case.lhs, cond)?;
        if case.cc == CondCode::SetTrue {
            let next = self.layout_next(case.this_bb);
            if Some(case.true_bb) != next {
                let root = self.control_root();
                let dest = self.dag.get_basic_block(case.true_bb.0);
                let node = self.dag.get_node(ISD::Br, EVT::OTHER, &[root, dest]);
                self.dag.set_root(node);
            }
            return Ok(());
        }
        let test = match case.mhs {
            None => {
                let rhs = self.case_constant(case.rhs, cond)?;
                self.dag.get_setcc(EVT::I1, cond_lhs, rhs, case.cc)
            }
            Some(mhs) => {
                let (CaseValue::Constant(low), CaseValue::Constant(high)) = (case.lhs, case.rhs) else {
                    return Err(Refused("range case"));
                };
                let op = self.case_constant(mhs, cond)?;
                let vt = self.dag.value_type(op);
                let width = vt.size_in_bits();
                let min_signed = -(1i128 << (width - 1));
                if low == min_signed {
                    let high = self.dag.get_constant(high as u128, vt);
                    self.dag.get_setcc(EVT::I1, op, high, CondCode::SetLe)
                } else {
                    let low_c = self.dag.get_constant(low as u128, vt);
                    let sub = self.dag.get_node(ISD::Sub, vt, &[op, low_c]);
                    let span = self.dag.get_constant((high - low) as u128, vt);
                    self.dag.get_setcc(EVT::I1, sub, span, CondCode::SetULe)
                }
            }
        };
        self.add_successor_with_prob(case.this_bb, case.true_bb, case.true_prob);
        self.add_successor_with_prob(case.this_bb, case.false_bb, case.false_prob);
        let (mut true_bb, mut false_bb, mut test) = (case.true_bb, case.false_bb, test);
        if Some(true_bb) == self.layout_next(case.this_bb) {
            std::mem::swap(&mut true_bb, &mut false_bb);
            let one = self.dag.get_constant(1, EVT::I1);
            test = self.dag.get_node(ISD::Xor, EVT::I1, &[test, one]);
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

/// `computeSplitWorkItemInfo`: the last cluster of the left half, the first
/// of the right, and the probabilities each side gets.
fn compute_split(
    item: &WorkItem,
    clusters: &[CaseCluster],
) -> (usize, usize, BranchProbability, BranchProbability) {
    let mut last_left = item.first;
    let mut first_right = item.last;
    let half = item.default_prob.div(2);
    let mut left = clusters[last_left].prob.saturating_add(half);
    let mut right = clusters[first_right].prob.saturating_add(half);
    let mut i = 0;
    while last_left + 1 < first_right {
        if left < right || (left == right && i & 1 == 1) {
            last_left += 1;
            left = left.saturating_add(clusters[last_left].prob);
        } else {
            first_right -= 1;
            right = right.saturating_add(clusters[first_right].prob);
        }
        i += 1;
    }
    let rank = |cluster: &CaseCluster, first: usize, last: usize| {
        clusters[first..=last]
            .iter()
            .filter(
                |other| if other.prob != cluster.prob { other.prob > cluster.prob } else { other.low < cluster.low },
            )
            .count()
    };
    loop {
        let num_left = last_left - item.first + 1;
        let num_right = item.last - first_right + 1;
        if num_left.min(num_right) < 3 && num_left.max(num_right) > 3 {
            if num_left < num_right {
                let cluster = clusters[first_right];
                if rank(&cluster, item.first, last_left) <= rank(&cluster, first_right, item.last) {
                    last_left += 1;
                    first_right += 1;
                    continue;
                }
            } else {
                let cluster = clusters[last_left];
                if rank(&cluster, first_right, item.last) <= rank(&cluster, item.first, last_left) {
                    last_left -= 1;
                    first_right -= 1;
                    continue;
                }
            }
        }
        break;
    }
    (last_left, first_right, left, right)
}
