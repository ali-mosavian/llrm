use super::*;
use crate::blocklist::BlockList;
use crate::cycleinfo::CycleInfo;
use crate::domtree::DomGraph;
use crate::module::BlockId;

/// A graph given by its successor lists, repeats kept, block 0 the entry.
struct Graph {
    succs: Vec<Vec<u32>>,
    blocks: Vec<BlockId>,
    probs: Vec<Vec<BranchProbability>>,
}

impl DomGraph for Graph {
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
        let mut out = BlockList::new();
        for &one in &self.succs[block.0 as usize] {
            if !out.contains(&BlockId(one)) {
                out.push(BlockId(one));
            }
        }
        out
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        (0..self.succs.len() as u32)
            .flat_map(|from| {
                self.succs[from as usize].iter().filter(move |&&to| to == block.0).map(move |_| BlockId(from))
            })
            .collect()
    }
}

impl FlowGraph for Graph {
    fn children(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.succs[block.0 as usize].iter().map(|&one| BlockId(one)).collect()
    }
}

impl EdgeProbabilities for Graph {
    fn get_edge_probability(
        &self,
        src: BlockId,
        index: usize,
    ) -> BranchProbability {
        self.probs[src.0 as usize][index]
    }
}

fn lists(text: &str) -> Vec<Vec<u32>> {
    text.split(';')
        .map(|one| {
            let items = one.split_once(':').map_or(one, |(_, items)| items);
            items.split(',').filter(|item| !item.is_empty()).map(|item| item.parse().unwrap()).collect()
        })
        .collect()
}

/// What the port makes of one graph, in the golden file's words.
fn computed(line: &str) -> String {
    let parts: Vec<&str> = line.split(" | ").collect();
    let succs = lists(parts[0].trim());
    let weights = lists(parts[1].trim());
    let probs = weights
        .iter()
        .map(|ws| {
            let mut probs: Vec<_> = ws.iter().map(|&w| BranchProbability::get_raw(w)).collect();
            BranchProbability::normalize_probabilities(&mut probs);
            probs
        })
        .collect();
    let blocks = (0..succs.len() as u32).map(BlockId).collect();
    let graph = Graph { succs, blocks, probs };
    let ci = CycleInfo::compute(&graph);
    let bfi = BlockFrequencyInfo::calculate(&graph, &graph, &ci);
    let n = graph.succs.len() as u32;
    let mut out = format!("{} | {} | freq", parts[0], parts[1]);
    for b in 0..n {
        out += &format!(" {}", bfi.get_block_freq(BlockId(b)).get_frequency());
    }
    out += " | irr";
    for b in (0..n).filter(|&b| bfi.is_irr_loop_header(BlockId(b))) {
        out += &format!(" {b}");
    }
    out += " | cycles";
    for c in ci.cycles() {
        let blocks: Vec<String> = ci.get_blocks(c).iter().map(|one| one.0.to_string()).collect();
        let kind = if ci.is_reducible(c) { "r" } else { "i" };
        out += &format!(" {}:{}:{kind}:{}", ci.get_header(c).0, ci.get_depth(c), blocks.join(","));
    }
    out
}

/// Every graph of `golden.txt` (`tools/llvmport/blockfrequency_dump.sh`):
/// the cycles, the irreducible headers and each block's integer frequency
/// LLVM's CycleInfo and BlockFrequencyInfo give it.
#[test]
fn test_cycles_and_frequencies_match_llvm_on_every_golden_graph() {
    let golden = include_str!("golden.txt");
    let mut wrong = Vec::new();
    let mut lines = 0;
    for line in golden.lines().filter(|one| !one.starts_with('#')) {
        lines += 1;
        let ours = computed(line);
        if ours != line {
            wrong.push(format!("llvm {line}\nours {ours}"));
        }
    }
    assert!(lines > 3000, "{lines} graphs");
    assert!(
        wrong.is_empty(),
        "{} of {lines} differ, the first:\n{}",
        wrong.len(),
        wrong[..wrong.len().min(3)].join("\n")
    );
}
