//! `GenericCycleInfo`, a translation of
//! llvm/include/llvm/ADT/GenericCycleInfo.h and GenericCycleImpl.h at
//! d1106deb71cc: the cycles of a graph as a tree, found by one depth-first
//! search (Wei et al., SAS 2007), each cycle's blocks contiguous in one layout,
//! outer cycles first. `BlockFrequencyInfoImpl` reads it for its loops.
//!
//! Generic over [`FlowGraph`], so a function of MIR and a body of LIR both
//! have one. Checked against LLVM by `blockfrequency`'s golden test.
//!
//! DEVIATE 1: no `splitCriticalEdge`/`addBlockToCycle`, `getExitBlocks`,
//! `getCyclePreheader` or the verifier: no reader updates a cycle info yet.

use crate::blocklist::BlockList;
use crate::domtree::DomGraph;
use crate::module::{BlockId, Function};

/// A [`DomGraph`] whose successors keep repeats, in the terminator's order:
/// `children<BlockT *>`, which `GenericCycleInfo` and
/// `BlockFrequencyInfoImpl` walk and an edge's index counts in.
pub trait FlowGraph: DomGraph {
    fn children(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.successors(block)
    }

    /// `BlockT::getIrrLoopHeaderWeight`.
    fn irr_loop_header_weight(
        &self,
        _block: BlockId,
    ) -> Option<u64> {
        None
    }
}

impl FlowGraph for Function {
    fn children(
        &self,
        block: BlockId,
    ) -> BlockList {
        crate::loopinfo::children(self, block).collect()
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleInfo.h#L54>
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CycleRef(Option<u32>);

impl CycleRef {
    pub fn is_valid(self) -> bool {
        self.0.is_some()
    }

    fn index(self) -> usize {
        self.0.expect("a cycle") as usize
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Cycle {
    parent: CycleRef,
    idx_begin: u32,
    idx_end: u32,
    depth: u32,
    num_descendants: u32,
    entry_begin: u32,
    entry_size: u32,
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleInfo.h#L79>
#[derive(Clone, Debug, Default)]
pub struct CycleInfo {
    block_map: Vec<CycleRef>,
    block_layout: Vec<BlockId>,
    cycles: Vec<Cycle>,
}

impl CycleInfo {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleImpl.h#L562>
    pub fn compute(graph: &impl FlowGraph) -> Self {
        let mut info = Self { block_map: vec![CycleRef::default(); graph.max_number()], ..Self::default() };
        if let Some(entry) = graph.entry() {
            Compute::new(&mut info, graph.max_number()).run(graph, entry);
        }
        info
    }

    fn deref(
        &self,
        c: CycleRef,
    ) -> &Cycle {
        &self.cycles[c.index()]
    }

    pub fn cycles(&self) -> impl Iterator<Item = CycleRef> {
        (0..self.cycles.len() as u32).map(|one| CycleRef(Some(one)))
    }

    pub fn get_cycle(
        &self,
        block: BlockId,
    ) -> CycleRef {
        self.block_map.get(block.0 as usize).copied().unwrap_or_default()
    }

    pub fn get_header(
        &self,
        c: CycleRef,
    ) -> BlockId {
        self.block_layout[self.deref(c).entry_begin as usize]
    }

    pub fn is_reducible(
        &self,
        c: CycleRef,
    ) -> bool {
        self.deref(c).entry_size == 1
    }

    pub fn get_parent_cycle(
        &self,
        c: CycleRef,
    ) -> CycleRef {
        self.deref(c).parent
    }

    pub fn get_depth(
        &self,
        c: CycleRef,
    ) -> u32 {
        self.deref(c).depth
    }

    pub fn get_entries(
        &self,
        c: CycleRef,
    ) -> &[BlockId] {
        let cycle = self.deref(c);
        &self.block_layout[cycle.entry_begin as usize..(cycle.entry_begin + cycle.entry_size) as usize]
    }

    pub fn is_entry(
        &self,
        c: CycleRef,
        block: BlockId,
    ) -> bool {
        self.get_entries(c).contains(&block)
    }

    pub fn get_blocks(
        &self,
        c: CycleRef,
    ) -> &[BlockId] {
        let cycle = self.deref(c);
        &self.block_layout[cycle.idx_begin as usize..cycle.idx_end as usize]
    }

    /// `contains(Outer, Inner)`.
    pub fn contains_cycle(
        &self,
        outer: CycleRef,
        inner: CycleRef,
    ) -> bool {
        let (o, i) = (self.deref(outer), self.deref(inner));
        o.idx_begin <= i.idx_begin && i.idx_end <= o.idx_end
    }

    pub fn contains(
        &self,
        c: CycleRef,
        block: BlockId,
    ) -> bool {
        let inner = self.get_cycle(block);
        inner.is_valid() && self.contains_cycle(c, inner)
    }

    /// `children(C)`: the cycles directly inside `c`, through
    /// `const_child_iterator`.
    pub fn children(
        &self,
        c: CycleRef,
    ) -> impl Iterator<Item = CycleRef> + '_ {
        let first = c.index() as u32 + 1;
        self.siblings(first, first + self.deref(c).num_descendants)
    }

    pub fn toplevel_cycles(&self) -> impl Iterator<Item = CycleRef> + '_ {
        self.siblings(0, self.cycles.len() as u32)
    }

    fn siblings(
        &self,
        mut index: u32,
        end: u32,
    ) -> impl Iterator<Item = CycleRef> + '_ {
        std::iter::from_fn(move || {
            (index < end).then(|| {
                let one = CycleRef(Some(index));
                index += 1 + self.cycles[index as usize].num_descendants;
                one
            })
        })
    }

    pub fn get_cycle_depth(
        &self,
        block: BlockId,
    ) -> u32 {
        let c = self.get_cycle(block);
        if c.is_valid() { self.get_depth(c) } else { 0 }
    }
}

const NO_CYCLE: u32 = u32::MAX;
const NO_BLOCK: u32 = u32::MAX;

/// `GenericCycleInfoCompute::BlockInfo`; `Pos` and `LoopIdx` share storage
/// in LLVM and are two fields here (`Pos` is dead once the search is done).
#[derive(Clone, Copy, Default)]
struct BlockInfo {
    block: Option<BlockId>,
    header: bool,
    pos: u32,
    loop_idx: u32,
    loop_header: u32,
}

#[derive(Clone, Copy)]
struct CycleBuild {
    child_head: u32,
    next_sibling: u32,
    own_count: u32,
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleImpl.h#L204>
struct Compute<'a> {
    info: &'a mut CycleInfo,
    block_infos: Vec<BlockInfo>,
    preorder: Vec<u32>,
    num_headers: u32,
    reentries: Vec<(u32, u32)>,
}

impl<'a> Compute<'a> {
    fn new(
        info: &'a mut CycleInfo,
        max_number: usize,
    ) -> Self {
        Self {
            info,
            block_infos: vec![BlockInfo::default(); max_number],
            preorder: Vec::new(),
            num_headers: 0,
            reentries: Vec::new(),
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleImpl.h#L272>
    fn tag_loop_header(
        &mut self,
        mut b: u32,
        mut h: u32,
    ) {
        assert!(h != NO_BLOCK);
        while b != h {
            let ih = self.block_infos[b as usize].loop_header;
            if ih == NO_BLOCK {
                self.block_infos[b as usize].loop_header = h;
                return;
            }
            if self.block_infos[ih as usize].pos >= self.block_infos[h as usize].pos {
                b = ih;
            } else {
                self.block_infos[b as usize].loop_header = h;
                b = h;
                h = ih;
            }
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleImpl.h#L350>
    fn flatten(
        &mut self,
        build: &[CycleBuild],
        top_head: u32,
    ) {
        let cycles = &mut self.info.cycles;
        *cycles = vec![Cycle::default(); build.len()];
        let mut flat_idx = vec![0u32; build.len()];
        // (flat, the next child to enter or NO_CYCLE).
        let mut stack: Vec<(u32, u32)> = Vec::new();
        let (mut cursor, mut next_id) = (0u32, 0u32);
        let mut tlc = top_head;
        while tlc != NO_CYCLE {
            let mut enter = Some((tlc, CycleRef::default()));
            loop {
                if let Some((c, parent)) = enter.take() {
                    let id = next_id;
                    next_id += 1;
                    flat_idx[c as usize] = id;
                    let depth = parent.0.map_or(1, |p| cycles[p as usize].depth + 1);
                    let flat = &mut cycles[id as usize];
                    flat.parent = parent;
                    flat.depth = depth;
                    flat.entry_begin = cursor;
                    flat.entry_size = 1;
                    cursor += build[c as usize].own_count;
                    flat.idx_begin = cursor;
                    stack.push((id, build[c as usize].child_head));
                }
                let Some(frame) = stack.last_mut() else { break };
                if frame.1 != NO_CYCLE {
                    let c = frame.1;
                    frame.1 = build[c as usize].next_sibling;
                    enter = Some((c, CycleRef(Some(frame.0))));
                } else {
                    let flat = &mut cycles[frame.0 as usize];
                    flat.idx_end = cursor;
                    flat.num_descendants = next_id - frame.0 - 1;
                    stack.pop();
                }
            }
            tlc = build[tlc as usize].next_sibling;
        }
        self.info.block_layout = vec![BlockId(0); cursor as usize];
        for &n in self.preorder.iter().rev() {
            let bi = self.block_infos[n as usize];
            if bi.loop_idx == NO_CYCLE {
                continue;
            }
            let flat = flat_idx[bi.loop_idx as usize];
            self.info.block_map[n as usize] = CycleRef(Some(flat));
            let cycle = &mut self.info.cycles[flat as usize];
            cycle.idx_begin -= 1;
            self.info.block_layout[cycle.idx_begin as usize] = bi.block.expect("a visited block");
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleImpl.h#L411>
    fn run(
        mut self,
        graph: &impl FlowGraph,
        entry: BlockId,
    ) {
        self.dfs(graph, entry);
        if self.num_headers == 0 {
            return;
        }
        let mut build: Vec<CycleBuild> = Vec::with_capacity(self.num_headers as usize);
        let mut top_head = NO_CYCLE;
        for index in 0..self.preorder.len() {
            let n = self.preorder[index] as usize;
            let bi = self.block_infos[n];
            if bi.header {
                let i = build.len() as u32;
                self.block_infos[n].loop_idx = i;
                let head = if bi.loop_header != NO_BLOCK {
                    let parent = self.block_infos[bi.loop_header as usize].loop_idx as usize;
                    &mut build[parent].child_head
                } else {
                    &mut top_head
                };
                let next_sibling = *head;
                *head = i;
                build.push(CycleBuild { child_head: NO_CYCLE, next_sibling, own_count: 1 });
            } else if bi.loop_header != NO_BLOCK {
                let idx = self.block_infos[bi.loop_header as usize].loop_idx;
                self.block_infos[n].loop_idx = idx;
                build[idx as usize].own_count += 1;
            } else {
                self.block_infos[n].loop_idx = NO_CYCLE;
            }
        }
        self.flatten(&build, top_head);
        if self.reentries.is_empty() {
            return;
        }
        let mut rank = vec![0u32; self.block_infos.len()];
        for (r, &n) in self.preorder.iter().enumerate() {
            rank[n as usize] = r as u32;
        }
        for one in &mut self.reentries {
            one.1 = rank[one.1 as usize];
        }
        self.reentries.sort_unstable();
        let mut prev_h = NO_BLOCK;
        for i in 0..self.reentries.len() {
            if i > 0 && self.reentries[i] == self.reentries[i - 1] {
                continue;
            }
            let (h, r) = self.reentries[i];
            let c = self.info.block_map[h as usize].index();
            if h != prev_h {
                let header = self.info.block_layout[self.info.cycles[c].entry_begin as usize];
                self.info.cycles[c].entry_begin = self.info.block_layout.len() as u32;
                self.info.block_layout.push(header);
                prev_h = h;
            }
            let block = self.block_infos[self.preorder[r as usize] as usize].block.expect("a visited block");
            self.info.block_layout.push(block);
            self.info.cycles[c].entry_size = self.info.block_layout.len() as u32 - self.info.cycles[c].entry_begin;
        }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/GenericCycleImpl.h#L481>
    fn dfs(
        &mut self,
        graph: &impl FlowGraph,
        entry: BlockId,
    ) {
        // (block, its successors, how many are left from the back).
        let mut stack: Vec<(u32, BlockList, usize)> = Vec::new();
        let mut counter = 0u32;
        let mut open = |this: &mut Self, stack: &mut Vec<(u32, BlockList, usize)>, block: BlockId| {
            let n = block.0;
            this.preorder.push(n);
            let bi = &mut this.block_infos[n as usize];
            bi.block = Some(block);
            bi.header = false;
            counter += 1;
            bi.pos = counter;
            bi.loop_header = NO_BLOCK;
            let succs = graph.children(block);
            let left = succs.len();
            stack.push((n, succs, left));
        };
        open(self, &mut stack, entry);
        while let Some(top) = stack.last_mut() {
            if top.2 > 0 {
                top.2 -= 1;
                let b0 = top.0;
                let b1p = top.1[top.2];
                let b1 = b1p.0;
                let b1_info = self.block_infos[b1 as usize];
                if b1_info.block.is_none() {
                    open(self, &mut stack, b1p);
                } else if b1_info.pos > 0 {
                    if !b1_info.header {
                        self.block_infos[b1 as usize].header = true;
                        self.num_headers += 1;
                    }
                    self.tag_loop_header(b0, b1);
                } else {
                    let mut h = b1_info.loop_header;
                    while h != NO_BLOCK {
                        if self.block_infos[h as usize].pos > 0 {
                            self.tag_loop_header(b0, h);
                            break;
                        }
                        self.reentries.push((h, b1));
                        h = self.block_infos[h as usize].loop_header;
                    }
                }
            } else {
                let b0 = top.0;
                self.block_infos[b0 as usize].pos = 0;
                stack.pop();
                let header = self.block_infos[b0 as usize].loop_header;
                if let Some(parent) = stack.last()
                    && header != NO_BLOCK
                {
                    self.tag_loop_header(parent.0, header);
                }
            }
        }
    }
}
