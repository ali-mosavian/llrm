//! Port of `qbopt/backend/lower_floats.py`: refuse floating operations the
//! backend cannot encode.

use crate::backend::lower::Unlowered;
use crate::frontend::raising_floats;
use crate::model::mir::{Kind, MirBody, Op};

fn _removed(op: &Op) -> Result<bool, Unlowered> {
    if !matches!(op.kind, Kind::Nothing | Kind::Fcheck) {
        return Ok(false);
    }
    if op.floating.is_some() || op.stack.is_some() {
        return Err(Unlowered("removed floating operation retains computation".into()));
    }
    Ok(true)
}

pub fn checked(body: &MirBody) -> Result<(), Unlowered> {
    let repetitions: crate::support::hash::HashMap<i64, i64> = body.repetitions.iter().copied().collect();
    if repetitions.len() != body.repetitions.len()
        || body.repetitions.iter().any(|&(at, count)| {
            body.block(at).is_none_or(|block| !(2 <= count && count <= block.ops.len() as i64))
        })
    {
        return Err(Unlowered("invalid block repetition provenance".into()));
    }
    for block in &body.blocks {
        for op in &block.ops {
            if _removed(op)? {
                continue;
            }
            let Some(floating) = &op.floating else {
                continue;
            };
            // Selection encodes the instruction's name and operands, never
            // `floating`; only the exception policy is not in the bytes.
            let encoded = raising_floats::semantics(op);
            if encoded.is_none_or(|encoded| {
                crate::model::floating::Semantics { exceptions: floating.exceptions, ..encoded } != *floating
            }) {
                return Err(Unlowered("floating semantics are not what the instruction encodes".into()));
            }
        }
    }
    Ok(())
}
