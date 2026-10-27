//! A function's blocks as the graph walks see them: block ids, successors,
//! dominance, natural loops and reachability, built once per body.
//!
//! # How it works
//! `graph` turns a `Function` into `Block { at, succ }`s, one per block in
//! layout order, the entry first, so the walks of [`crate::graph::loops`] run
//! on the rich MIR. `successors_of` and `predecessors_of` answer for one block
//! without building the whole graph, and the `Around` trait does the same for
//! a `Loop`: `entering` (outside blocks that name the header), `exits` (edges
//! leaving the body) and `preheader` (the single entering block whose only
//! successor is the header). Building the graph per loop made a function of N
//! loops cost N^2.
//!
//! `Shape` bundles what the passes ask of a body:
//!
//! ```text
//!  Shape
//!   +- dominance: Dominance   over llrm_mir Dominators; reachable, dominates,
//!   |                         immediate, depths, frontiers, irreducible
//!   +- loops: Vec<Loop>       from llrm_mir Loops, ids not BlockIds,
//!   |                         inner loops first
//!                             and cached
//! ```
//!
//! `Dominance` treats an unreachable block as dominated by nothing and
//! dominating nothing. `dominates` and `depths` answer without building each
//! block's full dominator set (`dominators`, quadratic in a chain of
//! diamonds). `natural` orders loops by body size, inner first.
//! `Reaching::to` walks predecessors from the target and returns the cached
//! set on a second ask.
//!
//! `_unreachable` deletes blocks the entry does not reach, trims phi inputs of
//! removed edges, and replaces uses of values defined in dead blocks with
//! poison before erasing them.
//!
//! # Example
//! A `switch` names its default and a case target `%b2` twice, and `dead` is
//! not reachable:
//!
//! ```mir
//! define void @f(i16 %s) {
//! b1:
//!   switch i16 %s, label %b2 [ i16 1, label %b2
//!                             i16 2, label %b3 ]
//!
//! b2:
//!   ret void
//!
//! b3:
//!   ret void
//!
//! dead:
//!   br label %b2
//! }
//! ```
//!
//! `graph(function)` lists the blocks in layout order, each target once, the
//! unreachable block kept:
//!
//! | `at` | `succ` |
//! | --- | --- |
//! | `b1` | `b2`, `b3` |
//! | `b2` | none |
//! | `b3` | none |
//! | `dead` | `b2` |
//!
//! From unit test
//! `a_switch_names_each_target_once_and_an_unreachable_block_stays_in_the_graph`
//! in `cfg.rs`.
//!
//! # Provides
//! `Shape` (a [`Analysis`](llrm_mir::passes::Analysis) named `shape`), the
//! `Dominance` and `Reaching` types, and the id/graph helpers. Consumers:
//! nearly every loop pass and analysis in `llrm-transforms` and this crate,
//! and the backend driver and isel (grep `cfg::Shape`, `cfg::id`).
//!
//! # Needs
//! llrm-mir's `Dominators` and `Loops`.
//!
//! # Keeps
//! `Shape` is preserved when it is kept, or when both `Dominators` and `Loops`
//! are. It does not read the outer (module) analyses.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::domtree_updater::{DomTreeUpdater, UpdateStrategy};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, Mark, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis, Dominators, Loops, PreservedAnalyses};
use llrm_mir::{Constant, ConstantKind, Context};

use crate::graph::loops::{self, Dominates, Loop, Node};

/// A loop in the form the loop passes copy and join: one latch, one way in
/// (a preheader that only enters it) and one way out, to a block only it
/// enters.
#[derive(Clone, Debug)]
pub struct Canonical {
    pub loop_: Loop,
    pub header: BlockId,
    pub latch: BlockId,
    pub preheader: BlockId,
    /// The block of the loop that leaves it, and where it leaves to.
    pub exiting: BlockId,
    pub exit: BlockId,
}

impl Canonical {
    pub fn of(
        function: &Function,
        loop_: &Loop,
    ) -> Option<Self> {
        let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
        let header = block(loop_.header);
        let entering = function
            .predecessors(header)
            .into_iter()
            .filter(|from| !loop_.body.contains(&id(*from)))
            .collect::<Vec<_>>();
        let [preheader] = entering[..] else { return None };
        if function.successors(preheader) != [header] {
            return None;
        }
        let leaving = loop_
            .body
            .iter()
            .flat_map(|&at| {
                let from = block(at);
                function.successors(from).into_iter().map(move |to| (from, to))
            })
            .filter(|(_, to)| !loop_.body.contains(&id(*to)))
            .collect::<Vec<_>>();
        let [(exiting, exit)] = leaving[..] else { return None };
        // Dedicated: only the loop enters its exit.
        (function.predecessors(exit) == [exiting]).then(|| Self {
            loop_: loop_.clone(),
            header,
            latch: block(latch),
            preheader,
            exiting,
            exit,
        })
    }

    pub fn holds(
        &self,
        block: BlockId,
    ) -> bool {
        self.loop_.body.contains(&id(block))
    }
}

/// A block's id and its successors' ids.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub at: i64,
    /// One or two ids for nearly every block, held in the block: a `Vec` for
    /// each was 117,763 of a d_faces -O2 compile's 2.9 M allocations.
    pub succ: Successors,
}

/// A block's successors' ids; two are inline (a branch).
pub type Successors = llrm_mir::inline::InlineVec<i64, 2>;

impl Node for Block {
    fn at(&self) -> i64 {
        self.at
    }

    fn succ(&self) -> &[i64] {
        &self.succ
    }
}

pub fn id(block: BlockId) -> i64 {
    i64::from(block.0)
}

pub fn block(at: i64) -> BlockId {
    BlockId(at as u32)
}

/// `function`'s graph, in layout order.
pub fn graph(function: &Function) -> Vec<Block> {
    GRAPHS.with(|count| count.set(count.get() + 1));
    function
        .layout()
        .iter()
        .map(|&block| Block { at: id(block), succ: function.successors(block).into_iter().map(id).collect() })
        .collect()
}

/// The ids of the blocks `at` names, from its terminator: one block's
/// successors, without the graph of the whole body (LLVM's `successors(BB)`,
/// gcc's `bb->succs`).
pub fn successors_of(
    function: &Function,
    at: i64,
) -> Successors {
    function.successors(block(at)).into_iter().map(id).collect()
}

/// The blocks that name `at`, from the users of the block (LLVM's
/// `predecessors(BB)`, gcc's `bb->preds`).
pub fn predecessors_of(
    function: &Function,
    at: i64,
) -> BTreeSet<i64> {
    function.predecessors(block(at)).into_iter().map(id).collect()
}

/// What a loop asks of the body around it, from its own blocks and the users of
/// its header (LLVM's `Loop::getLoopPredecessor`, `getExitEdges`,
/// `getLoopPreheader`; gcc's `loop_preheader_edge` and `get_loop_exit_edges`),
/// not from the graph of the whole body: found for each loop of a function of N
/// loops that was N^2.
pub trait Around {
    /// The blocks outside the loop that name its header: where it is entered
    /// from.
    fn entering(
        &self,
        function: &Function,
    ) -> BTreeSet<i64>;

    /// The edges leaving the loop, as (from, to), in the order of its blocks.
    fn exits(
        &self,
        function: &Function,
    ) -> Vec<(i64, i64)>;

    /// The one block entering the loop that only goes to its header.
    fn preheader(
        &self,
        function: &Function,
    ) -> Option<i64>;
}

impl Around for Loop {
    fn entering(
        &self,
        function: &Function,
    ) -> BTreeSet<i64> {
        predecessors_of(function, self.header).into_iter().filter(|at| !self.body.contains(at)).collect()
    }

    fn exits(
        &self,
        function: &Function,
    ) -> Vec<(i64, i64)> {
        llrm_mir::counts::bump(llrm_mir::counts::Event::LoopExitScans);
        self.body
            .iter()
            .flat_map(|&from| successors_of(function, from).into_iter().map(move |to| (from, to)))
            .filter(|(_, to)| !self.body.contains(to))
            .collect()
    }

    fn preheader(
        &self,
        function: &Function,
    ) -> Option<i64> {
        let entering = self.entering(function);
        let [one] = entering.iter().copied().collect::<Vec<_>>()[..] else { return None };
        (successors_of(function, one) == [self.header]).then_some(one)
    }
}

/// `Dominators` as the graph walks read it: over block ids, and nothing
/// dominates a block the entry does not reach.
///
/// The tree may be one a pass has not asked to bring up to date yet (a lazy
/// `DomTreeUpdater`: the edits since `mark` queue as the change log, and the
/// first reader applies them as one batch). That reader says which function
/// it asks of, so no pass pays for a tree nobody reads: `rotate` rotated N/8
/// loops in a function and asked after each.
#[derive(Clone, Debug)]
pub struct Dominance(Rc<Lazy>);

struct Lazy {
    tree: std::cell::OnceCell<Rc<DomTree>>,
    /// The tree as it was at `mark`, until the first read.
    pending: RefCell<Option<(Rc<DomTree>, Mark)>>,
}

impl std::fmt::Debug for Lazy {
    fn fmt(
        &self,
        out: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        out.debug_struct("Lazy").field("resolved", &self.tree.get().is_some()).finish()
    }
}

/// Two are the same where they are one, or both resolved to equal trees.
impl PartialEq for Dominance {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
            || matches!(
                (self.0.tree.get(), other.0.tree.get()),
                (Some(a), Some(b)) if a == b
            )
    }
}

impl Dominance {
    pub fn new(tree: Rc<DomTree>) -> Self {
        Self(Rc::new(Lazy { tree: std::cell::OnceCell::from(tree), pending: RefCell::new(None) }))
    }

    /// `tree`, true of the function at `mark`, to be brought up to date from
    /// the edits since by the first reader (`DomTreeUpdater`, Lazy).
    pub fn pending(
        tree: Rc<DomTree>,
        mark: Mark,
    ) -> Self {
        Self(Rc::new(Lazy { tree: std::cell::OnceCell::new(), pending: RefCell::new(Some((tree, mark))) }))
    }

    /// Of a body no manager holds.
    pub fn of(function: &Function) -> Self {
        Self::new(Rc::new(DomTree::of(function)))
    }

    /// The tree of `function`: the edits queued since it was true applied now
    /// as one batch, or the tree built afresh where the log cannot tell them.
    pub fn tree(
        &self,
        function: &Function,
    ) -> &DomTree {
        self.0.tree.get_or_init(|| {
            let (tree, mark) = self.0.pending.borrow_mut().take().expect("a tree is resolved or pending");
            let Some(updates) = function.changes_since(mark).and_then(|changes| tree.updates_for(function, changes))
            else {
                return Rc::new(DomTree::of(function));
            };
            let mut updater = DomTreeUpdater::new(Some(Rc::unwrap_or_clone(tree)), None, UpdateStrategy::Lazy);
            updater.apply_updates(function, &updates);
            updater.get_dom_tree(function);
            let (tree, _) = updater.into_trees(function);
            Rc::new(tree.expect("a dominator tree was given"))
        })
    }

    /// `dominates` of a `Dominance` its owner resolved (the memory SSA asks
    /// the tree it was built over, for a function it no longer holds).
    pub fn dominates_resolved(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        let tree = self.0.tree.get().expect("the owner resolved the tree");
        tree.is_reachable(block(at)) && tree.dominates(block(dominator), block(at))
    }

    /// `reachable` of a resolved `Dominance`.
    pub fn reachable_resolved(
        &self,
        at: i64,
    ) -> bool {
        self.0.tree.get().expect("the owner resolved the tree").is_reachable(block(at))
    }

    /// `immediate` of a resolved `Dominance`.
    pub fn immediate_resolved(
        &self,
        at: i64,
    ) -> Option<i64> {
        self.0.tree.get().expect("the owner resolved the tree").immediate_dominator(block(at)).map(id)
    }

    pub fn reachable(
        &self,
        function: &Function,
        at: i64,
    ) -> bool {
        self.tree(function).is_reachable(block(at))
    }

    pub fn dominates(
        &self,
        function: &Function,
        dominator: i64,
        at: i64,
    ) -> bool {
        self.reachable(function, at) && self.tree(function).dominates(block(dominator), block(at))
    }

    /// The nearest strict dominator: none for the entry or an unreachable
    /// block.
    pub fn immediate(
        &self,
        function: &Function,
        at: i64,
    ) -> Option<i64> {
        self.tree(function).immediate_dominator(block(at)).map(id)
    }

    /// Each block's dominators, itself among them; none for an unreachable one.
    pub fn dominators(
        &self,
        function: &Function,
    ) -> BTreeMap<i64, BTreeSet<i64>> {
        let above = |at: i64| {
            let mut found = BTreeSet::new();
            let mut next = self.reachable(function, at).then_some(at);
            while let Some(one) = next {
                found.insert(one);
                next = self.immediate(function, one);
            }
            found
        };
        function.layout().iter().map(|&one| (id(one), above(id(one)))).collect()
    }

    /// How many blocks dominate each, itself among them (`dominators`' set
    /// sizes): none for an unreachable block.
    pub fn depths(
        &self,
        function: &Function,
    ) -> BTreeMap<i64, usize> {
        let mut found = BTreeMap::<i64, usize>::new();
        for &start in function.layout() {
            let mut path = Vec::new();
            let mut next = self.reachable(function, id(start)).then_some(id(start));
            let mut base = 0;
            while let Some(one) = next {
                if let Some(&known) = found.get(&one) {
                    base = known;
                    break;
                }
                path.push(one);
                next = self.immediate(function, one);
            }
            for (below, one) in path.into_iter().rev().enumerate() {
                found.insert(one, base + below + 1);
            }
            found.entry(id(start)).or_insert(0);
        }
        found
    }

    pub fn immediate_dominators(
        &self,
        function: &Function,
    ) -> BTreeMap<i64, Option<i64>> {
        function.layout().iter().map(|&one| (id(one), self.immediate(function, id(one)))).collect()
    }

    /// Where a definition stops being the only one that reaches: the
    /// blocks a phi belongs in.
    pub fn frontiers(
        &self,
        function: &Function,
    ) -> BTreeMap<i64, BTreeSet<i64>> {
        let mut found = function.layout().iter().map(|&one| (id(one), BTreeSet::new())).collect::<BTreeMap<_, _>>();
        for &one in function.layout() {
            let at = id(one);
            if !self.reachable(function, at) {
                continue;
            }
            let preds = function
                .predecessors(one)
                .into_iter()
                .map(id)
                .filter(|&from| self.reachable(function, from))
                .collect::<BTreeSet<_>>();
            if preds.len() < 2 {
                continue;
            }
            for from in preds {
                let mut runner = Some(from);
                while let Some(walked) = runner {
                    if Some(walked) == self.immediate(function, at) {
                        break;
                    }
                    found.get_mut(&walked).expect("a block").insert(at);
                    runner = self.immediate(function, walked);
                }
            }
        }
        found
    }

    /// Whether `irreducible` is not empty, from one walk: some edge back to a
    /// block on the walk's stack goes to one that does not dominate its source.
    /// Only blocks the entry reaches count, as in `irreducible`.
    pub fn is_irreducible(
        &self,
        function: &Function,
    ) -> bool {
        let Some(entry) = function.entry() else { return false };
        const GREY: u8 = 1;
        const BLACK: u8 = 2;
        let mut colour = vec![0_u8; function.block_capacity()];
        let mut stack = vec![(entry, function.successors(entry), 0_usize)];
        colour[entry.0 as usize] = GREY;
        while let Some((at, successors, next)) = stack.last_mut() {
            let Some(&successor) = successors.get(*next) else {
                colour[at.0 as usize] = BLACK;
                stack.pop();
                continue;
            };
            *next += 1;
            let from = *at;
            match colour[successor.0 as usize] {
                GREY if !self.dominates(function, id(successor), id(from)) => return true,
                0 => {
                    colour[successor.0 as usize] = GREY;
                    stack.push((successor, function.successors(successor), 0));
                }
                _ => {}
            }
        }
        false
    }

    /// Blocks left in a cycle once every natural loop's back edge is cut.
    pub fn irreducible(
        &self,
        function: &Function,
    ) -> BTreeSet<i64> {
        loops::irreducible_under(&graph(function), &Bound { dominance: self, function })
    }
}

/// A `Dominance` and the function it is asked of, to the graph walks that
/// take any `Dominates`.
pub struct Bound<'a> {
    pub dominance: &'a Dominance,
    pub function: &'a Function,
}

impl Dominates for Bound<'_> {
    fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.dominance.reachable(self.function, at)
    }

    fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        self.dominance.dominates(self.function, dominator, at)
    }
}

/// `Loops` as the graph walks read them: innermost first where they nest,
/// else in the order their first back edge comes in the layout.
pub fn natural(
    function: &Function,
    info: &LoopInfo,
) -> Vec<Loop> {
    let forest = std::rc::Rc::new(llrm_mir::forest::Forest::of_graph(
        function,
        info,
        function.layout().iter().copied(),
        |block| i64::from(block.0),
    ));
    let mut found: Vec<Loop> = forest
        .loops
        .iter()
        .enumerate()
        .map(|(index, one)| Loop {
            header: one.header,
            latches: one.latches.iter().copied().collect(),
            body: llrm_mir::forest::Body::new(&forest, index),
        })
        .collect();
    found.sort_by_key(|one| one.body.len());
    found
}

/// A function's dominance and natural loops, as block ids: what the graph
/// walks read of `Dominators` and `Loops`.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub dominance: Dominance,
    pub loops: Vec<Loop>,
}

thread_local! {
    static DERIVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static GRAPHS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many whole-body graphs `graph` has built on this thread, for a test
/// that a pass asking about a few blocks builds none.
pub fn graphs_built() -> usize {
    GRAPHS.with(std::cell::Cell::get)
}

/// How many shapes (or loop sets, `loops_of`) this thread has derived, for a
/// test that a pass asks for its body's once.
pub fn shapes_derived() -> usize {
    DERIVED.with(std::cell::Cell::get)
}

/// The natural loops of `function`, outermost last: what a pass that reads only
/// the loops asks, with no `Shape` to hold.
pub fn loops_of(function: &Function) -> Vec<Loop> {
    DERIVED.with(|count| count.set(count.get() + 1));
    llrm_mir::counts::bump(llrm_mir::counts::Event::Shapes);
    let dominance = Dominance::of(function);
    natural(function, &LoopInfo::analyze(function, || dominance.tree(function)))
}

impl Shape {
    /// Of a body no manager holds.
    pub fn of(function: &Function) -> Self {
        DERIVED.with(|count| count.set(count.get() + 1));
        llrm_mir::counts::bump(llrm_mir::counts::Event::Shapes);
        let dominance = Dominance::of(function);
        let loops = natural(function, &LoopInfo::analyze(function, || dominance.tree(function)));
        Self { dominance, loops }
    }
}

impl Analysis for Shape {
    type Result = Shape;
    const NAME: &'static str = "shape";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Shape {
        DERIVED.with(|count| count.set(count.get() + 1));
        let dominance = Dominance::new(analyses.get::<Dominators>(context, layout, function));
        let loops = natural(function, &analyses.get::<Loops>(context, layout, function));
        Shape { dominance, loops }
    }

    const READS_OUTER: bool = false;

    fn preserved(preserved: &PreservedAnalyses) -> bool {
        preserved.kept::<Shape>() || (Dominators::preserved(preserved) && Loops::preserved(preserved))
    }
}

/// Blocks the entry does not reach, gone, and each phi's inputs from
/// edges that no longer exist with them.
///
/// Copied from llrm-codegen's `optimize/transform.rs` `_unreachable`. The old
/// MIR kept a dead block as an inert byte owner; the rich MIR owns no bytes.
pub fn _unreachable(
    context: &Context,
    function: &mut Function,
) -> bool {
    let blocks = graph(function);
    let Some(entry) = blocks.first().map(|block| block.at) else { return false };
    let successors = blocks.iter().map(|block| (block.at, &block.succ)).collect::<std::collections::BTreeMap<_, _>>();
    let (mut reached, mut pending) = (BTreeSet::new(), vec![entry]);
    while let Some(at) = pending.pop() {
        if reached.insert(at) {
            pending.extend(successors[&at].iter().copied());
        }
    }
    let mut changed = false;
    for &at in &reached {
        // Terminators name blocks; the phis edited below do not change who
        // the predecessors are.
        let predecessors = function.predecessors(block(at));
        let live = |from: &Operand| {
            matches!(
                from,
                Operand::Block(one) if reached.contains(&id(*one)) && predecessors.contains(one)
            )
        };
        for phi in function.iter_block(block(at)).collect::<Vec<_>>() {
            if function.instruction(phi).opcode != Opcode::Phi {
                break;
            }
            let operands = &function.instruction(phi).operands;
            let kept: Vec<Operand> = operands.chunks(2).filter(|pair| live(&pair[1])).flatten().copied().collect();
            if kept.len() != operands.len() {
                function.set_operands(phi, kept);
                changed = true;
            }
        }
    }
    let dead: Vec<BlockId> = blocks.iter().filter(|one| !reached.contains(&one.at)).map(|one| block(one.at)).collect();
    // Values defined there are used only there, or by phis on dropped edges.
    for &one in &dead {
        for inst in function.iter_block(one).collect::<Vec<_>>().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
    }
    function.erase_blocks(&dead).expect("emptied blocks nothing names");
    changed || !dead.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{function, parsed};

    #[test]
    fn a_functions_loop_is_found_through_its_graph() {
        let module = parsed(
            "define void @f(i1 %c) {
b1:
  br label %b2

b2:
  br i1 %c, label %b2, label %b3

b3:
  ret void
}
",
        );
        let function = function(&module, "f");
        let found = Shape::of(function).loops;
        assert_eq!(found.len(), 1);
        assert_eq!(block(found[0].header), function.layout()[1]);
    }

    #[test]
    fn a_switch_names_each_target_once_and_an_unreachable_block_stays_in_the_graph() {
        let module = parsed(
            "define void @f(i16 %s) {
b1:
  switch i16 %s, label %b2 [ i16 1, label %b2
                            i16 2, label %b3 ]

b2:
  ret void

b3:
  ret void

dead:
  br label %b2
}
",
        );
        let function = function(&module, "f");
        let [b1, b2, b3, dead] = [0, 1, 2, 3].map(|at| id(function.layout()[at]));
        assert_eq!(
            graph(function),
            [
                Block { at: b1, succ: vec![b2, b3].into() },
                Block { at: b2, succ: vec![].into() },
                Block { at: b3, succ: vec![].into() },
                Block { at: dead, succ: vec![b2].into() },
            ]
        );
    }

    /// Every pass that asked whether one block dominates another built every
    /// block's whole set of dominators (a set a block deep in a chain of N
    /// diamonds, N^2 entries): 39% of `mir gvn` at `branches` N=512. The answer
    /// is `dominates`, and the count of a set is `depths`; both must give what
    /// the sets did, unreachable blocks included.
    #[test]
    fn test_dominates_and_depths_are_what_the_dominator_sets_give() {
        let module = parsed(
            "define void @f(i1 %c, i1 %d) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br i1 %d, label %b3, label %b4

b3:
  br label %b5

b4:
  br label %b5

b5:
  br i1 %d, label %b5, label %b6

b6:
  ret void

dead:
  br label %b6
}
",
        );
        let function = function(&module, "f");
        let dominance = Dominance::of(function);
        let sets = dominance.dominators(function);
        let depths = dominance.depths(function);
        assert!(sets.values().any(|set| set.is_empty()), "the premise: a block the entry does not reach");
        for &below in function.layout() {
            assert_eq!(depths[&id(below)], sets[&id(below)].len(), "depth of {}", id(below));
            for &above in function.layout() {
                assert_eq!(
                    dominance.dominates(function, id(above), id(below)),
                    sets[&id(below)].contains(&id(above)),
                    "{} over {}",
                    id(above),
                    id(below)
                );
            }
        }
    }

    /// The blocks with a path to a block, as a pass that follows predecessors
    /// finds them, kept for the next ask.
    #[test]
    fn test_reaching_is_the_blocks_with_a_path_to_a_block() {
        let module = parsed(
            "define void @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  br i1 %c, label %b3, label %b4

b4:
  ret void

dead:
  br label %b4
}
",
        );
        let function = function(&module, "f");
        let shape = Shape::of(function);
        let tree = shape.dominance.tree(function);
        let at = |name: &str| {
            function.layout().iter().copied().find(|&b| function.block(b).name.as_deref() == Some(name)).unwrap()
        };
        let reaching = |to: &str| {
            function
                .layout()
                .iter()
                .filter(|&&b| crate::reachability::is_potentially_reachable_blocks(function, b, at(to), Some(tree)))
                .map(|&b| function.block(b).name.clone().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(reaching("b1"), ["b0", "b1"]);
        assert_eq!(reaching("b3"), ["b0", "b1", "b2", "b3"]);
        assert_eq!(reaching("b4"), ["b0", "b1", "b2", "b3", "b4", "dead"]);
    }
}
