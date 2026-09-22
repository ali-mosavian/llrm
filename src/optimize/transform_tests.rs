//! Port of `tests/test_transform.py`.

// ==== BEGIN preheader tests (primary) ====
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
// ==== END preheader tests ====

// ==== BEGIN tests A ====
// ==== END tests A ====

// ==== BEGIN tests B ====
// ==== END tests B ====

// ==== BEGIN tests C ====
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
// ==== END tests C ====

// ==== BEGIN tests D ====
// ==== END tests D ====

// ==== BEGIN tests E ====
// ==== END tests E ====

// ==== BEGIN tests F ====
// ==== END tests F ====

