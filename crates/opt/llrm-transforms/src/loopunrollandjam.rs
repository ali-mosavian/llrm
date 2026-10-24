//! Unroll and jam: a loop with a loop inside is unrolled by `count`, and the
//! copies of the inner loop are fused into one, so a load of the inner loop
//! that does not move with the outer one is made once for all the copies
//! (`Utils/LoopUnrollAndJam.cpp` and `Scalar/LoopUnrollAndJamPass.cpp` at
//! `d1106deb71cc`).
//!
//! # How it works
//! The blocks of the outer loop `L` are the *fore* blocks (before the inner
//! loop), the inner loop's, and the *aft* blocks (after it).
//! `isSafeToUnrollAndJam` asks, in order, that the nest is simple (every loop
//! in simplify and rotated form with one exit and one exiting block, the inner
//! loop an only child, the aft block a single one, each inner loop's trip count
//! the same every time round), that no block may throw, that what the header
//! phis read from the aft block can be moved before the inner loop, and that no
//! pair of memory accesses of the fore, inner and aft blocks has a dependence
//! (DA) the fuse would reverse: a direction `<` at the unrolled level that the
//! inner levels no longer carry forward, or `>` where the copies interleave.
//!
//! `UnrollAndJamLoop` clones the fore blocks, the inner loop and the aft blocks
//! `count - 1` times (when the trip count is not a multiple of `count`, an
//! epilog of the outer loop runs the rest, [`crate::loopunrollruntime`]), then
//! chains the copies: all fore copies, then one inner loop whose header phis
//! and body are those of the copies in turn, then the aft copies. The outer
//! loop's latch tests once for the whole set.
//!
//! ```text
//!  for i              for i step 2
//!    Fore(i)            Fore(i); Fore(i+1)
//!    for j        =>    for j
//!      Sub(i,j)           Sub(i,j); Sub(i+1,j)
//!    Aft(i)             Aft(i); Aft(i+1)
//! ```
//!
//! The pass chooses `count` the way the unroller does for the outer loop
//! (`computeUnrollCount`), cuts it until the jammed inner loop is under
//! `-unroll-and-jam-threshold` (60), and refuses unless the inner loop has a
//! load that does not move with the outer one, or the user asked.
//!
//! # Deviations from LLVM
//! - D1: on at -O3 and -Omax only, and then with `Partial` and `Runtime` unroll
//!   preferences on. The pin's pass is off unless `-allow-unroll-and-jam` or a
//!   pragma asks (`enable-unroll-and-jam` is false) and the targets leave
//!   `UP.Partial` and `UP.Runtime` off; the `particle` row at -O3 is the
//!   reason. The pragma and `-allow-unroll-and-jam` ask as they do at the pin.
//! - D2: no loop metadata is made: not `llvm.loop.unroll_and_jam.followup_*`,
//!   not `setLoopAlreadyUnrolled` (a function pass cannot add metadata nodes),
//!   not `llvm.loop.unroll.disable` on the epilog. llrm has no partial or
//!   runtime unroller to run on the result.
//! - D3: `LoopInfo`, the dominator tree and SCEV are made again after the
//!   epilog and after the jam; `DomTreeUpdater` and `addClonedBlockToLoopInfo`
//!   are not translated.
//! - D4: the epilog remainder is [`crate::loopunrollruntime`]'s: epilog only,
//!   one exit, no weights.
//! - D5: no optimization remarks.
//! - D6: closed. BasicAA's `aliasPHI`/`aliasSelect` are translated; `function`
//!   of `pragma-explicit.ll` is jammed as the pin does (`function2` still
//!   differs in the remainder's phis: `tools/loopunrollandjam_expected.toml`).
//! - D7: `ScalarEvolution::isKnownViaInduction` is a stub (S5), so the outer
//!   latch compare `niter.next != N` stays where LLVM folds it to `false`
//!   (`test6`, `test10`). Exit: S5.
//!
//! # Example
//! `for i<n { for j<m { s[i] += a[j] * b[i] } }` with `a[j]` loaded in the
//! inner loop and not moving with `i`: unrolled by 2, the inner loop loads
//! `a[j]` once for `i` and `i+1`.
//!
//! # Runs
//! -O3 and -Omax, in the `Late` stage (`loop-unroll-and-jam@1474`).
//!
//! # See also
//! `isSafeToUnrollAndJam` (LoopUnrollAndJam.cpp:862), `UnrollAndJamLoop`
//! (:214), `computeUnrollAndJamCount` (LoopUnrollAndJamPass.cpp:142),
//! `tryToUnrollAndJamLoop` (:263).

use llrm_analysis::depanalysis::{Dependence, DependenceInfo, dir};
use llrm_analysis::mustexecute::SimpleLoopSafetyInfo;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::disposition::LoopDisposition;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::Loop;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_write_to_memory};

type Blocks = Vec<BlockId>;

fn instruction_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(v) = operand else { return None };
    match function.value(v).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

fn phis(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

fn incoming_for(
    function: &Function,
    phi: InstId,
    block: BlockId,
) -> Option<Operand> {
    function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(block)).map(|pair| pair[0])
}

/// `Loop::getLoopsInPreorder`.
pub(crate) fn loops_in_preorder<'a>(l: Loop<'a>) -> Vec<Loop<'a>> {
    let mut loops = vec![l];
    l.get_inner_loops_in_preorder(&mut loops);
    loops
}

/// `partitionLoopBlocks`: the blocks of `l` outside its only child, split into
/// those before it (fore) and those after (aft); `None` if the fore blocks,
/// together, do not lead only into the child's preheader.
fn partition_loop_blocks(
    cx: &Cx,
    l: Loop,
) -> Option<(Blocks, Blocks)> {
    let sub = l.get_sub_loops().next()?;
    let sub_latch = sub.get_loop_latch(cx.function)?;
    let (mut fore, mut aft) = (Vec::new(), Vec::new());
    for &block in l.get_blocks() {
        if !sub.contains(block) {
            if cx.tree.dominates(sub_latch, block) {
                aft.push(block);
            } else {
                fore.push(block);
            }
        }
    }
    // All the fore blocks together dominate the sub-loop.
    let sub_preheader = sub.get_loop_preheader(cx.function)?;
    for &block in &fore {
        if block == sub_preheader {
            continue;
        }
        if cx.function.successors(block).into_iter().any(|succ| !fore.contains(&succ)) {
            return None;
        }
    }
    Some((fore, aft))
}

/// The fore and aft blocks of each loop from `root` down to, not including,
/// `jam_loop`.
struct Partition {
    sub_loop_blocks: Blocks,
    fore: Vec<(BlockId, Blocks)>,
    aft: Vec<(BlockId, Blocks)>,
}

impl Partition {
    fn fore_of(
        &self,
        header: BlockId,
    ) -> Option<&Blocks> {
        self.fore.iter().find(|(one, _)| *one == header).map(|(_, blocks)| blocks)
    }

    fn aft_of(
        &self,
        header: BlockId,
    ) -> Option<&Blocks> {
        self.aft.iter().find(|(one, _)| *one == header).map(|(_, blocks)| blocks)
    }
}

/// `partitionOuterLoopBlocks(Root, JamLoop, ...)`.
fn partition_outer_loop_blocks(
    cx: &Cx,
    root: Loop,
    jam_loop: Loop,
) -> Option<Partition> {
    let mut partition =
        Partition { sub_loop_blocks: jam_loop.get_blocks().to_vec(), fore: Vec::new(), aft: Vec::new() };
    for l in loops_in_preorder(root) {
        if l == jam_loop {
            break;
        }
        let (fore, aft) = partition_loop_blocks(cx, l)?;
        partition.fore.push((l.get_header(), fore));
        partition.aft.push((l.get_header(), aft));
    }
    Some(partition)
}

/// `processHeaderPhiOperands`: `visit` every instruction the header phis read
/// from the latch, and the operands of those in `aft`, operands first; false
/// once `visit` says so.
fn process_header_phi_operands(
    function: &Function,
    header: BlockId,
    latch: BlockId,
    aft: &[BlockId],
    visit: &mut dyn FnMut(InstId) -> bool,
) -> bool {
    fn process(
        function: &Function,
        aft: &[BlockId],
        visited: &mut Vec<InstId>,
        visit: &mut dyn FnMut(InstId) -> bool,
        inst: InstId,
    ) -> bool {
        if visited.contains(&inst) {
            return true;
        }
        visited.push(inst);
        if function.parent(inst).is_some_and(|block| aft.contains(&block)) {
            for operand in function.instruction(inst).operands.clone() {
                if let Some(operand_inst) = instruction_of(function, operand)
                    && !process(function, aft, visited, visit, operand_inst)
                {
                    return false;
                }
            }
        }
        visit(inst)
    }
    let mut visited: Vec<InstId> = Vec::new();
    for phi in phis(function, header) {
        let Some(value) = incoming_for(function, phi, latch) else { continue };
        if let Some(inst) = instruction_of(function, value)
            && !process(function, aft, &mut visited, visit, inst)
        {
            return false;
        }
    }
    true
}

/// `isEligibleLoopForm`.
fn is_eligible_loop_form(
    cx: &Cx,
    root: Loop,
) -> bool {
    // Root must have a child.
    if root.get_sub_loops().count() != 1 {
        return false;
    }
    let mut l = root;
    loop {
        // All loops in Root need to be in simplify and rotated form.
        if !l.is_loop_simplify_form(cx.function) {
            return false;
        }
        // `isRotatedForm`: a latch that exits.
        if !l.get_loop_latch(cx.function).is_some_and(|latch| l.is_loop_exiting(cx.function, latch)) {
            return false;
        }
        let subs = l.get_sub_loops().count();
        if subs == 0 {
            return true;
        }
        // Only one child is allowed.
        if subs != 1 {
            return false;
        }
        // Only loops with a single exit block (not `getUniqueExitBlock`: that
        // allows several exit edges) and a single exiting block.
        if l.get_exit_block(cx.function).is_none() {
            llrm_support::debug!("unroll-jam", "only loops with single exit blocks can be unrolled and jammed");
            return false;
        }
        if l.get_exiting_block(cx.function).is_none() {
            llrm_support::debug!("unroll-jam", "only loops with single exiting blocks can be unrolled and jammed");
            return false;
        }
        l = l.get_sub_loops().next().expect("one child");
    }
}

/// `getInnerMostLoop`.
fn innermost_loop<'a>(mut l: Loop<'a>) -> Loop<'a> {
    while let Some(sub) = l.get_sub_loops().next() {
        l = sub;
    }
    l
}

/// `hasIterationCountInvariantInParent`.
fn has_iteration_count_invariant_in_parent(
    cx: &Cx,
    se: &mut ScalarEvolution,
    inner: Loop,
) -> bool {
    let Some(outer) = inner.get_parent_loop() else { return true };
    let Some(latch) = inner.get_loop_latch(cx.function) else { return false };
    // The backedge count of the inner loop.
    let count = se.get_exit_count(cx, inner.get_header(), latch, ExitCountKind::Exact);
    if se.is_could_not_compute(count) || !matches!(se.type_of(count), llrm_analysis::scev::expr::Ty::Int(_)) {
        return false;
    }
    // Whether the count is invariant to the outer loop.
    se.get_loop_disposition(cx, count, Some(outer.get_header())) == LoopDisposition::Invariant
}

/// `getLoadsAndStores`: the simple loads and stores of `blocks`; `None` where
/// another instruction touches memory.
fn loads_and_stores(
    cx: &Cx,
    blocks: &[BlockId],
) -> Option<Vec<InstId>> {
    let query = cx.query(None);
    let mut found = Vec::new();
    for &block in blocks {
        for inst in cx.function.iter_block(block) {
            match cx.function.instruction(inst).opcode {
                Opcode::Load { volatile, .. } | Opcode::Store { volatile, .. } => {
                    if volatile {
                        return None;
                    }
                    found.push(inst);
                }
                _ => {
                    if may_read_from_memory(&query, inst) || may_write_to_memory(&query, inst) {
                        return None;
                    }
                }
            }
        }
    }
    Some(found)
}

/// `preservesForwardDependence`.
fn preserves_forward_dependence(
    unroll_level: u32,
    jam_level: u32,
    d: &Dependence,
) -> bool {
    // UnrollLevel might carry the dependency Src --> Dst. Does a different
    // loop after unrolling?
    for level in unroll_level + 1..=jam_level {
        let jammed = d.direction(level, false);
        if jammed == dir::LT {
            return true;
        }
        if jammed & dir::GT != 0 {
            return false;
        }
    }
    true
}

/// `preservesBackwardDependence`.
fn preserves_backward_dependence(
    unroll_level: u32,
    jam_level: u32,
    sequentialized: bool,
    d: &Dependence,
) -> bool {
    // UnrollLevel might carry the dependency Dst --> Src.
    for level in unroll_level + 1..=jam_level {
        let jammed = d.direction(level, false);
        if jammed == dir::GT {
            return true;
        }
        if jammed & dir::LT != 0 {
            return false;
        }
    }
    // Backward dependencies are only preserved if not interleaved.
    sequentialized
}

/// `checkDependency`: whether unroll-and-jam keeps the dependence between
/// `src` and `dst`.
fn check_dependency<A: llrm_analysis::aa::AAResults>(
    cx: &Cx,
    se: &mut ScalarEvolution,
    di: &mut DependenceInfo<A>,
    src: InstId,
    dst: InstId,
    unroll_level: u32,
    jam_level: u32,
    sequentialized: bool,
) -> bool {
    debug_assert!(unroll_level <= jam_level, "Expecting JamLevel to be at least UnrollLevel");
    if src == dst {
        return true;
    }
    // Ignore input dependencies.
    let is_load = |inst| matches!(cx.function.instruction(inst).opcode, Opcode::Load { .. });
    if is_load(src) && is_load(dst) {
        return true;
    }
    // Every dependence is lexicographically non-negative by construction, such
    // as (0,0,>,*,*). Unroll-and-jam changes the GT execution of two executions
    // to the same iteration of the unrolled level: a GT becomes a GE (EQ, if
    // fully unrolled) at that loop's position: (0,0,>=,*,*), which is not
    // necessarily non-negative any more.
    let Some(d) = di.depends(se, src, dst, false) else { return true };
    if d.is_confused() {
        llrm_support::debug!("unroll-jam", "confused dependency");
        return false;
    }
    // If the levels enclosing the unrolled loop have a direction that is not
    // equal, the inner locations cannot overlap in memory.
    for level in 1..unroll_level {
        if d.direction(level, false) & dir::EQ == 0 {
            return true;
        }
    }
    let unroll_direction = d.direction(unroll_level, false);
    // A distance of zero at the unrolled level stays non-overlapping.
    if unroll_direction == dir::EQ {
        return true;
    }
    if unroll_direction & dir::LT != 0 && !preserves_forward_dependence(unroll_level, jam_level, &d) {
        return false;
    }
    if unroll_direction & dir::GT != 0 && !preserves_backward_dependence(unroll_level, jam_level, sequentialized, &d) {
        return false;
    }
    true
}

/// `checkDependencies`.
fn check_dependencies<A: llrm_analysis::aa::AAResults>(
    cx: &Cx,
    se: &mut ScalarEvolution,
    di: &mut DependenceInfo<A>,
    root: Loop,
    partition: &Partition,
) -> bool {
    let mut all: Vec<&Blocks> = Vec::new();
    for l in loops_in_preorder(root) {
        if let Some(blocks) = partition.fore_of(l.get_header()) {
            all.push(blocks);
        }
    }
    all.push(&partition.sub_loop_blocks);
    for l in loops_in_preorder(root) {
        if let Some(blocks) = partition.aft_of(l.get_header()) {
            all.push(blocks);
        }
    }
    let loop_depth = root.get_loop_depth();
    let mut earlier_loads_and_stores: Vec<InstId> = Vec::new();
    for blocks in all {
        let Some(current) = loads_and_stores(cx, blocks) else { return false };
        let Some(&first) = blocks.first() else { continue };
        let cur_depth = cx.loops.get_loop_for(first).map_or(0, |one| one.get_loop_depth());
        for &earlier in &earlier_loads_and_stores {
            let earlier_block = cx.function.parent(earlier).expect("a placed instruction");
            let earlier_depth = cx.loops.get_loop_for(earlier_block).map_or(0, |one| one.get_loop_depth());
            let common = earlier_depth.min(cur_depth);
            for &later in &current {
                if !check_dependency(cx, se, di, earlier, later, loop_depth, common, false) {
                    return false;
                }
            }
        }
        for (i, &a) in current.iter().enumerate() {
            for &b in &current[i..] {
                if !check_dependency(cx, se, di, a, b, loop_depth, cur_depth, true) {
                    return false;
                }
            }
        }
        earlier_loads_and_stores.extend(current);
    }
    true
}

/// `isSafeToUnrollAndJam`.
pub(crate) fn is_safe_to_unroll_and_jam<A: llrm_analysis::aa::AAResults>(
    cx: &Cx,
    se: &mut ScalarEvolution,
    di: &mut DependenceInfo<A>,
    l: Loop,
) -> bool {
    if !is_eligible_loop_form(cx, l) {
        llrm_support::debug!("unroll-jam", "ineligible loop form");
        return false;
    }
    // Split the blocks into Fore/SubLoop/Aft by dominators.
    let jam_loop = innermost_loop(l);
    let Some(partition) = partition_outer_loop_blocks(cx, l, jam_loop) else {
        llrm_support::debug!("unroll-jam", "incompatible loop layout");
        return false;
    };
    // Aft blocks may need to move instructions to the fore blocks, which is
    // harder with several, perhaps conditional, blocks.
    if partition.aft_of(l.get_header()).is_none_or(|blocks| blocks.len() != 1) {
        llrm_support::debug!("unroll-jam", "can't handle multiple blocks after the loop");
        return false;
    }
    // The inner loop's backedge count is the same on every iteration of the
    // outer loop.
    if loops_in_preorder(l).into_iter().any(|sub| !has_iteration_count_invariant_in_parent(cx, se, sub)) {
        llrm_support::debug!("unroll-jam", "inner loop iteration count is not consistent on each iteration");
        return false;
    }
    // The loop safety info for exceptions.
    if SimpleLoopSafetyInfo::new(cx, l).any_block_may_throw() {
        llrm_support::debug!("unroll-jam", "something may throw");
        return false;
    }
    // The instructions needed can be moved before the inner loop.
    let header = l.get_header();
    let Some(latch) = l.get_loop_latch(cx.function) else { return false };
    let aft = partition.aft_of(header).expect("the aft blocks of the loop").clone();
    let sub_loop = l.get_sub_loops().next().expect("one child");
    let query = cx.query(None);
    let movable = process_header_phi_operands(cx.function, header, latch, &aft, &mut |inst| {
        let block = cx.function.parent(inst).expect("a placed instruction");
        if sub_loop.contains(block) {
            return false;
        }
        if aft.contains(&block) {
            // A phi in the aft blocks: done (probably LCSSA).
            if cx.function.instruction(inst).opcode == Opcode::Phi {
                return false;
            }
            // Instructions with side effects or memory accesses cannot move.
            if may_have_side_effects(cx, inst)
                || may_read_from_memory(&query, inst)
                || may_write_to_memory(&query, inst)
            {
                return false;
            }
        }
        true
    });
    if !movable {
        llrm_support::debug!("unroll-jam", "can't move the instructions after the subloop to before it");
        return false;
    }
    // Memory dependences between Fore-Sub, Fore-Aft, Sub-Aft and Sub-Sub.
    if !check_dependencies(cx, se, di, l, &partition) {
        llrm_support::debug!("unroll-jam", "failed dependency check");
        return false;
    }
    true
}

/// `Instruction::mayHaveSideEffects`.
fn may_have_side_effects(
    cx: &Cx,
    inst: InstId,
) -> bool {
    let query = cx.query(None);
    llrm_mir::valuetracking::guarantee::may_write_to_memory(&query, inst)
        || llrm_mir::valuetracking::guarantee::may_throw(&query, inst)
        || !llrm_mir::valuetracking::guarantee::will_return(&query, inst)
}

// ---------------------------------------------------------------------------
// UnrollAndJamLoop
// ---------------------------------------------------------------------------

use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::expand_cost::ExpansionTti;
use llrm_mir::edit::{Position, ValueMap};
use llrm_support::hash::HashMap;

use crate::edges::phis as block_phis;
use crate::loopunrollpass::{Analyzed, Run, simplify_loop_after_unroll};

/// `LoopUnrollResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Jammed {
    Unmodified,
    PartiallyUnrolled,
    FullyUnrolled,
}

fn first_non_phi(
    function: &Function,
    block: BlockId,
) -> InstId {
    function
        .iter_block(block)
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .expect("a terminated block")
}

/// `BasicBlock::replacePhiUsesWith`.
fn replace_phi_uses_with(
    function: &mut Function,
    block: BlockId,
    old: BlockId,
    new: BlockId,
) {
    for phi in block_phis(function, block) {
        let operands = function.instruction(phi).operands.clone();
        for (at, operand) in operands.into_iter().enumerate() {
            if operand == Operand::Block(old) {
                function.set_operand(phi, at, Operand::Block(new));
            }
        }
    }
}

/// `Terminator::setSuccessor(index, block)`.
fn set_successor(
    function: &mut Function,
    term: InstId,
    index: usize,
    block: BlockId,
) {
    let at = if function.instruction(term).operands.len() == 3 { 1 + index } else { index };
    function.set_operand(term, at, Operand::Block(block));
}

/// A conditional branch replaced by a jump to `to`.
fn replace_with_jump(
    function: &mut Function,
    term: InstId,
    to: BlockId,
) {
    let void = function.instruction(term).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(to)], Default::default(), None);
    function.insert(jump, Position::Before(term)).expect("a placed branch");
    function.erase(term).expect("the branch has no users");
}

/// Every phi of `src` moves before the first non-phi of `dest`.
fn move_phis(
    function: &mut Function,
    src: BlockId,
    dest: BlockId,
) {
    let at = first_non_phi(function, dest);
    for phi in block_phis(function, src) {
        function.move_to(phi, Position::Before(at)).expect("a placed phi");
    }
}

/// `updatePHIBlocksAndValues`: the phis of `block` take, from `new`, the latest
/// version of what they took from `old`.
fn update_phi_blocks_and_values(
    function: &mut Function,
    block: BlockId,
    old: BlockId,
    new: BlockId,
    last: &HashMap<Operand, Operand>,
) {
    for phi in block_phis(function, block) {
        let mut operands = function.instruction(phi).operands.clone();
        for pair in 0..operands.len() / 2 {
            if operands[2 * pair + 1] == Operand::Block(old) {
                if let Some(&latest) = last.get(&operands[2 * pair]) {
                    operands[2 * pair] = latest;
                }
                operands[2 * pair + 1] = Operand::Block(new);
                break;
            }
        }
        function.set_operands(phi, operands);
    }
}

/// `UnrollLoop(RemainderLoop, ULO{Count = Count - 1, Runtime = false, ...})`:
/// the epilog loop, which runs at most `count` times, copied out where its
/// maximum trip count is `count` (the complete unroll; a partial one is not
/// translated: nothing then is done).
fn unroll_remainder_loop(
    run: &mut Run,
    epilog_header: BlockId,
    count: u32,
) {
    // The `llvm.assume` the epilog guard made is declared in `Declared`, which
    // the module has after the pass: the analyses name it by a declaration
    // beside the module's while the run lasts.
    let mut globals = run.globals.to_vec();
    let void = run.context.types.void();
    let bool_ty = run.context.types.int(1);
    let assume_type = run
        .context
        .types
        .intern(
            llrm_mir::types::Type::Function {
                returns: void,
                parameters: vec![bool_ty],
                variadic: false,
            },
        );
    if run.declared.find("llvm.assume").is_some_and(|id| id.0 as usize == globals.len()) {
        let function =
            llrm_mir::build::function_declaration(run.context, "llvm.assume", assume_type).expect("a function type");
        globals.push(llrm_mir::module::GlobalValue {
            name: "llvm.assume".to_owned().into(),
            linkage: llrm_mir::module::Linkage::External,
            unnamed_addr: Default::default(),
            address_space: 0,
            kind: llrm_mir::module::GlobalKind::Function(Box::new(function)),
        });
    }
    let tree = DomTree::of(run.function);
    let loops = LoopInfo::analyze(run.function, || &tree);
    let assumptions = AssumptionCache::of(run.context, &globals, run.metadata, run.function);
    let Some(lp) = loops.get_loop_for(epilog_header) else { return };
    let mut se = ScalarEvolution::new();
    let cx = Cx {
        context: &*run.context,
        globals: &globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let max_trip_count = se.get_small_constant_max_trip_count(&cx, epilog_header);
    let max_or_zero = se.is_backedge_taken_count_max_or_zero(&cx, epilog_header);
    // Effectively "DCE" the iterations beyond the maximum trip count.
    let count = if max_trip_count != 0 && count > max_trip_count { max_trip_count } else { count };
    if max_trip_count == 0 || count != max_trip_count {
        llrm_support::debug!("unroll-jam", "the remainder loop is not completely unrolled");
        return;
    }
    let mut exits = Vec::new();
    for exiting in lp.get_exiting_blocks(cx.function) {
        let trip_count = se.get_small_constant_trip_count_for_exit(&cx, epilog_header, exiting);
        if trip_count != 0 {
            exits.push(crate::loopunroll::Exit { block: exiting, trip_count });
        }
    }
    let body = lp.get_blocks().to_vec();
    let latches = lp.get_loop_latches(cx.function);
    let capacity = run.function.block_capacity();
    let options = crate::loopunroll::Options { count, max_trip_count: count, max_or_zero, exits };
    if let Err(error) =
        crate::loopunroll::unroll_loop_blocks(run.function, epilog_header, &body, &latches, &options, &mut run.emptied)
    {
        panic!("remainder unroll: {error}");
    }
    let _ = crate::lcssa::closed(run.function);
    let analyzed = Analyzed::of(run);
    let after = crate::loopunrollpass::unrolled_blocks(run, capacity, &body);
    simplify_loop_after_unroll(run, Some(&analyzed.loops), &after);
}

/// `UnrollAndJamLoop(L, Count, TripCount, TripMultiple, UnrollRemainder, ...)`
/// of the loop headed by `header`.
pub(crate) fn unroll_and_jam_loop(
    run: &mut Run,
    expansion: &dyn ExpansionTti,
    header: BlockId,
    count: u32,
    trip_count: u32,
    trip_multiple: u32,
    unroll_remainder: bool,
) -> Jammed {
    // Don't enter the unroll code if there is nothing to do.
    if trip_count == 0 && count < 2 {
        llrm_support::debug!("unroll-jam", "almost nothing to do");
        return Jammed::Unmodified;
    }
    debug_assert!(count > 0 && trip_multiple > 0 && (trip_count == 0 || trip_count % trip_multiple == 0));
    // Are we eliminating the loop control altogether?
    let completely_unroll = count == trip_count;

    // The runtime remainder, where the trip multiple does not say that the
    // count divides the trips.
    if trip_multiple % count != 0 {
        let mut session = ScevSession::of(run.context, run.function, run.outer);
        let mut env = crate::loopunrollruntime::Env {
            context: &mut *run.context,
            layout: run.layout,
            function: &mut *run.function,
            declared: &mut *run.declared,
            outer: run.outer,
            tti: expansion,
        };
        let done =
            crate::loopunrollruntime::unroll_runtime_loop_remainder(&mut env, &mut session, header, count, false);
        if done.is_none() {
            llrm_support::debug!("unroll-jam", "the remainder loop could not be generated");
            return Jammed::Unmodified;
        }
        if unroll_remainder && let Some(Some(epilog)) = done {
            unroll_remainder_loop(run, epilog, count - 1);
        }
        // The loops, the tree and the loop's blocks are made again (D3); the
        // remainder keeps the loop in simplify form.
    }

    let analyzed = Analyzed::of(run);
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let l = analyzed.loops.get_loop_for(header).expect("a loop headed by this block");
    let sub_loop = l.get_sub_loops().next().expect("one child");
    let preheader = l.get_loop_preheader(cx.function).expect("a preheader");
    let latch_block = l.get_loop_latch(cx.function).expect("a latch block");
    let latch_br = cx.function.terminator(latch_block).expect("a terminated latch");
    let branch = cx.function.instruction(latch_br);
    let Operand::Block(succ0) = branch.operands[1] else { unreachable!("a conditional branch") };
    let continue_on_true = l.contains(succ0);
    let Operand::Block(loop_exit) = branch.operands[1 + usize::from(continue_on_true)] else {
        unreachable!("a conditional branch")
    };
    let sub_latch = sub_loop.get_loop_latch(cx.function).expect("a sub-loop latch");
    let sub_term = cx.function.terminator(sub_latch).expect("a terminated latch");
    let Operand::Block(sub_succ0) = cx.function.instruction(sub_term).operands[1] else {
        unreachable!("a conditional branch")
    };
    let sub_loop_continue_on_true = sub_loop.contains(sub_succ0);

    // The blocks before and after the inner loop.
    let (fore_blocks, aft_blocks) = partition_loop_blocks(&cx, l).expect("checked safe to unroll and jam");
    let sub_loop_blocks: Blocks = sub_loop.get_blocks().to_vec();
    let loop_blocks: Blocks = l.get_blocks().to_vec();
    let sub_preheader = sub_loop.get_loop_preheader(cx.function).expect("a sub-loop preheader");
    let sub_header = sub_loop.get_header();
    let sub_exiting = sub_loop.get_exiting_block(cx.function).expect("an exiting block");
    let sub_exit = sub_loop.get_exit_block(cx.function).expect("an exit block");
    let l_exiting = l.get_exiting_block(cx.function).expect("an exiting block");

    // The entering and exiting block of each of Fore, SubLoop and Aft in each
    // iteration, to staple the blocks up.
    let mut fore_first = vec![header];
    let mut fore_last = vec![sub_preheader];
    let mut sub_first = vec![sub_header];
    let mut sub_last = vec![sub_exiting];
    let mut aft_first = vec![sub_exit];
    let mut aft_last = vec![l_exiting];
    // Blocks[0] -> Blocks[It].
    let mut last_value_map: HashMap<Operand, Operand> = HashMap::default();

    // Move the instructions the header phis read from Aft into Fore.
    {
        let insert_before = run.function.terminator(fore_last[0]).expect("a terminated block");
        let mut moving: Vec<InstId> = Vec::new();
        process_header_phi_operands(run.function, header, latch_block, &aft_blocks, &mut |inst| {
            if run.function.parent(inst).is_some_and(|block| aft_blocks.contains(&block)) {
                moving.push(inst);
            }
            true
        });
        for inst in moving {
            run.function.move_to(inst, Position::Before(insert_before)).expect("a placed instruction");
        }
    }

    // The blocks in reverse postorder, so the on-the-fly SSA update finds the
    // right value at each exit.
    let rpo = crate::loopunrollruntime::reverse_post_order(run.function, header, &|block| loop_blocks.contains(&block));
    let original_blocks = rpo.clone();

    // Copy all blocks.
    let mut all_new_blocks: Vec<BlockId> = Vec::new();
    for it in 1..count {
        let mut new_blocks: Vec<BlockId> = Vec::new();
        // Maps Blocks[It] -> Blocks[It-1].
        let mut prev_it_value_map: HashMap<Operand, Operand> = HashMap::default();
        for &bb in &original_blocks {
            let mut vmap = ValueMap::default();
            let new = run.function.clone_block(bb, &mut vmap);
            let name = format!("{}.{it}", run.function.block(bb).name.clone().unwrap_or_default());
            run.function.set_block_name(new, &name);
            run.function.insert_block(new, None).expect("a new block");
            for (from, to) in vmap.values.iter().map(|(from, to)| (from, *to)).collect::<Vec<_>>() {
                if let Operand::Value(copy) = to
                    && let Some(original) = run.function.value(from).name.clone()
                {
                    run.function.set_name(copy, &format!("{original}.{it}"));
                }
            }
            if fore_blocks.contains(&bb) {
                if bb == fore_first[0] {
                    fore_first.push(new);
                }
                if bb == fore_last[0] {
                    fore_last.push(new);
                }
            } else if sub_loop_blocks.contains(&bb) {
                if bb == sub_first[0] {
                    sub_first.push(new);
                }
                if bb == sub_last[0] {
                    sub_last.push(new);
                }
            } else if aft_blocks.contains(&bb) {
                if bb == aft_first[0] {
                    aft_first.push(new);
                }
                if bb == aft_last[0] {
                    aft_last.push(new);
                }
            } else {
                unreachable!("BB being cloned should be in Fore/Sub/Aft");
            }

            // Update our running maps of newest clones.
            let key = Operand::Block(bb);
            let last = last_value_map.get(&key).copied();
            prev_it_value_map.insert(Operand::Block(new), if it == 1 { key } else { last.expect("a previous copy") });
            last_value_map.insert(key, Operand::Block(new));
            for (from, to) in vmap.values.iter().map(|(from, to)| (from, *to)).collect::<Vec<_>>() {
                let key = Operand::Value(from);
                let last = last_value_map.get(&key).copied();
                prev_it_value_map.insert(to, if it == 1 { key } else { last.expect("a previous copy") });
                last_value_map.insert(key, to);
            }
            new_blocks.push(new);
        }

        // Remap all instructions in the most recent iteration.
        let mut map = ValueMap::default();
        for (&from, &to) in &last_value_map {
            match (from, to) {
                (Operand::Value(from), to) => {
                    map.values.insert(from, to);
                }
                (Operand::Block(from), Operand::Block(to)) => {
                    map.blocks.insert(from, to);
                }
                _ => {}
            }
        }
        for &block in &new_blocks {
            for inst in run.function.iter_block(block).collect::<Vec<_>>() {
                run.function.remap_instruction(inst, &map);
            }
        }

        // Alter the Fore blocks' phis to read the latest version of the value
        // from the previous iteration's phis.
        for phi in block_phis(run.function, fore_first[it as usize]) {
            let old_value = incoming_for(run.function, phi, aft_last[it as usize]).expect("an input from Aft[It]");
            let new_value = prev_it_value_map.get(&old_value).copied().unwrap_or(old_value);
            debug_assert_eq!(run.function.instruction(phi).operands.len(), 4);
            run.function.set_operands(phi, vec![new_value, Operand::Block(fore_last[it as usize - 1])]);
        }
        all_new_blocks.extend(new_blocks);
    }

    // Now that all the blocks of the unrolled iterations are in place, finish
    // connecting the blocks and phis. LastValueMap holds the last iteration's
    // values.
    let aft_last_back = *aft_last.last().expect("an aft block");
    let fore_last_back = *fore_last.last().expect("a fore block");
    let sub_last_back = *sub_last.last().expect("a sub-loop block");

    // The phis outside the loop point to the last block.
    update_phi_blocks_and_values(run.function, loop_exit, aft_last[0], aft_last_back, &last_value_map);

    // The Fore blocks' successors and phis.
    let fore_term = run.function.terminator(fore_last_back).expect("a terminated block");
    set_successor(run.function, fore_term, 0, sub_first[0]);

    if completely_unroll {
        for phi in block_phis(run.function, fore_first[0]) {
            let value = incoming_for(run.function, phi, preheader).expect("an input from the preheader");
            let result = run.function.instruction(phi).result.expect("a phi");
            run.function.replace_all_uses_with(result, value);
            run.function.erase(phi).expect("an unused phi");
        }
    } else {
        // The phis point to the last Aft block.
        update_phi_blocks_and_values(run.function, fore_first[0], aft_last[0], aft_last_back, &last_value_map);
    }

    for it in 1..count as usize {
        // Remap the Fore blocks' successors from the previous iteration to
        // this.
        let term = run.function.terminator(fore_last[it - 1]).expect("a terminated block");
        set_successor(run.function, term, 0, fore_first[it]);
    }

    // The inner loop's successors and phis.
    let sub_term = run.function.terminator(sub_last_back).expect("a terminated block");
    set_successor(run.function, sub_term, usize::from(!sub_loop_continue_on_true), sub_first[0]);
    set_successor(run.function, sub_term, usize::from(sub_loop_continue_on_true), aft_first[0]);
    replace_phi_uses_with(run.function, sub_first[0], fore_last[0], fore_last_back);
    replace_phi_uses_with(run.function, sub_first[0], sub_last[0], sub_last_back);

    for it in 1..count as usize {
        // The previous iteration's inner loop branch becomes a jump to this
        // one.
        let term = run.function.terminator(sub_last[it - 1]).expect("a terminated block");
        replace_with_jump(run.function, term, sub_first[it]);
        replace_phi_uses_with(run.function, sub_first[it], fore_last[it], fore_last_back);
        replace_phi_uses_with(run.function, sub_first[it], sub_last[it], sub_last_back);
        move_phis(run.function, sub_first[it], sub_first[0]);
    }

    // The Aft blocks' successors and phis.
    let aft_term = run.function.terminator(aft_last_back).expect("a terminated block");
    if completely_unroll {
        replace_with_jump(run.function, aft_term, loop_exit);
    } else {
        set_successor(run.function, aft_term, usize::from(!continue_on_true), fore_first[0]);
        debug_assert_eq!(
            run.function.instruction(aft_term).operands[1 + usize::from(continue_on_true)],
            Operand::Block(loop_exit),
            "Expecting the ContinueOnTrue successor of AftTerm to be LoopExit"
        );
    }
    replace_phi_uses_with(run.function, aft_first[0], sub_last[0], sub_last_back);

    for it in 1..count as usize {
        let term = run.function.terminator(aft_last[it - 1]).expect("a terminated block");
        replace_with_jump(run.function, term, aft_first[it]);
        replace_phi_uses_with(run.function, aft_first[it], sub_last[it], sub_last_back);
        move_phis(run.function, aft_first[it], aft_first[0]);
    }

    // Merge adjacent basic blocks, if possible.
    let mut loop_set: Vec<BlockId> = loop_blocks.clone();
    loop_set.extend(all_new_blocks.iter().copied());
    let mut merge_blocks: Vec<BlockId> = Vec::new();
    for list in [&fore_last, &sub_last, &aft_last] {
        for &block in list.iter() {
            if !merge_blocks.contains(&block) {
                merge_blocks.push(block);
            }
        }
    }
    let mut merged: Vec<BlockId> = Vec::new();
    while let Some(bb) = merge_blocks.first().copied() {
        let successors = run.function.successors(bb);
        let dest = (successors.len() == 1).then(|| successors[0]);
        match dest {
            Some(dest) if loop_set.contains(&dest) => {
                if crate::local::merge_block_into_predecessor(&*run.context, run.function, dest, false) {
                    merge_blocks.retain(|&one| one != dest);
                    loop_set.retain(|&one| one != dest);
                    merged.push(dest);
                } else {
                    merge_blocks.retain(|&one| one != bb);
                }
            }
            _ => merge_blocks.retain(|&one| one != bb),
        }
    }

    // The blocks merged away are empty: they go before any analysis looks.
    run.function.erase_blocks(&merged).expect("blocks merged away are empty and unnamed");

    // The code is well formed: a sweep over the inserted code, doing constant
    // propagation and dead code elimination as we go.
    let blocks_of_l: Blocks = loop_set.clone();
    let sub_header_now = sub_first[0];
    let analyzed = Analyzed::of(run);
    let sub_blocks_now: Blocks =
        analyzed.loops.get_loop_for(sub_header_now).map(|one| one.get_blocks().to_vec()).unwrap_or_default();
    let did = crate::simplifyindvar::simplified_loop_ivs(
        run.context,
        run.layout,
        run.function,
        run.declared,
        run.outer,
        sub_header_now,
    );
    llrm_support::debug!("unroll-jam", "simplified the IVs of the inner loop: {did}");
    simplify_loop_after_unroll(run, Some(&analyzed.loops), &sub_blocks_now);
    if !completely_unroll && count > 1 {
        let did = crate::simplifyindvar::simplified_loop_ivs(
            run.context,
            run.layout,
            run.function,
            run.declared,
            run.outer,
            header,
        );
        llrm_support::debug!("unroll-jam", "simplified the IVs of the outer loop: {did}");
    }
    let analyzed = Analyzed::of(run);
    simplify_loop_after_unroll(
        run,
        Some(&analyzed.loops),
        &blocks_of_l.iter().copied().filter(|&block| !run.function.block(block).is_empty() || true).collect::<Vec<_>>(),
    );

    if completely_unroll { Jammed::FullyUnrolled } else { Jammed::PartiallyUnrolled }
}

// ---------------------------------------------------------------------------
// LoopUnrollAndJamPass
// ---------------------------------------------------------------------------

use llrm_analysis::aggregate::AggregateAA;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::scev::expand_cost::MachineCosts;
use llrm_analysis::typebasedaa::TypeBasedAA;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::looppeel::PeelingPreferences;
use crate::loopunrollpass::{
    LoopOptions, Options as UnrollOptions, UnrollPragmaInfo, compute_unroll_count, cost_estimator, ephemeral_values,
    gather_unrolling_preferences_of_target, loop_has_calls,
};

/// `UnrollAndJamThreshold`'s default and `PragmaUnrollAndJamThreshold`.
const PRAGMA_UNROLL_AND_JAM_THRESHOLD: u32 = 1024;

/// The `cl::opt`s of the pass, and D1.
#[derive(Clone, Debug, Default)]
pub struct JamOptions {
    /// D1: unroll and jam without being asked (-O3), with `Partial` and
    /// `Runtime` on.
    pub on: bool,
    /// `-allow-unroll-and-jam`.
    pub allow: Option<bool>,
    /// `-unroll-and-jam-count`.
    pub count: Option<u32>,
    /// `-unroll-and-jam-threshold`.
    pub threshold: Option<u32>,
    /// `-unroll-runtime`.
    pub runtime: Option<bool>,
    /// `-unroll-partial-threshold`.
    pub partial_threshold: Option<u32>,
    /// `-unroll-remainder`.
    pub unroll_remainder: bool,
    /// `-da-*`.
    pub da: llrm_analysis::depanalysis::Options,
}

impl JamOptions {
    /// The flags of `arguments` this pass reads, taken out of it.
    pub fn take_flags(arguments: &mut Vec<String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut kept = Vec::new();
        for argument in std::mem::take(arguments) {
            let flag = argument.trim_start_matches('-');
            let number = |value: &str| value.parse::<u32>().map_err(|_| format!("{argument}: not a number"));
            if flag == "allow-unroll-and-jam" {
                options.allow = Some(true);
            } else if let Some(value) = flag.strip_prefix("unroll-and-jam-count=") {
                options.count = Some(number(value)?);
            } else if let Some(value) = flag.strip_prefix("unroll-and-jam-threshold=") {
                options.threshold = Some(number(value)?);
            } else if flag == "unroll-runtime" {
                options.runtime = Some(true);
            } else if let Some(value) = flag.strip_prefix("unroll-partial-threshold=") {
                options.partial_threshold = Some(number(value)?);
            } else if flag == "unroll-remainder" {
                options.unroll_remainder = true;
            } else if flag == "da-disable-delinearization-checks" {
                options.da.disable_delinearization_checks = true;
            } else if flag == "da-enable-dependence-test=all" {
                options.da.enable_test = llrm_analysis::depanalysis::TestType::All;
            } else {
                kept.push(argument);
            }
        }
        *arguments = kept;
        Ok(options)
    }
}

/// `getUnrollAndJammedLoopSize`.
fn unrolled_and_jammed_loop_size(
    loop_size: u32,
    be_insns: u32,
    count: u32,
) -> u64 {
    debug_assert!(loop_size >= be_insns, "LoopSize should not be less than BEInsns!");
    u64::from(loop_size - be_insns) * u64::from(count) + u64::from(be_insns)
}

/// `unrollAndJamCountPragmaValue`: the count of the pragma, else 0.
fn unroll_and_jam_count_pragma_value(
    run: &Run,
    options: &LoopOptions,
) -> u32 {
    options.int(run, "llvm.loop.unroll_and_jam.count").unwrap_or(0) as u32
}

/// What `computeUnrollAndJamCount` reads of the loop.
struct JamLoop<'a> {
    l: Loop<'a>,
    sub_loop: Loop<'a>,
    outer_trip_count: u32,
    outer_trip_multiple: u32,
    inner_trip_count: u32,
    inner_loop_size: u32,
}

/// `computeUnrollAndJamCount`: the count, and whether the jam is explicit.
fn compute_unroll_and_jam_count(
    run: &mut Run,
    analyzed: &Analyzed,
    se: &mut ScalarEvolution,
    options: &JamOptions,
    pragma: &LoopOptions,
    eph: &llrm_mir::dense::IdSet<InstId>,
    info: &JamLoop<'_>,
    outer_uce: &crate::loopunrollpass::UnrollCostEstimator,
    up: &mut llrm_mir::tti::UnrollingPreferences,
) -> (u32, bool) {
    let outer_loop_size = outer_uce.rolled_loop_size();

    // The outer loop's count as the unroller computes it. We have already
    // checked that the loop has no unroll.* pragmas.
    let pinfo = UnrollPragmaInfo {
        pragma_full_unroll: false,
        pragma_count: 0,
        pragma_enable_unroll: false,
        pragma_runtime_unroll_disable: false,
        explicit_unroll: false,
    };
    let mut pp = PeelingPreferences::default();
    let mut count = compute_unroll_count(
        run,
        analyzed,
        info.l,
        eph,
        &pinfo,
        info.outer_trip_count,
        0,
        false,
        info.outer_trip_multiple,
        outer_uce,
        up,
        se,
        &mut pp,
    );

    // An explicit count from the `unroll-and-jam-count` option.
    let user_unroll_count = options.count.is_some();
    if let Some(user) = options.count {
        count = user;
        up.force = true;
        if up.allow_remainder
            && unrolled_and_jammed_loop_size(outer_loop_size, up.be_insns, count) < u64::from(up.threshold)
            && unrolled_and_jammed_loop_size(info.inner_loop_size, up.be_insns, count)
                < u64::from(up.unroll_and_jam_inner_loop_threshold)
        {
            return (count, true);
        }
    }

    // The unroll_and_jam pragmas.
    let pragma_count = unroll_and_jam_count_pragma_value(run, pragma);
    if pragma_count > 0 {
        count = pragma_count;
        up.runtime = true;
        up.force = true;
        if (up.allow_remainder || info.outer_trip_multiple % pragma_count == 0)
            && unrolled_and_jammed_loop_size(outer_loop_size, up.be_insns, count) < u64::from(up.threshold)
            && unrolled_and_jammed_loop_size(info.inner_loop_size, up.be_insns, count)
                < u64::from(up.unroll_and_jam_inner_loop_threshold)
        {
            return (count, true);
        }
    }

    let pragma_enable_unroll = pragma.find("llvm.loop.unroll_and_jam.enable").is_some();
    let explicit_count = pragma_count > 0 || user_unroll_count;
    let explicit_unroll_and_jam = pragma_enable_unroll || explicit_count;

    // With an unrolling pragma, the limits are more aggressive.
    if explicit_unroll_and_jam {
        up.unroll_and_jam_inner_loop_threshold = PRAGMA_UNROLL_AND_JAM_THRESHOLD;
    }

    if !up.allow_remainder
        && unrolled_and_jammed_loop_size(info.inner_loop_size, up.be_insns, count)
            >= u64::from(up.unroll_and_jam_inner_loop_threshold)
    {
        llrm_support::debug!("unroll-jam", "can't create a remainder and the inner loop is too large");
        return (0, false);
    }

    // A sensible limit for the outer loop; now for the inner loop and
    // `UnrollAndJamInnerLoopThreshold`. An outer limit set explicitly stays.
    if !explicit_count && up.allow_remainder {
        while count != 0
            && unrolled_and_jammed_loop_size(info.inner_loop_size, up.be_insns, count)
                >= u64::from(up.unroll_and_jam_inner_loop_threshold)
        {
            count -= 1;
        }
    }

    // Explicitly unroll and jamming: done. Otherwise a number of performance
    // heuristics follow.
    if explicit_unroll_and_jam {
        return (count, true);
    }

    // A known, small inner count is left to the unroller.
    if info.inner_trip_count != 0
        && u64::from(info.inner_loop_size) * u64::from(info.inner_trip_count) < u64::from(up.threshold)
    {
        llrm_support::debug!("unroll-jam", "a small inner loop count is left to the unroller");
        return (0, false);
    }

    // Subloops of more than one block are likely unprofitable.
    if info.sub_loop.get_num_blocks() != 1 {
        llrm_support::debug!("unroll-jam", "more than one inner loop block");
        return (0, false);
    }

    // Something to gain: loads of the inner loop that are invariant in the
    // outer loop, to be shared.
    let cx = Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut num_invariant = 0;
    for &block in info.sub_loop.get_blocks() {
        for inst in cx.function.iter_block(block) {
            if let Opcode::Load { .. } = cx.function.instruction(inst).opcode {
                let pointer = cx.function.instruction(inst).operands[0];
                let scev = se.get_scev_at_scope_of_value(&cx, pointer, Some(info.l.get_header())).scev;
                if se.is_loop_invariant(&cx, scev, Some(info.l.get_header())) {
                    num_invariant += 1;
                }
            }
        }
    }
    if num_invariant == 0 {
        llrm_support::debug!("unroll-jam", "no loop invariant loads");
        return (0, false);
    }
    (count, false)
}

/// What the pass reads and changes in one go.
pub struct LoopUnrollAndJam {
    pub opt_level: u32,
    pub opt_size: bool,
    pub options: JamOptions,
}

impl LoopUnrollAndJam {
    /// `tryToUnrollAndJamLoop(L, ...)` of the loop headed by `header`.
    fn try_loop(
        &self,
        run: &mut Run,
        expansion: &dyn ExpansionTti,
        header: BlockId,
    ) -> Jammed {
        let analyzed = Analyzed::of(run);
        let Some(l) = analyzed.loops.get_loop_for(header) else { return Jammed::Unmodified };
        let pragma = LoopOptions::of(run, l);
        let has_calls = loop_has_calls(run, l);
        let mut up = gather_unrolling_preferences_of_target(run, has_calls);
        // D1, then the `cl::opt`s that set the preferences.
        if self.options.on {
            up.unroll_and_jam = true;
            up.partial = true;
            up.runtime = true;
        }
        if let Some(runtime) = self.options.runtime {
            up.runtime = runtime;
        }
        if let Some(threshold) = self.options.partial_threshold {
            up.partial_threshold = threshold;
        }
        up.unroll_remainder |= self.options.unroll_remainder;

        let enable_mode = pragma.unroll_and_jam(run);
        if matches!(
            enable_mode,
            crate::loopunrollpass::Mode::Disable | crate::loopunrollpass::Mode::SuppressedByUser
        ) {
            return Jammed::Unmodified;
        }
        if enable_mode.forced() {
            up.unroll_and_jam = true;
        }
        if let Some(allow) = self.options.allow {
            up.unroll_and_jam = allow;
        }
        if let Some(threshold) = self.options.threshold {
            up.unroll_and_jam_inner_loop_threshold = threshold;
        }
        // Exit early if unrolling is disabled.
        if !up.unroll_and_jam || up.unroll_and_jam_inner_loop_threshold == 0 {
            return Jammed::Unmodified;
        }
        llrm_support::debug!("unroll-jam", "loop %{}", run.function.block(header).name.clone().unwrap_or_default());

        // A loop with any unroll pragma is left to the unroller, so long as it
        // has no unroll_and_jam metadata.
        if pragma.has_any("llvm.loop.unroll.") && !pragma.has_any("llvm.loop.unroll_and_jam.") {
            llrm_support::debug!("unroll-jam", "disabled due to a pragma");
            return Jammed::Unmodified;
        }

        // The dependence analysis of the pass manager's alias analysis.
        let safe = {
            let mut session = ScevSession::of(run.context, run.function, run.outer);
            let (se, senv) = session.split(run.layout, &run.outer.globals, &run.outer.metadata);
            let cx = senv.cx(&*run.context, &*run.function);
            let Some(l) = cx.loops.get_loop_for(header) else { return Jammed::Unmodified };
            let memory_unit = llrm_analysis::memory::Unit::within(&*run.context, run.layout, &*run.function, run.outer);
            let mut aa = AggregateAA::new();
            aa.add_aa_result(BasicAA::new(&memory_unit));
            aa.add_aa_result(TypeBasedAA::new(&memory_unit));
            let mut di = DependenceInfo::new(&mut aa, &memory_unit, &cx, self.options.da);
            is_safe_to_unroll_and_jam(&cx, se, &mut di, l)
        };
        if !safe {
            llrm_support::debug!("unroll-jam", "disabled due to not being safe");
            return Jammed::Unmodified;
        }

        // Approximate the loop size and collect useful info.
        let eph = ephemeral_values(run, &analyzed, l);
        let sub_loop = l.get_sub_loops().next().expect("one child");
        let inner_uce = cost_estimator(run, sub_loop, &eph, up.be_insns);
        let outer_uce = cost_estimator(run, l, &eph, up.be_insns);
        if !inner_uce.can_unroll() || !outer_uce.can_unroll() {
            llrm_support::debug!("unroll-jam", "loop not considered unrollable");
            return Jammed::Unmodified;
        }
        let inner_loop_size = inner_uce.rolled_loop_size();
        if inner_uce.num_inline_candidates != 0 || outer_uce.num_inline_candidates != 0 {
            llrm_support::debug!("unroll-jam", "not unrolling a loop with inlinable calls");
            return Jammed::Unmodified;
        }

        // The trip count and the trip multiple.
        let mut se = ScalarEvolution::new();
        let cx = Cx {
            context: &*run.context,
            globals: run.globals,
            metadata: run.metadata,
            function: &*run.function,
            layout: run.layout,
            loops: &analyzed.loops,
            tree: &analyzed.tree,
            assumptions: &analyzed.assumptions,
        };
        let latch = l.get_loop_latch(cx.function).expect("a latch");
        let sub_latch = sub_loop.get_loop_latch(cx.function).expect("a latch");
        let outer_trip_count = se.get_small_constant_trip_count_for_exit(&cx, header, latch);
        let outer_trip_multiple = se.get_small_constant_trip_multiple_for_exit(&cx, header, latch);
        let inner_trip_count = se.get_small_constant_trip_count_for_exit(&cx, sub_loop.get_header(), sub_latch);

        // Decide if, and by how much, to unroll.
        let info = JamLoop { l, sub_loop, outer_trip_count, outer_trip_multiple, inner_trip_count, inner_loop_size };
        let (count, _explicit) = compute_unroll_and_jam_count(
            run,
            &analyzed,
            &mut se,
            &self.options,
            &pragma,
            &eph,
            &info,
            &outer_uce,
            &mut up,
        );
        if count <= 1 {
            return Jammed::Unmodified;
        }
        // The unroll factor must not be more than the trip count.
        let count = if outer_trip_count != 0 && count > outer_trip_count { outer_trip_count } else { count };
        unroll_and_jam_loop(run, expansion, header, count, outer_trip_count, outer_trip_multiple, up.unroll_remainder)
    }
}

/// `appendLoopsToWorklist` of a nest: the pop order, innermost first.
fn nest_worklist(top: Loop) -> Vec<BlockId> {
    // `appendReversedLoopsToWorklist(reverse(Loops))`: each root in reverse
    // preorder adds itself and what it holds in preorder; an element already
    // there moves to the back (`PriorityWorklist::insert`).
    let loops = loops_in_preorder(top);
    let mut worklist: Vec<BlockId> = Vec::new();
    for &root in loops.iter().rev() {
        let mut group: Vec<BlockId> = Vec::new();
        let mut pending = vec![root];
        while let Some(l) = pending.pop() {
            group.push(l.get_header());
            pending.extend(l.get_sub_loops());
        }
        for header in group {
            worklist.retain(|&one| one != header);
            worklist.push(header);
        }
    }
    worklist
}

impl FunctionPass for LoopUnrollAndJam {
    fn name(&self) -> &'static str {
        "unroll-jam"
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
        let tti = crate::looprotate::target_tti(outer.target());
        let expansion = MachineCosts { tti: Some(std::rc::Rc::clone(&tti)), minimum_size: false };
        // Only a function with a loop that holds a loop is looked at.
        let tops: Vec<BlockId> = {
            let tree = llrm_mir::domtree::DomTree::of(unit.function);
            let loops = llrm_mir::loopinfo::LoopInfo::analyze(unit.function, || &tree);
            loops
                .top_level_loops()
                .filter(|top| top.get_sub_loops().next().is_some())
                .map(|top| top.get_header())
                .collect()
        };
        if tops.is_empty() {
            return PreservedAnalyses::all();
        }
        // The loop pass manager's canonical form, on a function that is kept
        // only if a loop is unrolled and jammed (as in `loopinterchange`).
        let saved = unit.function.clone();
        crate::loopsimplify::simplified(unit.function);
        let _ = crate::lcssa::closed(unit.function);
        let mut run = Run {
            context: unit.context,
            layout: unit.layout,
            globals: &outer.globals,
            metadata: unit.metadata,
            callees: outer.callees(),
            this: unit.id,
            tti: &*tti,
            function: unit.function,
            outer: &outer,
            declared: unit.declared,
            emptied: Vec::new(),
            peeled: Default::default(),
            created: Vec::new(),
            scev: llrm_analysis::scev::ScalarEvolution::new(),
            options: UnrollOptions {
                opt_level: self.opt_level,
                opt_size: self.opt_size,
                min_size: false,
                only_when_forced: false,
                prepare_for_lto: false,
            },
        };
        let mut changed = false;
        // The nests, the last first.
        for top in tops.into_iter().rev() {
            let worklist = {
                let analyzed = Analyzed::of(&run);
                match analyzed.loops.get_loop_for(top) {
                    Some(l) => nest_worklist(l),
                    None => continue,
                }
            };
            for header in worklist.into_iter().rev() {
                // A loop the previous jams removed is not looked at.
                if !Analyzed::of(&run).loops.is_loop_header(header) {
                    continue;
                }
                if self.try_loop(&mut run, &expansion, header) != Jammed::Unmodified {
                    changed = true;
                }
            }
        }
        let emptied = std::mem::take(&mut run.emptied);
        run.function.erase_blocks(&emptied).expect("blocks merged away are empty and unnamed");
        if !changed {
            *unit.function = saved;
            return PreservedAnalyses::all();
        }
        PreservedAnalyses::none()
    }
}

#[cfg(test)]
#[path = "loopunrollandjam_tests.rs"]
mod tests;
