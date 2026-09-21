//! Snapshot-local MIR occurrence keys.
//!
//! Python analyses use `id(op)` and object identity to distinguish two
//! structurally equal operations.  Rust MIR operations are values, so an
//! immutable body view supplies the equivalent identity: its block ordinal
//! and operation ordinal.  These keys are valid only for the exact
//! [`MirBody`] they were enumerated from.  A rewritten or reconstructed body
//! must be enumerated again; keys deliberately do not become persistent MIR
//! metadata.

use crate::model::mir::{MirBlock, MirBody, Op, Phi};

/// One operation occurrence in an immutable MIR body snapshot.
///
/// This is the direct Rust representation of Python's `id(op)` for analysis
/// results.  It is not `Op.id`, which names source provenance instead.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct OpOccurrence {
    block_index: usize,
    operation_index: usize,
}

/// One phi occurrence in an immutable MIR body snapshot.
///
/// Python's `control_replacement` also compares its proven phi by object
/// identity.  Keep that distinction separate from an operation occurrence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PhiOccurrence {
    block_index: usize,
    phi_index: usize,
}

/// Enumerate every operation in Python body/block/operation order.
pub(crate) fn operations(body: &MirBody) -> impl Iterator<Item = (OpOccurrence, &MirBlock, &Op)> {
    body.blocks
        .iter()
        .enumerate()
        .flat_map(|(block_index, block)| {
            block
                .ops
                .iter()
                .enumerate()
                .map(move |(operation_index, operation)| {
                    (
                        OpOccurrence {
                            block_index,
                            operation_index,
                        },
                        block,
                        operation,
                    )
                })
        })
}

/// Enumerate every phi in Python body/block/phi order.
pub(crate) fn phis(body: &MirBody) -> impl Iterator<Item = (PhiOccurrence, &MirBlock, &Phi)> {
    body.blocks
        .iter()
        .enumerate()
        .flat_map(|(block_index, block)| {
            block.phis.iter().enumerate().map(move |(phi_index, phi)| {
                (
                    PhiOccurrence {
                        block_index,
                        phi_index,
                    },
                    block,
                    phi,
                )
            })
        })
}

#[cfg(test)]
mod tests {
    use crate::model::mir::{MirBlock, MirBody, Phi, Value};

    use super::{operations, phis};

    #[test]
    fn direct_induction_transparent_aliases_distinguishes_equal_operation_occurrences() {
        let source = Value::new(1, 0);
        let operation = crate::model::mir::Op::new(0, None, "", vec![source], vec![]);
        let body = MirBody::new(
            0,
            vec![MirBlock::new(
                0,
                vec![],
                vec![operation.clone(), operation],
                vec![],
            )],
        );

        let found = operations(&body)
            .map(|(occurrence, _, _)| occurrence)
            .collect::<Vec<_>>();

        assert_eq!(found.len(), 2);
        assert_ne!(found[0], found[1]);
    }

    #[test]
    fn direct_induction_transparent_aliases_distinguishes_equal_phi_occurrences() {
        let phi = Phi::new(Value::new(1, 0));
        let body = MirBody::new(
            0,
            vec![MirBlock::new(0, vec![phi.clone(), phi], vec![], vec![])],
        );

        let found = phis(&body)
            .map(|(occurrence, _, _)| occurrence)
            .collect::<Vec<_>>();

        assert_eq!(found.len(), 2);
        assert_ne!(found[0], found[1]);
    }
}
