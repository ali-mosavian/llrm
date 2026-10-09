//! The blocks of a body by position, and each one's predecessors: asked of a
//! body by every pass that walks its edges (a liveness for a few values, an
//! interval, a carve, a spill's flow), and the same for every body that keeps
//! the blocks' labels and successors, which is every rewrite of the
//! instructions. The manager keeps it with the body (`analysis::facts`).

use std::sync::Arc;

use crate::analysis::facts::Fact;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// The blocks' positions by label, and the positions of each block's
/// predecessors, in the order the blocks list their successors.
#[derive(Debug, PartialEq)]
pub struct Graph {
    pub position: IndexMap<i64, usize>,
    pub parents: Vec<Vec<usize>>,
}

impl Graph {
    pub fn of(body: &LirBody) -> Arc<Self> {
        body.facts.0.get::<Edges>(body)
    }

    /// The labels of the predecessors of the block at position `at`.
    pub fn predecessors<'a>(
        &'a self,
        body: &'a LirBody,
        at: usize,
    ) -> impl Iterator<Item = i64> + 'a {
        self.parents[at].iter().map(|parent| body.blocks[*parent].at)
    }
}

/// The blocks' labels and successors, which a rewrite of the instructions
/// leaves.
pub struct Shape {
    entry: i64,
    blocks: Vec<(i64, Vec<i64>)>,
}

impl Shape {
    pub fn of(body: &LirBody) -> Self {
        Shape { entry: body.entry, blocks: body.blocks.iter().map(|block| (block.at, block.succ.clone())).collect() }
    }

    /// Whether `body` has this entry, these labels and these successors.
    pub fn holds(
        &self,
        body: &LirBody,
    ) -> bool {
        self.entry == body.entry
            && self.blocks.len() == body.blocks.len()
            && self.blocks.iter().zip(&body.blocks).all(|((at, succ), block)| *at == block.at && *succ == block.succ)
    }
}

/// How many loops contain each block: a fact of the control-flow graph, so the
/// same for every body that has the blocks' labels and successors (a split adds
/// instructions, not blocks).
pub struct LoopDepths;

impl Fact for LoopDepths {
    type Result = IndexMap<i64, u32>;
    type Inputs = Shape;
    const NAME: &'static str = "loop-depths";

    fn run(body: &LirBody) -> IndexMap<i64, u32> {
        crate::analysis::intervals::depths_afresh(body)
    }

    fn inputs(body: &LirBody) -> Shape {
        Shape::of(body)
    }

    fn held_by(
        kept: &Shape,
        body: &LirBody,
    ) -> bool {
        kept.holds(body)
    }
}

pub struct Edges;

impl Fact for Edges {
    type Result = Graph;
    type Inputs = Shape;
    const NAME: &'static str = "graph";

    fn run(body: &LirBody) -> Graph {
        let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
        let mut parents: Vec<Vec<usize>> = vec![Vec::new(); body.blocks.len()];
        for (at, block) in body.blocks.iter().enumerate() {
            for to in &block.succ {
                if let Some(&to) = position.get(to) {
                    parents[to].push(at);
                }
            }
        }
        Graph { position, parents }
    }

    fn inputs(body: &LirBody) -> Shape {
        Shape::of(body)
    }

    fn held_by(
        kept: &Shape,
        body: &LirBody,
    ) -> bool {
        kept.holds(body)
    }
}

#[cfg(test)]
mod tests {
    use super::{Edges, Graph};
    use crate::model::lir::{LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn diamond() -> LirBody {
        let block = |at: i64, succ: &[i64]| LirBlock { succ: succ.to_vec(), ..LirBlock::new(at, Vec::new()) };
        LirBody::new(
            "g",
            1,
            vec![block(1, &[2, 3]), block(2, &[3]), block(3, &[])],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    /// Every pass that walks a body's edges found the predecessors by a pass
    /// over the blocks of its own (some thirty of them, a few thousand
    /// times a compile). They are the body's, found once for every body that
    /// keeps the labels and successors, and again when an edge moves.
    #[test]
    fn test_the_predecessors_are_found_once_for_bodies_that_keep_the_edges() {
        let body = diamond();
        let graph = Graph::of(&body);
        assert_eq!(graph.parents[graph.position[&3]].iter().map(|at| body.blocks[*at].at).collect::<Vec<_>>(), [1, 2]);
        let rewritten = body.with_blocks(body.blocks.iter().map(|block| block.with_insns(Vec::new())).collect());
        Graph::of(&rewritten);
        assert_eq!(body.facts.0.runs::<Edges>(), 1, "a rewrite of the instructions asked the edges again");
        let mut moved = body.blocks.clone();
        moved[1].succ = vec![];
        let moved = body.with_blocks(moved);
        assert_eq!(Graph::of(&moved).parents[2], [0], "an edge that moved");
        assert_eq!(body.facts.0.runs::<Edges>(), 2, "a moved edge was answered from the old graph");
    }

    /// The edge bundles and the loop depths were worked out again after every
    /// split (490 splits of d_faces -O1: ~270 M of 27 G instructions), though a
    /// split adds instructions and never a block. They are the graph's: found
    /// once for every body that keeps the labels and successors.
    #[test]
    fn test_bundles_and_depths_are_found_once_for_bodies_that_keep_the_edges() {
        let body = diamond();
        let first = crate::backend::spillplacement::edge_bundles(&body);
        let depths = crate::analysis::intervals::depths_shared(&body);
        let rewritten = body.with_blocks(body.blocks.iter().map(|block| block.with_insns(Vec::new())).collect());
        assert_eq!(*crate::backend::spillplacement::edge_bundles(&rewritten), *first);
        assert_eq!(*crate::analysis::intervals::depths_shared(&rewritten), *depths);
        assert_eq!(body.facts.0.runs::<crate::backend::spillplacement::EdgeBundles>(), 1, "bundles asked again");
        assert_eq!(body.facts.0.runs::<super::LoopDepths>(), 1, "depths asked again");
        let mut moved = body.blocks.clone();
        moved[1].succ = vec![];
        let moved = body.with_blocks(moved);
        crate::backend::spillplacement::edge_bundles(&moved);
        assert_eq!(body.facts.0.runs::<crate::backend::spillplacement::EdgeBundles>(), 2, "a moved edge, old bundles");
    }
}
