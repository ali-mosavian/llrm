//! `VirtRegRewriter`: the allocation put into the function. Translated from
//! llvm/lib/CodeGen/VirtRegMap.cpp (`VirtRegRewriter::rewrite`,
//! `handleIdentityCopy`) at d1106deb71cc.
//!
//! # How it works
//! `rewrite` replaces every operand's value by the register the allocator
//! gave it, at the operand's width (`MO.setReg(PhysReg)`), puts selector and
//! address registers in their cells, and removes the copies that became
//! identities (`handleIdentityCopy`), keeping the bytes such a copy owned.
//! `_dead_insertions` first drops the allocator's own copies nothing reads.
//!
//! DEVIATE W1 (no number): no sub-register operands. A value is one lane
//! (coalescer C1), so `SuperKills`, `SuperDefs`, `SuperDeads` and
//! `readsUndefSubreg` have nothing to do; x86's AL/AH are registers of the
//! class table.
//!
//! DEVIATE W2 (no number): no kill, dead or `renamable` flags and no block
//! live-ins (`LIS->addKillFlags`, `addMBBLiveIns`): LIR keeps no such flags;
//! the passes after allocation recompute liveness from the instructions.
//!
//! DEVIATE W3 (no number): a parallel copy is not expanded here
//! (`expandCopyBundle`); `parcopy` sequences it after allocation, where which
//! moves conflict is a question about locations.
//!
//! DEVIATE W4 (no number): an x87 value placed in the stack registers loses
//! its value names; the stackifier numbers it again (`fpregs`).
//!
//! # Example
//! The allocator placed `v1` in AX and `v2` in BX; before and after:
//! ```text
//! v1 := mov 5          AX := mov 5
//! v2 := mov v1         BX := mov AX
//! v1 := mov v1         (an identity move: dropped, its bytes kept)
//! ret v2               ret BX
//! ```
//!
//! # Used by
//! `allocate::RegAlloc`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::intervals as ranges;
use crate::backend::allocate::_g;
use crate::backend::allocresult::{Assignment, Error, Spilled, Unplaced};
use crate::backend::classes::RegisterClasses;
use crate::backend::target;
use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Reg, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

/// `VirtRegRewriter::rewrite`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/VirtRegMap.cpp#L654>):
/// `body` with every operand naming a value replaced by its register.
pub fn rewrite(
    body: &LirBody,
    got: &Assignment,
    classes: &RegisterClasses,
) -> Result<LirBody, Error> {
    if !got.spilled.is_empty() {
        let worst: Vec<String> = got.spilled.iter().take(4).map(|one| format!("value#{one}")).collect();
        return Err(Spilled(format!(
            "{} values want a stack slot ({}) at a cost of {} and spilling them did not settle. Measured on \
             bools-q-evt: the same twelve every round, thirty-six instructions added each time. Their ranges \
             cross calls that clobber every register, so no register can hold them and the reload cannot either",
            got.spilled.len(),
            worst.join(", "),
            _g(got.cost),
        ))
        .into());
    }
    let body = _dead_insertions(body);
    let held = &got.r#where;
    let undef_super = _undef_supers(&body);
    let failed: RefCell<Option<Unplaced>> = RefCell::new(None);
    let blocks = body
        .blocks
        .iter()
        .map(|block| LirBlock {
            at: block.at,
            insns: lir::without(
                &block.insns,
                |one| _discardable_identity(one),
                Some(|one: &Arc<Insn>| match _placed_for_rewrite(one, held, classes) {
                    Ok(placed) if undef_super.contains(&ranges::key(one)) => {
                        Arc::new(Insn { undef_super: true, ..(*placed).clone() })
                    }
                    Ok(placed) => placed,
                    Err(error) => {
                        failed.borrow_mut().get_or_insert(error);
                        Arc::clone(one)
                    }
                }),
            )
            .into_iter()
            .map(_identity_anchor)
            .collect(),
            succ: block.succ.clone(),
            phis: block.phis.clone(),
            cold: block.cold,
            align: block.align,
            max_skip: block.max_skip,
        })
        .collect::<Vec<_>>();
    if let Some(error) = failed.into_inner() {
        return Err(error.into());
    }
    Ok(LirBody {
        origin: body.origin.clone(),
        pins: body.pins.clone(),
        ordered: body.ordered,
        ..body.with_blocks(blocks)
    })
}

/// The instructions that define a narrower part of a value than the value is
/// read at while the value is not live before them: what `VirtRegRewriter`
/// gives an `implicit-def` of the super register and no implicit use, which
/// `X86FixupBWInsts::getSuperRegDestIfDead` reads as "the rest is undefined".
/// DEVIATE W1 above has no sub-register operands, so the widths of the value's
/// operands stand in for the sub-register indices.
fn _undef_supers(body: &LirBody) -> BTreeSet<usize> {
    let mut widest: IndexMap<u32, u32> = IndexMap::default();
    let held_places = |one: &Insn| {
        one.what
            .iter()
            .flat_map(|what| what.dests.iter().chain(&what.sources))
            .filter_map(|place| match place {
                Loc::Held(held) => Some(*held),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for block in &body.blocks {
        for one in block.insns.iter() {
            for held in held_places(one) {
                let width = u32::from(held.width);
                widest.entry(held.value).and_modify(|most| *most = (*most).max(width)).or_insert(width);
            }
        }
    }
    let index = ranges::indexed_shared(body);
    let live = ranges::intervals_shared(body, Some(&index));
    let mut found = BTreeSet::new();
    for block in &body.blocks {
        for (position, one) in block.insns.iter().enumerate() {
            let Some(what) = &one.what else { continue };
            let slot = index.slot(block, position);
            for place in &what.dests {
                let Loc::Held(held) = place else { continue };
                let narrower = u32::from(held.width) < widest.get(&held.value).copied().unwrap_or(0);
                let live_before = live
                    .get(&held.value)
                    .is_some_and(|value| value.segments.iter().any(|run| run.start <= slot && slot < run.end));
                if narrower && !live_before && !one.uses.contains(&held.value) {
                    found.insert(ranges::key(one));
                }
            }
        }
    }
    found
}

/// Delete unused allocator copies before physical identity loses their use
/// graph.
pub(crate) fn _dead_insertions(body: &LirBody) -> LirBody {
    if body
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .any(|one| one.what.as_ref().is_none_or(|what| what.op == Operation::Barrier))
    {
        return body.clone();
    }
    let mut body = body.clone();
    loop {
        let mut used: BTreeSet<u32> =
            body.blocks.iter().flat_map(|block| &block.insns).flat_map(|one| one.uses.iter().copied()).collect();
        used.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.insns)
                .flat_map(|one| one.requires.iter().map(|(held, _)| held.value)),
        );
        used.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.phis)
                .flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)),
        );
        let mut dead: BTreeSet<usize> = BTreeSet::new();
        for one in body.blocks.iter().flat_map(|block| &block.insns) {
            if one.covers.is_none_or(|covers| covers.0 != covers.1)
                || !one.spread.is_empty()
                || !one.clobbers.is_empty()
                || !one.requires.is_empty()
                || !one.delivers.is_empty()
                || one.symbol == Some(true)
            {
                continue;
            }
            let Some(what) = &one.what else {
                continue;
            };
            if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
                if let ([Loc::Held(dest)], [source]) = (what.dests.as_slice(), what.sources.as_slice()) {
                    let value = dest.value;
                    if one.defines == [value]
                        && !used.contains(&value)
                        && (matches!(source, Loc::Held(_) | Loc::Imm(_))
                            || matches!(source, Loc::Mem(_)) && one.spill_reload)
                    {
                        dead.insert(ranges::key(one));
                    }
                }
            }
        }
        if dead.is_empty() {
            return body;
        }
        body = body.with_blocks(
            body.blocks
                .iter()
                .map(|block| {
                    block.with_insns(
                        block.insns.iter().filter(|one| !dead.contains(&ranges::key(one))).cloned().collect(),
                    )
                })
                .collect(),
        );
    }
}

/// Retain byte ownership without requiring an encodable register self-copy.
fn _identity_anchor(one: Arc<Insn>) -> Arc<Insn> {
    if one.group.is_some() || !_pointless(&one) {
        return one;
    }
    let mut made = (*one).clone();
    made.what = Some(lir::inert());
    Arc::new(made)
}

/// `handleIdentityCopy`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/VirtRegMap.cpp#L492>):
/// an identity that is not still owned by the parallel-copy scheduler.
fn _discardable_identity(one: &Insn) -> bool {
    one.group.is_none() && _pointless(one)
}

/// Place one instruction without discarding an inserted definition.
fn _placed_for_rewrite(
    one: &Arc<Insn>,
    held: &IndexMap<u32, RegId>,
    classes: &RegisterClasses,
) -> Result<Arc<Insn>, Unplaced> {
    let placed = _placed(one, held, classes)?;
    if placed.group.is_none() && _pointless(&placed) {
        return Ok(lir::anchor(placed));
    }
    Ok(placed)
}

/// `MachineInstr::isIdentityCopy`: whether this instruction moves a register
/// into itself.
fn _pointless(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    if what.op != Operation::Move {
        return false;
    }
    if what.dests.len() != 1 || what.sources.len() != 1 {
        return false;
    }
    matches!(
        (&what.dests[0], &what.sources[0]),
        (Loc::Reg(into), Loc::Reg(out_of)) if into.register == out_of.register
    )
}

pub(crate) fn _placed(
    one: &Arc<Insn>,
    held: &IndexMap<u32, RegId>,
    classes: &RegisterClasses,
) -> Result<Arc<Insn>, Unplaced> {
    let regs = classes.registers;
    let Some(what) = &one.what else {
        return Ok(Arc::clone(one));
    };
    // A value read as undef reads nothing, so the register it was given does
    // not matter: it takes the one the instruction defines (`sbb r, r`, any
    // tied read undef: `Expand2AddrUndef`), else any register of the class
    // (`VirtRegRewriter` gives an undef operand the first of its class).
    let undef_held;
    let ignored = |value: &u32| !one.uses.contains(value);
    let held = if one.undefs.iter().any(|value| ignored(value)) {
        let mut more = held.clone();
        let own = one.defines.iter().find_map(|value| held.get(value)).copied();
        for value in one.undefs.iter().filter(|value| ignored(value)) {
            // The register must have a form at the width the read names: esi
            // has no byte.
            let width = what
                .sources
                .iter()
                .chain(&what.dests)
                .find_map(
                    |place| match place {
                        Loc::Held(read) if read.value == *value => Some(u32::from(read.width) * 8),
                        _ => None,
                    },
                );
            let fits = |register: &RegId| width.is_none_or(|bits| regs.view(regs.root(*register), bits).is_some());
            more.insert(*value, own.filter(fits).or_else(|| held.get(value).copied()).unwrap_or(classes.available[0]));
        }
        undef_held = more;
        &undef_held
    } else {
        held
    };
    let dests = what.dests.iter().map(|x| _settled(x, held, classes)).collect::<Result<Vec<_>, _>>()?;
    let sources = what.sources.iter().map(|x| _settled(x, held, classes)).collect::<Result<Vec<_>, _>>()?;
    let mut name = what.name.clone();
    if target::far_load(regs, what) {
        if let Loc::Reg(selector) = &dests[1] {
            if let Some(spelt) = regs.load_form(selector.register) {
                name = Some(spelt.into());
            }
        }
    }
    let mut made = (**one).clone();
    // A value now in an x87 register is named by the register: the stackifier
    // numbers it again (`fpregs`).
    let in_rfp = |value: &u32| held.get(value).is_some_and(|at| regs.in_class(*at, llrm_lir::registers::class::RFP));
    made.uses.retain(|value| !in_rfp(value));
    made.defines.retain(|value| !in_rfp(value));
    // The copy's own operands are replaced in place: building a `Semantics`
    // from `what.clone()` copied both lists again to drop them.
    let own = made.what.as_mut().expect("an instruction with semantics");
    own.name = name;
    own.dests = dests;
    own.sources = sources;
    Ok(Arc::new(made))
}

/// One operand with its value resolved to the register holding it.
pub(crate) fn _settled(
    place: &Loc,
    held: &IndexMap<u32, RegId>,
    classes: &RegisterClasses,
) -> Result<Loc, Unplaced> {
    let regs = classes.registers;
    // A value or any operand but a cell is not copied to be looked at.
    match place {
        Loc::Held(value) => return _register_of(value, held, classes),
        Loc::Mem(_) => {}
        _ => return Ok(place.clone()),
    }
    let mut place = place.clone();
    if let Loc::Mem(cell) = &place {
        if let Some(selector) = cell.selector {
            let Some(register) = held.get(&selector.value) else {
                return Err(Unplaced(format!("selector value#{} has no register", selector.value)));
            };
            let addr = cell.addr.expect("a selected cell has an address");
            place = Loc::Mem(Mem { addr: Some(Addr { segment: (*register).iced(), ..addr }), ..cell.clone() });
        }
    }
    if let Loc::Mem(cell) = &place {
        if let Some(index) = cell.index {
            let base = cell.base.and_then(|base| held.get(&base.value).copied());
            let index_register = held.get(&index.value).copied();
            let Some(index_register) = index_register.filter(|_| cell.base.is_none() || base.is_some()) else {
                return Err(Unplaced(format!("scaled cell {} has no register for its base or index", cell.repr())));
            };
            let mut base_register = match (base, cell.base) {
                (Some(base), Some(held_base)) => regs.named(base, i64::from(held_base.width)),
                _ => cell.through,
            };
            let mut index_register = regs.named(index_register, i64::from(index.width));
            if cell.base.is_some_and(|held_base| held_base.width == 2 && index.width == 2)
                && cell.scale == 1
                && classes.word_indexes.contains(&base_register)
                && classes.word_bases.contains(&index_register)
            {
                std::mem::swap(&mut base_register, &mut index_register);
            }
            return Ok(Loc::Mem(Mem { through: base_register, index_through: index_register, ..cell.clone() }));
        }
        if let Some(base) = cell.base {
            let Some(register) = held.get(&base.value) else {
                return Ok(place.clone());
            };
            let placed = regs.named(*register, i64::from(base.width).max(2));
            if cell.addr.is_some_and(|addr| addr.space == Space::Frame) {
                return Ok(Loc::Mem(Mem { through: RegId::BP, index_through: placed, ..cell.clone() }));
            }
            return Ok(Loc::Mem(Mem { through: placed, ..cell.clone() }));
        }
    }
    let Loc::Held(value) = &place else {
        return Ok(place);
    };
    _register_of(value, held, classes)
}

/// A value at its width, in the register holding it.
fn _register_of(
    value: &Held,
    held: &IndexMap<u32, RegId>,
    classes: &RegisterClasses,
) -> Result<Loc, Unplaced> {
    let regs = classes.registers;
    let Some(register) = held.get(&value.value) else {
        return Err(Unplaced(format!("value#{} at width {} has no register", value.value, value.width)));
    };
    // An x87 value is placed in FP0..FP6: the stackifier puts it on the stack
    // and its operands become st(i).
    if regs.in_class(*register, llrm_lir::registers::class::RFP) {
        return Ok(Loc::Reg(Reg { register: *register, width: value.width }));
    }
    if value.width == 1 && regs.bytes(*register) == Some(1) {
        return Ok(Loc::Reg(Reg { register: *register, width: 1 }));
    }
    Ok(Loc::Reg(Reg { register: regs.named(*register, i64::from(value.width)), width: value.width }))
}
