//! The Ext-TSP block layout: `computeExtTspLayout` and `calcExtTspScore` of
//! [CodeLayout.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CodeLayout.cpp),
//! which `MachineBlockPlacement::applyExtTsp` runs at `-apply-ext-tsp-for-size`.
//!
//! # How it works
//! Every block is a chain. Chains are merged in three passes: blocks that are
//! each other's only successor and predecessor (`mergeForcedPairs`), the pair
//! of chains whose merge raises the score most, in every way of cutting one
//! chain in two (`mergeChainPairs`), then what is left in its original order
//! (`mergeColdChains`). The score of a layout rewards a jump by how near its
//! target is: a fall-through most, a forward jump within 1024 bytes and a
//! backward one within 640 less.
//!
//! ```text
//! chains   [a b] [c] [d e]      a -> d is the hottest jump
//! merge    [a b] + [d e]  =>  [a b d e] [c]     (c stays where it was)
//! ```
//!
//! # Example
//! Blocks 0 -> 1 -> 2 with a jump 0 -> 2 of the same weight: the order stays
//! 0, 1, 2, and every jump scores as a short forward or a fall-through.
//!
//! DEVIATE: `CDSort` (`computeCacheDirectedLayout`) is not translated; nothing
//! here uses it. The merged node lists are built as vectors where the pin
//! walks three ranges (`MergedNodesT`).

/// A jump of the layout graph (`codelayout::EdgeCount`).
#[derive(Clone, Copy, Debug)]
pub(super) struct EdgeCount {
    pub(super) src: usize,
    pub(super) dst: usize,
    pub(super) count: u64,
}

/// `EPS`.
const EPS: f64 = 1e-8;
/// `ext-tsp-forward-weight-cond`.
const FORWARD_WEIGHT_COND: f64 = 0.1;
/// `ext-tsp-forward-weight-uncond`.
const FORWARD_WEIGHT_UNCOND: f64 = 0.1;
/// `ext-tsp-backward-weight-cond`.
const BACKWARD_WEIGHT_COND: f64 = 0.1;
/// `ext-tsp-backward-weight-uncond`.
const BACKWARD_WEIGHT_UNCOND: f64 = 0.1;
/// `ext-tsp-fallthrough-weight-cond`.
const FALLTHROUGH_WEIGHT_COND: f64 = 1.0;
/// `ext-tsp-fallthrough-weight-uncond`.
const FALLTHROUGH_WEIGHT_UNCOND: f64 = 1.05;
/// `ext-tsp-forward-distance`.
const FORWARD_DISTANCE: u64 = 1024;
/// `ext-tsp-backward-distance`.
const BACKWARD_DISTANCE: u64 = 640;
/// `ext-tsp-max-chain-size`.
const MAX_CHAIN_SIZE: usize = 512;
/// `ext-tsp-chain-split-threshold`.
const CHAIN_SPLIT_THRESHOLD: usize = 128;
/// `ext-tsp-max-merge-density-ratio`.
const MAX_MERGE_DENSITY_RATIO: f64 = 100.0;

/// `jumpExtTSPScore`.
fn jump_score(
    distance: u64,
    max_distance: u64,
    count: u64,
    weight: f64,
) -> f64 {
    if distance > max_distance {
        return 0.0;
    }
    let probability = 1.0 - distance as f64 / max_distance as f64;
    weight * probability * count as f64
}

/// `extTSPScore`: a jump between blocks at the given addresses.
fn score_of_jump(
    src_addr: u64,
    src_size: u64,
    dst_addr: u64,
    count: u64,
    conditional: bool,
) -> f64 {
    let end = src_addr + src_size;
    if end == dst_addr {
        return jump_score(
            0,
            1,
            count,
            if conditional { FALLTHROUGH_WEIGHT_COND } else { FALLTHROUGH_WEIGHT_UNCOND },
        );
    }
    if end < dst_addr {
        return jump_score(
            dst_addr - end,
            FORWARD_DISTANCE,
            count,
            if conditional { FORWARD_WEIGHT_COND } else { FORWARD_WEIGHT_UNCOND },
        );
    }
    jump_score(
        end - dst_addr,
        BACKWARD_DISTANCE,
        count,
        if conditional { BACKWARD_WEIGHT_COND } else { BACKWARD_WEIGHT_UNCOND },
    )
}

/// `codelayout::calcExtTspScore`: the score of `order`.
pub(super) fn score(
    order: &[usize],
    sizes: &[u64],
    edges: &[EdgeCount],
) -> f64 {
    let mut addr = vec![0u64; sizes.len()];
    for at in 1..order.len() {
        addr[order[at]] = addr[order[at - 1]] + sizes[order[at - 1]];
    }
    let mut out_degree = vec![0u64; sizes.len()];
    for edge in edges {
        out_degree[edge.src] += 1;
    }
    edges
        .iter()
        .map(|edge| {
            score_of_jump(addr[edge.src], sizes[edge.src], addr[edge.dst], edge.count, out_degree[edge.src] > 1)
        })
        .sum()
}

/// `codelayout::calcExtTspScore` of the original order.
pub(super) fn original_score(
    sizes: &[u64],
    edges: &[EdgeCount],
) -> f64 {
    let order: Vec<usize> = (0..sizes.len()).collect();
    score(&order, sizes, edges)
}

/// `MergeTypeT`: chain X is cut in X1 and X2 and joined with Y in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MergeType {
    XY,
    YX,
    X1YX2,
    YX2X1,
    X2X1Y,
}

/// `MergeGainT`.
#[derive(Clone, Copy, Debug)]
struct MergeGain {
    score: f64,
    offset: usize,
    ty: MergeType,
}

impl MergeGain {
    fn none() -> Self {
        Self { score: -1.0, offset: 0, ty: MergeType::XY }
    }

    /// `operator<`: whether `other` is preferred.
    fn less_than(
        &self,
        other: &MergeGain,
    ) -> bool {
        other.score > EPS && other.score > self.score + EPS
    }

    fn update_if_less_than(
        &mut self,
        other: MergeGain,
    ) {
        if self.less_than(&other) {
            *self = other;
        }
    }
}

/// `NodeT`: a block.
struct Node {
    index: usize,
    cur_index: usize,
    size: u64,
    count: u64,
    cur_chain: usize,
    forced_succ: Option<usize>,
    forced_pred: Option<usize>,
    out_jumps: Vec<usize>,
    in_jumps: Vec<usize>,
}

/// `JumpT`.
struct Jump {
    source: usize,
    target: usize,
    count: u64,
    conditional: bool,
}

/// `ChainT`.
struct Chain {
    id: usize,
    score: f64,
    count: f64,
    size: u64,
    nodes: Vec<usize>,
    /// The adjacent chains and the edge to each.
    edges: Vec<(usize, usize)>,
}

impl Chain {
    fn density(&self) -> f64 {
        self.count / self.size as f64
    }
}

/// `ChainEdge`: the jumps between two chains.
struct ChainEdge {
    src: usize,
    dst: usize,
    jumps: Vec<usize>,
    forward: MergeGain,
    backward: MergeGain,
    forward_valid: bool,
    backward_valid: bool,
}

impl ChainEdge {
    fn is_self_edge(&self) -> bool {
        self.src == self.dst
    }
}

/// `mergeNodes`: the nodes of X (cut at `offset`) and Y in the order `ty`.
fn merged_nodes(
    x: &[usize],
    y: &[usize],
    offset: usize,
    ty: MergeType,
) -> Vec<usize> {
    let (x1, x2) = x.split_at(offset);
    let parts: [&[usize]; 3] = match ty {
        MergeType::XY => [x1, x2, y],
        MergeType::YX => [y, x1, x2],
        MergeType::X1YX2 => [x1, y, x2],
        MergeType::YX2X1 => [y, x2, x1],
        MergeType::X2X1Y => [x2, x1, y],
    };
    parts.concat()
}

/// `ExtTSPImpl`.
struct Layout {
    nodes: Vec<Node>,
    jumps: Vec<Jump>,
    chains: Vec<Chain>,
    edges: Vec<ChainEdge>,
    succ_nodes: Vec<Vec<usize>>,
    pred_nodes: Vec<Vec<usize>>,
    hot_chains: Vec<usize>,
    /// `NodeT::EstimatedAddr`.
    addr: Vec<u64>,
}

impl Layout {
    /// `initialize`.
    fn new(
        sizes: &[u64],
        counts: &[u64],
        edge_counts: &[EdgeCount],
    ) -> Self {
        let n = sizes.len();
        let mut nodes: Vec<Node> = (0..n)
            .map(|index| {
                let mut count = counts[index];
                // The execution count of the entry node is at least one.
                if index == 0 && count == 0 {
                    count = 1;
                }
                Node {
                    index,
                    cur_index: 0,
                    size: sizes[index].max(1),
                    count,
                    cur_chain: index,
                    forced_succ: None,
                    forced_pred: None,
                    out_jumps: Vec::new(),
                    in_jumps: Vec::new(),
                }
            })
            .collect();
        let mut succ_nodes = vec![Vec::new(); n];
        let mut pred_nodes = vec![Vec::new(); n];
        let mut out_degree = vec![0u64; n];
        let mut jumps = Vec::new();
        for edge in edge_counts {
            out_degree[edge.src] += 1;
            // Self-edges are ignored.
            if edge.src == edge.dst {
                continue;
            }
            succ_nodes[edge.src].push(edge.dst);
            pred_nodes[edge.dst].push(edge.src);
            if edge.count > 0 {
                let id = jumps.len();
                jumps.push(Jump { source: edge.src, target: edge.dst, count: edge.count, conditional: false });
                nodes[edge.dst].in_jumps.push(id);
                nodes[edge.src].out_jumps.push(id);
                nodes[edge.src].count = nodes[edge.src].count.max(edge.count);
                nodes[edge.dst].count = nodes[edge.dst].count.max(edge.count);
            }
        }
        for jump in &mut jumps {
            jump.conditional = out_degree[jump.source] > 1;
        }
        let mut chains = Vec::with_capacity(n);
        let mut hot_chains = Vec::new();
        for node in &nodes {
            chains.push(Chain {
                id: node.index,
                score: 0.0,
                count: node.count as f64,
                size: node.size,
                nodes: vec![node.index],
                edges: Vec::new(),
            });
            if node.count > 0 {
                hot_chains.push(node.index);
            }
        }
        let mut layout = Layout {
            nodes,
            jumps,
            chains,
            edges: Vec::new(),
            succ_nodes,
            pred_nodes,
            hot_chains,
            addr: vec![0; n],
        };
        // Chain edges.
        for pred in 0..n {
            for jump in layout.nodes[pred].out_jumps.clone() {
                let succ = layout.jumps[jump].target;
                let (from, to) = (layout.nodes[pred].cur_chain, layout.nodes[succ].cur_chain);
                if let Some(edge) = layout.edge_between(from, to) {
                    layout.edges[edge].jumps.push(jump);
                    continue;
                }
                let edge = layout.edges.len();
                layout.edges.push(ChainEdge {
                    src: from,
                    dst: to,
                    jumps: vec![jump],
                    forward: MergeGain::none(),
                    backward: MergeGain::none(),
                    forward_valid: false,
                    backward_valid: false,
                });
                layout.chains[from].edges.push((to, edge));
                layout.chains[to].edges.push((from, edge));
            }
        }
        layout
    }

    /// `ChainT::getEdge`.
    fn edge_between(
        &self,
        chain: usize,
        other: usize,
    ) -> Option<usize> {
        self.chains[chain].edges.iter().find(|(to, _)| *to == other).map(|(_, edge)| *edge)
    }

    fn is_entry(
        &self,
        chain: usize,
    ) -> bool {
        self.chains[chain].nodes.first() == Some(&0)
    }

    /// `run`.
    fn run(mut self) -> Vec<usize> {
        self.merge_forced_pairs();
        self.merge_chain_pairs();
        self.merge_cold_chains();
        self.concat_chains()
    }

    /// `mergeForcedPairs`.
    fn merge_forced_pairs(&mut self) {
        let n = self.nodes.len();
        for node in 0..n {
            if self.succ_nodes[node].len() == 1
                && self.pred_nodes[self.succ_nodes[node][0]].len() == 1
                && self.succ_nodes[node][0] != 0
            {
                let succ = self.succ_nodes[node][0];
                self.nodes[node].forced_succ = Some(succ);
                self.nodes[succ].forced_pred = Some(node);
            }
        }
        // Cycles of forced dependencies are broken at the node with the
        // smallest index.
        for node in 0..n {
            let (Some(_), Some(pred)) = (self.nodes[node].forced_succ, self.nodes[node].forced_pred) else {
                continue;
            };
            let mut succ = self.nodes[node].forced_succ;
            while let Some(one) = succ {
                if one == node {
                    break;
                }
                succ = self.nodes[one].forced_succ;
            }
            if succ.is_none() {
                continue;
            }
            self.nodes[pred].forced_succ = None;
            self.nodes[node].forced_pred = None;
        }
        // Merge nodes with their fall-through successors.
        for node in 0..n {
            if self.nodes[node].forced_pred.is_none() && self.nodes[node].forced_succ.is_some() {
                let mut current = node;
                while let Some(next) = self.nodes[current].forced_succ {
                    let into = self.nodes[node].cur_chain;
                    let from = self.nodes[next].cur_chain;
                    self.merge_chains(into, from, 0, MergeType::XY);
                    current = next;
                }
            }
        }
    }

    /// `mergeChainPairs`.
    fn merge_chain_pairs(&mut self) {
        while self.hot_chains.len() > 1 {
            let mut best: Option<(usize, usize)> = None;
            let mut best_gain = MergeGain::none();
            for pred in self.hot_chains.clone() {
                for (succ, edge) in self.chains[pred].edges.clone() {
                    if self.edges[edge].is_self_edge() {
                        continue;
                    }
                    if self.chains[pred].nodes.len() + self.chains[succ].nodes.len() >= MAX_CHAIN_SIZE {
                        continue;
                    }
                    // Chains of very different density are not merged.
                    let (a, b) = (self.chains[pred].density(), self.chains[succ].density());
                    let (min, max) = if b < a { (b, a) } else { (a, b) };
                    if max / min > MAX_MERGE_DENSITY_RATIO {
                        continue;
                    }
                    let gain = self.best_merge_gain(pred, succ, edge);
                    if gain.score <= EPS {
                        continue;
                    }
                    let tie = (gain.score - best_gain.score).abs() < EPS
                        && best.is_some_and(|(best_pred, best_succ)| {
                            (self.chains[pred].id, self.chains[succ].id)
                                < (self.chains[best_pred].id, self.chains[best_succ].id)
                        });
                    if best_gain.less_than(&gain) || tie {
                        best_gain = gain;
                        best = Some((pred, succ));
                    }
                }
            }
            if best_gain.score <= EPS {
                break;
            }
            let Some((pred, succ)) = best else { break };
            self.merge_chains(pred, succ, best_gain.offset, best_gain.ty);
        }
    }

    /// `mergeColdChains`: what remains keeps its original order.
    fn merge_cold_chains(&mut self) {
        for src in 0..self.nodes.len() {
            let succs = self.succ_nodes[src].clone();
            for dst in succs.into_iter().rev() {
                let (src_chain, dst_chain) = (self.nodes[src].cur_chain, self.nodes[dst].cur_chain);
                if src_chain != dst_chain
                    && !self.is_entry(dst_chain)
                    && self.chains[src_chain].nodes.last() == Some(&src)
                    && self.chains[dst_chain].nodes.first() == Some(&dst)
                    && self.is_cold(src_chain) == self.is_cold(dst_chain)
                {
                    self.merge_chains(src_chain, dst_chain, 0, MergeType::XY);
                }
            }
        }
    }

    /// `ChainT::isCold`.
    fn is_cold(
        &self,
        chain: usize,
    ) -> bool {
        self.chains[chain].nodes.iter().all(|&node| self.nodes[node].count == 0)
    }

    /// `extTSPScore` of a node order and the jumps among them.
    fn score_of(
        &mut self,
        order: &[usize],
        jumps: &[usize],
    ) -> f64 {
        let mut at = 0;
        for &node in order {
            self.addr[node] = at;
            at += self.nodes[node].size;
        }
        jumps
            .iter()
            .map(|&jump| {
                let jump = &self.jumps[jump];
                let (source, target) = (&self.nodes[jump.source], &self.nodes[jump.target]);
                score_of_jump(self.addr[jump.source], source.size, self.addr[jump.target], jump.count, jump.conditional)
            })
            .sum()
    }

    /// `getBestMergeGain`.
    fn best_merge_gain(
        &mut self,
        pred: usize,
        succ: usize,
        edge: usize,
    ) -> MergeGain {
        let forward = self.edges[edge].src == pred;
        if forward && self.edges[edge].forward_valid {
            return self.edges[edge].forward;
        }
        if !forward && self.edges[edge].backward_valid {
            return self.edges[edge].backward;
        }
        // The jumps between the two chains and those inside the first.
        let mut jumps = self.edges[edge].jumps.clone();
        if let Some(inside) = self.edge_between(pred, pred) {
            jumps.extend(self.edges[inside].jumps.iter().copied());
        }
        let mut gain = MergeGain::none();
        // Concatenation without splitting.
        gain.update_if_less_than(self.merge_gain(pred, succ, &jumps, 0, MergeType::XY));
        // Attach (a part of) the first chain before the first node of the second.
        let first = self.chains[succ].nodes[0];
        for jump in self.nodes[first].in_jumps.clone() {
            let source = self.jumps[jump].source;
            if self.nodes[source].cur_chain != pred {
                continue;
            }
            let offset = self.nodes[source].cur_index + 1;
            self.try_merging(pred, succ, &jumps, offset, &[MergeType::X1YX2, MergeType::X2X1Y], &mut gain);
        }
        // Attach (a part of) the first chain after the last node of the second.
        let last = *self.chains[succ].nodes.last().expect("a chain has a node");
        for jump in self.nodes[last].out_jumps.clone() {
            let target = self.jumps[jump].target;
            if self.nodes[target].cur_chain != pred {
                continue;
            }
            let offset = self.nodes[target].cur_index;
            self.try_merging(pred, succ, &jumps, offset, &[MergeType::X1YX2, MergeType::YX2X1], &mut gain);
        }
        // Break the first chain in every way.
        if self.chains[pred].nodes.len() <= CHAIN_SPLIT_THRESHOLD {
            for offset in 1..self.chains[pred].nodes.len() {
                // Not along a fall-through jump.
                let (before, after) = (self.chains[pred].nodes[offset - 1], self.chains[pred].nodes[offset]);
                if self.nodes[before].out_jumps.iter().any(|&jump| self.jumps[jump].target == after) {
                    continue;
                }
                // X2_Y_X1 almost never helps, so it is not tried.
                self.try_merging(
                    pred,
                    succ,
                    &jumps,
                    offset,
                    &[MergeType::X1YX2, MergeType::YX2X1, MergeType::X2X1Y],
                    &mut gain,
                );
            }
        }
        if forward {
            self.edges[edge].forward = gain;
            self.edges[edge].forward_valid = true;
        } else {
            self.edges[edge].backward = gain;
            self.edges[edge].backward_valid = true;
        }
        gain
    }

    /// `tryChainMerging`.
    fn try_merging(
        &mut self,
        pred: usize,
        succ: usize,
        jumps: &[usize],
        offset: usize,
        types: &[MergeType],
        gain: &mut MergeGain,
    ) {
        // Concatenation without splitting is tried on its own.
        if offset == 0 || offset == self.chains[pred].nodes.len() {
            return;
        }
        // A merge that breaks a forced successor is skipped.
        let node = self.chains[pred].nodes[offset - 1];
        if self.nodes[node].forced_succ.is_some() {
            return;
        }
        for &ty in types {
            let candidate = self.merge_gain(pred, succ, jumps, offset, ty);
            gain.update_if_less_than(candidate);
        }
    }

    /// `computeMergeGain`.
    fn merge_gain(
        &mut self,
        pred: usize,
        succ: usize,
        jumps: &[usize],
        offset: usize,
        ty: MergeType,
    ) -> MergeGain {
        let merged = merged_nodes(&self.chains[pred].nodes, &self.chains[succ].nodes, offset, ty);
        // The merge must keep the original entry point.
        if (self.is_entry(pred) || self.is_entry(succ)) && merged[0] != 0 {
            return MergeGain::none();
        }
        let new_score = self.score_of(&merged, jumps);
        MergeGain { score: new_score - self.chains[pred].score, offset, ty }
    }

    /// `mergeChains`: `from` into `into`.
    fn merge_chains(
        &mut self,
        into: usize,
        from: usize,
        offset: usize,
        ty: MergeType,
    ) {
        debug_assert_ne!(into, from, "a chain cannot be merged with itself");
        let merged = merged_nodes(&self.chains[into].nodes, &self.chains[from].nodes, offset, ty);
        // `ChainT::merge`.
        let (count, size) = (self.chains[from].count, self.chains[from].size);
        self.chains[into].count += count;
        self.chains[into].size += size;
        self.chains[into].id = merged[0];
        for (at, &node) in merged.iter().enumerate() {
            self.nodes[node].cur_chain = into;
            self.nodes[node].cur_index = at;
        }
        self.chains[into].nodes = merged;
        // `ChainT::mergeEdges`.
        for (dst_chain, dst_edge) in self.chains[from].edges.clone() {
            let target = if dst_chain == from { into } else { dst_chain };
            match self.edge_between(into, target) {
                None => {
                    let edge = &mut self.edges[dst_edge];
                    if edge.src == from {
                        edge.src = into;
                    }
                    if edge.dst == from {
                        edge.dst = into;
                    }
                    self.chains[into].edges.push((target, dst_edge));
                    if dst_chain != into && dst_chain != from {
                        self.chains[dst_chain].edges.push((into, dst_edge));
                    }
                }
                Some(current) => {
                    let moved = std::mem::take(&mut self.edges[dst_edge].jumps);
                    self.edges[current].jumps.extend(moved);
                }
            }
            // Clean up the leftover edge.
            if dst_chain != from {
                let list = &mut self.chains[dst_chain].edges;
                if let Some(at) = list.iter().position(|(to, _)| *to == from) {
                    list.remove(at);
                }
            }
        }
        self.chains[from].nodes.clear();
        self.chains[from].edges.clear();
        // The cached score of the new chain.
        if let Some(inside) = self.edge_between(into, into) {
            let order = self.chains[into].nodes.clone();
            let jumps = self.edges[inside].jumps.clone();
            self.chains[into].score = self.score_of(&order, &jumps);
        }
        self.hot_chains.retain(|&chain| chain != from);
        for (_, edge) in self.chains[into].edges.clone() {
            self.edges[edge].forward_valid = false;
            self.edges[edge].backward_valid = false;
        }
    }

    /// `concatChains`: the entry's chain first, then by density.
    fn concat_chains(&self) -> Vec<usize> {
        let mut sorted: Vec<usize> = (0..self.chains.len()).filter(|&one| !self.chains[one].nodes.is_empty()).collect();
        sorted.sort_by(|&left, &right| {
            let (l, r) = (&self.chains[left], &self.chains[right]);
            match (self.is_entry(left), self.is_entry(right)) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            (-l.density()).total_cmp(&-r.density()).then(l.id.cmp(&r.id))
        });
        sorted.into_iter().flat_map(|one| self.chains[one].nodes.iter().copied()).collect()
    }
}

/// `codelayout::computeExtTspLayout`: an order of the blocks that starts at
/// block 0. `sizes` and `counts` have one entry per block.
pub(super) fn compute_layout(
    sizes: &[u64],
    counts: &[u64],
    edges: &[EdgeCount],
) -> Vec<usize> {
    debug_assert_eq!(sizes.len(), counts.len());
    debug_assert!(sizes.len() > 2, "a layout needs at least three blocks");
    let order = Layout::new(sizes, counts, edges).run();
    debug_assert_eq!(order.first(), Some(&0), "the entry point is preserved");
    debug_assert_eq!(order.len(), sizes.len());
    order
}

/// `MachineBlockPlacement::applyExtTsp` with `OptForSize`: the blocks' order
/// by Ext-TSP, every block four bytes an instruction and every jump counted
/// alike (110 for a block's only successor, 100 for each of several).
/// `None` where the pin does not run it: fewer than three blocks, or the
/// entry not first. The original order stays where it scores higher.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBlockPlacement.cpp#L3680>
pub(super) fn apply_for_size(
    blocks: &[crate::model::lir::Block],
    index: &crate::support::hash::IndexMap<i64, usize>,
    entry: usize,
) -> Option<Vec<usize>> {
    if blocks.len() < 3 || entry != 0 {
        return None;
    }
    let sizes: Vec<u64> = blocks.iter().map(|block| 4 * block.insns.iter().filter(|insn| !insn.is_meta()).count() as u64).collect();
    let counts = vec![1; blocks.len()];
    let mut edges = Vec::new();
    for (src, block) in blocks.iter().enumerate() {
        let mut seen: Vec<usize> = Vec::new();
        for to in block.succ.iter() {
            if let Some(&dst) = index.get(to) {
                if !seen.contains(&dst) {
                    seen.push(dst);
                }
            }
        }
        let count = if seen.len() == 1 { 110 } else { 100 };
        edges.extend(seen.into_iter().map(|dst| EdgeCount { src, dst, count }));
    }
    let order = compute_layout(&sizes, &counts, &edges);
    let original: Vec<usize> = (0..blocks.len()).collect();
    if score(&original, &sizes, &edges) > score(&order, &sizes, &edges) {
        return Some(original);
    }
    Some(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(src: usize, dst: usize) -> EdgeCount {
        EdgeCount { src, dst, count: 100 }
    }

    /// A fall-through scores its full weight, a short forward jump a tenth.
    #[test]
    fn test_score_prefers_fallthrough_over_forward_jump() {
        let sizes = [4, 4, 4];
        let edges = [edge(0, 1)];
        let fall = score(&[0, 1, 2], &sizes, &edges);
        let jump = score(&[0, 2, 1], &sizes, &edges);
        assert!(fall > 10.0 * jump * 0.99, "fall-through {fall}, jump {jump}");
    }

    /// The entry stays first and a chain 0 -> 2 -> 1 (both conditional edges
    /// of 0 weighted alike, 2 falling to 1) is not split by block 3.
    #[test]
    fn test_layout_keeps_entry_first_and_chains_the_path() {
        let sizes = [4, 4, 4, 4];
        let counts = [1; 4];
        let edges = [edge(0, 2), edge(2, 1), edge(1, 3), edge(0, 3)];
        let order = compute_layout(&sizes, &counts, &edges);
        assert_eq!(order[0], 0);
        let pos = |block| order.iter().position(|&one| one == block).unwrap();
        assert_eq!(pos(1), pos(2) + 1, "2 falls into 1: {order:?}");
    }

    /// A layout is a permutation and never scores below the original order
    /// once `apply_for_size` falls back.
    #[test]
    fn test_layout_is_a_permutation() {
        let n = 12;
        let sizes = vec![8; n];
        let counts = vec![1; n];
        let mut edges = Vec::new();
        for block in 0..n - 1 {
            edges.push(edge(block, block + 1));
            if block + 3 < n {
                edges.push(edge(block, block + 3));
            }
        }
        let mut order = compute_layout(&sizes, &counts, &edges);
        order.sort_unstable();
        assert_eq!(order, (0..n).collect::<Vec<_>>());
    }
}
