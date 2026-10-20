//! Loop rotation: a loop proven to run at least once is entered at its body,
//! not its test; a loop that is not proven is entered behind a copy of its
//! test (gcc's `-ftree-ch`).
//!
//! # How it works
//! A rotatable loop (`Shape`) has one preheader that jumps only to the header;
//! one latch that goes only back to the header; a header that holds the exit
//! test, a conditional branch to the first body block `first` and to the
//! exit; and a `first` reached from nowhere else, with no phis. What the
//! header computes besides its phis must be test-only (compares, casts, geps,
//! selects; a division only where the test is copied before the loop, so
//! skipping it on entry skips no trap), and a value a header phi takes round
//! the back edge, a counter's step, is not test-only.
//!
//! `_rotate` moves the header's phis to `first`: `p(preheader: a, latch: b)`
//! becomes `p'(preheader: a, header: b')`, phis read in parallel so a swap
//! needs no order. `ssa::SsaUpdater` rewrites every other use, outside the loop
//! included: the body reads `p'`, the header and what follows the loop read
//! `a` as it left the preheader and `b'` as it left the latch. Afterwards the
//! test is reached only from the latch, and `cfg::merged` merges it into it.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   h["<b>H</b><br/><small>p = phi [a, pre] [b, L]<br/>if t(p)</small>"]:::mid
//!   f["<b>F</b><br/><small>the body</small>"]:::mid
//!   l["<b>L</b><br/><small>b = ...</small>"]:::mid
//!   x["<b>X</b><br/><small>the exit</small>"]:::mid
//!   pre --> h
//!   h -->|"true"| f
//!   h -->|"false"| x
//!   f --> l
//!   l --> h
//! ```
//!
//! *Before: the test is in the header*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   f["<b>F</b><br/><small>p2 = phi [a, pre] [b2, L]<br/>the body</small>"]:::mid
//!   l["<b>L</b><br/><small>b2 = ...<br/>if t(b2)</small>"]:::mid
//!   x["<b>X</b><br/><small>the exit</small>"]:::mid
//!   pre --> f
//!   f --> l
//!   l -->|"true"| f
//!   l -->|"false"| x
//! ```
//!
//! *After: rotated, the test is in the latch*
//!
//! Two modes, set by the fields of `Rotate`:
//! - `proven` (`rotated`): the loop has a proven trip count
//!   (`induction::trip_count`), so its first test passes. The entry goes
//!   straight to the body. A loop of none or of an unknown count keeps its
//!   first test.
//! - `copy` (`copied`): a loop not proven to run is entered behind a copy of
//!   its test, in the preheader, which skips the loop to the exit when it fails
//!   (gcc's `should_duplicate_loop_header_p`: a header that ends in the exit
//!   test and computes only test values). A loop with a known trip count is
//!   left to `proven`. The header may have at most 20 instructions besides phis
//!   (`param_max_loop_header_insns`); a chain of header tests (`while (a &&
//!   b)`) is copied test by test within what is left of that budget. The guard
//!   takes the loop test's branch weights, else the loop heuristic's, so the
//!   way into the loop is laid out as the likely one.
//!
//! The copy is on at -O1 and up, not at -Os (`-fno-tree-ch` turns it off): the
//! user's decision of 2026-10-12 (docs/reference/levels.md) accepts its compile
//! cost (+9.3% at -O1, +14.0% at -O2 over the vsgcc programs) and the m16 bench
//! rows it moves (#1557); the steeper slopes of rotate, loopslots and
//! `analysis registers` on nests and many-loop functions are #1559.
//!
//! A rotated loop is no longer the pre-tested shape the counted-loop
//! analyses, [`crate::looppeel`] and [`crate::loopunrollpass`] read, so
//! rotation goes after them. [`crate::lsr_pass`] enters a loop it counts to
//! zero at its body too, behind a skip guard, with the same `_rotate`.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @"_sum@3"(ptr nocapture readonly %0, i16 %1) ... {
//! b1:
//!   br label %b2
//!
//! b2:
//!   %2 = phi i16 [ 0, %b1 ], [ %10, %b4 ]
//!   %3 = phi i16 [ 0, %b1 ], [ %9, %b4 ]
//!   %4 = icmp slt i16 %2, %1
//!   br i1 %4, label %b4, label %b3
//!
//! b3:
//!   %5 = phi i16 [ %3, %b2 ]
//!   ret i16 %5
//!
//! b4:
//!   %6 = mul i16 %2, 2
//!   %7 = getelementptr inbounds i8, ptr %0, i16 %6
//!   %8 = load i16, ptr %7, !tbaa !9
//!   %9 = add nsw i16 %3, %8
//!   %10 = add nsw i16 %2, 1
//!   br label %b2
//! }
//! ```
//! After:
//! ```mir
//! define ... @"_sum@3"(ptr nocapture readonly %0, i16 %1) ... {
//! b1:
//!   %2 = icmp slt i16 0, %1
//!   br i1 %2, label %b4, label %b3, !prof !24
//!
//! b3:
//!   %3 = phi i16 [ %9, %b4 ], [ 0, %b1 ]
//!   ret i16 %3
//!
//! b4:
//!   %4 = phi i16 [ 0, %b1 ], [ %9, %b4 ]
//!   %5 = phi i16 [ 0, %b1 ], [ %10, %b4 ]
//!   %6 = mul i16 %5, 2
//!   %7 = getelementptr inbounds i8, ptr %0, i16 %6
//!   %8 = load i16, ptr %7, !tbaa !9
//!   %9 = add nsw i16 %4, %8
//!   %10 = add nsw i16 %5, 1
//!   %11 = icmp slt i16 %10, %1
//!   br i1 %11, label %b4, label %b3
//! }
//! ```
//! The loop test is copied into the preheader (`%2`) as a guard. The header
//! phis moved into the body `b4`, and the test now sits at the latch, so each
//! trip runs one branch.
//!
//! From `--dump` of `sum` (`for (i = 0; i < n; i++) s += a[i];`) at -O2 -m16
//! -ftree-ch, stages `calleepop` and `rotate`.
//!
//! # Runs
//! Twice in [`crate::pipeline`], outside the per-body fixed point. `copy` runs
//! with `copy_headers` (`-ftree-ch`, on at -O1 and up, `-fno-tree-ch` turns it
//! off) and not at `-Os`, after the interprocedural step and before
//! `fixednarrow`, followed by loop simplification, LCSSA and cleanup. `proven`
//! always runs, last of the loop passes, after `window`. `LLRM_DEBUG=census`
//! prints what the trip-count analyses prove per loop, to compare builds with
//! and without `-ftree-ch`.
//!
//! On by default is the user's decision of 2026-10-12
//! (docs/reference/levels.md): compile +9.3% at -O1 and +14.0% at -O2 over the
//! vsgcc programs, the m16 bench rows it moves (#1557), and steeper slopes of
//! rotate, loopslots and `analysis registers` on nests and many-loop functions
//! (#1559).
//!
//! # Keeps
//! Nothing where it changes a body.
//!
//! # See also
//! gcc's `pass_ch` (`tree-ssa-loop-ch.cc`) and LLVM's LoopRotate.

use std::collections::BTreeSet;

use llrm_analysis::cfg::Around;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, MetadataOperand, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, PreservedAnalyses};

use crate::lcssa::{arms, from_arms};

/// `copy`: the loops not proven to run are entered behind a copy of their test
/// too. gcc does this at -O1 and above and not at -Os, where it adds code
/// (`optimize_loop_for_size_p`): there only the proven loops are entered at
/// their body.
pub struct Rotate {
    /// Loops proven to run are entered at their body.
    pub proven: bool,
    pub copy: bool,
}

impl FunctionPass for Rotate {
    fn name(&self) -> &'static str {
        "rotate"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if self.proven && llrm_support::debug::enabled("census") {
            census(unit.context, unit.layout, unit.function, unit.id, analyses);
        }
        match entered(unit.context, unit.layout, unit.function, unit.declared, analyses, self.proven, self.copy) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("rotate: {error}"),
        }
    }
}

/// Every proven loop entered at its body, and each test merged into its
/// latch; whether any loop was.
pub fn entered(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    analyses: &mut Analyses,
    proven: bool,
    copy: bool,
) -> Result<bool, String> {
    let mut done: llrm_mir::dense::IdSet<BlockId> = llrm_mir::dense::IdSet::new();
    let mut forest = Forest::default();
    while proven && rotated(context, layout, function, analyses, &mut forest, &mut done)? {}
    while copy && copied(context, layout, function, declared, analyses, &mut forest, &mut done)? {}
    if done.is_empty() {
        return Ok(false);
    }
    crate::cfg::merged(function);
    crate::canonical::identities(context, function);
    Ok(true)
}

/// The function's natural loops, held through the rotations of one run. A
/// rotation moves a loop's header to the block that follows it and its one
/// latch to the old header, and splitting the edge into a loop puts one block
/// into each loop around it: no other loop's blocks, headers or latches change,
/// so the loops are not derived again for each rotation (a nest of N loops
/// rotated N times, N^2 blocks of loops each time). `LLRM_CHECK_SHAPE` holds
/// them to a new derivation after each.
#[derive(Default)]
struct Forest {
    loops: Option<Vec<Loop>>,
}

impl Forest {
    /// The shape of `function` as it stands: its dominance, queued (see below),
    /// and the loops held, made from `fresh` the first time; `fresh` holds
    /// it too, so that what it derives next does not derive it again.
    /// Give it back with `keep` once `fresh` is dropped.
    fn shape(
        &mut self,
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        fresh: &mut Analyses,
        analyses: &mut Analyses,
    ) -> std::rc::Rc<cfg::Shape> {
        let loops = match self.loops.take() {
            Some(loops) => loops,
            None => fresh.get::<cfg::Shape>(context, layout, function).loops.clone(),
        };
        let shape = std::rc::Rc::new(cfg::Shape {
            // The manager's, as it was true: the edges the rotations change
            // queue in the log (a lazy `DomTreeUpdater`) and the first reader
            // applies them, so a rotation nobody asks the tree after costs
            // none.
            dominance: match analyses.held::<llrm_mir::passes::Dominators>() {
                Some((tree, mark)) => cfg::Dominance::pending(tree, mark),
                None => cfg::Dominance::new(analyses.get::<llrm_mir::passes::Dominators>(context, layout, function)),
            },
            loops,
        });
        fresh.seed::<cfg::Shape>(function, std::rc::Rc::clone(&shape));
        shape
    }

    fn keep(
        &mut self,
        shape: std::rc::Rc<cfg::Shape>,
    ) {
        self.loops = std::rc::Rc::try_unwrap(shape).ok().map(|shape| shape.loops);
    }

    /// The loop that had `header` is entered at `first` now and goes back from
    /// `latch` alone.
    fn rotated(
        &mut self,
        function: &Function,
        header: i64,
        first: i64,
        latch: i64,
    ) {
        let Some(loops) = self.loops.as_mut() else { return };
        if let Some(one) = loops.iter_mut().find(|one| one.header == header) {
            one.header = first;
            one.latches = BTreeSet::from([latch]);
        }
        Self::ordered(function, loops);
        self.check(function);
    }

    /// `bridge` stands on the edge from `from` into the loop at `header`: it is
    /// in the loops `from` is in.
    fn split(
        &mut self,
        function: &Function,
        bridge: i64,
        from: i64,
        header: i64,
    ) {
        let Some(loops) = self.loops.as_mut() else { return };
        for one in loops.iter_mut().filter(|one| one.header != header && one.body.contains(&from)) {
            one.body.insert(bridge);
        }
        // A loop one block larger may now be the larger of two siblings.
        Self::ordered(function, loops);
    }

    /// As `cfg::natural` lists them: smallest first, equal ones by the place in
    /// the layout of the first of their latches.
    fn ordered(
        function: &Function,
        loops: &mut [Loop],
    ) {
        let place: llrm_support::hash::HashMap<i64, usize> =
            function.layout().iter().enumerate().map(|(at, &block)| (cfg::id(block), at)).collect();
        loops.sort_by_key(|one| {
            (
                one.body.len(),
                one.latches.iter().filter_map(|latch| place.get(latch)).min().copied().unwrap_or(usize::MAX),
            )
        });
    }

    fn check(
        &self,
        function: &Function,
    ) {
        if cfg!(test) || llrm_support::env_flag!("LLRM_CHECK_SHAPE") {
            // Loops of equal size are disjoint, so the order the layout gives
            // them (it moves as blocks are copied) does not matter: only which
            // loops there are.
            let by_size = |loops: &[Loop]| {
                let mut sorted = loops.to_vec();
                sorted.sort_by_key(|one| (one.body.len(), one.header));
                sorted
            };
            let fresh = cfg::Shape::of(function).loops;
            assert!(
                self.loops.as_deref().map(by_size) == Some(by_size(&fresh)),
                "the loops held through a rotation are not those of the function as it stands"
            );
        }
    }
}

/// A loop this can enter at its body: `preheader` jumps only to `header`,
/// which tests and branches to `first`, reached from nowhere else; the one
/// latch goes only back to `header`.
pub(crate) struct Shape {
    pub preheader: BlockId,
    pub header: BlockId,
    pub first: BlockId,
    pub latch: BlockId,
    pub exit: BlockId,
    /// The loop's blocks but its header: where a moved phi is read.
    pub body: BTreeSet<BlockId>,
    /// What the header computes that blocks after it read: each is also made by
    /// the copy of the test before the loop, and what it reaches reads the
    /// copy's on entry and the header's after.
    pub carried: Vec<InstId>,
}

fn _phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

/// Whether an instruction may be skipped on entry: it computes, and nothing
/// outside `header` reads it.
fn _test_only(
    function: &Function,
    header: BlockId,
    inst: InstId,
    divides: bool,
    carried: bool,
) -> bool {
    let op = function.instruction(inst);
    let computes = match op.opcode {
        Opcode::ICmp(_)
        | Opcode::FCmp(_)
        | Opcode::Cast(_)
        | Opcode::GetElementPtr { .. }
        | Opcode::Select
        | Opcode::Freeze => true,
        // A division may trap, and a loop entered past its test never traps
        // there: not where the test is not also copied before the loop.
        Opcode::Binary(kind) => {
            divides || !matches!(
                kind,
                BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem
            )
        }
        _ => false,
    };
    // A header phi reading it takes it round the back edge: it is the step of a
    // counter, which must run before the trip it starts, not skip the
    // first. Another block may read it where the copy of the test makes it
    // before the loop too.
    computes
        && op.result.is_none_or(|value| {
            function
                .users(value)
                .iter()
                .all(
                    |one| {
                        let inside = function.parent(one.user) == Some(header);
                        (inside || carried) && !(inside && function.instruction(one.user).opcode == Opcode::Phi)
                    },
                )
        })
}

pub(crate) fn _shape(
    function: &Function,
    loop_: &Loop,
) -> Option<Shape> {
    _shape_of(function, loop_, false, false)
}

/// `_shape`; `chained`: the loop is the rest of one already entered behind the
/// copy of an earlier test, whose latch is that test, a branch to this header
/// or out. `copied`: its test is copied before it, so that skipping it on entry
/// skips no trap.
fn _shape_of(
    function: &Function,
    loop_: &Loop,
    chained: bool,
    copied: bool,
) -> Option<Shape> {
    let succ = |at: i64| cfg::successors_of(function, at);
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    let [preheader] = loop_.entering(function).into_iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    let header = cfg::block(loop_.header);
    let terminator = |at: i64| function.terminator(cfg::block(at)).map(|last| function.instruction(last));
    if succ(preheader) != [loop_.header]
        || terminator(preheader).is_none_or(|last| last.opcode != Opcode::Br || last.operands.len() != 1)
    {
        return None;
    }
    let to_header = if chained { succ(latch).contains(&loop_.header) } else { succ(latch) == [loop_.header] };
    if latch == loop_.header || !to_header {
        return None;
    }
    let branch = terminator(loop_.header)?;
    if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
        return None;
    }
    let (inside, outside): (Vec<i64>, Vec<i64>) =
        succ(loop_.header).into_iter().partition(|at| loop_.body.contains(at));
    let ([first], [exit]) = (&inside[..], &outside[..]) else {
        return None;
    };
    if cfg::predecessors_of(function, *first) != BTreeSet::from([loop_.header])
        || !_phis(function, cfg::block(*first)).is_empty()
    {
        return None;
    }
    let work = function
        .iter_block(header)
        .filter(
            |&inst| {
                let op = function.instruction(inst);
                op.opcode != Opcode::Phi && !op.opcode.is_terminator()
            },
        );
    let work: Vec<InstId> = work.collect();
    if !work.iter().all(|&inst| _test_only(function, header, inst, copied, copied)) {
        return None;
    }
    let block = cfg::block;
    let body = loop_.body.iter().copied().filter(|&at| at != loop_.header).map(block).collect();
    let carried =
        work.iter()
            .copied()
            .filter(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(
                        |value| function.users(value).iter().any(|one| function.parent(one.user) != Some(header)),
                    )
            })
            .collect();
    Some(Shape {
        preheader: block(preheader),
        header,
        first: block(*first),
        latch: block(latch),
        exit: block(*exit),
        body,
        carried,
    })
}

/// `shape`'s loop entered at `first`, or, with `guard`, entered there only
/// where the guard sends it to `first` and skipped to the exit otherwise
/// (`true`: the guard is true on the way out).
pub(crate) fn _rotate(
    context: &Context,
    function: &mut Function,
    shape: &Shape,
    guard: Option<(Operand, bool)>,
    copies: &[(llrm_mir::module::ValueId, Operand)],
) -> Result<(), String> {
    let entering = function.terminator(shape.preheader).expect("a terminated preheader");
    if let Some((guard, exits_on_true)) = guard {
        // The exit is now also reached before the loop: each of its phis reads
        // there what it read from the header, which the updater below makes
        // the value from before the loop.
        for phi in _phis(function, shape.exit) {
            let mut incoming = arms(function, phi);
            if incoming.iter().any(|&(_, from)| from == shape.preheader) {
                continue;
            }
            let (value, _) = *incoming.iter().find(|&&(_, from)| from == shape.header).expect("an arm from the header");
            incoming.push((value, shape.preheader));
            function.set_operands(phi, from_arms(&incoming));
        }
        let (taken, other) = if exits_on_true { (shape.exit, shape.first) } else { (shape.first, shape.exit) };
        function.set_operands(entering, vec![guard, Operand::Block(taken), Operand::Block(other)]);
    } else {
        function.set_operands(entering, vec![Operand::Block(shape.first)]);
    }
    let phis = _phis(function, shape.header);
    let values = phis.iter().map(|&phi| function.instruction(phi).result.expect("a phi's value")).collect::<Vec<_>>();
    let side = |function: &Function, phi: InstId, from: BlockId| {
        arms(function, phi).into_iter().find(|&(_, source)| source == from).expect("a preheader and a latch arm").0
    };
    let starts = phis.iter().map(|&phi| side(function, phi, shape.preheader)).collect::<Vec<_>>();
    let nexts = phis.iter().map(|&phi| side(function, phi, shape.latch)).collect::<Vec<_>>();
    for &phi in &phis {
        function.set_operands(phi, Vec::new());
    }
    // Each header phi `p(preheader: a, latch: b)` moves to `first` as
    // `p'(preheader: a, header: b')`, a `b` that is another header phi read
    // as its moved `b'`. Phis read in parallel, so a swap needs no order.
    let moved = values
        .iter()
        .map(|&value| {
            let phi = function.create_instruction(
                Opcode::Phi,
                function.value(value).ty,
                Vec::new(),
                Flags::default(),
                function.value(value).name.clone().as_deref(),
            );
            let top = function.block(shape.first).first();
            function.insert(phi, top.map_or(Position::End(shape.first), Position::Before)).expect("a placed block");
            (phi, Operand::Value(function.instruction(phi).result.expect("a phi's value")))
        })
        .collect::<Vec<_>>();
    let latest = |read: Operand| match read {
        Operand::Value(value) => values.iter().position(|&one| one == value).map_or(read, |index| moved[index].1),
        other => other,
    };
    for (index, &(phi, _)) in moved.iter().enumerate() {
        function.set_operands(
            phi,
            vec![starts[index], Operand::Block(shape.preheader), latest(nexts[index]), Operand::Block(shape.header)],
        );
    }
    // The body reads the moved phi; the header and what follows the loop
    // read `a` as it left the preheader and `b'` as it left the latch.
    for (index, &phi) in phis.iter().enumerate() {
        let value = values[index];
        let mut updater = SsaUpdater::new(function.value(value).ty, function.value(value).name.clone().as_deref());
        updater.add_available_value(shape.preheader, starts[index]);
        updater.add_available_value(shape.latch, latest(nexts[index]));
        for one in function.users(value).snapshot() {
            let user = function.instruction(one.user);
            let at = match (user.opcode == Opcode::Phi).then(|| user.operands[one.index as usize + 1]) {
                Some(Operand::Block(from)) => from,
                _ => function.parent(one.user).expect("a placed user"),
            };
            if shape.body.contains(&at) {
                function.set_operand(one.user, one.index as usize, moved[index].1);
            } else {
                updater.rewrite_use(context, function, one);
            }
        }
        function.erase(phi)?;
    }
    // What the header computes that others read: the copy's value before the
    // loop, the header's after it.
    for &inst in &shape.carried {
        let Some(value) = function.instruction(inst).result else { continue };
        let Some(&(_, copy)) = copies.iter().find(|(old, _)| *old == value) else { continue };
        let mut updater = SsaUpdater::new(function.value(value).ty, function.value(value).name.clone().as_deref());
        updater.add_available_value(shape.preheader, copy);
        updater.add_available_value(shape.header, Operand::Value(value));
        for one in function.users(value).snapshot() {
            if function.parent(one.user) != Some(shape.header) {
                updater.rewrite_use(context, function, one);
            }
        }
    }
    Ok(())
}

/// The first proven loop not in `done` entered at its body; whether one was.
fn rotated(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    forest: &mut Forest,
    done: &mut llrm_mir::dense::IdSet<BlockId>,
) -> Result<bool, String> {
    let mut fresh = analyses.fresh();
    let found = forest.shape(context, layout, function, &mut fresh, analyses);
    let mut scevs = None;
    for loop_ in found.loops.iter() {
        if done.contains(&cfg::block(loop_.header)) {
            continue;
        }
        let Some(shape) = _shape(function, &loop_) else {
            continue;
        };
        // Asked of the first loop that could be rotated: a body with none
        // derives nothing.
        let session = scevs.get_or_insert_with(|| {
            ScevSession::within(context, &analyses.outer().globals, &analyses.outer().metadata, function)
        });
        let unit = memory::Unit::within(context, layout, function, analyses.outer()).with_shape(&found);
        let proven = crate::profit::scev_trips(session, &unit, &loop_).is_some();
        if !proven {
            continue;
        }
        _rotate(context, function, &shape, None, &[])?;
        done.insert(shape.first);
        drop(fresh);
        forest.keep(found);
        forest.rotated(function, cfg::id(shape.header), cfg::id(shape.first), cfg::id(shape.header));
        analyses.invalidate(function, &PreservedAnalyses::none());
        return Ok(true);
    }
    drop(fresh);
    forest.keep(found);
    Ok(false)
}

/// `LLRM_DEBUG=census`: per loop, what the trip-count analyses prove of it, as
/// the last loop pass finds it. Comparing a build with and without `-ftree-ch`
/// shows a loop whose symbolic count the copy lost.
fn census(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    id: Option<llrm_mir::context::GlobalId>,
    analyses: &Analyses,
) {
    let mut fresh = analyses.fresh();
    let found = fresh.get::<cfg::Shape>(context, layout, function);
    let unit = memory::Unit::within(context, layout, function, analyses.outer()).with_shape(&found);
    let mut scevs = ScevSession::within(context, &analyses.outer().globals, &analyses.outer().metadata, function);
    let name = format!("{id:?}");
    let (mut proofs, mut constant, mut symbolic) = (0, 0, 0);
    for loop_ in found.loops.iter() {
        let found = induction::counted(&unit, &loop_, None, false);
        proofs += usize::from(!found.is_empty());
        constant += usize::from(crate::profit::scev_trips(&mut scevs, &unit, &loop_).is_some());
        symbolic += usize::from(found.iter().any(|proof| {
            induction::trips(proof, &mut |_, _| induction::AffineOperand::constant(0, proof.width())).is_some()
        }));
    }
    llrm_support::debug!(
        "census",
        "{name}: loops {} proved {proofs} constant {constant} symbolic {symbolic}",
        found.loops.len()
    );
}

/// gcc's `param_max_loop_header_insns`: the statements a copied header may
/// have.
const MAX_HEADER_INSNS: usize = 20;

/// The first loop not in `done` whose test is copied before it, entered at its
/// body behind the copy; whether one was (`copy_headers`,
/// `should_duplicate_loop_header_p`: a header that ends in the exit test,
/// within the limit, calling nothing).
fn copied(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    analyses: &mut Analyses,
    forest: &mut Forest,
    done: &mut llrm_mir::dense::IdSet<BlockId>,
) -> Result<bool, String> {
    let mut fresh = analyses.fresh();
    let found = forest.shape(context, layout, function, &mut fresh, analyses);
    let mut scevs = None;
    for loop_ in found.loops.iter() {
        if done.contains(&cfg::block(loop_.header)) {
            continue;
        }
        let Some(shape) = _shape_of(function, &loop_, false, true) else {
            continue;
        };
        let insns =
            function.iter_block(shape.header).filter(|&inst| function.instruction(inst).opcode != Opcode::Phi).count();
        if insns > MAX_HEADER_INSNS {
            continue;
        }
        // A loop proven to run needs no guard: it is entered at its body once
        // the loop passes are done (`rotated`, which unroll, peel and
        // fill wait for), as gcc's value propagation leaves it after its copy.
        let session = scevs.get_or_insert_with(|| {
            ScevSession::within(context, &analyses.outer().globals, &analyses.outer().metadata, function)
        });
        let unit = memory::Unit::within(context, layout, function, analyses.outer()).with_shape(&found);
        if crate::profit::scev_trips(session, &unit, &loop_).is_some() {
            continue;
        }
        let Some((guard, copies)) = _copy_test(function, &shape) else {
            continue;
        };
        let weights = function
            .instruction(function.terminator(shape.header).expect("a terminated header"))
            .metadata
            .iter()
            .find(|(kind, _)| kind == "prof")
            .map(|(_, node)| *node);
        _rotate(context, function, &shape, Some(guard), &copies)?;
        _weighed(context, declared, function, &shape, weights, guard.1);
        drop(fresh);
        forest.keep(found);
        forest.rotated(function, cfg::id(shape.header), cfg::id(shape.first), cfg::id(shape.header));
        analyses.invalidate(function, &PreservedAnalyses::none());
        // Each following test that leaves the loop is copied too while the
        // budget lasts (`while (a && b)`): gcc's chain of headers.
        let mut first = shape.first;
        let mut remaining = MAX_HEADER_INSNS - insns;
        while let Some((next, spent)) =
            _chained(context, layout, function, declared, analyses, forest, first, remaining)?
        {
            first = next;
            remaining -= spent;
        }
        done.insert(first);
        return Ok(true);
    }
    drop(fresh);
    forest.keep(found);
    Ok(false)
}

/// The loop entered at `first` copied once more: its header, which tests and
/// leaves the loop, has its test copied behind the one before, entering at
/// the block it goes on to (gcc's `copy_headers` chain, each header after the
/// first with one predecessor and no call, all within
/// `param_max_loop_header_insns`). The body block now entered and the
/// instructions the copy spent, or none.
fn _chained(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    analyses: &mut Analyses,
    forest: &mut Forest,
    first: BlockId,
    remaining: usize,
) -> Result<Option<(BlockId, usize)>, String> {
    let preheader = cfg::predecessors_of(function, cfg::id(first)).into_iter().find(|&at| {
        function.terminator(cfg::block(at)).is_some_and(|last| function.instruction(last).operands.len() == 3)
    });
    let Some(preheader) = preheader else { return Ok(None) };
    let mut fresh = analyses.fresh();
    let found = forest.shape(context, layout, function, &mut fresh, analyses);
    let loop_ = found.loops.iter().find(|one| one.header == cfg::id(first)).cloned();
    drop(fresh);
    forest.keep(found);
    let Some(loop_) = loop_ else { return Ok(None) };
    // The loop must be entered by the guard alone, over an edge of its own.
    if loop_.entering(function) != BTreeSet::from([preheader]) {
        return Ok(None);
    }
    let insns = function.iter_block(first).filter(|&inst| function.instruction(inst).opcode != Opcode::Phi).count();
    if insns > remaining {
        return Ok(None);
    }
    // Only a header that tests and leaves is copied: the others end the chain
    // before an edge is split for them.
    let leaves = function.terminator(first).is_some_and(|last| {
        let last = function.instruction(last);
        last.opcode == Opcode::Br
            && last.operands.len() == 3
            && cfg::successors_of(function, cfg::id(first)).iter().filter(|at| loop_.body.contains(at)).count() == 1
    });
    if !leaves {
        return Ok(None);
    }
    let bridge = crate::edges::split(function, cfg::block(preheader), first, Vec::new())?;
    // The loop is as it was and a block stands on its way in, in the loops
    // around.
    forest.split(function, cfg::id(bridge), preheader, cfg::id(first));
    let Some(shape) = _shape_of(function, &loop_, true, true).filter(|shape| shape.preheader == bridge) else {
        forest.check(function);
        return Ok(None);
    };
    let Some((guard, copies)) = _copy_test(function, &shape) else {
        forest.check(function);
        return Ok(None);
    };
    let weights = function
        .instruction(function.terminator(shape.header).expect("a terminated header"))
        .metadata
        .iter()
        .find(|(kind, _)| kind == "prof")
        .map(|(_, node)| *node);
    _rotate(context, function, &shape, Some(guard), &copies)?;
    _weighed(context, declared, function, &shape, weights, guard.1);
    forest.rotated(function, cfg::id(shape.header), cfg::id(shape.first), cfg::id(shape.header));
    analyses.invalidate(function, &PreservedAnalyses::none());
    Ok(Some((shape.first, insns)))
}

/// The guard the copy left in the preheader is taken as the header's test is:
/// the weights it declares, else the loop heuristic's (`branchprob::LOOP`)
/// for staying in against leaving. gcc's header copying duplicates the test's
/// edge probabilities and LLVM's `LoopRotate` puts its weights on the guard
/// (`updateBranchWeights`); left to be guessed again from structure, the
/// guard outside the loop is a branch like any other and the way into the
/// loop is laid out as the unlikely one.
fn _weighed(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    shape: &Shape,
    weights: Option<llrm_mir::module::MetadataId>,
    exits_on_true: bool,
) {
    let guard = function.terminator(shape.preheader).expect("a terminated preheader");
    let node = weights.unwrap_or_else(|| {
        let word = context.types.int(32);
        let weight = |context: &mut Context, n: f64| MetadataOperand::Constant(context.int(word, n as i128));
        let (staying, leaving) = llrm_analysis::branchprob::LOOP;
        let (first, second) = if exits_on_true { (leaving, staying) } else { (staying, leaving) };
        declared.node(MetadataNode {
            distinct: false,
            operands: vec![
                MetadataOperand::String("branch_weights".to_owned()),
                weight(context, first),
                weight(context, second),
            ],
        })
    });
    function.annotate(guard, "prof", node);
}

/// The header's test computed in the preheader, on what the header's phis read
/// from it: the guard, and whether it is true on the way out. The header keeps
/// its own for the trips that come round.
fn _copy_test(
    function: &mut Function,
    shape: &Shape,
) -> Option<((Operand, bool), Vec<(llrm_mir::module::ValueId, Operand)>)> {
    let branch = function.terminator(shape.header).expect("a terminated header");
    let [Operand::Value(cond), Operand::Block(taken), _] = function.instruction(branch).operands[..] else {
        return None;
    };
    let mut known: Vec<(llrm_mir::module::ValueId, Operand)> = Vec::new();
    for phi in _phis(function, shape.header) {
        let value = function.instruction(phi).result.expect("a phi's value");
        let start = arms(function, phi).into_iter().find(|&(_, from)| from == shape.preheader)?.0;
        known.push((value, start));
    }
    let phis_known = known.len();
    let entering = function.terminator(shape.preheader).expect("a terminated preheader");
    let work: Vec<InstId> = function
        .iter_block(shape.header)
        .filter(|&inst| {
            function.instruction(inst).opcode != Opcode::Phi && !function.instruction(inst).opcode.is_terminator()
        })
        .collect();
    for inst in work {
        let copy = function.clone_instruction(inst);
        for index in 0..function.instruction(copy).operands.len() {
            if let Operand::Value(value) = function.instruction(copy).operands[index] {
                if let Some((_, now)) = known.iter().find(|(old, _)| *old == value) {
                    function.set_operand(copy, index, *now);
                }
            }
        }
        function.insert(copy, Position::Before(entering)).expect("a placed preheader");
        if let (Some(old), Some(new)) = (function.instruction(inst).result, function.instruction(copy).result) {
            known.push((old, Operand::Value(new)));
        }
    }
    let guard = known.iter().find(|(old, _)| *old == cond)?.1;
    // The copies of the work (the phis' starts stay out).
    let copies = known.iter().skip(phis_known).copied().collect();
    Some(((guard, taken == shape.exit), copies))
}

#[cfg(test)]
#[path = "rotate_tests.rs"]
mod tests;
