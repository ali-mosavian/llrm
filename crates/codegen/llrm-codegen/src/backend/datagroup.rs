//! Make the data segment register a fourth selector register where the
//! stack lives in the data group.
//!
//! # How it works
//! When the stack segment reaches the same memory as the data segment, an
//! access to the data group can name the stack segment instead (`ss:`).
//! The data segment register is then free to hold a far-pointer selector
//! between the points that need it to hold the data group
//! (`target::needs_data_group`: calls, returns, traps, opaque
//! instructions, x87 and string instructions). Allocation may place a
//! selector there. This pass makes the rest of the body agree.
//!
//! [`restored`] runs a forward analysis, "the register may hold something
//! else", over the allocated body. A write to the register sets it. It
//! stays set across blocks to every successor.
//!
//! ```text
//!   mov  ds, bx            ; DS now a selector       dirty
//!   mov  ax, [si]          ; a data cell      =>     mov ax, ss:[si]
//!   les  di, [var]         ; data cell        =>     les di, ss:[var]
//!   call f                 ; needs DS = data  =>     mov ds, ss   (restore)
//!                                                    call f
//! ```
//!
//! Where it is set, each memory operand that would take its segment from
//! the data segment register is prefixed with the stack segment. That means
//! an operand with no segment of its own, in a segment, external or
//! literal space, whose base is not itself the frame or stack register
//! (those select the stack segment already). Immediately before an
//! instruction that needs the data group the register is reloaded from the
//! stack segment register and the flag clears. A block with no successor
//! that leaves with the flag set restores before its jump or return.
//!
//! Nothing is done when the target has no stack-segment route
//! (`Segments::through` is none) or when the body writes no data segment
//! register. [`names_data_segment`] tells allocation to leave the register
//! alone when the body already names it, since such a body manages DS
//! itself, and then `data_free` is false here.
//!
//! The runtime's interrupt handlers load their own data segment, so a value
//! held in DS is safe from them.
//!
//! # Example
//! No honest example: the file has no unit test, and a small program that makes
//! the allocator put a selector in `ds` under `-mstack-is-data` was not found.
//!
//! # Used by
//! Register allocation (`allocate.rs`): `names_data_segment` before it,
//! `restored` on the allocated body it keeps.
//!
//! # See also
//! `-m[no-]stack-is-data`; [`crate::backend`]

use std::collections::BTreeMap;
use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::backend::target::{self, Segments};
use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::objectfile::module::{Addr, Space};

/// Whether `body` names the data segment register itself: it then manages
/// that register, and allocation leaves it alone.
pub fn names_data_segment(
    body: &LirBody,
    segments: &Segments,
) -> bool {
    let data = segments.data;
    body.blocks
        .iter()
        .flat_map(|block| &block.insns)
        .any(
            |one| one.requires.iter().chain(&one.delivers).any(|(_, register)| ir::root(*register) == data)
                || one.what.as_ref().is_some_and(|what| {
                    what.dests
                        .iter()
                        .chain(&what.sources)
                        .any(|place| matches!(place, Loc::Reg(reg) if ir::root(reg.register) == data))
                }),
        )
}

/// `body`, allocated, with the data group reached through the stack segment
/// wherever the data segment register may hold a value, and restored before
/// every point that needs it.
pub fn restored(
    body: &LirBody,
    data_free: bool,
    segments: &Segments,
) -> LirBody {
    let regs = body.regs();
    let data = segments.data;
    let Some(through) = segments.through else {
        return body.clone();
    };
    if !data_free || !body.blocks.iter().flat_map(|block| &block.insns).any(|one| _writes_data(one, data)) {
        return body.clone();
    }
    let at: BTreeMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    // Whether the register may hold something else on entry to each block.
    let mut dirty = vec![false; body.blocks.len()];
    loop {
        let mut changed = false;
        for (index, block) in body.blocks.iter().enumerate() {
            let out = _walk(regs, &block.insns, dirty[index], data, through, segments.offset_bytes, None);
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
        let left = _walk(regs, &block.insns, dirty[index], data, through, segments.offset_bytes, Some(&mut insns));
        if left && block.succ.is_empty() {
            let last = insns.last().cloned();
            let position = if last.as_ref().is_some_and(|one| _leaves(one)) { insns.len() - 1 } else { insns.len() };
            let beside =
                last.unwrap_or_else(|| Arc::new(Insn::new(block.at, Some((block.at, block.at)), None, vec![], vec![])));
            insns.insert(position, _restore(&beside, data, through));
        }
        out.blocks[index] = block.with_insns(insns);
    }
    out
}

/// Walk `insns` from `dirty`, returning whether the register may hold
/// something else after them; into `out`, the rewritten instructions.
fn _walk(
    regs: Regs,
    insns: &[Arc<Insn>],
    mut dirty: bool,
    data: RegId,
    through: RegId,
    offset_bytes: i64,
    mut out: Option<&mut Vec<Arc<Insn>>>,
) -> bool {
    for one in insns {
        if dirty && target::needs_data_group(one) {
            if let Some(out) = out.as_deref_mut() {
                out.push(_restore(one, data, through));
            }
            dirty = false;
        }
        if let Some(out) = out.as_deref_mut() {
            out.push(if dirty { _through(regs, one, through, offset_bytes) } else { Arc::clone(one) });
        }
        if _writes_data(one, data) {
            dirty = true;
        }
    }
    dirty
}

fn _writes_data(
    one: &Insn,
    data: RegId,
) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.dests.iter().any(|place| matches!(place, Loc::Reg(reg) if ir::root(reg.register) == data)),
        )
}

/// A jump or return that ends its block.
fn _leaves(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(
                what.op,
                Operation::Jump | Operation::Branch | Operation::Return
            ),
        )
}

/// The data segment register loaded with the data group, beside `one`.
fn _restore(
    one: &Insn,
    data: RegId,
    through: RegId,
) -> Arc<Insn> {
    let at = one.covers.map_or(one.at, |covers| covers.0);
    let what = Semantics {
        op: Operation::Move,
        name: Some("mov".into()),
        dests: vec![Loc::Reg(ir::Reg { register: data, width: 2 })],
        sources: vec![Loc::Reg(ir::Reg { register: through, width: 2 })],
        target: None,
        indirect: false,
        table: Vec::new(),
        repeat: false,
        pops: None,
    };
    let mut made = Insn::new(one.at, Some((at, at)), Some(what), vec![], vec![]);
    made.call = one.call.clone();
    Arc::new(made)
}

/// `one`, with each access that would read the data group through the data
/// segment register reaching it through `through` instead.
fn _through(
    regs: Regs,
    one: &Arc<Insn>,
    through: RegId,
    offset_bytes: i64,
) -> Arc<Insn> {
    let Some(what) = &one.what else {
        return Arc::clone(one);
    };
    let moved = |place: &Loc| match place {
        Loc::Mem(cell) => _prefixed(regs, cell, through, offset_bytes).map(Loc::Mem),
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
fn _prefixed(
    regs: Regs,
    cell: &ir::Mem,
    through: RegId,
    offset_bytes: i64,
) -> Option<ir::Mem> {
    let stack_based = |base: RegId| regs.is_frame(base) || regs.is_stack(base);
    let Some(addr) = cell.addr else {
        if cell.index.is_some() || !regs.holds_a_segment_offset(cell.through, offset_bytes) {
            return None;
        }
        let addr = Addr {
            space: Space::Literal,
            disp: cell.offset,
            index: 0,
            base: cell.through.iced(),
            segment: through.iced(),
        };
        return Some(ir::Mem { addr: Some(addr), ..cell.clone() });
    };
    if addr.segment != (RegId::None).iced() {
        return None;
    }
    if !matches!(addr.space, Space::Segment | Space::External | Space::Literal) {
        return None;
    }
    let base = if cell.index.is_some() || cell.base.is_some() { cell.through } else { RegId::from(addr.base) };
    if stack_based(base) {
        return None;
    }
    Some(ir::Mem { addr: Some(Addr { segment: through.iced(), ..addr }), ..cell.clone() })
}
