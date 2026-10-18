//! `FPS`'s handlers and bundle protocol, one behaviour each, run on the x87
//! simulator of `floatalloc`'s tests: the answers are checked as well as the
//! instructions.

use std::sync::Arc;

use super::converted;
use crate::backend::classes::RegisterClasses;
use crate::backend::cpu::{self as targets};
use crate::backend::floatalloc::_truncating;
use crate::backend::floatalloc::tests::{
    _along, _arithmetic, _body, _load, _store, _x87_traced, block, fl, frame_cell, m, name,
};
use crate::backend::floatassign;
use crate::backend::frame::Frame;
use crate::backend::phielim;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};

/// As the pipeline runs it, with the translated stackifier in place of the old.
fn fps(body: &LirBody) -> LirBody {
    let target = targets::profile("386").unwrap();
    let assigned =
        floatassign::assigned(&phielim::eliminated(body).unwrap(), Some(&mut Frame::new(-64)), None, true, target)
            .unwrap();
    let stacked = converted(&assigned, &RegisterClasses::of(&llrm_x86_m16::M16)).unwrap();
    _truncating(&stacked, Some(&mut Frame::new(-64))).unwrap()
}

fn names(body: &LirBody) -> Vec<String> {
    body.insns().iter().map(|one| name(one).to_owned()).filter(|name| !name.is_empty()).collect()
}

fn count(
    body: &LirBody,
    wanted: &str,
) -> usize {
    names(body).iter().filter(|name| *name == wanted).count()
}

/// `handleTwoArgFP`: neither operand on top and one dies: that one is brought
/// up by an exchange and the result overwrites it, no copy.
#[test]
fn test_a_dying_operand_below_the_top_is_exchanged_up_not_copied() {
    // The stores to the first two cells keep `floatassign` from reading them as
    // memory operands: the values stay in registers.
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8), frame_cell(-32, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _load(3, &cells[2]),
        _store(&cells[0], 3),
        _store(&cells[1], 3),
        _arithmetic("fsub", 4, fl(1), fl(2)),
        _store(&cells[3], 4),
        _store(&cells[3], 3),
    ]);
    let result = fps(&body);
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 10.0), (&cells[1], 3.0), (&cells[2], 100.0)]);
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![100.0, 100.0, 7.0, 100.0]);
    assert!(stack.is_empty());
    assert_eq!(count(&result, "fld"), 3, "no copy: {:?}", names(&result));
    assert_eq!(count(&result, "fxch"), 1, "{:?}", names(&result));
}

/// `handleTwoArgFP`: both operands live after: one is copied to the top first.
#[test]
fn test_two_live_operands_are_computed_on_a_copy() {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _arithmetic("fadd", 3, fl(1), fl(2)),
        _store(&cells[2], 3),
        _store(&cells[2], 1),
        _store(&cells[2], 2),
        _store(&cells[0], 3),
    ]);
    let result = fps(&body);
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 1.0), (&cells[1], 2.0)]);
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![3.0, 1.0, 2.0, 3.0]);
    assert!(stack.is_empty());
    assert_eq!(count(&result, "fld"), 3, "two loads and the copy: {:?}", names(&result));
}

/// `popStackAfter`: both operands die, so the pop folds into the instruction.
#[test]
fn test_two_dying_operands_use_the_popping_form() {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _load(4, &cells[2]),
        _store(&cells[0], 4),
        _store(&cells[1], 4),
        _arithmetic("fmul", 3, fl(1), fl(2)),
        _store(&cells[2], 3),
        _store(&cells[2], 4),
    ]);
    let result = fps(&body);
    assert!(names(&result).contains(&"fmulp".to_owned()), "{:?}", names(&result));
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 3.0), (&cells[1], 4.0), (&cells[2], 5.0)]);
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![5.0, 5.0, 12.0, 5.0]);
    assert!(stack.is_empty());
}

/// `handleOneArgFP`: a store of a value that stays leaves it (`fst`), a store
/// of one that dies pops it.
#[test]
fn test_a_store_of_a_live_value_keeps_it_and_of_a_dead_one_pops_it() {
    let cell = frame_cell(-8, 8);
    let body = _body(vec![_load(1, &cell), _store(&cell, 1), _store(&cell, 1)]);
    let result = fps(&body);
    assert_eq!(names(&result), ["fld", "fst", "fstp"]);
}

/// `adjustLiveRegs`, `shuffleStackTop`: a value live across a branch to a join
/// is where the first exit put it on the other path too, by an exchange there.
#[test]
fn test_the_second_exit_into_a_bundle_shuffles_to_the_first_exits_order() {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8)];
    let mut body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        Semantics { target: Some(32), ..sem_branch() },
        _store(&cells[2], 1),
        _store(&cells[2], 2),
    ]);
    let insns = body.insns();
    body.blocks = vec![
        block(0, insns[..3].to_vec(), vec![16, 32]).into(),
        block(16, vec![], vec![32]).into(),
        block(32, insns[3..].to_vec(), vec![]).into(),
    ];
    let result = fps(&body);
    let insns = _along(&result, &[0, 32]);
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 1.0), (&cells[1], 2.0)]);
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![1.0, 2.0]);
    assert!(stack.is_empty());
    let via = _along(&result, &[0, 16, 32]);
    let (_, stack, stores) = _x87_traced(&via, &[(&cells[0], 1.0), (&cells[1], 2.0)]);
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![1.0, 2.0]);
    assert!(stack.is_empty());
}

fn sem_branch() -> Semantics {
    crate::backend::floatalloc::tests::sem(Operation::Branch, "jne", vec![], vec![])
}

/// A loop whose conditional arm updates two accumulators in a different stack
/// order than they enter it in, the shape of `nbody_single`'s inner loop (entry
/// 0, header 16, update 32, join 48): the header's order is fixed by the
/// entry, and the join's by whichever exit reached it first. Taking the order
/// that the back edge needs (DEVIATE 6) puts the one exchange the loop needs
/// on the update arm, and the arm that skips the update takes none. Before
/// that, the update arm fixed the join and the skip arm paid two exchanges a
/// trip: `fpbench` and `nbody_single` ran 0.3% to 0.7% more instructions.
#[test]
fn test_the_arm_that_skips_the_update_takes_no_exchange_in_a_loop() {
    let (a, b, k) = (frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8));
    let answer = frame_cell(-32, 8);
    // 0: u0, w0. 16: loop phis u (3), w (4); skip to 48 or go to 32. 32: w' (5)
    // then u' (6), so they come out in the opposite order to the header's.
    // 48: join phis u (7), w (8); back to 16 or on to 64.
    let mut body = _body(vec![
        _load(1, &a),
        _load(2, &b),
        Semantics { target: Some(48), ..sem_branch() },
        _arithmetic("fadd", 5, fl(4), m(&k)),
        _arithmetic("fadd", 6, fl(3), m(&k)),
        Semantics { target: Some(16), ..sem_branch() },
        _store(&answer, 7),
        _store(&answer, 8),
    ]);
    let insns = body.insns();
    body.blocks = vec![
        block(0, insns[..2].to_vec(), vec![16]).into(),
        LirBlock {
            phis: vec![
                Phi { result: 3, incoming: vec![(0, 1), (48, 7)] },
                Phi { result: 4, incoming: vec![(0, 2), (48, 8)] },
            ]
            .into(),
            ..block(16, insns[2..3].to_vec(), vec![32, 48])
        }
        .into(),
        block(32, insns[3..5].to_vec(), vec![48]).into(),
        LirBlock {
            phis: vec![
                Phi { result: 7, incoming: vec![(16, 3), (32, 6)] },
                Phi { result: 8, incoming: vec![(16, 4), (32, 5)] },
            ]
            .into(),
            ..block(48, insns[5..6].to_vec(), vec![16, 64])
        }
        .into(),
        block(64, insns[6..].to_vec(), vec![]).into(),
    ];
    let result = fps(&body);
    let exchanges = |path: &[i64]| _along(&result, path).iter().filter(|one| name(one) == "fxch").count();
    // The edge blocks the phi copies stand on: 1<<32|1 skips, 1<<32|2 is the
    // back edge.
    let skip = exchanges(&[16, (1 << 32) | 1, 48, (1 << 32) | 2, 16]);
    let update = exchanges(&[16, 32, 48, (1 << 32) | 2, 16]);
    let old = crate::backend::floatalloc::tests::run(&body);
    let before = |path: &[i64]| _along(&old, path).iter().filter(|one| name(one) == "fxch").count();
    let (old_skip, old_update) =
        (before(&[16, (1 << 32) | 1, 48, (1 << 32) | 2, 16]), before(&[16, 32, 48, (1 << 32) | 2, 16]));
    assert!(skip <= old_skip, "the arm that skips the update exchanges {skip} times a trip, the old pass {old_skip}");
    assert!(update <= old_update, "the update arm exchanges {update} times, the old pass {old_update}");
}

/// The pass is one walk of each block and one `adjustLiveRegs` and shuffle at
/// each border: a chain of blocks, one float live through all of them, costs
/// instructions in proportion to the blocks.
#[test]
fn test_a_chain_of_blocks_costs_in_proportion_to_its_length() {
    let chain = |blocks: i64| {
        let cell = frame_cell(-8, 8);
        let mut all: Vec<Arc<Insn>> = _body(vec![_load(1, &cell), _store(&cell, 1)]).insns().to_vec();
        let last = all.pop().unwrap();
        let mut body = _body(vec![]);
        let mut made: Vec<crate::model::lir::Block> = vec![block(0, all, vec![16]).into()];
        for k in 1..blocks {
            made.push(block(k * 16, vec![], vec![(k + 1) * 16]).into());
        }
        made.push(block(blocks * 16, vec![last], vec![]).into());
        body.blocks = made;
        fps(&body).insns().len()
    };
    let (small, big) = (chain(200), chain(400));
    assert!(big <= 2 * small + 8, "{small} instructions for 200 blocks, {big} for 400");
}

/// DEVIATE 5: `v*v` then `v+v` then a use of both: `handleTwoArgFP` puts the
/// product on top and the next read of `v` needs an exchange; the product
/// goes below the copy instead, so none follows. FPDEEP's `p*p` shuffled `p`
/// back to the top.
#[test]
fn test_deviate_5_a_square_keeps_the_value_read_next_on_top() {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        _store(&cells[0], 1),
        _arithmetic("fmul", 2, fl(1), fl(1)),
        _arithmetic("fadd", 3, fl(1), fl(1)),
        _arithmetic("fdiv", 4, fl(2), fl(3)),
        _store(&cells[1], 4),
    ]);
    let result = fps(&body);
    assert_eq!(count(&result, "fxch"), 0, "{:?}", names(&result));
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 3.0)]);
    assert_eq!(stores.last().map(|(_, value)| *value), Some(1.5));
    assert!(stack.is_empty());
}

/// DEVIATE 7: a branch that skips a block, whose other arm reads two floats the
/// join does not: the pops belong on the skip edge. Without the split the join
/// pops two values on entry, and the arm that read them defines two (`fldz`)
/// to match: `nbody`'s inner loop exit, 4 instructions a trip.
#[test]
fn test_deviate_7_values_the_join_does_not_read_are_popped_on_the_edge_that_brings_them() {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8)];
    let mut body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _store(&cells[0], 2),
        Semantics { target: Some(48), ..sem_branch() },
        _store(&cells[2], 1),
        _store(&cells[2], 2),
        _store(&cells[1], 2),
    ]);
    let insns = body.insns();
    // 0 loads and branches; 32 reads both and joins at 48, which reads none.
    body.blocks = vec![
        block(0, insns[..4].to_vec(), vec![32, 48]).into(),
        block(32, insns[4..6].to_vec(), vec![48]).into(),
        block(48, vec![], vec![]).into(),
    ];
    let result = fps(&body);
    let arm = _along(&result, &[0, 32, 48]);
    assert!(arm.iter().all(|one| name(one) != "fldz"), "the arm that read them defines them again");
    let (_, stack, _) = _x87_traced(&arm, &[(&cells[0], 1.0), (&cells[1], 2.0)]);
    assert!(stack.is_empty());
    let skip = _along(&result, &[0, 48]);
    let (_, stack, _) = _x87_traced(&skip, &[(&cells[0], 1.0), (&cells[1], 2.0)]);
    assert!(stack.is_empty());
}

/// Liveness for the stack was solved over every value of the body and then
/// cut to the floating ones: a body of integer work with one float paid for
/// all of it (11 M of d_faces -O2's 26.6 M in `fpstack::converted`).
#[test]
fn test_liveness_for_the_stack_numbers_only_the_floating_values() {
    let body = {
        let mut insns: Vec<std::sync::Arc<crate::model::lir::Insn>> = Vec::new();
        for n in 0..40u32 {
            insns.push(std::sync::Arc::new(crate::model::lir::Insn::new(n as i64, None, None, vec![100 + n], vec![])));
        }
        crate::model::lir::LirBody::new(
            "t",
            0,
            vec![crate::model::lir::LirBlock::new(0, insns).into()],
            Default::default(),
            Default::default(),
        )
    };
    let (live_in, live_out) = crate::backend::allocate::live_among(&body, |value| value == 100);
    let kept: usize = live_in.values().chain(live_out.values()).map(|set| set.iter().count()).sum();
    assert!(kept <= 2, "{kept} values in the sets");
    let rows = crate::backend::allocate::live_rows_by(&body, |value| value == 100);
    assert_eq!(rows.numbered(), 1, "the values numbered");
}
