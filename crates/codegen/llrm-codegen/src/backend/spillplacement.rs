//! Decides, for each value being split, which edge bundles hold it in a
//! register and which on the stack.
//!
//! # How it works
//! An edge bundle is the unit that is either all register or all stack. Each
//! block has an entry and an exit node; the exit of a block and the entries of
//! its successors are joined into one bundle.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   x["<b>X</b><br/><small>in #124; out</small>"]:::mid
//!   y["<b>Y</b><br/><small>in #124; out</small>"]:::mid
//!   z["<b>Z</b><br/><small>in #124; out</small>"]:::mid
//!   w["<b>W</b>"]:::mid
//!   x -->|"bundle 1"| y
//!   x -->|"bundle 1"| z
//!   y -->|"bundle 2"| w
//!   z -->|"bundle 2"| w
//! ```
//!
//! *Edge bundles join a block's exit to its successors' entries*
//!
//! Blocks bias the bundles at their borders (`Constraint`): `PrefReg` toward a
//! register, `PrefSpill` toward the stack, `MustSpill` where the register is
//! occupied. The bias is weighted by the block's frequency, taken from its loop
//! depth. A block the value only passes through links its entry and exit
//! bundles with its frequency as the weight, so the two settle together.
//!
//! A bundle is a node with a value of -1 (stack), 0 or 1 (register), set by
//! comparing the weight against and toward a register, plus the weight of
//! linked bundles already settled, with a dead zone (`threshold`) around a tie.
//! Changing a node queues the bundles linked to it that disagree. `iterate`
//! runs this to a fixed point, bounded at ten times the number of bundles in
//! updates. A bundle joining more than a hundred blocks needs a stronger pull
//! before it chooses a register. `finish` returns the bundles that settled in a
//! register.
//!
//! The result drives [`crate::backend::splitkit`]: blocks whose borders are in
//! registers form the region a piece holds.
//!
//! # Example
//! None: this module answers a placement query for `splitkit` and prints
//! nothing, and the tests build bodies in code and assert on the bundles. Read
//! `splitkit` for the effect.
//!
//! # Runs
//! Not a pass: [`crate::backend::splitkit`] queries it for each value it
//! splits, and `floatalloc` asks for the bundles of a body, at every -O level.
//!
//! # Provides
//! `edge_bundles`: the bundles of a body, a fact depending only on block labels
//! and successors, so every body a split makes of the original shares them.
//! `Placement`: one placement query over those bundles.
//!
//! # See also
//! LLVM's
//! [`EdgeBundles`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/EdgeBundles.cpp)
//! and
//! [`SpillPlacement`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/SpillPlacement.cpp).

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_mir::dense::IdSet;

use crate::analysis::facts::Fact;
use crate::analysis::graph::Shape;
use crate::analysis::intervals as ranges;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

/// Every block's entry and exit bundle.
#[derive(Debug, PartialEq)]
pub struct Bundles {
    /// Block address -> (entry bundle, exit bundle).
    pub of: IndexMap<i64, (usize, usize)>,
    /// Bundle -> the blocks with an entry or exit in it.
    pub blocks: Vec<BTreeSet<i64>>,
}

/// `EdgeBundles`, the manager's: the bundles depend on the blocks' labels and
/// successors alone, so every body a split makes of this one has them.
pub fn edge_bundles(body: &LirBody) -> Arc<Bundles> {
    body.facts.0.get::<EdgeBundles>(body)
}

pub struct EdgeBundles;

impl Fact for EdgeBundles {
    type Result = Bundles;
    type Inputs = Shape;
    const NAME: &'static str = "edge-bundles";

    fn run(body: &LirBody) -> Bundles {
        bundles(body)
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

/// `EdgeBundles`: a block's exit shares a bundle with each successor's entry.
pub fn bundles(body: &LirBody) -> Bundles {
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut parent: Vec<usize> = (0..2 * body.blocks.len()).collect();
    fn find(
        parent: &mut [usize],
        mut one: usize,
    ) -> usize {
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

    fn clear(
        &mut self,
        threshold: f64,
    ) {
        *self = Node { linked: threshold, ..Node::default() };
    }

    fn link(
        &mut self,
        to: usize,
        weight: f64,
    ) {
        self.linked += weight;
        match self.links.iter_mut().find(|(_, other)| *other == to) {
            Some(found) => found.0 += weight,
            None => self.links.push((weight, to)),
        }
    }

    fn bias(
        &mut self,
        frequency: f64,
        direction: Border,
    ) {
        match direction {
            Border::PrefReg => self.toward += frequency,
            Border::PrefSpill => self.against += frequency,
            Border::MustSpill => self.against = f64::INFINITY,
            Border::DontCare => {}
        }
    }
}

/// How often each block runs per entry to the body, by its loop depth: a fact
/// of the blocks' shape, found once for every split of a function.
pub struct Levels;

impl Fact for Levels {
    type Result = IndexMap<i64, f64>;
    type Inputs = Shape;
    const NAME: &'static str = "block-levels";

    fn run(body: &LirBody) -> IndexMap<i64, f64> {
        ranges::depths_shared(body).iter().map(|(at, depth)| (*at, ranges::level(*depth))).collect()
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

/// One placement query over a body's bundles.
pub struct Placement<'a> {
    pub bundles: &'a Bundles,
    pub frequency: Arc<IndexMap<i64, f64>>,
    nodes: Vec<Node>,
    active: IdSet<usize>,
    todo: IdSet<usize>,
    /// Bundles that turned positive since the last scan or iteration.
    pub recent: Vec<usize>,
    threshold: f64,
}

impl<'a> Placement<'a> {
    /// A block runs `level(depth)` times per entry to the body. The threshold
    /// is LLVM's dead zone, 2 when the entry frequency is 2^14.
    pub fn new(
        body: &LirBody,
        bundles: &'a Bundles,
    ) -> Self {
        let frequency = body.facts.0.get::<Levels>(body);
        Self {
            bundles,
            frequency,
            nodes: vec![Node::default(); bundles.blocks.len()],
            active: IdSet::new(),
            todo: IdSet::new(),
            recent: Vec::new(),
            threshold: 2.0 / 16384.0,
        }
    }

    pub fn prepare(&mut self) {
        self.recent.clear();
        self.todo.clear();
        self.active.clear();
    }

    fn activate(
        &mut self,
        bundle: usize,
    ) {
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

    pub fn add_constraints(
        &mut self,
        constraints: &[Constraint],
    ) {
        for one in constraints {
            let frequency = self.frequency[&one.block] * one.weight;
            let (entry, exit) = self.bundles.of[&one.block];
            self.constrain(entry, exit, frequency, one.entry, one.exit);
        }
    }

    /// What one block wants at its borders, for a caller that has the block's
    /// bundles and its frequency (times the constraint's weight).
    pub fn constrain(
        &mut self,
        entry: usize,
        exit: usize,
        frequency: f64,
        on_entry: Border,
        on_exit: Border,
    ) {
        if on_entry != Border::DontCare {
            self.activate(entry);
            self.nodes[entry].bias(frequency, on_entry);
        }
        if on_exit != Border::DontCare {
            self.activate(exit);
            self.nodes[exit].bias(frequency, on_exit);
        }
    }

    /// Blocks that would rather not hold the value at all, twice as much when
    /// strong.
    pub fn add_pref_spill(
        &mut self,
        blocks: &[i64],
        strong: bool,
    ) {
        for block in blocks {
            let frequency = self.frequency[block] * if strong { 2.0 } else { 1.0 };
            let (entry, exit) = self.bundles.of[block];
            self.prefer_stack(entry, exit, frequency);
        }
    }

    /// `add_pref_spill` for a block of known bundles, at `frequency` already
    /// doubled where strong.
    pub fn prefer_stack(
        &mut self,
        entry: usize,
        exit: usize,
        frequency: f64,
    ) {
        for bundle in [entry, exit] {
            self.activate(bundle);
            self.nodes[bundle].bias(frequency, Border::PrefSpill);
        }
    }

    /// Blocks the value passes through untouched and uncontested.
    pub fn add_links(
        &mut self,
        blocks: &[i64],
    ) {
        for block in blocks {
            let (entry, exit) = self.bundles.of[block];
            let frequency = self.frequency[block];
            self.link_bundles(entry, exit, frequency);
        }
    }

    /// `add_links` for a block of known bundles and frequency.
    pub fn link_bundles(
        &mut self,
        entry: usize,
        exit: usize,
        frequency: f64,
    ) {
        if entry == exit {
            return;
        }
        self.activate(entry);
        self.activate(exit);
        self.nodes[entry].link(exit, frequency);
        self.nodes[exit].link(entry, frequency);
    }

    fn update(
        &mut self,
        bundle: usize,
    ) -> bool {
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
        let dissenting: Vec<usize> = self.nodes[bundle]
            .links
            .iter()
            .map(|(_, other)| *other)
            .filter(|other| self.nodes[*other].value != value)
            .collect();
        self.todo.extend(dissenting);
        true
    }

    /// Whether any bundle wants a register once the constraints are in.
    pub fn scan(&mut self) -> bool {
        self.recent.clear();
        for bundle in self.active.clone().iter() {
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

    /// `finish`, ascending, to search.
    pub fn finished(&self) -> Vec<usize> {
        self.active.iter().filter(|bundle| self.nodes[*bundle].prefers_register()).collect()
    }

    /// The bundles that settled in a register.
    pub fn finish(&self) -> IdSet<usize> {
        self.active.iter().filter(|bundle| self.nodes[*bundle].prefers_register()).collect()
    }
}
