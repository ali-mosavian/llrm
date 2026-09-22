//! Port of `qbopt/backend/machinedce.py`: eliminate pure allocated
//! instructions whose complete result is dead.
//!
//! MIR dead-code elimination reasons about values before allocation. Lowering,
//! splitting and physical rewrites can leave a machine computation whose virtual
//! definition still exists but whose physical register and flag results are all
//! dead. This pass answers only that machine question. Source memory reads,
//! control flow, trapping arithmetic, x87 work and relocations are deliberately
//! outside it.

use crate::support::hash::HashSet;
use std::sync::Arc;

use iced_x86::Register;

use crate::backend::liveness;
use crate::backend::peephole::{Lanes, _branch_reads, _register_effects, id};
use crate::model::ir::{Loc, Operation, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

const _PURE: [Operation; 9] = [
    Operation::Move,
    Operation::Exchange,
    Operation::Address,
    Operation::Binary,
    Operation::Multiply,
    Operation::Compare,
    Operation::Unary,
    Operation::Funnel,
    Operation::Extend,
];
const _STATEFUL_REGISTERS: [Register; 6] =
    [Register::ES, Register::CS, Register::SS, Register::DS, Register::FS, Register::GS];

/// Remove an allocated computation with no live physical result.
pub struct MachineDCE;

impl LIRTransform for MachineDCE {
    fn class_name(&self) -> &'static str {
        "MachineDCE"
    }

    fn name(&self) -> &str {
        "machine-dce"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(eliminated(body))
    }
}

fn _relocated(where_: &Loc) -> bool {
    match where_ {
        Loc::Imm(one) => one.address.is_some(),
        Loc::Address(one) => one.addr.is_some_and(|addr| addr.space != Space::Frame),
        _ => false,
    }
}

/// Architectural state whose writes are not ordinary dead values.
fn _stateful_destination(where_: &Loc) -> bool {
    matches!(where_, Loc::Reg(one)
        if one.register.full_register32() == Register::ESP || _STATEFUL_REGISTERS.contains(&one.register))
}

/// Whether removing this occurrence can remove only registers and flags.
fn _pure(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    _PURE.contains(&what.op)
        && what.target.is_none()
        && !what.indirect
        && what.dests.iter().chain(&what.sources).all(|arg| matches!(arg, Loc::Reg(_) | Loc::Imm(_) | Loc::Address(_)))
        && !what.dests.iter().chain(&what.sources).any(_relocated)
        && !what.dests.iter().any(_stateful_destination)
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && one.group.is_none()
        && one.symbol != Some(true)
        && !one.frame_adjust
        && !one.spill_reload
        && !one.spill_store
        && !one.op.as_ref().is_some_and(|op| op.barrier())
}

/// `body` with one sweep's dead work anchored, or None where Python returns `body` itself.
fn _once(body: &LirBody) -> Option<LirBody> {
    let exits = liveness::dead_at_exit(body);
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut dead = exits[&block.at].clone();
        let mut redundant: HashSet<usize> = HashSet::default();
        for one in block.insns.iter().rev() {
            let what = one.what.as_ref();
            if liveness::_terminator(what) {
                let what = what.expect("a terminator has semantics");
                if what.op == Operation::Branch {
                    dead = dead.difference(&_branch_reads(what)).copied().collect();
                }
                continue;
            }
            let mut effects = _register_effects(one, false, true);
            if effects.is_none() {
                effects = liveness::_declared(one);
            }
            let Some((reads, writes)) = effects else {
                dead.clear();
                continue;
            };
            if !writes.is_empty() && writes.is_subset(&dead) && _pure(one) {
                redundant.insert(id(one));
                changed = true;
                continue;
            }
            dead = dead.union(&writes).copied().collect::<Lanes>().difference(&reads).copied().collect();
        }
        blocks.push(if redundant.is_empty() {
            block.clone()
        } else {
            LirBlock {
                insns: block
                    .insns
                    .iter()
                    .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                    .collect(),
                ..block.clone()
            }
        });
    }
    if changed { Some(LirBody { blocks, ..body.clone() }) } else { None }
}

/// Remove dead pure machine work to a fixed point across CFG edges.
pub fn eliminated(body: LirBody) -> LirBody {
    let mut body = body;
    for _round in 0..std::cmp::max(1, body.insns().len()) {
        // `after is body`: `_once` answers None where it returned `body` itself.
        match _once(&body) {
            None => return body,
            Some(after) => body = after,
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::eliminated;
    use crate::model::ir::{Imm, Loc, Mem, Operation, Reg, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn what(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Option<Semantics> {
        Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) })
    }

    fn reg(register: Register) -> Loc {
        Loc::Reg(Reg { register, width: 2 })
    }

    fn _mov(at: i64, value: u32, register: Register) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 3)),
            what(Operation::Move, "mov", vec![reg(register)], vec![Loc::Imm(Imm {
                value: i64::from(value),
                width: 2,
                address: None,
            })]),
            vec![value],
            vec![],
        ))
    }

    fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
        LirBlock { succ, ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        LirBody::new("machine-dce", 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn branch(at: i64, name: &str, uses: Vec<u32>) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 2)),
            Some(Semantics { name: Some(name.to_owned()), target: Some(10), ..Semantics::new(Operation::Branch) }),
            vec![],
            uses,
        ))
    }

    #[test]
    fn test_dead_register_definition_is_eliminated_across_a_cfg_edge() {
        // An allocated result overwritten on every successor used to survive.
        let dead = _mov(0, 1, Register::AX);
        let overwrite = _mov(5, 2, Register::AX);
        let result = eliminated(body(vec![block(0, vec![Arc::clone(&dead)], vec![5]), block(5, vec![overwrite], vec![])]));
        let first = result.blocks.iter().find(|block| block.at == 0).unwrap();
        assert_eq!(first.insns[0].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(first.insns[0].defines, dead.defines);
    }

    #[test]
    fn test_definition_live_on_one_successor_is_kept() {
        let definition = _mov(0, 1, Register::AX);
        let read = Arc::new(Insn::new(
            10,
            Some((10, 12)),
            what(Operation::Move, "mov", vec![reg(Register::BX)], vec![reg(Register::AX)]),
            vec![3],
            vec![1],
        ));
        let result = eliminated(body(vec![
            block(0, vec![Arc::clone(&definition)], vec![5, 10]),
            block(5, vec![_mov(5, 2, Register::AX)], vec![]),
            block(10, vec![read], vec![]),
        ]));
        assert_eq!(result.blocks[0].insns[0].what, definition.what);
    }

    #[test]
    fn test_dead_compare_is_eliminated_when_the_next_compare_replaces_flags() {
        let first = Arc::new(Insn::new(
            0,
            Some((0, 2)),
            what(Operation::Compare, "cmp", vec![], vec![reg(Register::AX), reg(Register::BX)]),
            vec![1],
            vec![],
        ));
        let second = Arc::new(Insn { at: 2, covers: Some((2, 4)), defines: vec![2], ..(*first).clone() });
        let result = eliminated(body(vec![
            block(0, vec![first, Arc::clone(&second), branch(4, "je", vec![2])], vec![10, 20]),
            block(10, vec![], vec![]),
            block(20, vec![], vec![]),
        ]));
        assert_eq!(result.blocks[0].insns[0].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(result.blocks[0].insns[1].what, second.what);
    }

    #[test]
    fn test_dead_value_is_kept_when_its_flags_feed_a_branch() {
        let add = Arc::new(Insn::new(
            0,
            Some((0, 3)),
            what(Operation::Binary, "add", vec![reg(Register::AX)], vec![
                reg(Register::AX),
                Loc::Imm(Imm { value: 1, width: 2, address: None }),
            ]),
            vec![1],
            vec![],
        ));
        let result = eliminated(body(vec![
            block(0, vec![Arc::clone(&add), branch(3, "jne", vec![1])], vec![10, 20]),
            block(10, vec![], vec![]),
            block(20, vec![], vec![]),
        ]));
        assert_eq!(result.blocks[0].insns[0].what, add.what);
    }

    #[test]
    fn test_dead_memory_load_is_kept() {
        let load = Arc::new(Insn::new(
            0,
            Some((0, 3)),
            what(Operation::Move, "mov", vec![reg(Register::AX)], vec![Loc::Mem(Mem {
                through: Register::BX,
                ..Mem::new(None, 2)
            })]),
            vec![1],
            vec![],
        ));
        let result =
            eliminated(body(vec![block(0, vec![Arc::clone(&load)], vec![5]), block(5, vec![_mov(5, 2, Register::AX)], vec![])]));
        assert_eq!(result.blocks[0].insns[0].what, load.what);
    }

    #[test]
    fn test_call_stack_cleanup_is_never_dead_machine_work() {
        // C calls lost `add sp,N` cleanup when a later compare killed its flags.
        let cleanup = Arc::new(Insn::new(
            0,
            Some((0, 3)),
            what(Operation::Binary, "add", vec![reg(Register::SP)], vec![
                reg(Register::SP),
                Loc::Imm(Imm { value: 4, width: 2, address: None }),
            ]),
            vec![],
            vec![],
        ));
        let compare = Arc::new(Insn::new(
            3,
            Some((3, 5)),
            what(Operation::Compare, "cmp", vec![], vec![reg(Register::AX), reg(Register::BX)]),
            vec![1],
            vec![],
        ));
        let result = eliminated(body(vec![
            block(0, vec![Arc::clone(&cleanup), compare, branch(5, "je", vec![1])], vec![10, 20]),
            block(10, vec![], vec![]),
            block(20, vec![], vec![]),
        ]));
        assert_eq!(result.blocks[0].insns[0].what, cleanup.what);
    }
}
