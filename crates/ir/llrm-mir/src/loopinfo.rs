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
    /// `isLoopSimplifyForm`, once asked: true for as long as the graph is as
    /// it was when the loops were found.
    simple: std::cell::Cell<Option<bool>>,
}

thread_local! {
    static EXIT_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times a loop's form was worked out on this thread: a test counts
/// what a pass asks afresh.
pub fn simplify_form_scans() -> usize {
    EXIT_SCANS.with(std::cell::Cell::get)
}

/// `LoopInfoBase<BlockT, LoopT>`, over blocks `B`.
#[derive(Clone, Debug)]
pub struct LoopInfo<B = BlockId> {
    /// `BBMap`: each block's innermost loop, by block number.
    bb_map: Vec<u32>,
    loops: Vec<LoopData>,
    /// `TopLevelLoops`.
    top_level: Vec<LoopId>,
    /// `BlockLayout`.
    layout: Vec<B>,
}

/// `LoopInfoBase::verify` compares each loop's header, parent and set of
/// blocks, not the order they were found in: swapping a branch's arms walks the
/// same CFG in another order, and numbers the loops in another order.
impl<B: BlockNumber> PartialEq for LoopInfo<B> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.by_header() == other.by_header()
    }
}

impl<B: BlockNumber> LoopInfo<B> {
    /// Each loop named by its header: its parent's header, its sub-loops'
    /// headers and its other blocks, sorted; each block's innermost loop's
    /// header; the top-level loops' headers.
    #[allow(clippy::type_complexity)]
    fn by_header(
        &self
    ) -> (std::collections::BTreeMap<usize, (Option<usize>, Vec<usize>, Vec<usize>)>, Vec<Option<usize>>, Vec<usize>)
    {
        let header = |id: LoopId| {
            let one = &self.loops[id.0 as usize];
            self.layout[one.start as usize].number()
        };
        let loops = self
            .loops
            .iter()
            .enumerate()
            .map(|(at, one)| {
                let mut subs: Vec<usize> = one.sub_loops.iter().map(|&sub| header(sub)).collect();
                subs.sort_unstable();
                let mut rest: Vec<usize> = self.layout[one.start as usize + 1..(one.start + one.len) as usize]
                    .iter()
                    .map(|&block| block.number())
                    .collect();
                rest.sort_unstable();
                (header(LoopId(at as u32)), (one.parent.map(header), subs, rest))
            })
            .collect();
        let innermost = self.bb_map.iter().map(|&at| (at != u32::MAX).then(|| header(LoopId(at)))).collect();
        let mut top: Vec<usize> = self.top_level.iter().map(|&one| header(one)).collect();
        top.sort_unstable();
        (loops, innermost, top)
    }
}

impl<B> Default for LoopInfo<B> {
    fn default() -> Self {
        Self { bb_map: Vec::new(), loops: Vec::new(), top_level: Vec::new(), layout: Vec::new() }
    }
}

/// `GraphTraits<const BlockT *>::getNumber`: dense, below the graph's
/// `max_number`.
pub trait BlockNumber: Copy + Eq + std::fmt::Debug {
    fn number(self) -> usize;
}

impl BlockNumber for BlockId {
    fn number(self) -> usize {
        self.0 as usize
    }
}

/// `GraphTraits<BlockT *>` and `GraphTraits<Inverse<BlockT *>>` of a function's
/// blocks, with `getNumber`: all `LoopInfoBase` asks of its blocks.
pub trait LoopGraph {
    type Block: BlockNumber;
    /// `GraphTraits<ParentT>::getEntryNode`.
    fn entry(&self) -> Option<Self::Block>;
    /// `GraphTraits<ParentT>::getMaxNumber`: one past the largest number.
    fn max_number(&self) -> usize;
    /// The child of `block` at or after `*cursor`, the cursor past it: repeats
    /// kept, as `children<BlockT *>` gives them.
    fn child(
        &self,
        block: Self::Block,
        cursor: &mut usize,
    ) -> Option<Self::Block>;
    /// `inverse_children<BlockT *>`: one per edge.
    fn inverse_children(
        &self,
        block: Self::Block,
    ) -> impl Iterator<Item = Self::Block>;
    /// `children<BlockT *>`.
    fn children(
        &self,
        block: Self::Block,
    ) -> impl Iterator<Item = Self::Block> {
        let mut cursor = 0;
        std::iter::from_fn(move || self.child(block, &mut cursor))
    }
    /// `detail::isLegalToHoistInto`: `BasicBlock::isLegalToHoistInto` where the
    /// block type has it, else false.
    fn is_legal_to_hoist_into(
        &self,
        _block: Self::Block,
    ) -> bool {
        false
    }
}

/// What `analyzeImpl` asks of the dominator tree, for a re-entered cycle: the
/// `DomTree.getNode` and `dominates` of its loop reduction.
pub trait Dominance<B> {
    fn is_reachable(
        &self,
        block: B,
    ) -> bool;
    fn dominates(
        &self,
        dominator: B,
        block: B,
    ) -> bool;
}

impl Dominance<BlockId> for DomTree {
    fn is_reachable(
        &self,
        block: BlockId,
    ) -> bool {
        self.is_reachable_from_entry(block)
    }

    fn dominates(
        &self,
        dominator: BlockId,
        block: BlockId,
    ) -> bool {
        DomTree::dominates(self, dominator, block)
    }
}

impl<D: Dominance<BlockId> + ?Sized> Dominance<BlockId> for std::rc::Rc<D> {
    fn is_reachable(
        &self,
        block: BlockId,
    ) -> bool {
        (**self).is_reachable(block)
    }

    fn dominates(
        &self,
        dominator: BlockId,
        block: BlockId,
    ) -> bool {
        (**self).dominates(dominator, block)
    }
}

impl<D: Dominance<BlockId> + ?Sized> Dominance<BlockId> for &D {
    fn is_reachable(
        &self,
        block: BlockId,
    ) -> bool {
        (**self).is_reachable(block)
    }

    fn dominates(
        &self,
        dominator: BlockId,
        block: BlockId,
    ) -> bool {
        (**self).dominates(dominator, block)
    }
}

/// MIR's `GraphTraits<Function *>`: the terminator's block operands with
/// repeats, as `children<BasicBlock *>` gives them; predecessors one per
/// edge, as `inverse_children` gives them.
impl LoopGraph for Function {
    type Block = BlockId;

    fn entry(&self) -> Option<BlockId> {
        Function::entry(self)
    }

    fn max_number(&self) -> usize {
        self.blocks.len()
    }

    fn child(
        &self,
        block: BlockId,
        cursor: &mut usize,
    ) -> Option<BlockId> {
        let operands = self.instruction(self.terminator(block)?).operands.as_slice();
        while let Some(operand) = operands.get(*cursor) {
            *cursor += 1;
            if let Operand::Block(target) = operand {
                return Some(*target);
            }
        }
        None
    }

    fn children(
        &self,
        block: BlockId,
    ) -> impl Iterator<Item = BlockId> {
        // The terminator is found once, not once per edge as `child` does.
        let operands = self.terminator(block).map(|one| self.instruction(one).operands.as_slice()).unwrap_or_default();
        operands
            .iter()
            .filter_map(
                |operand| match operand {
                    Operand::Block(target) => Some(*target),
                    _ => None,
                },
            )
    }

    fn inverse_children(
        &self,
        block: BlockId,
    ) -> impl Iterator<Item = BlockId> {
        self.block_users(block)
            .iter()
            .filter(|one| self.instruction(one.user).opcode.is_terminator())
            .filter_map(|one| self.parent(one.user))
    }

    /// `BasicBlock::isLegalToHoistInto`: not across a terminator that does
    /// more than branch (`Instruction::isSpecialTerminator`; `invoke` is the
    /// one MIR has with successors).
    fn is_legal_to_hoist_into(
        &self,
        block: BlockId,
    ) -> bool {
        self.terminator(block).is_none_or(|term| !matches!(self.instruction(term).opcode, Opcode::Invoke(_)))
    }
}

/// `children<BasicBlock *>(block)`: repeats kept.
pub fn children(
    function: &Function,
    block: BlockId,
) -> impl Iterator<Item = BlockId> + '_ {
    let mut cursor = 0;
    std::iter::from_fn(move || LoopGraph::child(function, block, &mut cursor))
}

/// `inverse_children<BasicBlock *>(block)`: one per edge.
pub fn inverse_children(
    function: &Function,
    block: BlockId,
) -> impl Iterator<Item = BlockId> + '_ {
    LoopGraph::inverse_children(function, block)
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

impl<B: BlockNumber> LoopInfo<B> {
    /// `LoopInfoBase::analyze`, with `dom_tree` asked for only if a loop is
    /// entered other than through its header.
    pub fn analyze<D: Dominance<B>>(
        graph: &impl LoopGraph<Block = B>,
        dom_tree: impl FnOnce() -> D,
    ) -> Self {
        crate::counts::bump(crate::counts::Event::Forests);
        let mut found = Self::default();
        found.analyze_impl(graph, dom_tree);
        found
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericLoopInfoImpl.h#L488
    fn analyze_impl<D: Dominance<B>>(
        &mut self,
        graph: &impl LoopGraph<Block = B>,
        dom_tree: impl FnOnce() -> D,
    ) {
        let Some(entry) = graph.entry() else { return };
        let max_number = graph.max_number();
        // (Pos, LoopHeader) per block number.
        let mut info: Vec<(u32, u32)> = vec![(UNVISITED, 0); max_number];
        let mut headers: Vec<u32> = Vec::new();
        let mut reentries: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut postorder: Vec<B> = Vec::with_capacity(max_number);
        let mut stack: Vec<(B, usize)> = Vec::new();
        let mut counter = FIRST_ON_PATH;
        let num = |block: B| block.number() as u32;
        let mut open = |bb: B, info: &mut Vec<(u32, u32)>, stack: &mut Vec<(B, usize)>| {
            visit();
            info[num(bb) as usize] = (counter, NO_BLOCK);
            counter += 1;
            stack.push((bb, 0));
        };

        open(entry, &mut info, &mut stack);
        while let Some(&(top, _)) = stack.last() {
            let cursor = &mut stack.last_mut().expect("nonempty").1;
            let Some(b1p) = graph.child(top, cursor) else {
                let b0 = num(top);
                info[b0 as usize].0 = OFF_PATH;
                postorder.push(top);
                stack.pop();
                let enclosing = info[b0 as usize].1;
                if let Some(&(parent, _)) = stack.last()
                    && enclosing != NO_BLOCK
                {
                    tag_loop_header(&mut info, num(parent), enclosing);
                }
                continue;
            };
            let b1 = num(b1p);
            if info[b1 as usize].0 == UNVISITED {
                open(b1p, &mut info, &mut stack);
            } else if info[b1 as usize].0 >= FIRST_ON_PATH {
                headers.push(b1);
                tag_loop_header(&mut info, num(top), b1);
            } else {
                let mut h = info[b1 as usize].1;
                while h != NO_BLOCK {
                    if info[h as usize].0 >= FIRST_ON_PATH {
                        tag_loop_header(&mut info, num(top), h);
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
            let mut mark = vec![NO_BLOCK; max_number];
            let mut worklist: Vec<B> = Vec::new();
            let mut first_child = vec![NO_BLOCK; max_number];
            let mut next_sibling = vec![NO_BLOCK; max_number];
            let mut blocks: Vec<Option<B>> = vec![None; max_number];
            for &bb in &postorder {
                let b = num(bb);
                blocks[b as usize] = Some(bb);
                let p = info[b as usize].1;
                if p != NO_BLOCK {
                    next_sibling[b as usize] = first_child[p as usize];
                    first_child[p as usize] = b;
                }
            }
            for &header in &postorder {
                let h = num(header);
                if info[h as usize].0 != IS_REENTERED {
                    continue;
                }
                mark[h as usize] = h;
                worklist.clear();
                let enqueue = |pred: B, mark: &mut Vec<u32>, worklist: &mut Vec<B>, info: &[(u32, u32)]| {
                    let mut p = num(pred);
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
                for pred in graph.inverse_children(header) {
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
                    for pred in graph.inverse_children(at) {
                        if blocks[num(pred) as usize].is_some() {
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
        let mut loop_headers: Vec<B> = Vec::new();
        self.bb_map = vec![NONE; max_number];
        for &bb in postorder.iter().rev() {
            let b = num(bb) as usize;
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
                    simple: std::cell::Cell::new(None),
                });
                loop_headers.push(bb);
            }
            self.bb_map[b] = l;
        }

        // Record each in-loop block with its innermost loop in forward CFG
        // postorder, and build the loop list in PO.
        let mut po: Vec<(B, u32)> = Vec::with_capacity(postorder.len());
        let mut block_len = vec![0u32; self.loops.len()];
        for &bb in &postorder {
            let l = self.bb_map[num(bb) as usize];
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

        self.layout = vec![entry; po.len()];
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

impl<B: BlockNumber> LoopInfo<B> {
    /// `lookupLoopFor`: the innermost loop holding `block`.
    fn lookup(
        &self,
        block: B,
    ) -> Option<LoopId> {
        self.bb_map.get(block.number()).copied().filter(|&one| one != NONE).map(LoopId)
    }

    /// `getLoopFor`.
    pub fn get_loop_for(
        &self,
        block: B,
    ) -> Option<Loop<'_, B>> {
        self.lookup(block).map(|id| Loop { info: self, id })
    }

    /// `getLoopDepth`: 0 outside any loop.
    pub fn get_loop_depth(
        &self,
        block: B,
    ) -> u32 {
        self.get_loop_for(block).map_or(0, |one| one.get_loop_depth())
    }
}

impl LoopInfo<BlockId> {
    /// `Loop::addBasicBlockToLoop`
    /// ([L283](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericLoopInfoImpl.h#L283)),
    /// for `new_blocks` at once: each is a new member of `loop_id`, and so of
    /// every loop around it; `getLoopFor` names `loop_id`. A block already in a
    /// loop, or the header, is not valid here.
    ///
    /// The blocks go at the end of the slice of `loop_id` in the block layout,
    /// which lies inside the slices of its parents. The pin pushes onto each
    /// parent's block vector (O(depth)); a slice of one flat layout needs a
    /// splice, so the cost is the layout plus the loops, once per call, not per
    /// block: pass all the blocks of one loop together.
    pub fn add_basic_blocks_to_loop(
        &mut self,
        loop_id: LoopId,
        new_blocks: &[BlockId],
    ) {
        if new_blocks.is_empty() {
            return;
        }
        let count = new_blocks.len() as u32;
        let highest = new_blocks.iter().map(|one| one.0 as usize).max().expect("a block");
        if highest >= self.bb_map.len() {
            self.bb_map.resize(highest + 1, NONE);
        }
        for block in new_blocks {
            debug_assert_eq!(self.bb_map[block.0 as usize], NONE, "a block already in a loop");
            self.bb_map[block.0 as usize] = loop_id.0;
        }
        let data = &self.loops[loop_id.0 as usize];
        let at = data.start + data.len;
        self.layout.splice(at as usize..at as usize, new_blocks.iter().copied());
        // The loops that hold `at` are `loop_id` and its parents, which grow;
        // a loop that starts at or after it moves. Sub-loops of `loop_id` end
        // at or before `at` and keep their slice.
        let mut around = vec![false; self.loops.len()];
        let mut walk = Some(loop_id);
        while let Some(one) = walk {
            around[one.0 as usize] = true;
            self.loops[one.0 as usize].len += count;
            walk = self.loops[one.0 as usize].parent;
        }
        for (index, one) in self.loops.iter_mut().enumerate() {
            if !around[index] && one.start >= at {
                one.start += count;
            }
        }
    }

    /// `Loop::addBasicBlockToLoop` for one block.
    pub fn add_basic_block_to_loop(
        &mut self,
        loop_id: LoopId,
        new_block: BlockId,
    ) {
        self.add_basic_blocks_to_loop(loop_id, &[new_block]);
    }

    /// Whether `self` and `other` hold the same loops of `function`: the same
    /// innermost loop per block, and per loop the same header, blocks, parent
    /// and sub-loops. The order of the blocks in a loop is not compared: an
    /// update appends, an analysis orders by postorder. For a check of an
    /// updated `LoopInfo` against one made afresh (`LoopInfoBase::verify`).
    pub fn equivalent(
        &self,
        other: &LoopInfo,
        function: &Function,
    ) -> bool {
        let header_of = |info: &LoopInfo, block| info.get_loop_for(block).map(|one| one.get_header());
        if function.layout().iter().any(|&block| header_of(self, block) != header_of(other, block)) {
            return false;
        }
        let summary = |info: &LoopInfo| {
            let mut loops: Vec<_> = info
                .get_loops_in_preorder()
                .iter()
                .map(|one| {
                    let mut blocks = one.get_blocks().to_vec();
                    blocks.sort();
                    let mut subs: Vec<_> = one.get_sub_loops().map(|sub| sub.get_header()).collect();
                    subs.sort();
                    (one.get_header(), blocks, one.get_parent_loop().map(|parent| parent.get_header()), subs)
                })
                .collect();
            loops.sort();
            loops
        };
        summary(self) == summary(other)
    }
}

impl<B: BlockNumber> LoopInfo<B> {
    /// `isLoopHeader`.
    pub fn is_loop_header(
        &self,
        block: B,
    ) -> bool {
        self.get_loop_for(block).is_some_and(|one| one.get_header() == block)
    }

    /// The top-level loops, in reverse program order.
    pub fn top_level_loops(&self) -> impl Iterator<Item = Loop<'_, B>> {
        self.top_level.iter().map(|&id| Loop { info: self, id })
    }

    pub fn is_empty(&self) -> bool {
        self.top_level.is_empty()
    }

    /// `getLoopsInPreorder`: every loop, outer loops first, in program order.
    pub fn get_loops_in_preorder(&self) -> Vec<Loop<'_, B>> {
        let mut found = Vec::new();
        for root in self.top_level_loops().collect::<Vec<_>>().into_iter().rev() {
            found.push(root);
            root.get_inner_loops_in_preorder(&mut found);
        }
        found
    }

    /// `getLoopsInReverseSiblingPreorder`.
    pub fn get_loops_in_reverse_sibling_preorder(&self) -> Vec<Loop<'_, B>> {
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
        a: Option<Loop<'a, B>>,
        b: Option<Loop<'a, B>>,
    ) -> Option<Loop<'a, B>> {
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
        graph: &impl LoopGraph<Block = B>,
        one: Loop<'_, B>,
    ) -> Vec<(B, B)> {
        let mut found = Vec::new();
        for &block in one.get_blocks() {
            for successor in graph.children(block) {
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
        graph: &impl LoopGraph<Block = B>,
        one: Loop<'_, B>,
    ) -> bool {
        one.get_blocks().iter().all(|&block| graph.children(block).all(|successor| one.contains(successor)))
    }

    /// `getUniqueLatchExitBlock`.
    pub fn get_unique_latch_exit_block(
        &self,
        graph: &impl LoopGraph<Block = B>,
        one: Loop<'_, B>,
    ) -> Option<B> {
        let latch = one.get_loop_latch(graph).expect("Latch block must exists");
        single(graph.children(latch).filter(|&successor| !one.contains(successor)), false)
    }

    /// `print`: `Loop info for function 'name':` and the loops with their
    /// nested loops, as `print<loops>` writes them.
    pub fn print(
        &self,
        graph: &impl LoopGraph<Block = B>,
        name: &str,
        block_name: &dyn Fn(B) -> String,
    ) -> String {
        let mut out = format!("Loop info for function '{name}':\n");
        for one in self.top_level_loops() {
            one.print(graph, block_name, 0, &mut out);
        }
        out
    }
}

/// `find_singleton`: the one element, `None` if none or (unless
/// `allow_repeats`) more than one.
fn single<B: Copy + Eq>(
    mut items: impl Iterator<Item = B>,
    allow_repeats: bool,
) -> Option<B> {
    let first = items.next()?;
    items.all(|next| allow_repeats && next == first).then_some(first)
}

impl Loop<'_, BlockId> {
    /// `contains(const InstT *)`.
    pub fn contains_instruction(
        &self,
        function: &Function,
        inst: crate::module::InstId,
    ) -> bool {
        function.parent(inst).is_some_and(|block| self.contains(block))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopInfo.cpp#L198
    /// `getLatchCmpInst`: the `icmp` a conditional branch at the latch's end
    /// tests.
    pub fn get_latch_cmp_inst(
        &self,
        function: &Function,
    ) -> Option<crate::module::InstId> {
        let latch = self.get_loop_latch(function)?;
        let branch = function.terminator(latch)?;
        let instruction = function.instruction(branch);
        if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
            return None;
        }
        let Operand::Value(cond) = instruction.operands[0] else { return None };
        let crate::module::ValueDef::Instruction(inst) = function.value(cond).def else { return None };
        matches!(function.instruction(inst).opcode, Opcode::ICmp(_)).then_some(inst)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopInfo.cpp#L148
    /// `getIncomingAndBackEdge`: the header's two predecessors, the one outside
    /// the loop first.
    pub fn get_incoming_and_back_edge(
        &self,
        function: &Function,
    ) -> Option<(BlockId, BlockId)> {
        let preds = function.predecessors(self.get_header());
        let [mut backedge, mut incoming] = preds[..] else { return None };
        if self.contains(incoming) {
            if self.contains(backedge) {
                return None;
            }
            std::mem::swap(&mut incoming, &mut backedge);
        } else if !self.contains(backedge) {
            return None;
        }
        Some((incoming, backedge))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopInfo.cpp#L174
    /// `getCanonicalInductionVariable`: a header phi that starts at 0 and adds
    /// 1 on the backedge.
    pub fn get_canonical_induction_variable(
        &self,
        context: &crate::context::Context,
        function: &Function,
    ) -> Option<crate::module::InstId> {
        use crate::context::ConstantKind;
        use crate::module::ValueDef;
        use crate::opcode::BinaryOp;
        let (incoming, backedge) = self.get_incoming_and_back_edge(function)?;
        let constant = |operand: Operand| match operand {
            Operand::Constant(id) => match context.get(id).kind {
                ConstantKind::Int(value) => Some(value),
                ConstantKind::Zero => Some(0),
                _ => None,
            },
            _ => None,
        };
        let value_for = |phi: crate::module::InstId, block: BlockId| {
            function
                .instruction(phi)
                .operands
                .chunks(2)
                .find(|pair| pair[1] == Operand::Block(block))
                .map(|pair| pair[0])
        };
        for phi in
            function.iter_block(self.get_header()).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        {
            if value_for(phi, incoming).and_then(constant) != Some(0) {
                continue;
            }
            let Some(Operand::Value(next)) = value_for(phi, backedge) else { continue };
            let ValueDef::Instruction(inc) = function.value(next).def else { continue };
            let inc = function.instruction(inc);
            let this = function.instruction(phi).result.map(Operand::Value);
            if inc.opcode == Opcode::Binary(BinaryOp::Add)
                && Some(inc.operands[0]) == this
                && constant(inc.operands[1]) == Some(1)
            {
                return Some(phi);
            }
        }
        None
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
}

/// A loop of a [`LoopInfo`].
#[derive(Clone, Copy)]
pub struct Loop<'a, B = BlockId> {
    info: &'a LoopInfo<B>,
    id: LoopId,
}

impl<B> PartialEq for Loop<'_, B> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        std::ptr::eq(self.info, other.info) && self.id == other.id
    }
}

impl<B> Eq for Loop<'_, B> {}

impl<B: BlockNumber> std::fmt::Debug for Loop<'_, B> {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(out, "Loop({:?} at {})", self.get_header(), self.id.0)
    }
}

impl<'a, B: BlockNumber> Loop<'a, B> {
    fn data(&self) -> &'a LoopData {
        &self.info.loops[self.id.0 as usize]
    }

    pub fn id(&self) -> LoopId {
        self.id
    }

    /// `getBlocks`: header first, each subloop's blocks contiguous.
    pub fn get_blocks(&self) -> &'a [B] {
        let one = self.data();
        &self.info.layout[one.start as usize..(one.start + one.len) as usize]
    }

    pub fn get_num_blocks(&self) -> usize {
        self.get_blocks().len()
    }

    /// `getHeader`.
    pub fn get_header(&self) -> B {
        self.get_blocks()[0]
    }

    /// `getParentLoop`.
    pub fn get_parent_loop(&self) -> Option<Loop<'a, B>> {
        self.data().parent.map(|id| Loop { info: self.info, id })
    }

    /// `getOutermostLoop`.
    pub fn get_outermost_loop(&self) -> Loop<'a, B> {
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
    pub fn get_sub_loops(&self) -> impl Iterator<Item = Loop<'a, B>> + 'a {
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
        found: &mut Vec<Loop<'a, B>>,
    ) {
        for sub in self.get_sub_loops() {
            found.push(sub);
            sub.get_inner_loops_in_preorder(found);
        }
    }

    /// `contains(const LoopT *)`: this loop or one inside it.
    pub fn contains_loop(
        &self,
        other: Option<Loop<'a, B>>,
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
        block: B,
    ) -> bool {
        self.contains_loop(self.info.get_loop_for(block))
    }

    /// `isLoopExiting`.
    pub fn is_loop_exiting(
        &self,
        graph: &impl LoopGraph<Block = B>,
        block: B,
    ) -> bool {
        debug_assert!(self.contains(block), "Exiting block must be part of the loop");
        graph.children(block).any(|successor| !self.contains(successor))
    }

    /// `isLoopLatch`.
    pub fn is_loop_latch(
        &self,
        graph: &impl LoopGraph<Block = B>,
        block: B,
    ) -> bool {
        debug_assert!(self.contains(block), "block does not belong to the loop");
        graph.inverse_children(self.get_header()).any(|pred| pred == block)
    }

    /// `getNumBackEdges`.
    pub fn get_num_back_edges(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> usize {
        graph.inverse_children(self.get_header()).filter(|&pred| self.contains(pred)).count()
    }

    /// `getLoopLatches`.
    pub fn get_loop_latches(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Vec<B> {
        graph.inverse_children(self.get_header()).filter(|&pred| self.contains(pred)).collect()
    }

    /// `getExitingBlocks`.
    pub fn get_exiting_blocks(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Vec<B> {
        self.get_blocks().iter().copied().filter(|&block| graph.children(block).any(|s| !self.contains(s))).collect()
    }

    /// `getExitingBlock`.
    pub fn get_exiting_block(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Option<B> {
        single(self.get_exiting_blocks(graph).into_iter(), false)
    }

    /// `getExitBlocks`: repeats kept.
    pub fn get_exit_blocks(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Vec<B> {
        self.get_blocks()
            .iter()
            .flat_map(|&block| graph.children(block))
            .filter(|&successor| !self.contains(successor))
            .collect()
    }

    /// `getExitBlock`: the one exit block edge, if exactly one.
    pub fn get_exit_block(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Option<B> {
        single(self.get_exit_blocks(graph).into_iter(), false)
    }

    /// `getUniqueExitBlocks`.
    pub fn get_unique_exit_blocks(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Vec<B> {
        self.unique_exits(graph, |_| true)
    }

    /// `getUniqueNonLatchExitBlocks`.
    pub fn get_unique_non_latch_exit_blocks(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Vec<B> {
        let latch = self.get_loop_latch(graph).expect("Latch block must exists");
        self.unique_exits(graph, |block| block != latch)
    }

    /// `getUniqueExitBlocksHelper`.
    fn unique_exits(
        &self,
        graph: &impl LoopGraph<Block = B>,
        keep: impl Fn(B) -> bool,
    ) -> Vec<B> {
        let mut found: Vec<B> = Vec::new();
        for &block in self.get_blocks().iter().filter(|&&block| keep(block)) {
            for successor in graph.children(block) {
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
        graph: &impl LoopGraph<Block = B>,
    ) -> Option<B> {
        single(self.get_exit_blocks(graph).into_iter(), true)
    }

    /// `isLoopSimplifyForm`: a preheader, one latch and dedicated exits. Worked
    /// out once per loop, for passes that ask it of each loop at every step
    /// and leave the graph as it is (the answer is kept with the loops).
    pub fn is_loop_simplify_form(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> bool {
        if let Some(known) = self.data().simple.get() {
            return known;
        }
        EXIT_SCANS.with(|made| made.set(made.get() + 1));
        let simple = self.get_loop_preheader(graph).is_some()
            && self.get_loop_latch(graph).is_some()
            && self.has_dedicated_exits(graph);
        self.data().simple.set(Some(simple));
        simple
    }

    /// `hasDedicatedExits`.
    pub fn has_dedicated_exits(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> bool {
        self.get_unique_exit_blocks(graph)
            .into_iter()
            .all(|exit| graph.inverse_children(exit).all(|pred| self.contains(pred)))
    }

    /// `getLoopPredecessor`: the one block outside the loop that branches to
    /// the header.
    pub fn get_loop_predecessor(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Option<B> {
        let mut out = None;
        for pred in graph.inverse_children(self.get_header()) {
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
        graph: &impl LoopGraph<Block = B>,
    ) -> Option<B> {
        let out = self.get_loop_predecessor(graph)?;
        if !graph.is_legal_to_hoist_into(out) {
            return None;
        }
        (graph.children(out).count() == 1).then_some(out)
    }

    /// `getLoopLatch`: the one block in the loop that branches to the header.
    pub fn get_loop_latch(
        &self,
        graph: &impl LoopGraph<Block = B>,
    ) -> Option<B> {
        let mut latch = None;
        for pred in graph.inverse_children(self.get_header()) {
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
        graph: &impl LoopGraph<Block = B>,
        block_name: &dyn Fn(B) -> String,
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
            if self.is_loop_latch(graph, block) {
                out.push_str("<latch>");
            }
            if self.is_loop_exiting(graph, block) {
                out.push_str("<exiting>");
            }
        }
        out.push('\n');
        for sub in self.get_sub_loops() {
            sub.print(graph, block_name, depth + 2, out);
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
