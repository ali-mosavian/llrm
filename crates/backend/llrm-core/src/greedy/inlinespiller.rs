//! `InlineSpiller`: spills a live range by rewriting each instruction that
//! names it through the stack, inside the allocation in progress. Translated
//! from llvm/lib/CodeGen/InlineSpiller.cpp (`spill`, `spillAll`,
//! `spillAroundUses`, `insertReload`, `insertSpill`) at d1106deb71cc.
//!
//! # How it works
//! `spill(reg)` takes every instruction that names `reg` (a parallel copy is
//! one unit) and has the per-instruction decisions of `backend::spiller` make
//! its replacement: a reload before, the instruction rewritten, a store after,
//! or the stack cell folded into the instruction itself. The replacement is put
//! into the edited function with `LiveIntervals::rewrite_instr` (the
//! instruction keeps its slot, so a call keeps its register mask) and
//! `insert_instr_before/after` (new slots between the neighbours: no other
//! interval moves). Each register the replacement names for the first time is
//! made with `LiveRangeEdit::create_from`, gets its interval and weight, and is
//! returned for the queue. The interval of `reg` is erased. Nothing is rebuilt:
//! this is why a spill no longer restarts the allocation.
//!
//! DEVIATE D-A: the choice of reload, store or fold is `backend::spiller`'s
//! (`_source`, `_in_place`, `_tied`, the parallel-copy folds), asked for one
//! instruction at a time through `spilled_from` on a body of that instruction.
//! The fact: the m16 forms (segment-offset pairs, 8/16/32-bit lanes, the folds
//! into a parallel copy) are not forms `TargetInstrInfo::foldMemoryOperand`
//! knows. Exit: those forms expressed as `foldMemoryOperand` rules, with the
//! spiller's whole-body plan (copies, merged updates) gone; issue to be
//! opened with step 2. Boundary: a replacement that does not hold the same
//! number of instructions as the unit it replaces, or that still names `reg`,
//! sets `failed` and the caller falls back to the rounds.
//!
//! DEVIATE D-B: no `analyzeSiblingValues`, `hoistAllSpills` or
//! `reMaterializeAll` ahead of the spill: they need SplitKit's sibling
//! relations (step 2). Exit: SplitKit translated.
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
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;
use crate::model::ir::{Held, Loc, Operation, Semantics};

use super::greedy::{SpillContext, Spiller};
use super::liverangeedit::{LiveRangeEdit, NoDelegate};
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::Frame;
use crate::backend::spiller::{recording_cores, spilled_from};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
use crate::model::lir::{Block, Insn, LirBlock, LirBody};

/// What the allocator keeps per register and the spiller must extend for the
/// registers it makes: the allocation order and the widest access.
#[derive(Default)]
pub struct Tables {
    pub orders: RefCell<HashMap<u32, Vec<RegId>>>,
    pub widths: RefCell<HashMap<u32, u32>>,
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
    /// Registers other than the spilled one that the units named: an edit may
    /// have given them a definition or a use.
    named: Vec<u32>,
}

impl<'a> InlineSpiller<'a> {
    pub fn new(
        template: &'a LirBody,
        frame: &'a mut Frame,
        classes: &'a RegisterClasses,
        order_of: &'a dyn Fn(u32) -> Vec<RegId>,
        tables: Rc<Tables>,
    ) -> Self {
        Self { template, frame, classes, order_of, tables, reloads: 0, stores: 0, folded: 0, failed: false, spilled: Vec::new(), named: Vec::new() }
    }

    /// The units of `reg`'s instructions: a parallel copy is one, listed by its
    /// members in block order.
    fn units(
        lis: &LiveIntervals,
        reg: u32,
    ) -> Vec<(usize, Vec<InstrKey>)> {
        let mut keys: Vec<InstrKey> = Vec::new();
        for operand in lis.operands(reg) {
            if !keys.contains(&operand.instr) {
                keys.push(operand.instr);
            }
        }
        let mut units: Vec<(usize, Vec<InstrKey>)> = Vec::new();
        for key in keys {
            if units.iter().any(|(_, members)| members.contains(&key)) {
                continue;
            }
            let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(key));
            let insns = lis.block_insns(block);
            let at = insns.iter().position(|one| one.id() == key).expect("in its block");
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
            units.push((block, members));
        }
        units
    }

    /// `reMaterializeAll` for a register defined once by `mov %reg, imm`: the
    /// move is made again, in a new register, before each instruction that
    /// reads it, and the original goes (`eliminateDeadDef`). A constant needs
    /// nothing to be available where it is read. Returns the registers made,
    /// or None when the register is not one of these.
    ///
    /// DEVIATE D-E2: only a constant is rematerialized here. The fact: the
    /// other forms the spiller makes again (a frame address, a load of a frame
    /// cell, an extension of a value kept) need `allUsesAvailableAt`, which
    /// needs the live intervals of what they read; they are spilled through
    /// the cell instead. Exit: `LiveRangeEdit::allUsesAvailable` over
    /// `LiveIntervals` (step 2).
    fn rematerialize_all(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
    ) -> Option<Vec<u32>> {
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
        let what = def.what.as_ref()?;
        let constant = what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && matches!(what.sources.as_slice(), [Loc::Imm(imm)] if imm.address.is_none())
            && matches!(what.dests.as_slice(), [Loc::Held(held)] if held.value == reg)
            && def.defines == [reg]
            && def.uses.is_empty()
            && def.group.is_none()
            && def.requires.is_empty()
            && def.delivers.is_empty()
            && def.clobbers.is_empty()
            && def.symbol != Some(true);
        if !constant {
            return None;
        }
        let def_block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(def.id()));
        let mut made = Vec::new();
        let Loc::Imm(imm) = &what.sources[0] else { unreachable!("checked above") };
        for (block, members) in Self::units(lis, reg) {
            if members == [def.id()] {
                continue;
            }
            // A parallel copy takes the literal in place: `mov %d, %reg` as a
            // member becomes `mov %d, imm`, and no register holds the constant
            // until the copy (the constants of every member would be live at
            // once).
            let in_place: Option<Vec<Arc<Insn>>> = members
                .iter()
                .map(|key| {
                    let one = lis.instr(*key);
                    if !one.uses.contains(&reg) && !one.defines.contains(&reg) {
                        return Some(Arc::clone(one));
                    }
                    let w = one.what.as_ref()?;
                    let plain = w.op == Operation::Move && w.name.as_deref() == Some("mov");
                    let ok = one.group.is_some()
                        && plain
                        && one.uses == [reg]
                        && one.requires.is_empty()
                        && one.delivers.is_empty()
                        && one.clobbers.is_empty()
                        && matches!(w.sources.as_slice(), [Loc::Held(h)] if h.value == reg && h.width == imm.width)
                        && matches!(w.dests.as_slice(), [Loc::Held(d)] if d.width == imm.width);
                    ok.then(|| {
                        let mut folded = (**one).clone();
                        folded.what = Some(Semantics { sources: vec![Loc::Imm(imm.clone())], ..w.clone() });
                        folded.uses.clear();
                        folded.symbol = Some(false);
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
            let again = Arc::new(Insn {
                defines: vec![value],
                what: Some(Semantics {
                    dests: vec![Loc::Held(Held {
                        value,
                        width: match &what.dests[0] {
                            Loc::Held(held) => held.width,
                            _ => unreachable!("checked above"),
                        },
                    })],
                    ..what.clone()
                }),
                spill_reload: true,
                rematerialized: true,
                ..(*def).clone()
            });
            // A parallel copy reads before it writes: the constant goes before
            // its first member, whichever member reads it.
            let mut first = None;
            for key in &members {
                let renamed = lis.instr(*key).renamed(&|name| if name == reg { value } else { name });
                first.get_or_insert(renamed.id());
                lis.rewrite_instr(block, *key, renamed);
            }
            lis.insert_instr_before(block, first, again);
            self.reloads += 1;
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

    /// The intervals of the registers the edit named other than the spilled one,
    /// made again: an instruction put before a use may define one (an index added
    /// into its base) and so number its values anew. It stays right for the matrix
    /// only if it covers the same points, which is checked; false when it does not.
    fn refresh_named(
        &mut self,
        lis: &mut LiveIntervals,
    ) -> bool {
        let mut names = std::mem::take(&mut self.named);
        names.sort_unstable();
        names.dedup();
        for value in names {
            if !lis.has_interval(value) {
                continue;
            }
            let held = lis.interval_rc(value);
            let mut again = lis.compute_virt_reg_interval(value);
            let merged = |li: &crate::liveness::liverange::LiveInterval| -> Vec<_> {
                let mut spans: Vec<(crate::liveness::slotindex::SlotIndex, crate::liveness::slotindex::SlotIndex)> = Vec::new();
                for seg in &li.segments {
                    match spans.last_mut() {
                        Some(last) if last.1 == seg.start => last.1 = seg.end,
                        _ => spans.push((seg.start, seg.end)),
                    }
                }
                spans
            };
            if merged(&held) != merged(&again) {
                return false;
            }
            again.set_weight(held.weight());
            lis.insert_interval(again);
        }
        true
    }

    /// `spillAroundUses` for one unit. Returns the registers it made.
    fn spill_unit(
        &mut self,
        lis: &mut LiveIntervals,
        reg: u32,
        block: usize,
        members: &[InstrKey],
    ) -> Option<Vec<u32>> {
        let old: Vec<Arc<Insn>> = members.iter().map(|key| Arc::clone(lis.instr(*key))).collect();
        self.named.extend(old.iter().flat_map(|one| one.defines.iter().chain(&one.uses)).copied().filter(|value| *value != reg));
        if let Some(folded) = self.fold_into_copy(lis, reg, block, members, &old) {
            return Some(folded);
        }
        // The plan of the spiller reads the definitions of a register it can see:
        // a constant defined once is made again at its uses and its definition
        // dropped. Here the definition may be one of several (or the uses are
        // other units), so a second, empty definition says the register is
        // stored, and its slot written at this one.
        let marker = Arc::new(Insn::new(0, None, None, vec![reg], Vec::new()));
        let marked = old.iter().any(|one| one.defines.contains(&reg));
        let mut seen = Vec::new();
        if marked {
            seen.push(Arc::clone(&marker));
        }
        seen.extend(old.iter().cloned());
        // What the spiller decides by the last use of a value (an index added
        // into its base register when the base is not read again) is decided over
        // the body it is given. A last empty instruction that reads what the unit
        // reads and lives on after it says those uses are not the last.
        let end = lis.indexes.get_instruction_index(*members.last().expect("a unit has members")).get_reg_slot(false);
        let mut later: Vec<u32> = old.iter().flat_map(|one| one.uses.iter().copied()).filter(|value| *value != reg).collect();
        later.sort_unstable();
        later.dedup();
        later.retain(|value| lis.interval_rc(*value).end_index() > end);
        let tail = Arc::new(Insn::new(0, None, None, Vec::new(), later));
        seen.push(Arc::clone(&tail));
        let mut mini = self.template.with_blocks(vec![Block::from(LirBlock::new(0, seen))]);
        mini.entry = 0;
        mini.inputs.clear();
        mini.frequencies = None;
        let values = std::iter::once(reg).collect();
        let floor = lis.next_virtual_register();
        let (spilled, made_cores) =
            recording_cores(|| spilled_from(&mini, &values, Some(&mut *self.frame), floor, self.classes));
        let Ok((out, _made)) = spilled else {
            if std::env::var("LLRM_ISP_DEBUG").is_ok() {
                eprintln!("inline spill of v{reg}: spilled_from refused");
            }
            return None;
        };
        let mut out: Vec<Arc<Insn>> = out.insns().into_iter().filter(|one| one.id() != tail.id()).collect();
        if marked {
            // The marker, defined first, comes out as itself and its store.
            let ok = out.len() >= 2
                && out[0].what.is_none()
                && out[1].spill_store
                && out[0].defines.iter().all(|value| out[1].uses.contains(value));
            if !ok {
                return None;
            }
            out.drain(..2);
        }
        // The instructions of the unit that went through unchanged, and those the
        // spiller wrote in their place.
        let given: Vec<u64> = old.iter().map(|one| one.id()).collect();
        let is_core = |one: &Arc<Insn>| given.contains(&one.id()) || made_cores.contains(&one.id());
        let cores: Vec<&Arc<Insn>> = out.iter().filter(|one| is_core(one)).collect();
        if cores.len() != members.len() || cores.iter().any(|one| one.defines.contains(&reg) || one.uses.contains(&reg)) {
            if std::env::var("LLRM_ISP_DEBUG").is_ok() {
                eprintln!("inline spill of v{reg}: {} members, {} cores", members.len(), cores.len());
                for one in &old {
                    eprintln!("  in d{:?} u{:?} {:?}", one.defines, one.uses, one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources)));
                }
                for one in &out {
                    eprintln!("  out d{:?} u{:?} reload {} store {} {:?}", one.defines, one.uses, one.spill_reload, one.spill_store, one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources)));
                }
            }
            return None;
        }
        let mut keys: Vec<InstrKey> = Vec::with_capacity(cores.len());
        for (key, core) in members.iter().zip(&cores) {
            lis.rewrite_instr(block, *key, Arc::clone(core));
            keys.push(core.id());
        }
        let mut seen = 0;
        let mut last = *keys.last().expect("a unit has members");
        for one in &out {
            if is_core(one) {
                seen += 1;
            } else if seen < keys.len() {
                lis.insert_instr_before(block, Some(keys[seen]), Arc::clone(one));
            } else {
                lis.insert_instr_after(block, last, Arc::clone(one));
                last = one.id();
            }
            if one.spill_reload {
                self.reloads += 1;
            }
            if one.spill_store {
                self.stores += 1;
            }
        }
        if out.iter().all(is_core) {
            self.folded += 1;
        }
        // A register an instruction requires or delivers in one register can be
        // given that one only.
        let pinned: HashMap<u32, RegId> = out
            .iter()
            .flat_map(|one| one.requires.iter().chain(&one.delivers))
            .map(|(held, register)| (held.value, crate::model::ir::root(*register)))
            .collect();
        let width = self.tables.widths.borrow().get(&reg).copied().unwrap_or(0);
        // Every register the replacement names that did not exist: the spiller's own
        // list of what it made leaves out a load put before an instruction.
        let mut fresh: Vec<u32> = out
            .iter()
            .flat_map(|one| one.defines.iter().chain(&one.uses))
            .copied()
            .filter(|value| *value >= floor)
            .collect();
        fresh.sort_unstable();
        fresh.dedup();
        for &value in &fresh {
            lis.reserve_virtual_register(value);
            let mut order: Vec<RegId> =
                (self.order_of)(reg).into_iter().map(crate::model::ir::root).collect();
            order.dedup();
            if let Some(only) = pinned.get(&value) {
                order = vec![*only];
            }
            self.tables.orders.borrow_mut().insert(value, order);
            self.tables.widths.borrow_mut().insert(value, width);
        }
        Some(fresh)
    }
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
        // The cell is as wide as the widest access anywhere in the function: a
        // unit asks for the width of its own accesses only, and a narrow cell
        // made by the first would be written whole by a wider one.
        if let Some(width) = self.tables.widths.borrow().get(&reg).copied().filter(|width| *width > 0) {
            if self.frame.cell(reg, width).is_err() {
                self.failed = true;
                return;
            }
        }
        let mut delegate = NoDelegate;
        let mut edit = LiveRangeEdit::new(reg, &mut delegate);
        if let Some(made) = self.rematerialize_all(lis, reg) {
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
        if !self.refresh_named(lis) {
            self.failed = true;
        }
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
