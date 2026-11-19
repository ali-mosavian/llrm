//! An integer constant too costly to be an immediate is made once, where
//! every use can read it, as LLVM's `ConstantHoisting`.
//!
//! # How it works
//! A translation of LLVM
//! [`ConstantHoisting.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp)
//! at d1106deb71cc. The integer constants the target prices above one
//! instruction (`getIntImmCostInst`, or `getIntImmCostIntrin` for an
//! intrinsic) are collected with their uses, a constant read through a cast
//! counting as read by the cast's user. They are sorted by type and value and
//! cut into runs where each is an `add` immediate (and an addressing-mode
//! offset, for a memory access) from the run's smallest. The run's base is the
//! constant of the greatest cumulative cost (at `optsize`, the one that leaves
//! most uses in range of a cheap immediate). A base with more than one use is
//! made as `bitcast iN C to iN` in the blocks of least total frequency that
//! dominate the uses, and each use is rebased: it reads the base, or
//! `add base, offset` made before it.
//!
//! # Example
//! ```text
//!  before                          after
//!  b0: store i64 K, ptr %p         b0: %const = bitcast i64 K to i64
//!  b1: store i64 K, ptr %q             store i64 %const, ptr %p
//!                                  b1: store i64 %const, ptr %q
//! ```
//!
//! DEVIATE 1 (exit 2026-12-31, [`consthoist-gep`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L84)):
//! constant `gep` expressions are not hoisted. The pin's option is off by
//! default and a MIR operand is never a constant expression.
//!
//! DEVIATE 2 (exit 2026-12-31, [`consthoist-with-block-frequency`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L78)):
//! block frequencies are always used; the option is on by default, so the
//! dominator-only insertion point search is not translated.
//!
//! DEVIATE 3 (exit 2026-12-31, [`optsize`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L937)):
//! `shouldOptimizeForSize` reads the function's `optsize` or `minsize`; there
//! is no profile summary.
//!
//! # Parity slot
//! None yet: the pin runs it in `addIRPasses`
//! ([TargetPassConfig.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L895)),
//! before CodeGenPrepare, which the pipeline does not wire.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.

use llrm_analysis::blockfreq::BlockFrequencies;
use llrm_mir::context::{ConstantId, ConstantKind, Context, mask, signed};
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::hash::HashMap;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::tti::{Cost, ImmUser, Tti};
use llrm_mir::types::TypeId;

use crate::twoentryphi::Speculation;

/// `MinNumOfDependentToRebase`
/// ([L66](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L89)).
const MIN_NUM_OF_DEPENDENT_TO_REBASE: usize = 0;

/// `OptForSize`'s cap on a range the quadratic base search reads
/// ([L568](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L568)).
const MAX_RANGE_FOR_SIZE: usize = 100;

pub struct ConstantHoisting;

#[cfg(test)]
#[path = "constanthoisting_tests.rs"]
mod tests;

/// `ConstantUser`: the instruction and the operand that reads the constant.
#[derive(Clone, Copy)]
struct User {
    inst: InstId,
    index: usize,
}

/// `ConstantCandidate`.
struct Candidate {
    ty: TypeId,
    width: u32,
    bits: u128,
    uses: Vec<User>,
    cumulative_cost: u32,
    /// Every use is a constant the target prices at one instruction where the
    /// pin prices two (x86 ZIMM: a zero-extended 32-bit `mov r32, imm32`).
    cheap_for_target: bool,
}

impl Candidate {
    fn value(&self) -> i128 {
        signed(self.bits, self.width)
    }
}

/// `RebasedConstantInfo`: the uses of one constant, and its offset from the
/// base (none: it is the base).
struct Rebased {
    uses: Vec<User>,
    offset: Option<ConstantId>,
}

/// `ConstantInfo`.
struct Info {
    ty: TypeId,
    base: ConstantId,
    rebased: Vec<Rebased>,
}

/// `UserAdjustment`.
struct Adjustment {
    offset: Option<ConstantId>,
    mat_insert_pt: InstId,
    user: User,
}

/// A `SetVector`: insertion order, no repeats.
#[derive(Default)]
struct SetVec {
    items: Vec<BlockId>,
    seen: IdSet<BlockId>,
}

impl SetVec {
    fn insert(
        &mut self,
        block: BlockId,
    ) {
        if self.seen.insert(block) {
            self.items.push(block);
        }
    }

    fn contains(
        &self,
        block: BlockId,
    ) -> bool {
        self.seen.contains(&block)
    }
}

struct Hoister<'a> {
    context: &'a Context,
    spec: &'a Speculation<'a>,
    tree: &'a DomTree,
    freqs: &'a BlockFrequencies,
    entry: BlockId,
    opt_for_size: bool,
    min_size: bool,
    candidates: Vec<Candidate>,
    infos: Vec<Info>,
    /// `ClonedCastMap`, in the order the clones were made.
    cloned_casts: IdMap<InstId, InstId>,
    cloned_order: Vec<InstId>,
}

/// The `ImmUser` of an instruction: the opcode `getIntImmCostInst` is asked.
fn imm_user(opcode: &Opcode) -> ImmUser {
    match opcode {
        Opcode::GetElementPtr { .. } => ImmUser::GetElementPtr,
        Opcode::Store { .. } => ImmUser::Store,
        Opcode::ICmp(_) => ImmUser::ICmp,
        Opcode::Binary(BinaryOp::And) => ImmUser::And,
        Opcode::Binary(BinaryOp::Add) => ImmUser::Add,
        Opcode::Binary(BinaryOp::Sub) => ImmUser::Sub,
        Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => ImmUser::Divide,
        Opcode::Binary(BinaryOp::Mul) => ImmUser::Mul,
        Opcode::Binary(BinaryOp::Or) => ImmUser::Or,
        Opcode::Binary(BinaryOp::Xor) => ImmUser::Xor,
        Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) => ImmUser::Shift,
        Opcode::Cast(_) | Opcode::Phi | Opcode::Call(_) | Opcode::Select | Opcode::Ret | Opcode::Load { .. } => {
            ImmUser::Other
        }
        _ => ImmUser::Unknown,
    }
}

fn is_cast(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Cast(_))
}

/// The instruction that defines `operand`, if one does.
fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => Some(def),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

/// `isEHPad` of a block: its first non-phi instruction is a landing pad.
fn block_is_eh_pad(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .iter_block(block)
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .is_some_and(|first| matches!(function.instruction(first).opcode, Opcode::LandingPad { .. }))
}

/// `getFirstInsertionPt`: after the phis and the eh pad.
fn first_insertion_pt(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    function
        .iter_block(block)
        .find(|&inst| !matches!(function.instruction(inst).opcode, Opcode::Phi | Opcode::LandingPad { .. }))
}

impl Hoister<'_> {
    fn tti(&self) -> &dyn Tti {
        self.spec.tti
    }

    /// The constant `operand` as `(type, width, bits)`.
    fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<(ConstantId, TypeId, u32, u128)> {
        let Operand::Constant(id) = operand else { return None };
        let constant = self.context.get(id);
        let ConstantKind::Int(bits) = constant.kind else { return None };
        let width = self.context.types.int_bits(constant.ty)?;
        Some((id, constant.ty, width, bits))
    }

    /// `findMatInsertPt`
    /// ([L170](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L170)):
    /// the instruction the constant's materialization goes before.
    fn find_mat_insert_pt(
        &self,
        function: &Function,
        inst: InstId,
        index: usize,
    ) -> InstId {
        let instruction = function.instruction(inst);
        // The operand is a cast: the constant is materialized before it.
        if let Some(cast) = defining(function, instruction.operands[index])
            && is_cast(function, cast)
        {
            return cast;
        }
        let is_phi = instruction.opcode == Opcode::Phi;
        let is_eh_pad = matches!(instruction.opcode, Opcode::LandingPad { .. });
        if !is_phi && !is_eh_pad {
            return inst;
        }
        // Neither a phi nor an eh pad takes an instruction before it: the
        // terminator of the incoming or dominating block does.
        let insertion = if is_phi {
            let Operand::Block(incoming) = instruction.operands[index + 1] else { unreachable!("a phi names blocks") };
            if !block_is_eh_pad(function, incoming) {
                return function.terminator(incoming).expect("a block ends in a terminator");
            }
            incoming
        } else {
            function.parent(inst).expect("a placed instruction")
        };
        let mut idom = self.tree.immediate_dominator(insertion).expect("an eh pad is not the entry");
        while block_is_eh_pad(function, idom) {
            idom = self.tree.immediate_dominator(idom).expect("an eh pad is not the entry");
        }
        function.terminator(idom).expect("a block ends in a terminator")
    }

    /// `findBestInsertionSet`
    /// ([L213](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L213)):
    /// the blocks that dominate `blocks` between them with the least total
    /// frequency.
    fn find_best_insertion_set(
        &self,
        function: &Function,
        blocks: &SetVec,
    ) -> SetVec {
        let entry = self.entry;
        // Candidates: each block of `blocks` no other strictly dominates, and
        // the dominator tree's path from the entry to it.
        let mut candidates: IdSet<BlockId> = IdSet::new();
        for &block in &blocks.items {
            if !self.tree.is_reachable_from_entry(block) {
                continue;
            }
            let mut path = Vec::new();
            let mut node = block;
            let mut is_candidate = false;
            loop {
                path.push(node);
                if node == entry || candidates.contains(&node) {
                    is_candidate = true;
                    break;
                }
                node = self.tree.immediate_dominator(node).expect("the entry dominates the node");
                if blocks.contains(node) {
                    break;
                }
            }
            if is_candidate {
                candidates.extend(path);
            }
        }

        // The candidates top-down.
        let mut orders = vec![entry];
        let mut at = 0;
        while at != orders.len() {
            let node = orders[at];
            at += 1;
            for child in self.tree.children(Some(node)) {
                if candidates.contains(&child) {
                    orders.push(child);
                }
            }
        }

        // Bottom-up: each block's best insertion points for the subtree below
        // it, and their total frequency.
        let position: IdMap<BlockId, usize> = orders.iter().enumerate().map(|(at, &block)| (block, at)).collect();
        let mut best: Vec<(SetVec, u64)> = orders.iter().map(|_| (SetVec::default(), 0)).collect();
        let freq = |block| self.freqs.get_block_freq(block).get_frequency();
        for &node in orders.iter().rev() {
            let (points, points_freq) = std::mem::take(&mut best[position[&node]]);
            if node == entry {
                let mut result = SetVec::default();
                if points_freq > freq(node) || (points_freq == freq(node) && points.items.len() > 1) {
                    result.insert(entry);
                } else {
                    points.items.iter().for_each(|&one| result.insert(one));
                }
                return result;
            }
            let parent = self.tree.immediate_dominator(node).expect("a candidate below the entry has a parent");
            let (parent_points, parent_freq) = &mut best[position[&parent]];
            // Not at an eh pad, where nothing can be inserted.
            if blocks.contains(node)
                || (!block_is_eh_pad(function, node)
                    && (points_freq > freq(node) || (points_freq == freq(node) && points.items.len() > 1)))
            {
                parent_points.insert(node);
                *parent_freq = parent_freq.saturating_add(freq(node));
            } else {
                points.items.iter().for_each(|&one| parent_points.insert(one));
                *parent_freq = parent_freq.saturating_add(points_freq);
            }
        }
        unreachable!("the entry is the first of the orders")
    }

    /// `findConstantInsertionPoint`
    /// ([L315](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L315)):
    /// the instructions the base goes before.
    fn find_constant_insertion_point(
        &self,
        function: &Function,
        mat_insert_pts: &[InstId],
    ) -> Vec<InstId> {
        let mut blocks = SetVec::default();
        for &point in mat_insert_pts {
            blocks.insert(function.parent(point).expect("a placed instruction"));
        }
        if blocks.contains(self.entry) {
            return function.iter_block(self.entry).next().into_iter().collect();
        }
        self.find_best_insertion_set(function, &blocks)
            .items
            .into_iter()
            .filter_map(|block| first_insertion_pt(function, block))
            .collect()
    }

    /// `getIntImmCostInst` / `getIntImmCostIntrin` of the constant at operand
    /// `index` of `inst`.
    fn cost_of(
        &self,
        function: &Function,
        inst: InstId,
        index: usize,
        width: u32,
        value: i128,
    ) -> Cost {
        let instruction = function.instruction(inst);
        if matches!(instruction.opcode, Opcode::Call(_))
            && let Some(name) = llrm_mir::memory::callee(self.context, function, inst)
                .and_then(|global| self.spec.outer.globals.get(global.0 as usize))
                .and_then(|global| global.name.as_deref())
                .filter(|name| name.starts_with("llvm."))
        {
            return self.tti().int_imm_cost_intrin(name, index as u32, value, width);
        }
        self.tti().int_imm_cost_inst_for_size(
            imm_user(&instruction.opcode),
            index as u32,
            value,
            width,
            self.opt_for_size,
        )
    }

    /// `cost`, or the pin's price of the constant when the target's differs.
    fn pin_cost_of(
        &self,
        function: &Function,
        inst: InstId,
        index: usize,
        width: u32,
        value: i128,
        cost: Cost,
    ) -> Cost {
        let instruction = function.instruction(inst);
        if self.opt_for_size || matches!(instruction.opcode, Opcode::Call(_)) {
            return cost;
        }
        self.tti().int_imm_cost_inst_pin(imm_user(&instruction.opcode), index as u32, value, width)
    }

    /// `collectConstantCandidates(.., Inst, Idx, ConstInt)`
    /// ([L362](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L362)).
    fn collect_int(
        &mut self,
        function: &Function,
        map: &mut HashMap<(u32, u128), usize>,
        inst: InstId,
        index: usize,
        constant: (TypeId, u32, u128),
    ) {
        let (ty, width, bits) = constant;
        let cost = self.cost_of(function, inst, index, width, signed(bits, width));
        let pin_cost = self.pin_cost_of(function, inst, index, width, signed(bits, width), cost);
        // Ignore cheap integer constants, as the pin prices them.
        if pin_cost.value > Cost::BASIC.value {
            let at = *map.entry((width, bits)).or_insert_with(|| {
                self.candidates.push(Candidate {
                    ty,
                    width,
                    bits,
                    uses: Vec::new(),
                    cumulative_cost: 0,
                    cheap_for_target: true,
                });
                self.candidates.len() - 1
            });
            let candidate = &mut self.candidates[at];
            candidate.cumulative_cost = candidate.cumulative_cost.wrapping_add(pin_cost.value as u32);
            candidate.cheap_for_target &= cost.value <= Cost::BASIC.value;
            candidate.uses.push(User { inst, index });
        }
    }

    /// `collectConstantCandidates(.., Inst, Idx)`
    /// ([L454](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L454)).
    fn collect_operand(
        &mut self,
        function: &Function,
        map: &mut HashMap<(u32, u128), usize>,
        inst: InstId,
        index: usize,
    ) {
        let operand = function.instruction(inst).operands[index];
        if let Some((_, ty, width, bits)) = self.int_constant(operand) {
            self.collect_int(function, map, inst, index, (ty, width, bits));
            return;
        }
        // A cast of a constant: the user is taken to read the constant.
        if let Some(cast) = defining(function, operand)
            && is_cast(function, cast)
            && let Some((_, ty, width, bits)) = self.int_constant(function.instruction(cast).operands[0])
        {
            self.collect_int(function, map, inst, index, (ty, width, bits));
        }
    }

    /// `collectConstantCandidates(.., Inst)`
    /// ([L500](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L500)).
    fn collect_instruction(
        &mut self,
        function: &Function,
        map: &mut HashMap<(u32, u128), usize>,
        inst: InstId,
    ) {
        // Casts are visited through their users.
        if is_cast(function, inst) {
            return;
        }
        let instruction = function.instruction(inst);
        for index in 0..instruction.operands.len() {
            // An incoming edge from an unreachable block.
            if instruction.opcode == Opcode::Phi
                && let Some(&Operand::Block(incoming)) = instruction.operands.get(index + 1)
                && index % 2 == 0
                && !self.tree.is_reachable_from_entry(incoming)
            {
                continue;
            }
            if crate::sinkcommon::can_replace_operand_with_variable(self.context, self.spec, function, inst, index) {
                self.collect_operand(function, map, inst, index);
            }
        }
    }

    /// `preferToKeepConstantsAttached` (`BasicTTIImpl`'s rule is the target's).
    fn keeps_constants_attached(
        &self,
        function: &Function,
        inst: InstId,
    ) -> bool {
        let instruction = function.instruction(inst);
        let by_constant = matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem)
        ) && matches!(instruction.operands[1], Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Int(_)));
        self.tti().prefer_to_keep_constants_attached(by_constant, self.min_size) || self.select_mask_kept(function, inst)
    }

    /// DEVIATE ZIMM (M64-ZIMM-CRC, owner m64, exit 2026-11-20): `select c, 0, K`
    /// or `select c, K, 0` with `K` an immediate the target's mask `and` takes.
    fn select_mask_kept(
        &self,
        function: &Function,
        inst: InstId,
    ) -> bool {
        let instruction = function.instruction(inst);
        if instruction.opcode != Opcode::Select || self.opt_for_size {
            return false;
        }
        let int = |operand: Operand| self.int_constant(operand).map(|(_, _, width, bits)| (bits, width));
        match (int(instruction.operands[1]), int(instruction.operands[2])) {
            (Some((0, _)), Some((bits, width))) | (Some((bits, width)), Some((0, _))) => {
                self.tti().keeps_select_mask_attached(bits, width)
            }
            _ => false,
        }
    }

    /// `collectConstantCandidates(Function &)`
    /// ([L527](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L527)).
    fn collect_candidates(
        &mut self,
        function: &Function,
    ) {
        let mut map = HashMap::default();
        for &block in function.layout() {
            if !self.tree.is_reachable_from_entry(block) {
                continue;
            }
            for inst in function.iter_block(block) {
                if !self.keeps_constants_attached(function, inst) {
                    self.collect_instruction(function, &mut map, inst);
                }
            }
        }
    }

    /// `maximizeConstantsInRange`
    /// ([L562](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L562)):
    /// the number of uses in `range` and the index of the best base.
    fn maximize_constants_in_range(
        &self,
        function: &Function,
        range: std::ops::Range<usize>,
    ) -> (usize, usize) {
        let mut best = range.start;
        let mut uses = 0;
        if !self.opt_for_size || range.len() > MAX_RANGE_FOR_SIZE {
            for at in range {
                uses += self.candidates[at].uses.len();
                if self.candidates[at].cumulative_cost > self.candidates[best].cumulative_cost {
                    best = at;
                }
            }
            return (uses, best);
        }
        // At `optsize`: the constant whose uses cost most to materialize, less
        // what the others cost as offsets from it.
        let mut max_cost = -1i64;
        for at in range.clone() {
            let candidate = &self.candidates[at];
            let mut cost = 0i64;
            uses += candidate.uses.len();
            for user in &candidate.uses {
                let op = imm_user(&function.instruction(user.inst).opcode);
                cost += self
                    .tti()
                    .int_imm_cost_inst_for_size(
                        op,
                        user.index as u32,
                        candidate.value(),
                        candidate.width,
                        self.opt_for_size,
                    )
                    .value;
                for other in &self.candidates[range.clone()] {
                    let diff = signed(other.bits.wrapping_sub(candidate.bits) & mask(candidate.width), candidate.width);
                    cost -= self.tti().int_imm_code_size_cost(op, user.index as u32, diff, candidate.width).value;
                }
            }
            if cost > max_cost {
                max_cost = cost;
                best = at;
            }
        }
        (uses, best)
    }

    /// `findAndMakeBaseConstant`
    /// ([L617](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L617)).
    fn find_and_make_base_constant(
        &mut self,
        function: &Function,
        range: std::ops::Range<usize>,
    ) {
        let (uses, best) = self.maximize_constants_in_range(function, range.clone());
        // Don't hoist constants that have only one use.
        if uses <= 1 {
            return;
        }
        // DEVIATE ZIMM (M64-ZIMM-DCL, owner m64, exit 2026-11-20): a lone
        // constant the target materializes in one instruction stays where it is;
        // beside a neighbour it is hoisted as the pin does, a base and an offset.
        if range.len() == 1 && self.candidates[range.start].cheap_for_target {
            return;
        }
        let (ty, width, base_bits) =
            (self.candidates[best].ty, self.candidates[best].width, self.candidates[best].bits);
        let mut rebased = Vec::with_capacity(range.len());
        for at in range {
            let candidate = &mut self.candidates[at];
            let diff = candidate.bits.wrapping_sub(base_bits) & mask(width);
            let offset = (diff != 0).then(|| self.context.int(ty, signed(diff, width)));
            rebased.push(Rebased { uses: std::mem::take(&mut candidate.uses), offset });
        }
        self.infos.push(Info { ty, base: self.context.int(ty, signed(base_bits, width)), rebased });
    }

    /// `findBaseConstants`
    /// ([L648](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L648)).
    fn find_base_constants(
        &mut self,
        function: &Function,
    ) {
        // By type, then by value.
        self.candidates.sort_by_key(|candidate| (candidate.width, candidate.bits));
        let count = self.candidates.len();
        let mut min = 0;
        for at in 1..count {
            if self.candidates[min].width == self.candidates[at].width && self.in_range_of_min(function, min, at) {
                continue;
            }
            // A different type, or the constant is not an `add` immediate
            // from the smallest any more.
            self.find_and_make_base_constant(function, min..at);
            min = at;
        }
        self.find_and_make_base_constant(function, min..count);
    }

    /// Whether candidate `at` is an `add` immediate from candidate `min`, and
    /// an offset its memory use can address.
    fn in_range_of_min(
        &self,
        function: &Function,
        min: usize,
        at: usize,
    ) -> bool {
        let (low, high) = (&self.candidates[min], &self.candidates[at]);
        let mut mem_use_ty = None;
        for user in &high.uses {
            let instruction = function.instruction(user.inst);
            match instruction.opcode {
                Opcode::Load { .. } => {
                    mem_use_ty = Some(instruction.ty);
                    break;
                }
                // The constant must be the store's pointer.
                Opcode::Store { .. } if instruction.operands[1] == instruction.operands[user.index] => {
                    mem_use_ty = function.operand_type(self.context, instruction.operands[0]);
                    break;
                }
                _ => {}
            }
        }
        if high.width > 64 {
            return false;
        }
        let diff = signed(high.bits.wrapping_sub(low.bits) & mask(high.width), high.width) as i64;
        self.tti().is_legal_add_immediate(diff)
            && mem_use_ty.is_none_or(|ty| {
                let bytes = self.spec.layout.store_size(&self.context.types, ty);
                self.tti().is_legal_addressing_mode_for(bytes, false, diff, true, 0)
            })
    }

    /// `updateOperand`
    /// ([L712](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L712)):
    /// whether `mat` was used.
    fn update_operand(
        function: &mut Function,
        user: User,
        mat: Operand,
    ) -> bool {
        let instruction = function.instruction(user.inst);
        if instruction.opcode == Opcode::Phi {
            // An earlier operand from the same incoming block (a `switch` with
            // two edges to the phi) gives the value, or the phi names two.
            let same = (0..user.index)
                .step_by(2)
                .find(|&earlier| instruction.operands[earlier + 1] == instruction.operands[user.index + 1]);
            if let Some(earlier) = same {
                let value = instruction.operands[earlier];
                function.set_operand(user.inst, user.index, value);
                return false;
            }
        }
        function.set_operand(user.inst, user.index, mat);
        true
    }

    /// `emitBaseConstants(Instruction *Base, UserAdjustment *Adj)`
    /// ([L736](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L736)).
    fn emit_adjustment(
        &mut self,
        function: &mut Function,
        base: InstId,
        ty: TypeId,
        adjustment: &Adjustment,
    ) {
        let base_value = function.instruction(base).result.expect("the base has a result");
        let mut mat = base;
        if let Some(offset) = adjustment.offset {
            mat = function.create_instruction(
                Opcode::Binary(BinaryOp::Add),
                ty,
                vec![Operand::Value(base_value), Operand::Constant(offset)],
                Flags::default(),
                Some("const_mat"),
            );
            function.insert(mat, Position::Before(adjustment.mat_insert_pt)).expect("an unplaced instruction");
        }
        let mat_value = Operand::Value(function.instruction(mat).result.expect("a result"));
        let user = adjustment.user;
        let operand = function.instruction(user.inst).operands[user.index];
        // The operand is the constant.
        if matches!(operand, Operand::Constant(_)) {
            if !Self::update_operand(function, user, mat_value) && adjustment.offset.is_some() {
                function.erase(mat).expect("mat is unused");
            }
            return;
        }
        // The operand is a cast of the constant: a copy of the cast reads the
        // base, made once.
        let cast = defining(function, operand).expect("a cast of a constant");
        let cloned = match self.cloned_casts.get(&cast) {
            Some(&cloned) => cloned,
            None => {
                let cloned = function.clone_instruction(cast);
                function.set_operand(cloned, 0, mat_value);
                function.insert(cloned, Position::After(cast)).expect("an unplaced instruction");
                self.cloned_casts.insert(cast, cloned);
                self.cloned_order.push(cast);
                cloned
            }
        };
        let cloned_value = Operand::Value(function.instruction(cloned).result.expect("a cast has a result"));
        Self::update_operand(function, user, cloned_value);
    }

    /// `emitBaseConstants(GlobalVariable *)`
    /// ([L829](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L829)).
    fn emit_base_constants(
        &mut self,
        function: &mut Function,
    ) -> bool {
        let mut changed = false;
        let infos = std::mem::take(&mut self.infos);
        for info in &infos {
            let mat_insert_pts: Vec<InstId> = info
                .rebased
                .iter()
                .flat_map(|one| &one.uses)
                .map(|user| self.find_mat_insert_pt(function, user.inst, user.index))
                .collect();
            let ip_set = self.find_constant_insertion_point(function, &mat_insert_pts);
            // Empty when the function has unreachable blocks only.
            if ip_set.is_empty() {
                continue;
            }
            for &ip in &ip_set {
                let mut to_be_rebased = Vec::new();
                let mut count = 0;
                let ip_block = function.parent(ip).expect("a placed instruction");
                for one in &info.rebased {
                    for &user in &one.uses {
                        let mat_insert_pt = mat_insert_pts[count];
                        count += 1;
                        let origin = function.parent(mat_insert_pt).expect("a placed instruction");
                        // Where the base is made in several places, a use is
                        // rebased on the one that dominates it.
                        if ip_set.len() == 1 || self.tree.dominates(ip_block, origin) {
                            to_be_rebased.push(Adjustment { offset: one.offset, mat_insert_pt, user });
                        }
                    }
                }
                // Few depend on this base: the base and the rebased are
                // assumed to cost the same.
                if to_be_rebased.len() < MIN_NUM_OF_DEPENDENT_TO_REBASE {
                    continue;
                }
                // The base, hidden behind a `bitcast` so nothing folds it into
                // its uses.
                let base = function.create_instruction(
                    Opcode::Cast(CastOp::BitCast),
                    info.ty,
                    vec![Operand::Constant(info.base)],
                    Flags::default(),
                    Some("const"),
                );
                function.insert(base, Position::Before(ip)).expect("an unplaced instruction");
                for adjustment in &to_be_rebased {
                    self.emit_adjustment(function, base, info.ty, adjustment);
                }
            }
            changed = true;
        }
        changed
    }

    /// `deleteDeadCastInst`
    /// ([L920](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L920)).
    fn delete_dead_casts(
        &self,
        function: &mut Function,
    ) {
        for &cast in &self.cloned_order {
            let result = function.instruction(cast).result.expect("a cast has a result");
            if function.users(result).is_empty() {
                function.erase(cast).expect("no users");
            }
        }
    }
}

/// `skipFunction` for `optnone` is the manager's; `optsize` and `minsize` as
/// `shouldOptimizeForSize` reads them (DEVIATE 3).
fn size_attributes(function: &Function) -> (bool, bool) {
    let has = |wanted: &str| function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == wanted));
    let min_size = has("minsize");
    (min_size || has("optsize"), min_size)
}

impl FunctionPass for ConstantHoisting {
    fn name(&self) -> &'static str {
        "constanthoisting"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("constant-hoisting")
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    /// `runImpl`
    /// ([L927](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstantHoisting.cpp#L927)).
    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        if !tti.constant_hoisting() {
            return PreservedAnalyses::all();
        }
        let Some(entry) = unit.function.entry() else { return PreservedAnalyses::all() };
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let freqs = analyses.get::<BlockFrequencies>(unit.context, unit.layout, unit.function);
        let spec = Speculation { tti: &*tti, layout: unit.layout, outer: &outer };
        let (opt_for_size, min_size) = size_attributes(unit.function);
        let mut hoister = Hoister {
            context: unit.context,
            spec: &spec,
            tree: &tree,
            freqs: &freqs,
            entry,
            opt_for_size,
            min_size,
            candidates: Vec::new(),
            infos: Vec::new(),
            cloned_casts: IdMap::new(),
            cloned_order: Vec::new(),
        };
        hoister.collect_candidates(unit.function);
        if !hoister.candidates.is_empty() {
            hoister.find_base_constants(unit.function);
        }
        let changed = !hoister.infos.is_empty() && hoister.emit_base_constants(unit.function);
        hoister.delete_dead_casts(unit.function);
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
