//! A function's blocks as `graph` walks them: each block by its id,
//! in layout order, the entry first.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::dominators::DominatorTree;
use llrm_mir::loops::LoopInfo;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis, Dominators, Loops, PreservedAnalyses};
use llrm_mir::{Constant, ConstantKind, Context};
use llrm_support::hash::{HashMap, HashSet};

use crate::graph::loops::{self, Dominates, Loop, Node};

/// A block's id and its successors' ids.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub at: i64,
    pub succ: Vec<i64>,
}

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
    function
        .layout()
        .iter()
        .map(|&block| Block { at: id(block), succ: function.successors(block).into_iter().map(id).collect() })
        .collect()
}

/// `Dominators` as the graph walks read it: over block ids, and nothing
/// dominates a block the entry does not reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Dominance(Rc<DominatorTree>);

impl Dominance {
    pub fn new(tree: Rc<DominatorTree>) -> Self {
        Self(tree)
    }

    /// Of a body no manager holds.
    pub fn of(function: &Function) -> Self {
        Self(Rc::new(DominatorTree::new(function)))
    }

    pub fn tree(&self) -> &DominatorTree {
        &self.0
    }

    pub fn reachable(
        &self,
        at: i64,
    ) -> bool {
        self.0.is_reachable(block(at))
    }

    pub fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        self.reachable(at) && self.0.dominates(block(dominator), block(at))
    }

    /// The nearest strict dominator: none for the entry or an unreachable
    /// block.
    pub fn immediate(
        &self,
        at: i64,
    ) -> Option<i64> {
        self.0.immediate_dominator(block(at)).map(id)
    }

    /// Each block's dominators, itself among them; none for an unreachable one.
    pub fn dominators(
        &self,
        function: &Function,
    ) -> BTreeMap<i64, BTreeSet<i64>> {
        let above = |at: i64| {
            let mut found = BTreeSet::new();
            let mut next = self.reachable(at).then_some(at);
            while let Some(one) = next {
                found.insert(one);
                next = self.immediate(one);
            }
            found
        };
        function.layout().iter().map(|&one| (id(one), above(id(one)))).collect()
    }

    pub fn immediate_dominators(
        &self,
        function: &Function,
    ) -> BTreeMap<i64, Option<i64>> {
        function.layout().iter().map(|&one| (id(one), self.immediate(id(one)))).collect()
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
            if !self.reachable(at) {
                continue;
            }
            let preds = function
                .predecessors(one)
                .into_iter()
                .map(id)
                .filter(|&from| self.reachable(from))
                .collect::<BTreeSet<_>>();
            if preds.len() < 2 {
                continue;
            }
            for from in preds {
                let mut runner = Some(from);
                while let Some(walked) = runner {
                    if Some(walked) == self.immediate(at) {
                        break;
                    }
                    found.get_mut(&walked).expect("a block").insert(at);
                    runner = self.immediate(walked);
                }
            }
        }
        found
    }

    /// Blocks left in a cycle once every natural loop's back edge is cut.
    pub fn irreducible(
        &self,
        function: &Function,
    ) -> BTreeSet<i64> {
        loops::irreducible_under(&graph(function), self)
    }
}

impl Dominates for Dominance {
    fn reachable(
        &self,
        at: i64,
    ) -> bool {
        Dominance::reachable(self, at)
    }

    fn dominates(
        &self,
        dominator: i64,
        at: i64,
    ) -> bool {
        Dominance::dominates(self, dominator, at)
    }
}

/// `Loops` as the graph walks read them: innermost first where they nest,
/// else in the order their first back edge comes in the layout.
pub fn natural(
    function: &Function,
    info: &LoopInfo,
) -> Vec<Loop> {
    let headers = info.loops.iter().map(|one| (one.header, one)).collect::<HashMap<_, _>>();
    let mut seen = HashSet::default();
    let mut found = Vec::with_capacity(info.loops.len());
    for &at in function.layout() {
        for successor in function.successors(at) {
            let Some(one) = headers.get(&successor) else { continue };
            if one.latches.contains(&at) && seen.insert(successor) {
                found.push(Loop {
                    header: id(successor),
                    latches: one.latches.iter().map(|&latch| id(latch)).collect(),
                    body: one.blocks.iter().map(|&inside| id(inside)).collect(),
                });
            }
        }
    }
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
}

/// How many shapes this thread has derived, for a test that a pass asks for its
/// body's once.
pub fn shapes_derived() -> usize {
    DERIVED.with(std::cell::Cell::get)
}

impl Shape {
    /// Of a body no manager holds.
    pub fn of(function: &Function) -> Self {
        DERIVED.with(|count| count.set(count.get() + 1));
        let dominance = Dominance::of(function);
        let loops = natural(function, &LoopInfo::new(function, dominance.tree()));
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
/// Copied from llrm-core's `optimize/transform.rs` `_unreachable`. The old
/// MIR kept a dead block as an inert byte owner; the rich MIR owns no bytes.
pub fn _unreachable(
    context: &mut Context,
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
    let predecessors = loops::predecessors(&blocks);
    let mut changed = false;
    for &at in &reached {
        let live = |from: &Operand| {
            matches!(
                from,
                Operand::Block(one) if reached.contains(&id(*one)) && predecessors.get(&at).is_some_and(|from| from.contains(&id(*one)))
            )
        };
        for phi in function.block(block(at)).instructions().to_vec() {
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
        for inst in function.block(one).instructions().to_vec().into_iter().rev() {
            if let Some(result) = function.instruction(inst).result {
                let poison = context.constant(Constant { ty: function.value(result).ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(inst).expect("its uses were replaced");
        }
    }
    for &one in &dead {
        function.erase_block(one).expect("an emptied block nothing names");
    }
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
                Block { at: b1, succ: vec![b2, b3] },
                Block { at: b2, succ: vec![] },
                Block { at: b3, succ: vec![] },
                Block { at: dead, succ: vec![b2] },
            ]
        );
    }
}
