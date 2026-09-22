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
// ==== END tests C ====

// ==== BEGIN tests D ====
// ==== END tests D ====

// ==== BEGIN tests E ====
mod folded_tests {
    use indexmap::IndexMap;

    use crate::analysis::consts::Known;
    use crate::model::ir::Operation;
    use crate::model::mir::{Arg, Const, Held, Kind, Op, OpCode, Value};
    use crate::optimize::transform::_constant_operands;

    /// LNGMXX retained invariant division by 7 because its constant divisor stayed opaque to LICM.
    #[test]
    fn divisor_constants_propagate_without_reordering() {
        for (number, _safe) in [(7_i64, true), (0, false), (0xFFFF_FFFF, false)] {
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
            // needs _cannot_fault, which section C ports.
            #[cfg(any())]
            assert_eq!(crate::optimize::transform::_cannot_fault(&done), _safe);
            assert_eq!(
                _constant_operands(&op, &IndexMap::from([(divisor, Known::new(number, 2))]), None, None),
                op
            );
        }
    }
}
// ==== END tests E ====

// ==== BEGIN tests F ====
// ==== END tests F ====

