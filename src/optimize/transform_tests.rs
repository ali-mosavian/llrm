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
// ==== END tests B ====

// ==== BEGIN tests C ====
// ==== END tests C ====

// ==== BEGIN tests D ====
// ==== END tests D ====

// ==== BEGIN tests E ====
// ==== END tests E ====

// ==== BEGIN tests F ====
// ==== END tests F ====

