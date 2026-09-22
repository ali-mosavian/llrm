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
// ==== END tests D ====

// ==== BEGIN tests E ====
// ==== END tests E ====

// ==== BEGIN tests F ====
// ==== END tests F ====

