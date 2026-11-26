use super::*;
use crate::blocklist::BlockList;
use crate::domtree::DomGraph;
use crate::module::BlockId;

struct Cfg {
    succs: Vec<Vec<u32>>,
    blocks: Vec<BlockId>,
}

impl DomGraph for Cfg {
    fn entry(&self) -> Option<BlockId> {
        Some(BlockId(0))
    }

    fn max_number(&self) -> usize {
        self.succs.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.blocks
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.succs[block.0 as usize].iter().map(|one| BlockId(*one)).collect()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        (0..self.succs.len() as u32).filter(|from| self.succs[*from as usize].contains(&block.0)).map(BlockId).collect()
    }
}

impl FlowGraph for Cfg {}

/// exec.c -Os: a dispatch loop whose handlers are entered from many blocks
/// outside it. Each such edge pushed the same (header, block) pair, and the
/// pin's sort of the pairs took 857,465 of them for 16,122 distinct ones: the
/// sort was 0.8 G instructions of the 4.2 G the `jumps` pass took.
#[test]
fn many_edges_into_one_finished_loop_block_record_one_reentry() {
    // 0 -> {x1..xm, 1}; 1 -> {2, 3}; 2 -> 1; 3 -> 1; each xk -> 2.
    let m = 50u32;
    let (dispatch, handler) = (m + 1, m + 2);
    let mut succs = vec![Vec::new(); (m + 4) as usize];
    succs[0] = (1..=m).chain([dispatch]).collect();
    succs[dispatch as usize] = vec![handler, handler + 1];
    succs[handler as usize] = vec![dispatch];
    succs[(handler + 1) as usize] = vec![dispatch];
    for k in 1..=m {
        succs[k as usize] = vec![handler];
    }
    let blocks = (0..succs.len() as u32).map(BlockId).collect();
    CycleInfo::compute(&Cfg { succs, blocks });
    assert_eq!(RECORDED.get(), 1, "an edge into the loop recorded its pair again");
}
