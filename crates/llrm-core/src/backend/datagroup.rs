//! The data segment register as a fourth selector register.
//!
//! Where the machine's stack lives in the data group, an access to the data
//! group can go through the stack segment instead. Allocation may then place
//! a selector in the data segment register between the points that need it to
//! hold the data group (`target::needs_data_group`); this pass makes the rest
//! of the body agree: every access that would read the data group through it
//! is prefixed with the stack segment while it may hold something else, and
//! it is restored before each point that needs it and at each exit.
//!
//! The runtime's interrupt handlers load their own data segment, so a value
//! held here is safe from them.

use std::collections::BTreeMap;
use std::sync::Arc;

use iced_x86::Register;

use crate::backend::target;
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::objectfile::module::{Addr, Space};

/// Whether `body` names the data segment register itself: it then manages
/// that register, and allocation leaves it alone.
pub fn names_data_segment(body: &LirBody) -> bool {
    let data = *target::DATA_SEGMENT;
    body.blocks.iter().flat_map(|block| &block.insns).any(|one| {
        one.requires.iter().chain(&one.delivers).any(|(_, register)| ir::root(*register) == data)
            || one.what.as_ref().is_some_and(|what| {
                what.dests
                    .iter()
                    .chain(&what.sources)
                    .any(|place| matches!(place, Loc::Reg(reg) if ir::root(reg.register) == data))
            })
    })
}

/// `body`, allocated, with the data group reached through the stack segment
/// wherever the data segment register may hold a value, and restored before
/// every point that needs it.
pub fn restored(body: &LirBody, data_free: bool) -> LirBody {
    let Some(through) = *target::DATA_THROUGH else {
        return body.clone();
    };
    if !data_free || !body.blocks.iter().flat_map(|block| &block.insns).any(|one| _writes_data(one)) {
        return body.clone();
    }
    let at: BTreeMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    // Whether the register may hold something else on entry to each block.
    let mut dirty = vec![false; body.blocks.len()];
    loop {
        let mut changed = false;
        for (index, block) in body.blocks.iter().enumerate() {
            let out = _walk(&block.insns, dirty[index], through, None);
            for next in &block.succ {
                let next = at[next];
                if out && !dirty[next] {
                    dirty[next] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut out = body.clone();
    for (index, block) in body.blocks.iter().enumerate() {
        let mut insns = Vec::with_capacity(block.insns.len());
        let left = _walk(&block.insns, dirty[index], through, Some(&mut insns));
        if left && block.succ.is_empty() {
            let last = insns.last().cloned();
            let position = if last.as_ref().is_some_and(|one| _leaves(one)) { insns.len() - 1 } else { insns.len() };
            let beside = last.unwrap_or_else(|| Arc::new(Insn::new(block.at, Some((block.at, block.at)), None, vec![], vec![])));
            insns.insert(position, _restore(&beside, through));
        }
        out.blocks[index] = block.with_insns(insns);
    }
    out
}

/// Walk `insns` from `dirty`, returning whether the register may hold
/// something else after them; into `out`, the rewritten instructions.
fn _walk(insns: &[Arc<Insn>], mut dirty: bool, through: Register, mut out: Option<&mut Vec<Arc<Insn>>>) -> bool {
    for one in insns {
        if dirty && target::needs_data_group(one) {
            if let Some(out) = out.as_deref_mut() {
                out.push(_restore(one, through));
            }
            dirty = false;
        }
        if let Some(out) = out.as_deref_mut() {
            out.push(if dirty { _through(one, through) } else { Arc::clone(one) });
        }
        if _writes_data(one) {
            dirty = true;
        }
    }
    dirty
}

fn _writes_data(one: &Insn) -> bool {
    let data = *target::DATA_SEGMENT;
    one.what.as_ref().is_some_and(|what| {
        what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if ir::root(reg.register) == data))
    })
}

/// A jump or return that ends its block.
fn _leaves(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch | Operation::Return))
}

/// The data segment register loaded with the data group, beside `one`.
fn _restore(one: &Insn, through: Register) -> Arc<Insn> {
    let at = one.covers.map_or(one.at, |covers| covers.0);
    let what = Semantics {
        op: Operation::Move,
        name: Some("mov".to_owned()),
        dests: vec![Loc::Reg(ir::Reg { register: *target::DATA_SEGMENT, width: 2 })],
        sources: vec![Loc::Reg(ir::Reg { register: through, width: 2 })],
        target: None,
        indirect: false,
    };
    let mut made = Insn::new(one.at, Some((at, at)), Some(what), vec![], vec![]);
    made.op = one.op.clone();
    Arc::new(made)
}

/// `one`, with each access that would read the data group through the data
/// segment register reaching it through `through` instead.
fn _through(one: &Arc<Insn>, through: Register) -> Arc<Insn> {
    let Some(what) = &one.what else {
        return Arc::clone(one);
    };
    let moved = |place: &Loc| match place {
        Loc::Mem(cell) => _prefixed(cell, through).map(Loc::Mem),
        _ => None,
    };
    if !what.dests.iter().chain(&what.sources).any(|place| moved(place).is_some()) {
        return Arc::clone(one);
    }
    let rewrite = |places: &[Loc]| places.iter().map(|place| moved(place).unwrap_or_else(|| place.clone())).collect();
    let mut made = (**one).clone();
    made.what = Some(Semantics { dests: rewrite(&what.dests), sources: rewrite(&what.sources), ..what.clone() });
    Arc::new(made)
}

/// `cell` reached through `through`, when without it the data segment
/// register would supply its segment: no prefix, and no base that selects
/// the stack segment itself.
fn _prefixed(cell: &ir::Mem, through: Register) -> Option<ir::Mem> {
    let stack_based = |base: Register| matches!(base, Register::BP | Register::EBP | Register::SP | Register::ESP);
    let Some(addr) = cell.addr else {
        if cell.index.is_some() || !matches!(cell.through, Register::SI | Register::DI | Register::BX) {
            return None;
        }
        let addr = Addr { space: Space::Literal, disp: cell.offset, index: 0, base: cell.through, segment: through };
        return Some(ir::Mem { addr: Some(addr), ..cell.clone() });
    };
    if addr.segment != Register::None {
        return None;
    }
    if !matches!(addr.space, Space::Segment | Space::External | Space::Literal) {
        return None;
    }
    let base = if cell.index.is_some() || cell.base.is_some() { cell.through } else { addr.base };
    if stack_based(base) {
        return None;
    }
    Some(ir::Mem { addr: Some(Addr { segment: through, ..addr }), ..cell.clone() })
}
