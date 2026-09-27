//! Where a split value should be in a register and where on the stack.
//!
//! LLVM's `EdgeBundles` and `SpillPlacement`. Each block has an entry and an
//! exit node; an edge joins a block's exit to its successor's entry, and the
//! joined nodes form a bundle, the unit that is either all register or all
//! stack. Blocks bias the bundles at their borders; a block the value only
//! passes through links its two bundles. A Hopfield network settles each
//! bundle's sign.

use std::collections::BTreeSet;

use crate::analysis::intervals as ranges;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// Every block's entry and exit bundle.
pub struct Bundles {
    /// Block address -> (entry bundle, exit bundle).
    pub of: IndexMap<i64, (usize, usize)>,
    /// Bundle -> the blocks with an entry or exit in it.
    pub blocks: Vec<BTreeSet<i64>>,
}

/// `EdgeBundles`: a block's exit shares a bundle with each successor's entry.
pub fn bundles(body: &LirBody) -> Bundles {
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut parent: Vec<usize> = (0..2 * body.blocks.len()).collect();
    fn find(parent: &mut [usize], mut one: usize) -> usize {
        while parent[one] != one {
            parent[one] = parent[parent[one]];
            one = parent[one];
        }
        one
    }
    for (at, block) in body.blocks.iter().enumerate() {
        for next in &block.succ {
            let Some(to) = position.get(next) else { continue };
            let (exit, entry) = (find(&mut parent, 2 * at + 1), find(&mut parent, 2 * to));
            parent[exit] = entry;
        }
    }
    let mut number: IndexMap<usize, usize> = IndexMap::default();
    let mut of = IndexMap::default();
    let mut blocks: Vec<BTreeSet<i64>> = Vec::new();
    for (at, block) in body.blocks.iter().enumerate() {
        let mut named = |node: usize, parent: &mut Vec<usize>| {
            let root = find(parent, node);
            let count = number.len();
            let bundle = *number.entry(root).or_insert(count);
            if bundle == blocks.len() {
                blocks.push(BTreeSet::new());
            }
            blocks[bundle].insert(block.at);
            bundle
        };
        let entry = named(2 * at, &mut parent);
        let exit = named(2 * at + 1, &mut parent);
        of.insert(block.at, (entry, exit));
    }
    Bundles { of, blocks }
}

/// What a block wants at one of its borders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Border {
    DontCare,
    PrefReg,
    PrefSpill,
    MustSpill,
}

/// One block's wishes at its entry and exit.
#[derive(Clone, Copy, Debug)]
pub struct Constraint {
    pub block: i64,
    pub entry: Border,
    pub exit: Border,
    /// Times the block's frequency each border's bias counts: 1 in LLVM.
    pub weight: f64,
}

#[derive(Clone, Default)]
struct Node {
    /// Frequency of the blocks that want the stack; infinite for must.
    against: f64,
    /// Frequency of the blocks that want a register.
    toward: f64,
    /// -1, 0 or 1: positive holds the value in a register.
    value: i8,
    /// (weight, bundle) for each block linking this bundle to another.
    links: Vec<(f64, usize)>,
    /// Sum of the link weights, plus the threshold.
    linked: f64,
}

impl Node {
    fn prefers_register(&self) -> bool {
        self.value > 0
    }

    fn must_spill(&self) -> bool {
        self.against >= self.toward + self.linked
    }

    fn clear(&mut self, threshold: f64) {
        *self = Node { linked: threshold, ..Node::default() };
    }

    fn link(&mut self, to: usize, weight: f64) {
        self.linked += weight;
        match self.links.iter_mut().find(|(_, other)| *other == to) {
            Some(found) => found.0 += weight,
            None => self.links.push((weight, to)),
        }
    }

    fn bias(&mut self, frequency: f64, direction: Border) {
        match direction {
            Border::PrefReg => self.toward += frequency,
            Border::PrefSpill => self.against += frequency,
            Border::MustSpill => self.against = f64::INFINITY,
            Border::DontCare => {}
        }
    }
}

/// One placement query over a body's bundles.
pub struct Placement<'a> {
    pub bundles: &'a Bundles,
    pub frequency: IndexMap<i64, f64>,
    nodes: Vec<Node>,
    active: BTreeSet<usize>,
    todo: BTreeSet<usize>,
    /// Bundles that turned positive since the last scan or iteration.
    pub recent: Vec<usize>,
    threshold: f64,
}

impl<'a> Placement<'a> {
    /// A block runs `level(depth)` times per entry to the body. The threshold
    /// is LLVM's dead zone, 2 when the entry frequency is 2^14.
    pub fn new(body: &LirBody, bundles: &'a Bundles) -> Self {
        let frequency = ranges::depths(body).into_iter().map(|(at, depth)| (at, ranges::level(depth))).collect();
        Self {
            bundles,
            frequency,
            nodes: vec![Node::default(); bundles.blocks.len()],
            active: BTreeSet::new(),
            todo: BTreeSet::new(),
            recent: Vec::new(),
            threshold: 2.0 / 16384.0,
        }
    }

    pub fn prepare(&mut self) {
        self.recent.clear();
        self.todo.clear();
        self.active.clear();
    }

    fn activate(&mut self, bundle: usize) {
        self.todo.insert(bundle);
        if !self.active.insert(bundle) {
            return;
        }
        self.nodes[bundle].clear(self.threshold);
        // A bundle joining many blocks needs many of them to want a register.
        if self.bundles.blocks[bundle].len() > 100 {
            self.nodes[bundle].against = 1.0 / 16.0;
        }
    }

    pub fn add_constraints(&mut self, constraints: &[Constraint]) {
        for one in constraints {
            let frequency = self.frequency[&one.block] * one.weight;
            let (entry, exit) = self.bundles.of[&one.block];
            if one.entry != Border::DontCare {
                self.activate(entry);
                self.nodes[entry].bias(frequency, one.entry);
            }
            if one.exit != Border::DontCare {
                self.activate(exit);
                self.nodes[exit].bias(frequency, one.exit);
            }
        }
    }

    /// Blocks that would rather not hold the value at all, twice as much when strong.
    pub fn add_pref_spill(&mut self, blocks: &[i64], strong: bool) {
        for block in blocks {
            let frequency = self.frequency[block] * if strong { 2.0 } else { 1.0 };
            let (entry, exit) = self.bundles.of[block];
            for bundle in [entry, exit] {
                self.activate(bundle);
                self.nodes[bundle].bias(frequency, Border::PrefSpill);
            }
        }
    }

    /// Blocks the value passes through untouched and uncontested.
    pub fn add_links(&mut self, blocks: &[i64]) {
        for block in blocks {
            let (entry, exit) = self.bundles.of[block];
            if entry == exit {
                continue;
            }
            self.activate(entry);
            self.activate(exit);
            let frequency = self.frequency[block];
            self.nodes[entry].link(exit, frequency);
            self.nodes[exit].link(entry, frequency);
        }
    }

    fn update(&mut self, bundle: usize) -> bool {
        let node = &self.nodes[bundle];
        let (mut against, mut toward) = (node.against, node.toward);
        for (weight, other) in &node.links {
            match self.nodes[*other].value {
                -1 => against += weight,
                1 => toward += weight,
                _ => {}
            }
        }
        let before = node.prefers_register();
        let value = if against >= toward + self.threshold {
            -1
        } else if toward >= against + self.threshold {
            1
        } else {
            0
        };
        self.nodes[bundle].value = value;
        if before == self.nodes[bundle].prefers_register() {
            return false;
        }
        let dissenting: Vec<usize> =
            self.nodes[bundle].links.iter().map(|(_, other)| *other).filter(|other| self.nodes[*other].value != value).collect();
        self.todo.extend(dissenting);
        true
    }

    /// Whether any bundle wants a register once the constraints are in.
    pub fn scan(&mut self) -> bool {
        self.recent.clear();
        for bundle in self.active.clone() {
            self.update(bundle);
            if self.nodes[bundle].must_spill() {
                continue;
            }
            if self.nodes[bundle].prefers_register() {
                self.recent.push(bundle);
            }
        }
        !self.recent.is_empty()
    }

    pub fn iterate(&mut self) {
        self.recent.clear();
        let mut limit = self.nodes.len() * 10;
        while limit > 0 {
            limit -= 1;
            let Some(bundle) = self.todo.pop_last() else { break };
            if self.update(bundle) && self.nodes[bundle].prefers_register() {
                self.recent.push(bundle);
            }
        }
    }

    /// The bundles that settled in a register.
    pub fn finish(&self) -> BTreeSet<usize> {
        self.active.iter().copied().filter(|bundle| self.nodes[*bundle].prefers_register()).collect()
    }
}
