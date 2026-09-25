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
mod subexpressions_tests {
    use std::collections::BTreeSet;

    use crate::support::hash::IndexMap;

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

            let done = subexpressions(&std::rc::Rc::new(MirBody::clone(&body)), &BTreeSet::new(), false).unwrap();

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

        let done = subexpressions(&std::rc::Rc::new(MirBody::clone(&body)), &BTreeSet::new(), false).unwrap();

        let ops = &done.blocks[0].ops;
        assert_eq!(ops.iter().filter(|one| one.kind == Kind::Address).count(), 1);
        assert_eq!(ops.last().unwrap().uses, vec![first]);
        assert_eq!(ops.last().unwrap().args, vec![Arg::Held(Held { value: first, width: 2 })]);
    }

    #[test]
    fn test_cse_refuses_an_operand_that_is_only_half_its_value() {
        let low = Held { value: versioned(1, 0, 1), width: 2 };
        let whole = |width: u32| IndexMap::from_iter([(1u32, width)]);
        assert!(_full(&low, &whole(2)));
        assert!(!_full(&low, &whole(4)), "half of a long passed as the whole of it");

        let defined = versioned(2, 0, 2);
        let fake = Op {
            kind: Kind::Not,
            args: vec![Arg::Held(low)],
            results: vec![Arg::Held(Held { value: defined, width: 2 })],
            ..Op::new(0, None, "not", vec![defined], vec![])
        };
        assert!(_computation(&fake, &IndexMap::default(), &whole(4)).is_none());
        assert!(_computation(&fake, &IndexMap::default(), &whole(2)).is_some());
    }
}
mod b_tests {
    use std::collections::BTreeSet;

    use crate::support::hash::IndexMap;

    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, Value};
    use crate::objectfile::module::{Addr, Space};
    use crate::optimize::transform::{forwarded, placed, without_dead_stores};

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
        let done = without_dead_stores(&std::rc::Rc::new(MirBody::clone(&body)), &BTreeSet::from([5]), &IndexMap::default(), None, None, true).unwrap();
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
        let done = forwarded(&std::rc::Rc::new(MirBody::clone(&body)), &BTreeSet::from([5]), &IndexMap::default(), false).unwrap();
        let done = &done.blocks[0].ops;
        assert_eq!(done[2].args, vec![Arg::Held(Held { value: source, width: 4 })]);
        assert!(done[2].loads.is_empty());
        assert_eq!(done[3], neighbor);
    }

    /// placed copied a body it left alone, so the proof caches keyed on it missed.
    #[test]
    fn test_placed_returns_an_unchanged_body_as_itself() {
        let source = Value::new(1, 0);
        let body = std::rc::Rc::new(MirBody::new(0, vec![MirBlock::new(0, vec![], vec![copy(0, source, 7, 2)], vec![])]));
        let done = placed(&body, &BTreeSet::from([5]), &IndexMap::default()).unwrap();
        assert!(std::rc::Rc::ptr_eq(&done, &body));
    }
}
mod hoist_tests {
    use std::collections::BTreeSet;

    use crate::support::hash::IndexMap;

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
            &IndexMap::default(),
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
            addressed: true,
            captured: true,
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
            let result = _threaded(&std::rc::Rc::new(MirBody::clone(&body))).expect("threads");
            if guard == "none" {
                assert_eq!(result.blocks[0].succ, vec![2]);
                assert_eq!(result.blocks[0].ops.last().unwrap().target, Some(2));
                assert_eq!(result.blocks[1].ops.last().unwrap().kind, Kind::Nothing);
                assert_eq!(result.blocks[1].ops.last().unwrap().name, "");
            } else {
                assert_eq!(result, std::rc::Rc::new(body), "{guard}");
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
        assert_ne!(*dead(&std::rc::Rc::new(plain.clone())).expect("dead"), plain, "a dead move goes when the body is readable");

        let mut opaque = Op::new(0x14, OpCode::Operation(Operation::Barrier), "?", vec![], vec![]);
        opaque.absorbed = vec![3];
        let body = MirBody::new(0x10, vec![MirBlock::new(0x10, vec![], vec![first, doomed, opaque], vec![])]);
        assert_eq!(*dead(&std::rc::Rc::new(body.clone())).expect("dead"), body, "and stays when the body holds a barrier");
    }
}
mod folded_tests {
    use crate::support::hash::IndexMap;

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
            let done = _constant_operands(&op, &IndexMap::from_iter([(divisor, Known::new(number, 4))]), None, None);
            assert_eq!(
                done.args,
                vec![Arg::Held(Held { value: dividend, width: 4 }), Arg::Const(Const::new(number, 4))]
            );
            assert_eq!(done.uses, vec![dividend]);
            assert_eq!(crate::optimize::transform::_cannot_fault(&done), safe);
            assert_eq!(
                _constant_operands(&op, &IndexMap::from_iter([(divisor, Known::new(number, 2))]), None, None),
                op
            );
        }
    }
}
mod reused_divides_tests {
    use std::collections::BTreeSet;
    use std::rc::Rc;

    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Held, Kind, MirBlock, MirBody, Op, OpCode, Value};
    use crate::optimize::transform::reused_divides;

    fn held(value: Value) -> Arg {
        Arg::Held(Held { value, width: 4 })
    }

    /// lngmix under SROA refused 0x0071: "mov ... defines [v24_1] through no operand".
    ///
    /// The second divide became a copy of the first's remainder, and the third
    /// was served the second's quotient, which that copy no longer computes.
    #[test]
    fn test_a_third_equal_divide_reads_the_answer_the_first_computed() {
        let (dividend, divisor) = (Value::new(1, 0), Value::new(2, 0));
        let divide = |at: i64| {
            let (quotient, remainder) = (Value::new(at as u32 + 3, at), Value::new(at as u32 + 4, at));
            let mut op =
                Op::new(at, OpCode::Operation(Operation::Divide), "idiv", vec![quotient, remainder], vec![dividend, divisor]);
            op.kind = Kind::Divmod;
            op.args = vec![held(dividend), held(divisor)];
            op.results = vec![held(quotient), held(remainder)];
            op
        };
        let (first, second, third) = (divide(0), divide(8), divide(16));
        let (remainder, quotient, total) = (Value::new(12, 8), Value::new(19, 16), Value::new(40, 24));
        let mut add = Op::new(24, OpCode::Operation(Operation::Binary), "add", vec![total], vec![remainder, quotient]);
        add.kind = Kind::Add;
        add.args = vec![held(remainder), held(quotient)];
        add.results = vec![held(total)];
        let mut returned = Op::new(28, OpCode::Operation(Operation::Return), "ret", vec![], vec![total]);
        returned.kind = Kind::Return;
        returned.args = vec![held(total)];
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![first, second, third, add, returned], vec![])]);

        let done = reused_divides(&Rc::new(body), &BTreeSet::new(), None).unwrap();
        let ops = &done.blocks[0].ops;
        assert_eq!(ops[..3].iter().map(|op| op.kind).collect::<Vec<_>>(), [Kind::Divmod, Kind::Copy, Kind::Copy]);

        let mut computed: BTreeSet<Value> = ops
            .iter()
            .flat_map(|op| &op.results)
            .filter_map(|one| if let Arg::Held(held) = one { Some(held.value) } else { None })
            .collect();
        computed.extend([dividend, divisor]);
        for op in ops {
            for value in &op.uses {
                assert!(computed.contains(value), "{value:?} is read and nothing computes it");
            }
        }
    }
}
mod pipeline_tests {
    use std::collections::BTreeSet;

    use crate::support::hash::IndexMap;
    use num_bigint::BigInt;

    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, Kind, MirBlock, MirBody, Op, OpCode};
    use crate::optimize::transform::{_without, applied, Applied, PASSES};

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
            &std::rc::Rc::new(MirBody::clone(&body)),
            &BTreeSet::new(),
            &IndexMap::default(),
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
            &std::rc::Rc::new(MirBody::clone(&body)),
            &BTreeSet::new(),
            &IndexMap::default(),
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
        let done = _without(&[first, survivor.clone(), last.clone()], |op| op.at == 0);
        assert_eq!(done.iter().map(|op| op.args.clone()).collect::<Vec<_>>(), vec![survivor.args, last.args]);
    }

    #[test]
    fn test_long_pair_recognition_is_not_an_optimizer_pass() {
        let defaults = Applied::default();
        assert!(defaults.options.drop_loads);
        assert!(defaults.options.drop_stores);

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

/// tests/test_transform.py's fixture tests.
///
/// Skipped, monkeypatching a pass:
/// test_one_idiv_serves_both_of_lngmix_s_divides_in_the_image.
mod corpus_tests {
    use crate::model::mir::{Arg, Kind};
    use crate::optimize::transform::{forwarded, placed};
    use crate::support::testing;

    /// `add ax,[y]` served from a register used to say which register.
    #[test]
    fn test_a_served_read_names_the_value_and_not_a_register() {
        let mut objects: Vec<_> = std::fs::read_dir(testing::path(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf")))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.file_name().unwrap().to_str().unwrap().ends_with("-p-g2.obj"))
            .collect();
        objects.sort();
        let mut seen = 0;
        for obj in objects {
            let found = testing::loaded(&obj).unwrap();
            let Ok(mapped) = crate::frontends::bc::blocks::code_map(&found) else { continue };
            let blocks = crate::frontends::bc::blocks::partition(&found, &mapped);
            for (_, body) in testing::raised_from(&found, &blocks, None).values {
                let after = forwarded(&body, &found.dgroup.members, &found.calls, false).unwrap();
                if std::rc::Rc::ptr_eq(&after, &body) {
                    continue;
                }
                for op in testing::ops(&after) {
                    let Some(raised) = &op.raised else { continue };
                    if (&op.args, &op.results) == (&raised.0, &raised.1) || !op.loads.is_empty() {
                        continue;
                    }
                    if !raised.0.iter().any(|one| matches!(one, Arg::Cell(_))) {
                        continue;
                    }
                    let held = op.args.iter().any(|one| matches!(one, Arg::Held(_)));
                    assert!(held, "{obj:?} {:#06x}: served read names a register", op.at);
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "nothing was served, so this proves nothing");
    }

    /// lngmix's second divide never folded: two stores stood in its run.
    #[test]
    fn test_place_takes_a_store_out_of_a_push_run() {
        let found = testing::module(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/lngmix-p-g2.obj"));
        let raised = testing::raised_from(&found, &testing::blocks_of(&found), None);
        let [(_, body)] = <[_; 1]>::try_from(raised.values).unwrap();
        let done = placed(&body, &found.dgroup.members, &found.calls).unwrap();
        let run = done.blocks.iter().find(|block| block.ops.iter().any(|op| op.kind == Kind::Call)).unwrap();
        let kinds: Vec<Kind> = run.ops.iter().map(|op| op.kind).collect();
        let call = kinds.iter().position(|&kind| kind == Kind::Call).unwrap();
        let first = kinds.iter().position(|&kind| kind == Kind::Arg).unwrap();
        let stray: Vec<&Kind> = kinds[first..call].iter().filter(|&&kind| kind != Kind::Arg).collect();
        assert!(stray.is_empty(), "a call's run still holds {stray:?}");
    }

    /// procs p-ot's REPORT moved `mov bp,sp` ahead of `push bp`, so every
    /// argument it read through bp was one word off.
    #[test]
    fn test_place_keeps_the_frame_pointer_behind_the_push_that_saves_it() {
        let found = testing::module(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/procs-p-ot.obj"));
        let raised = testing::raised_from(&found, &testing::blocks_of(&found), None);
        let body = raised.values.iter().find(|(name, _)| name.contains("REPORT")).unwrap().1.clone();
        let done = placed(&body, &found.dgroup.members, &found.calls).unwrap();
        assert_eq!(done.blocks[0].ops[..2].iter().map(|op| op.at).collect::<Vec<_>>(), [0x142, 0x143]);
    }

    /// 952 -> 930 bytes, no runtime divide: the second call folds around the store.
    #[test]
    fn test_both_lngmix_divides_absorb() {
        let mut data = testing::data(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/lngmix-p-g2.obj"));
        for _ in 0..3 {
            data = crate::wholeseg::rebuilt(&data, true, true, None, false, false).unwrap().0;
        }
        let found = testing::loaded_bytes(&data).unwrap();
        let blocks = testing::partitioned_bytes(&data);
        let reached: Vec<_> = blocks.iter().flat_map(|block| block.insns.iter().cloned()).collect();
        assert_eq!(crate::legacy::calls::sites(&found, &reached, &blocks), vec![]);
    }
}


