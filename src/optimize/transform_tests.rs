//! Port of `tests/test_transform.py`.

mod preheader_tests {
    use std::collections::BTreeSet;

    use crate::analysis::loops::Loop;
    use crate::model::mir::{MirBlock, MirBody};

    use crate::optimize::transform::_preheader as preheader;

    fn block(at: i64, succ: Vec<i64>) -> MirBlock {
        MirBlock::new(at, Vec::new(), Vec::new(), succ)
    }

    fn loop_(header: i64, body: &[i64]) -> Loop {
        Loop {
            header,
            latches: BTreeSet::new(),
            body: body.iter().copied().collect(),
        }
    }

    #[test]
    fn preheader_returns_one_outside_predecessor_with_an_inside_latch() {
        let body = MirBody::new(
            10,
            vec![
                block(10, vec![20]),
                block(20, vec![20]),
                block(30, vec![20]),
            ],
        );

        assert_eq!(preheader(&body, &loop_(20, &[20, 30])), Some(10));
    }

    #[test]
    fn preheader_refuses_zero_or_two_outside_predecessor_occurrences() {
        let no_entry = MirBody::new(20, vec![block(20, vec![20])]);
        assert_eq!(preheader(&no_entry, &loop_(20, &[20])), None);

        let two_entries = MirBody::new(
            10,
            vec![
                block(10, vec![20]),
                block(11, vec![20]),
                block(20, vec![20]),
            ],
        );
        assert_eq!(preheader(&two_entries, &loop_(20, &[20])), None);
    }

    #[test]
    fn preheader_counts_duplicate_outside_block_occurrences() {
        let body = MirBody::new(
            10,
            vec![block(10, vec![20]), block(10, vec![20]), block(20, vec![])],
        );

        assert_eq!(preheader(&body, &loop_(20, &[20])), None);
    }
}
/// Skipped, needing .obj fixtures or wholeseg:
/// test_place_takes_a_store_out_of_a_push_run,
/// test_place_keeps_the_frame_pointer_behind_the_push_that_saves_it,
/// test_both_lngmix_divides_absorb,
/// test_one_idiv_serves_both_of_lngmix_s_divides_in_the_image.
mod subexpressions_tests {
    use std::collections::BTreeSet;

    use indexmap::IndexMap;

    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, FrameAddress, Held, Kind, MirBlock, MirBody, Op, OpCode, Value};
    use crate::optimize::transform::{_computation, _full, subexpressions};

    fn versioned(id: u32, at: i64, variable: u32) -> Value {
        Value { variable, version: 1, ..Value::new(id, at) }
    }

    fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
        Op { kind, ..Op::new(at, OpCode::Operation(operation), name, defines, uses) }
    }

    fn one_block(ops: Vec<Op>) -> MirBody {
        MirBody::new(0, vec![MirBlock::new(0, Vec::new(), ops, Vec::new())])
    }

    #[test]
    fn test_cse_propagates_a_complete_narrow_copy_to_an_opaque_reader() {
        for preserves_high in [false, true] {
            let source = versioned(1, 0, 1);
            let copied = versioned(2, 2, 2);
            let first = Op {
                args: vec![Arg::Const(Const::new(7, 2))],
                results: vec![Arg::Held(Held { value: source, width: 2 })],
                ..op(0, Operation::Move, "mov", vec![source], vec![], Kind::Copy)
            };
            let mut copy = Op {
                args: vec![Arg::Held(Held { value: source, width: 2 })],
                results: vec![Arg::Held(Held { value: copied, width: 2 })],
                ..op(2, Operation::Move, "mov", vec![copied], vec![source], Kind::Copy)
            };
            if preserves_high {
                let previous = versioned(3, 0, 3);
                copy.uses = vec![source, previous];
                copy.merges.insert(previous, copied);
            }
            let reader = op(4, Operation::Push, "push", vec![], vec![copied], Kind::Opaque);
            let body = one_block(vec![first, copy, reader]);

            let done = subexpressions(&body, &BTreeSet::new(), false).unwrap();

            let ops = &done.blocks[0].ops;
            assert_eq!(ops.last().unwrap().uses, vec![if preserves_high { copied } else { source }]);
            assert_eq!(ops.iter().any(|one| one.defines.contains(&copied)), preserves_high);
        }
    }

    #[test]
    fn test_cse_reuses_one_frame_object_address() {
        let first = Value::new(1, 0);
        let duplicate = Value::new(2, 1);
        let address = |at: i64, result: Value| Op {
            args: vec![Arg::FrameAddress(FrameAddress { extent: Some((-132, -4)), ..FrameAddress::new(-132, 2) })],
            results: vec![Arg::Held(Held { value: result, width: 2 })],
            ..op(at, Operation::Address, "lea", vec![result], vec![], Kind::Address)
        };
        let reader = Op {
            args: vec![Arg::Held(Held { value: duplicate, width: 2 })],
            ..op(2, Operation::Push, "push", vec![], vec![duplicate], Kind::Opaque)
        };
        let body = one_block(vec![address(0, first), address(1, duplicate), reader]);

        let done = subexpressions(&body, &BTreeSet::new(), false).unwrap();

        let ops = &done.blocks[0].ops;
        assert_eq!(ops.iter().filter(|one| one.kind == Kind::Address).count(), 1);
        assert_eq!(ops.last().unwrap().uses, vec![first]);
        assert_eq!(ops.last().unwrap().args, vec![Arg::Held(Held { value: first, width: 2 })]);
    }

    #[test]
    fn test_cse_refuses_an_operand_that_is_only_half_its_value() {
        let low = Held { value: versioned(1, 0, 1), width: 2 };
        let whole = |width: u32| IndexMap::from([(1u32, width)]);
        assert!(_full(&low, &whole(2)));
        assert!(!_full(&low, &whole(4)), "half of a long passed as the whole of it");

        let defined = versioned(2, 0, 2);
        let fake = Op {
            kind: Kind::Not,
            args: vec![Arg::Held(low)],
            results: vec![Arg::Held(Held { value: defined, width: 2 })],
            ..Op::new(0, None, "not", vec![defined], vec![])
        };
        assert!(_computation(&fake, &IndexMap::new(), &whole(4)).is_none());
        assert!(_computation(&fake, &IndexMap::new(), &whole(2)).is_some());
    }
}
// Skipped: test_a_served_read_names_the_value_and_not_a_register (corpus fixture).
mod b_tests {
    use std::collections::BTreeSet;

    use indexmap::IndexMap;

    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
    use crate::objectfile::module::{Addr, Space};
    use crate::optimize::transform::{forwarded, without_dead_stores};

    fn cell(disp: i64, width: u32) -> MemRef {
        MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, disp) }), width)
    }

    fn op(at: i64, code: Operation, name: &str, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
        let mut op = Op::new(at, OpCode::Operation(code), name, defines, uses);
        op.kind = kind;
        op
    }

    fn copy(at: i64, into: Value, n: i64, width: u32) -> Op {
        let mut one = op(at, Operation::Move, "mov", vec![into], vec![], Kind::Copy);
        one.args = vec![Arg::Const(Const::new(n, width))];
        one.results = vec![Arg::Held(Held { value: into, width })];
        one
    }

    fn store(at: i64, from: Value, target: &MemRef) -> Op {
        let mut one = op(at, Operation::Move, "mov", vec![], vec![from], Kind::Store);
        one.args = vec![Arg::Held(Held { value: from, width: target.width })];
        one.results = vec![Arg::Cell(Cell { r#ref: target.clone() })];
        one.stores = vec![target.clone()];
        one
    }

    fn load(at: i64, into: Value, source: &MemRef) -> Op {
        let mut one = op(at, Operation::Move, "mov", vec![into], vec![], Kind::Load);
        one.args = vec![Arg::Cell(Cell { r#ref: source.clone() })];
        one.results = vec![Arg::Held(Held { value: into, width: source.width })];
        one.loads = vec![source.clone()];
        one
    }

    /// nbody printed PX0=-7627 instead of 1258 after losing its counter load.
    #[test]
    fn test_dead_store_does_not_delete_a_load_at_the_same_address() {
        let (source, loaded) = (Value::new(1, 0), Value::new(2, 3));
        let target = cell(0, 2);
        let counter = cell(2, 2);
        let first = copy(0, source, 7, 2);
        let written = store(3, source, &target);
        let read = load(3, loaded, &counter);
        let overwrite = Op { at: 6, ..written.clone() };
        let mut used = op(9, Operation::Push, "push", vec![], vec![loaded], Kind::Arg);
        used.args = vec![Arg::Held(Held { value: loaded, width: 2 })];
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![first, written, read, overwrite, used], vec![])]);
        let done = without_dead_stores(&body, &BTreeSet::from([5]), &IndexMap::new(), None, None, true).unwrap();
        let ops = &done.blocks[0].ops;
        assert!(ops.iter().any(|op| op.defines.contains(&loaded)));
        assert_eq!(ops.iter().filter(|op| !op.stores.is_empty()).count(), 1);
    }

    /// Nbody kept statement reloads because their providers were not already live.
    #[test]
    fn test_forwarding_extends_lifetime_without_conflating_shared_addresses() {
        let [source, loaded, unrelated] = [1, 2, 3].map(|index| Value::new(index, index as i64));
        let target = cell(0, 4);
        let other = cell(8, 4);
        let first = copy(0, source, 7, 4);
        let written = store(1, source, &target);
        let read = load(2, loaded, &target);
        let neighbor = load(2, unrelated, &other);
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![first, written, read, neighbor.clone()], vec![])]);
        let done = forwarded(&body, &BTreeSet::from([5]), &IndexMap::new(), false).unwrap();
        let done = &done.blocks[0].ops;
        assert_eq!(done[2].args, vec![Arg::Held(Held { value: source, width: 4 })]);
        assert!(done[2].loads.is_empty());
        assert_eq!(done[3], neighbor);
    }
}
mod hoist_tests {
    use std::collections::BTreeSet;

    use indexmap::IndexMap;

    use crate::model::ir::{Operation, Semantics};
    use crate::model::memory::{Identity, MemoryKind, MemoryObject, Provenance};
    use crate::model::mir::{
        kind_of, Arg, Cell, Const, FrameAddress, Held, IntegerRange, Kind, MemRef, MirBlock, MirBody, Op, OpCode,
        OrderedMap, Phi, Value,
    };
    use crate::objectfile::module::{Addr, Space};
    use crate::optimize::transform::{_crossing, _invariant_run, _reparented, _rewritten, _starts};
    use crate::support::pyset::PySet;

    fn flag(id: u32, at: i64) -> Value {
        Value { flags: true, ..Value::new(id, at) }
    }

    fn run<'a>(ops: &[&'a Op], stores: &[(&MemRef, Option<&std::collections::BTreeMap<Value, crate::analysis::ranges::Interval>>)], phis: &[&Phi], starts: Option<&BTreeSet<Value>>) -> Vec<&'a Op> {
        _invariant_run(
            ops,
            &BTreeSet::new(),
            stores,
            &BTreeSet::new(),
            &IndexMap::new(),
            phis,
            None,
            starts,
            None,
            None,
            false,
            &BTreeSet::new(),
        )
        .unwrap()
    }

    #[test]
    fn test_a_run_whose_flag_the_loop_still_reads_is_not_hoistable() {
        let op = |at: i64, name: &str, defines: Vec<Value>, uses: Vec<Value>| {
            let mut out = Op::new(at, OpCode::Operation(Operation::Compare), name, defines, uses);
            out.kind = Kind::Sub;
            out
        };
        let flag = flag(1, 0x10);
        let got = Value::new(2, 0x10);
        let compare = op(0x10, "cmp", vec![flag, got], vec![]);
        let branch = op(0x14, "jle", vec![], vec![flag]);
        let reader = op(0x18, "add", vec![], vec![got]);

        assert_eq!(
            _crossing(&[&compare], &[&reader], None, None),
            Some(BTreeSet::from([got])),
            "a plain value crosses in a register"
        );
        assert_eq!(_crossing(&[&compare], &[&branch, &reader], None, None), None, "a flag the loop reads does not");
        assert_eq!(_crossing(&[&compare], &[&branch], None, None), None);
    }

    #[test]
    fn test_an_operand_nothing_writes_down_may_leave_with_its_run() {
        let op = |at: i64, name: &str, what: Operation, defines: Vec<Value>, uses: Vec<Value>| {
            let mut semantics = Semantics::new(what);
            semantics.name = Some(name.to_owned());
            let mut out = Op::new(at, OpCode::Operation(what), name, defines, uses);
            out.loads = vec![MemRef::new(None, 2)];
            out.kind = kind_of(&semantics, &[], &[]);
            out
        };
        let load = op(0x10, "mov", Operation::Move, vec![Value::new(1, 0x10)], vec![]);
        // dx:ax = ax * [k], and ax is written down nowhere.
        let widening = op(0x13, "imul", Operation::Multiply, vec![Value::new(2, 0x13)], vec![Value::new(1, 0x10)]);

        let found = run(&[&load, &widening], &[], &[], None);
        assert!(found.contains(&&load), "an ordinary load is invariant here");
        assert!(found.contains(&&widening), "and the multiply behind it leaves with it");
    }

    #[test]
    fn test_a_precise_volatile_access_does_not_block_disjoint_invariant_work() {
        let mut argument = MemRef::new(Some(Addr::new(Space::Frame, 6)), 2);
        argument.space = Some(Space::Frame);
        let mut local = MemRef::new(Some(Addr::new(Space::Frame, -8)), 8);
        local.space = Some(Space::Frame);
        local.volatile = true;
        let value = Value::new(1, 0x10);
        let mut load = Op::new(0x10, OpCode::Operation(Operation::Move), "mov", vec![value], vec![]);
        load.kind = Kind::Load;
        load.args = vec![Arg::Cell(Cell { r#ref: argument.clone() })];
        load.results = vec![Arg::Held(Held { value, width: 2 })];
        load.loads = vec![argument];
        let mut observable = Op::new(0x12, OpCode::Operation(Operation::Move), "fstp", vec![], vec![]);
        observable.kind = Kind::Fstore;
        observable.args = vec![Arg::Const(Const::new(0, 8))];
        observable.results = vec![Arg::Cell(Cell { r#ref: local.clone() })];
        observable.stores = vec![local.clone()];
        observable.volatile = true;
        observable.memory_complete = true;
        observable.reads_complete = true;
        let stores = [(&local, None)];

        assert_eq!(run(&[&load, &observable], &stores, &[], None), vec![&load]);
        let mut opaque = observable.clone();
        opaque.op = Some(OpCode::Operation(Operation::Barrier));
        opaque.volatile = false;
        assert!(run(&[&load, &opaque], &stores, &[], None).is_empty());
    }

    #[test]
    fn test_a_definition_a_phi_carries_and_the_loop_rewrites_does_not_leave_it() {
        let start = Value::new(1, 0x10);
        let again = Value::new(2, 0x14);
        let merged = Value::new(3, 0x14);

        let mut begins = Op::new(0x10, OpCode::Operation(Operation::Move), "mov", vec![start], vec![]);
        begins.kind = Kind::Copy;
        let mut counts = Op::new(0x14, OpCode::Operation(Operation::Unary), "inc", vec![again], vec![merged]);
        counts.kind = Kind::Add;
        let carried = [Phi {
            result: merged,
            incoming: OrderedMap::from_iter([(0x00, start), (0x14, again)]),
        }];
        let phis = carried.iter().collect::<Vec<_>>();

        assert_eq!(_starts(&phis), BTreeSet::from([start, again]), "a phi carries both");
        assert!(_rewritten(&[&begins, &counts], &phis).contains(&start), "and the counter is written twice");

        let both = run(&[&begins, &counts], &[], &phis, Some(&_starts(&phis)));
        assert!(!both.contains(&&begins), "so what starts the counter stays in the loop");
        assert!(!run(&[&begins, &counts], &[], &phis, Some(&BTreeSet::new())).contains(&&begins));
    }

    #[test]
    fn test_reparenting_a_hoisted_pointer_keeps_its_object_facts() {
        let pointer = Value { variable: 3, version: 1, ..Value::new(1, 0) };
        let object = MemoryObject {
            kind: MemoryKind::Frame,
            identity: Some(Identity::Tuple(vec![Identity::Int(5), Identity::Int(-16), Identity::Int(-4)])),
            generation: 0,
            extent: Some(12),
        };
        let provenance = Provenance::one_with_slice(object, 0, 1, 1, 1, BTreeSet::new()).unwrap();
        let interval = IntegerRange::new(0, 31, 2);
        let mut address = Op::new(0, OpCode::Operation(Operation::Address), "lea", vec![pointer], vec![]);
        address.kind = Kind::Address;
        address.args = vec![Arg::FrameAddress(FrameAddress { offset: -16, width: 2, extent: Some((-16, -4)) })];
        address.results = vec![Arg::Held(Held { value: pointer, width: 2 })];
        let mut body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![address], vec![])]);
        body.pointer_values = BTreeSet::from([pointer]);
        body.pointer_seeds = OrderedMap::from_iter([(pointer, provenance.clone())]);
        body.integer_ranges = OrderedMap::from_iter([(pointer, interval.clone())]);

        let result = _reparented(&body, &PySet::from_iter([pointer]));

        let renamed = result.blocks[0].ops[0].defines[0];
        assert_ne!(renamed.variable, pointer.variable);
        assert_eq!(result.pointer_values, BTreeSet::from([renamed]));
        assert_eq!(result.pointer_seeds, OrderedMap::from_iter([(renamed, provenance)]));
        assert_eq!(result.integer_ranges, OrderedMap::from_iter([(renamed, interval)]));
    }
}
mod d_tests {
    use std::collections::BTreeSet;

    use crate::model::ir::Operation;
    use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode, Phi, Value};
    use crate::optimize::transform::{_removable, _threaded, dead};

    fn op(at: i64, operation: Operation, name: &str, defines: Vec<Value>, kind: Kind) -> Op {
        let mut made = Op::new(at, OpCode::Operation(operation), name, defines, vec![]);
        made.kind = kind;
        made
    }

    fn jump(at: i64, target: i64) -> Op {
        let mut made = op(at, Operation::Jump, "jmp", vec![], Kind::Jump);
        made.target = Some(target);
        made
    }

    /// Collapsed FPCSE's trampoline is removable; a phi edge or store is not.
    #[test]
    fn test_empty_jump_threading_preserves_phi_inputs_and_effects() {
        for guard in ["none", "phi", "store", "cycle"] {
            let entry = MirBlock::new(0, vec![], vec![jump(0, 1)], vec![1]);
            let mut middle = MirBlock::new(1, vec![], vec![jump(1, 2)], vec![2]);
            let mut end = MirBlock::new(2, vec![], vec![], vec![]);
            match guard {
                "phi" => {
                    end.phis =
                        vec![Phi { result: Value::new(1, 2), incoming: [(1, Value::new(2, 1))].into_iter().collect() }];
                }
                "store" => middle.ops.insert(0, op(1, Operation::Move, "mov", vec![], Kind::Store)),
                "cycle" => {
                    middle.ops = vec![jump(1, 1)];
                    middle.succ = vec![1];
                }
                _ => {}
            }
            let body = MirBody::new(0, vec![entry, middle, end]);
            let result = _threaded(&body).expect("threads");
            if guard == "none" {
                assert_eq!(result.blocks[0].succ, vec![2]);
                assert_eq!(result.blocks[0].ops.last().unwrap().target, Some(2));
                assert_eq!(result.blocks[1].ops.last().unwrap().kind, Kind::Nothing);
                assert_eq!(result.blocks[1].ops.last().unwrap().name, "");
            } else {
                assert_eq!(result, body, "{guard}");
            }
        }
    }

    /// A move nothing reads, removed, without losing what it stood for.
    #[test]
    fn test_dead_code_goes_and_the_bytes_are_still_accounted_for() {
        let mut live_one = op(0x10, Operation::Move, "mov", vec![Value::new(1, 0x10)], Kind::Copy);
        live_one.absorbed = vec![1];
        let mut doomed = op(0x13, Operation::Move, "mov", vec![Value::new(2, 0x13)], Kind::Copy);
        doomed.absorbed = vec![2];
        assert!(_removable(&doomed, &BTreeSet::new()), "nothing reads it");
        assert!(!_removable(&live_one, &BTreeSet::from([Value::new(1, 0x10)])), "and this is read");
    }

    /// An opaque instruction reads registers no semantics mention.
    #[test]
    fn test_dead_code_leaves_a_body_it_cannot_read_alone() {
        let mut first = op(0x10, Operation::Move, "mov", vec![Value::new(1, 0x10)], Kind::Copy);
        first.absorbed = vec![1];
        let mut doomed = op(0x12, Operation::Move, "mov", vec![Value::new(2, 0x12)], Kind::Copy);
        doomed.absorbed = vec![2];
        let plain = MirBody::new(0x10, vec![MirBlock::new(0x10, vec![], vec![first.clone(), doomed.clone()], vec![])]);
        assert_ne!(dead(&plain).expect("dead"), plain, "a dead move goes when the body is readable");

        let mut opaque = Op::new(0x14, OpCode::Operation(Operation::Barrier), "?", vec![], vec![]);
        opaque.absorbed = vec![3];
        let body = MirBody::new(0x10, vec![MirBlock::new(0x10, vec![], vec![first, doomed, opaque], vec![])]);
        assert_eq!(dead(&body).expect("dead"), body, "and stays when the body holds a barrier");
    }
}
mod folded_tests {
    use indexmap::IndexMap;

    use crate::analysis::consts::Known;
    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, Held, Kind, Op, OpCode, Value};
    use crate::optimize::transform::_constant_operands;

    /// LNGMXX retained invariant division by 7 because its constant divisor stayed opaque to LICM.
    #[test]
    fn divisor_constants_propagate_without_reordering() {
        for (number, safe) in [(7_i64, true), (0, false), (0xFFFF_FFFF, false)] {
            let [dividend, divisor, quotient, remainder] = [1, 2, 3, 4].map(|index| Value::new(index, 0));
            let mut op = Op::new(
                0,
                OpCode::Operation(Operation::Divide),
                "idiv",
                vec![quotient, remainder],
                vec![dividend, divisor],
            );
            op.kind = Kind::Divmod;
            op.args = vec![
                Arg::Held(Held { value: dividend, width: 4 }),
                Arg::Held(Held { value: divisor, width: 4 }),
            ];
            op.results = vec![
                Arg::Held(Held { value: quotient, width: 4 }),
                Arg::Held(Held { value: remainder, width: 4 }),
            ];
            let done = _constant_operands(&op, &IndexMap::from([(divisor, Known::new(number, 4))]), None, None);
            assert_eq!(
                done.args,
                vec![Arg::Held(Held { value: dividend, width: 4 }), Arg::Const(Const::new(number, 4))]
            );
            assert_eq!(done.uses, vec![dividend]);
            assert_eq!(crate::optimize::transform::_cannot_fault(&done), safe);
            assert_eq!(
                _constant_operands(&op, &IndexMap::from([(divisor, Known::new(number, 2))]), None, None),
                op
            );
        }
    }
}
mod pipeline_tests {
    use std::collections::BTreeSet;

    use indexmap::IndexMap;
    use num_bigint::BigInt;

    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, Kind, MirBlock, MirBody, Op, OpCode};
    use crate::optimize::transform::{_absorb, applied, Applied, PASSES};

    fn move_(at: i64, kind: Kind, args: Vec<Arg>) -> Op {
        let mut op = Op::new(at, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
        op.kind = kind;
        op.args = args;
        op
    }

    #[test]
    fn test_final_pipeline_inerts_unreachable_executable_blocks() {
        let dead_store = move_(9, Kind::Store, vec![]);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![], vec![]), MirBlock::new(0x3100_0000_2B, vec![], vec![dead_store], vec![])],
        );

        let result = applied(
            &body,
            &BTreeSet::new(),
            &IndexMap::new(),
            Applied { only: Some("no-such-pass".to_owned()), ..Default::default() },
        )
        .unwrap();

        let orphan = result.block(0x3100_0000_2B).expect("the orphan stays");
        assert!(orphan.succ.is_empty());
        assert!(orphan.ops.iter().all(|op| op.kind == Kind::Nothing && op.stores.is_empty()));
    }

    #[test]
    fn test_final_pipeline_drops_unreachable_empty_blocks() {
        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![], vec![], vec![]), MirBlock::new(0x3100_0000_2B, vec![], vec![], vec![])],
        );

        let result = applied(
            &body,
            &BTreeSet::new(),
            &IndexMap::new(),
            Applied { only: Some("no-such-pass".to_owned()), ..Default::default() },
        )
        .unwrap();

        assert!(result.block(0x3100_0000_2B).is_none());
    }

    #[test]
    fn test_leading_deletion_does_not_delete_its_survivor() {
        let constant = |n: i64| vec![Arg::Const(Const::new(BigInt::from(n), 2))];
        let first = move_(0, Kind::Copy, constant(3));
        let survivor = move_(3, Kind::Copy, constant(21));
        let last = move_(6, Kind::Copy, constant(5));
        let done = _absorb(&[first, survivor.clone(), last.clone()], &BTreeSet::from([0]));
        assert_eq!(done.iter().map(|op| op.args.clone()).collect::<Vec<_>>(), vec![survivor.args, last.args]);
    }

    #[test]
    fn test_long_pair_recognition_is_not_an_optimizer_pass() {
        let defaults = Applied::default();
        assert!(defaults.drop_loads);
        assert!(defaults.drop_stores);

        assert!(!PASSES.iter().any(|one| one == "widen"));
        assert!(PASSES.iter().any(|one| one == "drop_stores"));
    }

    #[test]
    fn test_value_reuse_is_one_gvn_pre_pass() {
        assert!(PASSES.iter().any(|one| one == "gvn"));
        assert!(!PASSES.iter().any(|one| ["forward", "drop_loads", "reuse", "cse"].contains(&one.as_str())));
    }

    #[test]
    fn test_scalar_replacement_precedes_scalar_and_cfg_simplification() {
        let index = |name: &str| PASSES.iter().position(|one| one == name).expect("a pipeline pass");
        assert!(index("sroa") < ["fold", "decide", "loopsimplify"].iter().map(|name| index(name)).min().unwrap());
    }

    #[test]
    fn test_the_rename_alone_is_what_was_unsound() {
        use crate::model::ir::root;
        use iced_x86::Register;

        assert_eq!(root(Register::AX), Register::EAX);
        assert_eq!(root(Register::DX), Register::EDX);
        assert_ne!(root(Register::AX), root(Register::DX));
    }
}

