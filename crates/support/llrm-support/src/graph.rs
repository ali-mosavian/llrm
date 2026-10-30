//! Graphs of blocks that are `Node`s: who reaches whom and in what order.
//! Dominance of them is `llrm_mir::domtree_address`.

use std::collections::{BTreeMap, BTreeSet};

/// All these walks read of a block: where it is and where it goes.
pub trait Node {
    fn at(&self) -> i64;
    fn succ(&self) -> &[i64];
}

impl<T: Node> Node for &T {
    fn at(&self) -> i64 {
        (*self).at()
    }

    fn succ(&self) -> &[i64] {
        (*self).succ()
    }
}

/// Who can reach each block, inverted from its own successors.
/// Blocks in reverse postorder from `entry`, then those it cannot reach in body
/// order: the order a forward dataflow worklist drains in, predecessors before
/// successors but for back edges.
pub fn reverse_postorder<N: Node>(
    blocks: &[N],
    entry: i64,
) -> Vec<i64> {
    let mut index = crate::hash::HashMap::with_capacity_and_hasher(blocks.len(), Default::default());
    index.extend(blocks.iter().enumerate().map(|(at, block)| (block.at(), at)));
    let mut seen = vec![false; blocks.len()];
    let mut post = Vec::with_capacity(blocks.len());
    let mut stack = Vec::new();
    if let Some(&first) = index.get(&entry) {
        seen[first] = true;
        stack.push((first, 0usize));
    }
    while let Some((block, next)) = stack.last_mut() {
        let block = *block;
        if let Some(successor) = blocks[block].succ().get(*next) {
            *next += 1;
            if let Some(&child) = index.get(successor)
                && !std::mem::replace(&mut seen[child], true)
            {
                stack.push((child, 0));
            }
        } else {
            post.push(blocks[block].at());
            stack.pop();
        }
    }
    post.reverse();
    post.extend(blocks.iter().enumerate().filter(|(at, _)| !seen[*at]).map(|(_, block)| block.at()));
    post
}

/// The strongly connected components of `blocks`, as Tarjan's walk finds them:
/// every component after the components its blocks reach, so the successors'
/// are first. Blocks named only as successors are not in the graph.
pub fn components<N: Node>(blocks: &[N]) -> Vec<Vec<i64>> {
    let index: crate::hash::HashMap<i64, usize> =
        blocks.iter().enumerate().map(|(at, block)| (block.at(), at)).collect();
    let successors: Vec<Vec<usize>> =
        blocks.iter().map(|block| block.succ().iter().filter_map(|to| index.get(to).copied()).collect()).collect();
    let count = successors.len();
    let (mut order, mut low) = (vec![usize::MAX; count], vec![0; count]);
    let (mut on, mut stack, mut next) = (vec![false; count], Vec::new(), 0);
    let mut found: Vec<Vec<i64>> = Vec::new();
    for root in 0..count {
        if order[root] != usize::MAX {
            continue;
        }
        let mut calls = vec![(root, 0_usize)];
        order[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on[root] = true;
        while let Some(&mut (at, ref mut child)) = calls.last_mut() {
            if let Some(&to) = successors[at].get(*child) {
                *child += 1;
                if order[to] == usize::MAX {
                    order[to] = next;
                    low[to] = next;
                    next += 1;
                    stack.push(to);
                    on[to] = true;
                    calls.push((to, 0));
                } else if on[to] {
                    low[at] = low[at].min(order[to]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[at]);
            }
            if low[at] == order[at] {
                let mut members = Vec::new();
                while let Some(one) = stack.pop() {
                    on[one] = false;
                    members.push(blocks[one].at());
                    if one == at {
                        break;
                    }
                }
                found.push(members);
            }
        }
    }
    found
}

/// The blocks that branch to each block, sorted, each list once: three
/// allocations whatever the blocks, where a map of sets made one for every
/// block and a node for every eleven predecessors (`predecessors` was 20 M of
/// a d_faces -O2 compile, 16 M of it in the trees).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Predecessors {
    /// Every block's `at`, ascending.
    ats: Vec<i64>,
    /// `from[start[k]..start[k + 1]]` are the predecessors of `ats[k]`.
    start: Vec<u32>,
    from: Vec<i64>,
}

impl Predecessors {
    fn place(
        &self,
        at: &i64,
    ) -> Option<usize> {
        self.ats.binary_search(at).ok()
    }

    /// The predecessors of block `at`, ascending, or none for an unknown block.
    pub fn get(
        &self,
        at: &i64,
    ) -> Option<&[i64]> {
        let k = self.place(at)?;
        Some(&self.from[self.start[k] as usize..self.start[k + 1] as usize])
    }

    pub fn contains_key(
        &self,
        at: &i64,
    ) -> bool {
        self.place(at).is_some()
    }

    /// Whether the predecessors of `at` are exactly the blocks `expected`
    /// names, in any order.
    pub fn are(
        &self,
        at: &i64,
        expected: &[i64],
    ) -> bool {
        let mut want = expected.to_vec();
        want.sort_unstable();
        want.dedup();
        self.get(at) == Some(&want[..])
    }

    /// How many blocks there are.
    pub fn len(&self) -> usize {
        self.ats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ats.is_empty()
    }

    /// Every block, ascending.
    pub fn keys(&self) -> impl Iterator<Item = &i64> {
        self.ats.iter()
    }

    /// Every block with its predecessors, ascending by block.
    pub fn iter(&self) -> impl Iterator<Item = (&i64, &[i64])> {
        self.ats.iter().enumerate().map(|(k, at)| (at, &self.from[self.start[k] as usize..self.start[k + 1] as usize]))
    }

    /// Every block's predecessors, ascending by block.
    pub fn values(&self) -> impl Iterator<Item = &[i64]> {
        self.iter().map(|(_, from)| from)
    }

    /// As the map of sets it was.
    pub fn named(&self) -> BTreeMap<i64, BTreeSet<i64>> {
        self.iter().map(|(at, from)| (*at, from.iter().copied().collect())).collect()
    }
}

impl std::ops::Index<&i64> for Predecessors {
    type Output = [i64];

    fn index(
        &self,
        at: &i64,
    ) -> &[i64] {
        self.get(at).expect("a block of the graph")
    }
}

impl<'a> IntoIterator for &'a Predecessors {
    type Item = (&'a i64, &'a [i64]);
    type IntoIter = Box<dyn Iterator<Item = (&'a i64, &'a [i64])> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

thread_local! {
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many predecessor graphs this thread has built, for a test that a query
/// about one block (LLVM's `predecessors(BB)`, a walk of the block's users)
/// builds none.
pub fn predecessor_graphs_built() -> usize {
    BUILT.with(std::cell::Cell::get)
}

pub fn predecessors<N: Node>(blocks: &[N]) -> Predecessors {
    BUILT.with(|built| built.set(built.get() + 1));
    let mut ats: Vec<i64> = blocks.iter().map(Node::at).collect();
    ats.sort_unstable();
    ats.dedup();
    let mut edges: Vec<(u32, i64)> = Vec::with_capacity(blocks.iter().map(|block| block.succ().len()).sum());
    for block in blocks {
        for successor in block.succ() {
            if let Ok(to) = ats.binary_search(successor) {
                edges.push((to as u32, block.at()));
            }
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let mut start = vec![0_u32; ats.len() + 1];
    for (to, _) in &edges {
        start[*to as usize + 1] += 1;
    }
    for k in 0..ats.len() {
        start[k + 1] += start[k];
    }
    Predecessors { ats, start, from: edges.into_iter().map(|(_, from)| from).collect() }
}
