//! A chain of `a.x == b.x && a.y == b.y` compares of adjacent fields is one
//! `memcmp` call, as LLVM's `MergeICmps`.
//!
//! # How it works
//! A translation of LLVM
//! [`MergeICmps.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/MergeICmps.cpp)
//! at d1106deb71cc. Each phi that joins a chain of blocks, every one a
//! compare of two loads at a constant offset from a base that branches out to
//! the phi on a difference, is looked at. The compares are sorted by
//! (base, offset); those whose bytes touch become one `memcmp(a, b, n) == 0`
//! block, and the chain is rebuilt from the groups in their first-seen order.
//! The groups' bytes must be dereferenceable, as the loads may now run in
//! another order. Only where the target expands `memcmp` later
//! (`Tti::enable_mem_cmp_expansion`) and the compile states `memcmp`.
//!
//! # Example
//! ```text
//! b0: %x = load a.0; %y = load b.0; br (%x == %y), b1, done
//! b1: %u = load a.2; %v = load b.2; br done   ; done: phi [false, b0], [%u == %v, b1]
//! ```
//! becomes one block calling `memcmp(a, b, 4)` and comparing it to zero.
//!
//! DEVIATE 1 (exit 2026-12-31,
//! [runImpl](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L948)):
//! the pin updates the dominator tree with a `DomTreeUpdater`; here a change
//! invalidates it.
//!
//! DEVIATE 2 (exit 2026-12-31,
//! [canSinkBCECmpInst](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L249)):
//! `AA.getModRefInfo` is the store against load `alias` answer; any other
//! writer (a call) is assumed to modify.
//!
//! DEVIATE 3 (exit 2026-12-31,
//! [isDereferenceable](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L759)):
//! `valuetracking::dereferenceable` has no context instruction, so what the
//! pin proves from an assume or a free before it is not seen.
//!
//! DEVIATE 4 (exit 2026-12-31,
//! [mergeComparisons](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L650)):
//! the merged location is the first compare's, and a merge of several blocks
//! has no `!prof` (`computeMergedBranchWeights` is untranslated).
//!
//! MIR has no `blockaddress`, so the address-taken checks have nothing to
//! refuse.
//!
//! # Parity slot
//! `mergeicmps@1671` (PassBuilderPipelines.cpp#L1671), between `divrempairs`
//! and `tailrec`. Not wired: `ExpandMemCmp` (L1676) is untranslated, and a
//! `memcmp` call left in place is worse than the compares.
//!
//! # Keeps
//! Nothing: blocks and edges change.

use std::cell::OnceCell;

use llrm_mir::alias::{self, Alias, Location};
use llrm_mir::context::{Constant, ConstantKind, Context, mask, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::hash::HashMap;
use llrm_mir::libfunc::Routine;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::{self, Query, Sizes};

use crate::simplifylibcalls::{NewCall, emit, target_library_info};

/// `BCEAtom`: an integer load at a constant offset from a base.
#[derive(Clone, Copy, Debug)]
struct Atom {
    gep: Option<InstId>,
    load: InstId,
    /// Order of the base's first appearance in the chain, from 1.
    base_id: u32,
    offset: i128,
}

impl Atom {
    /// `BCEAtom::operator<`.
    fn key(&self) -> (u32, i128) {
        (self.base_id, self.offset)
    }
}

/// `BCECmp`.
#[derive(Clone, Copy, Debug)]
struct Cmp {
    lhs: Atom,
    rhs: Atom,
    size_bits: u32,
    cmp: InstId,
}

/// `BCECmpBlock`.
#[derive(Clone, Debug)]
struct CmpBlock {
    block: BlockId,
    /// The compare's own instructions and the terminator.
    insts: Vec<InstId>,
    cmp: Cmp,
    require_split: bool,
    orig_order: usize,
}

/// What the analysis half reads.
struct Env<'a> {
    context: &'a Context,
    layout: &'a DataLayout,
    globals: &'a [GlobalValue],
    callees: &'a Callees,
    metadata: &'a [MetadataNode],
    sizes: OnceCell<Sizes>,
}

/// `BaseIdentifier`: ids in the order bases are first seen.
#[derive(Default)]
struct BaseIdentifier {
    ids: HashMap<Operand, u32>,
}

impl BaseIdentifier {
    fn id(
        &mut self,
        base: Operand,
    ) -> u32 {
        let next = self.ids.len() as u32 + 1;
        *self.ids.entry(base).or_insert(next)
    }
}

/// `Instruction::isUsedOutsideOfBlock`.
fn used_outside_of_block(
    function: &Function,
    inst: InstId,
    block: BlockId,
) -> bool {
    let Some(result) = function.instruction(inst).result else { return false };
    function
        .users(result)
        .iter()
        .any(|one| function.instruction(one.user).opcode == Opcode::Phi || function.parent(one.user) != Some(block))
}

fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    }
}

impl Env<'_> {
    /// `GEP->accumulateConstantOffset`: the offset when every index is a
    /// constant.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L134>
    fn constant_offset(
        &self,
        function: &Function,
        gep: InstId,
    ) -> Option<i128> {
        let Opcode::GetElementPtr { source } = function.instruction(gep).opcode else { return None };
        let indices = function.instruction(gep).operands[1..]
            .iter()
            .map(|&index| {
                let Operand::Constant(id) = index else { return None };
                let constant = self.context.get(id);
                let ConstantKind::Int(bits) = constant.kind else { return None };
                Some(Some(signed(bits, self.context.types.int_bits(constant.ty)?)))
            })
            .collect::<Option<Vec<_>>>()?;
        let (offset, variable) = self.layout.collect_offset(&self.context.types, source, &indices);
        let width = self.layout.pointer(0).index_bits;
        variable.is_empty().then(|| signed(offset as u128 & mask(width), width))
    }

    /// `visitICmpLoadOperand`: the atom `value` is, if it is a load at a
    /// constant offset that nothing else reads.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L134>
    fn visit_load_operand(
        &self,
        function: &Function,
        value: Operand,
        bases: &mut BaseIdentifier,
    ) -> Option<Atom> {
        let load = defining(function, value)?;
        let instruction = function.instruction(load);
        let Opcode::Load { volatile: false, .. } = instruction.opcode else { return None };
        let block = function.parent(load)?;
        if used_outside_of_block(function, load, block) {
            return None;
        }
        let address = instruction.operands[0];
        let address_ty = function.operand_type(self.context, address)?;
        if !matches!(self.context.types.get(address_ty), Type::Pointer(0)) {
            return None;
        }
        // The pass only works when every compared element has a byte-multiple
        // size.
        let types = &self.context.types;
        if self.layout.size_bits(types, instruction.ty) != self.layout.store_size(types, instruction.ty) * 8 {
            return None;
        }
        let (mut base, mut offset, mut gep) = (address, 0, None);
        if let Some(found) = defining(function, address)
            && matches!(function.instruction(found).opcode, Opcode::GetElementPtr { .. })
        {
            if used_outside_of_block(function, found, block) {
                return None;
            }
            offset = self.constant_offset(function, found)?;
            base = function.instruction(found).operands[0];
            gep = Some(found);
        }
        Some(Atom { gep, load, base_id: bases.id(base), offset })
    }

    /// `visitICmp`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L313>
    fn visit_icmp(
        &self,
        function: &Function,
        cmp: InstId,
        expected: IntPredicate,
        bases: &mut BaseIdentifier,
    ) -> Option<Cmp> {
        let instruction = function.instruction(cmp);
        // The compare is used once: as a branch condition, or as the last
        // incoming value of the phi.
        if function.users(instruction.result?).iter().count() != 1 || instruction.opcode != Opcode::ICmp(expected) {
            return None;
        }
        let mut lhs = self.visit_load_operand(function, instruction.operands[0], bases)?;
        let mut rhs = self.visit_load_operand(function, instruction.operands[1], bases)?;
        // The smaller atom goes on the left.
        if rhs.key() < lhs.key() {
            std::mem::swap(&mut lhs, &mut rhs);
        }
        let ty = function.operand_type(self.context, instruction.operands[0])?;
        let size_bits = self.layout.size_bits(&self.context.types, ty) as u32;
        Some(Cmp { lhs, rhs, size_bits, cmp })
    }

    /// `visitCmpBlock`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L344>
    fn visit_cmp_block(
        &self,
        function: &Function,
        value: Operand,
        block: BlockId,
        phi_block: BlockId,
        bases: &mut BaseIdentifier,
    ) -> Option<CmpBlock> {
        let term = function.terminator(block)?;
        let operands = &function.instruction(term).operands;
        let (cond, expected) = match (function.instruction(term).opcode == Opcode::Br, operands.len()) {
            // The last link: the incoming value is the compare itself.
            (true, 1) => (value, IntPredicate::Eq),
            // Chained: the incoming value is the constant false.
            (true, 3) => {
                let Operand::Constant(constant) = value else { return None };
                if !matches!(self.context.get(constant).kind, ConstantKind::Int(0)) {
                    return None;
                }
                let false_to_phi = operands[2] == Operand::Block(phi_block);
                (operands[0], if false_to_phi { IntPredicate::Eq } else { IntPredicate::Ne })
            }
            _ => return None,
        };
        let cmp = defining(function, cond)?;
        if !matches!(function.instruction(cmp).opcode, Opcode::ICmp(_)) {
            return None;
        }
        let result = self.visit_icmp(function, cmp, expected, bases)?;
        let mut insts = vec![result.lhs.load, result.rhs.load, cmp, term];
        insts.extend(result.lhs.gep);
        insts.extend(result.rhs.gep);
        Some(CmpBlock { block, insts, cmp: result, require_split: false, orig_order: 0 })
    }

    /// Whether `AA.getModRefInfo(inst, MemoryLocation::get(load))` is a mod
    /// (DEVIATE 2).
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L249>
    fn may_modify(
        &self,
        function: &Function,
        inst: InstId,
        load: InstId,
    ) -> bool {
        let instruction = function.instruction(inst);
        if !matches!(instruction.opcode, Opcode::Store { .. }) {
            return true;
        }
        let Some(value_ty) = function.operand_type(self.context, instruction.operands[0]) else { return true };
        let loaded = function.instruction(load);
        let location = |access: InstId, pointer: Operand, ty: TypeId| Location {
            pointer,
            bytes: self.layout.store_size(&self.context.types, ty),
            tbaa: alias::tag(function, access),
        };
        alias::alias(
            self.context,
            self.layout,
            self.callees,
            self.metadata,
            function,
            location(inst, instruction.operands[1], value_ty),
            location(load, loaded.operands[0], loaded.ty),
        ) != Alias::No
    }
}

impl CmpBlock {
    fn lhs(&self) -> &Atom {
        &self.cmp.lhs
    }

    fn rhs(&self) -> &Atom {
        &self.cmp.rhs
    }

    /// `BCECmpBlock::doesOtherWork`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L299>
    fn does_other_work(
        &self,
        function: &Function,
    ) -> bool {
        function.iter_block(self.block).any(|inst| !self.insts.contains(&inst))
    }

    /// `BCECmpBlock::canSinkBCECmpInst`: `inst` can go below the compare.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L249>
    fn can_sink(
        &self,
        env: &Env,
        function: &Function,
        inst: InstId,
    ) -> bool {
        let query = Query { globals: env.globals, ..Query::bare(env.context, function, env.layout) };
        if valuetracking::guarantee::may_write_to_memory(&query, inst) {
            // A clobber before the load can still sink the load.
            let clobbers = |load: InstId| {
                (function.parent(inst) != function.parent(load) || !function.comes_before(inst, load))
                    && env.may_modify(function, inst, load)
            };
            if clobbers(self.lhs().load) || clobbers(self.rhs().load) {
                return false;
            }
        }
        // It does not read what the compare makes.
        function
            .instruction(inst)
            .operands
            .iter()
            .all(|&operand| defining(function, operand).is_none_or(|def| !self.insts.contains(&def)))
    }

    /// `BCECmpBlock::canSplit`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L287>
    fn can_split(
        &self,
        env: &Env,
        function: &Function,
    ) -> bool {
        function
            .iter_block(self.block)
            .filter(|inst| !self.insts.contains(inst))
            .all(|inst| self.can_sink(env, function, inst))
    }

    /// `BCECmpBlock::split`: the other instructions move to the start of
    /// `parent`, in their order.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L271>
    fn split(
        &self,
        function: &mut Function,
        parent: BlockId,
    ) {
        let others: Vec<InstId> = function.iter_block(self.block).filter(|inst| !self.insts.contains(inst)).collect();
        for inst in others.into_iter().rev() {
            let at = function.block(parent).first().map_or(Position::End(parent), Position::Before);
            function.move_to(inst, at).expect("placed");
        }
    }
}

/// `areContiguous`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L437>
fn are_contiguous(
    first: &CmpBlock,
    second: &CmpBlock,
) -> bool {
    let bytes = i128::from(first.cmp.size_bits / 8);
    first.lhs().base_id == second.lhs().base_id
        && first.rhs().base_id == second.rhs().base_id
        && first.lhs().offset + bytes == second.lhs().offset
        && first.rhs().offset + bytes == second.rhs().offset
}

/// `mergeBlocks`: the compares grouped into runs of touching bytes.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L454>
fn merge_blocks(mut blocks: Vec<CmpBlock>) -> Vec<Vec<CmpBlock>> {
    // Sort to find the runs.
    blocks.sort_by_key(|block| (block.lhs().key(), block.rhs().key()));
    let mut merged: Vec<Vec<CmpBlock>> = Vec::new();
    for block in blocks {
        match merged.last_mut() {
            Some(last) if are_contiguous(last.last().expect("a run has a block"), &block) => last.push(block),
            _ => merged.push(vec![block]),
        }
    }
    // Compares that are not merged keep their order: reordering them could
    // branch on poison.
    merged.sort_by_key(|run| run.iter().map(|block| block.orig_order).min());
    merged
}

/// `BCECmpChain`: the runs, and the block the chain was entered by.
struct Chain {
    merged: Vec<Vec<CmpBlock>>,
    entry: BlockId,
}

impl Chain {
    fn at_least_one_merged(&self) -> bool {
        self.merged.iter().any(|run| run.len() > 1)
    }
}

/// The value `phi` takes from `block`.
fn incoming_for(
    function: &Function,
    phi: InstId,
    block: BlockId,
) -> Option<Operand> {
    function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).map(|pair| pair[0])
}

/// `BCECmpChain::BCECmpChain`; none where the chain does not merge.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L486>
fn build_chain(
    env: &Env,
    function: &Function,
    blocks: &[BlockId],
    phi: InstId,
) -> Option<Chain> {
    let phi_block = function.parent(phi)?;
    let mut comparisons: Vec<CmpBlock> = Vec::new();
    let mut bases = BaseIdentifier::default();
    for &block in blocks {
        let value = incoming_for(function, phi, block)?;
        let mut comparison = env.visit_cmp_block(function, value, block, phi_block, &mut bases)?;
        if comparison.does_other_work(function) {
            // Only the first block can be split: its other work moves to the
            // new block; later, instructions before the compares in program
            // order would stay.
            if !comparisons.is_empty() {
                return None;
            }
            if comparison.can_split(env, function) {
                comparison.require_split = true;
                comparisons.push(comparison);
            }
            continue;
        }
        comparison.orig_order = comparisons.len();
        comparisons.push(comparison);
    }
    let entry = comparisons.first()?.block;
    Some(Chain { merged: merge_blocks(comparisons), entry })
}

/// `BCECmpChain::isDereferenceable`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L759>
fn is_dereferenceable(
    env: &Env,
    function: &Function,
    chain: &Chain,
) -> bool {
    let sizes = env.sizes.get_or_init(|| valuetracking::sizes_of(env.context, env.globals, env.layout));
    chain.merged.iter().all(|run| {
        let bytes = u64::from(run.iter().map(|block| block.cmp.size_bits).sum::<u32>() / 8);
        [run[0].lhs(), run[0].rhs()].into_iter().all(|atom| {
            let pointer = function.instruction(atom.load).operands[0];
            valuetracking::dereferenceable(env.context, env.layout, sizes, function, pointer, bytes)
        })
    })
}

/// `getOrderedBlocks`: the chain's blocks from the first, walking up from the
/// last; none where the walk fails.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L841>
fn ordered_blocks(
    function: &Function,
    phi: InstId,
    last: BlockId,
    count: usize,
) -> Option<Vec<BlockId>> {
    let mut blocks = vec![last; count];
    let mut current = last;
    for index in (1..count).rev() {
        blocks[index] = current;
        // One predecessor, and it links back to the phi.
        let [single] = function.edge_predecessors(current)[..] else { return None };
        incoming_for(function, phi, single)?;
        current = single;
    }
    blocks[0] = current;
    Some(blocks)
}

/// `processPhi`, up to the decision: the chain to rebuild.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L874>
fn analyze_phi(
    env: &Env,
    function: &Function,
    phi: InstId,
) -> Option<Chain> {
    let incoming: Vec<(Operand, BlockId)> = function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| match pair[1] {
            Operand::Block(block) => (pair[0], block),
            _ => unreachable!("a phi's incoming block"),
        })
        .collect();
    if incoming.len() <= 1 {
        return None;
    }
    let phi_block = function.parent(phi)?;
    // One incoming value is not a constant: the last block's compare.
    let mut last = None;
    for &(value, block) in &incoming {
        if matches!(value, Operand::Constant(id) if matches!(env.context.get(id).kind, ConstantKind::Int(_))) {
            continue;
        }
        let compare = defining(function, value)
            .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::ICmp(_)))
            .filter(|&inst| function.parent(inst) == Some(block));
        if last.is_some() || compare.is_none() {
            return None;
        }
        last = Some(block);
    }
    let last = last?;
    if function.sole_successor(last) != Some(phi_block) {
        return None;
    }
    let blocks = ordered_blocks(function, phi, last, incoming.len())?;
    let chain = build_chain(env, function, &blocks, phi)?;
    (chain.at_least_one_merged() && is_dereferenceable(env, function, &chain)).then_some(chain)
}

/// `MergedBlockName`: the names of the blocks joined with `+`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L577>
fn merged_name(
    function: &Function,
    run: &[CmpBlock],
) -> Option<String> {
    let name = |block: &CmpBlock| function.block(block.block).name.clone().unwrap_or_default();
    let mut text = name(&run[0]);
    for block in &run[1..] {
        let next = name(block);
        if !next.is_empty() {
            text.push('+');
            text.push_str(&next);
        }
    }
    (!text.is_empty()).then_some(text)
}

/// The types and routines `mergeComparisons` forms code with.
struct Emit<'a> {
    routines: &'a [Routine],
    int: TypeId,
    size: TypeId,
}

/// `mergeComparisons`: the block of the run `run`, going to `next` when equal
/// and to the phi's block when not.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L650>
fn merge_comparisons(
    unit: &mut Unit,
    emitter: &Emit,
    run: &[CmpBlock],
    insert_before: BlockId,
    next: BlockId,
    phi: InstId,
) -> BlockId {
    let first = &run[0];
    let phi_block = unit.function.parent(phi).expect("placed");
    let name = merged_name(unit.function, run);
    let block = unit.function.create_block(name.as_deref());
    unit.function.insert_block_before(block, insert_before).expect("a placed chain entry");
    let void = unit.context.types.void();
    let flag = unit.context.types.int(1);

    let address = |function: &mut Function, atom: &Atom| match atom.gep {
        Some(gep) => {
            let copy = function.clone_instruction(gep);
            function.insert(copy, Position::End(block)).expect("a new block");
            Operand::Value(function.instruction(copy).result.expect("a gep's value"))
        }
        None => function.instruction(atom.load).operands[0],
    };
    let lhs = address(unit.function, first.lhs());
    let rhs = address(unit.function, first.rhs());

    // The instructions that do other work run before the chain.
    if let Some(split) = run.iter().find(|one| one.require_split) {
        split.split(unit.function, block);
    }

    let compare = if run.len() == 1 {
        let load = |function: &mut Function, atom: &Atom, pointer: Operand| {
            let copy = function.clone_instruction(atom.load);
            function.set_operand(copy, 0, pointer);
            function.insert(copy, Position::End(block)).expect("a new block");
            Operand::Value(function.instruction(copy).result.expect("a load's value"))
        };
        let left = load(unit.function, first.lhs(), lhs);
        let right = load(unit.function, first.rhs(), rhs);
        (left, right)
    } else {
        let total: u32 = run.iter().map(|one| one.cmp.size_bits).sum();
        let size = Operand::Constant(unit.context.int(emitter.size, i128::from(total / 8)));
        let pointer = unit.context.types.ptr(0);
        let call = emit(
            unit,
            emitter.routines,
            NewCall {
                standard: "memcmp",
                args: vec![lhs, rhs, size],
                returns: emitter.int,
                params: vec![pointer, pointer, emitter.size],
            },
            Position::End(block),
        )
        .expect("memcmp is stated");
        crate::divrempairs::copy_location(unit.function, first.cmp.cmp, call);
        let result = Operand::Value(unit.function.instruction(call).result.expect("memcmp returns int"));
        (result, Operand::Constant(unit.context.int(emitter.int, 0)))
    };
    let equal = unit.function.create_instruction(
        Opcode::ICmp(IntPredicate::Eq),
        flag,
        vec![compare.0, compare.1],
        Flags::default(),
        None,
    );
    unit.function.insert(equal, Position::End(block)).expect("a new block");
    crate::divrempairs::copy_location(unit.function, first.cmp.cmp, equal);
    let equal_value = Operand::Value(unit.function.instruction(equal).result.expect("a compare's value"));

    let old_term = unit.function.terminator(first.block).expect("a terminator");
    let (branch, incoming) = if next == phi_block {
        // Continue to the phi, passing it the compare.
        let branch =
            unit.function.create_instruction(Opcode::Br, void, vec![Operand::Block(next)], Flags::default(), None);
        (branch, equal_value)
    } else {
        // To the next block if equal, to the phi else.
        let operands = vec![equal_value, Operand::Block(next), Operand::Block(phi_block)];
        let branch = unit.function.create_instruction(Opcode::Br, void, operands, Flags::default(), None);
        let weights = unit.function.instruction(old_term).metadata.iter().find(|(kind, _)| kind == "prof");
        if let (1, Some(&(_, weights))) = (run.len(), weights) {
            unit.function.set_metadata(branch, "prof", Some(weights));
        }
        (branch, Operand::Constant(unit.context.int(flag, 0)))
    };
    unit.function.insert(branch, Position::End(block)).expect("a new block");
    crate::divrempairs::copy_location(unit.function, old_term, branch);
    let mut operands = unit.function.instruction(phi).operands.clone();
    operands.extend([incoming, Operand::Block(block)]);
    unit.function.set_operands(phi, operands);
    block
}

/// `BCECmpChain::simplify`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L786>
fn simplify(
    unit: &mut Unit,
    emitter: &Emit,
    chain: &Chain,
    phi: InstId,
) {
    let phi_block = unit.function.parent(phi).expect("placed");
    // From the phi back, so the next block is there to branch to.
    let (mut insert_before, mut next) = (chain.entry, phi_block);
    for run in chain.merged.iter().rev() {
        next = merge_comparisons(unit, emitter, run, insert_before, next, phi);
        insert_before = next;
    }

    // The old chain is unreachable once what entered it enters the new.
    for pred in unit.function.predecessors(chain.entry) {
        let term = unit.function.terminator(pred).expect("a terminator");
        for at in 0..unit.function.instruction(term).operands.len() {
            if unit.function.instruction(term).operands[at] == Operand::Block(chain.entry) {
                unit.function.set_operand(term, at, Operand::Block(next));
            }
        }
    }

    // `DeleteDeadBlocks`.
    let dead: Vec<BlockId> = chain.merged.iter().flatten().map(|one| one.block).collect();
    for &block in &dead {
        let term = unit.function.terminator(block).expect("a terminator");
        let targets: Vec<BlockId> = unit
            .function
            .instruction(term)
            .operands
            .iter()
            .filter_map(|operand| if let Operand::Block(target) = operand { Some(*target) } else { None })
            .filter(|target| !dead.contains(target))
            .collect();
        for target in targets {
            unit.function.predecessor_removed(unit.context, target, block, false);
        }
    }
    let insts: Vec<InstId> = dead.iter().flat_map(|&block| unit.function.iter_block(block)).collect();
    for &inst in &insts {
        if let Some(result) = unit.function.instruction(inst).result
            && !unit.function.users(result).is_empty()
        {
            let ty = unit.function.instruction(inst).ty;
            let poison = unit.context.constant(Constant { ty, kind: ConstantKind::Poison });
            unit.function.replace_all_uses_with(result, Operand::Constant(poison));
        }
    }
    for inst in insts {
        unit.function.erase(inst).expect("its uses were replaced");
    }
    unit.function.erase_blocks(&dead).expect("nothing names a dead block");
}

/// `MergeICmpsPass`.
#[derive(Default)]
pub struct MergeICmps {
    routines: Vec<Routine>,
}

impl MergeICmps {
    /// The pass over a compile that states the library `routines` its program
    /// links (`isLibFuncEmittable(memcmp)`).
    pub fn with_routines(routines: Vec<Routine>) -> Self {
        Self { routines }
    }
}

impl FunctionPass for MergeICmps {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "mergeicmps"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("mergeicmps")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        // Only where the target expands `memcmp` later: no small chain becomes
        // a call.
        let opt_size = unit
            .function
            .attrs
            .iter()
            .any(|attr| matches!(attr, Attribute::Flag(name) if name == "optsize" || name == "minsize"));
        let expands = outer.target().tti().is_some_and(|tti| tti.enable_mem_cmp_expansion(opt_size, true).is_some_and(|options| !options.load_sizes.is_empty()));
        if !expands || !self.routines.iter().any(|routine| routine.standard == "memcmp") {
            return PreservedAnalyses::all();
        }
        let size_bits = unit.layout.pointer(0).index_bits;
        let int = unit.context.types.int(target_library_info(&outer, size_bits).int_bits());
        let size = unit.context.types.int(size_bits);
        let emitter = Emit { routines: &self.routines, int, size };
        let mut changed = false;
        // The entry block has no phi worth looking at (`drop_begin(F)`).
        let blocks: Vec<BlockId> = unit.function.layout().iter().skip(1).copied().collect();
        for block in blocks {
            // A phi is always first in its block.
            let Some(phi) = unit.function.block(block).first() else { continue };
            if unit.function.instruction(phi).opcode != Opcode::Phi {
                continue;
            }
            let chain = {
                let env = Env {
                    context: unit.context,
                    layout: unit.layout,
                    globals: &outer.globals,
                    callees: outer.callees(),
                    metadata: &outer.metadata,
                    sizes: OnceCell::new(),
                };
                analyze_phi(&env, unit.function, phi)
            };
            if let Some(chain) = chain {
                simplify(unit, &emitter, &chain, phi);
                changed = true;
            }
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

#[cfg(test)]
#[path = "mergeicmps_tests.rs"]
mod tests;
