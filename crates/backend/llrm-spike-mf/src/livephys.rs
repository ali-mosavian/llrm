//! Which physical registers are live at a point of an allocated function: the
//! answer `LivePhysRegs` and `MachineBasicBlock::computeRegisterLiveness` give
//! a pass that wants a scratch register after allocation.
//!
//! A register is a set of lanes of its root (the description's `lanes`), so a
//! byte of a pair is live when its lane is, and a write of one byte leaves the
//! other live.
//!
//! - [`LiveLanes::step_backward`] is `LivePhysRegs::stepBackward`
//!   ([LivePhysRegs.cpp#L68]): the defs are removed, a call's clobbers with
//!   them (`removeRegsInMask`, [L31]), then the uses are added.
//! - [`PhysLiveness::live_before`] starts from the block's live-outs, the union
//!   of its successors' live-ins (`addBlockLiveOuts`, [L179]) and steps back
//!   over the instructions after the one asked about.
//! - [`LiveLanes::is_live`] is the negation of `LivePhysRegs::available`
//!   ([L141]), without the reserved test, which the caller makes with
//!   `Target::reserved`.
//!
//! DEVIATE: lanes where the pin keeps register units (`LiveRegUnits`) or
//! registers with their aliases: a write of `b` does not leave `bc` counted
//! live, so `available(b)` is true where `LivePhysRegs` says false. Reason: the
//! Z80, 6502 and m68k descriptions give lanes, not units. Exit: none.
//!
//! DEVIATE: the live-ins are solved here, over the allocated function, where
//! the pin keeps them on the blocks after allocation (`addLiveIns`). Reason:
//! the spike machine function has no live-in lists. Exit: the neutral-LIR task.
//!
//! DEVIATE: [`PhysLiveness::live_before`] answers from checkpoints kept every
//! [`STRIDE`] instructions of a block, built on the first query of the block,
//! so a pass that asks about every instruction of a block steps back at most
//! `STRIDE` instructions a query, not to the block's end (a block of 6000
//! instructions took 140 M steps for 800 call sites). The pin's users keep a
//! running `LivePhysRegs` per block instead (`stepForward`). Reason: the
//! callers here ask at random positions. Exit: none.
//!
//! DEVIATE: `MachineBasicBlock::computeRegisterLiveness`
//! ([MachineBasicBlock.cpp#L1706]) scans a bounded neighbourhood and answers
//! unknown beyond it; this answers exactly, from the solved live-ins. Reason:
//! the whole function is at hand. Exit: none.
//!
//! [MachineBasicBlock.cpp#L1706]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineBasicBlock.cpp#L1706
//! [LivePhysRegs.cpp#L68]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LivePhysRegs.cpp#L68
//! [L31]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LivePhysRegs.cpp#L31
//! [L179]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LivePhysRegs.cpp#L179
//! [L141]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LivePhysRegs.cpp#L141

use llrm_lir::registers::{Info, RegId};
use crate::mfunc::{Inst, MFunction, Operand, Reg};
use crate::regalloc::{Assignment, Target, successors};

/// The live lanes of each root register, by root. A register file has a few
/// dozen roots and few are live at once: a sorted list is searched and copied
/// faster than a hash map.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveLanes(Vec<(RegId, i64)>);

impl LiveLanes {
    fn find(
        &self,
        root: RegId,
    ) -> Result<usize, usize> {
        self.0.binary_search_by_key(&root, |(one, _)| *one)
    }

    /// The lanes of `root`, made an entry (of none) where it has none.
    fn entry(
        &mut self,
        root: RegId,
    ) -> &mut i64 {
        let at = match self.find(root) {
            Ok(at) => at,
            Err(at) => {
                self.0.insert(at, (root, 0));
                at
            }
        };
        &mut self.0[at].1
    }

    /// `addReg`.
    pub fn add(
        &mut self,
        info: &Info,
        register: RegId,
    ) {
        *self.entry(info.root(register)) |= info.lanes(register);
    }

    /// `removeReg`.
    pub fn remove(
        &mut self,
        info: &Info,
        register: RegId,
    ) {
        let root = info.root(register);
        if let Ok(at) = self.find(root) {
            self.0[at].1 &= !info.lanes(register);
            if self.0[at].1 == 0 {
                self.0.remove(at);
            }
        }
    }

    /// Whether any lane of `register` is live: not `available`.
    pub fn is_live(
        &self,
        info: &Info,
        register: RegId,
    ) -> bool {
        self.find(info.root(register)).is_ok_and(|at| self.0[at].1 & info.lanes(register) != 0)
    }

    fn union(
        &mut self,
        other: &LiveLanes,
    ) -> bool {
        let mut changed = false;
        for (root, lanes) in &other.0 {
            let mine = self.entry(*root);
            if *mine | lanes != *mine {
                *mine |= lanes;
                changed = true;
            }
        }
        changed
    }

    /// `stepBackward`: `inst` is stepped over, so the set is what is live
    /// before it given what was live after.
    pub fn step_backward<T: Target>(
        &mut self,
        info: &Info,
        assignment: &Assignment,
        inst: &Inst,
    ) {
        let register = |reg: Reg| match reg {
            Reg::Virt(v) => assignment.0[v as usize],
            Reg::Phys(id) => id,
        };
        for operand in &inst.operands {
            if let Operand::Def(reg) = operand {
                self.remove(info, register(*reg));
            }
        }
        if inst.call {
            let clobbered =
                inst.call_clobbers.as_ref().map_or_else(T::caller_saved, |known| known.iter().copied().collect());
            for root in clobbered {
                self.remove(info, root);
            }
        }
        for (index, operand) in inst.operands.iter().enumerate() {
            if let Operand::Use(reg) = operand {
                self.add(info, T::read_part(inst, index, register(*reg)));
            }
        }
    }
}

thread_local! {
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The instructions [`PhysLiveness::of`] has stepped over on this thread since
/// the last call.
pub fn take_steps() -> usize {
    STEPS.with(std::cell::Cell::take)
}

/// How many instructions apart a block's checkpoints are.
const STRIDE: usize = 16;

/// A block's live sets every [`STRIDE`] instructions from its end: entry `j`
/// is what is live before instruction `len - j * STRIDE` (the live-out first).
struct Checkpoints {
    len: usize,
    sets: Vec<LiveLanes>,
}

/// The registers live out of each block of an allocated function.
pub struct PhysLiveness {
    live_out: Vec<LiveLanes>,
    checkpoints: std::cell::RefCell<Vec<Option<Checkpoints>>>,
}

thread_local! {
    /// Functions [`PhysLiveness::of`] solved on this thread since the last [`take_solved`].
    static SOLVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The functions solved on this thread since the last call.
pub fn take_solved() -> usize {
    SOLVED.with(|one| one.replace(0))
}

impl PhysLiveness {
    /// Solves the live-outs of every block: a block's live-in is its live-out
    /// stepped back over its instructions.
    pub fn of<T: Target>(
        function: &MFunction<T::Class>,
        assignment: &Assignment,
        info: &Info,
    ) -> PhysLiveness {
        SOLVED.with(|one| one.set(one.get() + 1));
        let succs = successors(function);
        let blocks = function.blocks.len();
        let mut preds: Vec<Vec<usize>> = vec![Vec::new(); blocks];
        for (b, out) in succs.iter().enumerate() {
            for &s in out {
                preds[s].push(b);
            }
        }
        let mut live_in = vec![LiveLanes::default(); blocks];
        let mut live_out = vec![LiveLanes::default(); blocks];
        // Last block first, as a sweep would; a block is solved again only when
        // a successor's live-in changed, so an acyclic function is stepped
        // once.
        let mut queue: std::collections::VecDeque<usize> = (0..blocks).rev().collect();
        let mut queued = vec![true; blocks];
        while let Some(b) = queue.pop_front() {
            queued[b] = false;
            let mut out = LiveLanes::default();
            for &s in &succs[b] {
                out.union(&live_in[s]);
            }
            let mut live = out.clone();
            STEPS.with(|count| count.set(count.get() + function.blocks[b].insts.len()));
            for inst in function.blocks[b].insts.iter().rev() {
                live.step_backward::<T>(info, assignment, inst);
            }
            let moved = live != live_in[b];
            if out != live_out[b] || moved {
                live_out[b] = out;
                live_in[b] = live;
                if moved {
                    for &p in &preds[b] {
                        if !queued[p] {
                            queued[p] = true;
                            queue.push_back(p);
                        }
                    }
                }
            }
        }
        let solved =
            PhysLiveness { live_out, checkpoints: std::cell::RefCell::new((0..blocks).map(|_| None).collect()) };
        // `LLRM_VERIFY=1`: the answer of the sweeps, which the worklist must
        // equal
        if llrm_support::env_flag!("LLRM_VERIFY") {
            let swept = Self::of_by_sweeps::<T>(function, assignment, info);
            assert!(
                solved.live_out == swept.live_out,
                "livephys: the worklist's live-outs differ from the sweeps' in @{}",
                function.name
            );
        }
        solved
    }

    /// [`Self::of`] by sweeps over every block until none moves: the reference
    /// the worklist is checked against (`LLRM_VERIFY`, the tests).
    pub fn of_by_sweeps<T: Target>(
        function: &MFunction<T::Class>,
        assignment: &Assignment,
        info: &Info,
    ) -> PhysLiveness {
        let succs = successors(function);
        let blocks = function.blocks.len();
        let mut live_in = vec![LiveLanes::default(); blocks];
        let mut live_out = vec![LiveLanes::default(); blocks];
        let mut changed = true;
        while changed {
            changed = false;
            for b in (0..blocks).rev() {
                let mut out = LiveLanes::default();
                for &s in &succs[b] {
                    out.union(&live_in[s]);
                }
                let mut live = out.clone();
                for inst in function.blocks[b].insts.iter().rev() {
                    live.step_backward::<T>(info, assignment, inst);
                }
                if out != live_out[b] || live != live_in[b] {
                    live_out[b] = out;
                    live_in[b] = live;
                    changed = true;
                }
            }
        }
        PhysLiveness { live_out, checkpoints: std::cell::RefCell::new((0..blocks).map(|_| None).collect()) }
    }

    /// What is live out of block `b`.
    pub fn live_out_of(
        &self,
        b: usize,
    ) -> &LiveLanes {
        &self.live_out[b]
    }

    /// What is live just before instruction `at` of block `b`. The block's
    /// instructions from `at` on are the ones asked about, so a change before
    /// `at` (not of the block's length) leaves the answer as it was.
    pub fn live_before<T: Target>(
        &self,
        function: &MFunction<T::Class>,
        assignment: &Assignment,
        info: &Info,
        b: usize,
        at: usize,
    ) -> LiveLanes {
        let insts = &function.blocks[b].insts;
        let mut checkpoints = self.checkpoints.borrow_mut();
        let stale = checkpoints[b].as_ref().is_none_or(|kept| kept.len != insts.len());
        if stale {
            let mut sets = vec![self.live_out[b].clone()];
            let mut live = self.live_out[b].clone();
            for (stepped, inst) in insts.iter().rev().enumerate() {
                live.step_backward::<T>(info, assignment, inst);
                if (stepped + 1) % STRIDE == 0 {
                    sets.push(live.clone());
                }
            }
            checkpoints[b] = Some(Checkpoints { len: insts.len(), sets });
        }
        let kept = checkpoints[b].as_ref().expect("built above");
        let nearest = (insts.len() - at) / STRIDE;
        let mut live = kept.sets[nearest].clone();
        for inst in insts[at..insts.len() - nearest * STRIDE].iter().rev() {
            live.step_backward::<T>(info, assignment, inst);
        }
        // a change after `at` that keeps the block's length is not seen by the
        // checkpoints: checked in full on short blocks, on every 4096th
        // position of a long one
        #[cfg(debug_assertions)]
        if insts.len() <= 512 || at % 4096 == 0 {
            let mut slow = self.live_out[b].clone();
            for inst in insts[at..].iter().rev() {
                slow.step_backward::<T>(info, assignment, inst);
            }
            assert_eq!(live, slow, "live_before: block {b} changed after its liveness was solved");
        }
        live
    }
}
