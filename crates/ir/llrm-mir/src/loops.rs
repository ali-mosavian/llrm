//! Natural loops, as LLVM's LoopInfo: the target of an edge that it
//! dominates heads a loop, whose blocks are those reaching that edge's
//! source without passing the header. Back edges into one header make one
//! loop.

use std::collections::BTreeMap;

use crate::dominators::DominatorTree;
use crate::module::{BlockId, Function, MetadataNode, MetadataOperand};

#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    pub header: BlockId,
    pub blocks: Body,
    pub latches: Vec<BlockId>,
    /// The innermost loop holding this one.
    pub parent: Option<usize>,
}

/// The blocks of one loop, those of the loops inside it too, as the forest of
/// the function holds them: asking whether a block is in it is a lookup, and
/// naming them all costs what they are, the first time.
#[derive(Clone)]
pub struct Body {
    ids: crate::forest::Body,
    named: std::rc::Rc<std::cell::OnceCell<Vec<BlockId>>>,
}

impl Body {
    pub fn contains(
        &self,
        block: &BlockId,
    ) -> bool {
        self.ids.contains(&i64::from(block.0))
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The same blocks as addresses.
    pub fn ids(&self) -> &crate::forest::Body {
        &self.ids
    }

    /// The blocks in ascending order.
    pub fn iter(&self) -> std::slice::Iter<'_, BlockId> {
        self.named.get_or_init(|| self.ids.iter().map(|&at| BlockId(at as u32)).collect()).iter()
    }
}

impl std::fmt::Debug for Body {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.ids.fmt(out)
    }
}

impl PartialEq for Body {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.ids == other.ids
    }
}

impl<'a> IntoIterator for &'a Body {
    type Item = &'a BlockId;
    type IntoIter = std::slice::Iter<'a, BlockId>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoopInfo {
    pub loops: Vec<Loop>,
    /// Each block's innermost loop.
    innermost: BTreeMap<BlockId, usize>,
}

impl LoopInfo {
    pub fn new(
        function: &Function,
        tree: &DominatorTree,
    ) -> Self {
        let nodes: Vec<(i64, crate::inline::InlineVec<i64, 2>)> = function
            .layout()
            .iter()
            .map(|&block| (i64::from(block.0), function.successors(block).iter().map(|one| i64::from(one.0)).collect()))
            .collect();
        let slices: Vec<(i64, &[i64])> = nodes.iter().map(|(at, succ)| (*at, succ.as_slice())).collect();
        let forest = std::rc::Rc::new(crate::forest::forest(
            &slices,
            function.layout().first().map(|block| i64::from(block.0)),
            &|at| tree.is_reachable(BlockId(at as u32)),
            &|dominator, at| tree.dominates(BlockId(dominator as u32), BlockId(at as u32)),
        ));
        // Outer loops first, so each block's innermost loop is its last; the
        // larger first, and by header among the same size.
        let mut order: Vec<usize> = (0..forest.loops.len()).collect();
        order.sort_by_key(|&index| (std::cmp::Reverse(forest.loops[index].size), forest.loops[index].header));
        let mut place = vec![0; forest.loops.len()];
        for (at, &index) in order.iter().enumerate() {
            place[index] = at;
        }
        let loops: Vec<Loop> = order
            .iter()
            .map(|&index| {
                let one = &forest.loops[index];
                Loop {
                    header: BlockId(one.header as u32),
                    blocks: Body { ids: crate::forest::Body::new(&forest, index), named: Default::default() },
                    latches: one.latches.iter().map(|&at| BlockId(at as u32)).collect(),
                    parent: one.parent.map(|parent| place[parent]),
                }
            })
            .collect();
        let innermost = function
            .layout()
            .iter()
            .filter_map(|&block| forest.innermost(i64::from(block.0)).map(|index| (block, place[index])))
            .collect();
        Self { loops, innermost }
    }

    /// The innermost loop holding `block`.
    pub fn loop_of(
        &self,
        block: BlockId,
    ) -> Option<&Loop> {
        self.innermost.get(&block).map(|&at| &self.loops[at])
    }

    /// Whether going from `from` to `to` leaves a loop: one holds `from`
    /// and not `to`.
    pub fn leaves(
        &self,
        from: BlockId,
        to: BlockId,
    ) -> bool {
        let mut at = self.innermost.get(&from).copied();
        while let Some(one) = at {
            if !self.loops[one].blocks.contains(&to) {
                return true;
            }
            at = self.loops[one].parent;
        }
        false
    }

    /// How many loops hold `block`: 0 outside any.
    pub fn depth(
        &self,
        block: BlockId,
    ) -> u32 {
        let mut depth = 0;
        let mut at = self.innermost.get(&block).copied();
        while let Some(one) = at {
            depth += 1;
            at = self.loops[one].parent;
        }
        depth
    }
}

/// Whether `one` has an edge out: a loop with none never ends, whatever marks
/// it.
pub fn exits(
    function: &Function,
    one: &Loop,
) -> bool {
    !one.latches.is_empty()
        && one.blocks.iter().any(|&block| function.successors(block).iter().any(|next| !one.blocks.contains(next)))
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
    let tree = DominatorTree::new(function);
    LoopInfo::new(function, &tree)
        .loops
        .iter()
        .all(
            |one| exits(function, one) && (whole || one.latches.iter().all(|&latch| marked(metadata, function, latch))),
        )
}
