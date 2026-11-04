//! PredicateInfo: what a branch, switch or assume says about a value where it
//! holds, as a copy of the value that only the code it holds for reads. A
//! translation of LLVM's `PredicateInfo.cpp` at d1106deb71cc.
//!
//! # How it works
//! For each conditional branch (and each `llvm.assume`), the values the
//! condition compares are candidates: an operand of an `icmp` or `fcmp`, the
//! condition itself, the operand of a `trunc nuw`, and what an `and` (on the
//! true edge) or `or` (on the false edge) of conditions joins, at most eight
//! per branch. A switch gives its condition a predicate for each case that is
//! the only edge into its target. A value with one use is not a candidate.
//!
//! Copies are made for the uses a predicate dominates only. The uses of the
//! value and the places a copy could stand are sorted in the dominator tree's
//! depth-first order, and one pass with a stack of the copies in scope
//! replaces each use with the innermost copy; a copy is made (a no-op
//! `bitcast` of the value, before the branch or after the assume) when a use
//! it dominates is found. [`PredicateInfo::predicate_info_for`] tells which
//! copies are such, and what they say.
//!
//! # Example
//! A use the branch dominates reads a copy that carries the predicate:
//!
//! ```text
//!  b0: %c = icmp ult i32 %x, 10           b0: %c = icmp ult i32 %x, 10
//!      br i1 %c, b1, b2                       %x.0 = bitcast i32 %x
//!  b1: use %x                                 br i1 %c, b1, b2
//!                                         b1: use %x.0     ; 0 <= %x.0 < 10
//! ```
//!
//! # DEVIATEs
//! 1. No operand-bundle assumes (`PredicateBundleAssume`, the `nonnull`
//!    bundle): MIR has no operand bundles.
//! 2. No printer, verifier or `DebugCounter`: the printer pass is a debugging
//!    aid. The copies are not removed here; the caller does
//!    (`replaceCreatedSSACopys` in LLVM's printer, `removeSSACopies` in SCCP).
//! 3. A use is `(user, operand index)`; its order in `Op->uses()` (newest first
//!    in LLVM) only matters among uses that sort as equal, which are replaced
//!    independently.
//!
//! # See also
//! - Used by: [`crate::sccp`]

use std::cmp::Ordering;

use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context};
use llrm_mir::dense::IdMap;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand, Use, ValueId};
use llrm_mir::opcode::{CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern::{logical_and, logical_or};
use llrm_mir::valuetracking::{AssumptionCache, Query};
use llrm_support::hash::HashSet;

use crate::local::{BasicBlockEdge, dominates_edge_use, dominates_instruction_use};

/// Maximum number of conditions considered for renaming for each
/// branch/assume. This limits renaming of deep and/or chains.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/PredicateInfo.cpp#L50>
const MAX_CONDS_PER_BRANCH: usize = 8;

/// `PredicateType` with its payload (`PredicateBranch`, `PredicateSwitch`,
/// `PredicateConditionAssume`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PredicateKind {
    /// `PredicateBranch`: the edge `from` -> `to` of a conditional branch,
    /// the true edge or not.
    Branch { from: BlockId, to: BlockId, true_edge: bool },
    /// `PredicateConditionAssume`.
    ConditionAssume { assume: InstId },
    /// `PredicateSwitch`: the edge of the case with this value.
    Switch { from: BlockId, to: BlockId, case_value: ConstantId },
}

/// A predicate of either kind, as `CmpInst::Predicate` is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CmpPredicate {
    Int(IntPredicate),
    Float(FloatPredicate),
}

/// `PredicateConstraint`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PredicateConstraint {
    pub predicate: CmpPredicate,
    pub other_op: Operand,
}

/// `PredicateBase`.
#[derive(Clone, Debug)]
pub struct PredicateBase {
    pub kind: PredicateKind,
    pub original_op: Operand,
    /// The copy this predicate's copy is a copy of; set when it is made.
    pub renamed_op: Option<Operand>,
    pub condition: Option<Operand>,
}

impl PredicateBase {
    /// `PredicateWithEdge` (a branch or a switch): the edge.
    fn edge(&self) -> Option<(BlockId, BlockId)> {
        match self.kind {
            PredicateKind::Branch { from, to, .. } | PredicateKind::Switch { from, to, .. } => Some((from, to)),
            PredicateKind::ConditionAssume { .. } => None,
        }
    }

    /// `PredicateBase::getConstraint`.
    pub fn get_constraint(
        &self,
        context: &Context,
        function: &Function,
    ) -> Option<PredicateConstraint> {
        let renamed = self.renamed_op?;
        match self.kind {
            PredicateKind::ConditionAssume { .. } | PredicateKind::Branch { .. } => {
                let true_edge = match self.kind {
                    PredicateKind::Branch { true_edge, .. } => true_edge,
                    _ => true,
                };
                let condition = self.condition?;
                if condition == renamed {
                    let ty = function.operand_type(context, condition)?;
                    let truth = context.int(ty, i128::from(true_edge));
                    return Some(PredicateConstraint {
                        predicate: CmpPredicate::Int(IntPredicate::Eq),
                        other_op: Operand::Constant(truth),
                    });
                }
                let Operand::Value(cond_value) = condition else { return None };
                let llrm_mir::module::ValueDef::Instruction(cond_inst) = function.value(cond_value).def else {
                    return None;
                };
                let cond = function.instruction(cond_inst);
                if cond.opcode == Opcode::Cast(CastOp::Trunc)
                    && cond.flags.contains(Flags::NUW)
                    && cond.operands[0] == renamed
                {
                    let ty = function.operand_type(context, renamed)?;
                    return Some(PredicateConstraint {
                        predicate: CmpPredicate::Int(if true_edge { IntPredicate::Ne } else { IntPredicate::Eq }),
                        other_op: Operand::Constant(zero_of(context, ty)),
                    });
                }
                let (predicate, other_op) = match cond.opcode {
                    Opcode::ICmp(pred) => {
                        let (pred, other) = swapped_if_second(
                            cond.operands[0],
                            cond.operands[1],
                            renamed,
                            pred,
                            IntPredicate::swapped,
                        )?;
                        (CmpPredicate::Int(if true_edge { pred } else { pred.inverse() }), other)
                    }
                    Opcode::FCmp(pred) => {
                        let (pred, other) = swapped_if_second(
                            cond.operands[0],
                            cond.operands[1],
                            renamed,
                            pred,
                            FloatPredicate::swapped,
                        )?;
                        (CmpPredicate::Float(if true_edge { pred } else { pred.inverse() }), other)
                    }
                    _ => return None,
                };
                Some(PredicateConstraint { predicate, other_op })
            }
            PredicateKind::Switch { case_value, .. } => {
                if self.condition != Some(renamed) {
                    return None;
                }
                Some(PredicateConstraint {
                    predicate: CmpPredicate::Int(IntPredicate::Eq),
                    other_op: Operand::Constant(case_value),
                })
            }
        }
    }
}

/// The predicate and the other operand of a compare of `renamed`, as the
/// compare's operand 0 or, swapped, its operand 1.
fn swapped_if_second<P>(
    op0: Operand,
    op1: Operand,
    renamed: Operand,
    pred: P,
    swap: fn(P) -> P,
) -> Option<(P, Operand)> {
    if op0 == renamed {
        Some((pred, op1))
    } else if op1 == renamed {
        Some((swap(pred), op0))
    } else {
        None
    }
}

/// `Constant::getNullValue(Ty)`.
fn zero_of(
    context: &Context,
    ty: llrm_mir::types::TypeId,
) -> ConstantId {
    let kind = match context.types.get(ty) {
        llrm_mir::types::Type::Int(_) => ConstantKind::Int(0),
        llrm_mir::types::Type::Pointer(_) => ConstantKind::Null,
        _ => ConstantKind::Zero,
    };
    context.constant(Constant { ty, kind })
}

/// `PredicateInfo`: the copies made, and what each says.
#[derive(Debug, Default)]
pub struct PredicateInfo {
    /// `PredicateMap`.
    predicate_map: IdMap<InstId, PredicateBase>,
}

impl PredicateInfo {
    /// `getPredicateInfoFor`.
    pub fn predicate_info_for(
        &self,
        inst: InstId,
    ) -> Option<&PredicateBase> {
        self.predicate_map.get(&inst)
    }

    /// The copies, in no order.
    pub fn copies(&self) -> impl Iterator<Item = InstId> + '_ {
        self.predicate_map.keys()
    }

    /// `PredicateInfo(F, DT, AC, Allocator)`: makes the copies in `function`.
    pub fn new(
        context: &Context,
        globals: &[GlobalValue],
        metadata: &[MetadataNode],
        function: &mut Function,
        dt: &DomTree,
        ac: &AssumptionCache,
    ) -> Self {
        let mut builder = Builder {
            context,
            globals,
            metadata,
            dt,
            ac,
            infos: Vec::new(),
            value_info: IdMap::new(),
            ops_to_rename: Vec::new(),
            info: PredicateInfo::default(),
        };
        builder.build_predicate_info(function);
        builder.info
    }
}

/// `LocalNum`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LocalNum {
    /// Operations that must appear first in the block.
    First,
    /// Operations that are somewhere in the middle of the block, and are
    /// sorted on demand.
    Middle,
    /// Operations that must appear last in a block, like successor phi node
    /// uses.
    Last,
}

/// `ValueDFS`: associates global and local DFS info with defs (`pinfo` set)
/// and uses (`at` set), so they sort into a global domination ordering.
#[derive(Clone, Copy, Debug)]
struct ValueDfs {
    dfs_in: u32,
    dfs_out: u32,
    local_num: LocalNum,
    /// Only one of `at` and `pinfo` is set.
    at: Option<Use>,
    /// An index into `Builder::infos`.
    pinfo: Option<usize>,
}

/// `PredicateInfoBuilder::StackEntry`.
struct StackEntry {
    v: ValueDfs,
    def: Option<Operand>,
}

struct Builder<'a> {
    context: &'a Context,
    globals: &'a [GlobalValue],
    metadata: &'a [MetadataNode],
    dt: &'a DomTree,
    ac: &'a AssumptionCache,
    /// Every predicate made, by index.
    infos: Vec<PredicateBase>,
    /// `ValueInfos` by `ValueInfoNums`: the predicates of each value.
    value_info: IdMap<ValueId, Vec<usize>>,
    /// `OpsToRename`, in the order they were first named.
    ops_to_rename: Vec<ValueId>,
    info: PredicateInfo,
}

impl Builder<'_> {
    fn query<'f>(
        &self,
        function: &'f Function,
    ) -> Query<'f>
    where
        Self: 'f,
    {
        Query {
            context: self.context,
            globals: self.globals,
            metadata: self.metadata,
            function,
            layout: &DEFAULT_LAYOUT,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    /// `shouldRename`: only want real values, not constants. Additionally,
    /// operands with one use are only being used in the comparison, which
    /// means they will not be useful for us to consider for predicateinfo.
    fn should_rename(
        &self,
        function: &Function,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Value(value) if function.users(value).only().is_none()
        )
    }

    /// `addInfoFor`: adds `pb` to the infos of `op`, and marks `op` to be
    /// renamed.
    fn add_info_for(
        &mut self,
        op: Operand,
        pb: PredicateBase,
    ) {
        let Operand::Value(value) = op else { return };
        let at = self.infos.len();
        self.infos.push(pb);
        let list = self.value_info.get_or_insert_with(value, Vec::new);
        if list.is_empty() {
            self.ops_to_rename.push(value);
        }
        list.push(at);
    }

    /// `collectCmpOps` and the `m_NUWTrunc` arm: the values of a condition
    /// worth a copy.
    fn condition_values(
        &self,
        function: &Function,
        cond: Operand,
    ) -> Vec<Operand> {
        let mut values = vec![cond];
        if let Operand::Value(value) = cond
            && let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def
        {
            let instruction = function.instruction(inst);
            match instruction.opcode {
                Opcode::ICmp(_) | Opcode::FCmp(_) => {
                    let (op0, op1) = (instruction.operands[0], instruction.operands[1]);
                    if op0 != op1 {
                        values.push(op0);
                        values.push(op1);
                    }
                }
                Opcode::Cast(CastOp::Trunc) if instruction.flags.contains(Flags::NUW) => {
                    values.push(instruction.operands[0]);
                }
                _ => {}
            }
        }
        values
    }

    /// `processAssume`.
    fn process_assume(
        &mut self,
        function: &Function,
        assume: InstId,
    ) {
        let mut worklist = vec![function.instruction(assume).operands[0]];
        let mut visited: HashSet<Operand> = HashSet::default();
        while let Some(cond) = worklist.pop() {
            if !visited.insert(cond) {
                continue;
            }
            if visited.len() > MAX_CONDS_PER_BRANCH {
                break;
            }
            if let Some((op0, op1)) = logical_and(&self.query(function), cond) {
                worklist.push(op1);
                worklist.push(op0);
            }
            for v in self.condition_values(function, cond) {
                if self.should_rename(function, v) {
                    let pa = PredicateBase {
                        kind: PredicateKind::ConditionAssume { assume },
                        original_op: v,
                        renamed_op: None,
                        condition: Some(cond),
                    };
                    self.add_info_for(v, pa);
                }
            }
        }
    }

    /// `processBranch`.
    fn process_branch(
        &mut self,
        function: &Function,
        branch: InstId,
        branch_bb: BlockId,
    ) {
        let operands = function.instruction(branch).operands.clone();
        let (Operand::Block(first), Operand::Block(second)) = (operands[1], operands[2]) else {
            unreachable!("a conditional branch names two blocks")
        };
        for succ in [first, second] {
            let taken_edge = succ == first;
            // Don't try to insert on a self-edge. This is mainly because we
            // will eliminate during renaming anyway.
            if succ == branch_bb {
                continue;
            }
            let mut worklist = vec![operands[0]];
            let mut visited: HashSet<Operand> = HashSet::default();
            while let Some(cond) = worklist.pop() {
                if !visited.insert(cond) {
                    continue;
                }
                if visited.len() > MAX_CONDS_PER_BRANCH {
                    break;
                }
                let q = self.query(function);
                let joined = if taken_edge { logical_and(&q, cond) } else { logical_or(&q, cond) };
                if let Some((op0, op1)) = joined {
                    worklist.push(op1);
                    worklist.push(op0);
                }
                for v in self.condition_values(function, cond) {
                    if self.should_rename(function, v) {
                        let pb = PredicateBase {
                            kind: PredicateKind::Branch { from: branch_bb, to: succ, true_edge: taken_edge },
                            original_op: v,
                            renamed_op: None,
                            condition: Some(cond),
                        };
                        self.add_info_for(v, pb);
                    }
                }
            }
        }
    }

    /// `processSwitch`.
    fn process_switch(
        &mut self,
        function: &Function,
        switch: InstId,
        branch_bb: BlockId,
    ) {
        let operands = function.instruction(switch).operands.clone();
        let op = operands[0];
        if !matches!(
            op,
            Operand::Value(value) if function.users(value).only().is_none()
        ) {
            return;
        }
        // Remember how many outgoing edges there are to every successor.
        let mut switch_edges: IdMap<BlockId, u32> = IdMap::new();
        for operand in &operands {
            if let Operand::Block(target) = operand {
                *switch_edges.get_or_insert_with(*target, || 0) += 1;
            }
        }
        // Now propagate info for each case value.
        for pair in operands[2..].chunks(2) {
            let (Operand::Constant(case_value), Operand::Block(target)) = (pair[0], pair[1]) else {
                unreachable!("a case is a constant and a block")
            };
            if switch_edges.get(&target) == Some(&1) {
                let ps = PredicateBase {
                    kind: PredicateKind::Switch { from: branch_bb, to: target, case_value },
                    original_op: op,
                    renamed_op: None,
                    condition: Some(op),
                };
                self.add_info_for(op, ps);
            }
        }
    }

    /// `buildPredicateInfo`.
    fn build_predicate_info(
        &mut self,
        function: &mut Function,
    ) {
        self.dt.update_dfs_numbers();
        // Collect operands to rename from all conditional branch terminators,
        // as well as assume statements.
        for block in function.layout().to_vec() {
            if !self.dt.is_reachable_from_entry(block) {
                continue;
            }
            let Some(term) = function.terminator(block) else { continue };
            let instruction = function.instruction(term);
            match instruction.opcode {
                Opcode::Br if instruction.operands.len() == 3 => {
                    // Can't insert conditional information if they all go to
                    // the same place.
                    if instruction.operands[1] == instruction.operands[2] {
                        continue;
                    }
                    self.process_branch(function, term, block);
                }
                Opcode::Switch => self.process_switch(function, term, block),
                _ => {}
            }
        }
        for &assume in &self.ac.assumes {
            if function.is_erased(assume) {
                continue;
            }
            if let Some(parent) = function.parent(assume)
                && self.dt.is_reachable_from_entry(parent)
            {
                self.process_assume(function, assume);
            }
        }
        // Now rename all our operations.
        self.rename_uses(function);
    }

    /// `getBlockEdge(VD)`: for a phi use, or a non-materialized def, the edge
    /// it represents.
    fn block_edge(
        &self,
        function: &Function,
        vd: &ValueDfs,
    ) -> (BlockId, BlockId) {
        if let Some(at) = vd.at {
            let incoming = incoming_block(function, at).expect("a phi use");
            return (incoming, function.parent(at.user).expect("a placed phi"));
        }
        self.infos[vd.pinfo.expect("a def")].edge().expect("a predicate with an edge")
    }

    /// `ValueDFS_Compare::operator()` as an ordering.
    fn compare(
        &self,
        function: &Function,
        a: &ValueDfs,
        b: &ValueDfs,
    ) -> Ordering {
        // Order by block first.
        if a.dfs_in != b.dfs_in {
            return a.dfs_in.cmp(&b.dfs_in);
        }
        // Then order by first/middle/last.
        if a.local_num != b.local_num {
            return a.local_num.cmp(&b.local_num);
        }
        match a.local_num {
            // We want to put the def that will get used for a given set of
            // phi uses, before those phi uses. So we sort by edge, then by
            // def. Only phi nodes uses and defs can come last.
            LocalNum::Last => {
                let (_, a_dest) = self.block_edge(function, a);
                let (_, b_dest) = self.block_edge(function, b);
                let a_in = self.dt.span(a_dest).map_or(0, |span| span.0);
                let b_in = self.dt.span(b_dest).map_or(0, |span| span.0);
                (a_in, a.at.is_some()).cmp(&(b_in, b.at.is_some()))
            }
            // Use block-local ordering for instructions in the middle.
            LocalNum::Middle => {
                let (a_inst, b_inst) = (self.def_or_user(function, a), self.def_or_user(function, b));
                if function.comes_before(a_inst, b_inst) {
                    Ordering::Less
                } else if function.comes_before(b_inst, a_inst) {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
            // The order of PredicateInfo definitions at the start of the
            // block does not matter.
            LocalNum::First => Ordering::Equal,
        }
    }

    /// `getDefOrUser`: for the purpose of ordering, a def is pretended right
    /// after the assume, because that is where the info is inserted.
    fn def_or_user(
        &self,
        function: &Function,
        vd: &ValueDfs,
    ) -> InstId {
        if let Some(at) = vd.at {
            return at.user;
        }
        let PredicateKind::ConditionAssume { assume } = self.infos[vd.pinfo.expect("a def")].kind else {
            unreachable!("Middle of block should only occur for assumes")
        };
        function.next_in_block(assume).expect("an assume is followed by an instruction")
    }

    /// `stackIsInScope`.
    fn stack_is_in_scope(
        &self,
        function: &Function,
        stack: &[StackEntry],
        vd_use: &ValueDfs,
    ) -> bool {
        let top = &stack.last().expect("Should not be called with empty stack").v;
        // If it's a phi only use, make sure it's for this phi node edge, and
        // that the use is in a phi node. If it's anything else, and the top of
        // the stack is a LN_Last def, we need to pop the stack. We
        // deliberately sort phi uses next to the defs they must go with so
        // that we can know it's time to pop the stack when we hit the end of
        // the phi uses for a given def.
        if top.local_num == LocalNum::Last {
            let top_edge = self.infos[top.pinfo.expect("a def")].edge().expect("an edge");
            let Some(at) = vd_use.at else {
                // We should reserve adjacent LN_Last defs for the same phi
                // use.
                let other = self.infos[vd_use.pinfo.expect("a def")].edge();
                return vd_use.local_num == LocalNum::Last && Some(top_edge) == other;
            };
            if function.instruction(at.user).opcode != Opcode::Phi {
                return false;
            }
            // Check edge.
            if incoming_block(function, at) != Some(top_edge.0) {
                return false;
            }
            // Use dominates, which knows how to handle edge dominance.
            return dominates_edge_use(self.dt, function, BasicBlockEdge { start: top_edge.0, end: top_edge.1 }, at);
        }
        vd_use.dfs_in >= top.dfs_in && vd_use.dfs_out <= top.dfs_out
    }

    /// `convertUsesToDFSOrdered`: the uses of `op` with their global and local
    /// DFS info.
    fn convert_uses_to_dfs_ordered(
        &self,
        function: &Function,
        op: ValueId,
        ordered: &mut Vec<ValueDfs>,
    ) {
        let query = self.query(function);
        for at in function.users(op).iter().copied().collect::<Vec<Use>>() {
            // Lifetime intrinsics must work directly on alloca, do not
            // replace them with a predicated copy.
            if matches!(
                query.intrinsic(at.user),
                Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd)
            ) {
                continue;
            }
            // Put the phi node uses in the incoming block, last.
            let (block, local_num) = if function.instruction(at.user).opcode == Opcode::Phi {
                (incoming_block(function, at).expect("a phi use"), LocalNum::Last)
            } else {
                (function.parent(at.user).expect("a placed user"), LocalNum::Middle)
            };
            // It's possible our use is in an unreachable block. Skip it if so.
            let Some((dfs_in, dfs_out)) = self.dt.span(block) else { continue };
            ordered.push(ValueDfs { dfs_in, dfs_out, local_num, at: Some(at), pinfo: None });
        }
    }

    /// `materializeStack`: makes real all the operands currently on the stack
    /// by inserting them into the IR. Returns the last operation's value.
    fn materialize_stack(
        &mut self,
        function: &mut Function,
        counter: &mut u32,
        stack: &mut [StackEntry],
        orig_op: Operand,
    ) -> Operand {
        // Find the first thing we have to materialize.
        let start = stack.iter().rev().take_while(|entry| entry.def.is_none()).count();
        let begin = stack.len() - start;
        for at in begin..stack.len() {
            let op = if at == 0 { orig_op } else { stack[at - 1].def.expect("the one below has a def") };
            let info = stack[at].v.pinfo.expect("a def");
            self.infos[info].renamed_op =
                Some(if begin == 0 { orig_op } else { stack[begin - 1].def.expect("the one below has a def") });
            let ty = function.operand_type(self.context, op).expect("a value");
            // Use a no-op bitcast to represent ssa copy. For edge predicates,
            // we can just place the operand in the block before the
            // terminator. For assume, we have to place it right after the
            // assume to ensure we dominate all uses except assume itself.
            let (name, before) = match self.infos[info].kind {
                PredicateKind::Branch { from, .. } | PredicateKind::Switch { from, .. } => {
                    let name = match op {
                        Operand::Value(value) => function.value(value).name.clone().unwrap_or_default(),
                        _ => String::new(),
                    };
                    let n = *counter;
                    *counter += 1;
                    (Some(format!("{name}.{n}")), function.terminator(from).expect("a terminated block"))
                }
                PredicateKind::ConditionAssume { assume } => {
                    (None, function.next_in_block(assume).expect("an assume is followed by an instruction"))
                }
            };
            let copy = function.create_instruction(
                Opcode::Cast(CastOp::BitCast),
                ty,
                vec![op],
                Flags::default(),
                name.as_deref(),
            );
            function.insert(copy, Position::Before(before)).expect("a placed instruction");
            self.info.predicate_map.insert(copy, self.infos[info].clone());
            stack[at].def = Some(Operand::Value(function.instruction(copy).result.expect("a copy has a value")));
        }
        stack.last().expect("a nonempty stack").def.expect("materialized")
    }

    /// `renameUses`: instead of the standard SSA renaming algorithm, which is
    /// O(Number of instructions), and walks the entire dominator tree, walks
    /// only the defs + uses, sorted into the DFS ordering of the tree.
    fn rename_uses(
        &mut self,
        function: &mut Function,
    ) {
        // Compute liveness, and rename in O(uses) per Op.
        for op in self.ops_to_rename.clone() {
            let mut counter = 0_u32;
            let mut ordered: Vec<ValueDfs> = Vec::new();
            // Insert the possible copies into the def/use list. They will
            // become real copies if we find a real use for them, and never
            // created otherwise.
            for &possible in self.value_info.get(&op).expect("named when added") {
                let info = &self.infos[possible];
                match info.kind {
                    // The predicate info for assumes will be somewhere in the
                    // middle, it will get materialized right after the assume.
                    PredicateKind::ConditionAssume { assume } => {
                        let block = function.parent(assume).expect("a placed assume");
                        let Some((dfs_in, dfs_out)) = self.dt.span(block) else { continue };
                        ordered.push(ValueDfs {
                            dfs_in,
                            dfs_out,
                            local_num: LocalNum::Middle,
                            at: None,
                            pinfo: Some(possible),
                        });
                    }
                    PredicateKind::Branch { from, to, .. } | PredicateKind::Switch { from, to, .. } => {
                        // If we can only do phi uses, we treat it like it's in
                        // the branch block, and handle it specially. We know
                        // that it goes last, and only dominate phi uses.
                        // `getSinglePredecessor`: one edge, not one block.
                        let single_predecessor = function
                            .block_users(to)
                            .iter()
                            .filter(|one| function.instruction(one.user).opcode.is_terminator())
                            .count()
                            == 1;
                        if !single_predecessor {
                            if let Some((dfs_in, dfs_out)) = self.dt.span(from) {
                                ordered.push(ValueDfs {
                                    dfs_in,
                                    dfs_out,
                                    local_num: LocalNum::Last,
                                    at: None,
                                    pinfo: Some(possible),
                                });
                            }
                        } else if let Some((dfs_in, dfs_out)) = self.dt.span(to) {
                            // Otherwise, we are in the split block (even
                            // though we perform insertion in the branch
                            // block). Insert a possible copy at the split
                            // block and before the branch.
                            ordered.push(ValueDfs {
                                dfs_in,
                                dfs_out,
                                local_num: LocalNum::First,
                                at: None,
                                pinfo: Some(possible),
                            });
                        }
                    }
                }
            }
            self.convert_uses_to_dfs_ordered(function, op, &mut ordered);
            // Here we require a stable sort because we do not bother to try to
            // assign an order to the operands the uses represent. Thus, two
            // uses in the same instruction do not have a strict sort order
            // currently and will be considered equal.
            {
                let f: &Function = function;
                ordered.sort_by(|a, b| self.compare(f, a, b));
            }
            let mut rename_stack: Vec<StackEntry> = Vec::new();
            // For each use, sorted into dfs order, push values and replaces
            // uses with top of stack, which will represent the reaching def.
            for vd in ordered {
                // Sync to our current scope.
                while !rename_stack.is_empty() && !self.stack_is_in_scope(function, &rename_stack, &vd) {
                    rename_stack.pop();
                }
                if vd.pinfo.is_some() {
                    rename_stack.push(StackEntry { v: vd, def: None });
                    continue;
                }
                // If we get to this point, and the stack is empty we must have
                // a use with no renaming needed, just skip it.
                if rename_stack.is_empty() {
                    continue;
                }
                // If the possible copy dominates something, materialize our
                // stack up to this point. This ensures every comparison that
                // affects our operation ends up with predicateinfo.
                let top_def = rename_stack.last().expect("nonempty").def;
                let def = match top_def {
                    Some(def) => def,
                    None => self.materialize_stack(function, &mut counter, &mut rename_stack, Operand::Value(op)),
                };
                let at = vd.at.expect("a use");
                debug_assert!(
                    match def {
                        Operand::Value(value) => match function.value(value).def {
                            llrm_mir::module::ValueDef::Instruction(inst) =>
                                dominates_instruction_use(self.dt, function, inst, at),
                            _ => true,
                        },
                        _ => true,
                    },
                    "Predicateinfo def should have dominated this use"
                );
                function.set_operand(at.user, at.index as usize, def);
            }
        }
    }
}

/// `PHINode::getIncomingBlock(Use)`.
fn incoming_block(
    function: &Function,
    at: Use,
) -> Option<BlockId> {
    let instruction = function.instruction(at.user);
    if instruction.opcode != Opcode::Phi {
        return None;
    }
    match instruction.operands.get(at.index as usize + 1) {
        Some(Operand::Block(block)) => Some(*block),
        _ => None,
    }
}

static DEFAULT_LAYOUT: std::sync::LazyLock<llrm_mir::datalayout::DataLayout> =
    std::sync::LazyLock::new(llrm_mir::datalayout::DataLayout::default);
