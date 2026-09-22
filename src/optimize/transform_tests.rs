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
// ==== END tests A ====

// ==== BEGIN tests B ====
// ==== END tests B ====

// ==== BEGIN tests C ====
// ==== END tests C ====

// ==== BEGIN tests D ====
// ==== END tests D ====

// ==== BEGIN tests E ====
// ==== END tests E ====

// ==== BEGIN tests F ====
// ==== END tests F ====

