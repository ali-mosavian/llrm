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
    let what = Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) };
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
        ["L0_1:", "cmp ax, bx", "jle L0_7", "L0_4:", "mov ax, cx", "L0_7:", "ret"]
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
        false,
    )
    .unwrap()
    .pop()
    .unwrap()
    .transform(source)
    .unwrap();

    let real: Vec<(Operation, Option<String>, Option<i64>)> = result.blocks[0]
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|one| (one.op, one.name.clone(), one.target))
        .collect();
    assert_eq!(
        real,
        [(Operation::Compare, Some("cmp".to_owned()), None), (Operation::Branch, Some("je".to_owned()), Some(4))]
    );
    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1, 9, 4]);
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
    assert_eq!(ats, [1, 2, 3, 4]);
}

#[test]
fn test_jump_to_the_next_block_is_dropped() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_move(1, bx()), _jump(2, 4)], vec![4]).into(),
            block(4, vec![_return(4)], vec![]).into(),
        ]),
        ["L0_1:", "mov ax, bx", "L0_4:", "ret"]
    );
}

/// SCALAR lost the three decoded bytes of its preheader jump after unrolling.
#[test]
fn test_source_owned_jump_to_fallthrough_becomes_an_anchor() {
    let jump = _insn(1, Operation::Jump, "jmp", vec![], vec![], Some(4));
    let jump = Arc::new(Insn { covers: Some((1, 4)), ..(*jump).clone() });
    let source = body("f", 1, vec![block(1, vec![jump], vec![4]).into(), block(4, vec![_return(4)], vec![]).into()]);

    let result = threaded(&source);

    let kept = &result.blocks[0].insns;
    assert_eq!(kept.len(), 1);
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
        ["L0_1:", "mov ax, bx", "L0_9:", "ret"]
    );
}

fn nothing(
    at: i64,
    covers: Option<(i64, i64)>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(String::new()), ..Semantics::new(Operation::Nothing) };
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

    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1, 5, 9]);
    assert_eq!(result.blocks[0].succ, [9]);
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
    let merged = tailmerge::merged(body, false)?.unwrap_or_else(|| body.clone());
    Ok(machinedce::eliminated(_converged(&threaded(&merged))))
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
            block(30, vec![_return(30)], vec![]).into(),
        ],
    )
}

/// qglsurf emitted the same zero-result tail from two failure arms.
#[test]
fn test_identical_result_tails_are_merged() {
    let result = merged(&placed(&two_zero_tails(true)).unwrap()).unwrap();
    let physical = physical(&result);

    assert_eq!(physical.iter().filter(|what| what.op == Operation::Move).count(), 1);
    assert_eq!(physical.iter().filter(|what| what.op == Operation::Branch).count(), 0);
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

    let result = tailmerge::merged(&source, false).unwrap().expect("nothing was merged");

    assert_eq!((count(&result, Operation::Move), count(&result, Operation::Compare)), (5, 2));
}

/// One shared instruction plus the jump taken off is two, under the three a
/// merge must save (`MinCommonTailLength`): the arms stay as they are.
#[test]
fn test_a_tail_of_one_is_not_shared() {
    let source = arms_sharing_a_tail(1);

    assert!(tailmerge::merged(&source, false).unwrap().is_none());
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

    assert!(tailmerge::merged(&source, false).unwrap().is_none());
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

    let result = tailmerge::merged(&source, false).unwrap().expect("nothing was merged");

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

    let result = tailmerge::merged(&source, true).unwrap().expect("nothing was merged");

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

/// The merge ran before tail duplication: two arms that end in the same move
/// and jump to a small join were merged into an empty tail block, so the join
/// could no longer be copied into an arm and every trip paid a jump (`x_cordic`
/// -O3, +5.0% clocks). LLVM duplicates while it places, then merges.
#[test]
fn test_a_small_join_is_copied_into_its_arm_before_the_arms_are_merged() {
    let arm = |at: i64, first: Loc| {
        let insns = vec![_inserted(_move(at, first)), _inserted(_move(at + 1, bx())), _inserted(_jump(at + 9, 30))];
        block(at, insns, vec![30]).into()
    };
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20), _jump(3, 10)], vec![20, 10]).into(),
            arm(10, imm(1)),
            arm(20, imm(2)),
            block(30, vec![_compare(30), _branch(31, "jge", 40), _jump(32, 50)], vec![40, 50]).into(),
            block(40, vec![_inserted(_move(40, cx())), _inserted(_return(41))], vec![]).into(),
            block(50, vec![_inserted(_move(50, bx())), _inserted(_return(51))], vec![]).into(),
        ],
    );

    let result = optimized(&source, cpu::profile("486").unwrap()).unwrap();

    let jumps_only = result
        .blocks
        .iter()
        .filter(|block| {
            _real(block).len() == 1 && _real(block)[0].what.as_ref().is_some_and(|what| what.op == Operation::Jump)
        })
        .count();
    assert_eq!(
        jumps_only,
        0,
        "{:?}",
        result.blocks.iter().map(|b| (b.at, b.succ.clone(), b.insns.len())).collect::<Vec<_>>()
    );
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
    assert!(tailmerge::merged(&source, true).unwrap().is_some(), "the premise: for size the merge applies here");

    let result = optimized(&source, cpu::profile("486").unwrap()).unwrap();

    assert_eq!(
        count(&result, Operation::Compare),
        count(&source, Operation::Compare),
        "{:?}",
        result.blocks.iter().map(|b| (b.at, b.succ.clone(), b.insns.len())).collect::<Vec<_>>()
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

    let result = tailmerge::merged(&source, false).unwrap().expect("nothing was merged");

    assert_eq!(count(&result, Operation::Return), 2);
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
        false,
    )
    .unwrap()
    .pop()
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

/// The leave sequence was encoded for every procedure (17.5 M on `d_faces` -O2, 11 procedures), also those whose one return
/// has nothing to duplicate into.
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
        ["L0_1:", "L0_8:", "mov ax, cx", "L0_4:", "cmp ax, bx", "jne L0_8", "L0_23:", "ret"]
    );
}

/// A loop whose first test could fail kept its test on top.
#[test]
fn test_loop_not_known_to_run_is_entered_at_its_test_placed_last() {
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
        ["L0_1:", "mov ax, cx", "jmp L0_3", "L0_8:", "mov ax, bx", "L0_3:", "cmp ax, bx", "jl L0_8", "L0_17:", "ret"]
    );
}

/// With the test placed after the latch, every pass took `jg out` and then `jmp
/// body`.
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
            "jmp L0_3",
            "L0_8:",
            "mov ax, bx",
            "L0_3:",
            "cmp ax, bx",
            "jl L0_8",
            "L0_20:",
            "mov ax, cx",
            "L0_12:",
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
    let mut made = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]).into(),
            block(10, vec![_move(10, imm(1)), _jump(11, 30)], vec![30]).into(),
            block(20, vec![_move(20, imm(2)), _jump(21, 30)], vec![30]).into(),
            block(30, vec![_return(30)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    made.odds.taken.insert((1, 10), fixed(hot));
    made.odds.taken.insert((1, 20), fixed(1.0 - hot));
    made
}

/// An arm leaves its join, to land after the return, only where the edge
/// into the join from the block before it is `LIKELY`, MachineBlockPlacement's
/// 80%. At even odds the arm went off the join, and a diamond no heuristic
/// tells apart lost the shorter layout: deedlines ran 177 ms slower.
#[test]
fn test_an_arm_leaves_its_join_only_for_a_likely_edge() {
    let order = |hot: f64| placed(&weighted_diamond(hot)).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    assert_eq!(order(0.9), vec![1, 10, 30, 20]);
    // Short of it both arms stay before the join, the likelier one second.
    assert_eq!(order(0.7), vec![1, 20, 10, 30]);
    assert_eq!(order(0.5), vec![1, 10, 20, 30]);
}

/// A diamond short of `LIKELY` keeps both arms before the join, so its
/// likelier arm goes second and falls into the join. Made the fall-through
/// at 62.5%, it jumped over the rare arm instead: deedlines' zoomdistort
/// ran a jump more each of 31,000 passes.
#[test]
fn test_a_diamonds_likelier_arm_falls_into_its_join() {
    let order = |hot: f64| placed(&weighted_diamond(hot)).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    // The branch arm 20 likelier: it goes second.
    assert_eq!(order(0.375), vec![1, 10, 20, 30]);
    // The fall-through arm 10 likelier: 20 first, 10 second.
    assert_eq!(order(0.625), vec![1, 20, 10, 30]);
    // `LIKELY` and over: the likely arm falls through, the rare one leaves its
    // join.
    assert_eq!(order(0.1), vec![1, 20, 30, 10]);
}

/// For size a diamond keeps both arms before its join, and its arm order
/// is free where both arms keep short jumps either way: the likelier arm
/// goes second, into the join, at no byte's cost. Not past a short jump's
/// reach, where the order could change a jump's size.
#[test]
fn test_for_size_a_diamonds_likelier_arm_goes_second_where_size_allows() {
    let order = |body: LirBody| _placed(&body, true).unwrap().blocks.iter().map(|one| one.at).collect::<Vec<_>>();
    // The premise: both arms are a few bytes, so either order uses short jumps.
    let small = weighted_diamond(0.7);
    assert!(
        small
            .blocks
            .iter()
            .filter(|one| [10, 20].contains(&one.at))
            .all(|one| _arm_bytes(16, one).is_some_and(|bytes| bytes < 16))
    );
    assert_eq!(order(small), vec![1, 20, 10, 30]);
    // Arm 10 past a short jump's reach: the order stays the source's.
    let mut large = weighted_diamond(0.7);
    let arm = large.blocks.iter_mut().find(|one| one.at == 10).unwrap();
    let mut insns: Vec<Arc<Insn>> = (0..60).map(|_| _move(10, imm(4660))).collect();
    insns.push(_jump(11, 30));
    arm.insns = insns.into();
    assert!(_arm_bytes(16, large.blocks.iter().find(|one| one.at == 10).unwrap()).is_some_and(|bytes| bytes > 127));
    assert_eq!(order(large), vec![1, 10, 20, 30]);
}

/// A loop entered at its test, its body ending `jmp test`.
fn entered_at_its_test(
    body_ends: Vec<Arc<Insn>>,
    body_succ: Vec<i64>,
) -> LirBody {
    let mut made = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, imm(0)), _jump(2, 30)], vec![30]).into(),
            block(10, body_ends, body_succ).into(),
            block(30, vec![_compare(30), _branch(31, "jne", 10)], vec![10, 40]).into(),
            block(40, vec![_return(40)], vec![]).into(),
            block(50, vec![_return(50)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    made.odds.taken.insert((30, 10), fixed(31.0 / 32.0));
    made.odds.taken.insert((30, 40), fixed(1.0 / 32.0));
    made
}

/// A loop body's `jmp` back to the test ran every trip: deedlines' blobs
/// loops executed one per pass. Copied, the test runs where the jump did,
/// and the copy's `jmp` to the exit only when the loop leaves (mark 8:
/// 9,474 ms to 9,069). The copy owns no bytes; the test keeps its own.
#[test]
fn test_a_jump_to_a_short_loop_test_runs_a_copy_of_the_test() {
    let before = entered_at_its_test(vec![_move(10, imm(1)), _jump(11, 30)], vec![30]);
    let after = duplicated_tails(&before);
    let jumps = |one: &LirBody| crate::backend::executed::executed(one).expect("reducible").jumps;
    assert!(jumps(&before) > 30.0, "premise: the jump back runs every trip: {}", jumps(&before));
    assert!(jumps(&after) < 3.0, "{}", jumps(&after));
    let copied = after.blocks.iter().find(|one| one.at == 10).unwrap();
    let names: Vec<String> = _real(copied).iter().filter_map(|one| one.what.as_ref()?.name.clone()).collect();
    assert_eq!(names, ["mov", "cmp", "jne", "jmp"]);
    assert_eq!(after.owned_bytes(), before.owned_bytes());
}

/// A block ending `je out; jmp test` took the test's successors for its own
/// and lost `out`: pipeline.nib's `je` led nowhere ("a jump to L2_162,
/// which is nowhere").
#[test]
fn test_a_copied_tail_keeps_its_parents_own_branch_targets() {
    let before = entered_at_its_test(vec![_compare(10), _branch(11, "je", 50), _jump(12, 30)], vec![50, 30]);
    let after = duplicated_tails(&before);
    let copied = after.blocks.iter().find(|one| one.at == 10).unwrap();
    assert!(copied.succ.contains(&50), "{:?}", copied.succ);
    assert!(after.blocks.iter().any(|one| one.at == 50));
}

/// A `jmp`'s edge carries no odds, being certain. Its copied test took none
/// either, and `Frequency` split the copy's branch evenly: the loop it
/// closes ran twice where it ran 32 times, and qbdemo's PLASMA read 16
/// times cheaper with its code unchanged.
#[test]
fn test_a_copied_test_keeps_its_loops_odds() {
    let before = entered_at_its_test(vec![_move(10, imm(1)), _jump(11, 30)], vec![30]);
    assert!(!before.odds.taken.contains_key(&(10, 30)), "premise: the jump's edge has no odds");
    let after = duplicated_tails(&before);
    let runs = |one: &LirBody| crate::analysis::frequency::Frequency::of(one).block(10);
    assert!((runs(&after) - runs(&before)).abs() < 1e-6, "{} {}", runs(&after), runs(&before));
}

/// A loop test reached from outside the loop by a `jmp` and by a `je`: a
/// copy at the `jmp` enters the loop at its body while the `je` still
/// enters at the test. deedlines' `__main` became irreducible that way and its
/// estimate read "executes an unbounded amount", dropping 777,870 instructions
/// from the sum.
#[test]
fn test_a_copy_never_makes_a_second_entry_into_a_loop() {
    let mut before = body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 30)], vec![30, 5]).into(),
            block(5, vec![_move(5, imm(2)), _jump(6, 30)], vec![30]).into(),
            block(10, vec![_move(10, imm(1))], vec![30]).into(),
            block(30, vec![_compare(30), _branch(31, "jne", 10)], vec![10, 40]).into(),
            block(40, vec![_return(40)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    before.odds.taken.insert((30, 10), fixed(31.0 / 32.0));
    before.odds.taken.insert((30, 40), fixed(1.0 / 32.0));
    assert!(crate::backend::executed::executed(&before).is_some(), "premise: reducible");
    assert!(
        crate::backend::executed::executed(&duplicated_tails(&before)).is_some(),
        "the copy made the loop irreducible"
    );
}

/// SPHEREMAPLASMA's block 390: a loop that leaves by an edge with no
/// stated odds (`jne` stated, its exit implied), into a `jmp` to a test
/// split 3:5. The copy's edges took no odds, `Frequency` split the exit
/// evenly, and the loop nest past `je` read 2.7% hotter (#217).
#[test]
fn test_a_copy_from_an_implied_edge_keeps_every_blocks_frequency() {
    let mut before = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, imm(0))], vec![390]).into(),
            block(390, vec![_move(390, imm(1)), _branch(391, "jne", 390), _jump(392, 187)], vec![390, 187]).into(),
            block(40, vec![_return(40)], vec![]).into(),
            block(187, vec![_compare(187), _branch(188, "je", 190)], vec![190, 428]).into(),
            block(428, vec![_return(428)], vec![]).into(),
            block(190, vec![_return(190)], vec![]).into(),
        ],
    );
    let fixed = |probability: f64| (probability * crate::model::lir::BlockOdds::CERTAIN) as u32;
    before.odds.taken.insert((390, 390), fixed(31.0 / 32.0));
    before.odds.taken.insert((187, 190), fixed(0.375));
    before.odds.taken.insert((187, 428), fixed(0.625));
    assert!(!before.odds.taken.contains_key(&(390, 187)), "premise: the loop's exit is implied");
    let after = duplicated_tails(&before);
    assert!(
        after.blocks.iter().find(|one| one.at == 390).is_some_and(|one| one.succ.contains(&190)),
        "premise: the test was copied"
    );
    let (old, new) =
        (crate::analysis::frequency::Frequency::of(&before), crate::analysis::frequency::Frequency::of(&after));
    for one in [1, 390, 190, 428] {
        assert!(
            (old.block(one) - new.block(one)).abs() < 1e-6,
            "block {one}: {} -> {}",
            old.block(one),
            new.block(one)
        );
    }
}

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

/// The copy limit is eight jumps of the target's own encoding, not a guess in
/// the pass: 3 bytes in real mode, 5 in flat.
#[test]
fn test_the_copy_limit_is_priced_from_the_targets_jump() {
    assert_eq!(uncond_jump_bytes(16), 3);
    assert_eq!(uncond_jump_bytes(32), 5);
    assert_eq!(copy_limit(32), 40);
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
    let got = inverted(&body);
    let names: Vec<(String, Option<i64>)> = got.blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().unwrap())
        .map(|what| (what.name.clone().unwrap(), what.target))
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
    assert_eq!(inverted(&body).blocks[0].insns.len(), 4);
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
    let result = threaded(&source);
    let jumps = result
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Jump));
    assert_eq!(jumps.count(), 0, "premise: every jump to the next block goes");
    let visits = result.facts.0.counted("jump-visits");
    assert!(visits <= 3 * (count as usize + 1), "{visits} block visits for {count} removable jumps");
}

/// The ends of a block's real instructions, by name.
fn named(block: &LirBlock) -> Vec<String> {
    block.insns.iter().filter_map(|one| one.what.as_ref()).map(|what| what.name.clone().unwrap_or_default()).collect()
}

fn with_odds(
    mut made: LirBody,
    edges: &[(i64, i64, f64)],
) -> LirBody {
    for &(from, to, chance) in edges {
        made.odds.taken.insert((from, to), (chance * crate::model::lir::BlockOdds::CERTAIN) as u32);
    }
    made
}

/// x_adler's loop: the latch `jne top; jmp exit` with a short return as the
/// exit. The copy of the return in the jump's place left the loop's back
/// branch taken over the copy on every trip (9 clocks a trip, not 7): the
/// likelier way of a branch before the jump is not given up for the jump's.
#[test]
fn test_a_tail_is_not_copied_over_the_likelier_way_of_the_branch_before_its_jump() {
    let looped = with_odds(
        body(
            "f",
            1,
            vec![
                block(1, vec![_move(1, imm(0))], vec![10]).into(),
                block(10, vec![_move(10, cx())], vec![20]).into(),
                block(20, vec![_compare(20), _branch(21, "jne", 10), _inserted(_jump(22, 30))], vec![10, 30]).into(),
                block(30, vec![_inserted(_return(30))], vec![]).into(),
            ],
        ),
        &[(20, 10, 31.0 / 32.0), (20, 30, 1.0 / 32.0)],
    );
    let copied = duplicated(&looped, true);
    assert_eq!(named(&copied.blocks[2]), ["cmp", "jne", "jmp"], "the exit's return was copied into the latch");
}

/// Not a loop: the same rule on a guard whose branch is the likelier way, and,
/// for the other way round, a copy that is made.
#[test]
fn test_a_tail_is_copied_where_the_jump_is_the_likelier_way() {
    let make = |branch: f64| {
        with_odds(
            body(
                "f",
                1,
                vec![
                    block(1, vec![_compare(1), _branch(2, "je", 10), _inserted(_jump(3, 30))], vec![10, 30]).into(),
                    block(10, vec![_move(10, cx()), _jump(11, 40)], vec![40]).into(),
                    block(30, vec![_inserted(_return(30))], vec![]).into(),
                    block(40, vec![_return(40)], vec![]).into(),
                ],
            ),
            &[(1, 10, branch), (1, 30, 1.0 - branch)],
        )
    };
    assert_eq!(named(&duplicated(&make(0.9), true).blocks[0]), ["cmp", "je", "jmp"], "copied over the likelier branch");
    assert_eq!(
        named(&duplicated(&make(0.1), true).blocks[0]),
        ["cmp", "je", "", "ret"],
        "not copied where the jump is likelier"
    );
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
    let result = inverted(&looped);
    let names: Vec<(String, Option<i64>)> = result.blocks[1]
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|what| (what.name.clone().unwrap_or_default(), what.target))
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
    let result = inverted(&looped);
    let names: Vec<(String, Option<i64>)> = result.blocks[1]
        .insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|what| (what.name.clone().unwrap_or_default(), what.target))
        .collect();
    assert_eq!(names, [("cmp".to_owned(), None), ("jne".to_owned(), Some(5)), ("jmp".to_owned(), Some(20))]);
}

/// A table jump's index to a block that only jumps goes to where it jumps: the
/// block is dropped and the table kept naming it ("references to nothing
/// defined or declared: L0_14").
#[test]
fn test_a_table_entry_for_a_block_that_only_jumps_goes_to_its_target() {
    let table = Semantics {
        name: Some("jmp".to_owned()),
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

/// Each branch threaded past a passage walked the whole body from the entry to
/// find what it stranded, though one block can only strand the blocks it alone
/// enters: N cases, N walks of N blocks (`cases` 1024: `lir jumps` 8196 Minstr
/// of 25 G). The edges entering each block are counted; a walk is made only for
/// a block on a cycle.
#[test]
fn threading_a_chain_of_passages_makes_no_walk_from_the_entry() {
    let cases = 40;
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
    WALKS.with(|one| one.set(0));
    let result = threaded(&body("f", 0, blocks.into_iter().collect()));
    assert!(result.blocks.len() < 2 * cases as usize, "the passages are threaded past");
    let walks = WALKS.with(|one| one.get());
    assert_eq!(walks, 0, "{walks} walks from the entry for {cases} passages");
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

/// Each trace after the first began with a look at every block waiting, for
/// the one the placed blocks reach last, and a map of the placed ones made
/// again: N returns, N traces, N^2 (`cases` 1024: `_placed` 7% of the compile).
/// They are kept as blocks are placed.
#[test]
fn starting_a_trace_does_not_look_at_every_block_waiting() {
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
    EXAMINED.with(|one| one.set(0));
    let placed_body = placed(&body("f", 0, blocks.into_iter().collect())).unwrap();
    assert_eq!(placed_body.blocks.len(), 2 * arms as usize + 3);
    let looked = EXAMINED.with(|one| one.get());
    assert!(looked <= 8 * arms as usize, "{looked} entries looked at for {arms} arms");
}
