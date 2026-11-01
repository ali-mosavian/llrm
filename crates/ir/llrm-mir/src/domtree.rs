//! `DominatorTreeBase`: the dominator and post-dominator tree, built by
//! Semi-NCA and brought up to date by edge insertions, deletions and batches.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `GenericDomTree.h`,
//! `GenericDomTreeConstruction.h`, `CFGDiff.h` and `CFGUpdate.h`, each function
//! linking the lines it translates. `DomTreeBase<false>` is `DominatorTree`,
//! `DomTreeBase<true>` is `PostDominatorTree` (virtual root, roots of infinite
//! loops). A tree is built from scratch by [`DomTreeBase::recalculate`]; after
//! a CFG change it is kept by [`DomTreeBase::insert_edge`],
//! [`DomTreeBase::delete_edge`] or [`DomTreeBase::apply_updates`]
//! (Ramalingam-Reps depth-based search for insertions, subtree rebuilds for
//! deletions). Children of a node are kept in LLVM's order (DFS order after a
//! build, newest first after an update), so a tree prints as LLVM's prints.
//!
//! DEVIATE (anything MIR or Rust lacks):
//! 1. Nodes are indices into a vector, not pointers from a bump allocator: a
//!    node is addressed by `block number + IsPostDom`, which is what
//!    `getNodeIndex` already computes.
//! 2. No `BlockNumberEpoch`: MIR block ids are never renumbered, so
//!    `updateBlockNumbers` and the epoch asserts have nothing to check.
//! 3. The tree does not hold `Parent`; a method that LLVM answers from it
//!    (`getMaxNumber`, `nodes`, the entry) takes the graph, a [`DomGraph`].
//!    `Parent != Other.Parent` in `compare` is therefore not tested.
//! 4. `verify` returns the message LLVM writes to `errs()`.
//! 5. `LLVM_ENABLE_ABI_BREAKING_CHECKS` code (`VisitedUnaffected`, the
//!    `IsSuccessor` assert) is a `debug_assert`.
//! 6. `std::priority_queue` is a copy of libstdc++'s heap, so ties between
//!    nodes of one level pop as LLVM's do and children list in LLVM's order.
//! 7. `Function::successors` and `predecessors` list a block once, where LLVM's
//!    `children` repeat a block a switch names twice. A set of nodes is the
//!    same; only the order of a post-dominator tree's children can differ. The
//!    test graph lists predecessors newest first, so the golden file of the
//!    pinned LLVM compares byte for byte.
//!
//! 8. A `Function` may have no block, where LLVM's has an entry: its tree is
//!    empty (`DomTree::of`).
//! 10. `NodeInfos` is kept between constructions with every record clear, and
//!     `clear` resets the records the walk wrote, not every block's.
//! 9. The readers' `Dominators` analysis is brought up to date from MIR's
//!    change log, not from calls a pass makes: [`DomTree::updated`] turns the
//!    log into the edge `Update`s `applyUpdates` takes. A pass that leaves a
//!    terminator's block or operands as they were makes no update.
//!
//! # Example
//! A diamond `a -> {b, c} -> d`: `d`'s immediate dominator is `a`.
//!
//! # Used by
//! The `Dominators` analysis (`passes`) and every reader of dominance in
//! `llrm-mir`, `llrm-analysis` and `llrm-transforms`.

use std::cell::{Cell, RefCell};

use crate::blocklist::BlockList;
use crate::dense::{IdMap, IdSet};
use crate::hash::{HashMap, HashSet};
use crate::module::{BlockId, Change, Function, InstId, Operand};

/// The graph a tree is over: LLVM's `GraphTraits<ParentPtr>` and
/// `GraphTraits<NodePtr>` (`getEntryNode`, `getMaxNumber`, `nodes`, `children`
/// and `inverse_children`).
pub trait DomGraph {
    fn entry(&self) -> Option<BlockId>;
    /// One more than the largest block number: `getMaxNumber`.
    fn max_number(&self) -> usize;
    /// Every block, in function order: `nodes(Parent)`.
    fn nodes(&self) -> &[BlockId];
    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList;
    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList;
}

impl DomGraph for Function {
    fn entry(&self) -> Option<BlockId> {
        Function::entry(self)
    }

    fn max_number(&self) -> usize {
        self.block_capacity()
    }

    fn nodes(&self) -> &[BlockId] {
        self.layout()
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        Function::successors(self, block)
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        Function::predecessors(self, block)
    }
}

/// A block, or the virtual root of a post-dominator tree.
type Bb = Option<BlockId>;

/// A node of the tree: its position in `DomTreeNodes`.
type Node = u32;

/// No node: LLVM's `nullptr` for a `DomTreeNodeBase *`.
const NO: u32 = u32::MAX;

/// `cfg::UpdateKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateKind {
    Insert,
    Delete,
}

/// `cfg::Update`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/CFGUpdate.h#L28>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Update {
    pub kind: UpdateKind,
    pub from: BlockId,
    pub to: BlockId,
}

impl Update {
    pub fn insert(
        from: BlockId,
        to: BlockId,
    ) -> Self {
        Self { kind: UpdateKind::Insert, from, to }
    }

    pub fn delete(
        from: BlockId,
        to: BlockId,
    ) -> Self {
        Self { kind: UpdateKind::Delete, from, to }
    }
}

/// `cfg::LegalizeUpdates`: drops updates that cancel, orders the rest by the
/// position of the last update of the edge (descending, as the result is
/// popped from the back).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/CFGUpdate.h#L72>
fn legalize_updates(
    all: &[Update],
    inverse_graph: bool,
    reverse_result_order: bool,
) -> Vec<Update> {
    // (insertions minus deletions, index of the last update) by edge.
    let mut operations: HashMap<(BlockId, BlockId), (i32, usize)> = HashMap::default();
    let mut first_seen: Vec<(BlockId, BlockId)> = Vec::new();
    for (at, update) in all.iter().enumerate() {
        let (from, to) = if inverse_graph { (update.to, update.from) } else { (update.from, update.to) };
        let entry = operations
            .entry((from, to))
            .or_insert_with(
                || {
                    first_seen.push((from, to));
                    (0, at)
                },
            );
        entry.0 += if update.kind == UpdateKind::Insert { 1 } else { -1 };
        entry.1 = at;
    }
    let mut result: Vec<(usize, Update)> = Vec::new();
    for edge in first_seen {
        let (count, last) = operations[&edge];
        debug_assert!(count.abs() <= 1, "Unbalanced operations!");
        if count == 0 {
            continue;
        }
        let kind = if count > 0 { UpdateKind::Insert } else { UpdateKind::Delete };
        result.push((last, Update { kind, from: edge.0, to: edge.1 }));
    }
    // The last index of each edge is distinct, so the order is total.
    result.sort_by(|a, b| if reverse_result_order { a.0.cmp(&b.0) } else { b.0.cmp(&a.0) });
    result.into_iter().map(|(_, update)| update).collect()
}

#[derive(Clone, Default)]
struct DeletesInserts {
    di: [Vec<BlockId>; 2],
}

/// `GraphDiff`: a CFG snapshot as a set of edge changes.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/CFGDiff.h#L50>
#[derive(Clone, Default)]
pub struct GraphDiff {
    inverse_graph: bool,
    succ: IdMap<BlockId, DeletesInserts>,
    pred: IdMap<BlockId, DeletesInserts>,
    updated_are_reverse_applied: bool,
    /// Kept in reverse to allow popping from the end.
    legalized: Vec<Update>,
}

impl GraphDiff {
    pub fn new(
        updates: &[Update],
        reverse_apply_updates: bool,
        inverse_graph: bool,
    ) -> Self {
        let mut diff = Self { inverse_graph, ..Self::default() };
        diff.legalized = legalize_updates(updates, inverse_graph, false);
        for update in &diff.legalized {
            let is_insert = usize::from((update.kind == UpdateKind::Insert) == !reverse_apply_updates);
            diff.succ.get_or_insert_with(update.from, DeletesInserts::default).di[is_insert].push(update.to);
            diff.pred.get_or_insert_with(update.to, DeletesInserts::default).di[is_insert].push(update.from);
        }
        diff.updated_are_reverse_applied = reverse_apply_updates;
        diff
    }

    pub fn num_legalized_updates(&self) -> usize {
        self.legalized.len()
    }

    /// `popUpdateForIncrementalUpdates`.
    pub fn pop_update_for_incremental_updates(&mut self) -> Update {
        let update = self.legalized.pop().expect("No updates to apply!");
        let is_insert = usize::from((update.kind == UpdateKind::Insert) == !self.updated_are_reverse_applied);
        let list = self.succ.get_mut(&update.from).expect("a legalized update is in the map");
        let popped = list.di[is_insert].pop();
        debug_assert_eq!(popped, Some(update.to));
        if list.di[is_insert].is_empty() && list.di[1 - is_insert].is_empty() {
            self.succ.remove(&update.from);
        }
        let list = self.pred.get_mut(&update.to).expect("a legalized update is in the map");
        let popped = list.di[is_insert].pop();
        debug_assert_eq!(popped, Some(update.from));
        if list.di[is_insert].is_empty() && list.di[1 - is_insert].is_empty() {
            self.pred.remove(&update.to);
        }
        update
    }

    /// `getChildren<InverseEdge>`: the CFG children of `node`, successors
    /// (reversed) or predecessors, with the snapshot's changes.
    fn children(
        &self,
        graph: &impl DomGraph,
        node: BlockId,
        inverse_edge: bool,
    ) -> Vec<BlockId> {
        let mut result: Vec<BlockId> =
            if inverse_edge { graph.predecessors(node).to_vec() } else { reversed(graph.successors(node)) };
        let map = if inverse_edge != self.inverse_graph { &self.pred } else { &self.succ };
        let Some(changes) = map.get(&node) else { return result };
        // Remove children present in the CFG but not in the snapshot.
        for child in &changes.di[0] {
            result.retain(|one| one != child);
        }
        // Add children present in the snapshot for not in the real CFG.
        result.extend_from_slice(&changes.di[1]);
        result
    }
}

fn reversed(list: BlockList) -> Vec<BlockId> {
    let mut out = list.to_vec();
    out.reverse();
    out
}

/// `SemiNCAInfo::BatchUpdateInfo`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L88>
pub struct BatchUpdateInfo {
    /// Updates inside are already legalized.
    pre_view_cfg: RefCell<GraphDiff>,
    post_view_cfg: Option<GraphDiff>,
    num_legalized: usize,
    /// Whether the whole tree was recalculated at some point of the batch.
    is_recalculated: Cell<bool>,
}

impl BatchUpdateInfo {
    fn new(
        pre_view_cfg: GraphDiff,
        post_view_cfg: Option<GraphDiff>,
    ) -> Self {
        let num_legalized = pre_view_cfg.num_legalized_updates();
        Self {
            pre_view_cfg: RefCell::new(pre_view_cfg),
            post_view_cfg,
            num_legalized,
            is_recalculated: Cell::new(false),
        }
    }
}

type Batch<'a> = Option<&'a BatchUpdateInfo>;

/// `DomTreeNodeBase`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L57>
#[derive(Clone, Debug)]
struct TreeNode {
    block: Bb,
    idom: Node,
    level: u32,
    first_child: Node,
    sibling: Node,
    dfs_num_in: Cell<u32>,
    dfs_num_out: Cell<u32>,
}

/// `DominatorTreeBase<NodeT, IsPostDom>`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L263>
#[derive(Clone, Debug)]
pub struct DomTreeBase<const POST: bool> {
    /// Dominators always have a single root, postdominators can have more.
    roots: Vec<BlockId>,
    nodes: Vec<Option<TreeNode>>,
    root_node: Node,
    /// `Parent != nullptr`.
    calculated: bool,
    dfs_info_valid: Cell<bool>,
    slow_queries: Cell<u32>,
}

pub type DomTree = DomTreeBase<false>;
pub type PostDomTree = DomTreeBase<true>;

/// `DominatorTreeBase::VerificationLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationLevel {
    Fast,
    Basic,
    Full,
}

impl<const POST: bool> Default for DomTreeBase<POST> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const POST: bool> DomTreeBase<POST> {
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            nodes: Vec::new(),
            root_node: NO,
            calculated: false,
            dfs_info_valid: Cell::new(false),
            slow_queries: Cell::new(0),
        }
    }

    pub fn is_post_dominator(&self) -> bool {
        POST
    }

    pub fn roots(&self) -> &[BlockId] {
        &self.roots
    }

    /// `getNodeIndex`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L382>
    fn node_index(block: Bb) -> usize {
        match block {
            None => {
                debug_assert!(POST, "dominator tree block must be non-null");
                0
            }
            Some(block) => block.0 as usize + usize::from(POST),
        }
    }

    fn at(
        &self,
        node: Node,
    ) -> &TreeNode {
        self.nodes[node as usize].as_ref().expect("a node of the tree")
    }

    fn at_mut(
        &mut self,
        node: Node,
    ) -> &mut TreeNode {
        self.nodes[node as usize].as_mut().expect("a node of the tree")
    }

    /// `getNode`: the node of `block`, None for a block not in the tree.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L398>
    fn node(
        &self,
        block: Bb,
    ) -> Option<Node> {
        let index = Self::node_index(block);
        match self.nodes.get(index) {
            Some(Some(_)) => Some(index as Node),
            _ => None,
        }
    }

    /// The tree has a node for `block`.
    pub fn contains(
        &self,
        block: BlockId,
    ) -> bool {
        self.node(Some(block)).is_some()
    }

    fn block_of(
        &self,
        node: Node,
    ) -> Bb {
        self.at(node).block
    }

    fn idom_of(
        &self,
        node: Node,
    ) -> Option<Node> {
        let idom = self.at(node).idom;
        (idom != NO).then_some(idom)
    }

    fn level_of(
        &self,
        node: Node,
    ) -> u32 {
        self.at(node).level
    }

    /// `children()` of a node, first child first.
    fn children_of(
        &self,
        node: Node,
    ) -> Vec<Node> {
        let mut out = Vec::new();
        let mut child = self.at(node).first_child;
        while child != NO {
            out.push(child);
            child = self.at(child).sibling;
        }
        out
    }

    /// `DomTreeNodeBase::addChild`.
    fn add_child(
        &mut self,
        parent: Node,
        child: Node,
    ) {
        debug_assert!(self.at(child).sibling == NO, "cannot add child that already has siblings");
        let first = self.at(parent).first_child;
        self.at_mut(child).sibling = first;
        self.at_mut(parent).first_child = child;
    }

    /// `DomTreeNodeBase::removeChild`.
    fn remove_child(
        &mut self,
        parent: Node,
        child: Node,
    ) {
        let after = self.at(child).sibling;
        if self.at(parent).first_child == child {
            self.at_mut(parent).first_child = after;
        } else {
            let mut at = self.at(parent).first_child;
            while self.at(at).sibling != child {
                debug_assert!(at != NO, "Not in immediate dominator children list!");
                at = self.at(at).sibling;
            }
            self.at_mut(at).sibling = after;
        }
        self.at_mut(child).sibling = NO;
    }

    /// `DomTreeNodeBase::setIDom`.
    fn set_idom(
        &mut self,
        node: Node,
        new_idom: Node,
    ) {
        let idom = self.at(node).idom;
        debug_assert!(idom != NO, "No immediate dominator?");
        if idom == new_idom {
            return;
        }
        self.remove_child(idom, node);
        // Switch to new dominator.
        self.at_mut(node).idom = new_idom;
        self.add_child(new_idom, node);
        self.update_level(node);
    }

    /// `DomTreeNodeBase::UpdateLevel`.
    fn update_level(
        &mut self,
        node: Node,
    ) {
        let idom = self.at(node).idom;
        debug_assert!(idom != NO);
        if self.at(node).level == self.at(idom).level + 1 {
            return;
        }
        let mut work_stack = vec![node];
        while let Some(current) = work_stack.pop() {
            let above = self.at(self.at(current).idom).level;
            self.at_mut(current).level = above + 1;
            let mut child = self.at(current).first_child;
            while child != NO {
                debug_assert!(self.at(child).idom != NO);
                if self.at(child).level != self.at(self.at(child).idom).level + 1 {
                    work_stack.push(child);
                }
                child = self.at(child).sibling;
            }
        }
    }

    /// `DomTreeNodeBase::DominatedBy`: only where the DFS numbers are valid.
    fn dominated_by(
        &self,
        node: Node,
        other: Node,
    ) -> bool {
        let (this, other) = (self.at(node), self.at(other));
        this.dfs_num_in.get() >= other.dfs_num_in.get() && this.dfs_num_out.get() <= other.dfs_num_out.get()
    }

    /// `DomTreeNodeBase::compare`: true if the nodes differ.
    fn compare_node(
        &self,
        node: Node,
        other_tree: &Self,
        other: Node,
    ) -> bool {
        if self.level_of(node) != other_tree.level_of(other) {
            return true;
        }
        let other_children: HashSet<Bb> =
            other_tree.children_of(other).into_iter().map(|child| other_tree.block_of(child)).collect();
        let mut own_count = 0;
        for child in self.children_of(node) {
            if !other_children.contains(&self.block_of(child)) {
                return true;
            }
            own_count += 1;
        }
        own_count != other_children.len()
    }

    /// `createNodeUnlinked`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L857>
    fn create_node_unlinked(
        &mut self,
        max_number: usize,
        block: Bb,
        idom: Option<Node>,
    ) -> Node {
        let level = idom.map_or(0, |idom| self.at(idom).level + 1);
        let node = TreeNode {
            block,
            idom: idom.unwrap_or(NO),
            level,
            first_child: NO,
            sibling: NO,
            dfs_num_in: Cell::new(u32::MAX),
            dfs_num_out: Cell::new(u32::MAX),
        };
        let index = Self::node_index(block);
        if index >= self.nodes.len() {
            // Add 1 for post-dominator trees, 0 is nullptr block.
            let max = max_number + usize::from(POST);
            debug_assert!(index < max, "getMaxNumber returned too small value");
            self.nodes.resize_with(max, || None);
        }
        self.nodes[index] = Some(node);
        index as Node
    }

    /// `createNode`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L872>
    fn create_node(
        &mut self,
        max_number: usize,
        block: Bb,
        idom: Option<Node>,
    ) -> Node {
        let node = self.create_node_unlinked(max_number, block, idom);
        if let Some(idom) = idom {
            self.add_child(idom, node);
        }
        node
    }

    /// `reset`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L843>
    pub fn reset(&mut self) {
        self.nodes.clear();
        self.roots.clear();
        self.root_node = NO;
        self.calculated = false;
        self.dfs_info_valid.set(false);
        self.slow_queries.set(0);
    }

    /// `getRootNode`'s block: the entry, or None for the virtual root of a
    /// post-dominator tree.
    pub fn root_block(&self) -> Option<Option<BlockId>> {
        (self.root_node != NO).then(|| self.block_of(self.root_node))
    }

    /// `getRoot`.
    pub fn root(&self) -> BlockId {
        assert!(self.roots.len() == 1, "Should always have entry node!");
        self.roots[0]
    }

    /// `getNode(BB)->getIDom()->getBlock()`: the immediate dominator of
    /// `block`, None for a root, a block not in the tree, or (post-dominators)
    /// a block under the virtual root.
    pub fn immediate_dominator(
        &self,
        block: BlockId,
    ) -> Option<BlockId> {
        let node = self.node(Some(block))?;
        self.idom_of(node).and_then(|idom| self.block_of(idom))
    }

    /// The tree level of `block` (the root is 0), None if not in the tree.
    pub fn level(
        &self,
        block: BlockId,
    ) -> Option<u32> {
        self.node(Some(block)).map(|node| self.level_of(node))
    }

    /// The blocks `block` immediately dominates, in `children()` order.
    pub fn children(
        &self,
        block: Bb,
    ) -> Vec<BlockId> {
        let Some(node) = self.node(block) else { return Vec::new() };
        self.children_of(node).into_iter().filter_map(|child| self.block_of(child)).collect()
    }

    /// `getDescendants`: every block dominated by `r`, `r` first.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L422>
    pub fn descendants(
        &self,
        r: BlockId,
    ) -> Vec<Bb> {
        let Some(rn) = self.node(Some(r)) else { return Vec::new() };
        let mut result = Vec::new();
        let mut work_list = vec![rn];
        while let Some(node) = work_list.pop() {
            result.push(self.block_of(node));
            work_list.extend(self.children_of(node));
        }
        result
    }

    /// `isReachableFromEntry`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L453>
    pub fn is_reachable_from_entry(
        &self,
        a: BlockId,
    ) -> bool {
        assert!(!POST, "This is not implemented for post dominators");
        self.node(Some(a)).is_some()
    }

    /// `dominates(const DomTreeNodeBase *, ...)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L462>
    fn dominates_nodes(
        &self,
        a: Option<Node>,
        b: Option<Node>,
    ) -> bool {
        // A node trivially dominates itself.
        if a == b {
            return true;
        }
        // An unreachable node is dominated by anything.
        let Some(b) = b else { return true };
        // And dominates nothing.
        let Some(a) = a else { return false };
        if self.at(b).idom == a {
            return true;
        }
        if self.at(a).idom == b {
            return false;
        }
        // A can only dominate B if it is higher in the tree.
        if self.at(a).level >= self.at(b).level {
            return false;
        }
        if self.dfs_info_valid.get() {
            return self.dominated_by(b, a);
        }
        // If we end up with too many slow queries, just update the DFS numbers
        // on the theory that we are going to keep querying.
        self.slow_queries.set(self.slow_queries.get() + 1);
        if self.slow_queries.get() > 32 {
            self.update_dfs_numbers();
            return self.dominated_by(b, a);
        }
        self.dominated_by_slow_tree_walk(a, b)
    }

    /// `dominates(const NodeT *, const NodeT *)`: every path to `b` passes
    /// `a`. A block not in the tree is dominated by everything.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L961>
    pub fn dominates(
        &self,
        a: BlockId,
        b: BlockId,
    ) -> bool {
        if a == b {
            return true;
        }
        self.dominates_nodes(self.node(Some(a)), self.node(Some(b)))
    }

    /// `dominates(const BasicBlockEdge &, const BasicBlock *)`: whether every
    /// path to `use_block` takes the edge `start` -> `end`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L162>
    pub fn dominates_edge(
        &self,
        function: &Function,
        start: BlockId,
        end: BlockId,
        use_block: BlockId,
    ) -> bool {
        assert!(!POST, "an edge dominates in a dominator tree");
        // If the end of the edge does not dominate the use block, nothing on
        // the edge does.
        if !self.dominates(end, use_block) {
            return false;
        }
        // The edge dominates the block if it is the only way in, or if every
        // other way in comes from a block `end` dominates.
        let predecessors: Vec<BlockId> = function
            .block_users(end)
            .iter()
            .filter(|one| function.instruction(one.user).opcode.is_terminator())
            .filter_map(|one| function.parent(one.user))
            .collect();
        if predecessors.len() == 1 {
            return true;
        }
        let mut duplicates = 0;
        for from in predecessors {
            if from == start {
                duplicates += 1;
                if duplicates > 1 {
                    return false;
                }
                continue;
            }
            if !self.dominates(end, from) {
                return false;
            }
        }
        true
    }

    /// `properlyDominates`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L969>
    pub fn properly_dominates(
        &self,
        a: BlockId,
        b: BlockId,
    ) -> bool {
        if a == b {
            return false;
        }
        self.dominates_nodes(self.node(Some(a)), self.node(Some(b)))
    }

    /// `dominatedBySlowTreeWalk`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L935>
    fn dominated_by_slow_tree_walk(
        &self,
        a: Node,
        mut b: Node,
    ) -> bool {
        debug_assert!(a != b);
        let a_level = self.level_of(a);
        // Don't walk nodes above A's subtree. When we reach A's level, we must
        // either find A or be in some other subtree not dominated by A.
        while let Some(idom) = self.idom_of(b) {
            if self.level_of(idom) < a_level {
                break;
            }
            b = idom;
        }
        b == a
    }

    /// `findNearestCommonDominator(NodeT *, NodeT *)`. None where the common
    /// dominator is the virtual root of a post-dominator tree.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L514>
    pub fn find_nearest_common_dominator(
        &self,
        graph: &impl DomGraph,
        a: BlockId,
        b: BlockId,
    ) -> Bb {
        // If either A or B is a entry block then it is nearest common dominator
        // (for forward-dominators).
        if !POST {
            let entry = graph.entry().expect("a function with blocks has an entry");
            if a == entry || b == entry {
                return Some(entry);
            }
        }
        self.nearest_common_dominator(a, b)
    }

    /// The walk of `findNearestCommonDominator` after the entry check, which
    /// needs the graph for the entry only.
    fn nearest_common_dominator(
        &self,
        a: BlockId,
        b: BlockId,
    ) -> Bb {
        let mut node_a = self.node(Some(a)).expect("A must be in the tree");
        let mut node_b = self.node(Some(b)).expect("B must be in the tree");
        // Use level information to go up the tree until the levels match. Then
        // continue going up til we arrive at the same node.
        while node_a != node_b {
            if self.level_of(node_a) < self.level_of(node_b) {
                std::mem::swap(&mut node_a, &mut node_b);
            }
            node_a = self.at(node_a).idom;
        }
        self.block_of(node_a)
    }

    /// `isVirtualRoot`.
    fn is_virtual_root(
        &self,
        node: Node,
    ) -> bool {
        POST && self.at(node).block.is_none()
    }

    /// `findNearestCommonDominator(iterator_range)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L556>
    pub fn find_nearest_common_dominator_of(
        &self,
        graph: &impl DomGraph,
        blocks: &[BlockId],
    ) -> Bb {
        assert!(!blocks.is_empty(), "Nodes list is empty!");
        let mut ncd = blocks[0];
        for &block in &blocks[1..] {
            let Some(next) = self.find_nearest_common_dominator(graph, ncd, block) else { return None };
            ncd = next;
            // Stop when the root is reached.
            if self.is_virtual_root(self.node(Some(ncd)).expect("in the tree")) {
                return None;
            }
        }
        Some(ncd)
    }

    /// `updateDFSNumbers`: In and Out numbers by a walk of the tree.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L757>
    pub fn update_dfs_numbers(&self) {
        if self.dfs_info_valid.get() {
            self.slow_queries.set(0);
            return;
        }
        let this_root = self.root_node;
        debug_assert!(!self.calculated || this_root != NO, "Empty constructed DomTree");
        if this_root == NO {
            return;
        }
        // Both dominators and postdominators have a single root node. In the
        // case of PostDominatorTree, this node is a virtual root.
        let mut work_stack: Vec<(Node, Node)> = vec![(this_root, self.at(this_root).first_child)];
        let mut dfs_num = 0;
        self.at(this_root).dfs_num_in.set(dfs_num);
        dfs_num += 1;
        while let Some(&(node, child_it)) = work_stack.last() {
            // If we visited all of the children of this node, "recurse" back
            // up the stack setting the DFOutNum.
            if child_it == NO {
                self.at(node).dfs_num_out.set(dfs_num);
                work_stack.pop();
            } else {
                // Otherwise, recursively visit this child.
                let child = child_it;
                work_stack.last_mut().expect("nonempty").1 = self.at(child).sibling;
                work_stack.push((child, self.at(child).first_child));
                self.at(child).dfs_num_in.set(dfs_num);
                dfs_num += 1;
            }
        }
        self.slow_queries.set(0);
        self.dfs_info_valid.set(true);
    }

    /// The preorder number of `block` in the tree and the last one in its
    /// subtree (`DFSNumIn`, `DFSNumOut - 1`): `a` dominates `b` where `b`'s
    /// lies between `a`'s two. None for a block not in the tree.
    pub fn span(
        &self,
        block: BlockId,
    ) -> Option<(u32, u32)> {
        let node = self.node(Some(block))?;
        self.update_dfs_numbers();
        let node = self.at(node);
        Some((node.dfs_num_in.get(), node.dfs_num_out.get() - 1))
    }

    /// `addNewBlock`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L649>
    pub fn add_new_block(
        &mut self,
        graph: &impl DomGraph,
        block: BlockId,
        dom_block: BlockId,
    ) {
        debug_assert!(self.node(Some(block)).is_none(), "Block already in dominator tree!");
        let idom_node = self.node(Some(dom_block)).expect("Not immediate dominator specified for block!");
        self.dfs_info_valid.set(false);
        self.create_node(graph.max_number(), Some(block), Some(idom_node));
    }

    /// `setNewRoot`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L662>
    pub fn set_new_root(
        &mut self,
        graph: &impl DomGraph,
        block: BlockId,
    ) {
        debug_assert!(self.node(Some(block)).is_none(), "Block already in dominator tree!");
        assert!(!POST, "Cannot change root of post-dominator tree");
        self.dfs_info_valid.set(false);
        let new_node = self.create_node(graph.max_number(), Some(block), None);
        if self.roots.is_empty() {
            self.roots.push(block);
        } else {
            assert!(self.roots.len() == 1);
            let old_root = self.roots[0];
            let old_node = self.node(Some(old_root)).expect("the old root is in the tree");
            self.add_child(new_node, old_node);
            self.at_mut(old_node).idom = new_node;
            self.update_level(old_node);
            self.roots[0] = block;
        }
        self.root_node = new_node;
    }

    /// `changeImmediateDominator`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L692>
    pub fn change_immediate_dominator(
        &mut self,
        block: BlockId,
        new_block: BlockId,
    ) {
        let (n, new_idom) = (self.node(Some(block)), self.node(Some(new_block)));
        let (n, new_idom) =
            (n.expect("Cannot change null node pointers!"), new_idom.expect("Cannot change null node pointers!"));
        self.dfs_info_valid.set(false);
        self.set_idom(n, new_idom);
    }

    /// `eraseNode`: `block` must not dominate any other block.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L699>
    pub fn erase_node(
        &mut self,
        block: BlockId,
    ) {
        self.erase_node_of(Some(block));
    }

    fn erase_node_of(
        &mut self,
        block: Bb,
    ) {
        let index = Self::node_index(block);
        let node = self.nodes[index].as_ref().expect("Removing node that isn't in dominator tree.");
        debug_assert!(node.first_child == NO, "Node is not a leaf node.");
        let idom = node.idom;
        self.dfs_info_valid.set(false);
        // Remove node from immediate dominator's children list.
        if idom != NO {
            self.remove_child(idom, index as Node);
        }
        self.nodes[index] = None;
        if !POST {
            return;
        }
        // Remember to update PostDominatorTree roots.
        if let Some(block) = block
            && let Some(at) = self.roots.iter().position(|one| *one == block)
        {
            self.roots.swap_remove(at);
        }
    }

    /// `splitBlock`: `new_bb` was split off and now has one successor.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L725>
    pub fn split_block(
        &mut self,
        graph: &impl DomGraph,
        new_bb: BlockId,
    ) {
        // `children<N>` is the successors of a dominator tree, the
        // predecessors of a post-dominator tree.
        let children = |block: BlockId| -> Vec<BlockId> {
            if POST { graph.predecessors(block).to_vec() } else { graph.successors(block).to_vec() }
        };
        let inverse_children = |block: BlockId| -> Vec<BlockId> {
            if POST { graph.successors(block).to_vec() } else { graph.predecessors(block).to_vec() }
        };
        let kids = children(new_bb);
        assert!(kids.len() == 1, "NewBB should have a single successor!");
        let new_bb_succ = kids[0];
        let pred_blocks = inverse_children(new_bb);
        assert!(!pred_blocks.is_empty(), "No predblocks?");

        let mut new_bb_dominates_new_bb_succ = true;
        for pred in inverse_children(new_bb_succ) {
            if pred != new_bb && !self.dominates(new_bb_succ, pred) && self.reachable_from_entry_of(pred) {
                new_bb_dominates_new_bb_succ = false;
                break;
            }
        }

        // Find NewBB's immediate dominator and create new dominator tree node
        // for NewBB.
        let mut new_bb_idom: Option<BlockId> = None;
        let mut i = 0;
        while i < pred_blocks.len() {
            if self.reachable_from_entry_of(pred_blocks[i]) {
                new_bb_idom = Some(pred_blocks[i]);
                break;
            }
            i += 1;
        }
        // It's possible that none of the predecessors of NewBB are reachable;
        // in that case, NewBB itself is unreachable, so nothing needs to be
        // changed.
        let Some(mut idom) = new_bb_idom else { return };
        for &pred in &pred_blocks[i + 1..] {
            if self.reachable_from_entry_of(pred) {
                idom = self.find_nearest_common_dominator(graph, idom, pred).expect("a block of the tree");
            }
        }
        // Create the new dominator tree node... and set the idom of NewBB.
        self.add_new_block(graph, new_bb, idom);
        // If NewBB strictly dominates other blocks, then it is now the
        // immediate dominator of NewBBSucc. Update the dominator tree as
        // appropriate.
        if new_bb_dominates_new_bb_succ {
            self.change_immediate_dominator(new_bb_succ, new_bb);
        }
    }

    /// `isReachableFromEntry`, which `Split` asks of a post-dominator tree as
    /// well: a node in the tree.
    fn reachable_from_entry_of(
        &self,
        block: BlockId,
    ) -> bool {
        self.node(Some(block)).is_some()
    }

    /// `compare`: true if the trees differ.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L347>
    pub fn compare(
        &self,
        other: &Self,
    ) -> bool {
        if self.roots.len() != other.roots.len() {
            return true;
        }
        if !self.roots.iter().all(|root| other.roots.contains(root)) {
            return true;
        }
        let mut num_nodes = 0;
        // All nodes we have must exist and be equal in the other tree.
        for (index, node) in self.nodes.iter().enumerate() {
            let Some(node) = node else { continue };
            let Some(other_node) = other.node(node.block) else { return true };
            if self.compare_node(index as Node, other, other_node) {
                return true;
            }
            num_nodes += 1;
        }
        // If the other tree has more nodes than we have, they're not equal.
        let num_other_nodes = other.nodes.iter().flatten().count();
        num_nodes != num_other_nodes
    }

    /// `print`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTree.h#L734>
    pub fn print(&self) -> String {
        use std::fmt::Write;
        let mut out = String::from("=============================--------------------------------\n");
        out.push_str(if POST { "Inorder PostDominator Tree: " } else { "Inorder Dominator Tree: " });
        if !self.dfs_info_valid.get() {
            let _ = write!(out, "DFSNumbers invalid: {} slow queries.", self.slow_queries.get());
        }
        out.push('\n');
        if self.root_node != NO {
            self.print_node(self.root_node, 1, &mut out);
        }
        out.push_str("Roots: ");
        for root in &self.roots {
            let _ = write!(out, "{} ", Self::name(Some(*root)));
        }
        out.push('\n');
        out
    }

    fn name(block: Bb) -> String {
        block.map_or_else(|| " <<exit node>>".to_string(), |block| format!("%{}", block.0))
    }

    /// `PrintDomTree`.
    fn print_node(
        &self,
        node: Node,
        level: usize,
        out: &mut String,
    ) {
        use std::fmt::Write;
        let n = self.at(node);
        let _ = writeln!(
            out,
            "{:indent$}[{level}] {} {{{},{}}} [{}]",
            "",
            Self::name(n.block),
            n.dfs_num_in.get() as i32,
            n.dfs_num_out.get() as i32,
            n.level,
            indent = 2 * level
        );
        for child in self.children_of(node) {
            self.print_node(child, level + 1, out);
        }
    }

    /// `recalculate(Func)`: the tree of `graph` from scratch.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1675>
    pub fn recalculate(
        &mut self,
        graph: &impl DomGraph,
    ) {
        self.calculated = true;
        SemiNca::<_, POST>::calculate_from_scratch(self, graph, None);
    }

    /// `recalculate(Func, Updates)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1682>
    pub fn recalculate_with_updates(
        &mut self,
        graph: &impl DomGraph,
        updates: &[Update],
    ) {
        self.calculated = true;
        // FIXME in LLVM: this needs the PostViewCFG; it uses the PreViewCFG.
        let pre = GraphDiff::new(updates, true, POST);
        let bui = BatchUpdateInfo::new(pre, None);
        SemiNca::<_, POST>::calculate_from_scratch(self, graph, Some(&bui));
    }

    /// `insertEdge`: tell the tree about an edge added to the CFG, just before
    /// or after the change.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1657>
    pub fn insert_edge(
        &mut self,
        graph: &impl DomGraph,
        from: BlockId,
        to: BlockId,
    ) {
        let (from, to) = if POST { (to, from) } else { (from, to) };
        SemiNca::<_, POST>::insert_edge(self, graph, None, Some(from), to);
    }

    /// `deleteEdge`: tell the tree about an edge removed from the CFG, just
    /// after the change.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1665>
    pub fn delete_edge(
        &mut self,
        graph: &impl DomGraph,
        from: BlockId,
        to: BlockId,
    ) {
        let (from, to) = if POST { (to, from) } else { (from, to) };
        SemiNca::<_, POST>::delete_edge(self, graph, None, from, to);
    }

    /// `applyUpdates(Updates)`: tell the tree about a sequence of CFG edge
    /// insertions and deletions, made after the last update of the tree.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1629>
    pub fn apply_updates(
        &mut self,
        graph: &impl DomGraph,
        updates: &[Update],
    ) {
        let mut pre_view_cfg = GraphDiff::new(updates, true, POST);
        SemiNca::<_, POST>::apply_updates(self, graph, &mut pre_view_cfg, None);
    }

    /// `applyUpdates(Updates, PostViewUpdates)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1637>
    pub fn apply_updates_with_post_view(
        &mut self,
        graph: &impl DomGraph,
        updates: &[Update],
        post_view_updates: &[Update],
    ) {
        if updates.is_empty() {
            let post_view_cfg = GraphDiff::new(post_view_updates, false, POST);
            let mut pre = post_view_cfg.clone();
            SemiNca::<_, POST>::apply_updates(self, graph, &mut pre, Some(post_view_cfg));
            return;
        }
        // PreViewCFG needs to merge Updates and PostViewCFG. The updates in
        // Updates need to be reversed, and match the direction in PostViewCFG.
        let mut all_updates = updates.to_vec();
        all_updates.extend_from_slice(post_view_updates);
        let mut pre_view_cfg = GraphDiff::new(&all_updates, true, POST);
        let post_view_cfg = GraphDiff::new(post_view_updates, false, POST);
        SemiNca::<_, POST>::apply_updates(self, graph, &mut pre_view_cfg, Some(post_view_cfg));
    }

    /// `verify`: Ok where the tree is the tree of `graph`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1597>
    pub fn verify(
        &self,
        graph: &impl DomGraph,
        level: VerificationLevel,
    ) -> Result<(), String> {
        let mut snca = SemiNca::<_, POST>::new(graph, None);
        // Simplist check is to compare against a new tree. This will also
        // usefully print the old and new trees, if they are different.
        snca.is_same_as_fresh_tree(self)?;
        // Common checks to verify the properties of the tree. O(N log N) at
        // worst.
        snca.verify_roots(self)?;
        snca.verify_reachability(self)?;
        Self::verify_levels(self)?;
        Self::verify_dfs_numbers(self)?;
        // Extra checks depending on VerificationLevel. Up to O(N^3).
        if matches!(level, VerificationLevel::Basic | VerificationLevel::Full) {
            snca.verify_parent_property(self)?;
        }
        if level == VerificationLevel::Full {
            snca.verify_sibling_property(self)?;
        }
        Ok(())
    }

    /// `VerifyLevels`: every child of a node of level L has level L + 1.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1290>
    fn verify_levels(dt: &Self) -> Result<(), String> {
        for tn in dt.nodes.iter().flatten() {
            let Some(bb) = tn.block else { continue };
            if tn.idom == NO && tn.level != 0 {
                return Err(format!("Node without an IDom {} has a nonzero level {}!", Self::name(Some(bb)), tn.level));
            }
            if tn.idom != NO {
                let idom = dt.at(tn.idom);
                if tn.level != idom.level + 1 {
                    return Err(format!(
                        "Node {} has level {} while its IDom {} has level {}!",
                        Self::name(Some(bb)),
                        tn.level,
                        Self::name(idom.block),
                        idom.level
                    ));
                }
            }
        }
        Ok(())
    }

    /// `VerifyDFSNumbers`: where they are valid, children cover their
    /// parent's numbers with no gaps.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1324>
    fn verify_dfs_numbers(dt: &Self) -> Result<(), String> {
        if !dt.dfs_info_valid.get() || !dt.calculated {
            return Ok(());
        }
        let root_bb = if POST { None } else { dt.roots.first().copied() };
        let root = dt.node(root_bb).expect("the root is in the tree");
        let numbers = |node: Node| {
            let n = dt.at(node);
            format!("{} {{{}, {}}}", Self::name(n.block), n.dfs_num_in.get(), n.dfs_num_out.get())
        };
        // Verify the root's DFS In number. Although DFS numbering would also
        // work if we started from some other value, we assume 0-based
        // numbering.
        if dt.at(root).dfs_num_in.get() != 0 {
            return Err(format!("DFSIn number for the tree root is not:\n\t{}", numbers(root)));
        }
        // For each tree node verify if children's DFS numbers cover their
        // parent's DFS numbers with no gaps.
        for (index, node) in dt.nodes.iter().enumerate() {
            let Some(node) = node else { continue };
            let index = index as Node;
            // Handle tree leaves.
            if node.first_child == NO {
                if node.dfs_num_in.get() + 1 != node.dfs_num_out.get() {
                    return Err(format!("Tree leaf should have DFSOut = DFSIn + 1:\n\t{}", numbers(index)));
                }
                continue;
            }
            // Make a copy and sort it such that it is possible to check if
            // there are no gaps between DFS numbers of adjacent children.
            let mut children = dt.children_of(index);
            children.sort_by_key(|child| dt.at(*child).dfs_num_in.get());
            let all = || children.iter().map(|child| numbers(*child)).collect::<Vec<_>>().join(", ");
            let error = |first: Node, second: Option<Node>| {
                let mut message =
                    format!("Incorrect DFS numbers for:\n\tParent {}\n\tChild {}", numbers(index), numbers(first));
                if let Some(second) = second {
                    message.push_str(&format!("\n\tSecond child {}", numbers(second)));
                }
                message.push_str(&format!("\nAll children: {}", all()));
                message
            };
            let (first, last) = (children[0], children[children.len() - 1]);
            if dt.at(first).dfs_num_in.get() != node.dfs_num_in.get() + 1 {
                return Err(error(first, None));
            }
            if dt.at(last).dfs_num_out.get() != node.dfs_num_out.get() {
                return Err(error(last, None));
            }
            for pair in children.windows(2) {
                if dt.at(pair[0]).dfs_num_out.get() != dt.at(pair[1]).dfs_num_in.get() {
                    return Err(error(pair[0], Some(pair[1])));
                }
            }
        }
        Ok(())
    }
}

/// `SemiNCAInfo::InfoRec`: the record of one node during construction.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L67>
#[derive(Clone, Copy, Default)]
struct InfoRec {
    /// DFS number + 1, so a zeroed record is unvisited.
    dfs_num_plus1: u32,
    parent: u32,
    semi: u32,
    label: u32,
    idom: u32,
    /// Head index + 1 into `reverse_children`; 0: empty list.
    reverse_children_start: u32,
}

/// Marks a node that hasn't been visited by DFS.
const UNVISITED: u32 = 0;

/// `SemiNCAInfo`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L54>
struct SemiNca<'a, G: DomGraph, const POST: bool> {
    graph: &'a G,
    batch: Batch<'a>,
    /// Map a 0-based DFS number to the node. 0 is the DFS root, or the virtual
    /// root for postdominators.
    num_to_node: Vec<Bb>,
    node_infos: Vec<InfoRec>,
    /// Reverse children of nodes: pairs of (DFSNum (predecessor),
    /// next-or-zero), a linked list in this vector.
    reverse_children: Vec<(u32, u32)>,
}

type NodeOrderMap = IdMap<BlockId, u32>;

thread_local! {
    static WORK: Cell<u64> = const { Cell::new(0) };
}

fn counted() {
    WORK.with(|work| work.set(work.get() + 1));
}

/// Blocks visited and ancestors climbed by the constructions on this thread:
/// the work a test counts, as the wall clock moves with the host's load.
pub fn work() -> u64 {
    WORK.with(Cell::get)
}

thread_local! {
    /// The records of the last `SemiNca` to go, all clear: LLVM's `NodeInfos`
    /// is allocated and zeroed for every block of the function by each
    /// construction, and an update of one edge makes several. DEVIATE 10.
    static INFOS: RefCell<Vec<InfoRec>> = const { RefCell::new(Vec::new()) };
}

impl<G: DomGraph, const POST: bool> Drop for SemiNca<'_, G, POST> {
    fn drop(&mut self) {
        self.clear();
        let infos = std::mem::take(&mut self.node_infos);
        INFOS.with(|pool| {
            let mut pool = pool.borrow_mut();
            if pool.len() < infos.len() {
                *pool = infos;
            }
        });
    }
}

/// A copy of the heap `std::priority_queue` keeps in libstdc++, ordered by
/// level: nodes of one level pop in the order LLVM's pop them.
struct LevelQueue {
    heap: Vec<(u32, Node)>,
}

impl LevelQueue {
    fn new() -> Self {
        Self { heap: Vec::new() }
    }

    fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    fn less(
        a: (u32, Node),
        b: (u32, Node),
    ) -> bool {
        a.0 < b.0
    }

    /// `std::__push_heap`.
    fn sift_up(
        &mut self,
        mut hole: i64,
        top: i64,
        value: (u32, Node),
    ) {
        let mut parent = (hole - 1) / 2;
        while hole > top && Self::less(self.heap[parent as usize], value) {
            self.heap[hole as usize] = self.heap[parent as usize];
            hole = parent;
            parent = (hole - 1) / 2;
        }
        self.heap[hole as usize] = value;
    }

    fn push(
        &mut self,
        level: u32,
        node: Node,
    ) {
        self.heap.push((level, node));
        let hole = self.heap.len() as i64 - 1;
        self.sift_up(hole, 0, (level, node));
    }

    /// `top` then `pop`: `std::pop_heap` and `pop_back`.
    fn pop(&mut self) -> Node {
        let top = self.heap[0];
        let len = self.heap.len() as i64;
        if len > 1 {
            let value = self.heap[(len - 1) as usize];
            self.heap[(len - 1) as usize] = self.heap[0];
            // `std::__adjust_heap` over the first len - 1 elements.
            let len = len - 1;
            let mut hole = 0_i64;
            let mut second = hole;
            while second < (len - 1) / 2 {
                second = 2 * (second + 1);
                if Self::less(self.heap[second as usize], self.heap[(second - 1) as usize]) {
                    second -= 1;
                }
                self.heap[hole as usize] = self.heap[second as usize];
                hole = second;
            }
            if len & 1 == 0 && second == (len - 2) / 2 {
                second = 2 * (second + 1);
                self.heap[hole as usize] = self.heap[(second - 1) as usize];
                hole = second - 1;
            }
            self.sift_up(hole, 0, value);
        }
        self.heap.pop();
        top.1
    }
}

/// `SemiNCAInfo::InsertionInfo`.
struct InsertionInfo {
    bucket: LevelQueue,
    visited: IdSet<Node>,
    affected: Vec<Node>,
    visited_unaffected: Vec<Node>,
}

impl<'a, G: DomGraph, const POST: bool> SemiNca<'a, G, POST> {
    /// If `batch` is None, there's no batch update in progress.
    fn new(
        graph: &'a G,
        batch: Batch<'a>,
    ) -> Self {
        let mut node_infos = INFOS.with(|pool| std::mem::take(&mut *pool.borrow_mut()));
        // Every record in the pool is clear (`Drop`), so the vector grows with
        // defaults.
        node_infos.resize(node_infos.len().max(graph.max_number() + usize::from(POST)), InfoRec::default());
        Self { graph, batch, num_to_node: Vec::new(), node_infos, reverse_children: Vec::new() }
    }

    /// `clear`: the batch stays, an update in progress needs it. Only the
    /// records of the nodes the walk reached are written, so only those are
    /// reset: `NodeInfos.assign` over every block was the cost of a walk of
    /// one.
    fn clear(&mut self) {
        for node in std::mem::take(&mut self.num_to_node) {
            self.node_infos[Self::info_index(node)] = InfoRec::default();
        }
        self.reverse_children.clear();
    }

    /// `getChildren<Inversed>(N, BUI)`: successors (reversed, so a LIFO
    /// worklist visits them in their natural order) or predecessors.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L120>
    fn children(
        graph: &G,
        batch: Batch<'_>,
        node: BlockId,
        inversed: bool,
    ) -> Vec<BlockId> {
        match batch {
            Some(batch) => batch.pre_view_cfg.borrow().children(graph, node, inversed),
            None if inversed => graph.predecessors(node).to_vec(),
            None => reversed(graph.successors(node)),
        }
    }

    /// `getNodeInfo`: for post-dominator trees, index 0 is the null block.
    fn info_index(block: Bb) -> usize {
        match block {
            None => 0,
            Some(block) => block.0 as usize + usize::from(POST),
        }
    }

    fn info(
        &mut self,
        block: Bb,
    ) -> &mut InfoRec {
        &mut self.node_infos[SemiNca::<G, POST>::info_index(block)]
    }

    /// `runDFS`: a DFS that skips what `condition` refuses and collects the
    /// reverse children, so SemiNCA needs no predecessors. With `is_reverse`
    /// it walks backwards relative to `POST`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L173>
    fn run_dfs(
        &mut self,
        is_reverse: bool,
        v: BlockId,
        mut last_num: u32,
        mut condition: impl FnMut(BlockId, BlockId) -> bool,
        attach_to_num: u32,
        succ_order: Option<&NodeOrderMap>,
    ) -> u32 {
        let mut work_list: Vec<(BlockId, u32)> = vec![(v, attach_to_num)];
        self.info(Some(v)).parent = attach_to_num;
        while let Some((bb, parent_num)) = work_list.pop() {
            let bb_info = self.info(Some(bb));
            let start = bb_info.reverse_children_start;
            let already = bb_info.dfs_num_plus1 != UNVISITED;
            self.reverse_children.push((parent_num, start));
            let size = self.reverse_children.len() as u32;
            self.info(Some(bb)).reverse_children_start = size;
            if already {
                continue;
            }
            counted();
            let num = last_num;
            last_num += 1;
            let bb_info = self.info(Some(bb));
            bb_info.parent = parent_num;
            bb_info.semi = num;
            bb_info.label = num;
            bb_info.dfs_num_plus1 = num + 1;
            self.num_to_node.push(Some(bb));

            let direction = is_reverse != POST; // XOR.
            let mut successors = SemiNca::<G, POST>::children(self.graph, self.batch, bb, direction);
            if let Some(order) = succ_order
                && successors.len() > 1
            {
                successors.sort_by_key(|node| *order.get(node).expect("a successor of an unvisited node is ordered"));
            }
            for succ in successors {
                if !condition(bb, succ) {
                    continue;
                }
                work_list.push((succ, num));
            }
        }
        last_num
    }

    /// `eval`: V is a predecessor of W. Returns V if V < W, otherwise the
    /// minimum of sdom(U), where U > W and there is a virtual forest path from
    /// U to V, with path compression.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L238>
    fn eval(
        &mut self,
        v: u32,
        last_linked: u32,
        stack: &mut Vec<usize>,
        num_to_info: &[usize],
    ) -> u32 {
        let mut v_info = num_to_info[v as usize];
        if self.node_infos[v_info].parent < last_linked {
            return self.node_infos[v_info].label;
        }
        // Store ancestors except the last (root of a virtual tree) into a
        // stack.
        debug_assert!(stack.is_empty());
        loop {
            counted();
            stack.push(v_info);
            v_info = num_to_info[self.node_infos[v_info].parent as usize];
            if self.node_infos[v_info].parent < last_linked {
                break;
            }
        }
        // Path compression. Point each vertex's Parent to the root and update
        // its Label if any of its ancestors (PLabel) has a smaller Semi.
        let mut p_info = v_info;
        let mut p_label = self.node_infos[p_info].label;
        loop {
            v_info = stack.pop().expect("a vertex");
            self.node_infos[v_info].parent = self.node_infos[p_info].parent;
            let v_label = self.node_infos[v_info].label;
            if p_label < v_label {
                self.node_infos[v_info].label = p_label;
            } else {
                p_label = v_label;
            }
            p_info = v_info;
            if stack.is_empty() {
                break;
            }
        }
        self.node_infos[v_info].label
    }

    /// `runSemiNCA`: requires the DFS to have run.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L270>
    fn run_semi_nca(&mut self) {
        let next_dfs_num = self.num_to_node.len();
        // NumToInfo is indexed by DFS number; 0 is the root. IDom holds
        // immediate dominators in DFS-number space, initialized to spanning
        // tree parents.
        let num_to_info: Vec<usize> =
            self.num_to_node.iter().map(|node| SemiNca::<G, POST>::info_index(*node)).collect();
        for &at in &num_to_info {
            let v_info = &mut self.node_infos[at];
            v_info.idom = v_info.parent;
        }

        // Step #1: Calculate the semidominators of all vertices.
        let mut eval_stack: Vec<usize> = Vec::new();
        for i in (1..next_dfs_num).rev() {
            let w = num_to_info[i];
            // Initialize the semi dominator to point to the parent node.
            self.node_infos[w].semi = self.node_infos[w].parent;
            let mut rc_idx = self.node_infos[w].reverse_children_start;
            while rc_idx != 0 {
                let entry = self.reverse_children[rc_idx as usize - 1];
                rc_idx = entry.1;
                let semi_u = self.eval(entry.0, i as u32 + 1, &mut eval_stack, &num_to_info);
                if semi_u < self.node_infos[w].semi {
                    self.node_infos[w].semi = semi_u;
                }
            }
            // Label now holds the semidominator value for later eval() calls.
            self.node_infos[w].label = self.node_infos[w].semi;
        }

        // Step #2: Explicitly define the immediate dominator of each vertex.
        //          IDom[i] = NCA(SDom[i], SpanningTreeParent(i)).
        // SDom[i]'s DFS number is just Semi.
        for i in 1..next_dfs_num {
            let w = num_to_info[i];
            let semi = self.node_infos[w].semi;
            let mut w_idom = self.node_infos[w].idom;
            while w_idom > semi {
                counted();
                w_idom = self.node_infos[num_to_info[w_idom as usize]].idom;
            }
            self.node_infos[w].idom = w_idom;
        }
    }

    /// `addVirtualRoot`: the root every exit of a post-dominator tree is
    /// connected to.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L318>
    fn add_virtual_root(&mut self) {
        assert!(POST, "Only postdominators have a virtual root");
        assert!(self.num_to_node.is_empty(), "SNCAInfo must be freshly constructed");
        let bb_info = self.info(None);
        bb_info.semi = 0;
        bb_info.label = 0;
        bb_info.dfs_num_plus1 = 1;
        self.num_to_node.push(None);
    }

    /// `HasForwardSuccessors`: nodes with none are trivial roots; the others
    /// are in infinite loops.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L332>
    fn has_forward_successors(
        graph: &G,
        batch: Batch<'_>,
        node: BlockId,
    ) -> bool {
        !SemiNca::<G, POST>::children(graph, batch, node, false).is_empty()
    }

    /// `FindRoots`: the roots, without relying on the set stored in the tree.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L345>
    fn find_roots(
        graph: &G,
        batch: Batch<'_>,
    ) -> Vec<BlockId> {
        let mut roots: Vec<BlockId> = Vec::new();
        // For dominators, function entry CFG node is always a tree root node.
        if !POST {
            roots.push(graph.entry().expect("a function with blocks has an entry"));
            return roots;
        }
        let mut snca = SemiNca::<G, POST>::new(graph, batch);
        // PostDominatorTree always has a virtual root.
        snca.add_virtual_root();
        let mut num = 1;

        // Step #1: Find all the trivial roots that are going to will
        // definitely remain tree roots.
        let mut total = 0;
        for &n in graph.nodes() {
            total += 1;
            // If it has no *successors*, it is definitely a root.
            if !SemiNca::<G, POST>::has_forward_successors(graph, batch, n) {
                roots.push(n);
                // Run DFS not to walk this part of CFG later.
                num = snca.run_dfs(false, n, num, |_, _| true, 0, None);
            }
        }

        // Step #2: Find all non-trivial root candidates. Those are CFG nodes
        // that are reverse-unreachable were not visited by previous DFS walks
        // (i.e. CFG nodes in infinite loops).
        let mut has_non_trivial_roots = false;
        // Accounting for the virtual exit, see if we had any
        // reverse-unreachable nodes.
        if total + 1 != num {
            has_non_trivial_roots = true;

            // SuccOrder is the order of blocks in the function. It makes the
            // FurthestAway node and the whole PostDomTree immune to swapping
            // successors. It is initialized lazily only for successors of
            // reverse unreachable nodes.
            let mut succ_order: Option<NodeOrderMap> = None;

            // Make another DFS pass over all other nodes to find the
            // reverse-unreachable blocks, and find the furthest paths we'll be
            // able to make. This looks N^2, but it's 2N worst case: each
            // unreachable node is visited once, in two directions at most.
            for &i in graph.nodes() {
                if snca.info(Some(i)).dfs_num_plus1 != UNVISITED {
                    continue;
                }
                // Find the furthest away we can get by following successors,
                // then follow them in reverse. This gives some reasonable
                // answer about the post-dom tree inside any infinite loop,
                // and matches GCC's behavior.
                if succ_order.is_none() {
                    let mut order = NodeOrderMap::new();
                    for &node in graph.nodes() {
                        if snca.info(Some(node)).dfs_num_plus1 == UNVISITED {
                            for succ in SemiNca::<G, POST>::children(graph, snca.batch, node, false) {
                                if !order.contains_key(&succ) {
                                    order.insert(succ, 0);
                                }
                            }
                        }
                    }
                    // Add mapping for all entries of SuccOrder.
                    let mut node_num = 0;
                    for &node in graph.nodes() {
                        node_num += 1;
                        if let Some(slot) = order.get_mut(&node) {
                            debug_assert_eq!(*slot, 0);
                            *slot = node_num;
                        }
                    }
                    succ_order = Some(order);
                }
                let order = succ_order.as_ref().expect("initialised above");

                let new_num = snca.run_dfs(true, i, num, |_, _| true, num, Some(order));
                let furthest_away = snca.num_to_node[new_num as usize - 1].expect("a real node");
                roots.push(furthest_away);
                // Remove the DFS info of the forward walk.
                for at in (num..new_num).rev() {
                    let n = snca.num_to_node[at as usize];
                    *snca.info(n) = InfoRec::default();
                    snca.num_to_node.pop();
                }
                num = snca.run_dfs(false, furthest_away, num, |_, _| true, 0, None);
            }
        }

        debug_assert!(total + 1 == num, "Everything should have been visited");

        // Step #3: If we found some non-trivial roots, make them
        // non-redundant.
        if has_non_trivial_roots {
            SemiNca::<G, POST>::remove_redundant_roots(graph, batch, &mut roots);
        }
        roots
    }

    /// `RemoveRedundantRoots`: a non-trivial root that another root reaches
    /// by a forward walk is not needed.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L501>
    fn remove_redundant_roots(
        graph: &G,
        batch: Batch<'_>,
        roots: &mut Vec<BlockId>,
    ) {
        assert!(POST, "This function is for postdominators only");
        let mut snca = SemiNca::<G, POST>::new(graph, batch);
        let mut i = 0;
        while i < roots.len() {
            let root = roots[i];
            // Trivial roots are always non-redundant.
            if !SemiNca::<G, POST>::has_forward_successors(graph, batch, root) {
                i += 1;
                continue;
            }
            snca.clear();
            // Do a forward walk looking for the other roots.
            let num = snca.run_dfs(true, root, 0, |_, _| true, 0, None);
            // Skip the start node (DFS number 0).
            let mut removed = false;
            for x in 1..num as usize {
                let n = snca.num_to_node[x].expect("a real node");
                // If we found another root in a (forward) DFS walk, remove the
                // current root from the set of roots, as it is
                // reverse-reachable from the other one.
                if roots.contains(&n) {
                    roots.swap_remove(i);
                    // Root at the back takes the current root's place. Start
                    // the next loop iteration with the same index.
                    removed = true;
                    break;
                }
            }
            if !removed {
                i += 1;
            }
        }
    }

    /// `doFullDFSWalk`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L541>
    fn do_full_dfs_walk(
        &mut self,
        dt: &DomTreeBase<POST>,
        mut descend: impl FnMut(BlockId, BlockId) -> bool,
    ) {
        if !POST {
            debug_assert!(dt.roots.len() == 1, "Dominators should have a singe root");
            self.run_dfs(false, dt.roots[0], 0, descend, 0, None);
            return;
        }
        self.add_virtual_root();
        let mut num = 1;
        for &root in &dt.roots {
            num = self.run_dfs(false, root, num, &mut descend, 0, None);
        }
    }

    /// `CalculateFromScratch`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L554>
    fn calculate_from_scratch(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
    ) {
        let calculated = dt.calculated;
        dt.reset();
        dt.calculated = calculated;
        // DEVIATE 8: a MIR function may have no block; LLVM's definition has an
        // entry.
        if graph.entry().is_none() {
            return;
        }
        // If the update is using the actual CFG, BUI is null. If it's using a
        // view, BUI is non-null and the PreCFGView is used. When calculating
        // from scratch, make the PreViewCFG equal to the PostCFGView, so Post
        // is used.
        let mut post_view_bui: Batch<'_> = None;
        if let Some(bui) = batch
            && let Some(post) = &bui.post_view_cfg
        {
            *bui.pre_view_cfg.borrow_mut() = post.clone();
            post_view_bui = Some(bui);
        }
        // This is rebuilding the whole tree, not incrementally, but
        // PostViewBUI is used in case the caller needs a DT update with a
        // CFGView.
        let mut snca = SemiNca::<G, POST>::new(graph, post_view_bui);
        // The full walk numbers at most every block and the virtual root.
        snca.num_to_node.reserve(graph.max_number() + usize::from(POST));

        // Step #0: Number blocks in depth-first order and initialize
        // variables used in later stages of the algorithm.
        dt.roots = SemiNca::<G, POST>::find_roots(graph, post_view_bui);
        snca.do_full_dfs_walk(dt, |_, _| true);

        snca.run_semi_nca();
        if let Some(bui) = batch {
            bui.is_recalculated.set(true);
        }

        if dt.roots.is_empty() {
            return;
        }

        // Add a node for the root. If the tree is a PostDominatorTree it will
        // be the virtual exit (denoted by (BasicBlock *) nullptr) which
        // postdominates all real exits (including multiple exit blocks,
        // infinite loops).
        let root = if POST { None } else { Some(dt.roots[0]) };
        dt.root_node = dt.create_node(graph.max_number(), root, None);
        snca.attach_new_subtree(dt);
    }

    /// `attachNewSubtree`: each non-root node of a subtree under its
    /// immediate dominator. Linked in reverse: `addChild` prepends, so the
    /// children of each node end in DFS order.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L597>
    fn attach_new_subtree(
        &mut self,
        dt: &mut DomTreeBase<POST>,
    ) {
        let e = self.num_to_node.len();
        let max = self.graph.max_number();
        for num in 1..e {
            let w = self.num_to_node[num];
            debug_assert!(dt.node(w).is_none(), "node was already attached");
            let idom_num = self.info(w).idom as usize;
            let idom = dt.node(self.num_to_node[idom_num]);
            dt.create_node_unlinked(max, w, idom);
        }
        for num in (1..e).rev() {
            let tn = dt.node(self.num_to_node[num]).expect("attached above");
            let idom = dt.at(tn).idom;
            dt.add_child(idom, tn);
        }
    }

    /// `reattachExistingSubtree`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L610>
    fn reattach_existing_subtree(
        &mut self,
        dt: &mut DomTreeBase<POST>,
        attach_to: Node,
    ) {
        let top = dt.node(self.num_to_node[0]).expect("the top is in the tree");
        dt.set_idom(top, attach_to);
        for num in 1..self.num_to_node.len() {
            let n = self.num_to_node[num];
            let idom_num = self.info(n).idom as usize;
            let idom_node = dt.node(self.num_to_node[idom_num]).expect("in the tree");
            let node = dt.node(n).expect("in the tree");
            dt.set_idom(node, idom_node);
        }
    }

    /// `InsertEdge` (the internal one: from and to as in the tree's graph).
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L638>
    fn insert_edge(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        from: Bb,
        to: BlockId,
    ) {
        debug_assert!(from.is_some() || POST, "From has to be a valid CFG node or a virtual root");
        let from_tn = match dt.node(from) {
            Some(node) => node,
            None => {
                // Ignore edges from unreachable nodes for (forward)
                // dominators.
                if !POST {
                    return;
                }
                // The unreachable node becomes a new root -- a tree node for
                // it.
                let virtual_root = dt.node(None);
                let node = dt.create_node(graph.max_number(), from, virtual_root);
                dt.roots.push(from.expect("a real block"));
                node
            }
        };
        dt.dfs_info_valid.set(false);
        match dt.node(Some(to)) {
            None => SemiNca::<G, POST>::insert_unreachable(dt, graph, batch, from_tn, to),
            Some(to_tn) => SemiNca::<G, POST>::insert_reachable(dt, graph, batch, from_tn, to_tn),
        }
    }

    /// `UpdateRootsBeforeInsertion`: rebuilds the tree where an existing root
    /// becomes reverse-reachable.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L669>
    fn update_roots_before_insertion(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        to: Node,
    ) -> bool {
        assert!(POST, "This function is only for postdominators");
        // Destination node is not attached to the virtual root, so it cannot
        // be a root.
        let to_idom = dt.at(to).idom;
        if to_idom == NO || !dt.is_virtual_root(to_idom) {
            return false;
        }
        let Some(to_block) = dt.at(to).block else { return false };
        if !dt.roots.contains(&to_block) {
            return false; // To is not a root, nothing to update.
        }
        SemiNca::<G, POST>::calculate_from_scratch(dt, graph, batch);
        true
    }

    /// `isPermutation`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L688>
    fn is_permutation(
        a: &[BlockId],
        b: &[BlockId],
    ) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let set: IdSet<BlockId> = a.iter().copied().collect();
        b.iter().all(|n| set.contains(n))
    }

    /// `UpdateRootsAfterUpdate`: keeps the roots those a build from scratch
    /// would choose.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L702>
    fn update_roots_after_update(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
    ) {
        assert!(POST, "This function is only for postdominators");
        // The tree has only trivial roots -- nothing to update.
        if !dt.roots.iter().any(|&n| SemiNca::<G, POST>::has_forward_successors(graph, batch, n)) {
            return;
        }
        // Recalculate the set of roots.
        let roots = SemiNca::<G, POST>::find_roots(graph, batch);
        if !SemiNca::<G, POST>::is_permutation(&dt.roots, &roots) {
            // The roots chosen in the CFG have changed. This is because the
            // incremental algorithm does not really know or use the set of
            // roots and can make a different (implicit) decision about which
            // node within an infinite loop becomes a root.
            SemiNca::<G, POST>::calculate_from_scratch(dt, graph, batch);
        }
    }

    /// `InsertReachable`: insertion to a node already in the tree.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L728>
    fn insert_reachable(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        from: Node,
        to: Node,
    ) {
        if POST && SemiNca::<G, POST>::update_roots_before_insertion(dt, graph, batch, to) {
            return;
        }
        // DT.findNCD expects both pointers to be valid. When From is a virtual
        // root, then its CFG block pointer is a nullptr, so we have to
        // 'compute' the NCD manually.
        let (from_block, to_block) = (dt.block_of(from), dt.block_of(to));
        let ncd_block = if let (Some(a), Some(b)) = (from_block, to_block) {
            dt.find_nearest_common_dominator(graph, a, b)
        } else {
            None
        };
        debug_assert!(ncd_block.is_some() || POST);
        let ncd = dt.node(ncd_block).expect("the NCD is in the tree");
        let ncd_level = dt.level_of(ncd);

        // Based on Lemma 2.5 from [2], after insertion of (From,To), v is
        // affected iff depth(NCD)+1 < depth(v) && a path P from To to v exists
        // where every w on P s.t. depth(v) <= depth(w)
        //
        // This reduces to a widest path problem (maximizing the depth of the
        // minimum vertex in the path) which can be solved by a modified
        // version of Dijkstra with a bucket queue (named depth-based search in
        // [2]).

        // To is in the path, so depth(NCD)+1 < depth(v) <= depth(To). Nothing
        // affected if this does not hold.
        if ncd_level + 1 >= dt.level_of(to) {
            return;
        }

        let mut ii = InsertionInfo {
            bucket: LevelQueue::new(),
            visited: IdSet::new(),
            affected: Vec::new(),
            visited_unaffected: Vec::new(),
        };
        let mut unaffected_on_current_level: Vec<Node> = Vec::new();
        ii.bucket.push(dt.level_of(to), to);
        ii.visited.insert(to);

        while !ii.bucket.is_empty() {
            let mut tn = ii.bucket.pop();
            ii.affected.push(tn);
            let current_level = dt.level_of(tn);
            debug_assert!(dt.at(tn).block.is_some() && ii.visited.contains(&tn), "Preconditions!");

            loop {
                // Unlike regular Dijkstra, we have an inner loop to expand
                // more vertices. The first iteration is for the (affected)
                // vertex popped from II.Bucket and the rest are for vertices
                // in UnaffectedOnCurrentLevel, which may eventually expand to
                // affected vertices.
                //
                // Invariant: there is an optimal path from `To` to TN with the
                // minimum depth being CurrentLevel.
                let tn_block = dt.at(tn).block.expect("a real block");
                for succ in SemiNca::<G, POST>::children(graph, batch, tn_block, POST) {
                    let succ_tn = dt.node(Some(succ)).expect("Unreachable successor found at reachable insertion");
                    let succ_level = dt.level_of(succ_tn);

                    // There is an optimal path from `To` to Succ with the
                    // minimum depth being min(CurrentLevel, SuccLevel).
                    //
                    // If depth(NCD)+1 < depth(Succ) is not satisfied, Succ is
                    // unaffected and no affected vertex may be reached by a
                    // path passing through it. Stop here. Also, Succ may be
                    // visited by other predecessors but the first visit has
                    // the optimal path. Stop if Succ has been visited.
                    if succ_level <= ncd_level + 1 || !ii.visited.insert(succ_tn) {
                        continue;
                    }

                    if succ_level > current_level {
                        // Succ is unaffected but it may (transitively) expand
                        // to affected vertices. Store it in
                        // UnaffectedOnCurrentLevel.
                        unaffected_on_current_level.push(succ_tn);
                        ii.visited_unaffected.push(succ_tn);
                    } else {
                        // The condition is satisfied (Succ is affected). Add
                        // Succ to the bucket queue.
                        ii.bucket.push(succ_level, succ_tn);
                    }
                }

                match unaffected_on_current_level.pop() {
                    None => break,
                    Some(next) => tn = next,
                }
            }
        }

        // Finish by updating immediate dominators and levels.
        SemiNca::<G, POST>::update_insertion(dt, graph, batch, ncd, &ii);
    }

    /// `UpdateInsertion`: immediate dominators and levels after an insertion.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L836>
    fn update_insertion(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        ncd: Node,
        ii: &InsertionInfo,
    ) {
        for &tn in &ii.affected {
            dt.set_idom(tn, ncd);
        }
        for &tn in &ii.visited_unaffected {
            debug_assert!(
                dt.level_of(tn) == dt.level_of(dt.at(tn).idom) + 1,
                "TN should have been updated by an affected ancestor"
            );
        }
        if POST {
            SemiNca::<G, POST>::update_roots_after_update(dt, graph, batch);
        }
    }

    /// `InsertUnreachable`: insertion to previously unreachable nodes.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L857>
    fn insert_unreachable(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        from: Node,
        to: BlockId,
    ) {
        // Collect discovered edges to already reachable nodes.
        let mut discovered_edges_to_reachable: Vec<(BlockId, Node)> = Vec::new();
        // Discover and connect nodes that became reachable with the insertion.
        SemiNca::<G, POST>::compute_unreachable_dominators(
            dt,
            graph,
            batch,
            to,
            from,
            &mut discovered_edges_to_reachable,
        );
        // Used the discovered edges and inset discovered connecting (incoming)
        // edges.
        for (edge_from, edge_to) in discovered_edges_to_reachable {
            let from_node = dt.node(Some(edge_from)).expect("discovered from a node now in the tree");
            SemiNca::<G, POST>::insert_reachable(dt, graph, batch, from_node, edge_to);
        }
    }

    /// `ComputeUnreachableDominators`: connects nodes that become reachable
    /// with an insertion.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L883>
    fn compute_unreachable_dominators(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        root: BlockId,
        incoming: Node,
        discovered_connecting_edges: &mut Vec<(BlockId, Node)>,
    ) {
        debug_assert!(dt.node(Some(root)).is_none(), "Root must not be reachable");
        // Visit only previously unreachable nodes.
        let mut snca = SemiNca::<G, POST>::new(graph, batch);
        {
            let dt_ref: &DomTreeBase<POST> = dt;
            let unreachable_descender = |from: BlockId, to: BlockId| {
                let Some(to_tn) = dt_ref.node(Some(to)) else { return true };
                discovered_connecting_edges.push((from, to_tn));
                false
            };
            snca.run_dfs(false, root, 0, unreachable_descender, 0, None);
        }
        snca.run_semi_nca();
        dt.create_node(graph.max_number(), snca.num_to_node[0], Some(incoming));
        snca.attach_new_subtree(dt);
    }

    /// `DeleteEdge` (the internal one).
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L909>
    fn delete_edge(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        from: BlockId,
        to: BlockId,
    ) {
        // Ensure that the edge was in fact deleted from the CFG before
        // informing the DomTree about it. The check is O(N): debug only.
        debug_assert!(
            !SemiNca::<G, POST>::children(graph, batch, from, POST).contains(&to),
            "Deleted edge still exists in the CFG!"
        );

        let Some(from_tn) = dt.node(Some(from)) else {
            // Deletion in an unreachable subtree -- nothing to do.
            return;
        };
        let Some(to_tn) = dt.node(Some(to)) else {
            // To is already unreachable -- there is no edge to delete.
            return;
        };

        let ncd_block = dt.find_nearest_common_dominator(graph, from, to);
        let ncd = dt.node(ncd_block);

        // If To dominates From -- nothing to do.
        if Some(to_tn) != ncd {
            dt.dfs_info_valid.set(false);
            let to_idom = dt.at(to_tn).idom;
            // To remains reachable after deletion.
            // (Based on the caption under Figure 4. from [2].)
            if from_tn != to_idom || SemiNca::<G, POST>::has_proper_support(dt, graph, batch, to_tn) {
                SemiNca::<G, POST>::delete_reachable(dt, graph, batch, from_tn, to_tn);
            } else {
                SemiNca::<G, POST>::delete_unreachable(dt, graph, batch, to_tn);
            }
        }

        if POST {
            SemiNca::<G, POST>::update_roots_after_update(dt, graph, batch);
        }
    }

    /// `DeleteReachable`: deletions that leave the destination reachable.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L964>
    fn delete_reachable(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        from_tn: Node,
        to_tn: Node,
    ) {
        // Find the top of the subtree that needs to be rebuilt.
        // (Based on the lemma 2.6 from [2].)
        let from_block = dt.at(from_tn).block.expect("a real block");
        let to_block = dt.at(to_tn).block.expect("a real block");
        let to_idom = dt.find_nearest_common_dominator(graph, from_block, to_block);
        debug_assert!(to_idom.is_some() || POST);
        let to_idom_tn = dt.node(to_idom).expect("the NCD is in the tree");
        let prev_idom_sub_tree = dt.at(to_idom_tn).idom;
        // Top of the subtree to rebuild is the root node. Rebuild the tree
        // from scratch.
        if prev_idom_sub_tree == NO {
            SemiNca::<G, POST>::calculate_from_scratch(dt, graph, batch);
            return;
        }

        // Only visit nodes in the subtree starting at To.
        let level = dt.level_of(to_idom_tn);
        let mut snca = SemiNca::<G, POST>::new(graph, batch);
        {
            let dt_ref: &DomTreeBase<POST> = dt;
            let descend_below =
                |_: BlockId, to: BlockId| dt_ref.level_of(dt_ref.node(Some(to)).expect("in the tree")) > level;
            snca.run_dfs(false, to_idom.expect("a real block"), 0, descend_below, 0, None);
        }
        snca.run_semi_nca();
        snca.reattach_existing_subtree(dt, prev_idom_sub_tree);
    }

    /// `HasProperSupport`: as defined on the page 3 and later explained on the
    /// page 7 of [2].
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1005>
    fn has_proper_support(
        dt: &DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        tn: Node,
    ) -> bool {
        let tnb = dt.at(tn).block.expect("a real block");
        for pred in SemiNca::<G, POST>::children(graph, batch, tnb, !POST) {
            if dt.node(Some(pred)).is_none() {
                continue;
            }
            let support = dt.find_nearest_common_dominator(graph, tnb, pred);
            if support != Some(tnb) {
                return true;
            }
        }
        false
    }

    /// `DeleteUnreachable`: deletions that make the destination unreachable.
    /// (Based on the lemma 2.7 from the [2].)
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1030>
    fn delete_unreachable(
        dt: &mut DomTreeBase<POST>,
        graph: &G,
        batch: Batch<'_>,
        to_tn: Node,
    ) {
        let to_block = dt.at(to_tn).block.expect("a real block");
        if POST {
            // Deletion makes a region reverse-unreachable and creates a new
            // root. Simulate that by inserting an edge from the virtual root
            // to ToTN and adding it as a new root.
            dt.roots.push(to_block);
            let virtual_root = dt.node(None).expect("a post-dominator tree has a virtual root");
            SemiNca::<G, POST>::insert_reachable(dt, graph, batch, virtual_root, to_tn);
            return;
        }

        let mut affected_queue: Vec<BlockId> = Vec::new();
        let level = dt.level_of(to_tn);

        // Traverse destination node's descendants with greater level in the
        // tree and collect visited nodes.
        let mut snca = SemiNca::<G, POST>::new(graph, batch);
        let last_dfs_num = {
            let dt_ref: &DomTreeBase<POST> = dt;
            let descend_and_collect = |_: BlockId, to: BlockId| {
                let tn = dt_ref.node(Some(to)).expect("in the tree");
                if dt_ref.level_of(tn) > level {
                    return true;
                }
                if !affected_queue.contains(&to) {
                    affected_queue.push(to);
                }
                false
            };
            snca.run_dfs(false, to_block, 0, descend_and_collect, 0, None)
        };

        let mut min_node = to_tn;

        // Identify the top of the subtree to rebuild by finding the NCD of all
        // the affected nodes.
        for &n in &affected_queue {
            let tn = dt.node(Some(n)).expect("in the tree");
            let ncd_block = dt.find_nearest_common_dominator(graph, n, to_block);
            debug_assert!(ncd_block.is_some() || POST);
            let ncd = dt.node(ncd_block).expect("the NCD is in the tree");
            if ncd != tn && dt.level_of(ncd) < dt.level_of(min_node) {
                min_node = ncd;
            }
        }

        // Root reached, rebuild the whole tree from scratch.
        if dt.at(min_node).idom == NO {
            SemiNca::<G, POST>::calculate_from_scratch(dt, graph, batch);
            return;
        }

        // Erase the unreachable subtree in reverse preorder to process all
        // children before deleting their parent.
        for i in (0..last_dfs_num as usize).rev() {
            dt.erase_node_of(snca.num_to_node[i]);
        }

        // The affected subtree start at the To node -- there's no extra work
        // to do.
        if min_node == to_tn {
            return;
        }

        let min_level = dt.level_of(min_node);
        let prev_idom = dt.at(min_node).idom;
        debug_assert!(prev_idom != NO);
        snca.clear();

        // Identify nodes that remain in the affected subtree.
        let min_block = dt.at(min_node).block.expect("a real block");
        let max = graph.max_number();
        {
            let descend_below = |r: BlockId, to: BlockId| {
                if let Some(to_tn) = dt.node(Some(to)) {
                    return dt.level_of(to_tn) > min_level;
                }
                let parent = dt.node(Some(r));
                dt.create_node(max, Some(to), parent);
                true
            };
            snca.run_dfs(false, min_block, 0, descend_below, 0, None);
        }

        // Rebuild the remaining part of affected subtree.
        snca.run_semi_nca();
        snca.reattach_existing_subtree(dt, prev_idom);
    }

    /// `ApplyUpdates`: a batch of updates, in the order the legalized list
    /// gives, or a rebuild where there are too many.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1137>
    fn apply_updates(
        dt: &mut DomTreeBase<POST>,
        graph: &'a G,
        pre_view_cfg: &mut GraphDiff,
        post_view_cfg: Option<GraphDiff>,
    ) {
        // Note: the PostViewCFG is only used when computing from scratch. Its
        // data should already be included in the PreViewCFG for incremental
        // updates.
        let num_updates = pre_view_cfg.num_legalized_updates();
        if num_updates == 0 {
            return;
        }

        // Take the fast path for a single update and avoid running the batch
        // update machinery.
        if num_updates == 1 {
            let update = pre_view_cfg.pop_update_for_incremental_updates();
            match post_view_cfg {
                None => {
                    if update.kind == UpdateKind::Insert {
                        SemiNca::<G, POST>::insert_edge(dt, graph, None, Some(update.from), update.to);
                    } else {
                        SemiNca::<G, POST>::delete_edge(dt, graph, None, update.from, update.to);
                    }
                }
                Some(post) => {
                    let bui = BatchUpdateInfo::new(post.clone(), Some(post));
                    if update.kind == UpdateKind::Insert {
                        SemiNca::<G, POST>::insert_edge(dt, graph, Some(&bui), Some(update.from), update.to);
                    } else {
                        SemiNca::<G, POST>::delete_edge(dt, graph, Some(&bui), update.from, update.to);
                    }
                }
            }
            return;
        }

        let bui = BatchUpdateInfo::new(std::mem::take(pre_view_cfg), post_view_cfg);
        // Recalculate the DominatorTree when the number of updates exceeds a
        // threshold, which usually makes direct updating slower than
        // recalculation. We select this threshold proportional to the size of
        // the DominatorTree. The constant is selected by choosing the one with
        // an acceptable performance on some real-world inputs.

        // Make unittests of the incremental algorithm work
        if dt.nodes.len() <= 100 {
            if bui.num_legalized > dt.nodes.len() {
                SemiNca::<G, POST>::calculate_from_scratch(dt, graph, Some(&bui));
            }
        } else if bui.num_legalized > dt.nodes.len() / 40 {
            SemiNca::<G, POST>::calculate_from_scratch(dt, graph, Some(&bui));
        }

        // If the DominatorTree was recalculated at some point, stop the batch
        // updates. Full recalculations ignore batch updates and look at the
        // actual CFG.
        let mut i = 0;
        while i < bui.num_legalized && !bui.is_recalculated.get() {
            SemiNca::<G, POST>::apply_next_update(dt, graph, &bui);
            i += 1;
        }
    }

    /// `ApplyNextUpdate`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1186>
    fn apply_next_update(
        dt: &mut DomTreeBase<POST>,
        graph: &'a G,
        bui: &BatchUpdateInfo,
    ) {
        // Popping the next update, will move the PreViewCFG to the next
        // snapshot.
        let current = bui.pre_view_cfg.borrow_mut().pop_update_for_incremental_updates();
        if current.kind == UpdateKind::Insert {
            SemiNca::<G, POST>::insert_edge(dt, graph, Some(bui), Some(current.from), current.to);
        } else {
            SemiNca::<G, POST>::delete_edge(dt, graph, Some(bui), current.from, current.to);
        }
    }

    /// `verifyRoots`: a dominator tree has the entry for its single root; a
    /// post-dominator tree has the roots a fresh walk finds.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1212>
    fn verify_roots(
        &mut self,
        dt: &DomTreeBase<POST>,
    ) -> Result<(), String> {
        if !dt.calculated && !dt.roots.is_empty() {
            return Err("Tree has no parent but has roots!".into());
        }
        if !POST {
            if dt.roots.is_empty() {
                return Err("Tree doesn't have a root!".into());
            }
            if Some(dt.root()) != self.graph.entry() {
                return Err("Tree's root is not its parent's entry node!".into());
            }
        }
        let computed_roots = SemiNca::<G, POST>::find_roots(self.graph, None);
        if !SemiNca::<G, POST>::is_permutation(&dt.roots, &computed_roots) {
            let names = |roots: &[BlockId]| roots.iter().map(|n| format!("%{}, ", n.0)).collect::<String>();
            return Err(format!(
                "Tree has different roots than freshly computed ones!\n\tPDT roots: {}\n\tComputed roots: {}",
                names(&dt.roots),
                names(&computed_roots)
            ));
        }
        Ok(())
    }

    /// `verifyReachability`: the tree has every node the graph reaches and no
    /// other.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1252>
    fn verify_reachability(
        &mut self,
        dt: &DomTreeBase<POST>,
    ) -> Result<(), String> {
        self.clear();
        self.do_full_dfs_walk(dt, |_, _| true);
        for tn in dt.nodes.iter().flatten() {
            // Virtual root has a corresponding virtual CFG node.
            let Some(bb) = tn.block else { continue };
            if self.info(Some(bb)).dfs_num_plus1 == UNVISITED {
                return Err(format!("DomTree node %{} not found by DFS walk!", bb.0));
            }
        }
        for n in self.num_to_node.iter().flatten() {
            if dt.node(Some(*n)).is_none() {
                return Err(format!("CFG node %{} not found in the DomTree!", n.0));
            }
        }
        Ok(())
    }

    /// `verifyParentProperty`: for all edges from V to W in the graph, such
    /// that V is reachable, the parent of W in the tree is an ancestor of V.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1462>
    fn verify_parent_property(
        &mut self,
        dt: &DomTreeBase<POST>,
    ) -> Result<(), String> {
        for (index, tn) in dt.nodes.iter().enumerate() {
            let Some(tn) = tn else { continue };
            let Some(bb) = tn.block else { continue };
            if tn.first_child == NO {
                continue;
            }
            self.clear();
            self.do_full_dfs_walk(dt, |from, to| from != bb && to != bb);
            for child in dt.children_of(index as Node) {
                let child_block = dt.at(child).block.expect("a real block");
                if self.info(Some(child_block)).dfs_num_plus1 != UNVISITED {
                    return Err(format!("Child %{} reachable after its parent %{} is removed!", child_block.0, bb.0));
                }
            }
        }
        Ok(())
    }

    /// `verifySiblingProperty`: no sibling dominates another.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1497>
    fn verify_sibling_property(
        &mut self,
        dt: &DomTreeBase<POST>,
    ) -> Result<(), String> {
        for (index, tn) in dt.nodes.iter().enumerate() {
            let Some(tn) = tn else { continue };
            if tn.block.is_none() || tn.first_child == NO {
                continue;
            }
            let siblings = dt.children_of(index as Node);
            for &n in &siblings {
                self.clear();
                let bbn = dt.at(n).block.expect("a real block");
                self.do_full_dfs_walk(dt, |from, to| from != bbn && to != bbn);
                for &s in &siblings {
                    if s == n {
                        continue;
                    }
                    let sibling = dt.at(s).block.expect("a real block");
                    if self.info(Some(sibling)).dfs_num_plus1 == UNVISITED {
                        return Err(format!(
                            "Node %{} not reachable when its sibling %{} is removed!",
                            sibling.0, bbn.0
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// `IsSameAsFreshTree`: the tree is the one a build from scratch gives.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/GenericDomTreeConstruction.h#L1538>
    fn is_same_as_fresh_tree(
        &self,
        dt: &DomTreeBase<POST>,
    ) -> Result<(), String> {
        let mut fresh = DomTreeBase::<POST>::new();
        fresh.recalculate(self.graph);
        if dt.compare(&fresh) {
            return Err(format!(
                "{}DominatorTree is different than a freshly computed one!\n\tCurrent:\n{}\n\tFreshly computed tree:\n{}",
                if POST { "Post" } else { "" },
                dt.print(),
                fresh.print()
            ));
        }
        Ok(())
    }
}

impl PartialEq for DomTreeBase<false> {
    /// Two trees are the same where `compare` finds no difference.
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        !self.compare(other)
    }
}

/// The trees [`DomTree::of`] keeps to copy.
const MEMO_TREES: usize = 8;

thread_local! {
    static MEMO: std::cell::RefCell<Vec<(Vec<u32>, DomTree)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn check_memo() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_env::set("LLRM_CHECK_DOMTREE_MEMO"))
}

/// Everything a dominator tree of `function` is made of.
fn cfg_key(function: &Function) -> Vec<u32> {
    const EDGE: u32 = 1 << 31;
    let mut key = Vec::with_capacity(function.layout().len() * 3 + 1);
    key.push(function.block_capacity() as u32);
    for &block in function.layout() {
        key.push(block.0);
        if let Some(term) = function.terminator(block) {
            key.extend(function.instruction(term).operands.iter().filter_map(|operand| match operand {
                Operand::Block(target) => Some(target.0 | EDGE),
                _ => None,
            }));
        }
    }
    key
}

impl DomTree {
    /// The tree of `function`. Where a tree was built for the same CFG (the
    /// layout and each terminator's block operands), that tree is copied: a
    /// tree is a function of the CFG alone, and the key is compared whole, so
    /// no pass has to say what it changed. `LLRM_CHECK_DOMTREE_MEMO` builds
    /// the tree again at each reuse and fails on a difference.
    pub fn of(function: &Function) -> Self {
        let key = cfg_key(function);
        if let Some(tree) =
            MEMO.with(|memo| memo.borrow().iter().find(|(seen, _)| *seen == key).map(|(_, tree)| tree.clone()))
        {
            crate::counts::bump(crate::counts::Event::DominatorsReused);
            if check_memo() {
                assert!(tree == Self::built(function), "a reused dominator tree differs from one built now");
            }
            return tree;
        }
        let tree = Self::built(function);
        MEMO.with(|memo| {
            let mut memo = memo.borrow_mut();
            if memo.len() == MEMO_TREES {
                memo.remove(0);
            }
            memo.push((key, tree.clone()));
        });
        tree
    }

    fn built(function: &Function) -> Self {
        crate::counts::bump(crate::counts::Event::Dominators);
        let mut tree = Self::new();
        tree.recalculate(function);
        tree
    }

    /// Whether `block` is reachable from the entry.
    pub fn is_reachable(
        &self,
        block: BlockId,
    ) -> bool {
        self.is_reachable_from_entry(block)
    }

    /// Whether `def` comes before `user` on every path to `user`.
    pub fn instruction_dominates(
        &self,
        function: &Function,
        def: InstId,
        user: InstId,
    ) -> bool {
        let (Some(a), Some(b)) = (function.parent(def), function.parent(user)) else { return false };
        if a != b {
            return self.dominates(a, b);
        }
        if !self.is_reachable(b) {
            return true;
        }
        // A phi is dominated only if the definition dominates every possible
        // use in its block: an earlier phi of that block does not
        // (`dominates(Value *, Instruction *)`, [L123][phi]).
        //
        // [phi]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L123
        if function.instruction(user).opcode == crate::opcode::Opcode::Phi {
            return false;
        }
        function.comes_before(def, user)
    }

    /// `findNearestCommonDominator(Instruction *, Instruction *)`: the earlier
    /// of two in one block, the one in the other's dominating block, else the
    /// terminator of the blocks' nearest common dominator.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L309>
    pub fn find_nearest_common_dominator_inst(
        &self,
        function: &Function,
        i1: InstId,
        i2: InstId,
    ) -> InstId {
        let (bb1, bb2) = (function.parent(i1).expect("placed"), function.parent(i2).expect("placed"));
        if bb1 == bb2 {
            return if function.comes_before(i1, i2) { i1 } else { i2 };
        }
        if !self.is_reachable(bb2) {
            return i1;
        }
        if !self.is_reachable(bb1) {
            return i2;
        }
        let dom = self.find_nearest_common_dominator(function, bb1, bb2).expect("a forward tree has a block");
        if bb1 == dom {
            i1
        } else if bb2 == dom {
            i2
        } else {
            function.terminator(dom).expect("a block ends in a terminator")
        }
    }

    /// `instruction_dominates`, where `position` is each instruction's index in
    /// its block (`Function::positions`): no scan of the block for two in
    /// the same one.
    pub fn instruction_dominates_at(
        &self,
        function: &Function,
        def: InstId,
        user: InstId,
        position: &[u32],
    ) -> bool {
        let (Some(a), Some(b)) = (function.parent(def), function.parent(user)) else { return false };
        if a != b {
            return self.dominates(a, b);
        }
        !self.is_reachable(b) || position[def.0 as usize] < position[user.0 as usize]
    }

    /// The tree of `function` after `changes`, from this one of the function
    /// before them: the edges the log says terminators lost and gained go to
    /// `applyUpdates`. None where the changes did something else to the graph
    /// (a block gone, a terminator moved, another entry): the caller builds
    /// afresh.
    pub fn updated(
        &self,
        function: &Function,
        changes: &[Change],
    ) -> Option<Self> {
        let mut tree = self.clone();
        tree.update_in_place(function, changes).then_some(tree)
    }

    /// `updated`, of this tree itself. Where it says false the tree is
    /// untouched.
    pub fn update_in_place(
        &mut self,
        function: &Function,
        changes: &[Change],
    ) -> bool {
        let Some(updates) = self.updates_for(function, changes) else { return false };
        self.apply_updates(function, &updates);
        true
    }

    /// The edge updates `changes` make to the CFG this tree is of, for
    /// `applyUpdates` or a `DomTreeUpdater`. None where the changes did
    /// something else to the graph (another entry, an operand out of range).
    pub fn updates_for(
        &self,
        function: &Function,
        changes: &[Change],
    ) -> Option<Vec<Update>> {
        let entry = function.entry()?;
        if self.roots.first() != Some(&entry) {
            return None;
        }
        edge_updates(function, changes)
    }
}

thread_local! {
    static SCANS: Cell<usize> = const { Cell::new(0) };
}

/// How many times this thread has scanned a block's instructions to order two
/// of them, for a test that the verifier does not.
pub fn scans() -> usize {
    SCANS.with(Cell::get)
}

/// The edges the terminators the log touched lost and gained, as `Update`s.
/// A terminator's old block and operands are what it has now with the log
/// undone from the last change; its edges then and now are compared as pairs.
/// None where an edge change cannot be told (an operand index out of range).
fn edge_updates(
    function: &Function,
    changes: &[Change],
) -> Option<Vec<Update>> {
    struct Touched {
        /// Where it sat before the log, None if it was inserted by it.
        block: Option<BlockId>,
        operands: Vec<Operand>,
    }
    // Sorted by instruction, looked up by search: a vector indexed by id is
    // as long as the function has instructions, for each update.
    let terminator = |inst: InstId| function.instruction(inst).opcode.is_terminator();
    let mut insts: Vec<InstId> = Vec::new();
    for change in changes {
        match change {
            Change::Operand { inst, .. }
            | Change::Operands { inst, .. }
            | Change::Inserted { inst, .. }
            | Change::Erased { inst, .. }
            | Change::Moved { inst, .. } => {
                if terminator(*inst) {
                    insts.push(*inst);
                }
            }
            _ => {}
        }
    }
    insts.sort_unstable();
    insts.dedup();
    let mut touched: Vec<Touched> = insts
        .iter()
        .map(|&inst| Touched { block: function.parent(inst), operands: function.instruction(inst).operands.clone() })
        .collect();
    let slot = |inst: &InstId| insts.binary_search(inst).ok();
    for change in changes.iter().rev() {
        match change {
            Change::Operand { inst, index, was } => {
                if let Some(at) = slot(inst) {
                    *touched[at].operands.get_mut(*index as usize)? = *was;
                }
            }
            Change::Operands { inst, was } => {
                if let Some(at) = slot(inst) {
                    touched[at].operands = was.to_vec();
                }
            }
            Change::Inserted { inst, .. } => {
                if let Some(at) = slot(inst) {
                    touched[at].block = None;
                }
            }
            Change::Erased { inst, block, .. } => {
                if let Some(at) = slot(inst) {
                    touched[at].block = Some(*block);
                }
            }
            Change::Moved { inst, from, .. } => {
                if let Some(at) = slot(inst) {
                    touched[at].block = Some(*from);
                }
            }
            _ => {}
        }
    }
    let edges = |block: Option<BlockId>, operands: &[Operand]| -> Vec<(BlockId, BlockId)> {
        let mut out: Vec<(BlockId, BlockId)> = Vec::new();
        for one in operands {
            if let (Some(block), Operand::Block(target)) = (block, one)
                && !out.contains(&(block, *target))
            {
                out.push((block, *target));
            }
        }
        out
    };
    let mut updates = Vec::new();
    for (&inst, one) in insts.iter().zip(&touched) {
        let old = edges(one.block, &one.operands);
        let now_block = if function.is_erased(inst) { None } else { function.parent(inst) };
        let now = edges(now_block, &function.instruction(inst).operands);
        for &(from, to) in old.iter().filter(|edge| !now.contains(edge)) {
            updates.push(Update::delete(from, to));
        }
        for &(from, to) in now.iter().filter(|edge| !old.contains(edge)) {
            updates.push(Update::insert(from, to));
        }
    }
    Some(updates)
}

#[cfg(test)]
#[path = "domtree_tests.rs"]
mod tests;
