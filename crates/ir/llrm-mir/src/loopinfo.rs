//! `LoopInfo` and `Loop`, a translation of LLVM's `GenericLoopInfo.h` and
//! `GenericLoopInfoImpl.h` at `d1106deb71cc`: the loops of a function as a
//! tree, each with its blocks header first, each subloop's blocks contiguous
//! inside its parent's. Found by `analyzeImpl`'s single depth-first search
//! (Wei et al., SAS 2007); the dominator tree is asked only when a loop has a
//! second entry.
//!
//! Top-level loops are in reverse program order and subloops in program
//! order, as LLVM keeps them. Successors are the terminator's block operands
//! with repeats, as `children<BasicBlock *>` gives them; predecessors are one
//! per edge, as `inverse_children` gives them.

use crate::domtree::DomTree;
use crate::module::{BlockId, Function, MetadataNode, MetadataOperand, Operand};
use crate::opcode::Opcode;

thread_local! {
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Blocks `analyze`'s searches have taken up on this thread, for a test of how
/// they grow with the nesting.
pub fn visits() -> usize {
    VISITS.with(std::cell::Cell::get)
}

fn visit() {
    VISITS.with(|visits| visits.set(visits.get() + 1));
}

const NO_BLOCK: u32 = u32::MAX;
const NONE: u32 = u32::MAX;
const UNVISITED: u32 = 0;
const OFF_PATH: u32 = 1;
const IS_HEADER: u32 = 2;
const IS_REENTERED: u32 = 3;
const FIRST_ON_PATH: u32 = 4;

/// A loop of a [`LoopInfo`]: its place in the loops, outer loops first.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct LoopId(pub u32);

#[derive(Clone, Debug, PartialEq)]
struct LoopData {
    parent: Option<LoopId>,
    sub_loops: Vec<LoopId>,
    /// A slice of the `LoopInfo`'s block layout (`BlockLayout`).
    start: u32,
    len: u32,
}

/// `LoopInfoBase<BasicBlock, Loop>`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoopInfo {
    /// `BBMap`: each block's innermost loop, by block number.
    bb_map: Vec<u32>,
    loops: Vec<LoopData>,
    /// `TopLevelLoops`.
    top_level: Vec<LoopId>,
    /// `BlockLayout`.
    layout: Vec<BlockId>,
}

/// The block named by the `cursor`th block operand of `block`'s terminator at
/// or after it, and the cursor past it: `children<BasicBlock *>`.
fn next_child(
    function: &Function,
    block: BlockId,
    cursor: &mut usize,
) -> Option<BlockId> {
    let operands = function.instruction(function.terminator(block)?).operands.as_slice();
    while let Some(operand) = operands.get(*cursor) {
        *cursor += 1;
        if let Operand::Block(target) = operand {
            return Some(*target);
        }
    }
    None
}

/// `children<BasicBlock *>(block)`: repeats kept.
pub fn children(
    function: &Function,
    block: BlockId,
) -> impl Iterator<Item = BlockId> + '_ {
    let mut cursor = 0;
    std::iter::from_fn(move || next_child(function, block, &mut cursor))
}

/// `inverse_children<BasicBlock *>(block)`: one per edge.
pub fn inverse_children(
    function: &Function,
    block: BlockId,
) -> impl Iterator<Item = BlockId> + '_ {
    function
        .block_users(block)
        .iter()
        .filter(|one| function.instruction(one.user).opcode.is_terminator())
        .filter_map(|one| function.parent(one.user))
}

/// `BasicBlock::isLegalToHoistInto`: not across a terminator that does more
/// than branch (`Instruction::isSpecialTerminator`; `invoke` is the one MIR
/// has with successors).
fn is_legal_to_hoist_into(
    function: &Function,
    block: BlockId,
) -> bool {
    function.terminator(block).is_none_or(|term| !matches!(function.instruction(term).opcode, Opcode::Invoke(_)))
}

/// `tagLoopHeader`.
fn tag_loop_header(
    info: &mut [(u32, u32)],
    mut b: u32,
    mut h: u32,
) {
    debug_assert!(h != NO_BLOCK);
    while b != h {
        let ih = info[b as usize].1;
        if ih == NO_BLOCK {
            info[b as usize].1 = h;
            return;
        }
        if info[ih as usize].0 >= info[h as usize].0 {
            b = ih;
        } else {
            info[b as usize].1 = h;
            b = h;
            h = ih;
        }
    }
}

impl LoopInfo {
    /// `LoopInfoBase::analyze`, with `dom_tree` asked for only if a loop is
    /// entered other than through its header.
    pub fn analyze<D: std::borrow::Borrow<DomTree>>(
        function: &Function,
        dom_tree: impl FnOnce() -> D,
    ) -> Self {
        crate::counts::bump(crate::counts::Event::Forests);
        let mut found = Self::default();
        found.analyze_impl(function, dom_tree);
        found
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericLoopInfoImpl.h#L488
    fn analyze_impl<D: std::borrow::Borrow<DomTree>>(
        &mut self,
        function: &Function,
        dom_tree: impl FnOnce() -> D,
    ) {
        let Some(entry) = function.entry() else { return };
        let max_number = function.blocks.len();
        // (Pos, LoopHeader) per block number.
        let mut info: Vec<(u32, u32)> = vec![(UNVISITED, 0); max_number];
        let mut headers: Vec<u32> = Vec::new();
        let mut reentries: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut postorder: Vec<BlockId> = Vec::with_capacity(max_number);
        let mut stack: Vec<(BlockId, usize)> = Vec::new();
        let mut counter = FIRST_ON_PATH;
        let mut open = |bb: BlockId, info: &mut Vec<(u32, u32)>, stack: &mut Vec<(BlockId, usize)>| {
            visit();
            info[bb.0 as usize] = (counter, NO_BLOCK);
            counter += 1;
            stack.push((bb, 0));
        };

        open(entry, &mut info, &mut stack);
        while let Some(&(top, _)) = stack.last() {
            let cursor = &mut stack.last_mut().expect("nonempty").1;
            let Some(b1p) = next_child(function, top, cursor) else {
                let b0 = top.0;
                info[b0 as usize].0 = OFF_PATH;
                postorder.push(top);
                stack.pop();
                let enclosing = info[b0 as usize].1;
                if let Some(&(parent, _)) = stack.last()
                    && enclosing != NO_BLOCK
                {
                    tag_loop_header(&mut info, parent.0, enclosing);
                }
                continue;
            };
            let b1 = b1p.0;
            if info[b1 as usize].0 == UNVISITED {
                open(b1p, &mut info, &mut stack);
            } else if info[b1 as usize].0 >= FIRST_ON_PATH {
                headers.push(b1);
                tag_loop_header(&mut info, top.0, b1);
            } else {
                let mut h = info[b1 as usize].1;
                while h != NO_BLOCK {
                    if info[h as usize].0 >= FIRST_ON_PATH {
                        tag_loop_header(&mut info, top.0, h);
                        break;
                    }
                    reentries.insert(h);
                    h = info[h as usize].1;
                }
            }
        }
        if headers.is_empty() {
            return;
        }
        for &h in &headers {
            info[h as usize].0 = IS_HEADER;
        }

        if !reentries.is_empty() {
            for &h in &reentries {
                info[h as usize].0 = IS_REENTERED;
            }
            let tree = dom_tree();
            let tree = tree.borrow();
            let mut mark = vec![NO_BLOCK; max_number];
            let mut worklist: Vec<BlockId> = Vec::new();
            let mut first_child = vec![NO_BLOCK; max_number];
            let mut next_sibling = vec![NO_BLOCK; max_number];
            let mut blocks: Vec<Option<BlockId>> = vec![None; max_number];
            for &bb in &postorder {
                let b = bb.0;
                blocks[b as usize] = Some(bb);
                let p = info[b as usize].1;
                if p != NO_BLOCK {
                    next_sibling[b as usize] = first_child[p as usize];
                    first_child[p as usize] = b;
                }
            }
            for &header in &postorder {
                let h = header.0;
                if info[h as usize].0 != IS_REENTERED {
                    continue;
                }
                mark[h as usize] = h;
                worklist.clear();
                let enqueue = |pred: BlockId, mark: &mut Vec<u32>, worklist: &mut Vec<BlockId>, info: &[(u32, u32)]| {
                    let mut p = pred.0;
                    let mut pred = pred;
                    let mut a = p;
                    while a != NO_BLOCK {
                        if info[a as usize].1 == h {
                            p = a;
                            pred = blocks[a as usize].expect("a postorder block");
                            break;
                        }
                        a = info[a as usize].1;
                    }
                    if mark[p as usize] == h {
                        return;
                    }
                    mark[p as usize] = h;
                    worklist.push(pred);
                };
                let mut has_backedge = false;
                for pred in inverse_children(function, header) {
                    if tree.is_reachable(pred) && tree.dominates(header, pred) {
                        has_backedge = true;
                        enqueue(pred, &mut mark, &mut worklist, &info);
                    }
                }
                let mut i = 0;
                while i != worklist.len() {
                    let at = worklist[i];
                    i += 1;
                    visit();
                    for pred in inverse_children(function, at) {
                        if blocks[pred.0 as usize].is_some() {
                            enqueue(pred, &mut mark, &mut worklist, &info);
                        }
                    }
                }
                info[h as usize].0 = if has_backedge { IS_HEADER } else { OFF_PATH };
                let parent = info[h as usize].1;
                let mut kept = NO_BLOCK;
                let mut b = first_child[h as usize];
                while b != NO_BLOCK {
                    let next = next_sibling[b as usize];
                    if mark[b as usize] == h {
                        next_sibling[b as usize] = kept;
                        kept = b;
                    } else {
                        info[b as usize].1 = parent;
                        if parent != NO_BLOCK {
                            next_sibling[b as usize] = first_child[parent as usize];
                            first_child[parent as usize] = b;
                        }
                    }
                    b = next;
                }
                first_child[h as usize] = kept;
            }
            if !headers.iter().any(|&h| info[h as usize].0 == IS_HEADER) {
                return;
            }
        }

        // Resolve the chains in reverse postorder.
        let mut loop_headers: Vec<BlockId> = Vec::new();
        self.bb_map = vec![NONE; max_number];
        for &bb in postorder.iter().rev() {
            let b = bb.0 as usize;
            let h = info[b].1;
            let enclosing = if h == NO_BLOCK { NONE } else { self.bb_map[h as usize] };
            let mut l = enclosing;
            if info[b].0 == IS_HEADER {
                l = self.loops.len() as u32;
                self.loops.push(LoopData {
                    parent: (enclosing != NONE).then_some(LoopId(enclosing)),
                    sub_loops: Vec::new(),
                    start: 0,
                    len: 0,
                });
                loop_headers.push(bb);
            }
            self.bb_map[b] = l;
        }

        // Record each in-loop block with its innermost loop in forward CFG
        // postorder, and build the loop list in PO.
        let mut po: Vec<(BlockId, u32)> = Vec::with_capacity(postorder.len());
        let mut block_len = vec![0u32; self.loops.len()];
        for &bb in &postorder {
            let l = self.bb_map[bb.0 as usize];
            if l == NONE {
                continue;
            }
            po.push((bb, l));
            block_len[l as usize] += 1;
            if bb != loop_headers[l as usize] {
                continue;
            }
            match self.loops[l as usize].parent {
                Some(parent) => block_len[parent.0 as usize] += block_len[l as usize],
                None => self.top_level.push(LoopId(l)),
            }
        }
        debug_assert!(!self.top_level.is_empty(), "discovered loops but found no header");

        self.layout = vec![BlockId(0); po.len()];
        let mut root_cursor = 0u32;
        let mut start = vec![0u32; self.loops.len()];
        let mut capacity = vec![0u32; self.loops.len()];
        let mut carved = vec![false; self.loops.len()];
        for &(bb, l) in po.iter().rev() {
            let li = l as usize;
            if !carved[li] {
                carved[li] = true;
                if let Some(parent) = self.loops[li].parent {
                    let p = parent.0 as usize;
                    debug_assert!(carved[p], "parent slice not carved before child");
                    start[li] = start[p] + capacity[p];
                    capacity[p] += block_len[li];
                    self.loops[p].sub_loops.push(LoopId(l));
                } else {
                    start[li] = root_cursor;
                    root_cursor += block_len[li];
                }
            }
            self.layout[(start[li] + capacity[li]) as usize] = bb;
            capacity[li] += 1;
        }
        for (li, one) in self.loops.iter_mut().enumerate() {
            debug_assert_eq!(capacity[li], block_len[li], "layout slice not fully used");
            (one.start, one.len) = (start[li], block_len[li]);
        }
    }
}

impl LoopInfo {
    /// `lookupLoopFor`: the innermost loop holding `block`.
    fn lookup(
        &self,
        block: BlockId,
    ) -> Option<LoopId> {
        self.bb_map.get(block.0 as usize).copied().filter(|&one| one != NONE).map(LoopId)
    }

    /// `getLoopFor`.
    pub fn get_loop_for(
        &self,
        block: BlockId,
    ) -> Option<Loop<'_>> {
        self.lookup(block).map(|id| Loop { info: self, id })
    }

    /// `getLoopDepth`: 0 outside any loop.
    pub fn get_loop_depth(
        &self,
        block: BlockId,
    ) -> u32 {
        self.get_loop_for(block).map_or(0, |one| one.get_loop_depth())
    }

    /// `isLoopHeader`.
    pub fn is_loop_header(
        &self,
        block: BlockId,
    ) -> bool {
        self.get_loop_for(block).is_some_and(|one| one.get_header() == block)
    }

    /// The top-level loops, in reverse program order.
    pub fn top_level_loops(&self) -> impl Iterator<Item = Loop<'_>> {
        self.top_level.iter().map(|&id| Loop { info: self, id })
    }

    pub fn is_empty(&self) -> bool {
        self.top_level.is_empty()
    }

    /// `getLoopsInPreorder`: every loop, outer loops first, in program order.
    pub fn get_loops_in_preorder(&self) -> Vec<Loop<'_>> {
        let mut found = Vec::new();
        for root in self.top_level_loops().collect::<Vec<_>>().into_iter().rev() {
            found.push(root);
            root.get_inner_loops_in_preorder(&mut found);
        }
        found
    }

    /// `getLoopsInReverseSiblingPreorder`.
    pub fn get_loops_in_reverse_sibling_preorder(&self) -> Vec<Loop<'_>> {
        let mut found = Vec::new();
        for root in self.top_level_loops() {
            let mut pending = vec![root];
            while let Some(one) = pending.pop() {
                pending.extend(one.get_sub_loops());
                found.push(one);
            }
        }
        found
    }

    /// `getSmallestCommonLoop`.
    pub fn get_smallest_common_loop<'a>(
        &'a self,
        a: Option<Loop<'a>>,
        b: Option<Loop<'a>>,
    ) -> Option<Loop<'a>> {
        let (mut a, mut b) = (a?, b?);
        let (mut depth_a, mut depth_b) = (a.get_loop_depth(), b.get_loop_depth());
        while depth_a > depth_b {
            a = a.get_parent_loop()?;
            depth_a -= 1;
        }
        while depth_b > depth_a {
            b = b.get_parent_loop()?;
            depth_b -= 1;
        }
        while a != b {
            a = a.get_parent_loop()?;
            b = b.get_parent_loop()?;
        }
        Some(a)
    }

    /// `getExitEdges`: (inside block, outside block) pairs.
    pub fn get_exit_edges(
        &self,
        function: &Function,
        one: Loop<'_>,
    ) -> Vec<(BlockId, BlockId)> {
        let mut found = Vec::new();
        for &block in one.get_blocks() {
            for successor in children(function, block) {
                if !one.contains(successor) {
                    found.push((block, successor));
                }
            }
        }
        found
    }

    /// `hasNoExitBlocks`.
    pub fn has_no_exit_blocks(
        &self,
        function: &Function,
        one: Loop<'_>,
    ) -> bool {
        one.get_blocks().iter().all(|&block| children(function, block).all(|successor| one.contains(successor)))
    }

    /// `getUniqueLatchExitBlock`.
    pub fn get_unique_latch_exit_block(
        &self,
        function: &Function,
        one: Loop<'_>,
    ) -> Option<BlockId> {
        let latch = one.get_loop_latch(function).expect("Latch block must exists");
        single(children(function, latch).filter(|&successor| !one.contains(successor)), false)
    }

    /// `print`: `Loop info for function 'name':` and the loops with their
    /// nested loops, as `print<loops>` writes them.
    pub fn print(
        &self,
        function: &Function,
        name: &str,
        block_name: &dyn Fn(BlockId) -> String,
    ) -> String {
        let mut out = format!("Loop info for function '{name}':\n");
        for one in self.top_level_loops() {
            one.print(function, block_name, 0, &mut out);
        }
        out
    }
}

/// `find_singleton`: the one element, `None` if none or (unless
/// `allow_repeats`) more than one.
fn single(
    mut items: impl Iterator<Item = BlockId>,
    allow_repeats: bool,
) -> Option<BlockId> {
    let first = items.next()?;
    items.all(|next| allow_repeats && next == first).then_some(first)
}

/// A loop of a [`LoopInfo`].
#[derive(Clone, Copy)]
pub struct Loop<'a> {
    info: &'a LoopInfo,
    id: LoopId,
}

impl PartialEq for Loop<'_> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        std::ptr::eq(self.info, other.info) && self.id == other.id
    }
}

impl Eq for Loop<'_> {}

impl std::fmt::Debug for Loop<'_> {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(out, "Loop({:?} at {})", self.get_header(), self.id.0)
    }
}

impl<'a> Loop<'a> {
    fn data(&self) -> &'a LoopData {
        &self.info.loops[self.id.0 as usize]
    }

    pub fn id(&self) -> LoopId {
        self.id
    }

    /// `getBlocks`: header first, each subloop's blocks contiguous.
    pub fn get_blocks(&self) -> &'a [BlockId] {
        let one = self.data();
        &self.info.layout[one.start as usize..(one.start + one.len) as usize]
    }

    pub fn get_num_blocks(&self) -> usize {
        self.get_blocks().len()
    }

    /// `getHeader`.
    pub fn get_header(&self) -> BlockId {
        self.get_blocks()[0]
    }

    /// `getParentLoop`.
    pub fn get_parent_loop(&self) -> Option<Loop<'a>> {
        self.data().parent.map(|id| Loop { info: self.info, id })
    }

    /// `getOutermostLoop`.
    pub fn get_outermost_loop(&self) -> Loop<'a> {
        let mut one = *self;
        while let Some(parent) = one.get_parent_loop() {
            one = parent;
        }
        one
    }

    /// `getLoopDepth`: 1 for an outermost loop.
    pub fn get_loop_depth(&self) -> u32 {
        let mut depth = 1;
        let mut one = self.get_parent_loop();
        while let Some(parent) = one {
            depth += 1;
            one = parent.get_parent_loop();
        }
        depth
    }

    /// `getSubLoops`, in program order.
    pub fn get_sub_loops(&self) -> impl Iterator<Item = Loop<'a>> + 'a {
        let info = self.info;
        self.data().sub_loops.iter().map(move |&id| Loop { info, id })
    }

    pub fn is_innermost(&self) -> bool {
        self.data().sub_loops.is_empty()
    }

    pub fn is_outermost(&self) -> bool {
        self.data().parent.is_none()
    }

    /// `getInnerLoopsInPreorder`.
    pub fn get_inner_loops_in_preorder(
        &self,
        found: &mut Vec<Loop<'a>>,
    ) {
        for sub in self.get_sub_loops() {
            found.push(sub);
            sub.get_inner_loops_in_preorder(found);
        }
    }

    /// `contains(const LoopT *)`: this loop or one inside it.
    pub fn contains_loop(
        &self,
        other: Option<Loop<'a>>,
    ) -> bool {
        let mut at = other;
        while let Some(one) = at {
            if one == *self {
                return true;
            }
            at = one.get_parent_loop();
        }
        false
    }

    /// `contains(const BlockT *)`, by the block-to-loop map.
    pub fn contains(
        &self,
        block: BlockId,
    ) -> bool {
        self.contains_loop(self.info.get_loop_for(block))
    }

    /// `contains(const InstT *)`.
    pub fn contains_instruction(
        &self,
        function: &Function,
        inst: crate::module::InstId,
    ) -> bool {
        function.parent(inst).is_some_and(|block| self.contains(block))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopInfo.cpp#L67
    /// `isLoopInvariant(const Value *)`: an instruction outside the loop, or
    /// anything that is not an instruction.
    pub fn is_loop_invariant(
        &self,
        function: &Function,
        value: crate::module::Operand,
    ) -> bool {
        use crate::module::{Operand, ValueDef};
        match value {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Instruction(inst) => !self.contains_instruction(function, inst),
                ValueDef::Argument(_) => true,
            },
            _ => true,
        }
    }

    /// `isLoopExiting`.
    pub fn is_loop_exiting(
        &self,
        function: &Function,
        block: BlockId,
    ) -> bool {
        debug_assert!(self.contains(block), "Exiting block must be part of the loop");
        children(function, block).any(|successor| !self.contains(successor))
    }

    /// `isLoopLatch`.
    pub fn is_loop_latch(
        &self,
        function: &Function,
        block: BlockId,
    ) -> bool {
        debug_assert!(self.contains(block), "block does not belong to the loop");
        inverse_children(function, self.get_header()).any(|pred| pred == block)
    }

    /// `getNumBackEdges`.
    pub fn get_num_back_edges(
        &self,
        function: &Function,
    ) -> usize {
        inverse_children(function, self.get_header()).filter(|&pred| self.contains(pred)).count()
    }

    /// `getLoopLatches`.
    pub fn get_loop_latches(
        &self,
        function: &Function,
    ) -> Vec<BlockId> {
        inverse_children(function, self.get_header()).filter(|&pred| self.contains(pred)).collect()
    }

    /// `getExitingBlocks`.
    pub fn get_exiting_blocks(
        &self,
        function: &Function,
    ) -> Vec<BlockId> {
        self.get_blocks()
            .iter()
            .copied()
            .filter(|&block| children(function, block).any(|s| !self.contains(s)))
            .collect()
    }

    /// `getExitingBlock`.
    pub fn get_exiting_block(
        &self,
        function: &Function,
    ) -> Option<BlockId> {
        single(self.get_exiting_blocks(function).into_iter(), false)
    }

    /// `getExitBlocks`: repeats kept.
    pub fn get_exit_blocks(
        &self,
        function: &Function,
    ) -> Vec<BlockId> {
        self.get_blocks()
            .iter()
            .flat_map(|&block| children(function, block))
            .filter(|&successor| !self.contains(successor))
            .collect()
    }

    /// `getExitBlock`: the one exit block edge, if exactly one.
    pub fn get_exit_block(
        &self,
        function: &Function,
    ) -> Option<BlockId> {
        single(self.get_exit_blocks(function).into_iter(), false)
    }

    /// `getUniqueExitBlocks`.
    pub fn get_unique_exit_blocks(
        &self,
        function: &Function,
    ) -> Vec<BlockId> {
        self.unique_exits(function, |_| true)
    }

    /// `getUniqueNonLatchExitBlocks`.
    pub fn get_unique_non_latch_exit_blocks(
        &self,
        function: &Function,
    ) -> Vec<BlockId> {
        let latch = self.get_loop_latch(function).expect("Latch block must exists");
        self.unique_exits(function, |block| block != latch)
    }

    /// `getUniqueExitBlocksHelper`.
    fn unique_exits(
        &self,
        function: &Function,
        keep: impl Fn(BlockId) -> bool,
    ) -> Vec<BlockId> {
        let mut found: Vec<BlockId> = Vec::new();
        for &block in self.get_blocks().iter().filter(|&&block| keep(block)) {
            for successor in children(function, block) {
                if !self.contains(successor) && !found.contains(&successor) {
                    found.push(successor);
                }
            }
        }
        found
    }

    /// `getUniqueExitBlock`: the one exit block, if exactly one, however many
    /// edges reach it.
    pub fn get_unique_exit_block(
        &self,
        function: &Function,
    ) -> Option<BlockId> {
        single(self.get_exit_blocks(function).into_iter(), true)
    }

    /// `hasDedicatedExits`.
    pub fn has_dedicated_exits(
        &self,
        function: &Function,
    ) -> bool {
        self.get_unique_exit_blocks(function)
            .into_iter()
            .all(|exit| inverse_children(function, exit).all(|pred| self.contains(pred)))
    }

    /// `getLoopPredecessor`: the one block outside the loop that branches to
    /// the header.
    pub fn get_loop_predecessor(
        &self,
        function: &Function,
    ) -> Option<BlockId> {
        let mut out = None;
        for pred in inverse_children(function, self.get_header()) {
            if !self.contains(pred) {
                if out.is_some_and(|one| one != pred) {
                    return None;
                }
                out = Some(pred);
            }
        }
        out
    }

    /// `getLoopPreheader`.
    pub fn get_loop_preheader(
        &self,
        function: &Function,
    ) -> Option<BlockId> {
        let out = self.get_loop_predecessor(function)?;
        if !is_legal_to_hoist_into(function, out) {
            return None;
        }
        (children(function, out).count() == 1).then_some(out)
    }

    /// `getLoopLatch`: the one block in the loop that branches to the header.
    pub fn get_loop_latch(
        &self,
        function: &Function,
    ) -> Option<BlockId> {
        let mut latch = None;
        for pred in inverse_children(function, self.get_header()) {
            if self.contains(pred) {
                if latch.is_some() {
                    return None;
                }
                latch = Some(pred);
            }
        }
        latch
    }

    /// `LoopBase::print`.
    fn print(
        &self,
        function: &Function,
        block_name: &dyn Fn(BlockId) -> String,
        depth: usize,
        out: &mut String,
    ) {
        use std::fmt::Write;
        let _ = write!(out, "{}Loop at depth {} containing: ", " ".repeat(depth * 2), self.get_loop_depth());
        let header = self.get_header();
        for (at, &block) in self.get_blocks().iter().enumerate() {
            if at > 0 {
                out.push(',');
            }
            let _ = write!(out, "%{}", block_name(block));
            if block == header {
                out.push_str("<header>");
            }
            if self.is_loop_latch(function, block) {
                out.push_str("<latch>");
            }
            if self.is_loop_exiting(function, block) {
                out.push_str("<exiting>");
            }
        }
        out.push('\n');
        for sub in self.get_sub_loops() {
            sub.print(function, block_name, depth + 2, out);
        }
    }
}

/// `print<loops>`'s text for each function of `module` with a body.
pub fn print_loops(module: &crate::module::Module) -> String {
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let info = LoopInfo::analyze(function, || DomTree::of(function));
        let labels = crate::print::block_labels(function);
        out += &info.print(function, global.name.as_deref().unwrap_or("?"), &|block| {
            labels.get(&block).cloned().unwrap_or_default()
        });
    }
    out
}

/// Whether the loop `latch` closes carries `llvm.loop.mustprogress`.
pub fn marked(
    metadata: &[MetadataNode],
    function: &Function,
    latch: BlockId,
) -> bool {
    let Some(branch) = function.terminator(latch) else { return false };
    let Some((_, node)) = function.instruction(branch).metadata.iter().find(|(kind, _)| kind == "llvm.loop") else {
        return false;
    };
    let Some(node) = metadata.get(node.0 as usize) else { return false };
    node.operands
        .iter()
        .any(
            |one| match one {
                MetadataOperand::Node(id) => metadata.get(id.0 as usize).is_some_and(|property| {
                    matches!(
                        property.operands.first(),
                        Some(MetadataOperand::String(name)) if name == "llvm.loop.mustprogress"
                    )
                }),
                _ => false,
            },
        )
}

/// Whether the language promises every loop of `function` ends: each has an
/// edge out, and the function says so of every loop (`whole`) or each loop
/// is marked on all its latches (`llvm.loop.mustprogress`, as C11 6.8.5p6
/// gives only the loops whose controlling expression is not constant:
/// `for (;;)` hangs).
pub fn ends_by_promise(
    metadata: &[MetadataNode],
    function: &Function,
    whole: bool,
) -> bool {
    let info = LoopInfo::analyze(function, || DomTree::of(function));
    info.get_loops_in_preorder()
        .iter()
        .all(
            |one| !info.has_no_exit_blocks(function, *one)
                && (whole || one.get_loop_latches(function).iter().all(|&latch| marked(metadata, function, latch))),
        )
}
