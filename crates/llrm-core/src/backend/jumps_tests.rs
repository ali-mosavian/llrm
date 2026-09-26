//! Port of tests/test_jumps.py.
//!
//! `test_qglsurf_shares_all_three_zero_result_tails` waits for
//! `cfront --opt` (phase 3).

use std::sync::Arc;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use crate::backend::frame::Frame;
use crate::model::ir::{Imm, Loc, Reg};

fn ax() -> Loc {
    Loc::Reg(Reg { register: Register::AX, width: 2 })
}

fn bx() -> Loc {
    Loc::Reg(Reg { register: Register::BX, width: 2 })
}

fn cx() -> Loc {
    Loc::Reg(Reg { register: Register::CX, width: 2 })
}

fn imm(value: i64) -> Loc {
    Loc::Imm(Imm { value, width: 2, address: None })
}

fn _insn(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Arc<Insn> {
    let what = Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, 1)), Some(what), Vec::new(), Vec::new()))
}

fn _compare(at: i64) -> Arc<Insn> {
    _insn(at, Operation::Compare, "cmp", vec![], vec![ax(), bx()], None)
}

fn _branch(at: i64, name: &str, target: i64) -> Arc<Insn> {
    _insn(at, Operation::Branch, name, vec![], vec![], Some(target))
}

fn _jump(at: i64, target: i64) -> Arc<Insn> {
    _insn(at, Operation::Jump, "jmp", vec![], vec![], Some(target))
}

fn _move(at: i64, source: Loc) -> Arc<Insn> {
    _insn(at, Operation::Move, "mov", vec![ax()], vec![source], None)
}

fn _return(at: i64) -> Arc<Insn> {
    _insn(at, Operation::Return, "ret", vec![], vec![], None)
}

fn _inserted(one: Arc<Insn>) -> Arc<Insn> {
    Arc::new(Insn { covers: Some((one.at, one.at)), ..(*one).clone() })
}

fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
    LirBlock { succ, ..LirBlock::new(at, insns) }
}

fn body(name: &str, entry: i64, blocks: Vec<LirBlock>) -> LirBody {
    LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default())
}

fn listed(body: LirBody, name: &str) -> Vec<String> {
    let procedure = masm::Procedure {
        name: name.to_owned(),
        public: true,
        far: false,
        body,
        reserve: 0,
        callees: IndexMap::default(),
        interrupt: None,
    };
    masm::_procedure(&procedure, &IndexMap::default(), 0)
        .unwrap()
        .iter()
        .map(|line| line.trim().to_owned())
        .collect()
}

fn _printed(blocks: Vec<LirBlock>) -> Vec<String> {
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
            block(1, vec![_compare(1), _branch(2, "je", 7)], vec![7, 4]),
            block(4, vec![_move(4, cx()), _return(5)], vec![]),
            block(7, vec![_jump(7, 9)], vec![9]),
            block(8, vec![_move(8, bx()), _return(8)], vec![]),
            block(9, vec![_return(9)], vec![]),
        ]),
        ["L0_1:", "cmp ax, bx", "je L0_9", "L0_4:", "mov ax, cx", "ret", "L0_9:", "ret"]
    );
}

#[test]
fn test_branch_over_a_jump_is_inverted() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_compare(1), _branch(2, "je", 9)], vec![9, 4]),
            block(4, vec![_jump(4, 12)], vec![12]),
            block(9, vec![_move(9, cx()), _return(10)], vec![]),
            block(12, vec![_move(12, bx()), _return(13)], vec![]),
        ]),
        ["L0_1:", "cmp ax, bx", "jne L0_12", "L0_9:", "mov ax, cx", "ret", "L0_12:", "mov ax, bx", "ret"]
    );
}

#[test]
fn test_branch_then_jump_in_one_block_is_inverted() {
    assert_eq!(
        _printed(vec![
            block(1, vec![_compare(1), _branch(2, "je", 4), _jump(3, 9)], vec![4, 9]),
            block(4, vec![_move(4, cx()), _return(5)], vec![]),
            block(9, vec![_move(9, bx()), _return(10)], vec![]),
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
            block(1, vec![_compare(1), _branch(2, "jg", 4)], vec![4, 3]),
            block(3, vec![_inserted(_jump(3, 7))], vec![7]),
            block(4, vec![_move(4, cx()), _jump(5, 7)], vec![7]),
            block(7, vec![_return(7)], vec![]),
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
            block(1, vec![_compare(1), _branch(2, "je", 4)], vec![4, 9]),
            block(4, vec![_return(4)], vec![]),
            block(9, vec![_return(9)], vec![]),
        ],
    );

    let result = crate::flow::machine(&IndexMap::default(), Some(Rc::new(RefCell::new(Frame::new(0)))), Some(&IndexMap::default()), false, "386")
        .unwrap()
        .pop()
        .unwrap()
        .transform(source).unwrap();

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
            block(1, vec![_jump(1, 2)], vec![2]),
            block(2, vec![marker, _compare(2), _branch(2, "je", 4), _jump(2, 3)], vec![4, 3]),
            block(3, vec![_jump(3, 2)], vec![2]),
            block(4, vec![_return(4)], vec![]),
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
            block(1, vec![_move(1, bx()), _jump(2, 4)], vec![4]),
            block(4, vec![_return(4)], vec![]),
        ]),
        ["L0_1:", "mov ax, bx", "L0_4:", "ret"]
    );
}

/// SCALAR lost the three decoded bytes of its preheader jump after unrolling.
#[test]
fn test_source_owned_jump_to_fallthrough_becomes_an_anchor() {
    let jump = _insn(1, Operation::Jump, "jmp", vec![], vec![], Some(4));
    let jump = Arc::new(Insn { covers: Some((1, 4)), ..(*jump).clone() });
    let source = body("f", 1, vec![block(1, vec![jump], vec![4]), block(4, vec![_return(4)], vec![])]);

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
            block(1, vec![_move(1, bx()), _jump(2, 9)], vec![9]),
            block(5, vec![], vec![]),
            block(9, vec![_return(9)], vec![]),
        ]),
        ["L0_1:", "mov ax, bx", "L0_9:", "ret"]
    );
}

fn nothing(at: i64, covers: Option<(i64, i64)>) -> Arc<Insn> {
    let what = Semantics { name: Some(String::new()), ..Semantics::new(Operation::Nothing) };
    Arc::new(Insn::new(at, covers, Some(what), Vec::new(), Vec::new()))
}

/// SCALAR unrolled to 1789, then final threading lost the old loop bytes.
#[test]
fn test_unreachable_inert_source_ownership_survives_threading() {
    let owned = nothing(5, Some((5, 9)));
    let source = body("f", 1, vec![block(1, vec![_return(1)], vec![]), block(5, vec![Arc::clone(&owned)], vec![])]);

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
            block(1, vec![_compare(1), _branch(2, "je", 7)], vec![7, 4]),
            block(4, vec![_return(4)], vec![]),
            block(7, vec![owned], vec![9]),
            block(9, vec![_return(9)], vec![]),
        ],
    );

    let result = threaded(&source);

    assert_eq!(result.blocks[0].succ, [9, 4]);
    assert_eq!(result.owned_bytes(), [7, 8, 9]);
}

/// A carried, non-generated LIR occurrence can have no contiguous `covers` span.
#[test]
fn test_unreachable_inert_carrier_without_a_byte_span_does_not_crash_threading() {
    let carrier = nothing(5, None);
    let source = body("f", 1, vec![block(1, vec![_return(1)], vec![]), block(5, vec![carrier], vec![])]);

    let result = threaded(&source);

    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1]);
}

/// PARITY's fully unrolled BASIC loop left only source-map anchors.
#[test]
fn test_reachable_inert_source_ownership_is_not_a_transparent_passage() {
    let owned = nothing(5, Some((5, 9)));
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_jump(1, 5)], vec![5]),
            block(5, vec![Arc::clone(&owned)], vec![9]),
            block(9, vec![_return(9)], vec![]),
        ],
    );

    let result = threaded(&source);

    assert_eq!(result.blocks.iter().map(|block| block.at).collect::<Vec<_>>(), [1, 5, 9]);
    assert_eq!(result.blocks[0].succ, [5]);
    assert_eq!(result.blocks[1].insns, [owned]);
}

/// Mandel's rewound coordinate made its outer header emit no bytes.
#[test]
fn test_threading_preserves_an_exact_empty_loop_header() {
    let mut source = body(
        "nested",
        1,
        vec![
            block(1, vec![_jump(1, 8)], vec![8]),
            block(8, vec![], vec![10]),
            block(10, vec![_compare(10), _branch(11, "je", 20)], vec![14, 20]),
            block(14, vec![_move(14, cx()), _jump(15, 10)], vec![10]),
            block(20, vec![_compare(20), _branch(21, "jne", 8)], vec![8, 30]),
            block(30, vec![_return(30)], vec![]),
        ],
    );
    source.loop_trip_counts = vec![(8, 24), (10, 32)];

    let result = threaded(&source);

    assert!(result.blocks.iter().any(|block| block.at == 8));
    let headers: std::collections::BTreeSet<i64> =
        loopy::loops(&intervals::_graph(&result.blocks), Some(result.entry)).iter().map(|one| one.header).collect();
    assert_eq!(headers, [8, 10].into());
}

/// `for (;;);` -- following jumps must not go round forever.
#[test]
fn test_a_block_that_jumps_to_itself_stays() {
    assert_eq!(_printed(vec![block(1, vec![_jump(1, 1)], vec![1])]), ["L0_1:", "jmp L0_1"]);
}

fn two_zero_tails(inserted: bool) -> LirBody {
    let fresh = |one: Arc<Insn>| if inserted { _inserted(one) } else { one };
    body(
        "f",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]),
            block(10, vec![fresh(_move(10, imm(0))), fresh(_jump(11, 30))], vec![30]),
            block(20, vec![fresh(_move(20, imm(0))), fresh(_jump(21, 30))], vec![30]),
            block(30, vec![_return(30)], vec![]),
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
    let result = crate::flow::machine(&IndexMap::default(), Some(Rc::new(RefCell::new(Frame::new(0)))), Some(&IndexMap::default()), false, "386")
        .unwrap()
        .pop()
        .unwrap()
        .transform(two_zero_tails(true)).unwrap();
    let physical = physical(&result);

    assert_eq!(physical.iter().filter(|what| what.op == Operation::Move).count(), 1);
    assert_eq!(physical.iter().filter(|what| what.op == Operation::Branch).count(), 0);
}

/// Unpriced tail sharing grew C sieve from 54 to 55 instructions.
#[test]
fn test_tail_sharing_rejects_a_static_saving_that_adds_hot_work() {
    let shaped = |entry: Vec<Arc<Insn>>, looped: Vec<Arc<Insn>>| {
        body(
            "f",
            1,
            vec![
                block(1, entry, vec![10]),
                block(10, looped, vec![30, 20]),
                block(20, vec![_jump(20, 10)], vec![10]),
                block(30, vec![_return(30)], vec![]),
            ],
        )
    };

    let before = shaped(
        vec![_move(1, bx()), _move(2, cx()), _move(3, imm(0)), _jump(4, 10)],
        vec![_compare(10), _branch(11, "je", 30)],
    );
    let after = shaped(vec![_jump(1, 10)], vec![_move(9, cx()), _compare(10), _branch(11, "je", 30)]);

    assert!(_work(&after).0 < _work(&before).0);
    assert!(_work(&after).1 > _work(&before).1);
    assert!(std::ptr::eq(preferred(&before, &after), &before));
}

/// choose's folded arms joined through a two-byte `jmp` to `pop bp; retf`.
#[test]
fn test_byte_neutral_return_duplication_removes_a_join_jump() {
    let source = body(
        "choose",
        1,
        vec![
            block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]),
            block(10, vec![_move(10, imm(8))], vec![30]),
            block(30, vec![_inserted(_return(30))], vec![]),
            block(20, vec![_move(20, imm(10)), _inserted(_jump(21, 30))], vec![30]),
        ],
    );

    let result = duplicated_returns(source, 1);
    let physical: Vec<Semantics> =
        result.blocks.iter().flat_map(|block| block.insns.iter()).filter_map(|one| one.what.clone()).collect();

    assert!(result.blocks.iter().all(|block| block.at != 30));
    assert_eq!(physical.iter().filter(|one| one.op == Operation::Return).count(), 2);
    assert!(physical.iter().all(|one| one.op != Operation::Jump));
}

/// A larger return tail, or one owning source bytes, stays shared.
#[test]
fn test_return_duplication_rejects_growth_and_source_owned_tails() {
    let candidate = |tail: Vec<Arc<Insn>>| {
        body(
            "f",
            1,
            vec![
                block(1, vec![_compare(1), _branch(2, "je", 20)], vec![20, 10]),
                block(10, vec![_move(10, ax())], vec![30]),
                block(30, tail, vec![]),
                block(20, vec![_move(20, bx()), _inserted(_jump(21, 30))], vec![30]),
            ],
        )
    };

    let large = candidate(vec![_inserted(_move(30, imm(1234))), _inserted(_return(31))]);
    let owned = candidate(vec![_return(30)]);

    // Python `is`: nothing changed, so the result equals the input.
    assert_eq!(duplicated_returns(large.clone(), 1), large);
    assert_eq!(duplicated_returns(owned.clone(), 1), owned);
}

/// The raise lays a C loop out test first, so entered at its body the
/// latch still jumped back to the test every pass.
#[test]
fn test_loop_test_is_placed_after_its_latch() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_jump(1, 8)], vec![8]),
            block(4, vec![_compare(4), _branch(5, "je", 23)], vec![23, 8]),
            block(8, vec![_move(8, cx()), _jump(9, 4)], vec![4]),
            block(23, vec![_return(23)], vec![]),
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
            block(1, vec![_move(1, cx())], vec![3]),
            block(3, vec![_compare(3), _branch(4, "jge", 17)], vec![17, 8]),
            block(8, vec![_move(8, bx()), _jump(9, 3)], vec![3]),
            block(17, vec![_return(17)], vec![]),
        ],
    );
    assert_eq!(
        inner(listed(threaded(&placed(&source).unwrap()), "_f")),
        ["L0_1:", "mov ax, cx", "jmp L0_3", "L0_8:", "mov ax, bx", "L0_3:", "cmp ax, bx", "jl L0_8", "L0_17:", "ret"]
    );
}

/// With the test placed after the latch, every pass took `jg out` and then `jmp body`.
#[test]
fn test_loop_test_is_followed_by_the_block_it_leaves_for() {
    let source = body(
        "f",
        1,
        vec![
            block(1, vec![_move(1, cx())], vec![3]),
            block(3, vec![_compare(3), _branch(4, "jge", 20)], vec![20, 8]),
            block(8, vec![_move(8, bx()), _jump(9, 3)], vec![3]),
            block(12, vec![_return(12)], vec![]),
            block(20, vec![_move(20, cx()), _jump(21, 12)], vec![12]),
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
            block(1, vec![_move(1, cx()), _jump(2, 12)], vec![12]),
            block(12, vec![_compare(12), _branch(13, "jge", 73)], vec![73, 16]),
            block(16, vec![_compare(16), _branch(17, "jge", 68)], vec![68, 57]),
            block(57, vec![_compare(57), _branch(58, "jne", 68)], vec![68, 73]),
            block(68, vec![_move(68, bx()), _jump(69, 12)], vec![12]),
            block(73, vec![_return(73)], vec![]),
        ],
    );
    let printed = inner(listed(threaded(&placed(&source).unwrap()), "_f"));
    assert!(!(0..printed.len() - 1).any(|index| printed[index].starts_with('j')
        && !printed[index].starts_with("jmp ")
        && printed[index + 1].starts_with("jmp ")));
}

/// An error call on the branch's fall-through edge was laid out before the return.
///
/// No frontend marked it cold; the call's NEVER contract is what says so.
#[test]
fn test_a_block_that_only_reaches_a_terminal_call_is_placed_after_the_return() {
    use crate::abi::runtime;
    use crate::backend::lower;
    use crate::model::mir::{self, Arg, Const, Held, Kind, MirBlock, MirBody};

    let x = mir::Value::new(1, 0);
    let flags = mir::Value { flags: true, ..mir::Value::new(2, 0) };
    let mut compare = mir::Op::new(1, mir::OpCode::Operation(Operation::Compare), "", vec![flags], vec![x]);
    compare.kind = Kind::Sub;
    compare.args = vec![Arg::Held(Held { value: x, width: 2 }), Arg::Const(Const::new(0, 2))];
    let mut branch = mir::Op::new(2, mir::OpCode::Operation(Operation::Branch), "", vec![], vec![flags]);
    branch.kind = Kind::Branch;
    branch.test = Some(Kind::Ge);
    branch.target = Some(20);
    let mut raised = mir::Op::new(10, mir::OpCode::Operation(Operation::Call), "call", vec![], vec![]);
    raised.kind = Kind::Call;
    raised.args_known = true;
    let mut returned = mir::Op::new(20, mir::OpCode::Operation(Operation::Return), "ret", vec![], vec![]);
    returned.kind = Kind::Return;
    let body = MirBody {
        sealed: true,
        ..MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![compare, branch], vec![10, 20]),
                MirBlock::new(10, vec![], vec![raised], vec![]),
                MirBlock::new(20, vec![], vec![returned], vec![]),
            ],
        )
    };
    let never = runtime::Contract {
        cleanup: Some(0),
        control: runtime::Control::Never,
        established: true,
        inputs: Some(BTreeSet::new()),
        ..runtime::worst("B$RUNERR")
    };
    let calls: IndexMap<i64, String> = [(10, "B$RUNERR".to_owned())].into_iter().collect();
    let contracts: IndexMap<i64, runtime::Contract> = [(10, never)].into_iter().collect();

    let low = lower::lowered("checked", &body, Some(&calls), BTreeSet::new(), Some(&contracts), "386", Default::default())
        .unwrap();

    assert_eq!(placed(&low).unwrap().blocks.iter().map(|block| block.at).collect::<Vec<_>>(), vec![0, 20, 10]);
}
