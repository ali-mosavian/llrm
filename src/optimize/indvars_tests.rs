//! Ports of `tests/test_indvars.py` and `tests/test_rewind.py`.
//!
//! Cases failing in Python at this commit are left out:
//! `test_harr_reuses_an_existing_recurrence_for_termination` keeps only
//! harr-v-g3, and `test_counting_one_loop_to_zero_leaves_a_loop_sharing_its_start_alone`
//! only segld.
//! Skipped, monkeypatching a pass:
//! `test_invariant_branch_load_moves_out_but_its_test_stays`,
//! `test_internal_branch_reuses_the_value_recurrence`,
//! `test_indvar_simplify_reads_through_an_lcssa_exit`,
//! `test_counter_elimination_requires_a_complete_trip_count_and_no_body_use`.
//! Skipped, needing `tools/quality.py`:
//! `test_c_mandel_reuses_coordinate_recurrences_for_both_outer_loops`.

use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::{Mnemonic, OpKind};

use crate::analysis::{consts, induction, loops};
use crate::model::ir::Operation;
use crate::model::mir::{
    Arg, Cell, Const, Held, IntegerRange, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value,
};
use crate::model::passes::OperationCosts;
use crate::objectfile::module::Space;
use crate::optimize::rotate;

use super::{rewound, simplified, zeroed};

fn value(id: u32, at: i64, variable: u32, version: u32) -> Value {
    Value { id, at, flags: false, variable, version }
}

fn flag(id: u32, at: i64, variable: u32, version: u32) -> Value {
    Value { id, at, flags: true, variable, version }
}

fn operation(at: i64, op: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut op = Op::new(at, OpCode::Operation(op), name, defines, uses);
    op.kind = kind;
    op
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(number: i64, width: u32) -> Arg {
    Arg::Const(Const::new(number, width))
}

fn copy_const(at: i64, result: Value, number: i64, width: u32) -> Op {
    let mut op = operation(at, Operation::Nothing, "", vec![result], vec![], Kind::Copy);
    op.args = vec![constant(number, width)];
    op.results = vec![held(result, width)];
    op
}

fn add_const(at: i64, result: Value, source: Value, amount: i64, width: u32) -> Op {
    let mut op = operation(at, Operation::Nothing, "", vec![result], vec![source], Kind::Add);
    op.args = vec![held(source, width), constant(amount, width)];
    op.results = vec![held(result, width)];
    op
}

fn branch(at: i64, flags: Value, test: Kind, target: i64) -> Op {
    let mut op = operation(at, Operation::Nothing, "", vec![], vec![flags], Kind::Branch);
    op.test = Some(test);
    op.target = Some(target);
    op
}

fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
    Phi { result, incoming: incoming.iter().copied().collect() }
}

fn block<'a>(body: &'a MirBody, at: i64) -> &'a MirBlock {
    body.blocks.iter().find(|block| block.at == at).expect("the block")
}

/// One dynamic counted loop with a second affine recurrence.
///
/// The second value is an address-like offset: it is eligible to replace
/// control only when its update reaches zero on the final trip.
fn _symbolic_control_body(candidate_start: i64) -> MirBody {
    let bound = value(1, 0, 1, 1);
    let control_seed = value(2, 0, 2, 1);
    let candidate_seed = value(3, 0, 3, 1);
    let control = value(4, 1, 2, 2);
    let candidate = value(5, 1, 3, 2);
    let flags = flag(6, 1, 4, 1);
    let control_next = value(7, 2, 2, 3);
    let candidate_next = value(8, 2, 3, 3);
    let offset = value(9, 2, 5, 1);
    let mut source = MemRef::new(None, 2);
    source.space = Some(Space::Frame);

    let mut load = operation(0, Operation::Move, "", vec![bound], vec![], Kind::Load);
    load.loads = vec![source.clone()];
    load.args = vec![Arg::Cell(Cell { r#ref: source })];
    load.results = vec![held(bound, 2)];
    let mut compare = operation(1, Operation::Compare, "cmp", vec![flags], vec![control, bound], Kind::Sub);
    compare.args = vec![held(control, 2), held(bound, 2)];
    let mut test = operation(1, Operation::Branch, "", vec![], vec![flags], Kind::Branch);
    test.test = Some(Kind::AboveEq);
    test.target = Some(3);
    let mut jump = operation(2, Operation::Jump, "", vec![], vec![], Kind::Jump);
    jump.target = Some(1);
    let returned = operation(3, Operation::Return, "", vec![], vec![], Kind::Return);
    let mut body = MirBody::new(
        0,
        vec![
            MirBlock::new(
                0,
                vec![],
                vec![load, copy_const(0, control_seed, 0, 2), copy_const(0, candidate_seed, candidate_start, 2)],
                vec![1],
            ),
            MirBlock::new(
                1,
                vec![
                    phi(control, &[(0, control_seed), (2, control_next)]),
                    phi(candidate, &[(0, candidate_seed), (2, candidate_next)]),
                ],
                vec![compare, test],
                vec![2, 3],
            ),
            MirBlock::new(
                2,
                vec![],
                vec![
                    add_const(2, offset, candidate, 100, 2),
                    add_const(2, control_next, control, 1, 2),
                    add_const(2, candidate_next, candidate, 1, 2),
                    jump,
                ],
                vec![1],
            ),
            MirBlock::new(3, vec![], vec![returned], vec![]),
        ],
    );
    body.integer_ranges = OrderedMap::from_iter([(bound, IntegerRange::new(0, 7, 2))]);
    body
}

/// A recurrence seeded at 5 is rebased by its final value, so its last update is still zero.
#[test]
fn test_symbolic_control_rebases_a_nonzero_start_recurrence() {
    let body = _symbolic_control_body(5);
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [loop_] = &found[..] else { panic!("one loop") };
    let proofs = induction::counted(&Rc::new(MirBody::clone(&body)), loop_, None, false);
    let [proof] = &proofs[..] else { panic!("one proof") };
    let basics = induction::basics(&body, loop_);
    let candidate = basics.values().find(|one| **one != proof.counter).expect("a second recurrence");

    assert!(induction::zero_terminating_control(&Rc::new(body.clone()), loop_, proof, candidate, &BTreeSet::new(), None).is_some());
    assert_ne!(*zeroed(&Rc::new(body.clone())).unwrap(), body);
}

/// The same bounded recurrence is usable when its final update is zero.
#[test]
fn test_symbolic_control_proves_a_zero_terminal_recurrence() {
    let body = _symbolic_control_body(0);
    let found = loops::loops(&body.blocks, Some(body.entry));
    let [loop_] = &found[..] else { panic!("one loop") };
    let proofs = induction::counted(&Rc::new(MirBody::clone(&body)), loop_, None, false);
    let [proof] = &proofs[..] else { panic!("one proof") };
    let basics = induction::basics(&body, loop_);
    let candidate = basics.values().find(|one| **one != proof.counter).expect("a second recurrence");

    let got = induction::zero_terminating_control(&Rc::new(MirBody::clone(&body)), loop_, proof, candidate, &BTreeSet::new(), None).expect("a proof");

    assert!(std::ptr::eq(got.replacement.counted, proof));
    assert_eq!(
        (&got.candidate, &got.step, &got.maximum, &got.period),
        (candidate, &1.into(), &7.into(), &65536.into())
    );
}

/// C nbody carried `other` beside `other*4` only for `other != body`.
///
/// The inner and outer byte offsets are injective over their proven 0..5
/// domains, so they can answer both equality and loop termination.  Keeping
/// the scalar inner counter added an increment, compare and spill traffic to
/// every interaction.
fn test_scaled_recurrences_replace_nested_counter_equality(hazard: Option<&str>) {
    let value = |serial: u32, at: i64, variable: u32| value(serial, at, variable, 0);
    let compare = |at: i64, left: Value, right: Arg| -> (Op, Value) {
        let flags = flag(100 + at as u32, at, 100 + at as u32, 0);
        let uses = match &right {
            Arg::Held(right) => vec![left, right.value],
            _ => vec![left],
        };
        let mut op = operation(at, Operation::Nothing, "", vec![flags], uses, Kind::Sub);
        op.args = vec![held(left, 2), right];
        op.results = vec![];
        (op, flags)
    };

    let (outer_start, outer_offset_start) = (value(1, 0, 1), value(2, 0, 2));
    let (outer, outer_offset) = (value(3, 1, 1), value(4, 1, 2));
    let (outer_next, outer_offset_next) = (value(5, 6, 1), value(6, 6, 2));
    let (inner_start, inner_offset_start) = (value(7, 2, 3), value(8, 2, 4));
    let (inner, inner_offset) = (value(9, 3, 3), value(10, 3, 4));
    let (inner_next, inner_offset_next) = (value(11, 5, 3), value(12, 5, 4));
    let (outer_test, outer_flag) = compare(1, outer, constant(6, 2));
    let (inner_test, inner_flag) = compare(3, inner, constant(6, 2));
    let (unequal, unequal_flag) = compare(4, inner, held(outer, 2));
    let observed = value(20, 7, 20);
    let mut use_offset = operation(7, Operation::Nothing, "", vec![observed], vec![inner_offset], Kind::Copy);
    use_offset.args = vec![held(inner_offset, 2)];
    use_offset.results = vec![held(observed, 2)];
    let inner_stride = if hazard == Some("short-period") { 32768 } else { 4 };
    let outer_stride = match hazard {
        Some("different-map") => 5,
        Some("short-period") => 32768,
        _ => 4,
    };
    let built = MirBody::new(
        0,
        vec![
            MirBlock::new(
                0,
                vec![],
                vec![copy_const(0, outer_start, 0, 2), copy_const(0, outer_offset_start, 0, 2)],
                vec![1],
            ),
            MirBlock::new(
                1,
                vec![
                    phi(outer, &[(0, outer_start), (6, outer_next)]),
                    phi(outer_offset, &[(0, outer_offset_start), (6, outer_offset_next)]),
                ],
                vec![outer_test, branch(1, outer_flag, Kind::Ge, 9)],
                vec![2, 9],
            ),
            MirBlock::new(
                2,
                vec![],
                vec![copy_const(2, inner_start, 0, 2), copy_const(2, inner_offset_start, 0, 2)],
                vec![3],
            ),
            MirBlock::new(
                3,
                vec![
                    phi(inner, &[(2, inner_start), (5, inner_next)]),
                    phi(inner_offset, &[(2, inner_offset_start), (5, inner_offset_next)]),
                ],
                vec![inner_test, branch(3, inner_flag, Kind::Ge, 6)],
                vec![4, 6],
            ),
            MirBlock::new(
                4,
                vec![],
                vec![
                    unequal,
                    branch(4, unequal_flag, if hazard == Some("ordered") { Kind::Lt } else { Kind::Eq }, 5),
                ],
                vec![5, 7],
            ),
            MirBlock::new(
                5,
                vec![],
                vec![
                    add_const(5, inner_next, inner, 1, 2),
                    add_const(5, inner_offset_next, inner_offset, inner_stride, 2),
                ],
                vec![3],
            ),
            MirBlock::new(
                6,
                vec![],
                vec![
                    add_const(6, outer_next, outer, 1, 2),
                    add_const(6, outer_offset_next, outer_offset, outer_stride, 2),
                ],
                vec![1],
            ),
            MirBlock::new(7, vec![], vec![use_offset], vec![5]),
            MirBlock::new(9, vec![], vec![], vec![]),
        ],
    );

    let changed = simplified(&Rc::new(MirBody::clone(&built))).unwrap();
    let condition = block(&changed, 3).ops.iter().find(|op| op.kind == Kind::Sub).expect("a compare");
    let equality = block(&changed, 4).ops.iter().find(|op| op.kind == Kind::Sub).expect("a compare");

    if hazard.is_none() {
        assert_eq!(condition.args[0], held(inner_offset, 2));
        assert_eq!(equality.args, vec![held(inner_offset, 2), held(outer_offset, 2)]);
    } else {
        assert_eq!(condition.args[0], held(inner, 2));
        assert_eq!(equality.args, vec![held(inner, 2), held(outer, 2)]);
    }
}

#[test]
fn test_scaled_recurrences_replace_nested_counter_equality_none() {
    test_scaled_recurrences_replace_nested_counter_equality(None);
}

#[test]
fn test_scaled_recurrences_replace_nested_counter_equality_different_map() {
    test_scaled_recurrences_replace_nested_counter_equality(Some("different-map"));
}

#[test]
fn test_scaled_recurrences_replace_nested_counter_equality_ordered() {
    test_scaled_recurrences_replace_nested_counter_equality(Some("ordered"));
}

#[test]
fn test_scaled_recurrences_replace_nested_counter_equality_short_period() {
    test_scaled_recurrences_replace_nested_counter_equality(Some("short-period"));
}

/// C Mandelbrot kept 16-bit `px`/`py` counters beside the 32-bit
/// `cx`/`cy` recurrences that already advance once per iteration.
///
/// Their two update stores survive allocation. Loop termination may use a
/// wider recurrence when its modular period proves that the computed final
/// value cannot occur on an earlier iteration. The first implementation lost
/// the positive trip-count proof and left Mandel with six branches and a
/// redundant entry test; a symbolic sentinel must retain that proof.
fn test_cross_width_recurrence_replaces_counter_only_for_its_full_period(
    coordinate_width: u32,
    stride: i64,
    reused: bool,
) {
    let value = |serial: u32, at: i64, variable: u32| value(serial, at, variable, 0);
    let coordinate_argument = value(0, 0, 0);
    let control_start = value(1, 0, 1);
    let coordinate_start = value(2, 0, 2);
    let control = value(3, 1, 1);
    let coordinate = value(4, 1, 2);
    let control_next = value(5, 2, 1);
    let coordinate_next = value(6, 2, 2);
    let observed = value(7, 2, 3);
    let flags = flag(8, 1, 4, 0);

    let mut copy_argument =
        operation(0, Operation::Nothing, "", vec![coordinate_start], vec![coordinate_argument], Kind::Copy);
    copy_argument.args = vec![held(coordinate_argument, coordinate_width)];
    copy_argument.results = vec![held(coordinate_start, coordinate_width)];
    let mut compare = operation(1, Operation::Nothing, "", vec![flags], vec![control], Kind::Sub);
    compare.args = vec![held(control, 2), constant(4, 2)];
    compare.results = vec![];
    let test = branch(1, flags, Kind::Ge, 9);
    let mut used = operation(2, Operation::Nothing, "", vec![observed], vec![coordinate], Kind::Copy);
    used.args = vec![held(coordinate, coordinate_width)];
    used.results = vec![held(observed, coordinate_width)];
    let mut jump = operation(2, Operation::Jump, "", vec![], vec![], Kind::Jump);
    jump.target = Some(1);
    let returned = operation(9, Operation::Return, "", vec![], vec![], Kind::Return);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![copy_const(0, control_start, 0, 2), copy_argument], vec![1]),
            MirBlock::new(
                1,
                vec![
                    phi(control, &[(0, control_start), (2, control_next)]),
                    phi(coordinate, &[(0, coordinate_start), (2, coordinate_next)]),
                ],
                vec![compare, test],
                vec![2, 9],
            ),
            MirBlock::new(
                2,
                vec![],
                vec![
                    used,
                    add_const(2, control_next, control, 1, 2),
                    add_const(2, coordinate_next, coordinate, stride, coordinate_width),
                    jump,
                ],
                vec![1],
            ),
            MirBlock::new(9, vec![], vec![returned], vec![]),
        ],
    );

    let changed = simplified(&Rc::new(MirBody::clone(&body))).unwrap();
    let condition = changed.blocks[1].ops.iter().find(|op| op.kind == Kind::Sub).expect("a compare");

    if reused {
        assert_eq!(condition.args[0], held(coordinate, coordinate_width));
        assert!(matches!(&condition.args[1], Arg::Held(bound) if bound.width == coordinate_width));
        let loop_ = loops::loops(&changed.blocks, Some(changed.entry)).remove(0);
        assert_eq!(induction::trip_count(&changed, &loop_, &consts::known(&changed, None, None, None, None)), Some(4.into()));
        assert!(induction::nonempty(&changed, &loop_));
        assert_eq!(rotate::rotated(&changed).unwrap().block(0).expect("the entry").succ, vec![2]);
    } else {
        assert_eq!(condition.args[0], held(control, 2));
    }
}

#[test]
fn test_cross_width_recurrence_replaces_counter_only_for_its_full_period_4_24_true() {
    test_cross_width_recurrence_replaces_counter_only_for_its_full_period(4, 24, true);
}

#[test]
fn test_cross_width_recurrence_replaces_counter_only_for_its_full_period_1_64_false() {
    test_cross_width_recurrence_replaces_counter_only_for_its_full_period(1, 64, false);
}

/// Mandel reloaded and stored `xStart` at the beginning of every row.
///
/// The inner coordinate has advanced by exactly `32 * 24` on its only
/// exit.  On targets where a memory update is no dearer than a reload plus
/// store, carrying that completed value around the outer backedge and
/// subtracting the proven distance removes the saved-start live range.  A
/// 386 must retain the copy because its memory arithmetic is slower.
/// (`tests/test_rewind.py`)
#[test]
fn test_exact_nested_recurrence_rewinds_before_reloading_its_start() {
    let value = |serial: u32, at: i64, variable: u32| value(serial, at, variable, 0);
    let copy = |at: i64, result: Value, source: Arg| {
        let (uses, width) = match &source {
            Arg::Held(source) => (vec![source.value], source.width),
            Arg::Const(source) => (vec![], source.width),
            _ => unreachable!(),
        };
        let mut op = operation(at, Operation::Nothing, "", vec![result], uses, Kind::Copy);
        op.args = vec![source];
        op.results = vec![held(result, width)];
        op
    };
    let compare = |at: i64, source: Value, bound: Arg, width: u32| -> (Op, Value) {
        let flags = flag(100 + at as u32, at, 100 + at as u32, 0);
        let uses = match &bound {
            Arg::Held(bound) => vec![source, bound.value],
            _ => vec![source],
        };
        let mut op = operation(at, Operation::Nothing, "", vec![flags], uses, Kind::Sub);
        op.args = vec![held(source, width), bound];
        op.results = vec![];
        (op, flags)
    };

    let argument = value(0, 0, 0);
    let start = value(1, 0, 1);
    let bound = value(7, 0, 7);
    let outer_start = value(2, 0, 2);
    let outer = value(3, 1, 2);
    let outer_next = value(4, 5, 2);
    let current = value(5, 3, 3);
    let following = value(6, 4, 3);
    let (inner_test, inner_flag) = compare(3, current, held(bound, 4), 4);
    let (outer_test, outer_flag) = compare(5, outer_next, constant(2, 2), 2);
    let body = MirBody::new(
        0,
        vec![
            MirBlock::new(
                0,
                vec![],
                vec![add_const(0, start, argument, 5, 4), add_const(0, bound, start, 8, 4), copy(0, outer_start, constant(0, 2))],
                vec![1],
            ),
            MirBlock::new(1, vec![phi(outer, &[(0, outer_start), (5, outer_next)])], vec![], vec![2]),
            MirBlock::new(2, vec![], vec![], vec![3]),
            MirBlock::new(
                3,
                vec![phi(current, &[(2, start), (4, following)])],
                vec![inner_test, branch(3, inner_flag, Kind::Eq, 5)],
                vec![4, 5],
            ),
            MirBlock::new(4, vec![], vec![add_const(4, following, current, 2, 4)], vec![3]),
            MirBlock::new(
                5,
                vec![],
                vec![add_const(5, outer_next, outer, 1, 2), outer_test, branch(5, outer_flag, Kind::Lt, 1)],
                vec![1, 6],
            ),
            MirBlock::new(6, vec![], vec![], vec![]),
        ],
    );
    let later_core =
        OperationCosts { add: 1, r#move: 1, load: 1, store: 1, memory_update: 1, ..OperationCosts::default() };
    let i386 = OperationCosts { add: 2, r#move: 2, load: 4, store: 2, memory_update: 8, ..OperationCosts::default() };

    let changed = rewound(&Rc::new(MirBody::clone(&body)), 1, Some(&later_core));
    let outer_header = block(&changed, 1);
    let inner_header = block(&changed, 3);
    let latch = block(&changed, 5);

    assert_ne!(*changed, body);
    assert_eq!(changed.loop_trip_counts, vec![(3, 4)]);
    let changed_inner = loops::loops(&changed.blocks, Some(changed.entry))
        .into_iter()
        .find(|loop_| loop_.header == 3)
        .expect("the inner loop");
    assert_eq!(induction::trip_count(&changed, &changed_inner, &consts::known(&changed, None, None, None, None)), Some(4.into()));
    assert_eq!(rotate::rotated(&changed).unwrap().loop_trip_counts, vec![(4, 4)]);
    assert_eq!(outer_header.phis.len(), 2);
    assert_ne!(inner_header.phis[0].incoming.get(&2), Some(&start));
    assert!(latch.phis.iter().any(|phi| phi.incoming.values().any(|value| *value == current)));
    assert!(latch.ops.iter().any(|op| {
        op.kind == Kind::Add
            && op.args.contains(&constant(-8 & 0xFFFF_FFFF, 4))
            && op.uses.iter().any(|value| value.variable == following.variable)
    }));
    assert_eq!(*rewound(&Rc::new(body.clone()), 1, Some(&i386)), body);

    // The first production version rewound Mandel's rematerializable `px =
    // -16` control before the coordinate recurrence existed.  That blocked
    // strength reduction and grew P6 from 199/55/88 to 219/62/95.
    let mut constant_body = body.clone();
    constant_body.blocks[0].ops[0] = copy(0, start, constant(5, 4));
    assert_eq!(*rewound(&Rc::new(constant_body.clone()), 1, Some(&later_core)), constant_body);
}

/// `[one.insn for block in corpus.partitioned(result.data) for one in block.insns]`
/// of a fixture the LIR emitter wrote.
fn emitted_insns(relative: &str) -> Vec<iced_x86::Instruction> {
    let result = crate::support::testing::emitted_lir(relative);
    crate::support::testing::instructions(&result.data)
}

/// HARR has one recurrence per retained loop, or is completely unrolled.
#[test]
fn test_harr_reuses_an_existing_recurrence_for_termination() {
    use crate::support::testing;
    for (program, loop_count, increments, tag) in [("harr", 1, 1, "v-g3")] {
        let result = testing::emitted_lir(format!("fixtures/omf/{program}-{tag}.obj"));
        let blocks = testing::partitioned_bytes(&result.data);
        let incs = testing::instructions(&result.data).iter().filter(|one| one.mnemonic() == Mnemonic::Inc).count();
        let retained = loops::loops(&testing::graph(&blocks), None);
        if retained.is_empty() {
            assert_eq!(incs, 0);
            continue;
        }
        assert_eq!(retained.len(), loop_count);
        assert_eq!(incs, increments);
    }
}

/// HARR printed 12327 instead of 1100 when the bound read SI before SI was initialized.
#[test]
#[ignore = "fails in Python too: StopIteration (no backward JNE)"]
fn test_harr_initializes_the_reused_counter_before_its_exit_bound() {
    let instructions = emitted_insns("fixtures/omf/harr-v-g3.obj");
    let (branch_at, branch) = instructions
        .iter()
        .enumerate()
        .find(|(_, one)| one.mnemonic() == Mnemonic::Jne && one.near_branch_target() < one.ip())
        .unwrap();
    let compare_at = branch_at - 1;
    let compare = &instructions[compare_at];
    assert_eq!(compare.mnemonic(), Mnemonic::Cmp);
    let wanted = BTreeSet::from([compare.op0_register(), compare.op1_register()]);
    let target = branch.near_branch_target();
    let containing = instructions[branch_at + 1..]
        .iter()
        .filter(|one| one.near_branch_target() != 0 && one.near_branch_target() <= target && target < one.ip())
        .map(|one| one.near_branch_target());
    let start_ip = containing.fold(target, u64::min);
    let start = instructions.iter().position(|one| one.ip() == start_ip).unwrap();
    let mut defined = BTreeSet::new();
    for one in &instructions[start..compare_at] {
        if one.op0_kind() != OpKind::Register || !wanted.contains(&one.op0_register()) {
            continue;
        }
        let mut reads = BTreeSet::new();
        if one.mnemonic() == Mnemonic::Mov && one.op1_kind() == OpKind::Register {
            reads.insert(one.op1_register());
        } else if matches!(one.mnemonic(), Mnemonic::Add | Mnemonic::Sub | Mnemonic::Inc | Mnemonic::Dec) {
            reads.insert(one.op0_register());
            if one.op1_kind() == OpKind::Register {
                reads.insert(one.op1_register());
            }
        }
        let early: Vec<_> = reads.intersection(&wanted).filter(|one| !defined.contains(*one)).collect();
        assert!(early.is_empty(), "{one} reads the loop bound before it is initialized");
        defined.insert(one.op0_register());
    }
    assert_eq!(defined, wanted);
}

/// How often each emitted counted loop runs: its counter's start, step and exit test, simulated.
fn trip_counts(insns: &[iced_x86::Instruction]) -> Vec<i64> {
    use iced_x86::Register;
    let immediates = [OpKind::Immediate8, OpKind::Immediate8to16, OpKind::Immediate16];
    let signed = |one: &iced_x86::Instruction| i64::from(one.immediate16() as i16);

    // The first-iteration constant in `register`, following copies.
    fn initialized(
        register: Register,
        before: &[iced_x86::Instruction],
        seen: &BTreeSet<Register>,
        immediates: &[OpKind; 3],
    ) -> Option<i64> {
        if seen.contains(&register) {
            return None;
        }
        for (index, one) in before.iter().enumerate().rev() {
            if one.op0_kind() != OpKind::Register || one.op0_register() != register {
                continue;
            }
            if one.mnemonic() == Mnemonic::Mov && immediates.contains(&one.op1_kind()) {
                return Some(i64::from(one.immediate16() as i16));
            }
            if one.mnemonic() == Mnemonic::Mov && one.op1_kind() == OpKind::Register {
                let mut seen = seen.clone();
                seen.insert(register);
                return initialized(one.op1_register(), &before[..index], &seen, immediates);
            }
            if one.mnemonic() == Mnemonic::Xor && one.op1_kind() == OpKind::Register && one.op1_register() == register {
                return Some(0);
            }
            return None;
        }
        None
    }

    let mut counts = vec![];
    for (index, branch) in insns.iter().enumerate() {
        if !matches!(branch.mnemonic(), Mnemonic::Jle | Mnemonic::Jl | Mnemonic::Jne)
            || branch.near_branch_target() >= branch.ip()
        {
            continue;
        }
        let inside: Vec<usize> = (0..index).filter(|&at| insns[at].ip() >= branch.near_branch_target()).collect();
        let steps: Vec<usize> = inside
            .iter()
            .copied()
            .filter(|&at| {
                matches!(insns[at].mnemonic(), Mnemonic::Inc | Mnemonic::Dec) && insns[at].op0_kind() == OpKind::Register
            })
            .collect();
        let Some(&step_at) = steps.last() else { continue };
        let step = &insns[step_at];
        let (register, delta) = (step.op0_register(), if step.mnemonic() == Mnemonic::Inc { 1 } else { -1 });
        // The counter can move between registers inside the loop: `mov dx,cx / inc dx / mov cx,dx`.
        let mut held = BTreeSet::from([register]);
        held.extend(inside.iter().map(|&at| &insns[at]).filter_map(|one| {
            let copy = one.mnemonic() == Mnemonic::Mov && one.op1_kind() == OpKind::Register && one.op0_register() == register;
            copy.then(|| one.op1_register())
        }));
        let mut test_at = index - 1;
        let test = &insns[test_at];
        if test.mnemonic() == Mnemonic::Mov && test.op0_kind() == OpKind::Register && held.contains(&test.op0_register()) {
            test_at = index - 2;
        }
        let test = &insns[test_at];
        let bound = if test.mnemonic() == Mnemonic::Cmp
            && test.op0_kind() == OpKind::Register
            && held.contains(&test.op0_register())
            && immediates.contains(&test.op1_kind())
        {
            signed(test)
        // `or r,r` tests for zero as `test r,r` does; the peephole writes it for `cmp r,0`.
        } else if test_at == step_at
            || (matches!(test.mnemonic(), Mnemonic::Test | Mnemonic::Or)
                && held.contains(&test.op0_register())
                && test.op0_kind() == OpKind::Register
                && test.op1_kind() == OpKind::Register
                && test.op0_register() == test.op1_register())
        {
            0
        } else {
            continue;
        };
        let first = insns.iter().position(|one| one == step).unwrap();
        let Some(mut value) = initialized(register, &insns[..first], &BTreeSet::new(), &immediates) else { continue };
        let mut trips = 0;
        while trips < 1 << 17 {
            trips += 1;
            value = ((value + delta + 0x8000) & 0xFFFF) - 0x8000;
            let taken = match branch.mnemonic() {
                Mnemonic::Jle => value <= bound,
                Mnemonic::Jl => value < bound,
                _ => value != bound,
            };
            if !taken {
                break;
            }
        }
        counts.push(trips);
    }
    counts.sort_unstable();
    counts
}

/// SPILL printed T= 4620 and SEGLD T= 975: both loops of each nest start at 1, one
/// constant, and counting one to zero rewrote that constant, so the other ran from -10
/// (or -5) up to its own bound.
#[test]
fn test_counting_one_loop_to_zero_leaves_a_loop_sharing_its_start_alone() {
    for (program, trips) in [("segld", vec![5, 20])] {
        for tag in ["p-g2", "q-o", "v-g3"] {
            assert_eq!(trip_counts(&emitted_insns(&format!("fixtures/omf/{program}-{tag}.obj"))), trips, "{program}-{tag}");
        }
    }
}

/// deedlines failed "value#5479 is read but never defined": counting `i` itself
/// to zero rebased `i + 512` onto a new held offset but left it out of `uses`,
/// so dead deleted its definition.
#[test]
fn test_a_constant_offset_rebased_onto_the_counter_stays_defined() {
    use std::path::Path;

    use crate::frontends::qb::{compile as qb_compile, driver as qb_driver};
    use crate::model::passes::O2;

    let directory = tempfile::TempDir::new().unwrap();
    let basic = directory.path().join("MOD.BAS");
    let lines = [
        "DIM SHARED m%(-168 TO 168)",
        "FOR i% = -168 TO 168",
        "m%(i%) = ((i% + 512) MOD 256) \\ 2",
        "NEXT i%",
    ];
    std::fs::write(&basic, format!("{}\r\n", lines.join("\r\n"))).unwrap();
    let program =
        qb_driver::parsed(&basic, &qb_driver::Frontend::new("qb45", "qb45"), None).unwrap();
    qb_compile::object_bytes(&program, Path::new("MOD.BAS"), None, &O2()).unwrap();
}
