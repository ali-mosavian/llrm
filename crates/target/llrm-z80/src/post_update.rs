//! A word access and the step of its pointer, merged: `ld c, (hl); inc hl; ld
//! b, (hl); inc hl`.
//!
//! # How it works
//! After allocation a word load or store through `hl` is followed, some
//! instructions later, by the step of its pointer by the access size: the
//! pointer lives in a home pair (`bc` or `de`) and is stepped through `hl`
//! (`copy hl <- rr; ld t, #2; add hl, t; copy rr <- hl`), or lives in `hl` and
//! is stepped there. The access put `hl` back with a `dec hl`. The access
//! becomes its post-incremented form (`LD16rm_pi`, `LD16mr_pi`: `hl` ends past
//! the word), the step goes, and a home pair gets a copy of `hl` after the
//! access.
//!
//! This is `AArch64LoadStoreOpt::tryToMergeLdStUpdate` with
//! `findMatchingUpdateInsnForward` and `mergeUpdateInsn` ([pin][find]), after
//! allocation as there: the same fold made by the DAG combine narrowed the
//! classes the allocator saw and reallocated pressured functions. The Rust
//! translation of those functions is `llrm-arm64`'s `ldst_update.rs`; the
//! scan below follows its order and limits: the update limit, a call or a
//! use of the base ends the scan, and the scan goes on into the single
//! successor the base is live into, when that has one predecessor.
//!
//! DEVIATE Z80-UPDATE-1: post-index only. The pin also folds an update before
//! the access (pre-index) and an offset (`findMatchingUpdateInsnBackward`);
//! the Z80 has neither form. Owner llrm-z80-c. Exit: none.
//! DEVIATE Z80-UPDATE-2: the base is a home pair and `hl` (the update is
//! copy in, step, copy out), the amount is the access size alone (the pin's
//! `isMatchingUpdateInsn`), word accesses only (a byte through `bc` or `de`
//! needs no `hl`), and the carry the `add` set must be dead (the pin's update
//! sets no flags). Owner llrm-z80-c. Exit: none.
//!
//! [find]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64LoadStoreOptimizer.cpp#L2535

use llrm_lir::registers::RegId;
use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::livephys::PhysLiveness;
use llrm_spike_mf::regalloc::{Assignment, Target};

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::Z80Registers;
use crate::{REGISTER_INFO, reg};

/// `UpdateLimit` (`-aarch64-update-scan-limit`).
const UPDATE_LIMIT: usize = 100;

/// `LdStConstLimit` (`-aarch64-load-store-const-scan-limit`): how far back the
/// constant of an `add` is looked for.
const CONST_LIMIT: usize = 10;

/// `LLRM_Z80_UPDATE_WHY`: one line a candidate access, merged or why not (the
/// premise count).
fn why_enabled() -> bool {
    std::env::var_os("LLRM_Z80_UPDATE_WHY").is_some()
}

/// Why an access is not merged: not a candidate at all, or a candidate the
/// pin's checks refuse.
enum Refused {
    Not,
    Because(&'static str),
}

/// A word access through `hl` that a step can merge into.
struct Access {
    /// Where the pointer lives besides `hl`: `bc` or `de`; none when it is
    /// `hl`.
    home: Option<RegId>,
}

/// The update found: its block, and the instructions of it, ascending.
struct Update {
    block: usize,
    gone: Vec<usize>,
}

struct Context<'a> {
    function: &'a MFunction,
    assignment: &'a Assignment,
    cfg: &'a Cfg,
    live: &'a PhysLiveness,
}

impl Context<'_> {
    fn register(
        &self,
        r: Reg,
    ) -> RegId {
        match r {
            Reg::Virt(v) => self.assignment.0[v as usize],
            Reg::Phys(id) => id,
        }
    }

    fn reads(
        &self,
        inst: &Inst,
        to: RegId,
    ) -> bool {
        inst.operands
            .iter()
            .any(
                |o| matches!(
                    o,
                    Operand::Use(r) if Z80Registers::overlaps(self.register(*r), to)
                ),
            )
    }

    fn writes(
        &self,
        inst: &Inst,
        to: RegId,
    ) -> bool {
        inst.operands
            .iter()
            .any(
                |o| matches!(
                    o,
                    Operand::Def(r) if Z80Registers::overlaps(self.register(*r), to)
                ),
            )
    }

    fn touches(
        &self,
        inst: &Inst,
        to: RegId,
    ) -> bool {
        self.reads(inst, to) || self.writes(inst, to)
    }

    /// Whether `r` is live just after instruction `at` of block `b`.
    fn live_after(
        &self,
        b: usize,
        at: usize,
        r: RegId,
    ) -> bool {
        self.live
            .live_before::<Z80Registers>(self.function, self.assignment, &REGISTER_INFO, b, at + 1)
            .is_live(&REGISTER_INFO, r)
    }

    /// Whether `r` is read from the start of block `b`.
    fn live_in(
        &self,
        b: usize,
        r: RegId,
    ) -> bool {
        self.live
            .live_before::<Z80Registers>(self.function, self.assignment, &REGISTER_INFO, b, 0)
            .is_live(&REGISTER_INFO, r)
    }

    /// The copy `dst <- src` of two registers.
    fn copy_of(
        &self,
        inst: &Inst,
    ) -> Option<(RegId, RegId)> {
        match (inst.kind, &inst.operands[..]) {
            (Kind::Copy, [Operand::Def(d), Operand::Use(s)]) => Some((self.register(*d), self.register(*s))),
            _ => None,
        }
    }

    /// `getLdStBaseOp`, `getLdStRegOp`: the access at `block[at]`, if it is one
    /// a step can merge into.
    fn access_at(
        &self,
        block: usize,
        at: usize,
    ) -> Result<Access, Refused> {
        let insts = &self.function.blocks[block].insts;
        let access = &insts[at];
        let (data, pointer) = match (access.kind, &access.operands[..]) {
            (Kind::Op("LD16rm"), [Operand::Def(d), Operand::Use(p), Operand::Def(_)]) => {
                (self.register(*d), self.register(*p))
            }
            (Kind::Op("LD16mr"), [Operand::Use(s), Operand::Use(p)]) => (self.register(*s), self.register(*p)),
            _ => return Err(Refused::Not),
        };
        if pointer != reg::HL {
            return Err(Refused::Not);
        }
        // "If the base register overlaps a source/destination register, we
        // can't merge the update."
        if ![reg::BC, reg::DE].contains(&data) {
            return Err(Refused::Because("data in hl"));
        }
        // the home pair: the nearest copy into `hl` before the access, with
        // `hl` unwritten since; `hl` made by anything else (a reload, a
        // constant, an add) is the pointer itself
        let mut home = None;
        for earlier in insts[..at].iter().rev().take(UPDATE_LIMIT) {
            if let Some((to, from)) = self.copy_of(earlier) {
                if to == reg::HL {
                    home = [reg::BC, reg::DE].contains(&from).then_some(from);
                    break;
                }
            }
            if earlier.call || earlier.barrier || self.writes(earlier, reg::HL) {
                break;
            }
        }
        if home == Some(data) {
            return Err(Refused::Because("data is the home pair"));
        }
        Ok(Access { home })
    }

    /// `findMatchingUpdateInsnForward`: the update after `block[at]`, along the
    /// block and the single successor the base is live into.
    fn update_after(
        &self,
        block: usize,
        at: usize,
        access: &Access,
    ) -> Result<Update, &'static str> {
        let base = access.home.unwrap_or(reg::HL);
        let (mut here, mut from, mut count) = (block, at + 1, 0);
        let mut seen = vec![block];
        loop {
            let insts = &self.function.blocks[here].insts;
            for index in from..insts.len() {
                let inst = &insts[index];
                if count >= UPDATE_LIMIT {
                    return Err("past the scan limit");
                }
                count += 1;
                if self.touches(inst, base) {
                    // the first use of the base must be the whole of the step
                    return self.matching_update(here, index, access);
                }
                // "If the base register is used or modified, we have no match":
                // a call may do either
                if inst.call || inst.barrier {
                    return Err("a call or a jump before the update");
                }
            }
            if count >= UPDATE_LIMIT {
                return Err("past the scan limit");
            }
            // "Try to go downward to successors along a CF path w/o side enters
            // such that BaseReg is alive along it but not at its
            // exits."
            let mut next = None;
            let mut live_successors = 0;
            for &successor in &self.cfg.succs[here] {
                if self.live_in(successor, base) {
                    live_successors += 1;
                    if live_successors > 1 {
                        return Err("the base is live into two successors");
                    }
                    if self.cfg.preds[successor].len() == 1 && !seen.contains(&successor) {
                        next = Some(successor);
                    }
                }
            }
            let Some(successor) = next else { return Err("no update in the block") };
            seen.push(successor);
            (here, from) = (successor, 0);
        }
    }

    /// `isMatchingUpdateInsn`: the use of the base at `block[index]` is the
    /// step of it by the access size.
    fn matching_update(
        &self,
        block: usize,
        index: usize,
        access: &Access,
    ) -> Result<Update, &'static str> {
        let insts = &self.function.blocks[block].insts;
        let gone = match access.home {
            // the pointer is `hl` itself: the step is the next use of `hl`
            None => self.step_at(block, index)?,
            // the update copies the home pair into `hl`, steps it, and copies
            // it back
            Some(home) => {
                if self.copy_of(&insts[index]) != Some((reg::HL, home)) {
                    return Err("home pair read between");
                }
                let mut at = index + 1;
                if matches!(insts.get(at).map(|inst| inst.kind), Some(Kind::Op("LD16ri"))) {
                    at += 1;
                }
                let mut gone = self.step_at(block, at)?;
                gone.push(index);
                const SHAPE: &str = "the update does not copy back to the home pair";
                let out = insts.get(at + 1).ok_or(SHAPE)?;
                if self.copy_of(out) != Some((home, reg::HL)) {
                    return Err(SHAPE);
                }
                // `hl` after the update is not read
                if self.live_after(block, at + 1, reg::HL) {
                    return Err("hl is read after the update");
                }
                gone.push(at + 1);
                gone.sort_unstable();
                gone
            }
        };
        Ok(Update { block, gone })
    }

    /// The step of `hl` at `i` (`INC16x2 hl`, or `add hl, t` with `t` made by a
    /// `ld t, #2`): the instructions to delete.
    fn step_at(
        &self,
        block: usize,
        i: usize,
    ) -> Result<Vec<usize>, &'static str> {
        const SHAPE: &str = "the next use of hl is not a step of two";
        let insts = &self.function.blocks[block].insts;
        let inst = insts.get(i).ok_or(SHAPE)?;
        match inst.kind {
            Kind::Op("INC16x2") => match &inst.operands[..] {
                [Operand::Def(d), Operand::Use(u)] if self.register(*d) == reg::HL && self.register(*u) == reg::HL => {
                    Ok(vec![i])
                }
                _ => Err(SHAPE),
            },
            Kind::Op("ADD16") => {
                let [Operand::Def(d), Operand::Use(a), Operand::Use(t)] = &inst.operands[..] else {
                    return Err(SHAPE);
                };
                if self.register(*d) != reg::HL || self.register(*a) != reg::HL {
                    return Err(SHAPE);
                }
                self.add_of_two(block, i, self.register(*t))
            }
            _ => Err(SHAPE),
        }
    }

    /// `isMatchingMovConstInsn`'s role: the `add hl, t` at `i` adds 2 (`t` made
    /// by a `ld t, #2` at most `CONST_LIMIT` before it), and its carry is
    /// dead. The `ld` goes too when nothing else reads `t`.
    fn add_of_two(
        &self,
        block: usize,
        i: usize,
        t: RegId,
    ) -> Result<Vec<usize>, &'static str> {
        const NO_CONSTANT: &str = "the constant of the add is not a ld nearby";
        let insts = &self.function.blocks[block].insts;
        // the carry the add sets is dead where a later instruction sets the
        // flags before any reads them; a block that ends first leaves
        // it unknown
        let dead = insts[i + 1..]
            .iter()
            .find(|one| one.reads_flags() || one.sets_flags())
            .is_some_and(|one| one.sets_flags() && !one.reads_flags());
        if !dead {
            return Err("the carry of the add is read");
        }
        let (mut shared, mut made) = (false, None);
        for at in (0..i).rev().take(CONST_LIMIT) {
            let inst = &insts[at];
            if let (Kind::Op("LD16ri"), [Operand::Def(d), Operand::Imm(k)]) = (inst.kind, &inst.operands[..]) {
                if self.register(*d) == t {
                    made = Some((at, *k));
                    break;
                }
            }
            if inst.call || self.writes(inst, t) {
                return Err(NO_CONSTANT);
            }
            shared |= self.reads(inst, t);
        }
        let Some((at, k)) = made else { return Err(NO_CONSTANT) };
        if k != 2 {
            return Err("the add is not a step of two");
        }
        let mut gone = vec![i];
        if !shared && !self.live_after(block, i, t) {
            gone.push(at);
        }
        gone.sort_unstable();
        Ok(gone)
    }
}

/// `mergeUpdateInsn`: the access becomes its post-increment form, the update
/// goes, and a home pair gets a copy of `hl` after the access.
fn merge_update(
    function: &mut MFunction,
    block: usize,
    at: usize,
    access: &Access,
    update: Update,
) {
    let hl = Reg::Phys(reg::HL);
    let old = function.blocks[block].insts[at].clone();
    let mut merged = old.clone();
    merged.operands = match (old.kind, &old.operands[..]) {
        (Kind::Op("LD16rm"), [Operand::Def(d), Operand::Use(p), Operand::Def(_)]) => {
            merged.kind = Kind::Op("LD16rm_pi");
            vec![Operand::Def(*d), Operand::Def(hl), Operand::Use(*p)]
        }
        (Kind::Op("LD16mr"), [Operand::Use(s), Operand::Use(p)]) => {
            merged.kind = Kind::Op("LD16mr_pi");
            vec![Operand::Def(hl), Operand::Use(*s), Operand::Use(*p)]
        }
        _ => unreachable!("planned from an access"),
    };
    merged.ties = vec![];
    for &gone in update.gone.iter().rev() {
        function.blocks[update.block].insts.remove(gone);
    }
    function.blocks[block].insts[at] = merged;
    if let Some(home) = access.home {
        let mut copy = Inst::new(Kind::Copy, vec![Operand::Def(Reg::Phys(home)), Operand::Use(hl)]);
        copy.cheap_as_move = true;
        function.blocks[block].insts.insert(at + 1, copy);
    }
}

/// `AArch64LoadStoreOpt::optimizeBlock` for the updates: each word access with
/// the step of its pointer folded in. Whether anything changed.
pub fn fold(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    let cfg = Cfg::new(function);
    let mut live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
    let mut changed = false;
    for block in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[block].insts.len() {
            let context = Context { function, assignment, cfg: &cfg, live: &live };
            let found = context.access_at(block, at).and_then(|access| {
                context.update_after(block, at, &access).map(|update| (access, update)).map_err(Refused::Because)
            });
            match found {
                Err(Refused::Not) => at += 1,
                Err(Refused::Because(why)) => {
                    if why_enabled() {
                        eprintln!("post-update refused: {why}");
                    }
                    at += 1;
                }
                Ok((access, update)) => {
                    if why_enabled() {
                        eprintln!("post-update merged");
                    }
                    merge_update(function, block, at, &access, update);
                    at += 1 + usize::from(access.home.is_some());
                    live = PhysLiveness::of::<Z80Registers>(function, assignment, &REGISTER_INFO);
                    changed = true;
                }
            }
        }
    }
    changed
}
