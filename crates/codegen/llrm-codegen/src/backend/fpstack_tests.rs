//! `FPS`'s handlers and bundle protocol, one behaviour each, run on the x87
//! simulator of `floatalloc`'s tests: the answers are checked as well as the
//! instructions.

use std::sync::Arc;

use super::converted;
use crate::backend::classes::RegisterClasses;
use crate::backend::floatalloc::_truncating;
use crate::backend::floatalloc::tests::{
    _along, _arithmetic, _body, _load, _store, _x87_traced, block, fl, frame_cell, name,
};
use crate::backend::frame::Frame;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{Insn, LirBody};

/// The stackifier on `body` as the pipeline hands it over: phis are copies, the
/// x87 values are in registers.
fn fps(body: &LirBody) -> LirBody {
    let lowered = crate::phielimination::lowered(body).unwrap();
    let stacked = converted(&lowered, &RegisterClasses::of(&llrm_x86_m16::M16)).unwrap();
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

/// A value spilled at 80 bits and then stored as a double (`b` of
/// `b = a*2.5: PRINT b: PRINT b+1`, QB at m16 -Os): `fstp tbyte` has no
/// non-popping form, so the pair was `fld st(0); fstp tbyte [spill]; fstp
/// qword [arg]`, 2 bytes more than `fst qword [arg]; fstp tbyte [spill]`.
#[test]
fn test_an_extended_spill_beside_a_final_store_stores_the_double_first() {
    let (source, spill, arg) = (frame_cell(-8, 8), frame_cell(-24, 10), frame_cell(-32, 8));
    let body = _body(vec![_load(1, &source), _store(&spill, 1), _store(&arg, 1)]);
    let result = fps(&body);
    assert_eq!(names(&result), ["fld", "fst", "fstp"]);
    let widths: Vec<u32> = result
        .insns()
        .iter()
        .filter_map(|one| match one.what.as_ref()?.dests.as_slice() {
            [crate::model::ir::Loc::Mem(cell)] => Some(cell.width),
            _ => None,
        })
        .collect();
    assert_eq!(widths, [8, 10], "the double first, the extended last (the pop)");
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&source, 3.5)]);
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![3.5, 3.5]);
    assert!(stack.is_empty());
}

/// The same pair into overlapping cells keeps its order: the last write wins.
#[test]
fn test_stores_to_overlapping_cells_are_not_swapped() {
    let (source, spill, arg) = (frame_cell(-8, 8), frame_cell(-24, 10), frame_cell(-20, 8));
    let body = _body(vec![_load(1, &source), _store(&spill, 1), _store(&arg, 1)]);
    let result = fps(&body);
    assert_eq!(names(&result), ["fld", "fld", "fstp", "fstp"]);
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
    let (live_in, live_out) = crate::backend::liverows::live_among(&body, |value| value == 100);
    let kept: usize = live_in.values().chain(live_out.values()).map(|set| set.iter().count()).sum();
    assert!(kept <= 2, "{kept} values in the sets");
    let rows = crate::backend::liverows::live_rows_by(&body, |value| value == 100);
    assert_eq!(rows.numbered(), 1, "the values numbered");
}

/// A port write leaves the x87 stack as it is: a value held across `out` stays
/// on the stack. Every `Barrier` was a boundary, so a value the allocator kept
/// in a register across an `out` was refused ("a floating value is live
/// across a boundary") and the QB demos qbdemo and deedlines did not compile.
#[test]
fn test_a_value_stays_on_the_stack_across_a_port_write() {
    let cell = frame_cell(-8, 8);
    let out = Semantics {
        op: Operation::Barrier,
        name: Some("out".into()),
        dests: vec![],
        sources: vec![],
        target: None,
        indirect: false,
        table: vec![],
        repeat: false,
        pops: None,
    };
    let body = _body(vec![_load(1, &cell), out, _store(&cell, 1)]);
    let converted = converted(&body, &RegisterClasses::of(&llrm_x86_m16::M16)).expect("a value across an out");
    assert_eq!(count(&converted, "out"), 1, "{:?}", names(&converted));
}
