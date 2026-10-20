//! Threads a path through a branch or switch that the way into it decides: the
//! blocks between the deciding value and the test are copied for that path,
//! which jumps straight to its target and does not run the test. LLVM's
//! `DFAJumpThreading` and gcc's backward (FSM) threader
//! (`tree-ssa-threadbackward.cc`).
//!
//! # How it works
//! `state = step(state); switch (state)` in a loop: the switch reads a phi of
//! the loop's header, and the value that comes round is a constant on each path
//! into the join that makes it (or constant from the preheader). Each such path
//! gets its own copy of the blocks from the join down to the switch, and the
//! copy of the switch is a jump to the case its constant picks, so the dispatch
//! is gone from that path.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   p1["<b>P1</b>"]:::mid
//!   p2["<b>P2</b>"]:::mid
//!   j["<b>J</b>"]:::mid
//!   h["<b>H (phi s)</b>"]:::mid
//!   s["<b>S</b>"]:::mid
//!   cs["<b>cases</b>"]:::mid
//!   p1 --> j
//!   p2 --> j
//!   j --> h
//!   h --> s
//!   s --> cs
//! ```
//!
//! *Before*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   p1["<b>P1</b>"]:::mid
//!   p2["<b>P2</b>"]:::mid
//!   j1["<b>J1</b>"]:::mid
//!   j2["<b>J2</b>"]:::mid
//!   h1["<b>H1</b>"]:::mid
//!   h2["<b>H2</b>"]:::mid
//!   c1["<b>case 1</b>"]:::mid
//!   c2["<b>case 2</b>"]:::mid
//!   p1 --> j1
//!   p2 --> j2
//!   j1 --> h1
//!   j2 --> h2
//!   h1 --> c1
//!   h2 --> c2
//! ```
//!
//! *After (J, H, S stay for other paths)*
//!
//! A candidate is a `switch` on a phi in a block `S` of a loop (`p` in `H`),
//! `H` reaching `S` through blocks each with the one before as its only
//! predecessor, and `p`'s input from the loop being a constant, or a phi in its
//! own block `J` (ending in a jump to `H`) with a constant from the predecessor
//! `P`. The copies of `J..S` take `H`'s phis as the values they have on the
//! path. A value made in the originals and read outside them is made again by
//! the copies, and an `SsaUpdater` joins the two where the paths meet. The
//! result is not a natural loop where several cases lead back; later loop
//! passes skip it.
//!
//! A conditional branch is threaded the same way where it tests an `i1` phi, or
//! a compare of a phi with a constant, whatever the loop. Every
//! such branch of the function as it came is threaded, within the budget of
//! copies (a branch the copies made is not, which would peel a loop a trip at a
//! time, and a block a round threaded through is left to the next rounds): the
//! peeled `place` of queens at -O3 tested a constant 5 times in 8 after the
//! first branch alone was threaded. With `correlated`, a
//! branch whose compare the way into its block already settles, from a branch
//! on a compare of one of its operands above (`guards::on_edge`), is threaded
//! to its arm on each way: the branches a header copy leaves. A path into a
//! loop header from outside is not threaded; that is a rotation, `Rotate`'s. A
//! block is not copied if it holds a landing pad, a call that unwinds to a
//! handler, a stack object, or a call that may not be duplicated.
//!
//! With `correlated` and `states`, a loop whose header branches on a value the
//! loop does not change, its arms rejoining before the latch, is first split
//! into a loop for each arm (`split_arms`).
//!
//! Copies cost code, so limits follow gcc's: a switch path may copy at most
//! `MAX_PATH` (100) instructions, sized as gcc counts a path
//! (`size_weighted`); a branch path at most `(BRANCH_PATH - 1) / PATH_SCALE`
//! (7) instructions plus the phis of a block with several predecessors and
//! successors, the compare the threading kills not counted; the whole function
//! at most `MAX_COPIED` (400). Paths are found on the function as it came, so a
//! branch the copies make is not threaded again. Tuned for size no path with
//! copies is taken; a branch whose copies hold nothing still is, as gcc does.
//!
//! Where anything was copied, `fold`, `algebraic`, `decide` and `dead` run once
//! over the function, since each copy knows its state.
//!
//! # Example
//! The branch case, on `r = phi(1, 0); if (r != 0)` after an inlined call.
//! Before:
//!
//! ```mir
//! ; ... b0, head (the loop header, branching to test or done)
//! test:
//!   %c = icmp sgt i16 %k, %i
//!   br i1 %c, label %yes, label %no
//!
//! yes:
//!   br label %join
//!
//! no:
//!   br label %join
//!
//! join:
//!   %r = phi i16 [ 1, %yes ], [ 0, %no ]
//!   %t = icmp ne i16 %r, 0
//!   br i1 %t, label %hit, label %step
//!
//! hit:
//!   %h = add i16 %acc, %i
//!   br label %step
//!
//! step:
//!   %acc1 = phi i16 [ %acc, %join ], [ %h, %hit ]
//!   %i1 = add nsw i16 %i, 1
//!   br label %head
//! ```
//!
//! After:
//!
//! ```mir
//! test:
//!   %c = icmp sgt i16 %k, %i
//!   br i1 %c, label %yes, label %no
//!
//! yes:
//!   %h = add i16 %acc, %i
//!   br label %step
//!
//! no:
//!   br label %step
//!
//! step:
//!   %acc1 = phi i16 [ %h, %yes ], [ %acc, %no ]
//!   %i1 = add nsw i16 %i, 1
//!   br label %head
//! ```
//!
//! `%r` is 1 on `yes` and 0 on `no`, so each path decides `%t` itself: `yes`
//! runs `hit`'s work and `no` skips it, and `join` and its compare are gone.
//!
//! From unit test
//! `test_a_branch_on_a_compare_of_a_phi_of_constants_is_decided_on_each_path`
//! in `jumpthread_tests.rs`.
//!
//! # Runs
//! After the loop passes and `rotate`, before the last `hoist`. Switch state
//! machines (`states`) follow `-fexpensive-optimizations` (-O2 and up); branch
//! threading by correlation (`correlated`) follows `-ftree-ch`, which is off by
//! default. `for_size` is -Os and -Oz.
//!
//! # Needs
//! `cfg::Shape`, `guards`, `SsaUpdater`, [`crate::fold`], [`crate::algebraic`],
//! [`crate::decide`], [`crate::dead`].
//!
//! # Keeps
//! Nothing; blocks and edges change.
//!
//! # See also
//! LLVM `DFAJumpThreading.cpp`; gcc `tree-ssa-threadbackward.cc`,
//! `tree-ssa-threadupdate.cc`. Tests: `jumpthread_tests.rs`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};

use crate::lcssa::arms;

/// GCC's `max-fsm-thread-path-insns`: instructions one path's copies may hold.
const MAX_PATH: usize = 100;
/// GCC's `max-jump-thread-duplication-stmts` (params.opt:589): instructions a
/// branch's path may copy besides the phis and the compare the threading kills.
/// Tuned for size, GCC threads only where it kills every statement of the block
/// (`tree-ssa-threadupdate.cc:2077`): none.
const BRANCH_PATH: usize = 15;
/// GCC's `fsm-scale-path-stmts` (params.opt:165): `profitable_path_p` rejects a
/// path of `n` instructions where `n * 2 >= BRANCH_PATH`.
const PATH_SCALE: usize = 2;
/// GCC's `max-fsm-thread-paths`, in instructions rather than paths: what one
/// function may copy in all.
const MAX_COPIED: usize = 400;

/// `size`: code size outranks speed, and a path is never copied.
pub struct JumpThread {
    pub size: bool,
    /// A branch the way into its block settles is threaded to its arm there:
    /// the guards a header copy leaves.
    pub correlated: bool,
    /// A switch the paths into it decide is threaded: gcc's
    /// `pass_thread_jumps`, gated on `flag_expensive_optimizations` (-O2
    /// and up; `tree-ssa-threadbackward.cc:1072`), where -O1 has only its
    /// early, size-optimising threader.
    pub states: bool,
}

impl FunctionPass for JumpThread {
    fn name(&self) -> &'static str {
        "jumpthread"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if !threaded(unit.context, unit.layout, unit.function, &outer, self.size, self.correlated, self.states) {
            return PreservedAnalyses::all();
        }
        llrm_mir::passes::check_phi_arity("jumpthread (threading)", unit.function);
        // What the copies know of their state: a product by a constant, a
        // compare of one. Only where something was copied.
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
        for pass in [
            &mut crate::fold::Fold as &mut dyn FunctionPass,
            &mut crate::algebraic::Algebraic { size: false, llvm: false },
            &mut crate::decide::Decide,
            &mut crate::dead::Dead,
        ] {
            let kept = llrm_mir::passes::run_checked(pass, unit, analyses);
            analyses.invalidate(unit.function, &kept);
        }
        PreservedAnalyses::none()
    }
}

/// One path found: the block its first edge leaves, the blocks copied, and the
/// case it ends in.
struct Path {
    from: BlockId,
    blocks: Vec<BlockId>,
    target: BlockId,
}

/// Every switch's paths that can be threaded, a switch's all at once within the
/// limits; whether any was.
pub fn threaded(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    size: bool,
    correlate: bool,
    states: bool,
) -> bool {
    // The common function has nothing to thread: no analysis is made for it.
    let unit = memory::Unit::within(context, layout, function, outer);
    let any = function
        .walk()
        .any(
            |(_, inst)| function.instruction(inst).opcode.is_terminator()
                && decided(function, &unit, inst).is_some_and(|(state, _)| {
                    matches!(
                        function.value(state).def,
                        ValueDef::Instruction(phi) if function.instruction(phi).opcode == Opcode::Phi
                    )
                }),
        );
    // Or a branch on a compare, at a join of ways that may each settle it.
    let joined = !size
        && correlate
        && function.walk().any(|(block, inst)| {
            let op = function.instruction(inst);
            op.opcode == Opcode::Br && op.operands.len() == 3 && function.predecessors(block).len() > 1 && matches!(
                op.operands[0],
                Operand::Value(c) if matches!(function.value(c).def, ValueDef::Instruction(made) if matches!(function.instruction(made).opcode, Opcode::ICmp(_)) && compared_again(function, block, c))
            )
        });
    let mut copied = 0;
    let mut changed = false;
    // A loop whose header branches on an invariant, its arms rejoining before
    // the latch: split into a loop for each arm.
    if correlate && states && !size && split_arms(context, function, &mut copied) {
        changed = true;
    }
    if !any && !joined {
        if changed {
            cfg::_unreachable(context, function);
        }
        return changed;
    }
    // The paths of the branches are those of the function as it came: a branch
    // the copies made is not threaded again, or a loop would be peeled one
    // trip at a time. GCC registers its threads once and applies them together.
    let originals: IdSet<BlockId> = function.layout().iter().copied().collect();
    // The blocks a round threaded through, which later rounds leave: their
    // phis and edges are those of the paths that were made.
    let mut touched: IdSet<BlockId> = IdSet::new();
    while let Some(paths) =
        found(context, layout, function, outer, copied, size, &originals, &touched, correlate, states)
    {
        llrm_support::debug!("jumpthread", "{} paths, {} instructions copied so far", paths.len(), copied);
        for path in &paths {
            touched.insert(path.from);
            touched.insert(path.target);
            touched.extend(path.blocks.iter().copied());
        }
        // A path of phis and a branch copies no instruction and still spends
        // the budget: or a loop of them never ends.
        copied += copy(context, function, &paths).max(paths.len());
        changed = true;
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

/// The paths of the first switch or branch that has any, within what is left of
/// `MAX_COPIED`.
fn found(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: &Outer,
    copied: usize,
    size: bool,
    originals: &IdSet<BlockId>,
    touched: &IdSet<BlockId>,
    correlate: bool,
    states: bool,
) -> Option<Vec<Path>> {
    let shape = cfg::Shape::of(function);
    let unit = memory::Unit::within(context, layout, function, outer).with_shape(&shape);
    let blocks: Vec<BlockId> = function
        .walk()
        .filter(|&(_, inst)| function.instruction(inst).opcode.is_terminator())
        .map(|(block, _)| block)
        .collect();
    for block in blocks {
        let Some(last) = function.terminator(block) else { continue };
        // A branch the copies made is not threaded again, which would peel a
        // loop one trip at a time; each branch of the function as it came is.
        let branches = originals.contains(&block) && !touched.contains(&block);
        // The innermost loop holding the block, the one a chain of blocks to
        // the state's phi stays within.
        let loop_ =
            shape.loops.iter().filter(|one| one.body.contains(&cfg::id(block))).min_by_key(|one| one.body.len());
        // A branch on a compare that the way into the block settles, from the
        // branches above that way: threaded to its arm there.
        if branches && correlate && size == false && function.instruction(last).opcode == Opcode::Br {
            let ways = match function.instruction(last).operands.first() {
                Some(&Operand::Value(condition)) if compared_again(function, block, condition) => {
                    correlated(function, &unit, block, last)
                }
                _ => Vec::new(),
            };
            let mut paths = Vec::new();
            let mut spent = copied;
            for (source, target) in ways {
                if touched.contains(&source)
                    || source == block
                    || !copyable(function, block)
                    || shape.loops.iter().any(|l| l.header == cfg::id(block) && !l.body.contains(&cfg::id(source)))
                {
                    continue;
                }
                let killed_here = matches!(
                    function.instruction(last).operands.first(),
                    Some(&Operand::Value(c)) if matches!(function.value(c).def, ValueDef::Instruction(made) if function.parent(made) == Some(block) && matches!(function.instruction(made).opcode, Opcode::ICmp(_)))
                );
                let copies = function
                    .iter_block(block)
                    .filter(|&inst| {
                        function.instruction(inst).opcode != Opcode::Phi
                            && !function.instruction(inst).opcode.is_terminator()
                    })
                    .count()
                    .saturating_sub(usize::from(killed_here));
                let phis = if function.predecessors(block).len() > 1 && function.successors(block).len() > 1 {
                    function.iter_block(block).filter(|&inst| function.instruction(inst).opcode == Opcode::Phi).count()
                } else {
                    0
                };
                let copies = copies + phis;
                if copies > (BRANCH_PATH - 1) / PATH_SCALE || spent + copies > MAX_COPIED {
                    continue;
                }
                spent += copies.max(1);
                paths.push(Path { from: source, blocks: vec![block], target });
            }
            if !paths.is_empty() {
                return Some(paths);
            }
        }
        // A switch is the state machine of a loop; a branch on a constant a
        // path decides is threaded wherever it is.
        let Some((state, decide)) = decided(function, &unit, last) else { continue };
        let is_switch = function.instruction(last).opcode == Opcode::Switch;
        if is_switch && (size || !states || loop_.is_none()) || !is_switch && !branches {
            continue;
        }
        let Some(chain) = chain_to_phi(function, loop_, block, state) else { continue };
        let ValueDef::Instruction(phi) = function.value(state).def else { continue };
        let limit = match (is_switch, size) {
            (true, _) => MAX_PATH,
            (false, false) => (BRANCH_PATH - 1) / PATH_SCALE,
            (false, true) => 0,
        };
        // The compare of the branch is decided by the threading, and is not a
        // copy.
        let killed = match function.instruction(last).operands.first() {
            Some(&Operand::Value(condition)) if !is_switch => match function.value(condition).def {
                ValueDef::Instruction(made) if matches!(function.instruction(made).opcode, Opcode::ICmp(_)) => {
                    Some(made)
                }
                _ => None,
            },
            _ => None,
        };
        let mut paths = Vec::new();
        let mut spent = copied;
        let mut seen = IdSet::new();
        for (value, from) in arms(function, phi) {
            // A switch with two cases to one block gives its phi two inputs
            // from the same predecessor: one way in, as LLVM's
            // `UniqueBlocks` takes it
            // (DFAJumpThreading.cpp#L618-L623 at d1106deb71cc).
            if !seen.insert(from) {
                continue;
            }
            // The constant on each way in, and the blocks copied ahead of the
            // chain.
            let ways = resolved(function, &unit, value, from, 4);
            for (source, prefix, number) in ways {
                let Some(target) = decide(number) else { continue };
                let blocks: Vec<BlockId> = prefix.into_iter().chain(chain.iter().copied()).collect();
                if touched.contains(&source) || blocks.iter().any(|one| touched.contains(one)) {
                    continue;
                }
                if blocks.contains(&source) {
                    continue;
                }
                if !blocks.iter().all(|&one| copyable(function, one)) {
                    continue;
                }
                // Into a loop through its header from outside is a rotation,
                // which `Rotate` makes where it pays: GCC
                // allows it only for the idioms of `thread_through_loop_header`
                // (tree-ssa-threadupdate.cc:1712), and not before the loop
                // passes are done.
                if !is_switch
                    && blocks.iter().any(|&one| {
                        shape.loops.iter().any(|l| l.header == cfg::id(one) && !l.body.contains(&cfg::id(source)))
                    })
                {
                    continue;
                }
                let copies: usize = if is_switch {
                    size_weighted(function, &blocks)
                } else {
                    // Besides the phis, the branch and its compare, what the
                    // copies hold.
                    blocks
                        .iter()
                        .flat_map(|&one| function.iter_block(one))
                        .filter(|&inst| {
                            function.instruction(inst).opcode != Opcode::Phi
                                && !function.instruction(inst).opcode.is_terminator()
                                && Some(inst) != killed
                        })
                        .count()
                };
                // GCC counts the phis of a block with several predecessors and
                // successors (`tree-ssa-threadbackward.cc`,
                // possibly_profitable_path_p): each is a phi at the
                // points where the copies rejoin the originals, and a move
                // there. The state's own phi dies.
                let phis: usize = if is_switch {
                    0
                } else {
                    blocks
                        .iter()
                        .filter(|&&one| function.predecessors(one).len() > 1 && function.successors(one).len() > 1)
                        .map(|&one| {
                            function
                                .iter_block(one)
                                .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi && inst != phi)
                                .count()
                        })
                        .sum()
                };
                llrm_support::debug!("jumpthread", "path from {:?}: {} copies, {} phis", source, copies, phis);
                let copies = copies + phis;
                if copies > limit || spent + copies > MAX_COPIED {
                    continue;
                }
                // A path of phis and a branch still costs a block, or the
                // budget would never run out.
                spent += copies.max(1);
                paths.push(Path { from: source, blocks, target });
            }
        }
        if !paths.is_empty() {
            return Some(paths);
        }
    }
    None
}

/// The size of `blocks` as GCC counts a threading path: `gcc_insns` over
/// them, less the switch the threading removes (`m_exit_jump_benefit`).
fn size_weighted(
    function: &Function,
    blocks: &[BlockId],
) -> usize {
    let insts = blocks.iter().flat_map(|&one| function.iter_block(one));
    (crate::inline::gcc_insns_in(function, insts) as usize).saturating_sub(1)
}

/// Whether the branch `at` on the compare `condition` has a branch above it, on
/// a compare of one of its operands: a branch the way in settles is one that
/// some branch on the way compared too. The compares of the operand are the
/// only ones asked, and each of their blocks must reach `at`.
fn compared_again(
    function: &Function,
    at: BlockId,
    condition: llrm_mir::module::ValueId,
) -> bool {
    let ValueDef::Instruction(made) = function.value(condition).def else { return false };
    let mut above = Vec::new();
    for operand in &function.instruction(made).operands {
        let Operand::Value(value) = operand else { continue };
        for one in function.users(*value) {
            if one.user == made || !matches!(function.instruction(one.user).opcode, Opcode::ICmp(_)) {
                continue;
            }
            let Some(result) = function.instruction(one.user).result else { continue };
            for branch in function.users(result) {
                if function.instruction(branch.user).opcode == Opcode::Br
                    && let Some(there) = function.parent(branch.user)
                    && there != at
                {
                    above.push(there);
                }
            }
        }
    }
    if above.is_empty() {
        return false;
    }
    // Reached from one of them by going back from `at`.
    let mut seen = std::collections::BTreeSet::new();
    let mut stack: Vec<BlockId> = function.predecessors(at).into();
    while let Some(block) = stack.pop() {
        if !seen.insert(block) {
            continue;
        }
        if above.contains(&block) {
            return true;
        }
        stack.extend(function.predecessors(block));
    }
    false
}

/// The predecessors of `block` from which the branch `last` goes one way for
/// certain, each with its arm: where the compare it branches on is settled by
/// what is known of the way in (`guards::on_edge`). A predecessor reaching it
/// by two edges is not one way.
fn correlated(
    function: &Function,
    unit: &memory::Unit,
    block: BlockId,
    last: InstId,
) -> Vec<(BlockId, BlockId)> {
    let operands = &function.instruction(last).operands;
    let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] = operands[..] else { return Vec::new() };
    if yes == no || function.predecessors(block).len() < 2 {
        return Vec::new();
    }
    // The compare it branches on was made above the way in, not by the block
    // itself: a compare the block makes is made again on each
    // trip, and what the way in knows of it is what the last trip's made.
    let ValueDef::Instruction(made) = function.value(condition).def else { return Vec::new() };
    let Some(home) = function.parent(made) else { return Vec::new() };
    if home == block {
        return Vec::new();
    }
    let shape = unit.shape();
    let mut ways = Vec::new();
    for source in function.predecessors(block) {
        if function.terminator(source).is_some_and(|end| {
            function.instruction(end).operands.iter().filter(|one| **one == Operand::Block(block)).count() != 1
        }) || !shape.dominance.dominates(unit.function, cfg::id(home), cfg::id(source))
        {
            continue;
        }
        let facts = llrm_analysis::guards::on_edge(unit, source, block);
        if let Some(answer) = llrm_analysis::guards::decides(unit, &facts, condition) {
            ways.push((source, if answer { yes } else { no }));
        }
    }
    ways
}

/// What the terminator `last` decides from a phi: the phi's value, and where
/// control goes for a constant of it. A switch on the phi; a branch on an `i1`
/// phi; a branch on a compare of the phi with a constant.
fn decided<'a>(
    function: &'a Function,
    unit: &'a memory::Unit,
    last: InstId,
) -> Option<(ValueId, Box<dyn Fn(u128) -> Option<BlockId> + 'a>)> {
    let terminator = function.instruction(last);
    let Operand::Value(condition) = *terminator.operands.first()? else { return None };
    match terminator.opcode {
        Opcode::Switch => Some((condition, Box::new(move |number| destination(function, unit, last, number)))),
        Opcode::Br if terminator.operands.len() == 3 => {
            let (Operand::Block(yes), Operand::Block(no)) = (terminator.operands[1], terminator.operands[2]) else {
                return None;
            };
            let ValueDef::Instruction(made) = function.value(condition).def else { return None };
            let compare = function.instruction(made);
            match compare.opcode {
                Opcode::Phi => Some((condition, Box::new(move |number| Some(if number & 1 == 1 { yes } else { no })))),
                Opcode::ICmp(predicate) => {
                    let Operand::Value(state) = compare.operands[0] else { return None };
                    let constant = unit.int_constant(compare.operands[1])?;
                    let bits = unit.int_bits(compare.operands[0])?;
                    Some((
                        state,
                        Box::new(move |number| Some(if holds(predicate, bits, number, constant) { yes } else { no })),
                    ))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// `left predicate right` for two integers of `bits` bits.
fn holds(
    predicate: IntPredicate,
    bits: u32,
    left: u128,
    right: u128,
) -> bool {
    let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
    let (left, right) = (left & mask, right & mask);
    let signed = |one: u128| if bits >= 128 { one as i128 } else { ((one << (128 - bits)) as i128) >> (128 - bits) };
    match predicate {
        IntPredicate::Eq => left == right,
        IntPredicate::Ne => left != right,
        IntPredicate::Ugt => left > right,
        IntPredicate::Uge => left >= right,
        IntPredicate::Ult => left < right,
        IntPredicate::Ule => left <= right,
        IntPredicate::Sgt => signed(left) > signed(right),
        IntPredicate::Sge => signed(left) >= signed(right),
        IntPredicate::Slt => signed(left) < signed(right),
        IntPredicate::Sle => signed(left) <= signed(right),
    }
}

/// Whether `block` may be copied: no call that unwinds to a handler (a second
/// landing pad is another handler), no landing pad, no stack object (a copy is
/// another frame object), no call that may not be duplicated.
fn copyable(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .iter_block(block)
        .all(
            |inst| {
                let instruction = function.instruction(inst);
                match &instruction.opcode {
                    Opcode::Invoke(_) | Opcode::LandingPad { .. } | Opcode::Resume | Opcode::Alloca { .. } => false,
                    Opcode::Call(info) => !llrm_mir::memory::has(&info.attrs, "noduplicate"),
                    _ => true,
                }
            },
        )
}

/// The ways `value` is a constant where it is read at the end of `from`: each
/// the block its way leaves, the blocks from there on that make it (phis, one
/// after another), and the constant.
fn resolved(
    function: &Function,
    unit: &memory::Unit,
    value: Operand,
    from: BlockId,
    depth: usize,
) -> Vec<(BlockId, Vec<BlockId>, u128)> {
    if let Some(number) = unit.int_constant(value) {
        return vec![(from, Vec::new(), number)];
    }
    let Operand::Value(made) = value else { return Vec::new() };
    let ValueDef::Instruction(phi) = function.value(made).def else { return Vec::new() };
    if depth == 0 || function.instruction(phi).opcode != Opcode::Phi || function.parent(phi) != Some(from) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (input, source) in arms(function, phi) {
        // Where the way leaves a block by two edges to `from`, it is not one
        // way.
        if function.terminator(source).is_some_and(|end| {
            function.instruction(end).operands.iter().filter(|one| **one == Operand::Block(from)).count() != 1
        }) {
            continue;
        }
        for (leaves, mut blocks, number) in resolved(function, unit, input, source, depth - 1) {
            blocks.push(from);
            out.push((leaves, blocks, number));
        }
    }
    out
}

/// The blocks from the one holding `state` as a phi to `block`, each the only
/// successor-by-predecessor of the one before; `None` where `state` is no phi
/// of a block of the loop that reaches `block` so.
fn chain_to_phi(
    function: &Function,
    loop_: Option<&Loop>,
    block: BlockId,
    state: ValueId,
) -> Option<Vec<BlockId>> {
    let ValueDef::Instruction(phi) = function.value(state).def else { return None };
    if function.instruction(phi).opcode != Opcode::Phi {
        return None;
    }
    let header = function.parent(phi)?;
    let mut chain = vec![block];
    while *chain.last()? != header {
        let last = *chain.last()?;
        let [only] = function.predecessors(last)[..] else { return None };
        if loop_.is_some_and(|one| !one.body.contains(&cfg::id(only))) || chain.contains(&only) || chain.len() > 8 {
            return None;
        }
        chain.push(only);
    }
    chain.reverse();
    Some(chain)
}

/// Where the switch `last` goes for `number`.
fn destination(
    function: &Function,
    unit: &memory::Unit,
    last: InstId,
    number: u128,
) -> Option<BlockId> {
    let switch = function.instruction(last);
    let Operand::Block(default) = switch.operands[1] else { return None };
    for pair in switch.operands[2..].chunks(2) {
        let [case, Operand::Block(target)] = pair else { return None };
        if unit.int_constant(*case)? == number {
            return Some(*target);
        }
    }
    Some(default)
}

/// What one path's copies are: its blocks, and the value each original value is
/// in them.
struct Copies {
    at: BTreeMap<BlockId, BlockId>,
    map: BTreeMap<ValueId, Operand>,
}

/// Every path's blocks copied, each path's first copy reached from the block
/// its way in leaves, its last jumping to the case it ends in; the instructions
/// copied.
fn copy(
    context: &Context,
    function: &mut Function,
    paths: &[Path],
) -> usize {
    let mut count = 0;
    let mut made = Vec::new();
    for path in paths {
        let originals = &path.blocks;
        let last = *originals.last().expect("a path has a block");
        let mut after = path.from;
        let clones: Vec<BlockId> = originals
            .iter()
            .map(|_| {
                let clone = function.create_block(None);
                function.insert_block(clone, Some(after)).expect("a block after the path");
                after = clone;
                clone
            })
            .collect();
        let at: BTreeMap<BlockId, BlockId> = originals.iter().copied().zip(clones.iter().copied()).collect();
        let mut map: BTreeMap<ValueId, Operand> = BTreeMap::new();
        let mapped = |one: Operand, map: &BTreeMap<ValueId, Operand>| match one {
            Operand::Value(value) => map.get(&value).copied().unwrap_or(one),
            other => other,
        };
        for (index, &block) in originals.iter().enumerate() {
            let clone = clones[index];
            // The way in: a phi takes what it takes from the block before on
            // the path.
            let before = if index == 0 { path.from } else { originals[index - 1] };
            for inst in function.iter_block(block).collect::<Vec<_>>() {
                let instruction = function.instruction(inst).clone();
                if instruction.opcode == Opcode::Phi {
                    // A phi of the path's one way in: the copy has one input,
                    // the block before it on the path. A value
                    // it names that the originals make is made again on the
                    // paths that lead here, and the updater below
                    // reads it where it is read.
                    let taken = arms(function, inst)
                        .into_iter()
                        .find(|(_, from)| *from == before)
                        .expect("an input from the path");
                    let way = if index == 0 { path.from } else { clones[index - 1] };
                    let copy = function.create_instruction(
                        Opcode::Phi,
                        instruction.ty,
                        vec![mapped(taken.0, &map), Operand::Block(way)],
                        Flags::default(),
                        None,
                    );
                    function.insert(copy, Position::End(clone)).expect("a new block");
                    if let (Some(result), Some(made)) = (instruction.result, function.instruction(copy).result) {
                        map.insert(result, Operand::Value(made));
                    }
                    continue;
                }
                if instruction.opcode.is_terminator() {
                    continue;
                }
                let copy = function.clone_instruction(inst);
                let operands = instruction.operands.iter().map(|&one| mapped(one, &map)).collect();
                function.set_operands(copy, operands);
                function.insert(copy, Position::End(clone)).expect("a new block");
                if let (Some(result), Some(made)) = (instruction.result, function.instruction(copy).result) {
                    map.insert(result, Operand::Value(made));
                }
                count += 1;
            }
            // The jump on: along the path, the others as they were; the last is
            // a jump to its target.
            let end = function.terminator(block).expect("a terminated block");
            let terminator = function.instruction(end).clone();
            let void = terminator.ty;
            if block == last {
                let jump = function.create_instruction(
                    Opcode::Br,
                    void,
                    vec![Operand::Block(path.target)],
                    Flags::default(),
                    None,
                );
                function.insert(jump, Position::End(clone)).expect("a new block");
            } else {
                let next = originals[index + 1];
                let operands = terminator
                    .operands
                    .iter()
                    .map(|&one| match one {
                        Operand::Block(to) if to == next => Operand::Block(at[&to]),
                        Operand::Value(_) => mapped(one, &map),
                        other => other,
                    })
                    .collect();
                let jump =
                    function.create_instruction(terminator.opcode.clone(), void, operands, terminator.flags, None);
                function.insert(jump, Position::End(clone)).expect("a new block");
            }
        }
        // Each block the copies leave for gets an input from the copy that
        // leaves.
        for (&block, &clone) in &at {
            let leaving: Vec<BlockId> =
                function.successors(clone).into_iter().filter(|to| !at.values().any(|one| one == to)).collect();
            for to in leaving {
                for phi in function.iter_block(to).collect::<Vec<_>>() {
                    if function.instruction(phi).opcode != Opcode::Phi {
                        break;
                    }
                    // One input for each edge the block has to it.
                    let mut inputs = function.instruction(phi).operands.clone();
                    for (value, _) in arms(function, phi).into_iter().filter(|(_, from)| *from == block) {
                        inputs.extend([mapped(value, &map), Operand::Block(clone)]);
                    }
                    function.set_operands(phi, inputs);
                }
            }
        }
        // The way in is the copy's now: the first original's phis lose it.
        let first = originals[0];
        let entry = function.terminator(path.from).expect("a terminated block");
        let redirected = function
            .instruction(entry)
            .operands
            .iter()
            .map(|&one| if one == Operand::Block(first) { Operand::Block(clones[0]) } else { one })
            .collect();
        function.set_operands(entry, redirected);
        for phi in function.iter_block(first).collect::<Vec<_>>() {
            if function.instruction(phi).opcode != Opcode::Phi {
                break;
            }
            let kept: Vec<Operand> = function
                .instruction(phi)
                .operands
                .chunks(2)
                .filter(|pair| pair[1] != Operand::Block(path.from))
                .flatten()
                .copied()
                .collect();
            function.set_operands(phi, kept);
        }
        made.push(Copies { at, map });
    }
    // A value made in the originals and read beyond them is made by each copy
    // too, and a copy reads what the originals made before it, in the
    // iteration before: one updater joins the definitions where the paths meet.
    let originals: BTreeSet<BlockId> = paths.iter().flat_map(|path| path.blocks.iter().copied()).collect();
    for &block in &originals {
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            let Some(result) = function.instruction(inst).result else { continue };
            // Read anywhere but its own block: an original block is reached
            // from the copies too, and reads what meets there.
            let outside: Vec<_> = function
                .users(result)
                .iter()
                .cloned()
                .filter(|one| {
                    let user = function.instruction(one.user);
                    let at = match (user.opcode == Opcode::Phi).then(|| user.operands[one.index as usize + 1]) {
                        Some(Operand::Block(from)) => Some(from),
                        _ => function.parent(one.user),
                    };
                    at.is_some_and(|at| at != block)
                })
                .collect();
            if outside.is_empty() {
                continue;
            }
            let mut updater = SsaUpdater::new(function.value(result).ty, None);
            updater.add_available_value(block, Operand::Value(result));
            for copies in &made {
                if let (Some(&clone), Some(&other)) = (copies.at.get(&block), copies.map.get(&result)) {
                    updater.add_available_value(clone, other);
                }
            }
            for one in outside {
                updater.rewrite_use(context, function, one);
            }
        }
    }
    count
}

/// A loop header's branch on a compare the loop does not change, the two arms
/// rejoining where a chain of blocks runs to the latch: each arm is given a
/// copy of the chain and of the header's phis, so the loop of an arm no longer
/// tests the branch. GCC reaches it by threading the latch's jump to the header
/// along each arm (`thread1`, `tree-ssa-threadupdate.cc`), the branch being
/// settled by the arm the trip took.
struct Arms {
    header: BlockId,
    /// The arms' heads, the join and the chain from it to the latch.
    heads: [BlockId; 2],
    chain: Vec<BlockId>,
    /// The arms' blocks, besides the chain.
    regions: [IdSet<BlockId>; 2],
    /// The block on each arm that enters the join: the header itself where the
    /// arm's head is the join.
    feeders: [BlockId; 2],
    /// Instructions of the chain, what one copy of it holds.
    size: usize,
}

/// `loop_`'s arms, where it has the shape.
fn arms_of(
    function: &Function,
    loop_: &Loop,
) -> Option<Arms> {
    let header = cfg::block(loop_.header);
    let [latch_at] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let latch = cfg::block(latch_at);
    let body: IdSet<BlockId> = loop_.body.iter().map(|&at| cfg::block(at)).collect();
    let outside = function.predecessors(header).into_iter().filter(|one| !body.contains(one)).count();
    if outside != 1 || latch == header {
        return None;
    }
    // The header holds phis and its branch.
    let last = function.terminator(header)?;
    if function.iter_block(header).any(|inst| inst != last && function.instruction(inst).opcode != Opcode::Phi) {
        return None;
    }
    let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] = function.instruction(last).operands[..]
    else {
        return None;
    };
    if yes == no || yes == header || no == header || !body.contains(&yes) || !body.contains(&no) {
        return None;
    }
    // The compare is made before the loop, of values made before it.
    let ValueDef::Instruction(made) = function.value(condition).def else { return None };
    if !matches!(function.instruction(made).opcode, Opcode::ICmp(_)) {
        return None;
    }
    let inside = |value: ValueId| match function.value(value).def {
        ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|block| body.contains(&block)),
        _ => false,
    };
    if function.parent(made).is_some_and(|block| body.contains(&block))
        || function.instruction(made).operands.iter().any(|one| matches!(one, Operand::Value(value) if inside(*value)))
    {
        return None;
    }
    // Other blocks of the loop are entered from the loop alone.
    if body.iter().any(|block| block != header && function.predecessors(block).iter().any(|one| !body.contains(one))) {
        return None;
    }
    let reach = |from: BlockId| {
        let mut seen = IdSet::new();
        let mut pending = vec![from];
        while let Some(block) = pending.pop() {
            if block == header || !body.contains(&block) || !seen.insert(block) {
                continue;
            }
            pending.extend(function.successors(block));
        }
        seen
    };
    let (from_yes, from_no) = (reach(yes), reach(no));
    let both: IdSet<BlockId> = from_yes.iter().filter(|block| from_no.contains(block)).collect();
    // The join: where the common blocks begin; the chain from it to the latch.
    let heads: Vec<BlockId> =
        both.iter().filter(|&block| !function.predecessors(block).iter().any(|one| both.contains(one))).collect();
    let [join] = heads[..] else { return None };
    let mut chain = vec![join];
    while *chain.last()? != latch {
        let here = *chain.last()?;
        let next: Vec<BlockId> =
            function.successors(here).into_iter().filter(|one| body.contains(one) && *one != header).collect();
        let [next] = next[..] else { return None };
        if function.predecessors(next) != [here] || chain.contains(&next) {
            return None;
        }
        chain.push(next);
    }
    if chain.len() != both.len() || chain.iter().any(|block| !both.contains(block)) {
        return None;
    }
    // Only the latch goes back to the header.
    if chain[..chain.len() - 1].iter().any(|&block| function.successors(block).contains(&header)) {
        return None;
    }
    // Each arm has its blocks, ending at the join by one block.
    let mut regions = [IdSet::new(), IdSet::new()];
    let mut feeders = [header, header];
    for (arm, (head, from)) in [(yes, &from_yes), (no, &from_no)].into_iter().enumerate() {
        regions[arm] = from.iter().filter(|block| !both.contains(block)).collect();
        let entering: Vec<BlockId> = function
            .predecessors(join)
            .into_iter()
            .filter(|one| regions[arm].contains(one) || (*one == header && head == join))
            .collect();
        let [one] = entering[..] else { return None };
        feeders[arm] = one;
        // An arm's blocks go only to its own, the join, or out of the loop.
        for block in regions[arm].iter() {
            if function
                .successors(block)
                .into_iter()
                .any(|to| body.contains(&to) && !regions[arm].contains(&to) && to != join)
            {
                return None;
            }
        }
    }
    if function.predecessors(join).len() != 2 || regions[0].iter().any(|block| regions[1].contains(&block)) {
        return None;
    }
    let size = crate::inline::gcc_insns_in(
        function,
        chain.iter().flat_map(|&block| function.iter_block(block).collect::<Vec<_>>()),
    ) as usize;
    Some(Arms { header, heads: [yes, no], chain, regions, feeders, size })
}

/// Every loop `arms_of` finds, split into a loop for each arm, within what is
/// left of `MAX_COPIED`; whether any was.
fn split_arms(
    context: &Context,
    function: &mut Function,
    copied: &mut usize,
) -> bool {
    let mut changed = false;
    let mut done = BTreeSet::new();
    loop {
        let shape = cfg::Shape::of(function);
        let found = shape
            .loops
            .iter()
            .filter(|one| !done.contains(&one.header))
            .find_map(
                |one| {
                    let arms = arms_of(function, one)?;
                    // Two copies of the chain, in gcc's unit.
                    let copies = 2 * arms.size.saturating_sub(1).max(1);
                    (copies <= MAX_PATH && *copied + copies <= MAX_COPIED).then_some((one.header, arms, copies))
                },
            );
        let Some((header, arms, copies)) = found else { break };
        // Whatever the split loop holds is not split again: its header is gone.
        done.insert(header);
        *copied += copies;
        llrm_support::debug!(
            "jumpthread",
            "split the loop headed b{header} by its invariant branch: {copies} instructions copied"
        );
        _split(context, function, &arms);
        changed = true;
    }
    if changed {
        cfg::_unreachable(context, function);
    }
    changed
}

fn _split(
    context: &Context,
    function: &mut Function,
    arms: &Arms,
) {
    let _ = context;
    let header = arms.header;
    let join = arms.chain[0];
    let latch = *arms.chain.last().expect("a chain");
    let branch = function.terminator(header).expect("a branch");
    let void = function.instruction(branch).ty;
    let phis = crate::edges::phis(function, header);
    // The header's phis once more at the head of each arm: what the trip
    // before made, or at the first trip what the header took.
    let mut heads = Vec::new();
    for _ in 0..2 {
        let block = function.create_block(None);
        function.insert_block(block, Some(header)).expect("a block after the header");
        heads.push(block);
    }
    let mut seen = [IdMap::<ValueId, Operand>::new(), IdMap::<ValueId, Operand>::new()];
    let mut made_phis: Vec<Vec<InstId>> = vec![Vec::new(), Vec::new()];
    for arm in 0..2 {
        for &phi in &phis {
            let ty = function.instruction(phi).ty;
            let copy = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
            function.insert(copy, Position::End(heads[arm])).expect("a new block");
            let result = function.instruction(phi).result.expect("a phi's value");
            let made = function.instruction(copy).result.expect("a phi's value");
            seen[arm].insert(result, Operand::Value(made));
            made_phis[arm].push(copy);
        }
    }
    // A chain for each arm.
    let mut chains: Vec<IdMap<BlockId, BlockId>> = Vec::new();
    let mut values: Vec<IdMap<ValueId, Operand>> = Vec::new();
    for arm in 0..2 {
        let mut at = IdMap::new();
        let mut after = heads[arm];
        for &block in &arms.chain {
            let clone = function.create_block(None);
            function.insert_block(clone, Some(after)).expect("a block after the chain");
            after = clone;
            at.insert(block, clone);
        }
        let mut map = seen[arm].clone();
        let mapped = |one: Operand, map: &IdMap<ValueId, Operand>| match one {
            Operand::Value(value) => map.get(&value).copied().unwrap_or(one),
            other => other,
        };
        let feeder = if arms.feeders[arm] == header { heads[arm] } else { arms.feeders[arm] };
        for &block in &arms.chain {
            let clone = at[&block];
            for inst in function.iter_block(block).collect::<Vec<_>>() {
                let original = function.instruction(inst).clone();
                if original.opcode == Opcode::Phi {
                    // The one way in of the join.
                    let taken = arms_of_phi(function, inst)
                        .into_iter()
                        .find(|(_, from)| *from == arms.feeders[arm])
                        .expect("an input from the arm");
                    let copy = function.create_instruction(
                        Opcode::Phi,
                        original.ty,
                        vec![mapped(taken.0, &map), Operand::Block(feeder)],
                        Flags::default(),
                        None,
                    );
                    function.insert(copy, Position::End(clone)).expect("a new block");
                    if let (Some(result), Some(made)) = (original.result, function.instruction(copy).result) {
                        map.insert(result, Operand::Value(made));
                    }
                    continue;
                }
                let copy = function.clone_instruction(inst);
                let operands = original
                    .operands
                    .iter()
                    .map(|&one| match one {
                        Operand::Block(to) if at.contains_key(&to) => Operand::Block(at[&to]),
                        Operand::Block(to) if to == header => Operand::Block(heads[arm]),
                        other => mapped(other, &map),
                    })
                    .collect();
                function.set_operands(copy, operands);
                function.insert(copy, Position::End(clone)).expect("a new block");
                if let (Some(result), Some(made)) = (original.result, function.instruction(copy).result) {
                    map.insert(result, Operand::Value(made));
                }
            }
        }
        chains.push(at);
        values.push(map);
    }
    // The header branches to the heads, each of which goes on into its arm: the
    // arm's own head, or the copy of the join.
    let Operand::Value(condition) = function.instruction(branch).operands[0] else { unreachable!("a compare") };
    let split = function.create_instruction(
        Opcode::Br,
        void,
        vec![Operand::Value(condition), Operand::Block(heads[0]), Operand::Block(heads[1])],
        Flags::default(),
        None,
    );
    function.insert(split, Position::Before(branch)).expect("the header's branch");
    function.erase(branch).expect("a branch");
    for arm in 0..2 {
        let target = if arms.heads[arm] == join { chains[arm][&join] } else { arms.heads[arm] };
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(target)], Flags::default(), None);
        function.insert(jump, Position::End(heads[arm])).expect("a new block");
        // Where the arm ends in the join, it goes to the join's copy.
        if arms.feeders[arm] != header {
            let end = function.terminator(arms.feeders[arm]).expect("a terminated block");
            crate::edges::retarget(function, end, join, chains[arm][&join]);
        }
        // The arm's head takes its way in from the arm's header copy.
        if arms.heads[arm] != join {
            for phi in crate::edges::phis(function, arms.heads[arm]) {
                let incoming = arms_of_phi(function, phi)
                    .into_iter()
                    .map(|(value, from)| if from == header { (value, heads[arm]) } else { (value, from) })
                    .collect::<Vec<_>>();
                function.set_operands(phi, crate::lcssa::from_arms(&incoming));
            }
        }
    }
    // The header copies' phis: from the header the first time, from the arm's
    // chain after.
    for arm in 0..2 {
        for (&phi, &copy) in phis.iter().zip(&made_phis[arm]) {
            let result = function.instruction(phi).result.expect("a phi's value");
            let latch_value = arms_of_phi(function, phi)
                .into_iter()
                .find(|(_, from)| *from == latch)
                .map(|(value, _)| value)
                .expect("an input from the latch");
            let back = match latch_value {
                Operand::Value(value) => values[arm].get(&value).copied().unwrap_or(latch_value),
                other => other,
            };
            let first = Operand::Value(result);
            function.set_operands(copy, vec![first, Operand::Block(header), back, Operand::Block(chains[arm][&latch])]);
        }
    }
    // The header is entered once now: its phis keep the way in.
    for &phi in &phis {
        let incoming = arms_of_phi(function, phi).into_iter().filter(|(_, from)| *from != latch).collect::<Vec<_>>();
        function.set_operands(phi, crate::lcssa::from_arms(&incoming));
    }
    // What the header made, an arm's blocks read as its copy has it.
    for arm in 0..2 {
        for block in arms.regions[arm].iter() {
            for inst in function.iter_block(block).collect::<Vec<_>>() {
                let operands = function
                    .instruction(inst)
                    .operands
                    .iter()
                    .map(|&one| match one {
                        Operand::Value(value) => seen[arm].get(&value).copied().unwrap_or(one),
                        other => other,
                    })
                    .collect();
                function.set_operands(inst, operands);
            }
        }
    }
    // Where the chain's blocks leave the loop, the copies leave too, and the
    // arms' blocks read the header's phis as their copy has them.
    let body: IdSet<BlockId> = arms
        .chain
        .iter()
        .copied()
        .chain(arms.regions.iter().flat_map(|region| region.iter()))
        .chain([header])
        .collect();
    let leaving: IdSet<BlockId> = body
        .iter()
        .flat_map(|block| function.successors(block))
        .filter(|to| {
            !body.contains(to) && !chains.iter().any(|at| at.values().any(|one| one == to)) && !heads.contains(to)
        })
        .collect();
    for to in leaving.iter() {
        for phi in crate::edges::phis(function, to) {
            let mut incoming = arms_of_phi(function, phi);
            let mut extra = Vec::new();
            for (value, from) in incoming.clone() {
                for arm in 0..2 {
                    if let Some(&copy) = chains[arm].get(&from) {
                        let mapped = match value {
                            Operand::Value(one) => values[arm].get(&one).copied().unwrap_or(value),
                            other => other,
                        };
                        extra.push((mapped, copy));
                    }
                }
            }
            // An arm's block leaves with the header's phis as its copy has
            // them.
            for (value, from) in &mut incoming {
                for arm in 0..2 {
                    if arms.regions[arm].contains(from) {
                        if let Operand::Value(one) = *value {
                            if let Some(&copy) = seen[arm].get(&one) {
                                *value = copy;
                            }
                        }
                    }
                }
            }
            incoming.extend(extra);
            function.set_operands(phi, crate::lcssa::from_arms(&incoming));
        }
    }
}

fn arms_of_phi(
    function: &Function,
    phi: InstId,
) -> Vec<(Operand, BlockId)> {
    arms(function, phi)
}

#[cfg(test)]
#[path = "jumpthread_tests.rs"]
mod tests;
