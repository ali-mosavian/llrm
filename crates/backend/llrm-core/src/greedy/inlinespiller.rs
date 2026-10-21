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
//! DEVIATE D-B: no snippets (`collectRegsToSpill`: the tiny copies to a
//! sibling are not spilled with the register) and no
//! `hoistAllSpills` (`HoistSpillHelper`: spills of one value in several blocks
//! are not merged up the dominator tree). `hoistSpillInsideBB`,
//! `eliminateRedundantSpills` and `coalesceStackAccess` are translated, and
//! `StackSlotColoring::RemoveDeadStores` in `spiller::_identities_dropped`.
//! Exit: `hoistAllSpills` and the snippets translated.
//!
//! DEVIATE D-G: a spilled phi value is not read from its home cell
//! (`storedhomes`); it is stored and reloaded. Exit: the homes worked out
//! once over the edited function. Costs memory operands where a home held.
//!
//! DEVIATE D-C: a spill slot is a `Frame` cell per spilled register (one for
//! the siblings of an original, D-F2), shared
//! afterwards by `slots_colored`, over the homes' ranges this spiller merges
//! as it spills (`LiveStacks`, `Frame::stacks`). LLVM's `StackSlotColoring`
//! gives a slot its weight and order from the same ranges; ours keeps the
//! colouring `slots_colored` has (copy partners, loop widths, four orders).
//! Exit: those rules rewritten as `StackSlotColoring`'s `ColorSlot`.
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
use super::matrix::LiveRegMatrix;
use super::tii::fold_memory_operand;
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::{Frame, SlotKey};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::{LiveRange, Vn};
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
    /// The bytes of each x87 value's float type: its spill cell's width.
    pub float_bytes: RefCell<HashMap<u32, u32>>,
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
//
// DEVIATE D-RFP4 (w-4, 2026-10-30): an x87 load (`fld`) of a global or of an
// incoming argument's cell, which nothing writes while the value lives, is made
// again where it is read. The fact: X86InstrInfo takes a load only when it is
// invariant (an immutable stack argument is; the integer loads of D-I are the
// same proof, `_stable_loads_from`), but the x87 loads were never offered to
// it, so the RFP route stored and reloaded them: QCport d_alias -Os 520 fld,
// 461 fstp, 161 fxch against 462, 392, 144 with it. A local cell is left out
// (the cell `fild` reads is written again between two loads). Exit: retire
// when the pin's foldMemoryOperand/rematerialisation for x87 reaches the same
// bytes, or with D-I.

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
            matches!(
                w.op,
                Operation::Move | Operation::Address | Operation::FloatLoad
            )
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
    /// `Original` (InlineSpiller.cpp#L1460): the register every sibling of the
    /// one being spilled descends from, whose home they all share.
    original: u32,
    /// `RegsToSpill`: the register being spilled.
    reg: u32,
    /// The register that names the home of the one being spilled.
    ///
    /// DEVIATE D-F2: LLVM shares one slot among all descendants of `Original`,
    /// because siblings hold one value wherever they are live together. A
    /// parallel copy gives the old and the new value of one register one index,
    /// where siblings hold two values, so a register whose value is live where
    /// a sibling's is in the home, or meets it at a parallel copy, has a home
    /// of its own. Exit: parallel copies sequenced before allocation (D-F).
    key: u32,
    /// The ranges spilled to each home, by the register that names it.
    homed: HashMap<u32, Vec<(u32, LiveRange)>>,
    /// The register that names the home of each register spilled.
    keys: HashMap<u32, u32>,
    /// `DeadDefs`: the stores and copies a hoist made dead.
    dead: Vec<InstrKey>,
    /// `LiveStacks`: the range of each home, the segments of what was spilled
    /// to it merged as one value.
    stacks: crate::support::hash::HashMap<i64, LiveRange>,
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
            original: 0,
            reg: 0,
            key: 0,
            homed: Default::default(),
            keys: Default::default(),
            dead: Vec::new(),
            stacks: Default::default(),
        }
    }

    /// `StackInt->MergeSegmentsInAsValue(OrigLI, ...)` (InlineSpiller.cpp
    /// `spillAll`): `range`, the range of the register spilled, joins the
    /// range of its home.
    fn merged(
        &mut self,
        range: &LiveRange,
    ) {
        let Some(&home) = self.frame.slots.get(&SlotKey::from(self.key)) else { return };
        let stack = self.stacks.entry(home).or_insert_with(LiveRange::new);
        let value = if stack.get_num_val_nums() > 0 {
            stack.get_val_num_info(0)
        } else if let Some(first) = range.segments.first() {
            stack.get_next_value(first.start.clone())
        } else {
            return;
        };
        stack.merge_segments_in_as_value(range, &value);
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
            let bytes = self.tables.float_bytes.borrow().get(&reg).copied();
            if let Some(bytes) = bytes {
                self.tables.float_bytes.borrow_mut().insert(value, bytes);
            }
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
            let cell = self.frame.cell(self.key, source.width).ok()?;
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
        if self.is_float(reg, &old) {
            return self.spill_float_unit(lis, reg, block, members, &old);
        }
        if let Some(folded) = self.fold_into_copy(lis, reg, block, members, &old) {
            return Some(folded);
        }
        let width = self.tables.widths.borrow().get(&reg).copied().unwrap_or(0);
        let width = if width > 0 { width } else { widest_operand(&old, reg)? };
        let cell = self.frame.cell(self.key, width).ok()?;
        let home_key = self.key;
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
                    crate::backend::spiller::_in_place(one, &values, self.frame, &|value| {
                        if value == reg { home_key } else { value }
                    })
                    .ok()
                    .flatten()
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
    /// writes it. DEVIATE D-RFP1: the cell is the value's float type
    /// (4, 8 or 10 bytes, `float_precisions`), as the pin's RFP32/RFP64/RFP80
    /// spill rounds, not one class width, where the language follows the pin
    /// (`Profile::rounds_float_spills`, llrm-c); Nib and BASIC keep the qword,
    /// as only a store the program writes rounds there (stft_single). Exit: the
    /// class width once the classes are split.
    ///
    /// OPEN D-RFP3 (RFP-5, owner w-1, 2026-10-31): where the qword is kept (Nib
    /// fast), a promoted float phi lives unrounded in a register or cell next
    /// to the f32 memory copy a store wrote, so which one a read sees
    /// follows the spill choice (nbody_single.bas printed 86015 or 86016
    /// across unrelated commits; BASIC now rounds each assignment, Nib has
    /// no row that shows it). Exit: a Nib row that fails, then rounding
    /// where promotion splits a copy.
    ///
    /// DEVIATE D-RFP2 (#TBD, RFP-4): an arithmetic instruction that only reads
    /// the value reads the cell as its memory operand (`fadd qword ptr [m]`),
    /// with no `fld`. Fact: X86GenFoldTables.inc has no x87 arithmetic entry,
    /// so LLVM reloads; floatassign folded, and nbody -O3 ran 36% more
    /// instructions without it. Exit: remove if a clang-parity row shows it
    /// unnecessary, or when the pin gains the fold.
    fn spill_float_unit(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        block: usize,
        members: &[InstrKey],
        old: &[Arc<Insn>],
    ) -> Option<Vec<u32>> {
        let bytes = self.tables.float_bytes.borrow().get(&reg).copied().unwrap_or(8);
        let cell = self.frame.cell(self.key, bytes).ok()?;
        if let [one] = old
            && one.group.is_none()
            && !one.defines.contains(&reg)
            && let Some(folded) = crate::backend::floatfold::read_from_cell(one, reg, &cell)
        {
            lis.rewrite_instr(block, members[0], Arc::new(folded));
            self.folded += 1;
            return Some(Vec::new());
        }
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
        self.tables.float_bytes.borrow_mut().insert(new, bytes);
        NATIVE_UNITS.with(|one| one.set(one.get() + 1));
        Some(vec![new])
    }
}

/// The bytes of the float type of every x87 value of `body` that selection's
/// numbering, or a copy or phi of such a value, gives: the type's own where
/// a value is the program's, else that of the values it is copied from or
/// to. A web joined to values of two types has the wider.
pub fn float_precisions(body: &LirBody) -> HashMap<u32, u32> {
    // No width table: the target does not round a spill
    // (`rounds_float_spills`).
    if body.float_bytes.is_empty() {
        return HashMap::default();
    }
    fn root(
        parent: &mut HashMap<u32, u32>,
        value: u32,
    ) -> u32 {
        let mut at = value;
        while let Some(&up) = parent.get(&at) {
            if up == at {
                break;
            }
            at = up;
        }
        let top = at;
        let mut at = value;
        while let Some(&up) = parent.get(&at) {
            if up == at {
                break;
            }
            parent.insert(at, top);
            at = up;
        }
        parent.entry(top).or_insert(top);
        top
    }
    let mut parent: HashMap<u32, u32> = HashMap::default();
    let join = |parent: &mut HashMap<u32, u32>, one: u32, other: u32| {
        let (one, other) = (root(parent, one), root(parent, other));
        parent.insert(one, other);
    };
    let floating = |loc: &Loc| match loc {
        Loc::Held(held) if held.width == 10 => Some(held.value),
        _ => None,
    };
    for block in &body.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                join(&mut parent, phi.result, *value);
            }
        }
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            if what.op == Operation::Move
                && let ([dest], [source]) = (what.dests.as_slice(), what.sources.as_slice())
                && let (Some(dest), Some(source)) = (floating(dest), floating(source))
            {
                join(&mut parent, dest, source);
            }
        }
    }
    let mut bytes: HashMap<u32, u32> = HashMap::default();
    for (&value, &size) in body.float_bytes.iter() {
        let top = root(&mut parent, value);
        let widest = bytes.entry(top).or_insert(0);
        *widest = (*widest).max(size);
    }
    // A value a pass made after selection is in no table: it is as wide as
    // what it is computed from, down to the memory a `fld` reads.
    loop {
        let mut changed = false;
        for block in &body.blocks {
            for one in &block.insns {
                let Some(what) = &one.what else { continue };
                // `fild` reads an integer: its width says nothing of the float.
                if what.name.as_deref().is_some_and(|name| name.starts_with("fild"))
                    || !matches!(
                        what.op,
                        Operation::FloatLoad | Operation::FloatArith | Operation::FloatUnary
                    )
                {
                    continue;
                }
                let Some(Loc::Held(dest)) = what.dests.first().filter(|dest| floating(dest).is_some()) else {
                    continue;
                };
                let mut known = 0;
                for source in &what.sources {
                    known = known.max(match source {
                        Loc::Held(held) if held.width == 10 => {
                            let top = root(&mut parent, held.value);
                            bytes.get(&top).copied().unwrap_or(0)
                        }
                        Loc::Mem(cell) if matches!(cell.width, 4 | 8 | 10) => cell.width,
                        _ => 0,
                    });
                }
                let top = root(&mut parent, dest.value);
                let widest = bytes.entry(top).or_insert(0);
                if known > *widest {
                    *widest = known;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let members: Vec<u32> = parent.keys().copied().collect();
    members
        .into_iter()
        .filter_map(|value| {
            let top = root(&mut parent, value);
            bytes.get(&top).filter(|&&size| size > 0).map(|&size| (value, size))
        })
        .collect()
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

impl InlineSpiller<'_> {
    /// The register that names the home `reg` is spilled to: the original's
    /// (or the home a hoist of its value was stored in), unless a value of that
    /// home is live where `range` is or meets it at a parallel copy; else its
    /// own.
    fn shared_key(
        &self,
        lis: &LiveIntervals,
        range: Option<&LiveRange>,
    ) -> u32 {
        let reg = self.reg;
        let key = self.keys.get(&reg).copied().unwrap_or(self.original);
        let clear = range.is_none_or(|range| {
            self.homed.get(&key).is_none_or(|ranges| {
                !ranges
                    .iter()
                    .any(|(owner, one)| *owner != reg && (one.overlaps(range) || Self::meet_at_group(lis, one, range)))
            })
        });
        if clear { key } else { reg }
    }

    /// Whether a value of `a` ends where one of `b` begins (or the reverse) at
    /// a parallel copy: the copy reads the old value and writes the new one
    /// at one index, which one cell cannot hold in turn.
    fn meet_at_group(
        lis: &LiveIntervals,
        a: &LiveRange,
        b: &LiveRange,
    ) -> bool {
        let meets = |from: &LiveRange, to: &LiveRange| {
            from.segments
                .iter()
                .any(
                    |end| to.segments
                        .iter()
                        .any(
                            |start| end.end == start.start
                                && lis
                                    .indexes
                                    .get_instruction_from_index(&end.end)
                                    .is_some_and(|key| lis.instr(key).group.is_some()),
                        ),
                )
        };
        meets(a, b) || meets(b, a)
    }

    /// Whether `reg` is an x87 value, from its class or, for a piece made by
    /// an edit that has no order of its own yet, from its width.
    fn is_float(
        &self,
        reg: u32,
        insns: &[Arc<Insn>],
    ) -> bool {
        self.tables.orders.borrow().get(&reg).is_some_and(|order| order.first() == Some(&RegId::MM0))
            || crate::backend::allocate::rfp_on() && widest_operand(insns, reg) == Some(10)
    }

    /// The cell of the home all siblings share, as wide as `reg`'s accesses.
    fn home_cell(
        &mut self,
        reg: u32,
        def: &Arc<Insn>,
    ) -> Option<(crate::model::ir::Mem, bool)> {
        let float = self.is_float(reg, std::slice::from_ref(def));
        let width = if float {
            self.tables.float_bytes.borrow().get(&reg).copied().unwrap_or(8)
        } else {
            let known = self.tables.widths.borrow().get(&reg).copied().filter(|width| *width > 0);
            known.or_else(|| widest_operand(std::slice::from_ref(def), reg))?
        };
        Some((self.frame.cell(self.key, width).ok()?, float))
    }

    /// `coalesceStackAccess` (InlineSpiller.cpp#L936): `key` is a load of the
    /// register being spilled from the home, or a store of it to the home.
    /// The home holds the value already, so the access goes.
    fn coalesce_stack_access(
        &mut self,
        lis: &mut LiveIntervals,
        key: InstrKey,
        block: usize,
    ) -> bool {
        let Some(&home) = self.frame.slots.get(&SlotKey::from(self.key)) else { return false };
        let one = lis.instr(key);
        let Some(what) = one.what.as_ref() else { return false };
        let in_home = |place: &Loc| {
            matches!(
                place,
                Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.slot_home() == Some(home))
            )
        };
        let access = match (what.dests.as_slice(), what.sources.as_slice()) {
            ([Loc::Held(held)], [from]) if held.value == self.reg && in_home(from) && one.uses.is_empty() => true,
            ([into], [Loc::Held(held)]) if held.value == self.reg && in_home(into) && one.defines.is_empty() => true,
            _ => false,
        };
        if access {
            lis.remove_instr(block, key);
        }
        access
    }

    /// `spillAroundUses`' sibling copy cases (InlineSpiller.cpp#L1315): `key`
    /// is a copy between the register being spilled and a sibling. When it
    /// writes the register, `hoistSpillInsideBB` stores the sibling's value
    /// at its own definition and the copy goes (true). When it reads the
    /// register, the sibling's value is read from the home, so its later
    /// stores are redundant (`eliminateRedundantSpills`); the copy is then
    /// spilled as any read (false).
    fn sibling_copy(
        &mut self,
        lis: &mut LiveIntervals,
        matrix: &mut LiveRegMatrix,
        key: InstrKey,
        block: usize,
    ) -> bool {
        let one = Arc::clone(lis.instr(key));
        if one.group.is_some() {
            return false;
        }
        let Some((dst, src)) = crate::backend::coalesce::_copy(&one) else { return false };
        // A copy of the program owns bytes of the object that its erasure
        // would leave unclaimed.
        let owns = one.covers.is_some_and(|(from, to)| from != to);
        let sibling = |reg: u32| reg != self.reg && matrix.vrm.get_original(reg) == self.original;
        let idx = lis.indexes.get_instruction_index(key);
        if dst == self.reg && sibling(src) && !owns {
            return self.hoist_spill_inside_bb(lis, matrix, key, src, block);
        }
        if src == self.reg && sibling(dst) && lis.has_interval(dst) {
            // This is a reload for a sib-reg copy. Drop spills downstream.
            self.keys.entry(dst).or_insert(self.key);
            if let Some(vni) = lis.get_interval(dst).range.get_vn_info_at(&idx.get_reg_slot(false)) {
                self.eliminate_redundant_spills(lis, matrix, dst, vni);
            }
        }
        false
    }

    /// `hoistSpillInsideBB` (InlineSpiller.cpp#L441): `copy` writes the
    /// register being spilled from `src`, which dies there and was defined in
    /// the same block. The spill of the copy's value is `src` stored right
    /// after its definition, which eases the register pressure between the
    /// two. The copy is dead: the value is already in the home.
    fn hoist_spill_inside_bb(
        &mut self,
        lis: &mut LiveIntervals,
        matrix: &mut LiveRegMatrix,
        copy: InstrKey,
        src: u32,
        block: usize,
    ) -> bool {
        let idx = lis.indexes.get_instruction_index(copy);
        if !lis.has_interval(src) || !lis.has_interval(self.original) {
            return false;
        }
        let (src_vni, killed) = {
            let range = &lis.get_interval(src).range;
            let Some(vni) = range.get_vn_info_at(&idx) else { return false };
            (vni, range.query(&idx).is_kill())
        };
        let def_block = lis.indexes.get_mbb_from_index(&src_vni.def());
        // The value of a phi is stored after the phis, a place this spiller
        // has no instruction for.
        if def_block != block || !killed || src_vni.is_phi_def() {
            return false;
        }
        let def_key = lis.defining_instr(&src_vni.def(), src);
        let def = Arc::clone(lis.instr(def_key));
        if def.group.is_some() {
            return false;
        }
        // The home must hold no other value where the source's is stored.
        let mut stored = LiveRange::new();
        let value = stored.get_next_value(src_vni.def());
        stored.merge_value_in_as_value(&lis.get_interval(src).range, &src_vni, &value);
        if self
            .homed
            .get(&self.key)
            .is_some_and(|ranges| ranges.iter().any(|(owner, one)| *owner != src && one.overlaps(&stored)))
        {
            return false;
        }
        let Some((cell, float)) = self.home_cell(src, &def) else { return false };
        // We are going to spill SrcVNI immediately after its def, so clear out
        // any later spills of the same value.
        self.eliminate_redundant_spills(lis, matrix, src, Rc::clone(&src_vni));
        // Insert spill without kill flag immediately after def.
        let store = if float {
            let at = def.covers.map_or(def.at, |covers| covers.0);
            crate::backend::floatassign::_stored(src, &cell, at)
        } else {
            crate::backend::spiller::_store(&def, src, &cell)
        };
        lis.insert_instr_after(block, def_key, store);
        self.stores += 1;
        // `src` is stored in this home: it is spilled to it when it comes to
        // be.
        self.keys.entry(src).or_insert(self.key);
        self.homed.entry(self.key).or_default().push((src, stored));
        // This COPY is now dead, the value is already in the stack slot.
        self.dead.push(copy);
        true
    }

    /// `eliminateRedundantSpills` (InlineSpiller.cpp#L528): `reg`'s value
    /// `vni` is known to be on the stack. Remove any redundant spills of this
    /// value in `reg` and in sibling copies.
    fn eliminate_redundant_spills(
        &mut self,
        lis: &LiveIntervals,
        matrix: &LiveRegMatrix,
        reg: u32,
        vni: Vn,
    ) {
        let mut work: Vec<(u32, Vn)> = vec![(reg, vni)];
        let mut seen: crate::support::hash::HashSet<u32> = Default::default();
        while let Some((reg, vni)) = work.pop() {
            // Regs to spill are taken care of.
            if reg == self.reg || !seen.insert(reg) || !lis.has_interval(reg) {
                continue;
            }
            let li = lis.get_interval(reg);
            // Add all of VNI's live range to StackInt.
            self.merge_value(&li.range, &vni);
            // Find all spills and copies of VNI.
            let mut keys: Vec<InstrKey> = Vec::new();
            for operand in lis.operands(reg) {
                if operand.reads_reg && !keys.contains(&operand.instr) {
                    keys.push(operand.instr);
                }
            }
            for key in keys {
                let one = lis.instr(key);
                let idx = lis.indexes.get_instruction_index(key);
                if !li.range.get_vn_info_at(&idx).is_some_and(|at| Rc::ptr_eq(&at, &vni)) {
                    continue;
                }
                // Follow sibling copies down the dominator tree.
                if let Some((dst, from)) = crate::backend::coalesce::_copy(one).filter(|_| one.group.is_none()) {
                    if from == reg && matrix.vrm.get_original(dst) == self.original && lis.has_interval(dst) {
                        if let Some(dst_vni) = lis.get_interval(dst).range.get_vn_info_at(&idx.get_reg_slot(false)) {
                            work.push((dst, dst_vni));
                        }
                    }
                    continue;
                }
                // Erase spills.
                if one.spill_store && one.uses == [reg] && self.stores_home(one) {
                    self.dead.push(key);
                }
            }
        }
    }

    /// Whether `one` stores to the home the siblings share.
    fn stores_home(
        &self,
        one: &Insn,
    ) -> bool {
        let Some(&home) = self.frame.slots.get(&SlotKey::from(self.key)) else { return false };
        one.what
            .as_ref()
            .is_some_and(
                |what| matches!(
                    what.dests.as_slice(),
                    [Loc::Mem(cell)] if cell.addr.is_some_and(|addr| addr.slot_home() == Some(home))
                ),
            )
    }

    /// `StackInt->MergeValueInAsValue(range, vni, StackInt->getValNumInfo(0))`.
    fn merge_value(
        &mut self,
        range: &LiveRange,
        vni: &Vn,
    ) {
        let Some(&home) = self.frame.slots.get(&SlotKey::from(self.key)) else { return };
        let stack = self.stacks.entry(home).or_insert_with(LiveRange::new);
        let value =
            if stack.get_num_val_nums() > 0 { stack.get_val_num_info(0) } else { stack.get_next_value(vni.def()) };
        stack.merge_value_in_as_value(range, vni, &value);
    }

    /// `eliminateDeadDefs` over `DeadDefs`: the stores and copies go, and the
    /// ranges of what they read are cut back to their other reads. An
    /// assigned register that is cut back is unassigned first and queued
    /// again (`LRE_WillShrinkVirtReg`).
    fn eliminate_dead_defs(
        &mut self,
        lis: &mut LiveIntervals,
        matrix: &mut LiveRegMatrix,
        shrunk: &mut Vec<u32>,
    ) {
        let mut dead = std::mem::take(&mut self.dead);
        dead.sort_unstable();
        dead.dedup();
        while !dead.is_empty() {
            let round = std::mem::take(&mut dead);
            for key in &round {
                for &read in &lis.instr(*key).uses {
                    if read != self.reg && lis.has_interval(read) && matrix.vrm.has_phys(read) {
                        matrix.unassign(lis.get_interval(read));
                        shrunk.push(read);
                    }
                }
            }
            for def in lis.erase_dead_defs(&round) {
                let one = lis.instr(def);
                // An instruction of the program owns bytes of the object that
                // an erased one would leave unclaimed.
                if one.covers.is_none_or(|(from, to)| from == to)
                    && (crate::backend::coalesce::_copy(one).is_some() || remat_shape(one))
                {
                    dead.push(def);
                }
            }
        }
    }
}

impl Spiller for InlineSpiller<'_> {
    fn spill(
        &mut self,
        ctx: SpillContext<'_, '_>,
        reg: u32,
        new_vregs: &mut Vec<u32>,
    ) {
        let SpillContext { lis, matrix, vrai, shrunk } = ctx;
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
        // Share a stack slot among all descendants of Original.
        self.original = matrix.vrm.get_original(reg);
        self.reg = reg;
        // The cell is as wide as the widest access anywhere in the function: a
        // unit asks for the width of its own accesses only, and a narrow cell
        // made by the first would be written whole by a wider one.
        let mut delegate = NoDelegate;
        let mut edit = LiveRangeEdit::new(reg, &mut delegate);
        let range = lis.has_interval(reg).then(|| lis.get_interval(reg).range.clone());
        if let Some(made) = self.rematerialize_all(lis, reg, vrai) {
            for value in made {
                edit.create_from(lis, &mut matrix.vrm, value, reg);
                let mut li = lis.compute_virt_reg_interval(value);
                vrai.calculate_spill_weight_and_hint(&mut li, lis);
                lis.insert_interval(li);
                new_vregs.push(value);
            }
            edit.erase_virt_reg(lis, reg);
            return;
        }
        self.key = self.shared_key(lis, range.as_ref());
        if let Some(range) = &range {
            self.homed.entry(self.key).or_default().push((reg, range.clone()));
        }
        self.keys.insert(reg, self.key);
        if let Some(width) = self.tables.widths.borrow().get(&reg).copied().filter(|width| *width > 0) {
            if self.frame.cell(self.key, width).is_err() {
                self.failed = true;
                return;
            }
        }
        // Update LiveStacks now that we are committed to spilling.
        if let Some(range) = &range {
            self.merged(range);
        }
        for (block, members) in Self::units(lis, reg) {
            if let [key] = members[..] {
                if self.coalesce_stack_access(lis, key, block) || self.sibling_copy(lis, matrix, key, block) {
                    continue;
                }
            }
            let Some(fresh) = self.spill_unit(lis, reg, block, &members) else {
                self.failed = true;
                return;
            };
            for value in fresh {
                edit.create_from(lis, &mut matrix.vrm, value, reg);
                let mut li = lis.compute_virt_reg_interval(value);
                vrai.calculate_spill_weight_and_hint(&mut li, lis);
                lis.insert_interval(li);
                new_vregs.push(value);
            }
        }
        // Hoisted spills may cause dead code.
        self.eliminate_dead_defs(lis, matrix, shrunk);
        edit.erase_virt_reg(lis, reg);
        if let Some(range) = &range {
            self.merged(range);
        }
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

    fn stacked(&mut self) {
        // Read now, while the numbers are the entries' own.
        let mut found: Vec<(i64, Vec<(i64, i64)>)> = self
            .stacks
            .iter()
            .map(|(home, range)| {
                (
                    *home,
                    range
                        .segments
                        .iter()
                        .map(|s| (i64::from(s.start.get_index()), i64::from(s.end.get_index())))
                        .collect(),
                )
            })
            .collect();
        found.sort_by_key(|one| one.0);
        self.frame.stacks = found.into_iter().collect();
    }

    fn recorded(&self) -> Vec<u32> {
        self.spilled.clone()
    }
}

#[cfg(test)]
mod tests {
    use llrm_lir::registers::Regs;

    use super::*;
    use crate::greedy::matrix::NoFixed;
    use crate::greedy::virtregmap::RegisterInfo;
    use crate::liveness::calcspillweights::{SpillWeightEnv, VirtRegAuxInfo};
    use crate::liveness::slotindex::SlotIndex;
    use crate::model::ir::{Addr, Mem, Space};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    struct Env;

    impl SpillWeightEnv for Env {
        fn block_freq_relative_to_entry(
            &self,
            _block: usize,
        ) -> f32 {
            1.0
        }

        fn is_loop_exiting(
            &self,
            _block: usize,
        ) -> bool {
            false
        }

        fn copy_of(
            &self,
            instr: &Insn,
        ) -> Option<(u32, u32)> {
            crate::backend::coalesce::_copy(instr)
        }

        fn pinned(
            &self,
            _reg: u32,
        ) -> Option<RegId> {
            None
        }

        fn is_rematerializable(
            &self,
            _instr: &Insn,
        ) -> bool {
            false
        }

        fn reg_mask_slots(&self) -> Vec<SlotIndex> {
            Vec::new()
        }

        fn spill_weight_scale(
            &self,
            _reg: u32,
        ) -> f32 {
            1.0
        }

        fn is_allocatable(
            &self,
            _reg: RegId,
        ) -> bool {
            true
        }
    }

    fn x87(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        // A copy made by a split claims no bytes.
        let covers = if op == Operation::Move { (at, at) } else { (at, at + 1) };
        Arc::new(Insn::new(at, Some(covers), Some(what), defines, uses))
    }

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 10 })
    }

    /// `%2` is a copy of `%1`, which dies there, and `%2` is spilled:
    /// `hoistSpillInsideBB` stores `%1` where it is defined and the copy goes.
    /// The spill stored `%2` after the copy into a home of its own, so every
    /// piece of a split value paid its own store: f9's nine live doubles
    /// stored ten times where the pin stores seven.
    #[test]
    fn test_the_spill_of_a_sibling_copy_stores_its_source_at_the_definition() {
        let global = Loc::Mem(Mem::new(Some(Addr::new(Space::Segment, 0)), 8));
        let insns = vec![
            x87(0, Operation::FloatLoad, "fld", vec![held(1)], vec![global.clone()], vec![1], vec![]),
            x87(1, Operation::Move, "mov", vec![held(2)], vec![held(1)], vec![2], vec![1]),
            x87(2, Operation::FloatArith, "fmul", vec![held(3)], vec![held(2), held(2)], vec![3], vec![2]),
            x87(3, Operation::FloatStore, "fstp", vec![global], vec![held(3)], vec![], vec![3]),
        ];
        let body =
            LirBody::new("hoist", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        let mut lis = LiveIntervals::new(&body);
        for value in 1..=3 {
            let li = lis.compute_virt_reg_interval(value);
            lis.insert_interval(li);
        }
        let mut matrix = LiveRegMatrix::new(RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO)), Box::new(NoFixed));
        matrix.vrm.set_is_split_from_reg(2, 1);
        let mut frame = Frame::new(0);
        let classes = RegisterClasses::m16();
        let tables = Rc::new(Tables::default());
        for value in 1..=3 {
            tables.orders.borrow_mut().insert(value, vec![RegId::MM0]);
            tables.float_bytes.borrow_mut().insert(value, 8);
            tables.widths.borrow_mut().insert(value, 10);
        }
        let order_of = |_: u32| Vec::new();
        let mut spiller = InlineSpiller::new(&body, &mut frame, &classes, &order_of, tables);
        let (env, original) = (Env, Default::default());
        let mut vrai = VirtRegAuxInfo::new(&env, &original);
        let (mut shrunk, mut made) = (Vec::new(), Vec::new());
        let ctx = SpillContext { lis: &mut lis, matrix: &mut matrix, vrai: &mut vrai, shrunk: &mut shrunk };
        spiller.spill(ctx, 2, &mut made);
        let stores = |lis: &LiveIntervals| -> Vec<usize> {
            let all = lis.block_insns(0);
            (0..all.len()).filter(|at| all[*at].spill_store).collect()
        };
        let after = lis.block_insns(0);
        let at = stores(&lis);
        assert_eq!(at.len(), 1, "stores of the home: {after:?}");
        assert_eq!(after[at[0]].uses, [1], "the source is stored, not the copy: {after:?}");
        assert_eq!(at[0], 1, "right after the source's definition: {after:?}");
        assert!(after.iter().all(|one| crate::backend::coalesce::_copy(one).is_none()), "the copy is dead: {after:?}");
        // Spilling the source in its turn stores its value once more, in the
        // same home: the store of the register itself goes.
        let ctx = SpillContext { lis: &mut lis, matrix: &mut matrix, vrai: &mut vrai, shrunk: &mut shrunk };
        spiller.spill(ctx, 1, &mut made);
        drop(spiller);
        let after = lis.block_insns(0);
        assert_eq!(stores(&lis).len(), 1, "one store for the two spills: {after:?}");
        assert_eq!(frame.slots.len(), 1, "one home for the two: {:?}", frame.slots);
    }

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
