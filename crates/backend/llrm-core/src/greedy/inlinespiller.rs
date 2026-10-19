//! `InlineSpiller`: spills a live range by rewriting each instruction that
//! names it through the stack, inside the allocation in progress. Translated
//! from llvm/lib/CodeGen/InlineSpiller.cpp (`spill`, `spillAll`,
//! `spillAroundUses`, `insertReload`, `insertSpill`) at d1106deb71cc.
//!
//! # How it works
//! `spill(reg)` takes every instruction that names `reg` (a parallel copy is
//! one unit). `foldMemoryOperand` (`tii`) puts the stack cell in place of
//! `reg` where the form exists; otherwise a register made from `reg` with
//! `LiveRangeEdit::create_from` replaces it in the unit, with a reload before
//! where the unit reads and a store after where it writes. The unit is
//! rewritten in place (`LiveIntervals::rewrite_instr`: it keeps its slot, so a
//! call keeps its register mask) and the reload and store take new slots
//! between its neighbours (`insert_instr_before/after`): no other register's
//! interval changes. Each register made gets its interval and weight and is
//! returned for the queue; the interval of `reg` is erased.
//!
//! DEVIATE D-A2: no `coalesceStackAccess` (a copy between `reg` and its own
//! slot is not dropped). Exit: translated with the sibling hoist.
//!
//! DEVIATE D-B: no `analyzeSiblingValues`, `hoistAllSpills` or
//! `reMaterializeAll` ahead of the spill: they need SplitKit's sibling
//! relations (step 2). Exit: SplitKit translated.
//!
//! DEVIATE D-G: a spilled phi value is not read from its home cell
//! (`storedhomes`); it is stored and reloaded. Exit: the homes worked out
//! once over the edited function. Costs memory operands where a home held.
//!
//! DEVIATE D-C: a spill slot is a `Frame` cell per original register, shared
//! afterwards by `slots_colored`; LLVM shares through `LiveStacks` as it goes.
//! Exit: `LiveStacks` translated with SplitKit.
//!
//! # Example
//! `%7` read at 48 and written at 64 becomes two registers, `[reload, 48r)` and
//! `[64r, store)`, with a frame cell between them; `%7` has no interval.
//!
//! # Used by
//! `shadow::decide_inline`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::greedy::{SpillContext, Spiller};
use super::liverangeedit::{LiveRangeEdit, NoDelegate};
use super::tii::fold_memory_operand;
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::Frame;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::HashMap;

/// What the allocator keeps per register and the spiller must extend for the
/// registers it makes: the allocation order and the widest access.
#[derive(Default)]
pub struct Tables {
    pub orders: RefCell<HashMap<u32, Vec<RegId>>>,
    pub widths: RefCell<HashMap<u32, u32>>,
    /// The registers whose one definition can be made again where they are
    /// read (`isTriviallyReMaterializable`).
    pub remat: RefCell<crate::support::hash::HashSet<u32>>,
}

// DEVIATE D-I: `isTriviallyReMaterializable` is answered by the spiller's own
// facts (`spiller::remade_values`): a literal, a frame or symbol address, or a
// load of a cell nothing writes while the value lives. The fact: LLVM takes a
// load only when its memory operand is a dereferenceable invariant load; llrm
// proves the same per live range (`_stable_loads_by`), which covers the m16
// descriptor loads (`mov es, [d+2]`). Exit: X86InstrInfo's rules translated
// with an invariant-load flag set from those facts.
/// Whether `one` is a definition `isTriviallyReMaterializable` takes: one
/// register written by a plain move or address, nothing pinned or destroyed.
pub fn remat_shape(one: &Insn) -> bool {
    one.defines.len() == 1
        && one.group.is_none()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.symbol != Some(true)
        && one.what.as_ref().is_some_and(|w| {
            matches!(w.op, Operation::Move | Operation::Address)
                && matches!(
                    w.dests.as_slice(),
                    [Loc::Held(held)] if held.value == one.defines[0]
                )
                && w.sources.len() == 1
        })
}

/// `InlineSpiller`.
pub struct InlineSpiller<'a> {
    template: &'a LirBody,
    frame: &'a mut Frame,
    classes: &'a RegisterClasses,
    order_of: &'a dyn Fn(u32) -> Vec<RegId>,
    tables: Rc<Tables>,
    reloads: usize,
    stores: usize,
    folded: usize,
    failed: bool,
    spilled: Vec<u32>,
}

impl<'a> InlineSpiller<'a> {
    pub fn new(
        template: &'a LirBody,
        frame: &'a mut Frame,
        classes: &'a RegisterClasses,
        order_of: &'a dyn Fn(u32) -> Vec<RegId>,
        tables: Rc<Tables>,
    ) -> Self {
        Self {
            template,
            frame,
            classes,
            order_of,
            tables,
            reloads: 0,
            stores: 0,
            folded: 0,
            failed: false,
            spilled: Vec::new(),
        }
    }

    /// The units of `reg`'s instructions: a parallel copy is one, listed by its
    /// members in block order.
    fn units(
        lis: &LiveIntervals,
        reg: u32,
    ) -> Vec<(usize, Vec<InstrKey>)> {
        let mut seen: crate::support::hash::HashSet<InstrKey> = Default::default();
        let keys: Vec<InstrKey> =
            lis.operands(reg).iter().map(|operand| operand.instr).filter(|key| seen.insert(*key)).collect();
        let mut covered: crate::support::hash::HashSet<InstrKey> = Default::default();
        let mut units: Vec<(usize, Vec<InstrKey>)> = Vec::new();
        for key in keys {
            if covered.contains(&key) {
                continue;
            }
            let index = lis.indexes.get_instruction_index(key);
            let block = lis.indexes.get_mbb_from_index(&index);
            let insns = lis.block_insns(block);
            // The block is in slot order: the run sharing `key`'s index starts
            // here.
            let first = lis.partition_at(block, &index, false);
            let at = first + insns[first..].iter().position(|one| one.id() == key).expect("in its block");
            let members = match insns[at].group {
                None => vec![key],
                Some(group) => {
                    let (mut from, mut to) = (at, at);
                    while from > 0 && insns[from - 1].group == Some(group) {
                        from -= 1;
                    }
                    while to + 1 < insns.len() && insns[to + 1].group == Some(group) {
                        to += 1;
                    }
                    insns[from..=to].iter().map(|one| one.id()).collect()
                }
            };
            covered.extend(members.iter().copied());
            units.push((block, members));
        }
        units
    }

    /// `reMaterializeAll` and `reMaterializeFor` (InlineSpiller.cpp#L586,
    /// #L640): a register defined once by an
    /// instruction `isTriviallyReMaterializable` takes (a literal, an address,
    /// a load of a cell nothing writes while it lives) is made again before
    /// each instruction that reads it, in a new register, when what the
    /// definition reads is still the same there (`allUsesAvailableAt`); the
    /// definition goes (`eliminateDeadDefs`). A plain copy of it takes the
    /// definition in place, the copy's destination written by the literal,
    /// address or load (`foldMemoryOperand` of a COPY with the remat as its
    /// LoadMI). Returns the registers made, or None when the register is not
    /// one of these.
    fn rematerialize_all(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        vrai: &crate::liveness::calcspillweights::VirtRegAuxInfo<'_>,
    ) -> Option<Vec<u32>> {
        if !self.tables.remat.borrow().contains(&reg) {
            return None;
        }
        let mut def: Option<InstrKey> = None;
        for operand in lis.operands(reg) {
            if operand.is_def {
                if def.is_some_and(|seen| seen != operand.instr) {
                    return None;
                }
                def = Some(operand.instr);
            }
        }
        let def = Arc::clone(lis.instr(def?));
        if !remat_shape(&def) || def.uses.contains(&reg) {
            return None;
        }
        let what = def.what.clone()?;
        let Loc::Held(defined) = what.dests[0] else { return None };
        let units: Vec<(usize, Vec<InstrKey>)> =
            Self::units(lis, reg).into_iter().filter(|(_, members)| members != &[def.id()]).collect();
        for (_, members) in &units {
            let first = lis.indexes.get_instruction_index(members[0]);
            if !vrai.all_uses_available_at(def.id(), &first, lis) {
                return None;
            }
        }
        let def_block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(def.id()));
        let mut made = Vec::new();
        for (block, members) in units {
            // The registers a copy's members write: a definition that reads one
            // cannot stand in for a member, which reads before any member
            // writes.
            let written: Vec<u32> = members.iter().flat_map(|key| lis.instr(*key).defines.clone()).collect();
            let in_place: Option<Vec<Arc<Insn>>> = members
                .iter()
                .map(|key| {
                    let one = lis.instr(*key);
                    if !one.uses.contains(&reg) && !one.defines.contains(&reg) {
                        return Some(Arc::clone(one));
                    }
                    let w = one.what.as_ref()?;
                    let plain = w.op == Operation::Move && w.name.as_deref() == Some("mov");
                    let [Loc::Held(dest)] = w.dests.as_slice() else { return None };
                    let ok = plain
                        && one.uses == [reg]
                        && one.defines == [dest.value]
                        && one.requires.is_empty()
                        && one.delivers.is_empty()
                        && one.clobbers.is_empty()
                        && dest.width == defined.width
                        && matches!(
                            w.sources.as_slice(),
                            [Loc::Held(h)] if h.value == reg && h.width == defined.width
                        )
                        // A copy group's members stay moves (`parcopy`).
                        && (one.group.is_none()
                            || what.op == Operation::Move && !def.uses.iter().any(|read| written.contains(read)));
                    ok.then(|| {
                        let mut folded = (**one).clone();
                        folded.what = Some(Semantics { dests: vec![Loc::Held(*dest)], ..what.clone() });
                        folded.uses = def.uses.clone();
                        folded.symbol = def.symbol;
                        Arc::new(folded)
                    })
                })
                .collect();
            if let Some(folded) = in_place {
                for (key, one) in members.iter().zip(folded) {
                    if lis.instr(*key).id() != one.id() {
                        lis.rewrite_instr(block, *key, one);
                    }
                }
                self.folded += 1;
                continue;
            }
            let value = lis.next_virtual_register();
            lis.reserve_virtual_register(value);
            // It stands beside the unit and claims none of the def's bytes,
            // as `spiller::_inserted` places what it makes.
            let beside = lis.instr(members[0]);
            let at = beside.covers.map_or(beside.at, |covers| covers.0);
            let again = Arc::new(Insn {
                at: beside.at,
                covers: Some((at, at)),
                defines: vec![value],
                what: Some(Semantics { dests: vec![Loc::Held(Held { value, width: defined.width })], ..what.clone() }),
                spill_reload: true,
                rematerialized: true,
                ..(*def).clone()
            });
            // A parallel copy reads before it writes: the definition goes
            // before its first member, whichever member reads it.
            let mut first = None;
            for key in &members {
                let renamed = lis.instr(*key).renamed(&|name| if name == reg { value } else { name });
                first.get_or_insert(renamed.id());
                lis.rewrite_instr(block, *key, renamed);
            }
            lis.insert_instr_before(block, first, again);
            self.reloads += 1;
            let order = self.tables.orders.borrow().get(&reg).cloned().unwrap_or_default();
            self.tables.orders.borrow_mut().insert(value, order);
            let width = self.tables.widths.borrow().get(&reg).copied().unwrap_or(0);
            self.tables.widths.borrow_mut().insert(value, width);
            self.tables.remat.borrow_mut().insert(value);
            made.push(value);
        }
        lis.remove_instr(def_block, def.id());
        Some(made)
    }

    /// A parallel copy that reads the register is a load: `mov %d, %reg` as a
    /// member becomes `mov %d, [slot]`, and no register holds the value. A
    /// reload before the copy would be live to it, together with the reloads
    /// of every other member's source. Returns Some(no registers) when every
    /// naming of `reg` in the unit is such a member, None otherwise.
    ///
    /// DEVIATE D-F: a parallel copy is an instruction LLVM does not have (its
    /// PHI copies are sequential); this is `foldMemoryOperand` of a COPY, into
    /// a bundle. Exit: parallel copies sequenced before allocation (the
    /// coalescer's plan), then deleted with this function.
    fn fold_into_copy(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        block: usize,
        members: &[InstrKey],
        old: &[Arc<Insn>],
    ) -> Option<Vec<u32>> {
        if old.iter().any(|one| one.group.is_none()) {
            return None;
        }
        let mut changed: Vec<Arc<Insn>> = Vec::with_capacity(old.len());
        for one in old {
            let what = one.what.as_ref()?;
            let reads = one.uses.contains(&reg);
            if !reads && !one.defines.contains(&reg) {
                changed.push(Arc::clone(one));
                continue;
            }
            let plain = what.op == Operation::Move && what.name.as_deref() == Some("mov");
            let [Loc::Held(source)] = what.sources.as_slice() else { return None };
            if !plain || source.value != reg || one.defines.contains(&reg) {
                return None;
            }
            let cell = self.frame.cell(reg, source.width).ok()?;
            let mut folded = (**one).clone();
            folded.what = Some(Semantics { sources: vec![Loc::Mem(cell)], ..what.clone() });
            folded.uses.retain(|value| *value != reg);
            folded.symbol = Some(false);
            changed.push(Arc::new(folded));
        }
        for (key, one) in members.iter().zip(changed) {
            if lis.instr(*key).id() != one.id() {
                lis.rewrite_instr(block, *key, one);
            }
        }
        self.folded += 1;
        Some(Vec::new())
    }

    /// `spillAroundUses` for one unit (an instruction, or a parallel copy):
    /// `foldMemoryOperand`, else a register made from `reg` with a reload
    /// before where the unit reads and a spill after where it writes. Only
    /// `reg`'s operands change, so no other interval moves. Returns the
    /// registers it made.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/InlineSpiller.cpp#L1315>
    fn spill_unit(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        block: usize,
        members: &[InstrKey],
    ) -> Option<Vec<u32>> {
        let old: Vec<Arc<Insn>> = members.iter().map(|key| Arc::clone(lis.instr(*key))).collect();
        // An x87 value (FP0..FP6) spills in floatassign's format, `fstp` and
        // `fld` of a qword cell, and folds into nothing: a copy folded to a
        // store would be a `mov` the stackifier has no rule for.
        // A piece made by an edit has no order of its own yet: its x87 width
        // says what it is.
        let float = self.tables.orders.borrow().get(&reg).is_some_and(|order| order.first() == Some(&RegId::MM0))
            || crate::backend::allocate::rfp_on() && widest_operand(&old, reg) == Some(10);
        if float {
            return self.spill_float_unit(lis, reg, block, members, &old);
        }
        if let Some(folded) = self.fold_into_copy(lis, reg, block, members, &old) {
            return Some(folded);
        }
        let width = self.tables.widths.borrow().get(&reg).copied().unwrap_or(0);
        let width = if width > 0 { width } else { widest_operand(&old, reg)? };
        let cell = self.frame.cell(reg, width).ok()?;
        let names = |one: &Arc<Insn>| one.uses.contains(&reg) || one.defines.contains(&reg);
        let folded: Option<Vec<Arc<Insn>>> = old
            .iter()
            .map(|one| {
                if !names(one) {
                    Some(Arc::clone(one))
                } else if one.group.is_some() {
                    // A parallel copy's member folds to memory on either side,
                    // memory to memory included: `parcopy` sequences it.
                    let values = std::iter::once(reg).collect();
                    crate::backend::spiller::_in_place(one, &values, self.frame).ok().flatten()
                } else {
                    fold_memory_operand(one, reg, &cell, self.template.bits, self.classes)
                }
            })
            .collect();
        if let Some(folded) = folded {
            for (key, one) in members.iter().zip(folded) {
                lis.rewrite_instr(block, *key, one);
            }
            self.folded += 1;
            return Some(Vec::new());
        }
        let new = lis.next_virtual_register();
        lis.reserve_virtual_register(new);
        // A write of part of the register keeps the rest: it reads it too (a
        // sub-register def without `undef`).
        let partial = old.iter().any(|one| written_narrower(one, reg, width));
        let reads = partial || old.iter().any(|one| one.uses.contains(&reg));
        let writes = old.iter().any(|one| one.defines.contains(&reg));
        let renamed: Vec<Arc<Insn>> =
            old.iter().map(|one| one.renamed(&|value| if value == reg { new } else { value })).collect();
        for (key, one) in members.iter().zip(&renamed) {
            lis.rewrite_instr(block, *key, Arc::clone(one));
        }
        if reads {
            let reload = crate::backend::spiller::_reload(&old[0], new, &cell);
            lis.insert_instr_before(block, Some(renamed[0].id()), reload);
            self.reloads += 1;
        }
        if writes {
            let last = renamed.last().expect("a unit has members");
            let store = crate::backend::spiller::_store(&old[old.len() - 1], new, &cell);
            lis.insert_instr_after(block, last.id(), store);
            self.stores += 1;
        }
        // A register an instruction requires or delivers in one register can be
        // given that one only.
        let pinned = renamed
            .iter()
            .flat_map(|one| one.requires.iter().chain(&one.delivers))
            .find(|(held, _)| held.value == new)
            .map(|(_, register)| crate::model::ir::root(*register));
        // The class of the value it stands for, which a split may have changed.
        let held = self.tables.orders.borrow().get(&reg).cloned();
        let mut order: Vec<RegId> = held.unwrap_or_else(|| {
            let mut order: Vec<RegId> = (self.order_of)(reg).into_iter().map(crate::model::ir::root).collect();
            order.dedup();
            order
        });
        if let Some(only) = pinned {
            order = vec![only];
        }
        self.tables.orders.borrow_mut().insert(new, order);
        self.tables.widths.borrow_mut().insert(new, width);
        NATIVE_UNITS.with(|one| one.set(one.get() + 1));
        Some(vec![new])
    }
}

impl InlineSpiller<'_> {
    /// `spill_unit` for an x87 value: a register made from `reg`, an `fld`
    /// of its cell before where the unit reads it and an `fstp` after where it
    /// writes it. DEVIATE D-RFP1: the cell is a qword, floatassign's spill
    /// format, not the class width (`RFP80` would be m80). Exit: the class
    /// width once floatassign is gone and the rows are compared.
    fn spill_float_unit(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        block: usize,
        members: &[InstrKey],
        old: &[Arc<Insn>],
    ) -> Option<Vec<u32>> {
        let cell = self.frame.cell(reg, 8).ok()?;
        let new = lis.next_virtual_register();
        lis.reserve_virtual_register(new);
        let reads = old.iter().any(|one| one.uses.contains(&reg));
        let writes = old.iter().any(|one| one.defines.contains(&reg));
        let renamed: Vec<Arc<Insn>> =
            old.iter().map(|one| one.renamed(&|value| if value == reg { new } else { value })).collect();
        for (key, one) in members.iter().zip(&renamed) {
            lis.rewrite_instr(block, *key, Arc::clone(one));
        }
        let beside = |one: &Insn| -> (i64, Option<(i64, i64)>) {
            let at = one.covers.map_or(one.at, |covers| covers.0);
            (one.at, Some((at, at)))
        };
        if reads {
            let mut reload = (*crate::backend::floatassign::_spill_home(new, cell.clone())).clone();
            (reload.at, reload.covers) = beside(&old[0]);
            reload.spill_reload = true;
            lis.insert_instr_before(block, Some(renamed[0].id()), Arc::new(reload));
            self.reloads += 1;
        }
        if writes {
            let last = &old[old.len() - 1];
            let (at, _) = beside(last);
            let store = crate::backend::floatassign::_stored(new, &cell, at);
            lis.insert_instr_after(block, renamed[renamed.len() - 1].id(), store);
            self.stores += 1;
        }
        self.tables.orders.borrow_mut().insert(new, crate::backend::allocate::RFP.to_vec());
        self.tables.widths.borrow_mut().insert(new, 10);
        NATIVE_UNITS.with(|one| one.set(one.get() + 1));
        Some(vec![new])
    }
}

/// The widest operand naming `reg` in `unit`.
fn widest_operand(
    unit: &[Arc<Insn>],
    reg: u32,
) -> Option<u32> {
    unit.iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .flat_map(crate::model::ir::values)
        .filter(|held| held.value == reg)
        .map(|held| held.width)
        .max()
}

/// Whether `one` writes only part of `reg` (its def narrower than the cell).
fn written_narrower(
    one: &Insn,
    reg: u32,
    width: u32,
) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| what.dests
                .iter()
                .any(
                    |loc| matches!(
                        loc,
                        Loc::Held(held) if held.value == reg && held.width < width
                    ),
                ),
        )
}

// Registers a spill worked out the interval of without making them.
#[cfg(test)]
thread_local! {
    static STRAY: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

thread_local! {
    static NATIVE_UNITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Units spilled by `spillAroundUses`'s reload and spill on this thread, for
/// a test that spills write no plan of the old spiller.
pub fn native_units() -> usize {
    NATIVE_UNITS.with(std::cell::Cell::get)
}

impl Spiller for InlineSpiller<'_> {
    fn spill(
        &mut self,
        ctx: SpillContext<'_, '_>,
        reg: u32,
        new_vregs: &mut Vec<u32>,
    ) {
        let SpillContext { lis, vrm, vrai } = ctx;
        self.spilled.push(reg);
        #[cfg(test)]
        crate::liveness::liveintervals::COMPUTED.with(|one| one.borrow_mut().clear());
        let first_new = new_vregs.len();
        // Bisection aid: only the first N spills are written inline.
        if std::env::var("LLRM_GREEDY_SPILLS")
            .ok()
            .and_then(|n| n.parse::<usize>().ok())
            .is_some_and(|n| self.spilled.len() > n)
        {
            self.failed = true;
            return;
        }
        // The cell is as wide as the widest access anywhere in the function: a
        // unit asks for the width of its own accesses only, and a narrow cell
        // made by the first would be written whole by a wider one.
        let mut delegate = NoDelegate;
        let mut edit = LiveRangeEdit::new(reg, &mut delegate);
        if let Some(made) = self.rematerialize_all(lis, reg, vrai) {
            for value in made {
                edit.create_from(lis, vrm, value, reg);
                let mut li = lis.compute_virt_reg_interval(value);
                vrai.calculate_spill_weight_and_hint(&mut li, lis);
                lis.insert_interval(li);
                new_vregs.push(value);
            }
            edit.erase_virt_reg(lis, reg);
            return;
        }
        if let Some(width) = self.tables.widths.borrow().get(&reg).copied().filter(|width| *width > 0) {
            if self.frame.cell(reg, width).is_err() {
                self.failed = true;
                return;
            }
        }
        for (block, members) in Self::units(lis, reg) {
            let Some(fresh) = self.spill_unit(lis, reg, block, &members) else {
                self.failed = true;
                return;
            };
            for value in fresh {
                edit.create_from(lis, vrm, value, reg);
                let mut li = lis.compute_virt_reg_interval(value);
                vrai.calculate_spill_weight_and_hint(&mut li, lis);
                lis.insert_interval(li);
                new_vregs.push(value);
            }
        }
        edit.erase_virt_reg(lis, reg);
        #[cfg(test)]
        {
            let made = &new_vregs[first_new..];
            let computed = crate::liveness::liveintervals::COMPUTED.with(|one| std::mem::take(&mut *one.borrow_mut()));
            STRAY.with(|stray| stray.borrow_mut().extend(computed.into_iter().filter(|value| !made.contains(value))));
        }
        let _ = first_new;
    }

    fn counts(&self) -> (usize, usize, usize) {
        (self.reloads, self.stores, self.folded)
    }

    fn failed(&self) -> bool {
        self.failed
    }

    fn recorded(&self) -> Vec<u32> {
        self.spilled.clone()
    }
}

#[cfg(test)]
mod tests {
    /// A spill wrote each unit through the old spiller's plan on a body of
    /// that unit (D-A), then worked out again the interval of every other
    /// register the unit named (`refresh_named`: 27 M of d_faces -O2 m16's
    /// 73 M in `spill`). `spillAroundUses` touches only the spilled register:
    /// no plan, and only the registers it makes get intervals.
    #[test]
    fn test_a_spill_writes_no_plan_and_works_out_only_the_registers_it_makes() {
        let plans = crate::backend::spiller::PLANS.with(std::cell::Cell::get);
        let units = super::native_units();
        crate::backend::regalloc_fuzz_tests::allocate_generated_bodies(40);
        assert!(super::native_units() > units, "premise: some unit was spilled");
        assert_eq!(crate::backend::spiller::PLANS.with(std::cell::Cell::get) - plans, 0, "plans of the old spiller");
        let stray = super::STRAY.with(|one| std::mem::take(&mut *one.borrow_mut()));
        assert!(stray.is_empty(), "intervals worked out of registers the spills did not make: {stray:?}");
    }
}
