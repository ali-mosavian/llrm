//! Port of tests/test_jumps.py.
//!
//! `test_qglsurf_shares_all_three_zero_result_tails` waits for
//! `cfront --opt` (phase 3).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::*;
use crate::backend::frame::Frame;
use crate::model::ir::{Imm, Loc, Reg};
use crate::support::hash::IndexMap;

fn ax() -> Loc {
    Loc::Reg(Reg { register: RegId::AX, width: 2 })
}

fn bx() -> Loc {
    Loc::Reg(Reg { register: RegId::BX, width: 2 })
}

fn cx() -> Loc {
    Loc::Reg(Reg { register: RegId::CX, width: 2 })
}

fn imm(value: i64) -> Loc {
    Loc::Imm(Imm { value, width: 2, address: None })
}

fn _insn(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    target: Option<i64>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, 1)), Some(what), Vec::new(), Vec::new()))
}

fn _compare(at: i64) -> Arc<Insn> {
    _insn(at, Operation::Compare, "cmp", vec![], vec![ax(), bx()], None)
}

fn _branch(
    at: i64,
    name: &str,
    target: i64,
) -> Arc<Insn> {
    _insn(at, Operation::Branch, name, vec![], vec![], Some(target))
}

fn _jump(
    at: i64,
    target: i64,
) -> Arc<Insn> {
    _insn(at, Operation::Jump, "jmp", vec![], vec![], Some(target))
}

fn _move(
    at: i64,
    source: Loc,
) -> Arc<Insn> {
    _insn(at, Operation::Move, "mov", vec![ax()], vec![source], None)
}

fn _return(at: i64) -> Arc<Insn> {
    _insn(at, Operation::Return, "ret", vec![], vec![], None)
}

fn _inserted(one: Arc<Insn>) -> Arc<Insn> {
    Arc::new(Insn { covers: Some((one.at, one.at)), ..(*one).clone() })
}

fn block(
    at: i64,
    insns: Vec<Arc<Insn>>,
    succ: Vec<i64>,
) -> LirBlock {
    LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
}

fn body(
    name: &str,
    entry: i64,
    blocks: Vec<Block>,
) -> LirBody {
    LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default())
}

fn listed(
    body: LirBody,
    name: &str,
) -> Vec<String> {
    let procedure = masm::Procedure {
        name: name.to_owned(),
        public: true,
        far: false,
        body,
        reserve: 0,
        callees: IndexMap::default(),
        interrupt: None,
        size: false,
        entry: 0,
        stack_check: None,
        registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
    };
    masm::_procedure(&procedure, &IndexMap::default(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect()
}

fn _printed(blocks: Vec<Block>) -> Vec<String> {
    let entry = blocks[0].at;
    let lines = listed(threaded(&body("f", entry, blocks)), "_f");
    lines[1..lines.len() - 1].to_vec()
}

fn inner(lines: Vec<String>) -> Vec<String> {
    lines[1..lines.len() - 1].to_vec()
}

fn physical(result: &LirBody) -> Vec<Semantics> {
    result
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter_map(|one| one.what.clone())
        .filter(|what| what.op != Operation::Nothing)
        .collect()
}

#[test]
fn test_branch_to_a_block_that_only_jumps_goes_to_its_target() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_compare(1), _branch(2, "je", 7)], vec![7, 4]).into(),
            block(4, vec![_move(4, cx()), _return(5)], vec![]).into(),
            block(7, vec![_jump(7, 9)], vec![9]).into(),
            block(8, vec![_move(8, bx()), _return(8)], vec![]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ]),
        ["L0_1:", "cmp ax, bx", "je L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "ret"]
    );
}

#[test]
fn test_branch_over_a_jump_is_inverted() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_compare(1), _branch(2, "je", 9)], vec![9, 4]).into(),
            block(4, vec![_jump(4, 12)], vec![12]).into(),
            block(9, vec![_move(9, cx()), _return(10)], vec![]).into(),
            block(12, vec![_move(12, bx()), _return(13)], vec![]).into(),
        ]),
        ["L0_1:", "cmp ax, bx", "jne L0_12", "L0_9:", "mov ax, cx", "ret", "L0_12:", "mov ax, bx", "ret"]
    );
}

#[test]
fn test_branch_then_jump_in_one_block_is_inverted() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_compare(1), _branch(2, "je", 4), _jump(3, 9)], vec![4, 9]).into(),
            block(4, vec![_move(4, cx()), _return(5)], vec![]).into(),
            block(9, vec![_move(9, bx()), _return(10)], vec![]).into(),
        ]),
        ["L0_1:", "cmp ax, bx", "jne L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "mov ax, bx", "ret"]
    );
}

/// QB qlight left a jump after each clamp assignment.
#[test]
fn test_conditional_assignment_arm_is_placed_before_its_join() {
    let clamp = body(
        "clamp",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "jg", 4)], vec![4, 3]).into(),
            block(3, vec![_inserted(_jump(3, 7))], vec![7]).into(),
            block(4, vec![_move(4, cx()), _jump(5, 7)], vec![7]).into(),
            block(7, vec![_return(7)], vec![]).into(),
        ],
    );
    assert_eq!(
        inner(listed(threaded(&placed(&clamp).unwrap()), "_clamp")),
        ["L0_1:", "cmp ax, bx", "jle L0_3", "L0_4:", "mov ax, cx", "ret", "L0_3:", "ret"]
    );
}

/// Fresh QB D_SURF retained 189 `jcc body; jmp exit; body` pairs.
#[test]
fn test_shared_machine_pipeline_threads_the_final_branch_pair() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 4)], vec![4, 9]).into(),
            block(4, vec![_return(4)], vec![]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ],
    );

    let result = crate::flow::machine(
        &IndexMap::default(),
        Some(Rc::new(RefCell::new(Frame::new(0)))),
        None,
        Some(&IndexMap::default()),
        false,
        "386",
        &crate::backend::target::BUILT_IN,
    )
    .unwrap()
    .into_iter()
    .find(|phase| phase.class_name() == "ControlFlow")
    .unwrap()
    .transform(source)
    .unwrap();

    let real: Vec<(Operation, Option<String>, Option<i64>)> = result.blocks[0]
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|one| (one.op, one.name.as_deref().map(str::to_owned), one.target))
        .collect();
    assert_eq!(
        real,
        [(Operation::Compare, Some("cmp".to_owned()), None), (Operation::Branch, Some("jne".to_owned()), Some(9))]
    );
    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1, 4, 9]);
    let emitted = listed(result, "_f");
    assert!(!emitted.iter().any(|line| line.starts_with("jmp ")), "{emitted:?}");
}

/// Optimized QB nbody crashed after scheduling instead of reaching timing.
#[test]
fn test_loop_placement_ignores_non_emitting_instruction_markers() {
    let marker = Arc::new(Insn::new(2, Some((2, 2)), None, Vec::new(), Vec::new()));
    let source = body(
        "marker-loop",
        1,
        vec![
            block(1, vec![_jump(1, 2)], vec![2]).into(),
            block(2, vec![marker, _compare(2), _branch(2, "je", 4), _jump(2, 3)], vec![4, 3]).into(),
            block(3, vec![_jump(3, 2)], vec![2]).into(),
            block(4, vec![_return(4)], vec![]).into(),
        ],
    );

    let result = placed(&source).unwrap();

    let mut ats: Vec<i64> = result.blocks.iter().map(|block| block.at).collect();
    ats.sort_unstable();
    assert!(ats.contains(&1) && ats.contains(&4), "{ats:?}");
}

#[test]
fn test_jump_to_the_next_block_is_dropped() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_move(1, bx()), _jump(2, 4)], vec![4]).into(),
            block(4, vec![_return(4)], vec![]).into(),
        ]),
        ["L0_1:", "mov ax, bx", "ret"]
    );
}

/// SCALAR lost the three decoded bytes of its preheader jump after unrolling.
#[test]
fn test_source_owned_jump_to_fallthrough_becomes_an_anchor() {
    let jump = _insn(1, Operation::Jump, "jmp", vec![], vec![], Some(4));
    let jump = Arc::new(Insn { covers: Some((1, 4)), ..(*jump).clone() });
    let source = body("f", 1, vec![block(1, vec![jump], vec![4]).into(), block(4, vec![_return(4)], vec![]).into()]);

    let result = threaded(&source);

    // The block it fell into is one block with it (L1515): the anchor, then the
    // return.
    let kept = &result.blocks[0].insns;
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].covers, Some((1, 4)));
    assert!(kept[0].what.as_ref().is_some_and(|what| what.op == Operation::Nothing));
}

/// cfg_trim kept `jne L0_18; jmp L0_13` over an empty block 12 nothing enters.
#[test]
fn test_jump_over_a_block_nothing_reaches_is_dropped() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_move(1, bx()), _jump(2, 9)], vec![9]).into(),
            block(5, vec![], vec![]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ]),
        ["L0_1:", "mov ax, bx", "ret"]
    );
}

/// `_reachable` emptied the table of frequencies, so the next `Frequency::of`
/// worked out all 257 blocks of d_faces again (2.12 M Ir in `inverted`).
#[test]
fn test_pruning_unreachable_blocks_keeps_the_frequencies_of_the_others() {
    let mut start = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, bx()), _jump(2, 9)], vec![9]).into(),
            block(5, vec![], vec![]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ],
    );
    start.frequencies =
        Some(Arc::new(crate::model::lir::BlockFrequencies([(1, 1.0), (5, 0.5), (9, 1.0)].into_iter().collect())));
    let kept = _reachable(&start, start.blocks.clone());
    let table = &kept.frequencies.as_ref().expect("a table").0;
    assert_eq!(table.get(&1), Some(&1.0));
    assert_eq!(table.get(&9), Some(&1.0));
    assert!(crate::analysis::frequency::Frequency::violations(&kept).is_empty());
}

/// Folding branches pruned the unreachable block 5 in place and left it in
/// the kept table: the verifier said "frequency kept for block 0x5, which is
/// gone" after `jumps`, and every QB and Nib program failed to compile.
#[test]
fn test_folding_branches_drops_the_frequency_of_a_pruned_block() {
    let mut start = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, bx()), _jump(2, 9)], vec![9]).into(),
            block(5, vec![], vec![]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ],
    );
    start.frequencies =
        Some(Arc::new(crate::model::lir::BlockFrequencies([(1, 1.0), (5, 0.5), (9, 1.0)].into_iter().collect())));
    let done = branchfold::folded(&start, &std::collections::BTreeSet::new());
    assert_eq!(crate::analysis::frequency::Frequency::violations(&done), Vec::<String>::new());
}

fn nothing(
    at: i64,
    covers: Option<(i64, i64)>,
) -> Arc<Insn> {
    let what = Semantics { name: Some("".into()), ..Semantics::new(Operation::Nothing) };
    Arc::new(Insn::new(at, covers, Some(what), Vec::new(), Vec::new()))
}

/// SCALAR unrolled to 1789, then final threading lost the old loop bytes.
#[test]
fn test_unreachable_inert_source_ownership_survives_threading() {
    let owned = nothing(5, Some((5, 9)));
    let source = body(
        "f",
        1,
        vec![block(1, vec![_return(1)], vec![]).into(), block(5, vec![Arc::clone(&owned)], vec![]).into()],
    );

    let result = threaded(&source);

    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1, 5]);
    assert_eq!(result.blocks[1].insns, [owned]);
}

/// A threaded passage's source `jmp` keeps its bytes. Dropped with its block,
/// it left a hole that made the emitter refuse 11 corpus builds, which then
/// shipped unoptimized.
#[test]
fn test_a_threaded_source_jump_keeps_its_bytes() {
    let owned = Arc::new(Insn { covers: Some((7, 10)), ..(*_jump(7, 9)).clone() });
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 7)], vec![7, 4]).into(),
            block(4, vec![_return(4)], vec![]).into(),
            block(7, vec![owned], vec![9]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ],
    );

    let result = threaded(&source);

    assert_eq!(result.blocks[0].succ, [9, 4]);
    assert_eq!(result.owned_bytes(), [7, 8, 9]);
}

/// Control falls through a block of only markers. Taken for the next
/// block, rotate's loop kept a `jmp` to the instruction right after it.
#[test]
fn test_a_jump_past_a_marker_block_to_the_next_code_is_dropped() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 9), _inserted(_jump(3, 7))], vec![9, 7]).into(),
            block(5, vec![nothing(5, Some((5, 7)))], vec![]).into(),
            block(7, vec![_return(7)], vec![]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ],
    );

    let result = threaded(&source);

    assert!(!result.blocks[0].insns.iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Jump)));
}

/// A carried, non-generated LIR occurrence can have no contiguous `covers`
/// span.
#[test]
fn test_unreachable_inert_carrier_without_a_byte_span_does_not_crash_threading() {
    let carrier = nothing(5, None);
    let source = body("f", 1, vec![block(1, vec![_return(1)], vec![]).into(), block(5, vec![carrier], vec![]).into()]);

    let result = threaded(&source);

    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1]);
}

/// PARITY's fully unrolled BASIC loop left only source-map anchors. They
/// are not work: the jump threads past them, and they keep their bytes.
#[test]
fn test_reachable_inert_source_ownership_is_a_passage_that_keeps_its_bytes() {
    let owned = nothing(5, Some((5, 9)));
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_jump(1, 5)], vec![5]).into(),
            block(5, vec![Arc::clone(&owned)], vec![9]).into(),
            block(9, vec![_return(9)], vec![]).into(),
        ],
    );

    let result = threaded(&source);

    // The passage is bypassed and the return it led to is one block with the
    // block before it (L1515); the passage stays after, for its bytes.
    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1, 5]);
    assert!(result.blocks[0].succ.is_empty());
    assert_eq!(result.blocks[1].insns, [owned]);
    assert!(result.blocks[1].succ.is_empty());
}

/// Mandel's rewound coordinate made its outer header emit no bytes.
#[test]
fn test_threading_preserves_an_exact_empty_loop_header() {
    let mut source = body(
        "nested",
        1,
        vec![
            block(1, vec![_jump(1, 8)], vec![8]).into(),
            block(8, vec![], vec![10]).into(),
            block(10, vec![_compare(10), _branch(11, "je", 20)], vec![14, 20]).into(),
            block(14, vec![_move(14, cx()), _jump(15, 10)], vec![10]).into(),
            block(20, vec![_compare(20), _branch(21, "jne", 8)], vec![8, 30]).into(),
            block(30, vec![_return(30)], vec![]).into(),
        ],
    );
    source.loop_trip_counts = vec![(8, 24), (10, 32)];

    let result = threaded(&source);

    assert!(result.blocks.iter().any(|block| block.at == 8));
    let headers: std::collections::BTreeSet<i64> =
        loopy::loops(&result.blocks, Some(result.entry)).iter().map(|one| one.header).collect();
    assert_eq!(headers, [8, 10].into());
}

/// `for (;;);` -- following jumps must not go round forever.
#[test]
fn test_a_block_that_jumps_to_itself_stays() {
    assert_eq!(_printed(vec![block(1, vec![_jump(1, 1)], vec![1]).into()]), ["L0_1:", "jmp L0_1"]);
}

/// `optimized`'s round: the tails merged, then the branches settled.
fn merged(body: &LirBody) -> Result<LirBody, masm::Unprintable> {
    let merged = tailmerge::merged(body, false, None)?.unwrap_or_else(|| body.clone());
    Ok(threaded(&merged))
}

fn two_zero_tails(inserted: bool) -> LirBody {
    let fresh = |one: Arc<Insn>| if inserted { _inserted(one) } else { one };
    body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![fresh(_move(10, imm(0))), fresh(_jump(11, 30))], vec![30]).into(),
            block(20, vec![fresh(_move(20, imm(0))), fresh(_jump(21, 30))], vec![30]).into(),
            // Three instructions: not copied into the arms, which end in the
            // same move and jump and are merged.
            block(30, vec![_compare(30), _compare(31), _compare(32), _return(33)], vec![]).into(),
        ],
    )
}

/// Every arm of a join was seen whole (`Seq` of 41 items) to hash its last
/// instruction, 82 items built where the pin reads three per arm
/// (`HashEndOfMBB`, BranchFolding.cpp:362).
#[test]
fn test_arms_that_end_differently_are_not_seen_whole() {
    let arm = |at: i64, last: Arc<Insn>| {
        let mut insns: Vec<Arc<Insn>> = (0..40).map(|n| _compare(at + 100 + n)).collect();
        insns.extend([_inserted(last), _inserted(_jump(at + 50, 30))]);
        block(at, insns, vec![30]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            arm(10, _move(10, imm(0))),
            arm(20, _compare(20)),
            block(30, vec![_compare(30), _compare(31), _compare(32), _return(33)], vec![]).into(),
        ],
    );
    let source = placed(&source).unwrap();
    tailmerge::ITEMS_BUILT.with(|one| one.set(0));

    tailmerge::merged(&source, false, None).unwrap();

    let built = tailmerge::ITEMS_BUILT.with(|one| one.get());
    assert!(built <= 12, "{built} items built for two arms that cannot share a tail");
}

/// Each merge that split a block made the predecessor map again from every
/// block: a body of N merging diamonds built N+1 maps (the pin's blocks keep
/// their predecessor lists), and `lir jumps` grew x2.21 from 256 to 512
/// branches.
#[test]
fn test_merges_that_split_blocks_do_not_make_the_predecessor_map_again() {
    let diamonds: i64 = 12;
    let mut blocks: Vec<Block> = Vec::new();
    for k in 0..diamonds {
        let at = 100 * (k + 1);
        let arm = |arm: i64| {
            let own = at + arm * 10;
            block(
                own,
                vec![_compare(own + 1), _inserted(_move(own + 2, imm(0))), _inserted(_jump(own + 3, at + 50))],
                vec![at + 50],
            )
            .into()
        };
        blocks.push(block(at, vec![_compare(at), _branch(at + 1, "je", at + 20)], vec![at + 20, at + 10]).into());
        blocks.extend([arm(1), arm(2)]);
        let next = if k + 1 < diamonds { 100 * (k + 2) } else { 0 };
        blocks.push(
            if next == 0 {
                block(at + 50, vec![_compare(at + 51), _return(at + 52)], vec![])
            } else {
                block(at + 50, vec![_compare(at + 51), _jump(at + 52, next)], vec![next])
            }
            .into(),
        );
    }
    let source = placed(&body("f", 100, blocks)).unwrap();
    tailmerge::PREDECESSOR_MAPS.with(|one| one.set(0));

    let merged = tailmerge::merged(&source, false, None).unwrap().expect("the diamonds merge");

    let made = tailmerge::PREDECESSOR_MAPS.with(|one| one.get());
    let moves = merged.insns().iter().filter(|one| one.what.as_ref().is_some_and(|w| w.op == Operation::Move)).count();
    assert!(moves < 24, "premise: tails were merged ({moves} moves left of 24)");
    assert!(made <= 6, "{made} predecessor maps made for {diamonds} diamonds");
}

/// qglsurf emitted the same zero-result tail from two failure arms.
#[test]
fn test_identical_result_tails_are_merged() {
    let result = merged(&placed(&two_zero_tails(true)).unwrap()).unwrap();
    let physical = physical(&result);

    assert_eq!(physical.iter().filter(|what| what.op == Operation::Move).count(), 1);
    assert_eq!(physical.iter().filter(|what| what.op == Operation::Branch).count(), 0);
}

/// lru: three zero arms, each laid out right after its conditional branch, were
/// kept apart (3 `xor` + 3 `jmp`) because the jump taken off was assumed to
/// fall through. It cannot: the block laid out next is not its successor
/// (`getFallThrough`), so the pin merges them (`BothFallThrough`).
#[test]
fn test_zero_arms_each_after_their_branch_are_merged() {
    let arm = |at: i64| block(at, vec![_inserted(_move(at, imm(0))), _inserted(_jump(at + 1, 90))], vec![90]);
    let body = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10).into(),
            block(20, vec![_compare(20), _branch(21, "je", 40), _jump(22, 30)], vec![40, 30]).into(),
            arm(30).into(),
            block(40, vec![_compare(40), _branch(41, "je", 60), _jump(42, 50)], vec![60, 50]).into(),
            arm(50).into(),
            block(60, vec![_move(60, bx()), _return(61)], vec![]).into(),
            block(90, vec![_compare(90), _compare(91), _compare(92), _return(93)], vec![]).into(),
        ],
    );
    let result = merged(&body).unwrap();

    assert_eq!(physical(&result).iter().filter(|what| what.op == Operation::Move && what.target.is_none()).count(), 2);
}

/// The compare of a branch the merge made redundant stays, as in LLVM (T6):
/// the dead-code sweep that followed each merge cost 1.7 M Ir on d_faces and
/// changed no object.
#[test]
fn test_a_merge_leaves_the_compare_of_the_branch_it_made_redundant() {
    let result = merged(&placed(&two_zero_tails(true)).unwrap()).unwrap();

    // The source's four compares: the branch's and the three of the common
    // tail.
    assert_eq!(physical(&result).iter().filter(|what| what.op == Operation::Compare).count(), 4);
}

/// Merged tails set the same register but named different values: a later
/// read of the dropped block's value had no definition (deedlines
/// SPHEREMAPLASMA, "value#5635 is read but never defined").
#[test]
fn test_merged_tails_share_their_values() {
    let named = |one: Arc<Insn>, value: u32| Arc::new(Insn { defines: vec![value], ..(*_inserted(one)).clone() });
    let push =
        Arc::new(Insn { uses: vec![2], ..(*_insn(30, Operation::Push, "push", vec![], vec![ax()], None)).clone() });
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![named(_move(10, imm(0)), 1), _inserted(_jump(11, 30))], vec![30]).into(),
            block(20, vec![named(_move(20, imm(0)), 2), _inserted(_jump(21, 30))], vec![30]).into(),
            block(30, vec![push, _return(31)], vec![]).into(),
        ],
    );
    let result = merged(&placed(&source).unwrap()).unwrap();

    assert_eq!(result.blocks.len(), 3, "the tails were not merged");
    let undefined: Vec<String> = crate::backend::verify::verify(&result, false)
        .into_iter()
        .filter(|said| said.contains("never defined"))
        .collect();
    assert_eq!(undefined, Vec::<String>::new());
}

/// Both ways of a branch led to bare jumps to one block: threading sent each
/// to it, which left `jae L; jmp L` with the successor listed twice. A second
/// threading then took the jump to `L` and stranded `L` under the branch's
/// other reference (qcport mdl_ai -Os: "a jump to L66_287, which is nowhere").
#[test]
fn test_a_branch_whose_two_ways_were_threaded_to_one_block_is_not_left_a_branch() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "jae", 10), _jump(3, 20)], vec![10, 20]).into(),
            block(10, vec![_jump(10, 30)], vec![30]).into(),
            block(20, vec![_jump(20, 30)], vec![30]).into(),
            block(30, vec![_move(30, cx()), _return(31)], vec![]).into(),
        ],
    );

    let once = threaded(&source);
    let twice = threaded(&once);

    assert_eq!(count(&once, Operation::Branch), 0);
    let known: std::collections::BTreeSet<i64> = twice.blocks.iter().map(|block| block.at).collect();
    for one in twice.blocks.iter().flat_map(|block| block.insns.iter()) {
        if let Some(target) = one.what.as_ref().and_then(|what| what.target) {
            assert!(known.contains(&target), "a jump to {target}, which is nowhere");
        }
    }
}

/// Two arms that differ in their first instruction and share the last three:
/// `BranchFolder::TailMergeBlocks` splits the shared three off one arm and
/// sends the other to it. The merge of whole blocks left both copies: 7 moves
/// and 3 compares where 5 and 2 do.
fn arms_sharing_a_tail(shared: usize) -> LirBody {
    let tail = |at: i64| -> Vec<Arc<Insn>> {
        [_move(at, bx()), _compare(at + 1), _move(at + 2, imm(7))].into_iter().take(shared).map(_inserted).collect()
    };
    let arm = |at: i64, first: Loc| {
        let mut insns = vec![_inserted(_move(at, first))];
        insns.extend(tail(at + 1));
        insns.push(_inserted(_jump(at + 9, 40)));
        block(at, insns, vec![40]).into()
    };
    body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10, imm(1)),
            arm(20, imm(2)),
            block(30, vec![_inserted(_move(30, cx())), _inserted(_return(31))], vec![]).into(),
            block(40, vec![_return(40)], vec![]).into(),
        ],
    )
}

fn count(
    result: &LirBody,
    op: Operation,
) -> usize {
    physical(result).iter().filter(|what| what.op == op).count()
}

#[test]
fn test_a_tail_of_three_shared_by_two_arms_is_split_off_and_shared() {
    let source = arms_sharing_a_tail(3);
    assert_eq!((count(&source, Operation::Move), count(&source, Operation::Compare)), (7, 3));

    let result = tailmerge::merged(&source, false, None).unwrap().expect("nothing was merged");

    assert_eq!((count(&result, Operation::Move), count(&result, Operation::Compare)), (5, 2));
}

/// One shared instruction plus the jump taken off is two, under the three a
/// merge must save (`MinCommonTailLength`): the arms stay as they are.
#[test]
fn test_a_tail_of_one_is_not_shared() {
    let source = arms_sharing_a_tail(1);

    assert!(tailmerge::merged(&source, false, None).unwrap().is_none());
}

/// A tail of one move and two zero-code anchors was shared: `x_cordic` -O3
/// split each diamond's join into an empty block, every other arm paid a jump
/// to it, +5.0% clocks. LLVM has no anchors after allocation, so they count for
/// nothing.
#[test]
fn test_anchors_that_emit_no_code_do_not_count_toward_a_shared_tail() {
    let anchor = |at: i64| {
        let one = _inserted(nothing(at, None));
        Arc::new(Insn { defines: vec![at as u32 + 500], uses: vec![at as u32 + 600], ..(*one).clone() })
    };
    let arm = |at: i64, first: Loc| {
        let insns = vec![
            _inserted(_move(at, first)),
            _inserted(_move(at + 1, bx())),
            anchor(at + 2),
            anchor(at + 3),
            _inserted(_jump(at + 9, 40)),
        ];
        block(at, insns, vec![40]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10, imm(1)),
            arm(20, imm(2)),
            block(30, vec![_inserted(_move(30, cx())), _inserted(_return(31))], vec![]).into(),
            block(40, vec![_return(40)], vec![]).into(),
        ],
    );

    assert!(tailmerge::merged(&source, false, None).unwrap().is_none());
}

/// An anchor just before the shared tail stayed in the arm that now only jumps,
/// and `threaded` would not go around that block: `recpow` -O2 ran a `jmp`
/// after every taken `jne` to it, +0.53% clocks.
#[test]
fn test_an_anchor_before_the_shared_tail_leaves_the_arm_with_it() {
    let anchored = |at: i64| {
        let one = _inserted(nothing(at, None));
        Arc::new(Insn { defines: vec![at as u32 + 500], uses: vec![at as u32 + 600], ..(*one).clone() })
    };
    let arm = |at: i64, first: Loc| {
        let mut insns = vec![_inserted(_move(at, first)), anchored(at + 1)];
        insns.extend([_move(at + 2, bx()), _compare(at + 3), _move(at + 4, imm(7))].into_iter().map(_inserted));
        insns.push(_inserted(_jump(at + 9, 40)));
        block(at, insns, vec![40]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10, imm(1)),
            arm(20, imm(2)),
            block(30, vec![_inserted(_move(30, cx())), _inserted(_return(31))], vec![]).into(),
            block(40, vec![_return(40)], vec![]).into(),
        ],
    );

    let result = tailmerge::merged(&source, false, None).unwrap().expect("nothing was merged");

    let anchors = result.blocks.iter().flat_map(|block| block.insns.iter()).filter(|one| super::_inert(one)).count();
    assert_eq!(anchors, 1, "the arm that now jumps still holds its anchor");
}

/// The stackifier's `sahf` is an inserted `Nothing` that emits a byte, and was
/// taken for an anchor: the shared tail began after it, one arm lost its
/// `sahf`, and `d_faces` -Os computed other results (feat/parity bisect).
#[test]
fn test_a_sahf_is_not_an_anchor_and_stays_on_every_path() {
    let sahf = |at: i64| {
        let what = Semantics { name: Some("sahf".into()), ..Semantics::new(Operation::Nothing) };
        _inserted(Arc::new(Insn::new(at, None, Some(what), Vec::new(), Vec::new())))
    };
    let arm = |at: i64, first: Loc| {
        let insns = vec![
            _inserted(_move(at, first)),
            sahf(at + 1),
            _inserted(_move(at + 2, bx())),
            _inserted(_jump(at + 9, 40)),
        ];
        block(at, insns, vec![40]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10, imm(1)),
            arm(20, imm(2)),
            block(40, vec![_return(40)], vec![]).into(),
        ],
    );

    let result = tailmerge::merged(&source, true, None).unwrap().expect("nothing was merged");

    let sahfs = |at: i64| {
        let mut path = 0;
        let mut at = at;
        loop {
            let block = result.blocks.iter().find(|one| one.at == at).expect("a block");
            path += block
                .insns
                .iter()
                .filter(|one| one.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("sahf")))
                .count();
            match block.succ.as_slice() {
                [next] => at = *next,
                _ => return path,
            }
        }
    };
    assert_eq!((sahfs(10), sahfs(20)), (1, 1), "an arm lost its sahf");
}

/// A block with a branch before its last two (`jne A; ...; jne B; jmp C`, B
/// laid out next) had its final jump taken for a fall-through and the branch
/// reversed: three ways out and no instruction choosing among them, and
/// QCport's `menu.c`, `triggers.c` and `sb_build.c` failed the verifier at
/// -O1/-O2/-O3.
#[test]
fn test_a_block_with_an_inner_branch_keeps_a_way_out_the_verifier_can_read() {
    let source = body(
        "f",
        1,
        vec![
            block(
                1,
                vec![_compare(1), _branch(2, "jne", 20), _compare(3), _branch(4, "jne", 77), _jump(5, 75)],
                vec![20, 77, 75],
            )
            .into(),
            block(77, vec![_move(77, imm(1)), _return(78)], vec![]).into(),
            block(75, vec![_move(75, imm(2)), _return(76)], vec![]).into(),
            block(20, vec![_move(20, imm(3)), _return(21)], vec![]).into(),
        ],
    );

    let result = threaded(&source);

    for block in &result.blocks {
        masm::_falls_to(block, "f").unwrap_or_else(|why| panic!("{why:?}"));
    }
}

/// A branch before the shared tail named a block its arm's successors no longer
/// listed after the merge: the block was dropped as unreachable and the branch
/// jumped nowhere (QCport `sb_build.c` -O2, "a jump to L1_149, which is
/// nowhere").
#[test]
fn test_a_branch_before_the_shared_tail_keeps_its_target_a_successor() {
    let arm = |at: i64, first: Loc| {
        let mut insns = vec![_inserted(_move(at, first)), _compare(at + 1), _branch(at + 2, "jne", 50)];
        insns.extend([_move(at + 3, bx()), _compare(at + 4), _move(at + 5, imm(7))].into_iter().map(_inserted));
        insns.push(_inserted(_jump(at + 9, 40)));
        block(at, insns, vec![50, 40]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10, imm(1)),
            arm(20, imm(2)),
            block(40, vec![_return(40)], vec![]).into(),
            block(50, vec![_move(50, imm(9)), _return(51)], vec![]).into(),
        ],
    );

    let result = tailmerge::merged(&source, false, None).unwrap().expect("nothing was merged");

    for block in &result.blocks {
        for one in &block.insns {
            let Some(target) = one
                .what
                .as_ref()
                .filter(|w| matches!(w.op, Operation::Branch | Operation::Jump))
                .and_then(|w| w.target)
            else {
                continue;
            };
            assert!(block.succ.contains(&target), "block {} names {target} but leaves for {:?}", block.at, block.succ);
        }
    }
}

/// An unnamed pseudo that clobbers a register emits no bytes but is not an
/// anchor: `_inert` asked the name alone and would have dropped it.
#[test]
fn test_an_unnamed_flag_clobbering_pseudo_is_not_inert() {
    let pseudo = Insn { clobbers: [RegId::AX].into(), ..(*_inserted(nothing(5, None))).clone() };
    assert!(!super::_inert(&pseudo));
    assert!(super::_inert(&_inserted(nothing(5, None))));
}

/// `jne T; jmp F` left T, a block that only jumps, in place: `recpow` -O2 ran
/// an extra `jmp` after every taken `jne`, +0.14% clocks and 15 bytes.
#[test]
fn test_a_branch_before_the_final_jump_goes_through_a_block_that_only_jumps() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "jne", 20), _jump(3, 30)], vec![20, 30]).into(),
            block(70, vec![_inserted(_move(70, cx())), _inserted(_return(71))], vec![]).into(),
            block(30, vec![_compare(30), _branch(31, "je", 70), _jump(32, 60)], vec![70, 60]).into(),
            block(20, vec![_inserted(_jump(20, 60))], vec![60]).into(),
            block(60, vec![_inserted(_move(60, bx())), _inserted(_return(61))], vec![]).into(),
        ],
    );

    let result = threaded(&source);

    let first = result.blocks.iter().find(|block| block.at == 1).expect("the entry stays");
    assert!(first.succ.contains(&60) && !first.succ.contains(&20), "{:?}", first.succ);
}

/// D1: the faithful merge turned three arms of a loop that each carry the same
/// 5-instruction latch into one shared latch and a jump to it, one more jump a
/// trip (`x_switch` -O2/-O3, +0.96% clocks). The blocks of a loop are kept
/// whole for speed.
#[test]
fn test_speed_keeps_a_cycle_block_that_ends_in_a_branch_whole() {
    let arm = |at: i64, first: Loc| {
        let mut insns = vec![_inserted(_move(at, first))];
        insns.extend(
            [_move(at + 1, bx()), _compare(at + 2), _move(at + 3, imm(7)), _compare(at + 4)].into_iter().map(_inserted),
        );
        insns.push(_inserted(_branch(at + 5, "jne", 40)));
        insns.push(_inserted(_jump(at + 9, 50)));
        block(at, insns, vec![40, 50]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 5)], vec![20, 5]).into(),
            block(5, vec![_compare(5), _branch(6, "je", 25), _jump(7, 10)], vec![25, 10]).into(),
            block(96, vec![_return(96)], vec![]).into(),
            arm(10, imm(1)),
            block(98, vec![_return(98)], vec![]).into(),
            arm(20, imm(2)),
            block(94, vec![_return(94)], vec![]).into(),
            arm(25, imm(3)),
            block(97, vec![_return(97)], vec![]).into(),
            block(
                40,
                vec![
                    _inserted(_move(40, cx())),
                    _inserted(_move(42, bx())),
                    _inserted(_move(43, cx())),
                    _inserted(_jump(41, 1)),
                ],
                vec![1],
            )
            .into(),
            block(95, vec![_return(95)], vec![]).into(),
            block(
                50,
                vec![
                    _inserted(_move(50, bx())),
                    _inserted(_move(52, cx())),
                    _inserted(_move(53, bx())),
                    _inserted(_jump(51, 1)),
                ],
                vec![1],
            )
            .into(),
            block(99, vec![_return(99)], vec![]).into(),
        ],
    );
    assert!(tailmerge::merged(&source, true, None).unwrap().is_some(), "the premise: for size the merge applies here");

    let result = optimized(&source, cpu::profile("486").unwrap(), None).unwrap();

    assert_eq!(
        count(&result, Operation::Compare),
        count(&source, Operation::Compare),
        "{:?}",
        result
            .blocks
            .iter()
            .map(|b| (
                b.at,
                b.succ.clone(),
                b.insns.iter().map(|i| i.what.as_ref().map(|w| format!("{:?}", w.op))).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}

/// Blocks that end the function share the tail they end in, the return
/// included.
#[test]
fn test_two_blocks_that_return_share_their_last_three_instructions() {
    let ends = |at: i64, first: Loc| {
        block(
            at,
            vec![
                _inserted(_move(at, first)),
                _inserted(_move(at + 1, bx())),
                _inserted(_compare(at + 2)),
                _inserted(_return(at + 3)),
            ],
            vec![],
        )
        .into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            ends(10, imm(1)),
            ends(20, imm(2)),
            block(30, vec![_inserted(_move(30, cx())), _inserted(_return(31))], vec![]).into(),
        ],
    );
    assert_eq!(count(&source, Operation::Return), 3);

    let result = tailmerge::merged(&source, false, None).unwrap().expect("nothing was merged");

    assert_eq!(count(&result, Operation::Return), 2);
}

fn failing_blocks() -> LirBody {
    let fails = |at: i64| {
        block(
            at,
            vec![
                _inserted(_move(at, imm(11))),
                _inserted(_insn(at + 1, Operation::Call, "call", vec![], vec![], None)),
            ],
            vec![],
        )
        .into()
    };
    body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            fails(10),
            fails(20),
            block(30, vec![_inserted(_return(30))], vec![]).into(),
        ],
    )
}

/// jpeg_single emitted `push 11; call B$SERR` once per failed check, 8 copies
/// (m16 -Os, tens of bytes each); a block ending in a call was never merged.
#[test]
fn test_blocks_ending_in_a_call_to_the_same_routine_share_it() {
    let source = failing_blocks();
    assert_eq!(count(&source, Operation::Call), 2);
    let calls = IndexMap::from_iter([(11, "B$SERR".to_owned()), (21, "B$SERR".to_owned())]);

    let result = tailmerge::merged(&source, false, Some(&calls)).unwrap().expect("nothing was merged");

    assert_eq!(count(&result, Operation::Call), 1);
}

/// The callee is not in the instruction: examples/channels merged `call N$PS`
/// into `call N$PU2` (the same registers, the same clobbers) and ran wrong.
#[test]
fn test_blocks_ending_in_calls_to_different_routines_are_not_merged() {
    let source = failing_blocks();
    let calls = IndexMap::from_iter([(11, "N$PS".to_owned()), (21, "N$PU2".to_owned())]);

    assert!(tailmerge::merged(&source, false, Some(&calls)).unwrap().is_none());
    assert!(tailmerge::merged(&source, false, None).unwrap().is_none(), "no names: nothing is known to match");
}

/// Decoded tails cannot share one copy without reconciling source maps.
#[test]
fn test_identical_source_owned_tails_keep_their_distinct_anchors() {
    let result = merged(&placed(&two_zero_tails(false)).unwrap()).unwrap();

    let moves = result
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Move))
        .count();
    assert_eq!(moves, 2);
}

/// Fresh frontends inherited C's two identical failure-result tails.
#[test]
fn test_shared_machine_pipeline_merges_fresh_identical_tails() {
    let result = crate::flow::machine(
        &IndexMap::default(),
        Some(Rc::new(RefCell::new(Frame::new(0)))),
        None,
        Some(&IndexMap::default()),
        false,
        "386",
        &crate::backend::target::BUILT_IN,
    )
    .unwrap()
    .into_iter()
    .find(|phase| phase.class_name() == "ControlFlow")
    .unwrap()
    .transform(two_zero_tails(true))
    .unwrap();
    let physical = physical(&result);

    assert_eq!(physical.iter().filter(|what| what.op == Operation::Move).count(), 1);
    assert_eq!(physical.iter().filter(|what| what.op == Operation::Branch).count(), 0);
}

/// choose's folded arms joined through a two-byte `jmp` to `pop bp; retf`.
#[test]
fn test_byte_neutral_return_duplication_removes_a_join_jump() {
    let source = body(
        "choose",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![_move(10, imm(8))], vec![30]).into(),
            block(30, vec![_inserted(_return(30))], vec![]).into(),
            block(20, vec![_move(20, imm(10)), _inserted(_jump(21, 30))], vec![30]).into(),
        ],
    );

    let result = duplicated_returns(source, &|| 1);
    let physical: Vec<Semantics> =
        result.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| one.what.clone()).collect();

    assert!(result.blocks.iter().all(|block| block.at != 30));
    assert_eq!(physical.iter().filter(|one| one.op == Operation::Return).count(), 2);
    assert!(physical.iter().all(|one| one.op != Operation::Jump));
}

/// The leave sequence was encoded for every procedure (17.5 M on `d_faces` -O2,
/// 11 procedures), also those whose one return has nothing to duplicate into.
#[test]
fn test_return_duplication_asks_for_the_overhead_only_for_a_tail_with_two_jumping_parents() {
    let asked = std::cell::Cell::new(0);
    let counted = || {
        asked.set(asked.get() + 1);
        1
    };
    let single = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![_move(10, ax())], vec![30]).into(),
            block(20, vec![_move(20, bx())], vec![30]).into(),
            block(30, vec![_inserted(_return(30))], vec![]).into(),
        ],
    );
    duplicated_returns(single, &counted);
    assert_eq!(asked.get(), 0);

    let joined = body(
        "g",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![_move(10, imm(8))], vec![30]).into(),
            block(30, vec![_inserted(_return(30))], vec![]).into(),
            block(20, vec![_move(20, imm(10)), _inserted(_jump(21, 30))], vec![30]).into(),
        ],
    );
    duplicated_returns(joined, &counted);
    assert_eq!(asked.get(), 1);
}

/// A larger return tail, or one owning source bytes, stays shared.
#[test]
fn test_return_duplication_rejects_growth_and_source_owned_tails() {
    let candidate = |tail: Vec<Arc<Insn>>| {
        body(
            "f",
            1,
            vec![
                block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
                block(10, vec![_move(10, ax())], vec![30]).into(),
                block(30, tail, vec![]).into(),
                block(20, vec![_move(20, bx()), _inserted(_jump(21, 30))], vec![30]).into(),
            ],
        )
    };

    let large = candidate(vec![_inserted(_move(30, imm(1234))), _inserted(_return(31))]);
    let owned = candidate(vec![_return(30)]);

    // Python `is`: nothing changed, so the result equals the input.
    assert_eq!(duplicated_returns(large.clone(), &|| 1), large);
    assert_eq!(duplicated_returns(owned.clone(), &|| 1), owned);
}

/// The raise lays a C loop out test first, so entered at its body the
/// latch still jumped back to the test every pass.
#[test]
fn test_loop_test_is_placed_after_its_latch() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_jump(1, 8)], vec![8]).into(),
            block(4, vec![_compare(4), _branch(5, "je", 23)], vec![23, 8]).into(),
            block(8, vec![_move(8, cx()), _jump(9, 4)], vec![4]).into(),
            block(23, vec![_return(23)], vec![]).into(),
        ],
    );
    assert_eq!(
        inner(listed(threaded(&placed(&source).unwrap()), "_f")),
        ["L0_1:", "L0_8:", "mov ax, cx", "cmp ax, bx", "jne L0_8", "L0_23:", "ret"]
    );
}

/// A loop entered by falling into its test is placed with the test on top
/// (`rotateLoop` leaves a loop whose top is fallen into as often as it exits)
/// and the test copied into the latch (tail duplication): the loop `llc -O2`
/// gives, a guard and one branch per pass.
#[test]
fn test_a_loop_entered_by_falling_into_its_test_keeps_it_on_top() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, cx())], vec![3]).into(),
            block(3, vec![_compare(3), _branch(4, "jge", 17)], vec![17, 8]).into(),
            block(8, vec![_move(8, bx()), _jump(9, 3)], vec![3]).into(),
            block(17, vec![_return(17)], vec![]).into(),
        ],
    );
    assert_eq!(
        inner(listed(threaded(&placed(&source).unwrap()), "_f")),
        [
            "L0_1:",
            "mov ax, cx",
            "cmp ax, bx",
            "jge L0_17",
            "L0_8:",
            "mov ax, bx",
            "cmp ax, bx",
            "jl L0_8",
            "L0_17:",
            "ret"
        ]
    );
}

/// Two blocks that both go to the same two blocks are a trellis: the heaviest
/// pair of edges that share no source decide the fall-throughs
/// (`getBestTrellisSuccessor`). Here `10 -> 40` (0.35) and `20 -> 30` (0.3)
/// beat `10 -> 30` and `20 -> 40`, so 40 follows 10 and the source order
/// (30 first) is broken; `llc -O2` on the same CFG puts the rare-first `b`
/// before `a` as well. Without it 30 followed 10.
#[test]
fn test_a_trellis_places_the_heaviest_pair_of_edges_as_fall_throughs() {
    let mut source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![10, 20]).into(),
            block(10, vec![_compare(10), _branch(11, "je", 40)], vec![30, 40]).into(),
            block(20, vec![_compare(20), _branch(21, "je", 40)], vec![30, 40]).into(),
            block(30, vec![_return(30)], vec![]).into(),
            block(40, vec![_return(40)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    for (edge, chance) in
        [((1, 10), 0.5), ((1, 20), 0.5), ((10, 30), 0.3), ((10, 40), 0.7), ((20, 30), 0.6), ((20, 40), 0.4)]
    {
        source.odds.taken.insert(edge, fixed(chance));
    }
    let order = placed(&source).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    let at = |block: i64| order.iter().position(|one| *one == block).unwrap();
    assert_eq!(at(40), at(10) + 1, "{order:?}");
}

/// A loop its entry rarely reaches, the entry falling to a likelier block: no
/// fall-through into the loop to keep, so its exit test goes last and the
/// body first (`rotateLoop`; `llc -O2` on the same CFG puts the latch before
/// the header). Left on top, every pass paid a `jmp` back to the test.
#[test]
fn test_a_loop_nothing_falls_into_is_rotated_to_end_at_its_exit_test() {
    let mut source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 3)], vec![3, 20]).into(),
            block(3, vec![_compare(3), _branch(4, "je", 30)], vec![30, 8]).into(),
            block(8, vec![_move(8, bx()), _jump(9, 3)], vec![3]).into(),
            block(20, vec![_return(20)], vec![]).into(),
            block(30, vec![_return(30)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    source.odds.taken.insert((1, 3), fixed(0.3));
    source.odds.taken.insert((1, 20), fixed(0.7));
    let order = placed(&source).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    assert_eq!(order, vec![1, 20, 8, 3, 30]);
}

/// The block the loop leaves for follows the test.
#[test]
fn test_loop_test_is_followed_by_the_block_it_leaves_for() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, cx())], vec![3]).into(),
            block(3, vec![_compare(3), _branch(4, "jge", 20)], vec![20, 8]).into(),
            block(8, vec![_move(8, bx()), _jump(9, 3)], vec![3]).into(),
            block(12, vec![_return(12)], vec![]).into(),
            block(20, vec![_move(20, cx()), _jump(21, 12)], vec![12]).into(),
        ],
    );
    assert_eq!(
        inner(listed(threaded(&placed(&source).unwrap()), "_f")),
        [
            "L0_1:",
            "mov ax, cx",
            "cmp ax, bx",
            "jge L0_20",
            "L0_8:",
            "mov ax, bx",
            "cmp ax, bx",
            "jl L0_8",
            "L0_20:",
            "mov ax, cx",
            "ret",
        ]
    );
}

/// pal_bestfit left its latch until after the rotated loop header.
#[test]
fn test_loop_trace_is_kept_before_its_exit() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, cx()), _jump(2, 12)], vec![12]).into(),
            block(12, vec![_compare(12), _branch(13, "jge", 73)], vec![73, 16]).into(),
            block(16, vec![_compare(16), _branch(17, "jge", 68)], vec![68, 57]).into(),
            block(57, vec![_compare(57), _branch(58, "jne", 68)], vec![68, 73]).into(),
            block(68, vec![_move(68, bx()), _jump(69, 12)], vec![12]).into(),
            block(73, vec![_return(73)], vec![]).into(),
        ],
    );
    let printed = inner(listed(threaded(&placed(&source).unwrap()), "_f"));
    assert!(!(0..printed.len() - 1).any(|index| printed[index].starts_with('j')
        && !printed[index].starts_with("jmp ")
        && printed[index + 1].starts_with("jmp ")));
}

/// A diamond, the fall-through arm 10 taken with `hot`, the branch arm 20
/// with the rest; both rejoin at 30.
fn weighted_diamond(hot: f64) -> LirBody {
    diamond_joined(hot, vec![_move(30, imm(3)), _move(31, imm(4)), _move(32, imm(5)), _return(33)])
}

/// `weighted_diamond` with the join `insns`.
fn diamond_joined(
    hot: f64,
    join: Vec<Arc<Insn>>,
) -> LirBody {
    let mut made = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![_move(10, imm(1)), _jump(11, 30)], vec![30]).into(),
            block(20, vec![_move(20, imm(2)), _jump(21, 30)], vec![30]).into(),
            block(30, join, vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    made.odds.taken.insert((1, 10), fixed(hot));
    made.odds.taken.insert((1, 20), fixed(1.0 - hot));
    made
}

/// A diamond is placed as `MachineBlockPlacement` does (`llc -O2
/// -tail-dup-placement=false` on the same CFG gives each order): the likelier
/// arm follows the branch; its join follows it unless the other arm is a real
/// alternative (`hasBetterLayoutPredecessor`), else that arm goes first and
/// the likelier one jumps over it. At even odds the first successor wins.
/// The gcc-era rules this replaced kept the likelier arm second.
#[test]
fn test_a_diamond_is_placed_as_machine_block_placement_does() {
    let order = |hot: f64| placed(&weighted_diamond(hot)).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    assert_eq!(order(0.9), vec![1, 10, 30, 20]);
    assert_eq!(order(0.7), vec![1, 10, 20, 30]);
    assert_eq!(order(0.5), vec![1, 20, 10, 30]);
    assert_eq!(order(0.375), vec![1, 20, 10, 30]);
    assert_eq!(order(0.1), vec![1, 20, 30, 10]);
}

/// A join of one instruction is copied into both arms and its block goes
/// (`TailDuplicator`, `-tail-dup-placement` on): `llc -O2` on the same CFG
/// gives each order and no join. The gcc-era pass copied it after placing.
#[test]
fn test_a_join_of_one_instruction_is_copied_into_both_arms() {
    let order = |hot: f64| {
        let made = placed(&diamond_joined(hot, vec![_return(30)])).unwrap();
        for block in made.blocks.iter().filter(|one| [10, 20].contains(&one.at)) {
            assert!(block.succ.is_empty(), "arm {} still goes to the join", block.at);
        }
        made.blocks.iter().map(|one| one.at).collect::<Vec<_>>()
    };
    assert_eq!(order(0.9), vec![1, 10, 20]);
    assert_eq!(order(0.7), vec![1, 10, 20]);
    assert_eq!(order(0.5), vec![1, 20, 10]);
    assert_eq!(order(0.1), vec![1, 20, 10]);
}

/// A join of two instructions is copied into the arms for speed and kept for
/// size (`TailDupSize` 2 against 1), the arms jumping to it.
#[test]
fn test_for_size_a_join_of_two_instructions_is_not_copied() {
    let join = || vec![_move(30, imm(3)), _return(31)];
    let speed = placed(&diamond_joined(0.9, join())).unwrap();
    assert!(speed.blocks.iter().all(|one| one.at != 30), "copied for speed");
    let size = _placed(&diamond_joined(0.9, join()), true, false).unwrap();
    assert!(size.blocks.iter().any(|one| one.at == 30), "kept for size");
}

/// At -O3 a join of four instructions is copied (`TailDupPlacementAggressive
/// Threshold`); at -O2 it is kept.
#[test]
fn test_at_o3_a_join_of_four_instructions_is_copied() {
    let join = || vec![_move(30, imm(3)), _move(31, imm(4)), _move(32, imm(5)), _return(33)];
    let o2 = _placed(&diamond_joined(0.9, join()), false, false).unwrap();
    assert!(o2.blocks.iter().any(|one| one.at == 30), "kept at -O2");
    let o3 = _placed(&diamond_joined(0.9, join()), false, true).unwrap();
    assert!(o3.blocks.iter().all(|one| one.at != 30), "copied at -O3");
}

/// Only a block that goes to the join and nowhere else takes a copy: one that
/// branches to it keeps the edge, and the join stays for it.
#[test]
fn test_a_join_is_not_copied_into_a_block_that_branches_to_it() {
    let mut source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            block(10, vec![_move(10, imm(1)), _jump(11, 30)], vec![30]).into(),
            block(20, vec![_compare(20), _branch(21, "je", 30), _jump(22, 40)], vec![30, 40]).into(),
            block(30, vec![_return(30)], vec![]).into(),
            block(40, vec![_move(40, imm(4)), _move(41, imm(5)), _move(42, imm(6)), _return(43)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    source.odds.taken.insert((20, 30), fixed(0.9));
    source.odds.taken.insert((20, 40), fixed(0.1));
    let result = placed(&source).unwrap();
    let succ = |at: i64| result.blocks.iter().find(|one| one.at == at).map(|one| one.succ.to_vec());
    assert_eq!(succ(10), Some(vec![]), "the jumping arm takes a copy");
    assert_eq!(succ(20), Some(vec![30, 40]), "the branching one keeps its edge");
    assert!(result.blocks.iter().any(|one| one.at == 30), "the join stays for it");
}

/// A copied test keeps the tail's odds on its own edges: a loop of 31 trips in
/// 32 still leaves by one edge in 32 from the copy at the latch.
#[test]
fn test_a_copied_test_keeps_its_loops_odds() {
    let mut source = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, cx())], vec![3]).into(),
            block(3, vec![_compare(3), _branch(4, "jge", 17)], vec![17, 8]).into(),
            block(8, vec![_move(8, bx()), _jump(9, 3)], vec![3]).into(),
            block(17, vec![_return(17)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    source.odds.taken.insert((3, 17), fixed(1.0 / 32.0));
    source.odds.taken.insert((3, 8), fixed(31.0 / 32.0));
    let result = placed(&source).unwrap();
    let latch = result.blocks.iter().find(|one| one.at == 8).unwrap();
    assert_eq!(latch.succ.to_vec(), vec![17, 8], "the latch took the test");
    let leaves = result.odds.chance(8, &latch.succ, 17);
    assert!((leaves - 1.0 / 32.0).abs() < 1e-6, "{leaves}");
}

/// Size places as speed does, but for a loop's top (`findBestLoopTop`).
#[test]
fn test_for_size_a_diamond_is_placed_as_for_speed() {
    let order =
        |body: LirBody| _placed(&body, true, false).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    assert_eq!(order(weighted_diamond(0.7)), vec![1, 10, 20, 30]);
    assert_eq!(order(weighted_diamond(0.9)), vec![1, 10, 30, 20]);
}

/// A loop body's `jmp` back to the test ran every trip: deedlines' blobs
/// loops executed one per pass. Copied, the test runs where the jump did,
/// and the copy's `jmp` to the exit only when the loop leaves (mark 8:
/// 9,474 ms to 9,069). The copy owns no bytes; the test keeps its own.
/// A block ending `je out; jmp test` took the test's successors for its own
/// and lost `out`: pipeline.nib's `je` led nowhere ("a jump to L2_162,
/// which is nowhere").
/// A `jmp`'s edge carries no odds, being certain. Its copied test took none
/// either, and `Frequency` split the copy's branch evenly: the loop it
/// closes ran twice where it ran 32 times, and qbdemo's PLASMA read 16
/// times cheaper with its code unchanged.
/// A loop test reached from outside the loop by a `jmp` and by a `je`: a
/// copy at the `jmp` enters the loop at its body while the `je` still
/// enters at the test. deedlines' `__main` became irreducible that way and its
/// estimate read "executes an unbounded amount", dropping 777,870 instructions
/// from the sum.
/// SPHEREMAPLASMA's block 390: a loop that leaves by an edge with no
/// stated odds (`jne` stated, its exit implied), into a `jmp` to a test
/// split 3:5. The copy's edges took no odds, `Frequency` split the exit
/// evenly, and the loop nest past `je` read 2.7% hotter (#217).
/// phielim split FRACLINE's self-loop 42 -> 42 through a landing block and
/// kept the old edge's odds for an undo; threading the landing's `jmp`
/// back to 42 added its odds to that kept entry, 0.97 + 0.97: the loop
/// read as never leaving and qbdemo's FRACLINE ran 355 million times
/// over, 22 thousand in truth.
#[test]
fn test_threading_a_split_loop_back_keeps_its_odds() {
    let mut split = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, imm(0))], vec![42]).into(),
            block(42, vec![_move(42, imm(1)), _branch(43, "jne", 50)], vec![50, 60]).into(),
            block(60, vec![_return(60)], vec![]).into(),
            block(50, vec![_jump(50, 42)], vec![42]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    split.odds.taken.insert((42, 42), fixed(31.0 / 32.0));
    split.odds.taken.insert((42, 50), fixed(31.0 / 32.0));
    let back = threaded(&split);
    assert!(
        back.blocks.iter().find(|one| one.at == 42).is_some_and(|one| one.succ.contains(&42)),
        "premise: threaded back to itself"
    );
    let runs = crate::analysis::frequency::Frequency::of(&back).block(42);
    assert!((runs - 32.0).abs() < 1e-3, "{runs}");
}

/// A loop whose trips are proven is tested once a trip. Its test copied into
/// the loop nested in it ran once a visit and the estimate lost the proven
/// count: sieve's bench_sieve read 45046 before ControlFlow and 5834 after, for
/// the same code.
#[test]
fn test_a_loops_proven_test_is_not_copied_into_a_loop_inside_it() {
    use crate::backend::regalloc_input::{Calls, before_phase};
    let work = |body: &LirBody| {
        crate::backend::executed::executed(body).map(|done| done.instructions + done.memory).expect("finite")
    };
    let (body, mut phases) = before_phase(Calls::Everything, "sievejumps.ll", "bench_sieve", "486", "ControlFlow");
    let before = work(&body);
    let after = work(&phases[0].transform(body).expect("places"));
    assert!(after >= 0.75 * before, "{before} before ControlFlow, {after} after");
}

/// Each change threading made copied every block, twice, and went back to the
/// first: a body of n jumps to the next block was n copies of n blocks, 2.1 s
/// of compiling 800 blocks (#560). A change edits in place. Measured as the
/// work, not the time (a loaded machine failed the old wall-clock bound, #913):
/// the blocks cloned grow with n, not with n squared.
#[test]
fn test_threading_a_long_run_of_jumps_does_not_copy_the_body_for_each() {
    let cloned = |n: i64| {
        let blocks: Vec<Block> = (0..n)
            .map(|at| block(at, vec![_move(at * 2, imm(at)), _jump(at * 2 + 1, at + 1)], vec![at + 1]))
            .chain([block(n, vec![_return(n * 2)], vec![])])
            .collect();
        let body = body("f", 0, blocks);
        let before = crate::model::lir::BLOCK_CLONES.with(std::cell::Cell::get);
        let threaded = threaded(&body);
        let copies = crate::model::lir::BLOCK_CLONES.with(std::cell::Cell::get) - before;
        assert!(
            threaded.blocks.iter().take(n as usize).all(|one| _real(one)
                .iter()
                .all(|insn| insn.what.as_ref().is_none_or(|what| what.op != Operation::Jump))),
            "a fall-through jump is left"
        );
        copies
    };
    let (small, large) = (cloned(500), cloned(1000));
    assert!(large <= 3 * small, "{small} block copies for 500 blocks, {large} for 1,000: more than linear");
}

/// `jne next; jmp elsewhere; next:` ran a jump on the path that falls to `next`
/// (hanoi: 3601 `jmp` in 99200 instructions, 72 sites in 34 of the 66
/// programs): one `je elsewhere` falls into `next` instead.
#[test]
fn test_a_branch_taken_to_the_next_block_then_a_jump_is_one_opposite_branch() {
    let body = LirBody::new(
        "f",
        1,
        vec![
            LirBlock {
                succ: vec![2, 3].into(),
                ..LirBlock::new(1, vec![_compare(1), _branch(2, "jne", 2), _jump(3, 3)])
            }
            .into(),
            LirBlock { succ: vec![].into(), ..LirBlock::new(2, vec![_return(4)]) }.into(),
            LirBlock { succ: vec![].into(), ..LirBlock::new(3, vec![_return(5)]) }.into(),
        ],
        IndexMap::default(),
        IndexMap::default(),
    );
    let got = inverted(&body, false);
    let names: Vec<(String, Option<i64>)> = got.blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().unwrap())
        .map(|what| (what.name.as_deref().map(str::to_owned).unwrap(), what.target))
        .collect();
    assert_eq!(names, vec![("cmp".to_owned(), None), ("je".to_owned(), Some(3))]);
}

/// A block with two branches and a jump (a three-way split) has three
/// successors; flipping its second branch would leave two of them with no
/// instruction choosing (queens nib: "block 0 leaves for (13, 20, 14)").
#[test]
fn test_a_three_way_block_keeps_its_jump() {
    let body = LirBody::new(
        "f",
        1,
        vec![
            LirBlock {
                succ: vec![4, 2, 3].into(),
                ..LirBlock::new(1, vec![_compare(1), _branch(2, "je", 4), _branch(3, "jne", 2), _jump(4, 3)])
            }
            .into(),
            LirBlock { succ: vec![].into(), ..LirBlock::new(2, vec![_return(5)]) }.into(),
            LirBlock { succ: vec![].into(), ..LirBlock::new(3, vec![_return(6)]) }.into(),
            LirBlock { succ: vec![].into(), ..LirBlock::new(4, vec![_return(7)]) }.into(),
        ],
        IndexMap::default(),
        IndexMap::default(),
    );
    assert_eq!(inverted(&body, false).blocks[0].insns.len(), 4);
}

/// `threaded` restarted its scan at the first block after every change: a
/// function of N blocks that each end in a jump to the next took N steps of
/// up to N blocks (d_faces: 210 steps over 243 blocks, 0.45% of the compile;
/// sc -O1 1.1%). It goes on from the changed block and repeats from the top
/// only while a pass changed something.
#[test]
fn test_threading_a_chain_of_removable_jumps_visits_blocks_a_few_times_not_n_times() {
    let count: i64 = 200;
    let mut blocks: Vec<Block> = (0..count)
        .map(|at| {
            block(
                at * 0x10,
                vec![_move(at * 0x10, imm(at)), _inserted(_jump(at * 0x10 + 1, (at + 1) * 0x10))],
                vec![(at + 1) * 0x10],
            )
        })
        .collect();
    blocks.push(block(count * 0x10, vec![_return(count * 0x10)], Vec::new()).into());
    let source = body("f", 0, blocks);
    super::branchfold::VISITS.with(|one| one.set(0));
    let result = threaded(&source);
    let jumps = result
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Jump));
    assert_eq!(jumps.count(), 0, "premise: every jump to the next block goes");
    let visits = super::branchfold::VISITS.with(|one| one.get());
    assert!(visits <= 3 * (count as usize + 1), "{visits} block visits for {count} removable jumps");
}

/// Every hoisted anchor solved the liveness of every value of the body afresh
/// (a nest of 64 copied loops: a third of `lir jumps`); a value is live where
/// its own reads and definitions say, so only the values a moved instruction
/// names are solved. Held to the solve of all of them over random graphs of
/// blocks with reads, definitions, delivered registers and phis.
#[test]
fn test_the_liveness_of_some_values_is_what_the_solve_of_all_of_them_gives() {
    for seed in 1..300u64 {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut next = move |most: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % most
        };
        let count = 3 + next(7) as usize;
        let mut blocks: Vec<Block> = Vec::new();
        for at in 0..count as i64 {
            let mut insns = Vec::new();
            for step in 0..next(5) {
                let defines = if next(2) == 0 { vec![next(6) as u32] } else { Vec::new() };
                let uses = if next(3) > 0 { vec![next(6) as u32] } else { Vec::new() };
                let mut one = Insn::new(at * 10 + step as i64, None, None, defines, uses);
                if next(5) == 0 {
                    one.requires = vec![(crate::model::ir::Held { value: next(6) as u32, width: 2 }, RegId::AX)];
                }
                insns.push(Arc::new(one));
            }
            let mut block = LirBlock::new(at, insns);
            block.succ = (0..next(3)).map(|_| next(count as u64) as i64).collect();
            block.succ.dedup();
            blocks.push(block.into());
        }
        for at in 0..count {
            if next(3) == 0 {
                let from: Vec<i64> = (0..count as i64).filter(|_| next(2) == 0).collect();
                blocks[at]
                    .phis
                    .push(
                        crate::model::lir::Phi {
                            result: 6 + next(2) as u32,
                            incoming: from.iter().map(|f| (*f, next(6) as u32)).collect(),
                        },
                    );
            }
        }
        let whole = _live_values(&blocks);
        let some: BTreeSet<u32> = (0..9).filter(|_| next(2) == 0).collect();
        let found = Mentions::of(&blocks).live_in_of(&blocks, &some);
        for value in &some {
            for block in &blocks {
                assert_eq!(
                    found[value].contains(&block.at),
                    whole.get(&block.at).is_some_and(|live| live.contains(value)),
                    "seed {seed}: value {value} at block {}",
                    block.at
                );
            }
        }
    }
}

/// An edge block's anchor moves up before the branch into it, and what is live
/// on entry is worked out for the value that anchor defines alone: the solve of
/// every value of the function ran once for the first anchor (2.3 M Ir on
/// d_faces -O2).
#[test]
fn test_hoisting_an_anchor_solves_the_liveness_of_its_value_alone() {
    let anchor = |at: i64| {
        let one = _inserted(nothing(at, None));
        Arc::new(Insn { defines: vec![500], ..(*one).clone() })
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            block(10, vec![_return(10)], vec![]).into(),
            block(20, vec![anchor(20), _jump(21, 30)], vec![30]).into(),
            block(30, vec![_return(30)], vec![]).into(),
        ],
    );
    let before = super::LIVE_SOLVES.with(std::cell::Cell::get);
    let hoisted = _hoisted(&source);
    // `LLRM_CHECK_JUMPS` solves it whole on purpose, to compare.
    let solves = super::LIVE_SOLVES.with(std::cell::Cell::get) - before;
    assert!(solves == 0 || llrm_support::env_flag!("LLRM_CHECK_JUMPS"), "every value's liveness was solved");
    let parent = hoisted.blocks.iter().find(|block| block.at == 1).expect("the compare's block");
    assert!(parent.insns.iter().any(|one| one.defines == [500]), "the anchor stayed in its edge block");
}

/// A loop of one block left by `je exit; jmp top` when the exit was laid out
/// elsewhere: the trip that goes on paid a branch not taken and a jump. The
/// likelier way is the conditional jump, `jne top`, and the exit is the jump.
#[test]
fn test_a_loop_block_with_neither_way_next_branches_back_conditionally() {
    let mut looped = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, imm(0))], vec![5]).into(),
            block(5, vec![_compare(5), _branch(6, "je", 20), _jump(7, 5)], vec![20, 5]).into(),
            block(10, vec![_move(10, cx()), _return(11)], vec![]).into(),
            block(20, vec![_return(20)], vec![]).into(),
        ],
    );
    // The loop goes round 31 times in 32, as isel's heuristic odds have it.
    let certain = crate::model::lir::BlockOdds::CERTAIN;
    looped.odds.taken.insert((5, 5), (certain * 31.0 / 32.0).round() as u32);
    looped.odds.taken.insert((5, 20), (certain / 32.0).round() as u32);
    let result = inverted(&looped, false);
    let names: Vec<(String, Option<i64>)> = result.blocks[1]
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|what| (what.name.as_deref().map(str::to_owned).unwrap_or_default(), what.target))
        .collect();
    assert_eq!(names, [("cmp".to_owned(), None), ("jne".to_owned(), Some(5)), ("jmp".to_owned(), Some(20))]);
}

#[test]
fn test_the_likelier_way_stays_the_conditional_jump() {
    let mut looped = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, imm(0))], vec![5]).into(),
            block(5, vec![_compare(5), _branch(6, "jne", 5), _jump(7, 20)], vec![5, 20]).into(),
            block(10, vec![_move(10, cx()), _return(11)], vec![]).into(),
            block(20, vec![_return(20)], vec![]).into(),
        ],
    );
    let certain = crate::model::lir::BlockOdds::CERTAIN;
    looped.odds.taken.insert((5, 5), (certain * 31.0 / 32.0).round() as u32);
    looped.odds.taken.insert((5, 20), (certain / 32.0).round() as u32);
    let result = inverted(&looped, false);
    let names: Vec<(String, Option<i64>)> = result.blocks[1]
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|what| (what.name.as_deref().map(str::to_owned).unwrap_or_default(), what.target))
        .collect();
    assert_eq!(names, [("cmp".to_owned(), None), ("jne".to_owned(), Some(5)), ("jmp".to_owned(), Some(20))]);
}

/// `optimizeBranches` makes the likelier way of a two-way branch the
/// conditional jump, but not for size, which keeps the condition as it was
/// (uniform instructions for folding).
#[test]
fn test_for_size_a_two_way_branch_keeps_its_condition() {
    let mut source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "jne", 30), _jump(3, 20)], vec![30, 20]).into(),
            block(10, vec![_move(10, cx()), _return(11)], vec![]).into(),
            block(20, vec![_return(20)], vec![]).into(),
            block(30, vec![_move(30, bx()), _return(31)], vec![]).into(),
        ],
    );
    let certain = crate::model::lir::BlockOdds::CERTAIN;
    source.odds.taken.insert((1, 30), (certain / 8.0).round() as u32);
    source.odds.taken.insert((1, 20), (certain * 7.0 / 8.0).round() as u32);
    let branch = |size: bool| {
        let result = inverted(&source, size);
        let what =
            result.blocks[0].insns.iter().filter_map(|one| one.what.as_ref()).find(|what| what.op == Operation::Branch);
        what.map(|what| (what.name.as_deref().map(str::to_owned).unwrap_or_default(), what.target)).unwrap()
    };
    assert_eq!(branch(false), ("je".to_owned(), Some(20)), "the likelier way is the jump");
    assert_eq!(branch(true), ("jne".to_owned(), Some(30)), "size keeps the condition");
}

/// `alignBlocks`: a block of a loop that nothing falls into starts at the
/// target's loop alignment; the one the loop falls through to, hot, does not;
/// nothing is aligned for size or where the target aligns no loop (m16).
#[test]
fn test_a_loop_block_nothing_falls_into_is_aligned() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, imm(0)), _jump(2, 3)], vec![3]).into(),
            block(8, vec![_move(8, bx())], vec![3]).into(),
            block(3, vec![_compare(3), _branch(4, "jl", 8), _jump(5, 17)], vec![8, 17]).into(),
            block(17, vec![_return(17)], vec![]).into(),
        ],
    );
    let m16 = cpu::profile("486").unwrap();
    assert_eq!(m16.loop_alignment, 0, "the test profiles are m16's");
    let mut on = m16.clone();
    on.loop_alignment = 4;
    let on = &on;
    let align =
        |one: &cpu::Profile| aligned(&source, one).blocks.iter().map(|one| (one.at, one.align)).collect::<Vec<_>>();
    assert_eq!(align(on), [(1, 0), (8, 4), (3, 0), (17, 0)]);
    let mut size = on.clone();
    size.size = true;
    assert!(align(&size).iter().all(|(_, align)| *align == 0), "size aligns nothing");
    assert!(align(m16).iter().all(|(_, align)| *align == 0), "m16 aligns nothing");
}

/// A table jump's index to a block that only jumps goes to where it jumps: the
/// block is dropped and the table kept naming it ("references to nothing
/// defined or declared: L0_14").
#[test]
fn test_a_table_entry_for_a_block_that_only_jumps_goes_to_its_target() {
    let table = Semantics {
        name: Some("jmp".into()),
        sources: vec![bx()],
        indirect: true,
        table: vec![2, 3, 2],
        ..Semantics::new(Operation::Jump)
    };
    let table_jump = Arc::new(Insn::new(1, Some((1, 1)), Some(table), Vec::new(), Vec::new()));
    let result = threaded(&body(
        "f",
        1,
        vec![
            block(1, vec![table_jump], vec![2, 3]).into(),
            block(2, vec![_jump(2, 4)], vec![4]).into(),
            block(3, vec![_move(3, cx()), _return(4)], vec![]).into(),
            block(4, vec![_return(5)], vec![]).into(),
        ],
    ));
    let first = &result.blocks[0];
    let jump = first.insns.last().and_then(|one| one.what.as_ref()).expect("the table jump");
    assert_eq!(jump.table, vec![4, 3, 4]);
    assert_eq!(first.succ, vec![4, 3]);
    assert!(result.blocks.iter().all(|one| one.at != 2));
}

/// `Semantics::remapped` renames the target and each index of a table, and is
/// `None` for what it leaves be.
#[test]
fn test_a_branch_or_table_is_remapped_by_its_blocks() {
    let to = |at: i64| if at == 2 { 9 } else { at };
    let direct = Semantics { target: Some(2), ..Semantics::new(Operation::Branch) };
    assert_eq!(direct.remapped(to).and_then(|one| one.target), Some(9));
    let table = Semantics { indirect: true, table: vec![2, 3, 2], ..Semantics::new(Operation::Jump) };
    assert_eq!(table.remapped(to).map(|one| one.table), Some(vec![9, 3, 9]));
    assert!(Semantics { target: Some(3), ..Semantics::new(Operation::Jump) }.remapped(to).is_none());
    assert!(Semantics::new(Operation::Move).remapped(to).is_none());
}

/// Threading past a passage must not look at the whole body again for each
/// one: N cases, N walks of N blocks (`cases` 1024: `lir jumps` 8196 Minstr of
/// 25 G). Blocks looked at stay a small multiple of the blocks, at 40 and 160
/// cases alike.
#[test]
fn threading_a_chain_of_passages_looks_at_each_block_a_few_times() {
    for cases in [40, 160] {
        let visits = visits_threading_passages(cases);
        assert!(visits <= 12 * (2 * cases + 2) as usize, "{visits} blocks looked at for {cases} passages");
    }
}

fn visits_threading_passages(cases: i64) -> usize {
    let (exit, last) = (10_000, 10 * cases);
    let mut blocks = Vec::new();
    let mut passages = Vec::new();
    for case in 0..cases {
        let (at, passage) = (10 * case, 20_000 + 10 * case);
        blocks.push(block(
            at,
            vec![_compare(at), _branch(at + 1, "jne", passage), _jump(at + 2, at + 10)],
            vec![passage, at + 10],
        ));
        passages.push(block(passage, vec![_inserted(_jump(passage, exit))], vec![exit]));
    }
    blocks.push(block(last, vec![_return(last)], vec![]));
    blocks.extend(passages);
    blocks.push(block(exit, vec![_return(exit)], vec![]));
    super::branchfold::VISITS.with(|one| one.set(0));
    let result = threaded(&body("f", 0, blocks.into_iter().collect()));
    assert!(result.blocks.len() < 2 * cases as usize, "the passages are threaded past");
    super::branchfold::VISITS.with(|one| one.get())
}

/// A table jump to one passage in several entries freed the passage once for
/// each, and its edge to the exit was taken off the count each time: the
/// count wrapped, the exit was thought unentered and removed while a block
/// still jumped to it (qcport/mdl: `a jump to L8_38, which is nowhere`).
#[test]
fn a_table_naming_one_passage_four_times_strands_it_once() {
    let (passage, exit, other) = (20, 30, 40);
    let table = Semantics { table: vec![passage; 4], ..Semantics::new(Operation::Jump) };
    let switch = Arc::new(Insn::new(1, Some((1, 1)), Some(table), Vec::new(), Vec::new()));
    let blocks = vec![
        block(0, vec![_compare(0), _branch(1, "jne", other)], vec![other, 10]),
        block(10, vec![switch], vec![passage]),
        block(passage, vec![_inserted(_jump(passage, exit))], vec![exit]),
        block(other, vec![_inserted(_jump(other, exit))], vec![exit]),
        block(exit, vec![_return(exit)], vec![]),
    ];
    let result = threaded(&body("f", 0, blocks.into_iter().collect()));
    let known: Vec<i64> = result.blocks.iter().map(|block| block.at).collect();
    for block in &result.blocks {
        assert!(
            block.succ.iter().all(|to| known.contains(to)),
            "{} jumps to a block that is gone: {:?}",
            block.at,
            block.succ
        );
    }
}

/// A dispatch of many arms was placed block by block, each trace looking at
/// every block waiting: `cases` 1024 spent 7% of the compile there. Every block
/// is placed once, the entry first.
#[test]
fn a_dispatch_of_many_arms_places_each_block_once() {
    let arms = 60;
    let mut blocks = vec![block(0, vec![_compare(0), _branch(1, "je", 1000)], vec![1000, 10])];
    blocks.extend((0..arms).map(|case| {
        let at = 10 + 10 * case;
        block(
            at,
            vec![_compare(at), _branch(at + 1, "je", 1000 + at), _jump(at + 2, at + 10)],
            vec![1000 + at, at + 10],
        )
    }));
    blocks.push(block(10 + 10 * arms, vec![_return(10 + 10 * arms)], vec![]));
    blocks.push(block(1000, vec![_return(1000)], vec![]));
    blocks.extend((0..arms).map(|case| block(1010 + 10 * case, vec![_return(1010 + 10 * case)], vec![])));
    let placed_body = placed(&body("f", 0, blocks.into_iter().collect())).unwrap();
    assert_eq!(placed_body.blocks.len(), 2 * arms as usize + 3);
    let mut seen: Vec<i64> = placed_body.blocks.iter().map(|one| one.at).collect();
    assert_eq!(seen[0], 0, "the entry is first");
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), placed_body.blocks.len(), "a block placed twice");
}

/// Finding the blocks on a cycle looked through every block for each block that
/// is a component alone: 1600 diamonds spent 247 M of 304 M instructions of
/// the tail merge there, the merge's cost growing 3.4x for a doubled body. The
/// scan is once per body; a self-loop and a two-block cycle are still found.
#[test]
fn test_the_blocks_on_a_cycle_are_found_without_a_scan_per_block() {
    use super::tailmerge::{SELF_LOOP_SCANS, cyclic_blocks};
    let n = 400;
    let mut blocks: Vec<LirBlock> = (0..n).map(|at| block(at, vec![_jump(at, at + 1)], vec![at + 1])).collect();
    blocks.push(block(n, vec![_branch(n, "je", n), _jump(n + 1, n + 2)], vec![n, n + 2]));
    blocks.push(block(n + 2, vec![_return(n + 2)], vec![]));
    SELF_LOOP_SCANS.with(|one| one.set(0));
    let blocks: Vec<crate::model::lir::Block> = blocks.into_iter().map(crate::model::lir::Block::from).collect();
    let found = cyclic_blocks(&blocks);
    let scans = SELF_LOOP_SCANS.with(|one| one.get());
    assert!(scans <= 4 * blocks.len(), "{scans} scans for {} blocks", blocks.len());
    assert_eq!(found.into_iter().collect::<Vec<_>>(), vec![n]);
}

/// Loops nested three deep, the innermost one block that loops on itself; each
/// level's header tests a rare base case (`e`) and otherwise enters the next,
/// and both reach the level's latch `l`.
fn loop_nest() -> LirBody {
    let (levels, done): (i64, i64) = (3, 9999);
    let (h, e, pre, l) = (|k: i64| 100 * k, |k: i64| 100 * k + 1, |k: i64| 100 * k + 2, |k: i64| 100 * k + 3);
    let mut blocks: Vec<Block> =
        vec![block(1, vec![_compare(1), _branch(2, "jle", done), _jump(3, 10)], vec![done, 10]).into()];
    blocks.push(block(10, vec![_move(10, imm(0)), _jump(11, h(1))], vec![h(1)]).into());
    let mut odds: Vec<(i64, i64, f64)> = vec![(1, done, 0.25), (1, 10, 0.75)];
    for k in 1..=levels {
        blocks.push(
            block(
                h(k),
                vec![_compare(h(k)), _branch(h(k) + 1, "jl", e(k)), _jump(h(k) + 2, pre(k))],
                vec![e(k), pre(k)],
            )
            .into(),
        );
        odds.extend([(h(k), e(k), 1.0 / 32.0), (h(k), pre(k), 31.0 / 32.0)]);
        blocks.push(block(e(k), vec![_move(e(k), imm(k)), _jump(e(k) + 50, l(k))], vec![l(k)]).into());
        let next = if k < levels { h(k + 1) } else { 7000 };
        blocks.push(block(pre(k), vec![_move(pre(k), imm(0)), _jump(pre(k) + 50, next)], vec![next]).into());
    }
    blocks.push(
        block(7000, vec![_compare(7000), _branch(7001, "jg", 7000), _jump(7002, l(levels))], vec![7000, l(levels)])
            .into(),
    );
    odds.extend([(7000, 7000, 31.0 / 32.0), (7000, l(levels), 1.0 / 32.0)]);
    for k in (1..=levels).rev() {
        let out = if k == 1 { done } else { l(k - 1) };
        let mut insns: Vec<_> = (0..5).map(|n| _move(l(k) + n + 1, imm(n))).collect();
        insns.extend([_compare(l(k)), _branch(l(k) + 10, "jg", h(k)), _jump(l(k) + 20, out)]);
        blocks.push(block(l(k), insns, vec![h(k), out]).into());
        odds.extend([(l(k), h(k), 31.0 / 32.0), (l(k), out, 1.0 / 32.0)]);
    }
    blocks.push(block(done, vec![_return(done)], vec![]).into());
    let mut made = body("f", 1, blocks);
    for (from, to, chance) in odds {
        made.odds.taken.insert((from, to), (chance * crate::model::lir::BlockOdds::CERTAIN) as u32);
    }
    made
}

/// A loop whose exit test leaves exactly as often as its top is fallen into is
/// not rotated (`rotateLoop`: `FallThrough2Top >= ExitFreq`). The two are equal
/// in the odds of 31 in 32, and in `f64` the exit came out larger by a
/// rounding: the innermost loop of a nest (bench/fib) was rotated, and the
/// latches of the levels were left at the end after the inner loops, each
/// base-case block ending in a jump to its latch (17 jumps, llc 9). Order
/// checked with `llc -O2 -mtriple=i486` on the same nest as IR with branch
/// weights: entry pre0 e1 l1 h1 pre1 e2 l2 h2 pre2 h3 pre3 s l3 e3 done.
#[test]
fn test_a_loop_is_not_rotated_where_its_exit_only_ties_the_top_fall_through() {
    let made = optimized(&loop_nest(), cpu::profile("486").unwrap(), None).unwrap();
    let order: Vec<i64> = made.blocks.iter().map(|one| one.at).collect();
    assert_eq!(order, [1, 10, 101, 103, 100, 102, 201, 203, 200, 202, 300, 302, 7000, 303, 301, 9999], "{order:?}");
}

/// Placement worked out the frequencies of the body again, as `over` over the
/// blocks it was given, though the body's own answer stood in its facts: 68 M
/// of 380 M instructions of `jumps` at 1600 diamonds. It asks `Frequency::of`.
#[test]
fn test_placement_reads_the_frequencies_the_body_already_has() {
    let made = diamond_joined(0.5, vec![_return(30)]);
    Frequency::of(&made);
    crate::analysis::frequency::WORKED.with(|one| one.set(0));
    placed(&made).unwrap();
    let worked = crate::analysis::frequency::WORKED.with(|one| one.get());
    assert_eq!(worked, 0, "{worked} frequency runs for frequencies the body had");
}

/// `Loop: xxx; jcc Out; jmp Loop` in `OptimizeBranches` alone: the pin reverses
/// it (`OptimizeBlock` L1685) so the loop is `jncc Loop; jmp Out`.
#[test]
fn test_a_one_block_loop_is_folded_into_a_branch_back() {
    let blocks: Vec<Block> = vec![
        block(1, vec![_compare(1), _branch(2, "je", 7)], vec![7, 4]).into(),
        block(4, vec![_compare(4), _branch(5, "je", 9), _jump(6, 4)], vec![9, 4]).into(),
        block(7, vec![_move(7, bx()), _return(8)], vec![]).into(),
        block(9, vec![_return(9)], vec![]).into(),
    ];
    let folded = branchfold::folded(&body("f", 1, blocks), &std::collections::BTreeSet::new());
    let tail: Vec<(String, Option<i64>)> = physical(&folded)
        .into_iter()
        .filter(|what| matches!(what.op, Operation::Branch | Operation::Jump))
        .map(|what| (what.name.as_deref().map(str::to_owned).unwrap_or_default(), what.target))
        .collect();
    assert_eq!(tail, [("je".to_owned(), Some(7)), ("jne".to_owned(), Some(4)), ("jmp".to_owned(), Some(9))]);
}

/// A block the block before falls into, and nothing else enters, is one block
/// with it (`OptimizeBlock` L1515). Left apart, the loop `4: mov; 6: cmp; jcc
/// Out; jmp 4` was two blocks, the jump went to a block that is not the one it
/// ends, and the loop kept `jcc Out; jmp Loop` instead of `jncc Loop; jmp Out`
/// (the L1685 reversal needs the one-block loop).
#[test]
fn test_a_block_only_the_block_before_falls_into_is_merged_into_it() {
    let blocks: Vec<Block> = vec![
        block(1, vec![_compare(1), _branch(2, "je", 7)], vec![7, 4]).into(),
        block(4, vec![_move(4, bx())], vec![6]).into(),
        block(6, vec![_compare(6), _branch(7, "je", 9), _jump(8, 4)], vec![9, 4]).into(),
        block(7, vec![_move(7, bx()), _return(8)], vec![]).into(),
        block(9, vec![_return(9)], vec![]).into(),
    ];
    let folded = branchfold::folded(&body("f", 1, blocks), &std::collections::BTreeSet::new());
    let tail: Vec<(String, Option<i64>)> = physical(&folded)
        .into_iter()
        .filter(|what| matches!(what.op, Operation::Branch | Operation::Jump))
        .map(|what| (what.name.as_deref().map(str::to_owned).unwrap_or_default(), what.target))
        .collect();
    assert_eq!(tail, [("je".to_owned(), Some(7)), ("jne".to_owned(), Some(4)), ("jmp".to_owned(), Some(9))]);
}

/// A block that is one conditional branch identical to the branch that ends
/// the block before it: the earlier one is redundant (`OptimizeBlock` L1586).
#[test]
fn test_a_repeated_conditional_branch_is_dropped_from_the_block_before() {
    let lines = _printed(vec![
        block(1, vec![_compare(1), _branch(2, "je", 9)], vec![9, 4]).into(),
        block(4, vec![_branch(5, "je", 9)], vec![9, 7]).into(),
        block(7, vec![_move(7, cx()), _return(8)], vec![]).into(),
        block(9, vec![_return(9)], vec![]).into(),
    ]);
    assert_eq!(lines.iter().filter(|line| line.starts_with("je")).count(), 1, "{lines:?}");
}

/// `loop` counts ECX down as it branches: two in a row are two counts, and
/// one to the block it runs into still counts. The repeat rule dropped the
/// first and the fall-through rule made the second a `jmp`.
#[test]
fn test_a_loop_branch_is_never_dropped_or_made_a_jump() {
    let loops = |blocks: Vec<Block>| {
        let folded = branchfold::folded(&body("f", 1, blocks), &std::collections::BTreeSet::new());
        physical(&folded).into_iter().filter(|what| what.name.as_deref() == Some("loop")).count()
    };
    let repeated = vec![
        block(1, vec![_compare(1), _branch(2, "loop", 9)], vec![9, 4]).into(),
        block(4, vec![_branch(5, "loop", 9)], vec![9, 7]).into(),
        block(7, vec![_move(7, cx()), _return(8)], vec![]).into(),
        block(9, vec![_return(9)], vec![]).into(),
    ];
    assert_eq!(loops(repeated), 2, "a repeated loop branch");
    let onward = vec![
        block(1, vec![_compare(1), _branch(2, "loop", 4)], vec![4]).into(),
        block(4, vec![_return(5)], vec![]).into(),
    ];
    assert_eq!(loops(onward), 1, "a loop branch to the next block");
}

/// `HoistCommonCodeInSuccs` (L2043): both ways begin with `mov cx, 5` and this
/// block is the only way into either; it runs once, ahead of the compare.
#[test]
fn test_the_instruction_both_ways_begin_with_is_hoisted_above_the_compare() {
    let set = |at| _inserted(_insn(at, Operation::Move, "mov", vec![cx()], vec![imm(5)], None));
    let lines = _printed(vec![
        block(1, vec![_compare(1), _branch(2, "je", 9)], vec![9, 4]).into(),
        block(4, vec![set(4), _move(5, bx()), _return(6)], vec![]).into(),
        block(9, vec![set(9), _move(10, cx()), _return(11)], vec![]).into(),
    ]);
    assert_eq!(lines.iter().filter(|line| line.as_str() == "mov cx, 5").count(), 1, "{lines:?}");
    let at = lines.iter().position(|line| line.as_str() == "mov cx, 5").unwrap();
    let compare = lines.iter().position(|line| line.starts_with("cmp")).unwrap();
    assert!(at < compare, "{lines:?}");
}

fn arms_after(
    head: Vec<Arc<Insn>>,
    set: impl Fn(i64) -> Vec<Arc<Insn>>,
) -> Vec<String> {
    let arm = |at: i64, tail: Arc<Insn>| {
        let mut insns = set(at);
        insns.extend([tail, _return(at + 90)]);
        insns
    };
    _printed(vec![
        block(1, head, vec![9, 4]).into(),
        block(4, arm(4, _move(5, bx())), vec![]).into(),
        block(9, arm(9, _move(10, bx())), vec![]).into(),
    ])
}

/// The zero both arms return is `xor ax, ax` by the time arms are folded: it
/// sets flags, which the compare in front of the branch sets again, so it runs
/// above the compare (x_prefix, x_life, recgcd: +2..3 instructions each).
#[test]
fn test_a_zeroing_xor_both_ways_begin_with_is_hoisted_above_the_compare() {
    let lines = arms_after(vec![_compare(1), _branch(2, "je", 9)], |at| {
        vec![_inserted(_insn(at, Operation::Binary, "xor", vec![cx()], vec![cx(), cx()], None))]
    });
    assert_eq!(lines.iter().filter(|line| line.as_str() == "xor cx, cx").count(), 1, "{lines:?}");
    let at = lines.iter().position(|line| line.as_str() == "xor cx, cx").unwrap();
    assert!(at < lines.iter().position(|line| line.starts_with("cmp")).unwrap(), "{lines:?}");
}

fn lined(
    one: Arc<Insn>,
    line: u32,
) -> Arc<Insn> {
    Arc::new(Insn { line: Some(crate::model::lir::SrcLoc::line(line)), ..(*one).clone() })
}

fn zero_lines(
    one: u32,
    other: u32,
) -> Vec<Option<u32>> {
    let zero =
        |at, line| lined(_inserted(_insn(at, Operation::Binary, "xor", vec![cx()], vec![cx(), cx()], None)), line);
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 9)], vec![9, 4]).into(),
            block(4, vec![zero(4, one), _move(5, bx()), _return(6)], vec![]).into(),
            block(9, vec![zero(9, other), _move(10, bx()), _return(11)], vec![]).into(),
        ],
    );
    let result = threaded(&source);
    result
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|insn| insn.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("xor")))
        .map(|insn| insn.line)
        .map(|one| one.map(|at| at.line))
        .collect()
}

/// A hoisted instruction took the line of the taken arm, so a debugger told
/// line 3 for code that also runs for line 7; the pin merges the two
/// (`getMergedLocation`, BranchFolding.cpp:2251) into line 0.
#[test]
fn test_the_instruction_hoisted_from_arms_of_different_lines_has_line_zero() {
    assert_eq!(zero_lines(3, 7), [Some(0)]);
}

#[test]
fn test_the_instruction_hoisted_from_arms_of_one_line_keeps_it() {
    assert_eq!(zero_lines(3, 3), [Some(3)]);
}

/// The kept tail of two arms of different lines took one arm's line.
#[test]
fn test_merged_tails_of_different_lines_have_line_zero() {
    let arm = |at: i64, line: u32| vec![lined(_inserted(_move(at, imm(0))), line), _inserted(_jump(at + 1, 30))];
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, arm(10, 3), vec![30]).into(),
            block(20, arm(20, 7), vec![30]).into(),
            block(30, vec![_compare(30), _compare(31), _compare(32), _return(33)], vec![]).into(),
        ],
    );
    let result = merged(&placed(&source).unwrap()).unwrap();
    let lines: Vec<Option<u32>> = result
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|insn| insn.what.as_ref().is_some_and(|what| what.op == Operation::Move))
        .map(|insn| insn.line)
        .map(|one| one.map(|at| at.line))
        .collect();
    assert_eq!(lines, [Some(0)]);
}

/// A branch whose flags come from an earlier instruction, not a compare in
/// front of it, reads what a hoisted `xor` would clobber.
#[test]
fn test_a_flag_setting_instruction_is_not_hoisted_over_a_branch_with_no_compare() {
    let lines = arms_after(vec![_move(1, bx()), _branch(2, "je", 9)], |at| {
        vec![_inserted(_insn(at, Operation::Binary, "xor", vec![cx()], vec![cx(), cx()], None))]
    });
    assert_eq!(lines.iter().filter(|line| line.as_str() == "xor cx, cx").count(), 2, "{lines:?}");
}

/// The pin copies propagation twice, again after tail duplication and block
/// placement (row 84): the phases end `ControlFlow`, `CopyPropagation`.
#[test]
fn test_copy_propagation_runs_again_after_the_control_flow() {
    let names = |cpu: &str| -> Vec<&'static str> {
        crate::flow::machine(
            &IndexMap::default(),
            Some(Rc::new(RefCell::new(Frame::new(0)))),
            None,
            Some(&IndexMap::default()),
            false,
            cpu,
            &crate::backend::target::BUILT_IN,
        )
        .unwrap()
        .iter()
        .map(|phase| phase.class_name())
        .collect()
    };
    let optimized = names("386");
    assert_eq!(&optimized[optimized.len() - 2..], ["ControlFlow", "CopyPropagation"]);
}
