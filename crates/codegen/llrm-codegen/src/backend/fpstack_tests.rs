//! `FPS`'s handlers and bundle protocol, one behaviour each, run on the x87
//! simulator of `floatalloc`'s tests: the answers are checked as well as the
//! instructions.

use std::sync::Arc;

use super::{converted, converted_with};
use crate::backend::classes::RegisterClasses;
use crate::backend::floatalloc::_truncating;
use crate::backend::floatalloc::tests::{
    _along, _arithmetic, _body, _load, _store, _x87_traced, block, fl, frame_cell, m, name, sem,
};
use crate::backend::frame::Frame;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{Insn, LirBody};

/// The stackifier on `body` as the pipeline hands it over: phis are copies, the
/// x87 values are in registers.
fn fps(body: &LirBody) -> LirBody {
    let lowered = crate::phielimination::lowered(body).unwrap();
    let stacked = converted(&lowered, &RegisterClasses::of(&llrm_x86_m16::M16), None).unwrap();
    _truncating(&stacked, Some(&mut Frame::new(-64))).unwrap()
}

/// `fps` with the target's prices, as the pipeline has it.
fn fps_priced(
    body: &LirBody,
    cpu: &str,
) -> LirBody {
    let lowered = crate::phielimination::lowered(body).unwrap();
    let cpu = crate::backend::cpu::profile(cpu).expect("a profile");
    let stacked = converted(&lowered, &RegisterClasses::of(&llrm_x86_m16::M16), Some(cpu)).unwrap();
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
        table_id: None,
        pops: None,
    };
    let body = _body(vec![_load(1, &cell), out, _store(&cell, 1)]);
    let converted = converted(&body, &RegisterClasses::of(&llrm_x86_m16::M16), None).expect("a value across an out");
    assert_eq!(count(&converted, "out"), 1, "{:?}", names(&converted));
}

/// A loop whose header wants its two floats in the order latch 2 leaves them
/// and latch 1 does not; both reach the header through `chain` blocks that only
/// rename. Returns the exchanges on the path through each latch.
fn latch_paths(chain: i64) -> (usize, usize) {
    use crate::model::lir::{LirBlock, Phi};
    let fchs = crate::backend::floatalloc::tests::fchs;
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8)];
    let (b1, b2, r1) = (32, 64, 96);
    let mut body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        // latch 1 leaves (7 on top, 8): the header's top is the second.
        fchs(8, 4),
        fchs(7, 3),
        // latch 2 leaves (6 on top, 5).
        fchs(5, 3),
        fchs(6, 4),
    ]);
    let insns = body.insns();
    let mut blocks: Vec<crate::model::lir::Block> = vec![
        block(0, insns[..2].to_vec(), vec![16]).into(),
        LirBlock {
            phis: vec![
                Phi { result: 3, incoming: vec![(0, 1), (r1 + 16 * (chain - 1), 9)] },
                Phi { result: 4, incoming: vec![(0, 2), (r1 + 16 * (chain - 1), 10)] },
            ]
            .into(),
            ..block(16, vec![], vec![b1, b2])
        }
        .into(),
        block(b1, insns[2..4].to_vec(), vec![r1]).into(),
        block(b2, insns[4..6].to_vec(), vec![r1]).into(),
    ];
    for k in 0..chain {
        let at = r1 + 16 * k;
        let phis = if k == 0 {
            vec![
                Phi { result: 9, incoming: vec![(b1, 7), (b2, 5)] },
                Phi { result: 10, incoming: vec![(b1, 8), (b2, 6)] },
            ]
        } else {
            Vec::new()
        };
        let next = if k + 1 < chain { at + 16 } else { 16 };
        blocks.push(LirBlock { phis: phis.into(), ..block(at, vec![], vec![next]) }.into());
    }
    body.blocks = blocks;
    let lowered = crate::phielimination::lowered(&body).unwrap();
    let stacked = converted(&lowered, &RegisterClasses::of(&llrm_x86_m16::M16), None).unwrap();
    let in_block = |at: i64| {
        stacked
            .blocks
            .iter()
            .find(|block| block.at == at)
            .map_or(0, |block| block.insns.iter().filter(|one| name(one) == "fxch").count())
    };
    let chain_fxch: usize = (0..chain).map(|k| in_block(r1 + 16 * k)).sum();
    (in_block(b1) + chain_fxch, in_block(b2) + chain_fxch)
}

/// DEVIATE 6 followed the renaming blocks ahead of a fixed bundle at most
/// three deep (`DEPTH`): with four or more, the latch that fixes the bundle
/// chose its own order and the other latch exchanged twice, once to match it
/// and once more at the header (4 exchanges on its path, 2 with the order
/// taken from the header).
#[test]
fn test_the_order_a_header_wants_is_taken_through_any_number_of_renaming_blocks() {
    let costs: Vec<(usize, usize)> = [2, 3, 6, 9].iter().map(|chain| latch_paths(*chain)).collect();
    assert!(
        costs.iter().all(|cost| *cost == (2, 2)),
        "exchanges on the two latches' paths, by chain 2, 3, 6, 9: {costs:?}"
    );
}

/// `t += (float)v` below two constants: `fadd [m]` needs `t` on top, so the
/// stackifier made two `fxch` a trip (x_ld_to_float -O2, 1,211 clocks over
/// gcc's `fld [m]; faddp st(2)`). Where the target prices an exchange above
/// the load, the load is made again and the operation writes the operand's
/// slot.
fn memory_form_below_the_top(cpu: &str) -> (LirBody, Vec<f64>) {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8), frame_cell(-32, 8), frame_cell(-40, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _load(3, &cells[2]),
        _store(&cells[0], 3),
        _store(&cells[1], 3),
        // v1 dies here, one below v2, which the stores below still read.
        sem(Operation::FloatArith, "fsub", vec![fl(4)], vec![fl(1), m(&cells[2])]),
        _store(&cells[4], 2),
        _store(&cells[3], 4),
    ]);
    let result = fps_priced(&body, cpu);
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 10.0), (&cells[1], 3.0), (&cells[2], 100.0)]);
    assert!(stack.is_empty());
    (result, stores.iter().map(|(_, value)| *value).collect())
}

#[test]
fn test_a_memory_form_below_the_top_is_a_load_and_a_popping_op_where_exchange_costs() {
    let (result, stores) = memory_form_below_the_top("486");
    assert_eq!(stores, vec![100.0, 100.0, 3.0, -90.0], "{:?}", names(&result));
    assert_eq!(count(&result, "fxch"), 0, "{:?}", names(&result));
}

/// x_ld_axpy -O2 m32 with FF2: `r += 1` is `fadd [K]` (1.0 in the pool) on a
/// `r` below the top, made `fld [K]; faddp st(2)`: a load of 6 bytes and 5
/// clocks where the constant's own `fld1` is 2 and 4 (+8 clocks, +4 B over the
/// program). A pool cell holding 1.0 or 0.0 is loaded by `fld1` or `fldz`.
fn memory_form_of_a_constant_below_the_top(constants: bool) -> LirBody {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8), frame_cell(-32, 8), frame_cell(-40, 8)];
    let mut pool = crate::backend::constpool::Pool::new(1000);
    let one = pool.cell(1.0_f32.to_le_bytes().to_vec());
    let body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _load(3, &cells[2]),
        _store(&cells[0], 3),
        _store(&cells[1], 3),
        sem(Operation::FloatArith, "fadd", vec![fl(4)], vec![fl(1), m(&one)]),
        _store(&cells[4], 2),
        _store(&cells[3], 4),
    ]);
    let lowered = crate::phielimination::lowered(&body).unwrap();
    let cpu = crate::backend::cpu::profile("486").expect("a profile");
    let cells = if constants { pool.x87_cells() } else { Default::default() };
    let stacked = converted_with(&lowered, &RegisterClasses::of(&llrm_x86_m16::M16), Some(cpu), &cells).unwrap();
    _truncating(&stacked, Some(&mut Frame::new(-64))).unwrap()
}

#[test]
fn test_a_pool_constant_made_a_load_again_is_loaded_by_its_own_instruction() {
    let kept = memory_form_of_a_constant_below_the_top(true);
    assert_eq!((count(&kept, "fld1"), count(&kept, "fxch")), (1, 0), "{:?}", names(&kept));
    let plain = memory_form_of_a_constant_below_the_top(false);
    assert_eq!((count(&plain, "fld1"), count(&plain, "fxch")), (0, 0), "{:?}", names(&plain));
}

/// A P5 pairs `fxch` with the operation: the memory form keeps it.
#[test]
fn test_a_memory_form_below_the_top_keeps_its_exchange_where_exchange_is_free() {
    let (result, stores) = memory_form_below_the_top("P5");
    assert_eq!(stores, vec![100.0, 100.0, 3.0, -90.0], "{:?}", names(&result));
    assert_eq!(count(&result, "fxch"), 2, "{:?}", names(&result));
}

/// `a - [m]` where `a` is read again: the pin copies `a` (`fld st(i)`) and
/// subtracts the cell (nbody -O2 m16: two copies a trip, 4 clocks each on a 486
/// against 3 for `fld m`). Where the target prices the copy and the memory form
/// above the load and the register form, the cell is loaded and `fsubr st,
/// st(i)` reads `a` where it lies.
fn memory_form_on_a_copy(cpu: &str) -> (LirBody, Vec<f64>) {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8), frame_cell(-32, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        sem(Operation::FloatArith, "fsub", vec![fl(2)], vec![fl(1), m(&cells[1])]),
        _store(&cells[2], 2),
        _store(&cells[3], 1),
    ]);
    let result = fps_priced(&body, cpu);
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 10.0), (&cells[1], 3.0)]);
    assert!(stack.is_empty());
    (result, stores.iter().map(|(_, value)| *value).collect())
}

#[test]
fn test_a_memory_form_on_a_copy_is_a_load_and_a_register_op_where_the_copy_costs() {
    let (result, stores) = memory_form_on_a_copy("486");
    assert_eq!(stores, vec![7.0, 10.0], "{:?}", names(&result));
    assert_eq!((count(&result, "fsub"), count(&result, "fsubr")), (0, 1), "{:?}", names(&result));
}

/// A P5 copies in the clock a load takes: the pin's memory form stays.
#[test]
fn test_a_memory_form_on_a_copy_keeps_its_copy_where_the_copy_is_free() {
    let (result, stores) = memory_form_on_a_copy("P5");
    assert_eq!(stores, vec![7.0, 10.0], "{:?}", names(&result));
    assert_eq!((count(&result, "fsub"), count(&result, "fsubr")), (1, 0), "{:?}", names(&result));
}

/// x_ld_max and x_ld_axpy at a 64-bit `long double` (+2 B each, axpy +3
/// clocks): the result is stored first, so the exchange that brought the
/// operand up also put the result on top, and the unfolded form added a `fld`
/// and a `faddp` for nothing.
#[test]
fn test_a_memory_form_whose_result_is_read_first_keeps_its_exchange() {
    let cells = [frame_cell(-8, 8), frame_cell(-16, 8), frame_cell(-24, 8), frame_cell(-32, 8), frame_cell(-40, 8)];
    let body = _body(vec![
        _load(1, &cells[0]),
        _load(2, &cells[1]),
        _load(3, &cells[2]),
        _store(&cells[0], 3),
        _store(&cells[1], 3),
        sem(Operation::FloatArith, "fsub", vec![fl(4)], vec![fl(1), m(&cells[2])]),
        _store(&cells[3], 4),
        _store(&cells[4], 2),
    ]);
    let result = fps_priced(&body, "486");
    let insns = result.insns();
    let (_, stack, stores) = _x87_traced(&insns, &[(&cells[0], 10.0), (&cells[1], 3.0), (&cells[2], 100.0)]);
    assert!(stack.is_empty());
    assert_eq!(stores.iter().map(|(_, value)| *value).collect::<Vec<_>>(), vec![100.0, 100.0, -90.0, 3.0]);
    assert_eq!(count(&result, "fld"), 3, "{:?}", names(&result));
}
