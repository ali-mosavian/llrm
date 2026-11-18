// SPDX-License-Identifier: GPL-3.0-or-later
//! Backward jump threading: a
//! branch whose outcome one path into its block decides is threaded for that
//! path, though the block's other paths leave it unknown.
//!
//! GCC's
//! [`tree-ssa-threadbackward.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/tree-ssa-threadbackward.cc)
//! walks back from a conditional branch along a path until the branch's inputs
//! are pinned down, then copies the path's blocks so that path jumps straight
//! to the arm. LLVM's `JumpThreading` asks LVI about each edge into the
//! branch's block, which merges all paths into the predecessor, so a
//! predecessor that is a join of a known and an unknown path is never threaded.
//!
//! Here a path is a predecessor `q` of a join `p` that enters `bb`. The facts
//! at `q` (the branches that dominate it, and its own edge to `p`) are read as
//! ranges of the values `bb`'s compare reads; when they decide the compare and
//! the facts at `p` do not, `p` and `bb` are copied for the edge `q -> p`, the
//! copy of `bb` jumps to the arm, and SSA form is restored.
//!
//! ```text
//!   q: ...  (knows x > 0)             q: ...
//!   p: join of q and r                p': copy of p        p: join of r alone
//!   bb: br (x > 0), A, B       ==>    bb': copy of bb, jumps to A
//! ```
//!
//! Relations (`Tuning::threads_by_relation`, DEVIATE BACKTHREAD-REL): the
//! compares of one pair of names that hold on a path decide the others of that
//! pair, as GCC's relation oracle (`value-relation.cc`, read by
//! `gimple-range-path.cc` in `pass_thread_jumps_full`) does: `a < b` taken
//! makes `a <= b` taken and `a > b` not (`Orders`; signed and unsigned orders
//! apart). A join that holds the branch itself is threaded for such a path
//! (`join_decided`), which collapses a ladder of compares of one pair. The pass
//! also runs before the first SimplifyCFG that makes selects of the branches
//! (`backthread@1243`).
//!
//! DEVIATE (no LLVM pass; GCC `tree-ssa-threadbackward.cc`): paths are `q`,
//! `p`, `bb` only (GCC's `max-fsm-thread-length` is 10); the facts are the
//! ranges of branch edges that dominate `q` (GCC asks ranger); the path may
//! not copy more than `max-jump-thread-duplication-stmts` (15) instructions;
//! a function with `optsize` or `minsize` is left alone.

use llrm_analysis::edges::{self, phis, single_predecessor, starts_with_landing_pad};
use llrm_analysis::occurrence::{arms, from_arms};
use llrm_analysis::ssaupdaterbulk::SsaUpdaterBulk;
use llrm_mir::constantrange::{ConstantRange, PreferredRangeType};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use, ValueDef};
use llrm_mir::opcode::{Attribute, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::types::Type;

use crate::pathbits::PathBits;

/// GCC `max-jump-thread-duplication-stmts`: the statements a threaded path
/// may copy (`max-fsm-thread-path-insns`, 100, is for a `switch`).
const MAX_PATH_INSNS: usize = 15;
/// Paths threaded in one function: a cap on the code a run can add.
pub const MAX_THREADS: usize = 16;

pub struct BackwardThreading {
    pub enabled: bool,
    /// Paths threaded per function at most.
    pub max_threads: usize,
    /// The compares of one pair of values decide each other (`a < b` taken: `a
    /// <= b` taken, `a > b` not), and a join that holds its own branch is
    /// threaded for the path that decides it.
    pub relations: bool,
    /// The slot before the first SimplifyCFG that speculates
    /// (`backthread@1243`): the pass runs only where the target threads by
    /// relation, since the branches it reads are selects by the slot after.
    pub early: bool,
}

impl BackwardThreading {
    pub fn from_env() -> Self {
        Self {
            enabled: !llrm_support::env_flag!("LLRM_NO_BACKTHREAD"),
            max_threads: MAX_THREADS,
            relations: false,
            early: false,
        }
    }

    pub fn when(
        self,
        on: bool,
    ) -> Self {
        Self { enabled: self.enabled && on, ..self }
    }

    pub fn with_relations(
        self,
        on: bool,
    ) -> Self {
        Self { relations: on, ..self }
    }

    pub fn early(self) -> Self {
        Self { early: true, ..self }
    }
}

impl FunctionPass for BackwardThreading {
    fn name(&self) -> &'static str {
        "backthread"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let relations = !llrm_support::env_flag!("LLRM_NO_BACKTHREAD_REL")
            && (self.relations || analyses.outer().target().tti().is_some_and(|tti| tti.threads_by_relation()));
        if !self.enabled
            || (self.early && !relations)
            || unit.function.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
                )
            })
        {
            return PreservedAnalyses::all();
        }
        let mut changed = false;
        for _ in 0..self.max_threads {
            let dt = DomTree::of(unit.function);
            let loops = LoopInfo::analyze(unit.function, || DomTree::of(unit.function));
            if let Some(path) = find(unit.context, unit.function, &dt, &loops, relations) {
                thread(unit.context, unit.function, &path);
                changed = true;
                continue;
            }
            // A select that sends the next trip down one arm of a branch is
            // a diamond GCC would still have: unfold it where that threads.
            let mut threaded = false;
            for select in select_candidates(unit.context, unit.function) {
                let mut trial = unit.function.clone();
                unfold_select(&mut trial, select);
                let dt = DomTree::of(&trial);
                let loops = LoopInfo::analyze(&trial, || DomTree::of(&trial));
                if let Some(path) = find(unit.context, &trial, &dt, &loops, relations) {
                    *unit.function = trial;
                    thread(unit.context, unit.function, &path);
                    threaded = true;
                    break;
                }
            }
            if !threaded {
                break;
            }
            changed = true;
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// One path to thread: `q -> p -> bb`, `bb`'s branch taking `arm`.
#[derive(Debug)]
struct Path {
    q: BlockId,
    p: BlockId,
    bb: BlockId,
    arm: BlockId,
}

/// The compare a block branches on.
struct Branch {
    pred: IntPredicate,
    lhs: Operand,
    rhs: Operand,
    if_true: BlockId,
    if_false: BlockId,
}

/// `block`'s `br (icmp pred lhs, rhs), if_true, if_false` with two arms.
fn branch_of(
    function: &Function,
    block: BlockId,
) -> Option<Branch> {
    let term = function.terminator(block)?;
    let &[Operand::Value(cond), Operand::Block(if_true), Operand::Block(if_false)] =
        &function.instruction(term).operands[..]
    else {
        return None;
    };
    if if_true == if_false || function.instruction(term).opcode != Opcode::Br {
        return None;
    }
    let ValueDef::Instruction(icmp) = function.value(cond).def else { return None };
    let Opcode::ICmp(pred) = function.instruction(icmp).opcode else { return None };
    let ops = &function.instruction(icmp).operands;
    Some(Branch { pred, lhs: ops[0], rhs: ops[1], if_true, if_false })
}

pub(crate) fn width_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    match context.types.get(function.operand_type(context, operand)?) {
        Type::Int(bits) => Some(*bits),
        _ => None,
    }
}

pub(crate) fn constant_of(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(value) => Some(value),
        _ => None,
    }
}

/// Whether `value` is the same instance wherever a fact about it was learned
/// at `at` and is read below it: a constant or argument, or an instruction in
/// a block other than `at` that dominates it.
fn stable(
    function: &Function,
    dt: &DomTree,
    value: Operand,
    at: BlockId,
) -> bool {
    match value {
        Operand::Value(id) => match function.value(id).def {
            ValueDef::Instruction(inst) => {
                let block = function.parent(inst).expect("a placed instruction");
                block != at && dt.dominates(block, at)
            }
            _ => true,
        },
        _ => true,
    }
}

/// Whether `value` is an instruction of `block`.
fn defined_in(
    function: &Function,
    value: Operand,
    block: BlockId,
) -> bool {
    matches!(
        value,
        Operand::Value(id) if matches!(function.value(id).def, ValueDef::Instruction(inst) if function.parent(inst) == Some(block))
    )
}

/// `range`, narrowed by the edge `from -> to` when `from` branches on a
/// compare of `value` against a constant.
fn narrowed(
    context: &Context,
    function: &Function,
    dt: &DomTree,
    value: Operand,
    range: ConstantRange,
    from: BlockId,
    to: BlockId,
) -> ConstantRange {
    let Some(branch) = branch_of(function, from) else { return range };
    if !stable(function, dt, value, from) {
        return range;
    }
    let taken = branch.if_true == to;
    let pred = if taken { branch.pred } else { branch.pred.inverse() };
    let (pred, constant) = if branch.lhs == value {
        (pred, constant_of(context, branch.rhs))
    } else if branch.rhs == value {
        (pred.swapped(), constant_of(context, branch.lhs))
    } else {
        return range;
    };
    let Some(constant) = constant else { return range };
    let width = range.width;
    let region = ConstantRange::make_allowed_icmp_region(pred, &ConstantRange::single(width, constant));
    range.intersect_with(&region, PreferredRangeType::Smallest)
}

/// The range of `value` at the end of `block`, from the branches whose edge
/// dominates it; `edge` adds the edge `from -> to` the path enters through.
fn range_at(
    context: &Context,
    function: &Function,
    dt: &DomTree,
    value: Operand,
    block: BlockId,
    edge: Option<(BlockId, BlockId)>,
) -> Option<ConstantRange> {
    let width = width_of(context, function, value)?;
    if let Some(constant) = constant_of(context, value) {
        return Some(ConstantRange::single(width, constant));
    }
    let mut range = ConstantRange::full(width);
    let mut at = Some(block);
    while let Some(dominator) = at {
        if let Some(pred) = single_predecessor(function, dominator) {
            range = narrowed(context, function, dt, value, range, pred, dominator);
        }
        at = dt.immediate_dominator(dominator);
    }
    if let Some((from, to)) = edge {
        range = narrowed(context, function, dt, value, range, from, to);
    }
    Some(range)
}

/// The orders two values can stand in: `x < y`, `x == y`, `x > y`, as a set.
/// GCC's `relation_kind` (`value-relation.cc`) is the same union of `VREL_LT`,
/// `VREL_EQ` and `VREL_GT`. One pair is ordered twice, by the signed compares
/// and by the unsigned ones (`a < b` unsigned says nothing of the signed
/// order).
#[derive(Clone, Copy)]
struct Orders {
    signed: u8,
    unsigned: u8,
}

const LESS: u8 = 1;
const EQUAL: u8 = 2;
const GREATER: u8 = 4;

/// Which order `pred` reads (None: both), and the orders where it holds.
fn reads(pred: IntPredicate) -> (Option<bool>, u8) {
    match pred {
        IntPredicate::Eq => (None, EQUAL),
        IntPredicate::Ne => (None, LESS | GREATER),
        IntPredicate::Slt => (Some(true), LESS),
        IntPredicate::Sle => (Some(true), LESS | EQUAL),
        IntPredicate::Sgt => (Some(true), GREATER),
        IntPredicate::Sge => (Some(true), GREATER | EQUAL),
        IntPredicate::Ult => (Some(false), LESS),
        IntPredicate::Ule => (Some(false), LESS | EQUAL),
        IntPredicate::Ugt => (Some(false), GREATER),
        IntPredicate::Uge => (Some(false), GREATER | EQUAL),
    }
}

impl Orders {
    const ANY: Self = Self { signed: 7, unsigned: 7 };

    /// What is left of the orders where `pred` holds.
    fn holding(
        self,
        pred: IntPredicate,
    ) -> Self {
        let (signed, set) = reads(pred);
        Self {
            signed: if signed != Some(false) { self.signed & set } else { self.signed },
            unsigned: if signed != Some(true) { self.unsigned & set } else { self.unsigned },
        }
    }

    /// Whether `pred` holds in every order left, or in none; None on a pair the
    /// facts contradict (a path no execution takes).
    fn decides(
        self,
        pred: IntPredicate,
    ) -> Option<bool> {
        if self.signed == 0 || self.unsigned == 0 {
            return None;
        }
        let (signed, set) = reads(pred);
        let groups = match signed {
            Some(true) => [Some(self.signed), None],
            Some(false) => [Some(self.unsigned), None],
            None => [Some(self.signed), Some(self.unsigned)],
        };
        groups
            .into_iter()
            .flatten()
            .find_map(
                |left| if left & !set == 0 {
                    Some(true)
                } else if left & set == 0 {
                    Some(false)
                } else {
                    None
                },
            )
    }
}

/// The compares, as taken, of the edges that dominate `block` and of `edge`:
/// what is known of each pair they compare.
fn compares_at(
    function: &Function,
    dt: &DomTree,
    block: BlockId,
    edge: Option<(BlockId, BlockId)>,
) -> Vec<(IntPredicate, Operand, Operand)> {
    let mut facts = Vec::new();
    let mut learn = |from: BlockId, to: BlockId| {
        let Some(branch) = branch_of(function, from) else { return };
        if !stable(function, dt, branch.lhs, from) || !stable(function, dt, branch.rhs, from) {
            return;
        }
        let pred = if branch.if_true == to { branch.pred } else { branch.pred.inverse() };
        facts.push((pred, branch.lhs, branch.rhs));
    };
    let mut at = Some(block);
    while let Some(dominator) = at {
        if let Some(pred) = single_predecessor(function, dominator) {
            learn(pred, dominator);
        }
        at = dt.immediate_dominator(dominator);
    }
    if let Some((from, to)) = edge {
        learn(from, to);
    }
    facts
}

/// The arm `branch` takes when the compares of the same pair that hold at
/// `block` (and `edge`) decide it.
fn decided_by_relation(
    function: &Function,
    dt: &DomTree,
    branch: &Branch,
    block: BlockId,
    edge: Option<(BlockId, BlockId)>,
) -> Option<BlockId> {
    // GCC's relations are between two names (`value-relation.cc`); a value
    // against a constant is a range's.
    if matches!(branch.lhs, Operand::Constant(_)) || matches!(branch.rhs, Operand::Constant(_)) {
        return None;
    }
    let mut known = Orders::ANY;
    let mut learned = false;
    for (pred, lhs, rhs) in compares_at(function, dt, block, edge) {
        let pred = if (lhs, rhs) == (branch.lhs, branch.rhs) {
            pred
        } else if (lhs, rhs) == (branch.rhs, branch.lhs) {
            pred.swapped()
        } else {
            continue;
        };
        known = known.holding(pred);
        learned = true;
    }
    if !learned {
        return None;
    }
    known.decides(branch.pred).map(|taken| if taken { branch.if_true } else { branch.if_false })
}

/// The arm `branch` takes when the facts at `block` (and `edge`) decide it.
fn decided(
    context: &Context,
    function: &Function,
    dt: &DomTree,
    branch: &Branch,
    block: BlockId,
    edge: Option<(BlockId, BlockId)>,
    relations: bool,
) -> Option<BlockId> {
    let by_range = (|| {
        let lhs = range_at(context, function, dt, branch.lhs, block, edge)?;
        let rhs = range_at(context, function, dt, branch.rhs, block, edge)?;
        if lhs.icmp(branch.pred, &rhs) {
            Some(branch.if_true)
        } else if lhs.icmp(branch.pred.inverse(), &rhs) {
            Some(branch.if_false)
        } else {
            None
        }
    })();
    by_range.or_else(|| if relations { decided_by_relation(function, dt, branch, block, edge) } else { None })
}

/// The arm `branch` takes on the path `q -> p -> bb` when the bits of its
/// inputs along it decide it: `q` is reached by one edge of a branch whose
/// compare is the assumption.
fn decided_by_bits(
    context: &Context,
    function: &Function,
    branch: &Branch,
    q: BlockId,
    p: BlockId,
    bb: BlockId,
) -> Option<BlockId> {
    let from = single_predecessor(function, q)?;
    let edge = branch_cond(function, from, q)?;
    let bits = PathBits::new(context, function, p, bb, Some(q), Some(edge));
    let yes = bits.decides(branch.pred, branch.lhs, branch.rhs)?;
    Some(if yes { branch.if_true } else { branch.if_false })
}

/// The condition of `from`'s branch and whether it is true on the edge to `to`.
fn branch_cond(
    function: &Function,
    from: BlockId,
    to: BlockId,
) -> Option<(Operand, bool)> {
    let term = function.terminator(from)?;
    let &[cond @ Operand::Value(_), Operand::Block(if_true), Operand::Block(if_false)] =
        &function.instruction(term).operands[..]
    else {
        return None;
    };
    if function.instruction(term).opcode != Opcode::Br || if_true == if_false {
        return None;
    }
    Some((cond, if_true == to))
}

/// The selects of a loop block that jumps to itself whose condition is a
/// compare the next trip decides: a trip that took one arm knows what the
/// compare says on the next. Unfolded, the block ends in that compare's
/// branch, the one GCC threads.
fn select_candidates(
    context: &Context,
    function: &Function,
) -> Vec<InstId> {
    let mut found = Vec::new();
    for &block in function.layout() {
        if !function.successors(block).contains(&block) || starts_with_landing_pad(function, block) {
            continue;
        }
        for inst in function.iter_block(block) {
            let Some((cond, _, _)) = crate::pathbits::select_parts(function, inst) else { continue };
            let Operand::Value(id) = cond else { continue };
            let ValueDef::Instruction(icmp) = function.value(id).def else { continue };
            let Opcode::ICmp(pred) = function.instruction(icmp).opcode else { continue };
            if function.parent(icmp) != Some(block) {
                continue;
            }
            let ops = &function.instruction(icmp).operands;
            let decides = |holds: bool| {
                PathBits::new(context, function, block, block, None, Some((cond, holds)))
                    .decides(pred, ops[0], ops[1])
                    .is_some()
            };
            if decides(true) || decides(false) {
                found.push(inst);
            }
        }
    }
    found
}

/// `select` as a diamond: its block ends in a branch on the condition to one
/// empty block per arm, and the rest of the block starts with a phi of the arms.
fn unfold_select(
    function: &mut Function,
    select: InstId,
) {
    let instruction = function.instruction(select);
    let (cond, yes, no) = (instruction.operands[0], instruction.operands[1], instruction.operands[2]);
    let ty = instruction.ty;
    let result = instruction.result.expect("a select's value");
    let name = function.value(result).name.clone();
    let block = function.parent(select).expect("a placed select");
    let void = function.instruction(function.terminator(block).expect("a terminated block")).ty;
    let tail = edges::split_block_at(function, block, select);
    let side = |function: &mut Function, after: BlockId, label: &str| {
        let new = function.create_block(Some(label));
        function.insert_block(new, Some(after)).expect("a new block");
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(tail)], Flags::default(), None);
        function.insert(jump, Position::End(new)).expect("a new block");
        new
    };
    let on_true = side(function, block, "select.true");
    let on_false = side(function, on_true, "select.false");
    let jump = function.terminator(block).expect("the split's jump");
    function.erase(jump).expect("a jump nothing reads");
    let branch = function.create_instruction(
        Opcode::Br,
        void,
        vec![cond, Operand::Block(on_true), Operand::Block(on_false)],
        Flags::default(),
        None,
    );
    function.insert(branch, Position::End(block)).expect("a placed block");
    let phi = function.create_instruction(
        Opcode::Phi,
        ty,
        from_arms(&[(yes, on_true), (no, on_false)]),
        Flags::default(),
        name.as_deref(),
    );
    function.insert(phi, Position::Before(select)).expect("a placed select");
    let phi_value = function.instruction(phi).result.expect("a phi's value");
    function.replace_all_uses_with(result, Operand::Value(phi_value));
    function.erase(select).expect("a select nothing reads");
}

/// Instructions that a copy of the block cannot take: a `noduplicate` or
/// `convergent` call, or an `alloca`.
fn copyable(
    function: &Function,
    block: BlockId,
) -> Option<usize> {
    let mut size = 0;
    for inst in function.iter_block(block) {
        let instruction = function.instruction(inst);
        if let Opcode::Call(call) = &instruction.opcode
            && call.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "noduplicate" || flag == "convergent"
                )
            })
        {
            return None;
        }
        if matches!(instruction.opcode, Opcode::Alloca { .. }) {
            return None;
        }
        if instruction.opcode != Opcode::Phi {
            size += 1;
        }
    }
    Some(size)
}

/// The first path the facts at a predecessor decide and the facts at its
/// join do not.
fn find(
    context: &Context,
    function: &Function,
    dt: &DomTree,
    loops: &LoopInfo<BlockId>,
    relations: bool,
) -> Option<Path> {
    for &bb in function.layout() {
        let Some(branch) = branch_of(function, bb) else { continue };
        let inputs_stable =
            |at: BlockId| stable(function, dt, branch.lhs, at) && stable(function, dt, branch.rhs, at) && at != bb;
        // Facts about the compare's inputs (ranges, relations) must read the
        // same wherever they hold: a value defined in `bb` itself (a header
        // phi) is the next trip's when the path comes back to `bb`. The bits
        // of the path evaluate `bb`'s own instructions on the path instead.
        let outside = !defined_in(function, branch.lhs, bb) && !defined_in(function, branch.rhs, bb);
        let facts_stable = |at: BlockId| outside && inputs_stable(at);
        for p in function.predecessors(bb) {
            if p == bb || !inputs_stable(p) || starts_with_landing_pad(function, p) {
                continue;
            }
            let join_preds = function.predecessors(p);
            if join_preds.len() < 2
                || join_preds.contains(&p)
                || function.edge_predecessors(bb).iter().filter(|&&b| b == p).count() != 1
            {
                continue;
            }
            // What every path into the join knows leaves nothing to thread.
            if outside && decided(context, function, dt, &branch, p, None, relations).is_some() {
                continue;
            }
            for &q in &join_preds {
                if q == bb || function.edge_predecessors(p).iter().filter(|&&b| b == q).count() != 1 {
                    continue;
                }
                let Some(arm) = outside
                    .then(|| decided(context, function, dt, &branch, q, Some((q, p)), relations))
                    .flatten()
                    .or_else(|| decided_by_bits(context, function, &branch, q, p, bb))
                else {
                    continue;
                };
                if arm == bb || arm == p {
                    continue;
                }
                let (Some(p_size), Some(bb_size)) = (copyable(function, p), copyable(function, bb)) else { continue };
                if p_size + bb_size > MAX_PATH_INSNS {
                    continue;
                }
                // Entering a loop's body past its header would split the loop.
                if let Some(l) = loops.get_loop_for(arm)
                    && !(l.contains(q) && l.contains(p) && l.contains(bb))
                {
                    continue;
                }
                return Some(Path { q, p, bb, arm });
            }
        }
        if relations && let Some(path) = join_decided(context, function, dt, loops, bb, &branch, &facts_stable) {
            return Some(path);
        }
    }
    None
}

/// A path `q -> bb` where `bb` is the join and holds the branch itself (`p ==
/// bb`): the compare ladder of one pair (`if (a < b) r += 1; if (a <= b) r +=
/// 2; ...`), whose every rung joins the arm before it, decided by what the
/// path's compares of the same pair imply.
fn join_decided(
    context: &Context,
    function: &Function,
    dt: &DomTree,
    loops: &LoopInfo<BlockId>,
    bb: BlockId,
    branch: &Branch,
    inputs_stable: &dyn Fn(BlockId) -> bool,
) -> Option<Path> {
    let preds = function.predecessors(bb);
    if preds.len() < 2 || preds.contains(&bb) || starts_with_landing_pad(function, bb) {
        return None;
    }
    // What every path into the join knows leaves nothing to thread.
    if decided(context, function, dt, branch, bb, None, true).is_some() {
        return None;
    }
    let size = copyable(function, bb)?;
    if size > MAX_PATH_INSNS {
        return None;
    }
    for &q in &preds {
        if q == bb || !inputs_stable(q) || function.edge_predecessors(bb).iter().filter(|&&b| b == q).count() != 1 {
            continue;
        }
        // Only what a relation decides: the range facts of a path into a join
        // are the p-shape's. This shape threaded seven latch tests of
        // `known_loads_walks` -O2 on range facts alone (+9.6% executed
        // instructions, a counted cell lost).
        let Some(arm) = decided_by_relation(function, dt, branch, q, Some((q, bb))) else { continue };
        if arm == bb {
            continue;
        }
        if let Some(l) = loops.get_loop_for(arm)
            && !(l.contains(q) && l.contains(bb))
        {
            continue;
        }
        return Some(Path { q, p: bb, bb, arm });
    }
    None
}

/// `block`'s instructions copied into `copy` before `at` with `map` applied.
/// Each phi of the block becomes a phi of one input in `copy`: the input
/// `block`'s phi has from `from`, read from `pred`, the one block that enters
/// `copy`. It stays a def, so that SSA repair renames the input like any other
/// use: a value the original phi names is not the copy's once the path loops.
fn copy_into(
    function: &mut Function,
    block: BlockId,
    copy: BlockId,
    pred: BlockId,
    from: BlockId,
    at: InstId,
    map: &mut ValueMap,
) {
    let mut first_phi: Option<InstId> = None;
    for inst in function.iter_block(block).collect::<Vec<_>>() {
        let opcode = function.instruction(inst).opcode.clone();
        let result = function.instruction(inst).result;
        if opcode == Opcode::Phi {
            let (incoming, _) = arms(function, inst)
                .into_iter()
                .find(|&(_, from_pred)| from_pred == from)
                .expect("an input from the new edge");
            let incoming = match incoming {
                Operand::Value(id) => map.values.get(&id).copied().unwrap_or(incoming),
                other => other,
            };
            let ty = function.instruction(inst).ty;
            let phi = function.create_instruction(
                Opcode::Phi,
                ty,
                vec![incoming, Operand::Block(pred)],
                Flags::default(),
                None,
            );
            match first_phi {
                None => {
                    let first = function.iter_block(copy).next().expect("a jump");
                    function.insert(phi, Position::Before(first)).expect("a placed block");
                }
                Some(last) => function.insert(phi, Position::After(last)).expect("a placed phi"),
            }
            first_phi = Some(phi);
            let new = function.instruction(phi).result.expect("a phi's value");
            if let Some(name) = result.and_then(|old| function.value(old).name.clone()) {
                function.set_name(new, &format!("{name}.bt"));
            }
            map.values.insert(result.expect("a phi's value"), Operand::Value(new));
            continue;
        }
        if opcode.is_terminator() {
            continue;
        }
        let duplicate = function.clone_instruction(inst);
        function.insert(duplicate, Position::Before(at)).expect("a placed jump");
        function.remap_instruction(duplicate, map);
        if let (Some(old), Some(new)) = (result, function.instruction(duplicate).result) {
            if let Some(name) = function.value(old).name.clone() {
                function.set_name(new, &format!("{name}.bt"));
            }
            map.values.insert(old, Operand::Value(new));
        }
    }
}

/// Each phi of `block` gets, for the edges `new_pred` has to it, the inputs
/// `existing_pred` has for its own: the copy of a switch keeps every edge of
/// the original, and a phi has an input for each (`predecessor_added` adds
/// one).
fn edges_copied(
    function: &mut Function,
    block: BlockId,
    new_pred: BlockId,
    existing_pred: BlockId,
) {
    for phi in phis(function, block) {
        let mut incoming = arms(function, phi);
        let copied: Vec<_> =
            incoming.iter().filter(|&&(_, from)| from == existing_pred).map(|&(value, _)| (value, new_pred)).collect();
        incoming.extend(copied);
        function.set_operands(phi, from_arms(&incoming));
    }
}

/// Each phi of `block` that has an input from `new_pred` reads it through
/// `map` (the input was copied from the original predecessor).
fn remap_input(
    function: &mut Function,
    block: BlockId,
    new_pred: BlockId,
    map: &ValueMap,
) {
    for phi in phis(function, block) {
        let mut incoming = arms(function, phi);
        for (value, from) in &mut incoming {
            if *from == new_pred
                && let Operand::Value(id) = *value
                && let Some(&mapped) = map.values.get(&id)
            {
                *value = mapped;
            }
        }
        function.set_operands(phi, from_arms(&incoming));
    }
}

/// Copy `p` and `bb` for the edge `q -> p`, the copy of `bb` jumping to the
/// arm, and restore SSA form.
fn thread(
    context: &mut Context,
    function: &mut Function,
    path: &Path,
) {
    let Path { q, p, bb, arm } = *path;
    let mut map = ValueMap::default();

    // The copy of `p`: a block of its own on the edge `q -> p`. Where `p` is
    // `bb` (the join holds the branch) the copy of `bb` below is the only
    // one.
    let e = if p == bb {
        q
    } else {
        let e = edges::split_block_predecessors(function, p, &[q]);
        let e_jump = function.terminator(e).expect("a jump");
        copy_into(function, p, e, q, e, e_jump, &mut map);
        let p_term = function.terminator(p).expect("a terminated block");
        let e_term = function.clone_instruction(p_term);
        function.insert(e_term, Position::Before(e_jump)).expect("a placed jump");
        function.remap_instruction(e_term, &map);
        function.predecessor_removed(context, p, e, true);
        function.erase(e_jump).expect("a jump nothing uses");
        for succ in function.successors(e) {
            edges_copied(function, succ, e, p);
            remap_input(function, succ, e, &map);
        }
        e
    };

    // The copy of `bb`: a block on the edge `e -> bb` that jumps to the arm.
    let f = edges::split_block_predecessors(function, bb, &[e]);
    let f_jump = function.terminator(f).expect("a jump");
    copy_into(function, bb, f, e, f, f_jump, &mut map);
    edges::predecessor_added(function, arm, f, bb);
    remap_input(function, arm, f, &map);
    function.predecessor_removed(context, bb, f, true);
    function.set_operand(f_jump, 0, Operand::Block(arm));

    // Uses of the values `p` and `bb` define, below the copies, are of either.
    let mut ssa = SsaUpdaterBulk::new();
    let copies: &[(BlockId, BlockId)] = if p == bb { &[(bb, f)] } else { &[(p, e), (bb, f)] };
    for &(block, copy_block) in copies {
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            let Some(value) = function.instruction(inst).result else { continue };
            let Some(&copy) = map.values.get(&value) else { continue };
            let mut uses: Vec<Use> = function
                .users(value)
                .iter()
                .filter(|one| {
                    let user_block = function.parent(one.user);
                    let user = function.instruction(one.user);
                    if user.opcode == Opcode::Phi {
                        user.operands[one.index as usize + 1] != Operand::Block(block)
                    } else {
                        user_block != Some(block)
                    }
                })
                .copied()
                .collect();
            if uses.is_empty() {
                continue;
            }
            let name = function.value(value).name.clone();
            let var = ssa.add_variable(name.as_deref(), function.instruction(inst).ty);
            ssa.add_available_value(var, block, Operand::Value(value));
            ssa.add_available_value(var, copy_block, copy);
            while let Some(one) = uses.pop() {
                ssa.add_use(var, one);
            }
        }
    }
    let dt = DomTree::of(function);
    ssa.rewrite_all_uses(context, function, &dt, None);
}
