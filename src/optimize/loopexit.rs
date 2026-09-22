//! Port of `qbopt/optimize/loopexit.py`.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ssa::{self, SubstitutionError};
use crate::model::mir::{MirBlock, MirBody, Op, Phi, Value};

// ---- early port (agent F) ----

pub(crate) fn _substituted_exits(
    body: &MirBody,
    exit_at: i64,
    following: &BTreeSet<i64>,
    added: Vec<Op>,
    swap: &BTreeMap<u32, Value>,
) -> Result<MirBody, SubstitutionError> {
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let mut phis = Vec::new();
        for phi in &block.phis {
            if block.at == exit_at && swap.contains_key(&phi.result.id) {
                continue;
            }
            let mut incoming = phi.incoming.clone();
            for (predecessor, value) in phi.incoming.iter() {
                if following.contains(predecessor) {
                    incoming.insert(*predecessor, ssa::provider(*value, swap)?);
                }
            }
            phis.push(Phi {
                result: phi.result,
                incoming,
            });
        }
        let mut ops = if block.at == exit_at {
            added.clone()
        } else {
            Vec::new()
        };
        if following.contains(&block.at) {
            for op in &block.ops {
                ops.push(ssa::substituted(op, swap)?);
            }
        } else {
            ops.extend(block.ops.iter().cloned());
        }
        blocks.push(MirBlock {
            at: block.at,
            phis,
            ops,
            succ: block.succ.clone(),
        });
    }
    Ok(MirBody {
        blocks,
        ..body.clone()
    })
}
