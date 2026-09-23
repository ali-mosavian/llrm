//! Port of `qbopt/frontend/raising_frame.py`: prove argument-stack accesses
//! disjoint from unescaped main-frame locals.
//!
//! Runtime-specific geometry and machine stack tracking end here. Consumers
//! see only byte-range exclusions on individual memory effects.

use iced_x86::{FlowControl, Mnemonic, OpKind, Register};

use crate::abi::runtime::{self, Contract, Control, Memory, Reg};
use crate::frontends::bc::blocks::{Block, ENTRY, event_enabled, has_header};
use crate::frontends::bc::declen::{Insn, WRITES, instruction_info_factory};
use crate::model::mir::{Arg, Cell, Kind, MemRef, Op, RaisedBody};
use crate::objectfile::module::{self, Addr, Family, Module, Space};
use crate::support::hash::IndexMap;

/// `FIXED.get(family)`.
#[allow(non_snake_case)]
pub fn FIXED(family: Family) -> Option<i64> {
    match family {
        Family::Quickbasic => Some(10),
        Family::Pds => Some(18),
        Family::Vbdos => Some(20),
        _ => None,
    }
}

pub const POINTERS: [Register; 4] = [Register::SP, Register::ESP, Register::BP, Register::EBP];

pub fn _layout(found: &Module) -> Option<(i64, i64)> {
    let fixed = FIXED(module::family(&found.records))?;
    if !has_header(found) {
        return None;
    }
    let size = i64::from(u16::from_le_bytes([
        found.code.get(0x22).copied().unwrap_or(0),
        found.code.get(0x23).copied().unwrap_or(0),
    ]));
    if !(0 < size && size < 0x8000 - fixed) {
        return None;
    }
    Some((-fixed - size, size))
}

pub fn _private(blocks: &[Block]) -> bool {
    for block in blocks {
        for decoded in &block.insns {
            let insn = &decoded.insn;
            if (0..insn.op_count())
                .any(|index| insn.op_kind(index) == OpKind::Register && POINTERS.contains(&insn.op_register(index)))
            {
                return false;
            }
            if insn.mnemonic() == Mnemonic::Lea && POINTERS.contains(&insn.memory_base()) {
                return false;
            }
        }
    }
    true
}

pub fn _after(decoded: &Insn, depth: Option<i64>, contracts: &IndexMap<i64, Contract>) -> Option<i64> {
    let depth = depth?;
    let insn = &decoded.insn;
    if insn.flow_control() == FlowControl::Interrupt {
        return None;
    }
    if insn.mnemonic() == Mnemonic::Call {
        let contract = contracts.get(&(decoded.at as i64))?;
        if !contract.established
            || runtime::barrier(contract)
            || contract.control != Control::Returns
            || contract.clobbers.contains(&Reg::Sp)
            || contract.clobbers.contains(&Reg::Bp)
        {
            return None;
        }
        return Some(depth + contract.cleanup?);
    }
    let mut info = instruction_info_factory();
    let writes: Vec<Register> = info
        .info(insn)
        .used_registers()
        .iter()
        .filter(|used| WRITES.contains(&used.access()))
        .map(|used| used.register())
        .collect();
    if writes.iter().any(|one| matches!(one, Register::BP | Register::EBP | Register::SS)) {
        return None;
    }
    if insn.mnemonic() == Mnemonic::Pop && matches!(insn.op0_register(), Register::SP | Register::ESP) {
        return None;
    }
    if writes.iter().any(|one| matches!(one, Register::SP | Register::ESP))
        && !matches!(insn.mnemonic(), Mnemonic::Push | Mnemonic::Pop)
    {
        return None;
    }
    Some(depth + i64::from(insn.stack_pointer_increment()))
}

pub fn _depths(
    blocks: &[Block],
    entry: usize,
    initial: i64,
    contracts: &IndexMap<i64, Contract>,
) -> IndexMap<usize, Option<i64>> {
    let mut predecessors: IndexMap<usize, Vec<usize>> = blocks.iter().map(|block| (block.at, Vec::new())).collect();
    for block in blocks {
        for successor in &block.succ {
            if let Some(one) = predecessors.get_mut(successor) {
                one.push(block.at);
            }
        }
    }
    // The outer `None` is Python's UNSEEN.
    let mut exits: IndexMap<usize, Option<Option<i64>>> = predecessors.keys().map(|&at| (at, None)).collect();
    let mut entries: IndexMap<usize, Option<i64>> = IndexMap::default();
    let mut changed = true;
    while changed {
        changed = false;
        for block in blocks {
            let mut incoming: Vec<Option<i64>> =
                predecessors[&block.at].iter().filter_map(|at| exits[at]).collect();
            if block.at == entry {
                incoming.push(Some(initial));
            }
            if incoming.is_empty() {
                continue;
            }
            let mut depth = if incoming.iter().all(|one| *one == incoming[0]) { incoming[0] } else { None };
            entries.insert(block.at, depth);
            for decoded in &block.insns {
                depth = _after(decoded, depth, contracts).filter(|&depth| -0x10000 < depth && depth <= initial);
            }
            if exits[&block.at] != Some(depth) {
                exits.insert(block.at, Some(depth));
                changed = true;
            }
        }
    }
    let mut result = IndexMap::default();
    for block in blocks {
        let mut depth = entries.get(&block.at).copied().flatten();
        for decoded in &block.insns {
            result.insert(decoded.at, depth);
            depth = _after(decoded, depth, contracts).filter(|&depth| -0x10000 < depth && depth <= initial);
        }
    }
    result
}

pub fn annotated(
    body: RaisedBody,
    found: &Module,
    blocks: &[Block],
    contracts: &IndexMap<i64, Contract>,
) -> RaisedBody {
    let layout = _layout(found);
    let Some((floor, size)) = layout.filter(|_| body.entry == ENTRY as i64) else {
        return body;
    };
    if event_enabled(found) || runtime::handles_errors(contracts.values()) {
        return body;
    }
    let depths = _depths(blocks, body.entry as usize, floor, contracts);
    let private = _private(blocks);
    let decoded: IndexMap<usize, &Insn> =
        blocks.iter().flat_map(|block| &block.insns).map(|one| (one.at, one)).collect();
    let exclusion = (Addr::new(Space::Frame, floor), size as u32);

    let rewrite = |op: &Op| -> Op {
        let at = op.at as usize;
        let (Some(depth), Some(one)) = (depths.get(&at).copied().flatten(), decoded.get(&at)) else {
            return op.clone();
        };
        let insn = &one.insn;
        let stack = matches!(insn.mnemonic(), Mnemonic::Push | Mnemonic::Pop);
        let Some(after) = _after(one, Some(depth), contracts) else {
            return op.clone();
        };
        if !(-0x10000 < depth.min(after) && depth.max(after) <= floor) {
            return op.clone();
        }
        let contract = contracts.get(&op.at);
        let own = private
            && op.kind == Kind::Call
            && contract.is_some_and(|contract| {
                contract.writes == Memory::Own
                    && contract.reads == Memory::Own
                    && contract
                        .inputs
                        .as_ref()
                        .is_some_and(|inputs| !inputs.contains(&Reg::Bp) && !inputs.contains(&Reg::Sp))
            });
        if !stack && !own {
            return op.clone();
        }

        let implicit: &[MemRef] = if insn.mnemonic() == Mnemonic::Push {
            &op.stores
        } else if stack {
            &op.loads
        } else {
            &[]
        };
        let implicit: Vec<&MemRef> = implicit
            .iter()
            .filter(|one| {
                one.base.is_none()
                    && one.segment.is_none()
                    && one.addr.is_none_or(|addr| addr.space == Space::Stack)
            })
            .collect();

        let reference = |one: &MemRef| -> MemRef {
            if implicit.contains(&one) || (own && one.addr.is_none()) {
                let mut made = one.clone();
                made.excludes = Vec::new();
                for &excluded in one.excludes.iter().chain([&exclusion]) {
                    if !made.excludes.contains(&excluded) {
                        made.excludes.push(excluded);
                    }
                }
                return made;
            }
            one.clone()
        };
        let argument = |arg: &Arg| -> Arg {
            match arg {
                Arg::Cell(cell) => Arg::Cell(Cell { r#ref: reference(&cell.r#ref) }),
                _ => arg.clone(),
            }
        };

        let mut made = op.clone();
        made.loads = op.loads.iter().map(reference).collect();
        made.stores = op.stores.iter().map(reference).collect();
        made.args = op.args.iter().map(argument).collect();
        made.results = op.results.iter().map(argument).collect();
        made
    };

    let blocks = body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(rewrite).collect())).collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
#[path = "raising_frame_tests.rs"]
mod tests;
