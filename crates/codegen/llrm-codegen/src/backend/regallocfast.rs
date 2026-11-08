//! `RegAllocFast`, the allocator of the pin's -O0 pipeline. Translated from
//! llvm/lib/CodeGen/RegAllocFast.cpp at d1106deb71cc
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegAllocFast.cpp>).
//!
//! # How it works
//! Each block is walked backwards. A value is given a register at its last
//! use, the register is free again at its def, and a value that may live out
//! of the block is stored to its slot right after each def. A register another
//! instruction needs (a pinned value, a clobber) displaces the value in it: the
//! value is reloaded right after that instruction. Values still held at the top
//! of the block are reloaded there. Nothing is analysed across blocks:
//! `may_live_out` and `may_live_in` look at the first eight uses and defs.
//!
//! The body is the one `RegAlloc` prepared (pins, one class per value). Values
//! with a pin stand for physical registers: the use of one is `usePhysReg`, its
//! def `definePhysReg`/`freePhysReg`. An instruction's `clobbers` are its
//! register mask.
//!
//! DEVIATE R1 (no number, exit 2026-11-13): the static copy hints are one level
//! deep (an argument's arrival, a copy out of a pinned value); `traceCopies`
//! follows no chain of copies (the pin follows three). A use copied into a
//! value already placed takes that value's register, so a chain ending in an
//! argument register is followed to its end.
//!
//! DEVIATE R2 (no number, exit 2026-11-13): a body this does not cover is left
//! to the greedy allocator (`Fallback`): x87 or SSE values, segment registers,
//! a def narrower than its value, a pinned value that crosses blocks, a copy
//! group that reads what an earlier member wrote, or no register to take.
//!
//! DEVIATE R3 (no number, exit 2026-11-13): no `DBG_VALUE` to move onto a
//! spill slot: LIR keeps `-g` as tags on instructions, which stay on the
//! instruction they were on.
//!
//! # Used by
//! `allocate::RegAlloc` at -O0.

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::classes::RegisterClasses;
use crate::backend::frame::{Frame, SlotKey};
use crate::backend::target::{self, Segments};
use crate::greedy::tii;
use crate::model::ir::{self, Loc};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// The body is one this does not allocate; the greedy allocator does.
#[derive(Debug)]
pub struct Fallback(pub String);

const SPILL_CLEAN: u32 = 50;
const SPILL_DIRTY: u32 = 100;
const SPILL_PREF_BONUS: u32 = 20;
const SPILL_IMPOSSIBLE: u32 = u32::MAX;
/// `mayLiveOut` and `mayLiveIn`: the uses or defs looked at.
const LIMIT: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Free,
    /// A physical register an instruction below needs, or a pinned value.
    Pre,
    Virt(u32),
}

/// `LiveReg`: a value live at this point of the backward walk.
struct Live {
    value: u32,
    phys: Option<usize>,
    live_out: bool,
    reloaded: bool,
}

/// Where a value is defined and used, the first `LIMIT` of each.
#[derive(Default)]
struct Sites {
    defs: Vec<(usize, usize)>,
    uses: Vec<(usize, usize)>,
    more_defs: bool,
    more_uses: bool,
    width: u32,
}

struct Fast<'a> {
    classes: &'a RegisterClasses,
    segments: &'a Segments,
    pins: &'a IndexMap<u32, RegId>,
    frame: &'a mut Frame,
    roots: Vec<RegId>,
    index: HashMap<RegId, usize>,
    state: Vec<State>,
    used: Vec<u32>,
    generation: u32,
    live: Vec<Live>,
    at: HashMap<u32, usize>,
    sites: HashMap<u32, Sites>,
    across: HashSet<u32>,
    stored: HashSet<u32>,
    orders: HashMap<u32, Vec<usize>>,
    /// `traceCopies`: the register a value is copied from, where that is a
    /// physical one (an argument's arrival, a copy out of a pinned value).
    hints: HashMap<u32, usize>,
    confined: crate::backend::regclass::Classes,
    block: usize,
    successors: Vec<Vec<usize>>,
    /// Instructions inserted after the one being allocated, newest last.
    after: Vec<Arc<Insn>>,
    here: IndexMap<u32, RegId>,
    clobbers: Vec<usize>,
}

fn fail<T>(why: impl Into<String>) -> Result<T, Fallback> {
    Err(Fallback(why.into()))
}

/// The body with every value in a register of its own choosing, or why not.
pub fn allocate(
    body: &LirBody,
    pins: &IndexMap<u32, RegId>,
    segments: &Segments,
    classes: &RegisterClasses,
    frame: &mut Frame,
) -> Result<LirBody, Fallback> {
    let saved = frame.saved();
    let done = run(body, pins, segments, classes, frame);
    if done.is_err() {
        frame.restore(&saved);
    }
    done
}

fn run(
    body: &LirBody,
    pins: &IndexMap<u32, RegId>,
    segments: &Segments,
    classes: &RegisterClasses,
    frame: &mut Frame,
) -> Result<LirBody, Fallback> {
    if body.blocks.iter().any(|block| !block.phis.is_empty()) {
        return fail("a phi reached the allocator");
    }
    if !crate::backend::floatassign::_floating_values(body).is_empty() {
        return fail("an x87 value");
    }
    let span = llrm_support::debug::span("fast classes");
    let confined = crate::backend::regclass::classes(body, &Default::default(), segments, classes);
    let mut roots: Vec<RegId> = classes.available.clone();
    roots.extend(classes.xmm.iter().copied());
    for pinned in pins.values() {
        let root = ir::root(*pinned);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    let index: HashMap<RegId, usize> = roots.iter().enumerate().map(|(at, root)| (*root, at)).collect();
    let number: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let successors: Vec<Vec<usize>> =
        body.blocks.iter().map(|block| block.succ.iter().filter_map(|to| number.get(to).copied()).collect()).collect();
    let mut fast = Fast {
        classes,
        segments,
        pins,
        frame,
        state: vec![State::Free; roots.len()],
        used: vec![0; roots.len()],
        roots,
        index,
        generation: 0,
        live: Vec::new(),
        at: HashMap::default(),
        sites: HashMap::default(),
        across: HashSet::default(),
        stored: HashSet::default(),
        orders: HashMap::default(),
        hints: HashMap::default(),
        confined,
        block: 0,
        successors,
        after: Vec::new(),
        here: IndexMap::default(),
        clobbers: Vec::new(),
    };
    drop(span);
    let span = llrm_support::debug::span("fast scan");
    fast.scan(body)?;
    drop(span);
    let _span = llrm_support::debug::span("fast blocks");
    let entry = body.blocks.iter().position(|block| block.at == body.entry).unwrap_or(0);
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for (number, block) in body.blocks.iter().enumerate() {
        let insns = fast.block(block, number, number == entry)?;
        blocks.push(crate::model::lir::Block::from(LirBlock {
            at: block.at,
            insns: insns.into(),
            succ: block.succ.clone(),
            phis: block.phis.clone(),
            cold: block.cold,
            align: block.align,
            max_skip: block.max_skip,
        }));
    }
    Ok(LirBody {
        origin: body.origin.clone(),
        pins: body.pins.clone(),
        ordered: body.ordered,
        ..body.with_blocks(blocks)
    })
}

impl Fast<'_> {
    fn pinned(
        &self,
        one: &Insn,
        value: u32,
    ) -> Option<RegId> {
        self.pins
            .get(&value)
            .copied()
            .or_else(
                || one.requires
                    .iter()
                    .chain(&one.delivers)
                    .find(|(held, _)| held.value == value)
                    .map(|(_, register)| *register),
            )
    }

    fn is_pinned(
        &self,
        value: u32,
    ) -> bool {
        self.pins.contains_key(&value)
    }

    /// One pass: where each value is defined and used, how wide it is, and
    /// whether this allocator can take the body.
    fn scan(
        &mut self,
        body: &LirBody,
    ) -> Result<(), Fallback> {
        let mut pinned_in: HashMap<u32, usize> = HashMap::default();
        for (number, block) in body.blocks.iter().enumerate() {
            let mut groups: IndexMap<i64, Vec<(Vec<u32>, Vec<u32>)>> = IndexMap::default();
            for (position, one) in block.insns.iter().enumerate() {
                let Some(what) = &one.what else { return fail("an instruction without semantics") };
                let pseudo = matches!(
                    what.name.as_deref(),
                    Some("call_frame_setup" | "call_frame_destroy")
                );
                if what.op == ir::Operation::Barrier && !pseudo {
                    return fail("a barrier");
                }
                if let Some(group) = one.group {
                    groups.entry(group).or_default().push((one.defines.clone(), one.uses.clone()));
                }
                for (value, width) in crate::backend::widths::_named_widths(one) {
                    let sites = self.sites.entry(value).or_default();
                    sites.width = sites.width.max(width);
                }
                for (held, register) in &one.delivers {
                    if let Some(&at) = self.index.get(&ir::root(*register)) {
                        self.hints.entry(held.value).or_insert(at);
                    }
                }
                if what.op == ir::Operation::Move
                    && let ([value], [source]) = (&one.defines[..], &one.uses[..])
                    && !self.pins.contains_key(value)
                    && let Some(register) = self.pins.get(source)
                    && let Some(&at) = self.index.get(&ir::root(*register))
                {
                    self.hints.entry(*value).or_insert(at);
                }
                let mut defs: Vec<u32> = one.defines.clone();
                defs.extend(one.delivers.iter().map(|(held, _)| held.value));
                let mut uses: Vec<u32> = one.uses.clone();
                uses.extend(one.requires.iter().map(|(held, _)| held.value));
                for value in defs {
                    let sites = self.sites.entry(value).or_default();
                    if sites.defs.len() < LIMIT {
                        sites.defs.push((number, position));
                    } else {
                        sites.more_defs = true;
                    }
                    if self.pins.contains_key(&value) && *pinned_in.entry(value).or_insert(number) != number {
                        return fail("a pinned value crosses blocks");
                    }
                }
                for value in uses {
                    let sites = self.sites.entry(value).or_default();
                    if sites.uses.len() < LIMIT {
                        sites.uses.push((number, position));
                    } else {
                        sites.more_uses = true;
                    }
                    if self.pins.contains_key(&value) && *pinned_in.entry(value).or_insert(number) != number {
                        return fail("a pinned value crosses blocks");
                    }
                }
            }
            // A copy group is sequential here: no member may read what an
            // earlier one wrote.
            for members in groups.values() {
                for (later, (_, reads)) in members.iter().enumerate() {
                    if members[..later].iter().any(|(wrote, _)| wrote.iter().any(|value| reads.contains(value))) {
                        return fail("a parallel copy that reads what an earlier member wrote");
                    }
                }
            }
        }
        // Every def must be as wide as its value.
        for block in &body.blocks {
            for one in block.insns.iter() {
                let Some(what) = &one.what else { continue };
                for place in &what.dests {
                    if let Loc::Held(held) = place {
                        if self.sites.get(&held.value).is_some_and(|sites| held.width < sites.width.min(4))
                            && !self.is_pinned(held.value)
                        {
                            return fail("a def narrower than its value");
                        }
                    }
                }
            }
        }
        // Classes: nothing on the x87 stack, SSE or a segment register.
        let values: Vec<u32> = self.sites.keys().copied().collect();
        for value in values {
            if self.is_pinned(value) {
                continue;
            }
            let order = self.order_of(value)?;
            if order.is_empty() {
                return fail("a value no register may hold");
            }
        }
        Ok(())
    }

    /// The registers `value` may take, in allocation order.
    fn order_of(
        &mut self,
        value: u32,
    ) -> Result<Vec<usize>, Fallback> {
        if let Some(found) = self.orders.get(&value) {
            return Ok(found.clone());
        }
        let wanted = self.confined.get(&value);
        let order = target::order(wanted, self.segments, self.classes);
        let regs = self.classes.registers;
        let mut out = Vec::with_capacity(order.len());
        for register in order {
            if regs.in_class(register, llrm_lir::registers::class::RFP) {
                return fail("an x87 value");
            }
            if self.segments.registers.is_segment(register) {
                return fail("a segment register");
            }
            match self.index.get(&ir::root(register)) {
                Some(at) => out.push(*at),
                None => return fail("a register outside the allocatable set"),
            }
        }
        self.orders.insert(value, out.clone());
        Ok(out)
    }

    fn root_at(
        &self,
        register: RegId,
    ) -> Option<usize> {
        self.index.get(&ir::root(register)).copied()
    }

    // --- register units ---------------------------------------------------

    fn is_used(
        &self,
        at: usize,
        look_at_phys_uses: bool,
    ) -> bool {
        if look_at_phys_uses && self.clobbers.contains(&at) {
            return true;
        }
        self.used[at] >= (self.generation | u32::from(!look_at_phys_uses))
    }

    fn mark_used(
        &mut self,
        at: usize,
    ) {
        self.used[at] = self.generation | 1;
    }

    fn calc_spill_cost(
        &self,
        at: usize,
    ) -> u32 {
        match self.state[at] {
            State::Free => 0,
            State::Pre => SPILL_IMPOSSIBLE,
            State::Virt(value) => {
                let sure = self.stored.contains(&value) || self.live[self.at[&value]].live_out;
                if sure { SPILL_CLEAN } else { SPILL_DIRTY }
            }
        }
    }

    /// `displacePhysReg`: free `at`, reloading its value after the instruction.
    fn displace(
        &mut self,
        one: &Insn,
        at: usize,
    ) -> Result<bool, Fallback> {
        match self.state[at] {
            State::Virt(value) => {
                let slot = self.at[&value];
                self.reload(one, value, at)?;
                self.state[at] = State::Free;
                self.live[slot].phys = None;
                self.live[slot].reloaded = true;
                Ok(true)
            }
            State::Pre => {
                self.state[at] = State::Free;
                Ok(true)
            }
            State::Free => Ok(false),
        }
    }

    fn free(
        &mut self,
        at: usize,
    ) {
        match self.state[at] {
            State::Free => {}
            State::Pre => self.state[at] = State::Free,
            State::Virt(value) => {
                let slot = self.at[&value];
                self.state[at] = State::Free;
                self.live[slot].phys = None;
            }
        }
    }

    // --- spill code -------------------------------------------------------

    fn cell(
        &mut self,
        value: u32,
    ) -> Result<ir::Mem, Fallback> {
        let width = self.sites.get(&value).map_or(0, |sites| sites.width).max(1);
        self.frame.cell(SlotKey::from(value), i64::from(width)).map_err(|refused| Fallback(refused.0))
    }

    fn placed(
        &self,
        made: Arc<Insn>,
        value: u32,
        at: usize,
    ) -> Result<Arc<Insn>, Fallback> {
        let mut held = IndexMap::default();
        held.insert(value, self.roots[at]);
        crate::backend::rewriter::_placed(&made, &held, self.classes).map_err(|why| Fallback(why.0))
    }

    /// `reload`: `value` into register `at`, after `one`.
    fn reload(
        &mut self,
        one: &Insn,
        value: u32,
        at: usize,
    ) -> Result<(), Fallback> {
        let cell = self.cell(value)?;
        let made = tii::load_reg_from_stack_slot(one, value, &cell);
        let made = self.placed(made, value, at)?;
        self.after.push(made);
        Ok(())
    }

    /// `spill`: `value` from register `at` to its slot, after `one`.
    fn spill(
        &mut self,
        one: &Insn,
        value: u32,
        at: usize,
    ) -> Result<(), Fallback> {
        let cell = self.cell(value)?;
        let made = tii::store_reg_to_stack_slot(one, value, &cell);
        let made = self.placed(made, value, at)?;
        self.stored.insert(value);
        self.after.push(made);
        Ok(())
    }

    // --- liveness across blocks -------------------------------------------

    fn may_live_out(
        &mut self,
        value: u32,
    ) -> bool {
        let succ_empty = self.successors[self.block].is_empty();
        if self.across.contains(&value) {
            return !succ_empty;
        }
        let sites = &self.sites[&value];
        let mut self_def: Option<usize> = None;
        if self.successors[self.block].contains(&self.block) {
            for (block, position) in &sites.defs {
                if *block != self.block {
                    self.across.insert(value);
                    return true;
                }
                self_def = Some(self_def.map_or(*position, |first| first.min(*position)));
            }
            if sites.more_defs || self_def.is_none() {
                self.across.insert(value);
                return true;
            }
        }
        let mut seen = 0;
        for (block, position) in &sites.uses {
            seen += 1;
            if *block != self.block || seen >= LIMIT {
                self.across.insert(value);
                return !succ_empty;
            }
            if let Some(def) = self_def {
                if def >= *position {
                    self.across.insert(value);
                    return true;
                }
            }
        }
        if sites.more_uses {
            self.across.insert(value);
            return !succ_empty;
        }
        false
    }

    fn may_live_in(
        &mut self,
        value: u32,
    ) {
        if self.across.contains(&value) {
            return;
        }
        let sites = &self.sites[&value];
        let mut seen = 0;
        for (block, _) in &sites.defs {
            seen += 1;
            if *block != self.block || seen >= LIMIT {
                self.across.insert(value);
                return;
            }
        }
        if sites.more_defs {
            self.across.insert(value);
        }
    }

    // --- values -----------------------------------------------------------

    fn live_of(
        &mut self,
        value: u32,
    ) -> (usize, bool) {
        if let Some(at) = self.at.get(&value) {
            return (*at, false);
        }
        self.live.push(Live { value, phys: None, live_out: false, reloaded: false });
        self.at.insert(value, self.live.len() - 1);
        (self.live.len() - 1, true)
    }

    /// `allocVirtReg`.
    fn alloc(
        &mut self,
        one: &Insn,
        slot: usize,
        look_at_phys_uses: bool,
        hint0: Option<usize>,
    ) -> Result<(), Fallback> {
        let value = self.live[slot].value;
        let order = self.order_of(value)?;
        // Take a hint when its register is free: the one this value is copied
        // into (`hint0`, a use), else the one it is copied from.
        let mut preferred = [None, None];
        for (n, hint) in [hint0, self.hints.get(&value).copied()].into_iter().enumerate() {
            let Some(at) = hint else { continue };
            if order.contains(&at) && !self.is_used(at, look_at_phys_uses) {
                if matches!(self.state[at], State::Free) {
                    self.assign(slot, at);
                    return Ok(());
                }
                preferred[n] = Some(at);
            }
        }
        let (mut best, mut cost) = (None, SPILL_IMPOSSIBLE);
        for at in order {
            if self.is_used(at, look_at_phys_uses) {
                continue;
            }
            let mut price = self.calc_spill_cost(at);
            if price == 0 {
                self.assign(slot, at);
                return Ok(());
            }
            if preferred.contains(&Some(at)) {
                price = price.saturating_sub(SPILL_PREF_BONUS);
            }
            if price < cost {
                (best, cost) = (Some(at), price);
            }
        }
        let Some(best) = best else { return fail("no register to take") };
        self.displace(one, best)?;
        self.assign(slot, best);
        Ok(())
    }

    fn assign(
        &mut self,
        slot: usize,
        at: usize,
    ) {
        self.live[slot].phys = Some(at);
        self.state[at] = State::Virt(self.live[slot].value);
    }

    /// `defineVirtReg`.
    fn define(
        &mut self,
        one: &Insn,
        value: u32,
        look_at_phys_uses: bool,
    ) -> Result<usize, Fallback> {
        let (slot, new) = self.live_of(value);
        if new && self.may_live_out(value) {
            self.live[slot].live_out = true;
        }
        if self.live[slot].phys.is_none() {
            self.alloc(one, slot, look_at_phys_uses, None)?;
        }
        let at = self.live[slot].phys.expect("assigned above");
        if self.live[slot].reloaded || self.live[slot].live_out {
            self.spill(one, value, at)?;
            self.live[slot].live_out = false;
            self.live[slot].reloaded = false;
        }
        self.mark_used(at);
        self.here.insert(value, self.roots[at]);
        Ok(at)
    }

    /// `useVirtReg`.
    fn use_value(
        &mut self,
        one: &Insn,
        value: u32,
    ) -> Result<(), Fallback> {
        self.may_live_in(value);
        let (slot, new) = self.live_of(value);
        if new && self.may_live_out(value) {
            self.live[slot].live_out = true;
        }
        if self.live[slot].phys.is_none() {
            // a copy into a pinned value hints that register (`useVirtReg`);
            // a copy into a value already placed, the register it was given
            let mut hint0 = None;
            if one.what.as_ref().is_some_and(|what| what.op == ir::Operation::Move)
                && let ([def], [_]) = (&one.defines[..], &one.uses[..])
                && let Some(register) = self.pins.get(def).or_else(|| self.here.get(def))
            {
                hint0 = self.index.get(&ir::root(*register)).copied();
            }
            self.alloc(one, slot, false, hint0)?;
        }
        let at = self.live[slot].phys.expect("assigned above");
        self.mark_used(at);
        self.here.insert(value, self.roots[at]);
        Ok(())
    }

    // --- blocks and instructions -------------------------------------------

    fn block(
        &mut self,
        block: &LirBlock,
        number: usize,
        entry: bool,
    ) -> Result<Vec<Arc<Insn>>, Fallback> {
        self.block = number;
        self.state.iter_mut().for_each(|state| *state = State::Free);
        self.used.iter_mut().for_each(|used| *used = 0);
        self.generation = 0;
        self.live.clear();
        self.at.clear();
        let mut out: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len());
        for one in block.insns.iter().rev() {
            self.generation += 2;
            self.after.clear();
            self.here.clear();
            let mut made = self.instruction(one)?;
            out.append(&mut self.after);
            if made.group.is_some() {
                Arc::make_mut(&mut made).group = None;
            }
            out.push(made);
        }
        // `reloadAtBegin`: after the arrival of the arguments, in the order
        // the values were first seen.
        let begin = block.insns.iter().take_while(|one| one.arrival()).count();
        let mut reloads: Vec<Arc<Insn>> = Vec::new();
        let held: Vec<(u32, usize)> =
            self.live.iter().filter_map(|live| live.phys.map(|at| (live.value, at))).collect();
        if !held.is_empty() && entry {
            return fail("a value read before it is defined in the entry block");
        }
        for (value, at) in held {
            let beside = block.insns.get(begin).or_else(|| block.insns.last());
            let Some(beside) = beside else { continue };
            self.after.clear();
            self.reload(beside, value, at)?;
            reloads.append(&mut self.after);
        }
        // `out` runs backwards: the reloads come last, in reverse order, and
        // the arrival that stays above them is put back in front.
        let keep = begin.min(out.len());
        let total = out.len();
        let mut arrivals: Vec<Arc<Insn>> = out.split_off(total - keep);
        for reload in reloads.into_iter().rev() {
            out.push(reload);
        }
        out.append(&mut arrivals);
        out.reverse();
        Ok(out)
    }

    /// `defineLiveThroughVirtReg`: the register a tied value holds is one the
    /// instruction needs for something else; the value takes another and is
    /// copied into the old one after the instruction.
    fn move_off_physical_reads(
        &mut self,
        one: &Insn,
        value: u32,
    ) -> Result<(), Fallback> {
        let Some(slot) = self.at.get(&value).copied() else { return Ok(()) };
        let Some(prev) = self.live[slot].phys else { return Ok(()) };
        if !self.is_used(prev, true) {
            return Ok(());
        }
        self.free(prev);
        self.live[slot].phys = None;
        self.alloc(one, slot, true, None)?;
        let now = self.live[slot].phys.expect("assigned above");
        let width = i64::from(self.sites.get(&value).map_or(4, |sites| sites.width));
        let regs = self.classes.registers;
        let place = |at: usize| {
            let register = regs.named(self.roots[at], width);
            Loc::Reg(ir::Reg { register, width: width as u32 })
        };
        let copy = tii::_inserted(one, tii::_mov(place(prev), place(now)), Vec::new(), Vec::new());
        self.after.push(copy);
        Ok(())
    }

    fn instruction(
        &mut self,
        one: &Arc<Insn>,
    ) -> Result<Arc<Insn>, Fallback> {
        self.clobbers.clear();
        for register in one.clobbers.iter().chain(&one.clobbers_high) {
            if let Some(at) = self.index.get(&ir::root(*register)).copied() {
                self.clobbers.push(at);
            }
        }
        // Most instructions name a handful of values: a linear scan beats a
        // set.
        let mut defs: Vec<u32> = Vec::with_capacity(one.defines.len() + one.delivers.len());
        for value in one.defines.iter().copied().chain(one.delivers.iter().map(|(held, _)| held.value)) {
            if !defs.contains(&value) {
                defs.push(value);
            }
        }
        let mut uses: Vec<u32> = Vec::with_capacity(one.uses.len() + one.requires.len());
        for value in one.uses.iter().copied().chain(one.requires.iter().map(|(held, _)| held.value)) {
            if !uses.contains(&value) {
                uses.push(value);
            }
        }
        let mut physical: IndexMap<u32, RegId> = IndexMap::default();
        // Physical defs: the pinned values this defines.
        let mut defined: Vec<usize> = Vec::new();
        for value in &defs {
            if let Some(register) = self.pinned(one, *value) {
                physical.insert(*value, register);
                if let Some(at) = self.root_at(register) {
                    self.displace(one, at)?;
                    self.state[at] = State::Pre;
                    defined.push(at);
                }
            }
        }
        // Virtual defs. A value this also reads keeps its register, unless the
        // instruction reads that register as a physical one
        // (`defineLiveThroughVirtReg`).
        let mut tied: Vec<usize> = Vec::new();
        let mut virtual_defs: Vec<(u32, usize)> = Vec::new();
        if defs.iter().any(|value| !physical.contains_key(value) && uses.contains(value)) {
            for value in &uses {
                if let Some(register) = self.pinned(one, *value) {
                    if let Some(at) = self.root_at(register) {
                        self.used[at] = self.generation;
                    }
                }
            }
        }
        for value in &defs {
            if physical.contains_key(value) {
                continue;
            }
            let through = uses.contains(value);
            if through {
                self.move_off_physical_reads(one, *value)?;
            }
            let at = self.define(one, *value, through)?;
            if through {
                tied.push(at);
            }
            virtual_defs.push((*value, at));
        }
        // Free the defs, so a use may take a def's register.
        for at in defined.iter().rev() {
            self.free(*at);
            self.used[*at] = 0;
        }
        for (_, at) in &virtual_defs {
            if !tied.contains(at) {
                self.free(*at);
                self.used[*at] = 0;
            }
        }
        // The register mask: whatever is live in a clobbered register moves.
        if !self.clobbers.is_empty() {
            let clobbered = self.clobbers.clone();
            for at in clobbered {
                if matches!(self.state[at], State::Virt(_)) {
                    self.displace(one, at)?;
                }
            }
        }
        // Physical uses.
        for value in &uses {
            if let Some(register) = self.pinned(one, *value) {
                physical.insert(*value, register);
                if let Some(at) = self.root_at(register) {
                    self.displace(one, at)?;
                    self.state[at] = State::Pre;
                    self.mark_used(at);
                }
            }
        }
        // Virtual uses, but not the ones read as undefined.
        for value in &uses {
            if physical.contains_key(value) || one.undefs.contains(value) && !one.uses.contains(value) {
                continue;
            }
            self.use_value(one, *value)?;
        }
        if self.here.is_empty() && physical.is_empty() && one.undefs.is_empty() {
            return Ok(Arc::clone(one));
        }
        self.here.extend(physical.iter().map(|(value, register)| (*value, *register)));
        let placed = crate::backend::rewriter::_placed(one, &self.here, self.classes).map_err(|why| Fallback(why.0))?;
        // An identity move stays as an inert anchor: its value is defined.
        if placed.group.is_none() && crate::backend::rewriter::_pointless(&placed) {
            return Ok(crate::model::lir::anchor(placed));
        }
        Ok(placed)
    }
}
