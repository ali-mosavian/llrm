//! Forward and backward copy propagation inside a block, and dead-copy
//! removal: LLVM's `MachineCopyPropagation`.
//!
//! # How it works
//! Registers are chosen. Each block is walked twice and alone, backward then
//! forward, with a tracker keyed by the byte lanes of the general registers
//! (28 of them, LLVM's register units). The tracker knows, per lane, the copy
//! that last defined it, whether that copy is still available (neither side
//! written since), the lanes it defines, and the instructions that read a
//! copy's source (`src_users`).
//!
//! Forward (`forwardCopyPropagateBlock`): a copy that repeats an available one
//! (`ax = bx ... bx = ax` too) is dropped; a read of a copy's destination
//! reads its source instead; a copy whose destination is read by nothing
//! before the block ends and is not live out is dead and removed.
//! Backward (`backwardCopyPropagateBlock`): a copy whose source dies there
//! (`Y = op ...; Z = Y`) moves into the producer: the producer writes Z, and
//! the reads of Y between them read Z.
//!
//! ```text
//!   mov bx, ax                 mov bx, ax      <- dead when bx is not read
//!   add cx, bx          =>     add cx, ax         again and is not live out
//!   mov bx, ax                 (dropped)       <- bx == ax already
//! ```
//!
//! A removed copy becomes an anchor (`lir::anchor`), as in `machinedce`. A
//! rewrite is kept only when the new spelling still selects and the decoded
//! effects are the old ones with the renamed lanes replaced.
//!
//! LLVM's `CopyTracker` is one `DenseMap` of units; here the units are 28
//! slots of an array and a set of units is a `u64` mask.
//!
//! DEVIATE 1: LLVM asks a machine operand whether it is `renamable` and
//! whether `getRegClassConstraint` admits the new register; llrm has neither
//! flag, so the check is `select::priced_in` plus the effects test above.
//! Exit: none needed; the flags exist only because LLVM's operands carry
//! constraints llrm's instructions state in their semantics.
//!
//! DEVIATE 2: an instruction with unknown effects, or a contract of requires,
//! delivers, clobbers, a spread or a group, is never rewritten and ends every
//! tracked copy (LLVM sees a regmask and the operands themselves).
//!
//! # Example
//! No honest example: it runs inside the peephole phase, whose dump mixes it
//! with every other rewrite. The tests build the bodies.
//!
//! # Runs
//! In the peephole phase, where `copyprop` ran (it and `regthrash`'s rename
//! are gone from the pipeline). Skipped when the body holds no register copy
//! (`needs::COPY`); again after the zero-extension rewrite on targets that
//! have one.
//!
//! # See also
//! LLVM [`MachineCopyPropagation`]
//! (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp);
//! [`crate::backend`]

use std::sync::Arc;

use llrm_lir::registers::{RegId, Regs};

use crate::backend::lanes::Lanes;
use crate::backend::liveness::{self, Effect};
use crate::backend::peephole::{_lanes, _register_effects_of_what, Lane, is_frame_base};
use crate::backend::select;
use crate::model::ir::{self, Loc, Operation, Reg, Semantics};
use crate::model::lir::{self, Block, Insn, LirBody};
use crate::support::hash::IndexMap;

/// The byte lanes of the general registers: seven roots of four bytes. The
/// segment registers are not tracked: a string instruction reads `es`, which
/// its effect does not say, so a dead-looking `mov es, ss` must stay.
const UNITS: usize = 28;

/// A set of units.
type Mask = u64;

thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has run copy propagation, for a test that a body
/// with no copy is not given to it.
pub fn runs() -> usize {
    RUNS.with(std::cell::Cell::get)
}

/// How many units this thread's tracker has visited, for the slope test.
pub fn steps() -> usize {
    STEPS.with(std::cell::Cell::get)
}

/// The units of a mask, lowest first.
fn units_of(mut mask: Mask) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        (mask != 0).then(|| {
            STEPS.with(|steps| steps.set(steps.get() + 1));
            let unit = mask.trailing_zeros() as usize;
            mask &= mask - 1;
            unit
        })
    })
}

/// A register-to-register `mov` of equal widths, as unit pairs.
#[derive(Clone, Debug)]
struct Copy {
    /// `(destination unit, source unit)` per byte.
    pairs: Vec<(usize, usize)>,
    dst: Mask,
    src: Mask,
}

impl Copy {
    fn source_of(
        &self,
        unit: usize,
    ) -> Option<usize> {
        self.pairs.iter().find(|(dst, _)| *dst == unit).map(|(_, src)| *src)
    }

    /// `other` moves every byte this one does.
    fn repeats(
        &self,
        other: &Copy,
    ) -> bool {
        other.pairs.iter().all(|pair| self.pairs.contains(pair))
    }
}

/// What the tracker knows about one unit (LLVM's `CopyInfo`).
#[derive(Clone, Debug, Default)]
struct Info {
    /// The copy that defined the unit.
    copy: Option<usize>,
    /// The last copy that read the unit as its source.
    last_use_in_copy: Option<usize>,
    /// Instructions that read this unit as a copy's source (backward only).
    src_users: Vec<usize>,
    /// The units the copies from this one defined.
    defs: Mask,
    avail: bool,
}

/// LLVM's `CopyTracker`: a copy's index is its place in the block.
struct Tracker {
    units: [Option<Info>; UNITS],
    any: bool,
}

impl Tracker {
    fn new() -> Self {
        Self { units: std::array::from_fn(|_| None), any: false }
    }

    fn clear(&mut self) {
        if self.any {
            self.units.iter_mut().for_each(|unit| *unit = None);
            self.any = false;
        }
    }

    /// `markRegsUnavailable`.
    fn mark_unavailable(
        &mut self,
        mask: Mask,
    ) {
        for unit in units_of(mask) {
            if let Some(info) = &mut self.units[unit] {
                info.avail = false;
            }
        }
    }

    /// `invalidateRegister`: the register and what the copies naming it
    /// defined.
    fn invalidate(
        &mut self,
        mask: Mask,
        copies: &[Option<Copy>],
    ) {
        if !self.any {
            return;
        }
        let mut doomed = 0 as Mask;
        for unit in units_of(mask) {
            if let Some(info) = &self.units[unit] {
                for copy in [info.copy, info.last_use_in_copy].into_iter().flatten() {
                    let rec = copies[copy].as_ref().expect("a tracked copy is a copy");
                    doomed |= rec.dst | rec.src;
                }
            }
        }
        for unit in units_of(doomed) {
            self.units[unit] = None;
        }
    }

    /// `clobberRegUnit`.
    fn clobber_unit(
        &mut self,
        unit: usize,
        copies: &[Option<Copy>],
    ) {
        let Some(info) = &self.units[unit] else {
            return;
        };
        // Clobbering a source: what it defined is no longer a copy of it.
        let (defs, copy) = (info.defs, info.copy);
        self.mark_unavailable(defs);
        if let Some(copy) = copy {
            let rec = copies[copy].as_ref().expect("a tracked copy is a copy");
            self.mark_unavailable(rec.dst);
            for source in units_of(rec.src) {
                if let Some(entry) = &mut self.units[source] {
                    if entry.last_use_in_copy.is_some() && entry.defs & rec.dst != 0 {
                        entry.defs &= !rec.dst;
                        if entry.defs == 0 && entry.copy.is_none() {
                            self.units[source] = None;
                        }
                    }
                }
            }
        }
        self.units[unit] = None;
    }

    fn clobber(
        &mut self,
        mask: Mask,
        copies: &[Option<Copy>],
    ) {
        if !self.any {
            return;
        }
        for unit in units_of(mask) {
            self.clobber_unit(unit, copies);
        }
    }

    /// `trackCopy`.
    fn track(
        &mut self,
        at: usize,
        copy: &Copy,
    ) {
        self.any = true;
        for unit in units_of(copy.dst) {
            self.units[unit] = Some(Info { copy: Some(at), avail: true, ..Info::default() });
        }
        for unit in units_of(copy.src) {
            let info = self.units[unit].get_or_insert_with(Info::default);
            info.defs |= copy.dst;
            info.last_use_in_copy = Some(at);
        }
    }

    /// `findCopyForUnit`.
    fn copy_for(
        &self,
        unit: usize,
        must_be_available: bool,
    ) -> Option<usize> {
        let info = self.units[unit].as_ref()?;
        if must_be_available && !info.avail {
            return None;
        }
        info.copy
    }

    /// `findCopyDefViaUnit`: the copy that defined the one register this unit
    /// was copied to.
    fn copy_def_via(
        &self,
        unit: usize,
        copies: &[Option<Copy>],
    ) -> Option<usize> {
        let info = self.units[unit].as_ref()?;
        if info.defs == 0 {
            return None;
        }
        let copy = self.copy_for(info.defs.trailing_zeros() as usize, true)?;
        (copies[copy].as_ref()?.dst == info.defs).then_some(copy)
    }

    /// `findAvailCopy`: the available copy whose destination holds the
    /// register.
    fn avail_copy(
        &self,
        mask: Mask,
        copies: &[Option<Copy>],
    ) -> Option<usize> {
        if !self.any {
            return None;
        }
        let copy = self.copy_for(mask.trailing_zeros() as usize, true)?;
        (mask & !copies[copy].as_ref()?.dst == 0).then_some(copy)
    }

    /// `findAvailBackwardCopy`: the copy whose source is the register.
    fn avail_backward_copy(
        &self,
        mask: Mask,
        copies: &[Option<Copy>],
    ) -> Option<usize> {
        if !self.any {
            return None;
        }
        let copy = self.copy_def_via(mask.trailing_zeros() as usize, copies)?;
        (mask & !copies[copy].as_ref()?.src == 0).then_some(copy)
    }

    /// `trackSrcUsers`.
    fn track_src_users(
        &mut self,
        mask: Mask,
        user: usize,
        copies: &[Option<Copy>],
    ) -> bool {
        let unit = mask.trailing_zeros() as usize;
        let Some(copy) = self.copy_def_via(unit, copies) else {
            return false;
        };
        if copies[copy].as_ref().is_none_or(|rec| rec.src != mask) {
            return false;
        }
        self.units[unit]
            .as_mut()
            .is_some_and(
                |info| {
                    info.src_users.push(user);
                    true
                },
            )
    }

    fn src_users(
        &self,
        mask: Mask,
    ) -> Vec<usize> {
        self.units[mask.trailing_zeros() as usize].as_ref().map_or_else(Vec::new, |info| info.src_users.clone())
    }
}

/// Where a register sits in an instruction.
#[derive(Clone, Copy, Debug)]
enum Place {
    Source(usize),
    Dest(usize),
    /// An address register of source or destination `index`: base or index.
    Address {
        dest: bool,
        index: usize,
        through: bool,
    },
}

/// The tables of one body: lanes to units and back, and the register that
/// holds a set of lanes.
struct Machine {
    regs: Regs,
    bits: u32,
    places: Vec<[u8; 4]>,
    lanes: Vec<Lane>,
    register_for: IndexMap<Vec<Lane>, RegId>,
    /// Every lane: what an instruction of unknown effect may read.
    universe: Lanes,
}

impl Machine {
    fn new(body: &LirBody) -> Self {
        let regs = body.regs();
        let lanes: Vec<Lane> = llrm_x86::registers::ROOTS.into_iter().flat_map(|one| _lanes(regs, one)).collect();
        assert_eq!(lanes.len(), UNITS, "the general registers' byte lanes");
        let mut places = vec![[u8::MAX; 4]; 512];
        for (unit, (register, byte)) in lanes.iter().enumerate() {
            places[register.index()][*byte as usize] = unit as u8;
        }
        let mut register_for = IndexMap::default();
        for &register in regs.integer_registers().iter() {
            let held: Vec<Lane> = _lanes(regs, register).into_iter().collect();
            if !held.is_empty() && !regs.is_stack(register) {
                register_for.insert(held, register);
            }
        }
        Self { regs, bits: body.bits, places, lanes, register_for, universe: liveness::_universe(regs) }
    }

    fn unit(
        &self,
        lane: &Lane,
    ) -> Option<usize> {
        let place = *self.places.get(lane.0.index())?.get(lane.1 as usize)?;
        (place != u8::MAX).then_some(place as usize)
    }

    /// The general-register units among `lanes`.
    fn mask(
        &self,
        lanes: &Lanes,
    ) -> Mask {
        lanes.iter().filter_map(|lane| self.unit(&lane)).fold(0, |mask, unit| mask | 1 << unit)
    }

    /// The units of one register, None when it is not wholly general.
    fn register_mask(
        &self,
        register: RegId,
    ) -> Option<Mask> {
        let held: Vec<Lane> = _lanes(self.regs, register).into_iter().collect();
        if held.is_empty() {
            return None;
        }
        held.iter().try_fold(0, |mask, lane| Some(mask | 1 << self.unit(lane)?))
    }

    /// `one` as a copy LLVM would track: a plain `mov` between registers of
    /// one width, bytes to bytes, that says nothing more than that.
    fn copy_of(
        &self,
        one: &Insn,
    ) -> Option<Copy> {
        let what = one.what.as_ref()?;
        let (Operation::Move, Some("mov"), [Loc::Reg(dest)], [Loc::Reg(source)]) =
            (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
        else {
            return None;
        };
        if dest.width != source.width
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
        {
            return None;
        }
        let (destinations, sources): (Vec<Lane>, Vec<Lane>) = (
            _lanes(self.regs, dest.register).into_iter().collect(),
            _lanes(self.regs, source.register).into_iter().collect(),
        );
        if destinations.len() != dest.width as usize || sources.len() != destinations.len() {
            return None;
        }
        let pairs = destinations
            .iter()
            .zip(&sources)
            .map(|(dst, src)| Some((self.unit(dst)?, self.unit(src)?)))
            .collect::<Option<Vec<_>>>()?;
        let (dst, src) = pairs.iter().fold((0 as Mask, 0 as Mask), |(dst, src), (d, s)| (dst | 1 << d, src | 1 << s));
        (dst & src == 0).then_some(Copy { pairs, dst, src })
    }

    /// Every register `what` names, with where it sits: explicit operands and
    /// the registers of addresses, but not the frame base the frame layout
    /// resolves.
    fn registers(
        &self,
        what: &Semantics,
    ) -> Vec<(Place, RegId, i64)> {
        let mut out = Vec::new();
        for (dest, locs) in [(true, &what.dests), (false, &what.sources)] {
            for (index, loc) in locs.iter().enumerate() {
                if let Loc::Reg(reg) = loc {
                    out.push((if dest { Place::Dest(index) } else { Place::Source(index) }, reg.register, 0));
                }
                let Some(at) = loc.address() else { continue };
                for through in [true, false] {
                    let register = if through { at.through } else { at.index_through };
                    if register == RegId::None || (through && is_frame_base(self.regs, at.addr, register)) {
                        continue;
                    }
                    out.push((
                        Place::Address { dest, index, through },
                        register,
                        self.regs.width_of(register).unwrap_or(0),
                    ));
                }
            }
        }
        out
    }

    /// `what` with the register at `place` named `replacement`.
    fn renamed(
        &self,
        what: &Semantics,
        place: Place,
        replacement: RegId,
    ) -> Semantics {
        let mut out = what.clone();
        match place {
            Place::Source(index) => {
                if let Loc::Reg(reg) = out.sources[index] {
                    out.sources[index] = Loc::Reg(Reg { register: replacement, width: reg.width });
                }
            }
            Place::Dest(index) => {
                if let Loc::Reg(reg) = out.dests[index] {
                    out.dests[index] = Loc::Reg(Reg { register: replacement, width: reg.width });
                }
            }
            Place::Address { dest, index, through } => {
                let locs = if dest { &mut out.dests } else { &mut out.sources };
                locs[index] = locs[index].map_address(|at| {
                    if through {
                        ir::AddressRef { through: replacement, ..at }
                    } else {
                        ir::AddressRef { index_through: replacement, ..at }
                    }
                });
            }
        }
        out
    }

    /// `after` is `before` with the lanes `from` named `to` and nothing else:
    /// it selects, writes what `before` wrote and reads what it read, the one
    /// side swapped. `write` says which side `from` was on.
    fn same_but(
        &self,
        before: &Semantics,
        after: &Semantics,
        (from, to): (&Lanes, &Lanes),
        write: bool,
    ) -> bool {
        let Some(old) = _register_effects_of_what(self.regs, self.bits, before, true, false) else {
            return false;
        };
        if select::priced_in(self.bits, after, 0, None, false, false, None).is_none() {
            return false;
        }
        let Some(new) = _register_effects_of_what(self.regs, self.bits, after, true, false) else {
            return false;
        };
        let (kept, moved) = if write { (&old.0, &old.1) } else { (&old.1, &old.0) };
        // The register it is moved to is not touched by the other side.
        if !to.is_disjoint(kept) || write && !to.is_disjoint(moved) {
            return false;
        }
        let swapped: Lanes = moved.iter().filter(|lane| !from.contains(lane)).chain(to.iter()).collect();
        let (new_kept, new_moved) = if write { (&new.0, &new.1) } else { (&new.1, &new.0) };
        new_kept == kept && *new_moved == swapped
    }
}

/// What a walk of one block needs of the instructions: each one's effect, and
/// its copy when it is one.
struct Walk<'a> {
    machine: &'a Machine,
    insns: Vec<Arc<Insn>>,
    effects: Vec<Option<Effect>>,
    copies: Vec<Option<Copy>>,
    tracker: Tracker,
    /// Candidates for removal: a copy's place, while nothing has read it.
    maybe_dead: Vec<usize>,
    dead: Vec<bool>,
    changed: bool,
}

impl<'a> Walk<'a> {
    fn new(
        machine: &'a Machine,
        block: &Block,
    ) -> Self {
        let insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
        let effects = insns.iter().map(|one| liveness::effect(machine.regs, machine.bits, one)).collect();
        let copies = insns.iter().map(|one| machine.copy_of(one)).collect();
        let dead = vec![false; insns.len()];
        Self { machine, insns, effects, copies, tracker: Tracker::new(), maybe_dead: Vec::new(), dead, changed: false }
    }

    /// `at` replaced by `one`, its effect and copy worked out again.
    fn put(
        &mut self,
        at: usize,
        one: Arc<Insn>,
    ) {
        self.effects[at] = liveness::effect(self.machine.regs, self.machine.bits, &one);
        self.copies[at] = self.machine.copy_of(&one);
        self.insns[at] = one;
        self.changed = true;
    }

    /// `at` removed: an anchor with no effect and no copy.
    fn remove(
        &mut self,
        at: usize,
    ) {
        self.insns[at] = lir::anchor(Arc::clone(&self.insns[at]));
        self.effects[at] = liveness::effect(self.machine.regs, self.machine.bits, &self.insns[at]);
        self.copies[at] = None;
        self.dead[at] = false;
        self.changed = true;
    }

    /// `readRegister`: a copy whose destination is read is not dead.
    fn read(
        &mut self,
        mask: Mask,
    ) {
        for unit in units_of(mask) {
            if let Some(copy) = self.tracker.copy_for(unit, false) {
                self.dead[copy] = false;
            }
        }
    }

    /// `eraseIfRedundant`: `at` copies what an available copy already did.
    fn erase_if_redundant(
        &mut self,
        at: usize,
        reversed: bool,
    ) -> bool {
        let copy = self.copies[at].clone().expect("a copy");
        let (look, pairs): (Mask, Vec<(usize, usize)>) = if reversed {
            (copy.src, copy.pairs.iter().map(|(dst, src)| (*src, *dst)).collect())
        } else {
            (copy.dst, copy.pairs.clone())
        };
        let Some(previous) = self.tracker.avail_copy(look, &self.copies) else {
            return false;
        };
        let wanted = Copy { pairs, dst: 0, src: 0 };
        if !self.copies[previous].as_ref().is_some_and(|prior| prior.repeats(&wanted)) {
            return false;
        }
        self.remove(at);
        true
    }

    /// `forwardUses`: reads of a copy's destination read its source.
    fn forward_uses(
        &mut self,
        at: usize,
    ) {
        if !self.tracker.any {
            return;
        }
        let one = Arc::clone(&self.insns[at]);
        let Some(what) = &one.what else {
            return;
        };
        if !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
        {
            return;
        }
        let machine = self.machine;
        let mut current = what.clone();
        let mut touched = false;
        for (place, register, width) in machine.registers(what) {
            if matches!(place, Place::Dest(_)) {
                continue;
            }
            let Some(mask) = machine.register_mask(register) else {
                continue;
            };
            let Some(copy) = self.tracker.avail_copy(mask, &self.copies) else {
                continue;
            };
            let rec = self.copies[copy].as_ref().expect("a tracked copy");
            let mut sources: Vec<Lane> = Vec::new();
            for unit in units_of(mask) {
                let Some(source) = rec.source_of(unit) else {
                    sources.clear();
                    break;
                };
                sources.push(machine.lanes[source]);
            }
            sources.sort();
            let Some(&candidate) = machine.register_for.get(&sources) else {
                continue;
            };
            if candidate == register || width != 0 && machine.regs.width_of(candidate) != Some(width) {
                continue;
            }
            let proposed = machine.renamed(&current, place, candidate);
            let from: Lanes = _lanes(machine.regs, register);
            let to: Lanes = sources.iter().copied().collect();
            if machine.same_but(&current, &proposed, (&from, &to), false) {
                current = proposed;
                touched = true;
            }
        }
        if touched {
            let replacement = Arc::new(Insn { what: Some(current), ..(*one).clone() });
            self.put(at, replacement);
        }
    }

    /// `forwardCopyPropagateBlock`.
    fn forward(
        &mut self,
        live_out: &Lanes,
    ) {
        let machine = self.machine;
        for at in 0..self.insns.len() {
            if self.copies[at].is_some() && (self.erase_if_redundant(at, false) || self.erase_if_redundant(at, true)) {
                continue;
            }
            self.forward_uses(at);
            if self.copies[at].is_some() {
                self.maybe_dead.push(at);
                self.dead[at] = true;
            }
            match self.effects[at].clone() {
                // Unknown: it may read any register and write any.
                None => {
                    for copy in self.maybe_dead.drain(..) {
                        self.dead[copy] = false;
                    }
                    self.tracker.clear();
                }
                Some(effect) => {
                    let (reads, writes) = (machine.mask(&effect.all_reads()), machine.mask(&effect.writes));
                    self.read(reads);
                    self.tracker.clobber(writes, &self.copies);
                    if let Some(copy) = self.copies[at].clone() {
                        self.tracker.track(at, &copy);
                    }
                }
            }
        }
        // `readSuccessorLiveIns`: a copy whose destination is live out is not
        // dead. The tracker holds the copies that still define their lanes.
        for unit in units_of(machine.mask(live_out)) {
            if let Some(copy) = self.tracker.copy_for(unit, false) {
                self.dead[copy] = false;
            }
        }
        for copy in std::mem::take(&mut self.maybe_dead) {
            if self.dead[copy] {
                self.remove(copy);
            }
        }
        self.tracker.clear();
    }

    /// `propagateDefs`: the producer of a copy's source writes the destination.
    fn propagate_defs(
        &mut self,
        at: usize,
    ) -> Option<Copy> {
        if !self.tracker.any {
            return None;
        }
        let one = Arc::clone(&self.insns[at]);
        let Some(what) = &one.what else {
            return None;
        };
        if !one.clobbers.is_empty()
            || !one.clobbers_high.is_empty()
            || !one.requires.is_empty()
            || !one.delivers.is_empty()
            || !one.spread.is_empty()
            || one.group.is_some()
            || one.symbol == Some(true)
        {
            return None;
        }
        let machine = self.machine;
        let reads = self.effects[at].as_ref().map_or(0, |effect| machine.mask(&effect.all_reads()));
        for (place, register, _) in machine.registers(what) {
            if !matches!(place, Place::Dest(_)) {
                continue;
            }
            let Some(mask) = machine.register_mask(register) else {
                continue;
            };
            // A tied definition is read too.
            if mask & reads != 0 {
                continue;
            }
            let Some(copy) = self.tracker.avail_backward_copy(mask, &self.copies) else {
                continue;
            };
            let rec = self.copies[copy].clone().expect("a tracked copy");
            if rec.src != mask {
                continue;
            }
            let Some(destination) = machine.register_for_mask(rec.dst) else {
                continue;
            };
            let proposed = machine.renamed(what, place, destination);
            let (from, to) = (_lanes(machine.regs, register), machine.lanes_of(rec.dst));
            if !machine.same_but(what, &proposed, (&from, &to), true) {
                continue;
            }
            // `canUpdateSrcUsers`: every reader of the source can read the
            // destination.
            let mut updated = Vec::new();
            let mut fits = true;
            for user in self.tracker.src_users(mask) {
                let Some(user_what) = self.insns[user].what.clone() else {
                    fits = false;
                    break;
                };
                let mut current = user_what;
                for (user_place, user_register, _) in machine.registers(&current.clone()) {
                    if matches!(user_place, Place::Dest(_)) || user_register != register {
                        continue;
                    }
                    let next = machine.renamed(&current, user_place, destination);
                    if !machine.same_but(&current, &next, (&from, &to), false) {
                        fits = false;
                        break;
                    }
                    current = next;
                }
                if !fits {
                    break;
                }
                updated.push((user, current));
            }
            if !fits {
                continue;
            }
            for (user, replaced) in updated {
                let replacement = Arc::new(Insn { what: Some(replaced), ..(*self.insns[user]).clone() });
                self.put(user, replacement);
            }
            self.put(at, Arc::new(Insn { what: Some(proposed), ..(*one).clone() }));
            self.dead[copy] = true;
            self.maybe_dead.push(copy);
            return Some(rec);
        }
        None
    }

    /// What is live before `at`, given what is live after it.
    fn live_before(
        &self,
        at: usize,
        live: &Lanes,
    ) -> Lanes {
        self.effects[at].as_ref().map_or(self.machine.universe, |effect| effect.live_before(live))
    }

    /// `backwardCopyPropagateBlock`. `live` is what is live after the last
    /// instruction.
    fn backward(
        &mut self,
        mut live: Lanes,
    ) {
        let machine = self.machine;
        for at in (0..self.insns.len()).rev() {
            if let Some(copy) = self.copies[at].clone() {
                // `isBackwardPropagatableCopy`: its source dies here.
                let dies = machine.lanes_of(copy.src).iter().all(|lane| !live.contains(lane));
                if dies {
                    self.tracker.invalidate(copy.src, &self.copies);
                    self.tracker.invalidate(copy.dst, &self.copies);
                    self.tracker.track(at, &copy);
                    live = self.live_before(at, &live);
                    continue;
                }
            }
            if let Some(copy) = self.propagate_defs(at) {
                // The destination is live from here, where the source was.
                live = live.minus(&machine.lanes_of(copy.src)).or(&machine.lanes_of(copy.dst));
            }
            let one = Arc::clone(&self.insns[at]);
            match (&one.what, &self.effects[at]) {
                (Some(what), Some(effect)) => {
                    let (reads, writes) = (machine.mask(&effect.all_reads()), machine.mask(&effect.writes));
                    let (mut seen_reads, mut seen_writes) = (0 as Mask, 0 as Mask);
                    for (place, register, _) in machine.registers(what) {
                        let Some(mask) = machine.register_mask(register) else {
                            continue;
                        };
                        if matches!(place, Place::Dest(_)) {
                            seen_writes |= mask;
                            self.tracker.invalidate(mask, &self.copies);
                        } else {
                            seen_reads |= mask;
                            if !self.tracker.track_src_users(mask, at, &self.copies) {
                                self.tracker.invalidate(mask, &self.copies);
                            }
                        }
                    }
                    // What the instruction does that no operand says.
                    self.tracker.invalidate((reads & !seen_reads) | (writes & !seen_writes), &self.copies);
                }
                _ => self.tracker.clear(),
            }
            live = self.live_before(at, &live);
        }
        for copy in std::mem::take(&mut self.maybe_dead) {
            if self.dead[copy] {
                self.remove(copy);
            }
        }
        self.tracker.clear();
    }
}

impl Machine {
    /// The lanes of the units in `mask`.
    fn lanes_of(
        &self,
        mask: Mask,
    ) -> Lanes {
        units_of(mask).map(|unit| self.lanes[unit]).collect()
    }

    /// The register that holds exactly the units of `mask`.
    fn register_for_mask(
        &self,
        mask: Mask,
    ) -> Option<RegId> {
        let mut held: Vec<Lane> = units_of(mask).map(|unit| self.lanes[unit]).collect();
        held.sort();
        self.register_for.get(&held).copied()
    }
}

/// `body` with each block's copies propagated and its dead copies removed.
pub fn propagated(body: &LirBody) -> LirBody {
    RUNS.with(|runs| runs.set(runs.get() + 1));
    if body.blocks.iter().any(|block| !block.phis.is_empty()) {
        return body.clone();
    }
    let machine = Machine::new(body);
    let dead = liveness::dead_at_exit(body);
    let universe = liveness::_universe(machine.regs);
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let live_out = dead.get(&block.at).map_or(universe, |dead| universe.minus(dead));
            let mut walk = Walk::new(&machine, block);
            walk.backward(live_out);
            walk.forward(&live_out);
            if walk.changed { block.with_insns(walk.insns) } else { block.clone() }
        })
        .collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
#[path = "machinecp_tests.rs"]
mod tests;
