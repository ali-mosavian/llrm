//! Early `MachineLICM`: a pure computation of values defined outside a loop,
//! made in the loop, is made once in its preheader. Translated from
//! [MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp)
//! (`HoistOutOfLoop` L827, `IsLICMCandidate` L1098, `IsLoopInvariantInst`
//! L1133, `IsProfitableToHoist` L1273, `CanCauseHighRegPressure` L1235,
//! `Hoist` L1615).
//!
//! # How it works
//! The loops come inner first. A loop with one block outside it that enters
//! the header, and that block leaving only for the header (its preheader),
//! is walked in layout order, and again until nothing moves. An instruction
//! moves to the end of the preheader, before its jump, when:
//!
//! - it is a pure operation on registers and constants with one result, no
//!   pinned operand, no call, no x87 value (`candidate`), an `fld1`/`fldz` in a
//!   loop with no call and a free stack slot, in a function that never writes
//!   the control word (`constant_load`), or a load in a block that runs on
//!   every trip of a loop with no store, call or volatile access
//!   (`InitializeLoadsHoistableLoops` L1472, `IsGuaranteedToExecute`); DEVIATE
//!   ML2: an `fld1`/`fldz` and its `fchs` also move where the function writes
//!   the control word, when the loop is innermost and only arithmetic reads
//!   them (below);
//! - every value it reads is defined outside the loop or was hoisted
//!   (`IsLoopInvariantInst`);
//! - it is not as cheap as a move (`IsProfitableToHoist`: a cheap instruction
//!   is hoisted only if it does not raise the pressure, and a constant cannot
//!   be made again where the allocator wants it, DEVIATE E3 of `precse`);
//! - it sets no flags a later instruction of its block reads;
//! - the loop does not already use as many values of its register class as the
//!   target has registers (`CanCauseHighRegPressure`, with the values named in
//!   the loop standing for the pressure: DEVIATE ML1, the pin tracks the live
//!   registers of each block of the loop; a block that does not run on every
//!   trip also needs the loop's peak of live registers below that,
//!   `AvoidSpeculation`).
//!
//! Not ported, priced at no yield (release/next 34d21c9f34, every C, nib and
//! BASIC program at -Os and -O2, counters in a throwaway build):
//! `getOrCreatePreheader` splitting an edge: the 6 to 32 loops per language
//! and level without a preheader hold 0 hoistable instructions;
//! `ExtractHoistableLoad`'s pricing held for the C, nib and BASIC corpus (no
//! scalar SSE) and is wrong for SSE code: nbody_single -O3 m64 left
//! `mulss xmm, [rip+K]` in a loop where the pin has the constant in a register
//! before it. It is ported (`Policy::unfold`): scalar SSE arithmetic that reads
//! a constant-pool cell is split, and the load moves if it can.
//!
//! `MayCSE`: only reached in the pin's high-pressure branch for an instruction
//! outside a block that runs every trip, where the `constant` path above
//! already hoists what the allocator can make again.
//!
//! DEVIATE ML2 (owner x87, exit 2026-12-08), 11-08 re-check: the pin keeps
//! `fld1; fchs` inside the loops of x_ld_abs -O2 (`-march=i486`, the pinned
//! clang's listing) and gcc carries the constant on the stack (`fsub st, st(1)`),
//! so neither exit holds. The pin's `FLD1` and `FLDZ` read `$fpcw`,
//! which the function's `FLDCW` (for an `fptosi`) writes, so nothing moves and
//! `fld1; fchs; faddp` (18 clk) is made on every trip of a loop that adds
//! `-1.0`. Both and `fchs` are exact under any rounding control. A constant
//! that an `fchs` flips moves, with the flip, when the loop is innermost,
//! stores no x87 value, every reader is an arithmetic instruction beside a
//! register (a compare wants the constant on top, a memory operand makes the
//! held value be copied: `fxch`es and `fld st(i)` cost what the hoist saves)
//! and the stack has room (`CARRY_RESERVE`); not at -Os, where the `fstp` that
//! ends the carried value costs 2 B (as X1). A plain `fld1`/`fldz` stays: it
//! saves 4 clk and the loop's back edge pays `fxch`es (recfloat, +8%).
//! x_ld_abs -O2 13,320 -> 12,693 clk (gcc 13,113) at 80 bits, 12,684 ->
//! 12,057 at 64; double x_horner -O2 92,084 -> 83,304. The exit: does the
//! pin's `isConstantPhysReg` stop counting `FLD1`/`FLDZ`, or gcc's reg-stack
//! keep the same cells.
//!
//! # Example
//! A loop that adds `a[i] * k3` where `k3 := k + 3` is made inside it (the
//! sum is made by instruction selection, after the IR's LICM has run). The
//! sum is made once, ahead of the loop:
//!
//! ```text
//!   pre:                              pre:
//!     jmp loop                          v9 := add k, 0x3
//!   loop:                       =>      jmp loop
//!     v9 := add k, 0x3                loop:
//!     v10 := imul v8, v9                v10 := imul v8, v9
//! ```
//!
//! # Runs
//! A machine phase before `precse` (`flow::machine_with`), at every -O level
//! but -O0.

use std::rc::Rc;
use std::sync::Arc;

use crate::backend::classes::RegisterClasses;
use crate::backend::licm_core as licm;
use crate::backend::liveness;
use crate::backend::regpressure::Model;
use crate::backend::target::Segments;
use crate::model::ir::{Loc, Mem, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

pub struct MachineLicm {
    pub classes: Rc<RegisterClasses>,
    /// The segment registers, to find each value's register class for the
    /// pressure it counts.
    pub segments: Segments,
    /// -Os, -Oz: a carried constant costs the `fstp` that ends it (DEVIATE X1
    /// is off).
    pub size: bool,
}

impl LIRTransform for MachineLicm {
    fn class_name(&self) -> &'static str {
        "MachineLICM"
    }

    fn name(&self) -> &str {
        "machinelicm"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(hoisted_sized(&body, &self.classes, self.size, &self.segments))
    }
}

/// The cell `one` loads into a value, where it is a plain load.
fn loaded(one: &Insn) -> Option<&Mem> {
    let what = one.what.as_ref()?;
    match (what.op, what.dests.as_slice(), what.sources.as_slice()) {
        (Operation::Move, [Loc::Held(_)], [Loc::Mem(cell)]) => Some(cell),
        _ => None,
    }
}

/// `isLoadFoldBarrier() || mayStore() || isCall() || hasOrderedMemoryRef()`:
/// what keeps every load of the loop in it (`InitializeLoadsHoistableLoops`).
pub(crate) fn keeps_loads(one: &Insn) -> bool {
    if one.is_meta() && one.what.as_ref().is_some_and(|what| what.op != Operation::Barrier) {
        return false;
    }
    crate::backend::machinesink::may_store(one)
        || one.what.as_ref().is_none_or(|what| {
            matches!(
                what.op,
                Operation::Push | Operation::Pop | Operation::Leave | Operation::Exchange | Operation::Nothing
            )
        })
}

/// `fld1` / `fldz`: an x87 constant, made again where it is read. The pin
/// hoists it (`LD_Fp164` is not as cheap as a move); the value then stays on
/// the stack across the loop. It reads `$fpcw`, which `IsLoopInvariantInst`
/// accepts only where nothing in the function writes it: a `fistp` that
/// truncates (`fisttp` here) writes it with `FLDCW`.
fn constant_load(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        what.op == Operation::FloatLoad
            && matches!(what.name.as_deref(), Some("fld1" | "fldz"))
            && what.sources.is_empty()
            && matches!(what.dests.as_slice(), [Loc::Held(_)])
    }) && one.defines.len() == 1
        && one.uses.is_empty()
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
}

/// `fld m80` of a constant-pool entry (`Insn::invariant`): the value is made
/// again on every trip, where gcc's reg-stack leaves it on the x87 stack.
/// DEVIATE X1 (owner x87, exit 2026-12-08), 11-08 re-check: for `s += a[i] * 0.1L + 0.3L`
/// the pinned clang loads both m80 constants every trip, gcc carries one and ours
/// both. The load reads `$fpcw`, which the pin's
/// `IsLoopInvariantInst` accepts only where nothing in the function writes it
/// (`fisttp` here does); `fld` of an extended constant does not depend on the
/// rounding control, so it moves regardless and the stack carries it as it
/// carries `fldz`. Only the 10-byte entries: a dword or qword one folds into
/// its reader (`fmul m64`, `fcomp m32`), and carried it costs an `fxch` each
/// trip to put the other operand where the compare wants it (x_ld_clamp +1,304
/// clk).
fn pool_load(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        what.op == Operation::FloatLoad
            && what.name.as_deref() == Some("fld")
            && matches!(
                what.sources.as_slice(),
                [Loc::Mem(cell)] if cell.selector.is_none() && !cell.stack_argument && cell.width == 10
            )
            && matches!(what.dests.as_slice(), [Loc::Held(_)])
    }) && one.invariant()
        && one.defines.len() == 1
        && one.uses.is_empty()
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
}

/// Stack slots a loop keeps free of hoisted constants for its own temporaries
/// (an operation's two operands and a result).
const CARRY_RESERVE: usize = 3;

/// `fchs`: the sign flip of an x87 value, exact under any rounding control.
/// Moves with the constant it flips (`IsLoopInvariantInst` has every value it
/// reads defined outside the loop or hoisted).
fn sign_flip(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| {
        what.op == Operation::FloatUnary
            && what.name.as_deref() == Some("fchs")
            && matches!(what.sources.as_slice(), [Loc::Held(_)])
            && matches!(what.dests.as_slice(), [Loc::Held(_)])
    }) && one.defines.len() == 1
        && one.uses.len() == 1
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
}

/// The loop has no loop inside it: one block it holds is the target of a back
/// edge. A value held across an inner loop costs stack depth there for a saving
/// once per outer trip.
fn innermost(facts: &licm::Facts<'_, Wrapped>) -> bool {
    facts
        .inside_blocks()
        .flat_map(|block| block.succ.iter().filter(move |&&to| to <= block.at))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        <= 1
}

/// The instructions of the loop that read `value`.
fn reads<'a>(
    facts: &'a licm::Facts<'_, Wrapped>,
    value: u32,
) -> impl Iterator<Item = &'a Wrapped> {
    facts.inside_blocks().flat_map(|block| block.insns.iter()).filter(move |one| one.0.uses.contains(&value))
}

/// Every read of `value` in the loop is an x87 arithmetic operand beside a
/// register, directly or through a sign flip: held on the stack it is `st(i)`
/// of the operation. A compare wants it on top, so holding it costs an `fxch`
/// (4 clocks) where `fldz` is 4.
fn read_by_arithmetic(
    facts: &licm::Facts<'_, Wrapped>,
    value: u32,
) -> bool {
    // Two registers: with a memory operand the constant is copied (`fld st(0)`,
    // 4 clk, as `fld1` is) and the hoist saves nothing.
    let arithmetic = |one: &Insn| {
        one.what
            .as_ref()
            .is_some_and(
                |what| (matches!(what.op, Operation::FloatArith | Operation::FloatArithPop)
                    || matches!(
                        what.name.as_deref(),
                        Some("fadd" | "fsub" | "fsubr" | "fmul" | "fdiv" | "fdivr")
                    ))
                    && what.sources.iter().all(|source| matches!(source, Loc::Held(_))),
            )
    };
    reads(facts, value).all(|one| {
        arithmetic(&one.0)
            || sign_flip(&one.0) && one.0.defines.first().is_some_and(|&flipped| read_by_arithmetic(facts, flipped))
    })
}

/// `IsLICMCandidate` for what this pass moves. `loads`: the loop has nothing
/// that keeps a load in it, and the load's block runs on every trip.
fn candidate(
    one: &Insn,
    classes: &RegisterClasses,
    loads: bool,
) -> bool {
    let Some(what) = &one.what else { return false };
    if !classes.requirements(what).is_empty() {
        return false;
    }
    let load = (loads || one.invariant()) && loaded(one).is_some();
    let pure = load
        || matches!(
            what.op,
            Operation::Address
                | Operation::Extend
                | Operation::Binary
                | Operation::Unary
                | Operation::Multiply
                | Operation::Funnel
        );
    // `lea` names a cell and reads none (LEA32r is not `mayLoad`).
    let cell = |cell: &Mem| (what.op == Operation::Address || load) && cell.selector.is_none() && !cell.stack_argument;
    let held = |place: &Loc| match place {
        Loc::Held(_) | Loc::Imm(_) | Loc::Address(_) => true,
        Loc::Mem(one) => cell(one),
        _ => false,
    };
    pure && what.target.is_none()
        && !what.indirect
        && (!one.uses.is_empty() || load && one.invariant())
        && one.defines.len() == 1
        && matches!(what.dests.as_slice(), [Loc::Held(_)])
        && what.sources.iter().all(held)
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
        && one.clobbers.is_empty()
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty()
        && !crate::backend::machinesink::as_cheap_as_a_move(one, classes.cheap_move_immediate)
}

/// An x86 instruction as the shared pass reads it.
#[derive(Clone)]
struct Wrapped(Arc<Insn>);

impl licm::Inst for Wrapped {
    fn defines(&self) -> &[u32] {
        &self.0.defines
    }

    fn uses(&self) -> &[u32] {
        &self.0.uses
    }

    fn each_value(
        &self,
        each: impl FnMut(u32),
    ) {
        self.0.each_value(each);
    }

    fn is_copy(&self) -> bool {
        self.0
            .what
            .as_ref()
            .is_some_and(
                |what| what.op == Operation::Move
                    && matches!(
                        (what.dests.as_slice(), what.sources.as_slice()),
                        ([Loc::Held(_)], [Loc::Held(_)])
                    ),
            )
    }

    fn is_move(&self) -> bool {
        self.0.what.as_ref().is_some_and(|what| what.op == Operation::Move)
    }

    fn is_meta(&self) -> bool {
        self.0.is_meta()
    }

    fn is_jump(&self) -> bool {
        self.0.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Jump | Operation::Branch))
    }

    /// `isIdenticalTo` with the definitions ignored: the same operation over
    /// the same values and operands, defining registers of the same widths.
    fn identical(
        &self,
        other: &Self,
    ) -> bool {
        let (Some(one), Some(two)) = (&self.0.what, &other.0.what) else { return false };
        // DEVIATE X2 (owner m64, exit 2026-12-20): an x87 value is a stack
        // entry and the allocator gives each loop-carried entry its own
        // definition; the pin's RFP registers are virtual until the stackifier
        // (X86FloatingPoint), which copes with one register feeding two. Two
        // `fldz` stay two values. Removed (2026-11-08): no object of 80 x87
        // size cells (m32, m64, O2/Os) changes and 52 run outputs are equal, so
        // the corpus never holds two equal hoistable x87 constants; the unit
        // test below differs (the merge is real), and no program shows the
        // stackifier coping. Delete once one does.
        if one.dests.iter().any(|place| matches!(place, Loc::Held(held) if held.width == 10)) {
            return false;
        }
        let widths = |what: &Semantics| -> Vec<Option<u32>> {
            what.dests.iter().map(|place| if let Loc::Held(held) = place { Some(held.width) } else { None }).collect()
        };
        one.op == two.op
            && one.name == two.name
            && one.sources == two.sources
            && widths(one) == widths(two)
            && self.0.uses == other.0.uses
            && self.0.defines.len() == other.0.defines.len()
    }

    fn is_plain_load(&self) -> bool {
        loaded(&self.0).is_some() && !self.0.invariant()
    }

    fn rename_use(
        &mut self,
        from: u32,
        to: u32,
    ) {
        #[cfg(test)]
        RENAMES.with(|renames| renames.set(renames.get() + 1));
        self.0 = self.0.renamed(&|value| if value == from { to } else { value });
    }
}

#[cfg(test)]
thread_local! {
    /// Calls of `Wrapped::rename_use`.
    static RENAMES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The x86 half of the pass: which instructions move (`IsLICMCandidate`,
/// `IsProfitableToHoist`, the x87 constants, the flags).
/// `MachineLICM`'s register pressure (`InitRegPressure` L933,
/// `UpdateRegPressure` L952, `calcRegisterCost` L970, `CanCauseHighRegPressure`
/// L1235, `UpdateBackTraceRegPressure` L1259 of the pin's
/// [MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp)):
/// the weight each value of a class adds to the pressure sets of the class,
/// counted over the loop's preheader and its blocks in dominator order. A
/// value counts where it is defined, where it is read first (a live-in, in
/// the preheader), and not again where the read is its last.
///
/// DEVIATE ML1, exit 2026-11-20 (owner x86-m32): the pin's walk is of the
/// instructions as `Hoist` leaves them; here the pressure of a loop is counted
/// ahead of its hoists, and again after each (a hoisted instruction is left
/// out and its cost added to every block: `UpdateBackTraceRegPressure`). The
/// children of a dominator node are taken in layout order, and a value's last
/// read is the only read in the function (`hasOneNonDBGUse`), not a kill flag.
struct Pressure {
    model: Model,
    /// How many operands read each value, in the function.
    reads: HashMap<u32, u32>,
    sets: usize,
    /// For each block of the loop, the most each set has been at the entry of
    /// the blocks that dominate it and of itself (`BackTrace`).
    path: HashMap<i64, Vec<i32>>,
    /// What the loop's hoists add to every block
    /// (`UpdateBackTraceRegPressure`).
    added: Vec<i32>,
    dirty: bool,
}

impl Pressure {
    fn new(
        body: &LirBody,
        model: Model,
    ) -> Self {
        let mut reads: HashMap<u32, u32> = HashMap::default();
        for block in &body.blocks {
            for one in &block.insns {
                for &used in &one.uses {
                    *reads.entry(used).or_default() += 1;
                }
            }
            for phi in &block.phis {
                for (_, value) in &phi.incoming {
                    *reads.entry(*value).or_default() += 1;
                }
            }
        }
        let sets = model.limits.len();
        Self { model, reads, sets, path: HashMap::default(), added: vec![0; sets], dirty: true }
    }

    /// A loop is about to be walked.
    fn begin(&mut self) {
        self.added.iter_mut().for_each(|slot| *slot = 0);
        self.dirty = true;
    }

    /// `calcRegisterCost`: what `one` adds to each set. A definition adds its
    /// class's weight. A read that is the value's only one takes it away,
    /// unless `seen` has not met the value (`ConsiderSeen`), when it is a
    /// live-in that adds it for `unseen_as_def`.
    fn cost(
        &self,
        one: &Insn,
        mut seen: Option<&mut HashSet<u32>>,
        unseen_as_def: bool,
    ) -> Vec<i32> {
        let mut cost = vec![0i32; self.sets];
        let mut count = |value: u32, defined: bool, seen: &mut Option<&mut HashSet<u32>>| {
            let Some((weight, sets)) = self.model.weight(value) else { return };
            let new = seen.as_mut().is_some_and(|seen| seen.insert(value));
            let kill = self.reads.get(&value).copied() == Some(1);
            let by = if defined {
                weight as i32
            } else if new && !kill && unseen_as_def {
                weight as i32
            } else if !new && kill {
                -(weight as i32)
            } else {
                0
            };
            if by != 0 {
                for &set in sets {
                    cost[set] += by;
                }
            }
        };
        for &value in &one.defines {
            count(value, true, &mut seen);
        }
        for &value in &one.uses {
            count(value, false, &mut seen);
        }
        cost
    }

    fn update(
        &self,
        pressure: &mut [i32],
        cost: &[i32],
    ) {
        for (at, &weight) in cost.iter().enumerate() {
            if pressure[at] < -weight {
                pressure[at] = 0;
            } else {
                pressure[at] += weight;
            }
        }
    }

    /// `HoistOutOfLoop`'s counting: the preheader, then the loop's blocks down
    /// the dominator tree from the header, the pressure at the entry of each
    /// kept on a stack (`BackTrace`) of which `path` keeps the largest.
    fn rebuild(
        &mut self,
        facts: &licm::Facts<'_, Wrapped>,
    ) {
        self.path.clear();
        self.dirty = false;
        let blocks = facts.blocks;
        let by_at: HashMap<i64, usize> = blocks.iter().enumerate().map(|(number, block)| (block.at, number)).collect();
        let into = |at: i64| blocks.iter().filter(move |block| block.succ.contains(&at));
        let Some(header) = facts
            .members
            .iter()
            .map(|&number| &blocks[number])
            .find(|block| into(block.at).any(|from| !facts.inside.contains(&from.at)))
        else {
            return;
        };
        let Some(preheader) = into(header.at).find(|from| !facts.inside.contains(&from.at)) else { return };
        let mut pressure = vec![0i32; self.sets];
        let mut seen: HashSet<u32> = HashSet::default();
        // `InitRegPressure`: the preheader and, while one block falls into
        // it, that block.
        let mut chain = vec![preheader.at];
        while let [.., last] = chain[..] {
            let from: Vec<i64> = into(last).map(|block| block.at).collect();
            match from[..] {
                [only] if !chain.contains(&only) && blocks[by_at[&only]].succ.len() == 1 => chain.push(only),
                _ => break,
            }
        }
        for &at in chain.iter().rev() {
            for one in &blocks[by_at[&at]].insns {
                let cost = self.cost(&one.0, Some(&mut seen), true);
                self.update(&mut pressure, &cost);
            }
        }
        // The dominator tree of the loop from its header.
        let members: Vec<i64> = facts.members.iter().map(|&number| blocks[number].at).collect();
        let order = reverse_post_order(blocks, &by_at, facts.inside, header.at);
        let position: HashMap<i64, usize> = order.iter().enumerate().map(|(at, &block)| (block, at)).collect();
        let mut idom: HashMap<i64, i64> = HashMap::default();
        idom.insert(header.at, header.at);
        let mut changed = true;
        while changed {
            changed = false;
            for &at in order.iter().skip(1) {
                let mut new: Option<i64> = None;
                for from in into(at).map(|block| block.at).filter(|from| idom.contains_key(from)) {
                    new = Some(match new {
                        None => from,
                        Some(mut left) => {
                            let mut right = from;
                            while left != right {
                                while position[&left] > position[&right] {
                                    left = idom[&left];
                                }
                                while position[&right] > position[&left] {
                                    right = idom[&right];
                                }
                            }
                            left
                        }
                    });
                }
                if let Some(new) = new
                    && idom.get(&at) != Some(&new)
                {
                    idom.insert(at, new);
                    changed = true;
                }
            }
        }
        let mut children: HashMap<i64, Vec<i64>> = HashMap::default();
        for &at in &members {
            if at != header.at
                && let Some(&parent) = idom.get(&at)
            {
                children.entry(parent).or_default().push(at);
            }
        }
        // Depth first, a block's children in layout order.
        let mut stack: Vec<(i64, Vec<i32>)> = Vec::new();
        let mut work = vec![header.at];
        while let Some(at) = work.pop() {
            let parent = idom.get(&at).copied().filter(|_| at != header.at);
            while stack.last().is_some_and(|(top, _)| Some(*top) != parent) {
                stack.pop();
            }
            stack.push((at, pressure.clone()));
            let mut most = vec![0i32; self.sets];
            for (_, snapshot) in &stack {
                for (slot, &value) in most.iter_mut().zip(snapshot) {
                    *slot = (*slot).max(value);
                }
            }
            self.path.insert(at, most);
            for one in &blocks[by_at[&at]].insns {
                let cost = self.cost(&one.0, Some(&mut seen), false);
                self.update(&mut pressure, &cost);
            }
            if let Some(below) = children.get(&at) {
                work.extend(below.iter().rev());
            }
        }
    }

    /// `CanCauseHighRegPressure` for hoisting `one` out of block `at`: some
    /// set that `one` adds to would reach its limit at the entry of a block on
    /// the way down from the header.
    fn high(
        &mut self,
        facts: &licm::Facts<'_, Wrapped>,
        at: i64,
        one: &Insn,
    ) -> bool {
        if self.dirty {
            self.rebuild(facts);
        }
        let Some(path) = self.path.get(&at) else { return false };
        let cost = self.cost(one, None, false);
        cost.iter()
            .enumerate()
            .any(|(set, &weight)| weight > 0 && path[set] + self.added[set] + weight >= self.model.limits[set] as i32)
    }

    /// `UpdateBackTraceRegPressure`: `one` leaves the loop.
    fn hoisted(
        &mut self,
        one: &Insn,
    ) {
        let cost = self.cost(one, None, false);
        for (set, weight) in cost.into_iter().enumerate() {
            self.added[set] += weight;
        }
        self.dirty = true;
    }
}

/// The blocks of the loop in reverse post order from `header`.
fn reverse_post_order(
    blocks: &[licm::Block<Wrapped>],
    by_at: &HashMap<i64, usize>,
    inside: &std::collections::BTreeSet<i64>,
    header: i64,
) -> Vec<i64> {
    let mut order = Vec::new();
    let mut visited: HashSet<i64> = HashSet::default();
    let mut stack: Vec<(i64, usize)> = vec![(header, 0)];
    visited.insert(header);
    while let Some((at, next)) = stack.pop() {
        let succ = &blocks[by_at[&at]].succ;
        if let Some(&to) = succ.get(next) {
            stack.push((at, next + 1));
            if inside.contains(&to) && visited.insert(to) {
                stack.push((to, 0));
            }
        } else {
            order.push(at);
        }
    }
    order.reverse();
    order
}

struct X86<'a> {
    pressure: Pressure,
    /// The values the loop names (`Target::hoist_by_pressure` false).
    named: usize,
    body: &'a LirBody,
    classes: &'a RegisterClasses,
    regs: llrm_lir::registers::Regs,
    floating: std::collections::BTreeSet<u32>,
    control_word_written: bool,
    /// The loop being walked.
    calls: bool,
    floats_named: usize,
    /// The most general registers live at once in the loop, once asked for.
    peak: Option<usize>,
    live_out: Option<crate::backend::liverows::Live>,
    loads: bool,
    size: bool,
    /// The next value an unfolded load defines.
    fresh: u32,
}

impl X86<'_> {
    /// The most general registers live at one point of the loop: the pin's
    /// `RegPressure` after it walked the loop's blocks. A block that does not
    /// run on every trip is not hoisted out of at that pressure
    /// (`AvoidSpeculation`, L1349).
    fn peak_pressure(
        &mut self,
        facts: &licm::Facts<'_, Wrapped>,
    ) -> usize {
        if let Some(peak) = self.peak {
            return peak;
        }
        let body = self.body;
        let live = self.live_out.get_or_insert_with(|| crate::backend::liverows::live_out(body));
        let mut peak = 0;
        for block in facts.inside_blocks() {
            let mut now: std::collections::BTreeSet<u32> = live
                .get(&block.at)
                .map(|set| set.iter().filter(|value| !self.floating.contains(value)).collect())
                .unwrap_or_default();
            peak = peak.max(now.len());
            for one in block.insns.iter().rev() {
                for value in one.0.defines.iter() {
                    now.remove(value);
                }
                now.extend(one.0.uses.iter().filter(|value| !self.floating.contains(value)));
                peak = peak.max(now.len());
            }
        }
        self.peak = Some(peak);
        peak
    }
}

impl licm::Policy<Wrapped> for X86<'_> {
    /// A carried constant is read on every trip, the accumulators beside it as
    /// often: it lies under them on the stack, where `fadd m64` finds the
    /// accumulator on top (to_double: two `fxch` a trip with it above).
    fn leads(
        &self,
        one: &Wrapped,
    ) -> bool {
        pool_load(&one.0)
    }

    /// `ExtractHoistableLoad` ([L1400][unfold]): scalar SSE arithmetic that
    /// reads an invariant cell (a constant-pool entry) beside a register is
    /// the load and the operation on a register (`unfoldMemoryOperand`).
    ///
    /// [unfold]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp#L1400
    fn unfold(
        &mut self,
        one: &Wrapped,
    ) -> Option<(Wrapped, Wrapped)> {
        let insn = &one.0;
        let what = insn.what.as_ref()?;
        let arithmetic = what
            .name
            .as_deref()
            .is_some_and(
                |name| ["add", "sub", "mul", "div", "min", "max"]
                    .iter()
                    .any(|op| name.strip_prefix(op).is_some_and(|rest| rest == "ss" || rest == "sd")),
            );
        let [Loc::Held(dest)] = what.dests.as_slice() else { return None };
        let cells = what.sources.iter().filter(|source| matches!(source, Loc::Mem(_))).count();
        let (at, cell) = what
            .sources
            .iter()
            .enumerate()
            .find_map(
                |(at, source)| match source {
                    Loc::Mem(cell) => Some((at, cell)),
                    _ => None,
                },
            )?;
        if !arithmetic
            || cells != 1
            || !insn.invariant()
            || insn.defines.len() != 1
            || insn.call.is_some()
            || insn.group.is_some()
            || insn.volatile()
            || cell.base.is_some()
            || cell.index.is_some()
            || cell.selector.is_some()
            || cell.stack_argument
        {
            return None;
        }
        let name = match cell.width {
            4 => "movss",
            8 => "movsd",
            _ => return None,
        };
        let value = self.fresh;
        self.fresh += 1;
        let held = llrm_lir::Held { value, width: cell.width, offset: 0 };
        let load = Insn {
            invariant: true,
            ..Insn::new(
                insn.at,
                insn.covers,
                Some(Semantics {
                    name: Some(name.into()),
                    dests: vec![Loc::Held(held)],
                    sources: vec![Loc::Mem(cell.clone())],
                    ..Semantics::new(Operation::Move)
                }),
                vec![value],
                vec![],
            )
        };
        let mut sources = what.sources.clone();
        sources[at] = Loc::Held(held);
        let mut uses = insn.uses.clone();
        uses.push(value);
        let rest =
            Insn { uses, what: Some(Semantics { sources, ..what.clone() }), invariant: false, ..(**insn).clone() };
        Some((Wrapped(Arc::new(load)), Wrapped(Arc::new(rest))))
    }

    fn begin(
        &mut self,
        facts: &licm::Facts<'_, Wrapped>,
    ) {
        self.peak = None;
        self.pressure.begin();
        self.named = facts.named.iter().filter(|value| !self.floating.contains(value)).count();
        self.floats_named = facts.named.iter().filter(|value| self.floating.contains(value)).count();
        // An x87 value cannot be live across a call (`floatassign` does not
        // spill across one), and the stack holds `float_stack` values.
        let inside = || facts.inside_blocks();
        self.calls = inside().any(|block| {
            block.insns.iter().any(|one| {
                one.0.call.is_some()
                    || one.0.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Call | Operation::Barrier))
            })
        });
        // `InitializeLoadsHoistableLoops`: no store, call or ordered load in
        // the loop, its inner loops among it.
        self.loads = !inside().any(|block| block.insns.iter().any(|one| keeps_loads(&one.0)));
    }

    fn movable(
        &mut self,
        facts: &licm::Facts<'_, Wrapped>,
        at: i64,
        insns: &[Wrapped],
        index: usize,
    ) -> bool {
        let one = &insns[index].0;
        let value = one.defines.first().copied();
        let flags = |lanes: &crate::backend::lanes::Lanes| {
            lanes.iter().any(|&(register, _)| register == llrm_lir::registers::RegId::None)
        };
        // An x87 constant is hoisted where the control word is constant and the
        // stack has room (`constant_load`).
        if constant_load(one) && !self.control_word_written {
            return !self.calls && self.floats_named < self.body.float_stack;
        }
        // DEVIATE ML2 (as X1): `fld1`/`fldz` and `fchs` are exact under any
        // rounding control, so the control word the function writes
        // does not keep them in the loop.
        if constant_load(one) || sign_flip(one) {
            // A loop that stores an x87 value works around a held constant with
            // `fxch`es (x_ld_to_float, 64-bit, +530 clk).
            let stores = facts.inside_blocks().any(|block| {
                block.insns.iter().any(|read| read.0.what.as_ref().is_some_and(|what| what.op == Operation::FloatStore))
            });
            // Only a constant that is flipped: `fld1; fchs` a trip (10 clk) is
            // what holding saves; a plain `fld1` saves 4 clk, and
            // the `fxch`es at the loop's back edge cost more (recfloat, +8%).
            let flipped = value.is_some_and(|value| reads(facts, value).any(|read| sign_flip(&read.0)));
            if constant_load(one) && !flipped {
                return false;
            }
            // A flip takes the slot of the constant it flips; a constant they
            // all flip is held as the flipped one.
            let room = sign_flip(one) || self.floats_named + CARRY_RESERVE < self.body.float_stack;
            if stores
                || self.size
                || self.calls
                || !room
                || !innermost(facts)
                || !value.is_some_and(|value| read_by_arithmetic(facts, value))
            {
                return false;
            }
            if constant_load(one) {
                self.floats_named += 1;
            }
            return true;
        }
        if pool_load(one) {
            if self.size || self.calls || self.floats_named + CARRY_RESERVE >= self.body.float_stack {
                return false;
            }
            self.floats_named += 1;
            return true;
        }
        let loads_here = self.loads && (facts.every_trip)(at);
        // `IsProfitableToHoist`'s high-pressure branch lets an
        // `isDereferenceableInvariantLoad` through: the allocator can load it
        // again where it wants it.
        // <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLICM.cpp#L1386-L1392>
        let invariant_load = loaded(one).is_some() && one.invariant();
        let limit = self.classes.available.len();
        let facts_every_trip = (facts.every_trip)(at);
        let mut movable = candidate(one, self.classes, loads_here)
            && value.is_some_and(|value| !self.floating.contains(&value))
            && (if self.classes.hoist_by_pressure {
                !self.pressure.high(facts, at, one)
            } else {
                self.named < limit
            } || invariant_load)
            && (invariant_load || facts_every_trip || self.peak_pressure(facts) < limit);
        // `IsProfitableToHoist`: "Trivially rematerializable instructions
        // should always be hoisted providing the register allocator can just
        // pull them down again", unless a copy in the loop takes the value.
        let constant = value.is_some_and(|value| {
            one.uses.is_empty()
                && facts.defs.get(&value) == Some(&1)
                && !facts.copied.contains(&value)
                && !self.floating.contains(&value)
                && one.defines.len() == 1
                && one.call.is_none()
                && one.group.is_none()
                && one.clobbers.is_empty()
                && one.requires.is_empty()
                && one.delivers.is_empty()
                && crate::greedy::remat::is_trivially_re_materializable(one, false)
        });
        if constant && !movable {
            self.pressure.hoisted(one);
            return true;
        }
        if movable {
            let (regs, bits) = (self.regs, self.body.bits);
            let effect = liveness::effect(regs, bits, one);
            let name = one.what.as_ref().and_then(|what| what.name.as_deref()).unwrap_or("");
            let reads = matches!(name, "adc" | "sbb" | "rcl" | "rcr")
                || effect.as_ref().is_some_and(|effect| flags(&effect.all_reads()));
            let sets = effect.as_ref().is_none_or(|effect| flags(&effect.writes));
            if reads {
                movable = false;
            } else if sets {
                // Flags set here must not be read below, in the block.
                for later in &insns[index + 1..] {
                    match liveness::effect(regs, bits, &later.0) {
                        Some(next) if flags(&next.all_reads()) => {
                            movable = false;
                            break;
                        }
                        Some(next) if flags(&next.writes) => break,
                        Some(_) => {}
                        None if later.0.what.as_ref().is_some_and(|what| {
                            what.op == Operation::Move
                                || what.op == Operation::Jump
                                || matches!(what.name.as_deref(), Some("jcxz" | "jecxz" | "loop"))
                        }) => {}
                        None => {
                            movable = false;
                            break;
                        }
                    }
                }
            }
        }
        if movable {
            self.pressure.hoisted(one);
        }
        movable
    }
}

/// `body` with the loop-invariant computations in the preheaders.
pub fn hoisted(
    body: &LirBody,
    classes: &RegisterClasses,
) -> LirBody {
    let Some(registers) = body.registers else { return body.clone() };
    // A flat machine: no selector to place, as `Segments::of` makes for one.
    let flat = Segments {
        selectors: Vec::new(),
        data: registers.data_segment,
        through: None,
        stack_shares_data: false,
        huge_shift: None,
        offset_bytes: 0,
        registers,
    };
    hoisted_sized(body, classes, false, &flat)
}

/// `hoisted`, where `size` keeps the x87 pool constants in their loops.
pub fn hoisted_sized(
    body: &LirBody,
    classes: &RegisterClasses,
    size: bool,
    segments: &Segments,
) -> LirBody {
    let Some(regs) = body.registers else { return body.clone() };
    let control_word_written = body
        .insns()
        .iter()
        .any(
            |one| one.what
                .as_ref()
                .is_some_and(
                    |what| what.op == Operation::FloatStore && matches!(what.name.as_deref(), Some("fisttp" | "fistt")),
                ),
        );
    let blocks: Vec<licm::Block<Wrapped>> = body
        .blocks
        .iter()
        .map(|block| licm::Block {
            at: block.at,
            succ: block.succ.to_vec(),
            phis: block
                .phis
                .iter()
                .map(|phi| licm::Phi {
                    result: phi.result,
                    incoming: phi.incoming.iter().map(|(_, value)| *value).collect(),
                })
                .collect(),
            insns: block.insns.iter().cloned().map(Wrapped).collect(),
        })
        .collect();
    let floating = crate::backend::regclass::floating(body);
    let model =
        crate::backend::machinesched::pressure_model(body, classes, segments, &|value| !floating.contains(&value));
    let mut policy = X86 {
        pressure: Pressure::new(body, model),
        named: 0,
        body,
        classes,
        regs,
        floating,
        control_word_written,
        calls: false,
        floats_named: 0,
        peak: None,
        live_out: None,
        loads: false,
        size,
        fresh: body.next_value(),
    };
    let Some(moved) = licm::hoist(&blocks, body.entry, &mut policy) else { return body.clone() };
    let blocks: Vec<crate::model::lir::Block> = body
        .blocks
        .iter()
        .zip(moved)
        .map(|(block, after)| {
            let insns: Vec<Arc<Insn>> = after.insns.into_iter().map(|one| one.0).collect();
            let same = insns.len() == block.insns.len()
                && insns.iter().zip(block.insns.iter()).all(|(a, b)| Arc::ptr_eq(a, b));
            if same { block.clone() } else { block.with_insns(insns) }
        })
        .collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    /// `hoisted` on the built-in (m16) machine's segments.
    fn hoisted(
        body: &LirBody,
        classes: &RegisterClasses,
    ) -> LirBody {
        hoisted_sized(body, classes, false, &crate::backend::target::BUILT_IN)
    }
    use crate::model::ir::{Held, Semantics};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    /// `x = a + b` made on every trip of a loop that changes neither (the pin's
    /// EarlyMachineLICM runs ahead of MachineCSE): it is made once, before the
    /// loop.
    #[test]
    fn test_a_sum_of_values_the_loop_does_not_change_is_made_before_the_loop() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let add = insn(10, Operation::Binary, "add", vec![held(10)], vec![held(1), held(2)], vec![10], vec![1, 2]);
        let copy = insn(11, Operation::Move, "mov", vec![held(11)], vec![held(3)], vec![11], vec![3]);
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], vec![add, copy, branch(12)]).into(),
                block(2, vec![], vec![insn(13, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2, 3].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let classes = RegisterClasses::m16();
        let done = hoisted(&body, &classes);
        let names = |at: usize| -> Vec<String> {
            done.blocks[at]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(names(0), ["add", "jmp"]);
        assert_eq!(names(1), ["mov", "jcxz"]);
    }

    /// `IsProfitableToHoist`: a sum made in a branch of a loop whose header
    /// holds `limit - 1` values at once was hoisted because only 3 values
    /// were named in the loop (`named < limit`), adding a register held across
    /// the loop (rectwo -O1, +291,719 clk). It stays where the pressure
    /// reaches the limit (`CanCauseHighRegPressure`).
    #[test]
    fn test_a_sum_stays_in_a_loop_whose_header_holds_the_limit_of_values() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let narrow = |value: u32| Loc::Held(Held { value, width: 2, offset: 0 });
        let run = |held_in_header: u32, by_pressure: bool| {
            let live: Vec<u32> = (0..held_in_header).map(|at| 20 + at).collect();
            let mut header: Vec<Arc<Insn>> = live
                .iter()
                .map(|&value| {
                    insn(
                        value as i64,
                        Operation::Binary,
                        "add",
                        vec![narrow(value)],
                        vec![narrow(5), narrow(5)],
                        vec![value],
                        vec![5, 5],
                    )
                })
                .collect();
            header.push(branch(60));
            let sum =
                insn(70, Operation::Binary, "add", vec![narrow(50)], vec![narrow(2), narrow(3)], vec![50], vec![2, 3]);
            let mut inside: Vec<Arc<Insn>> = live
                .iter()
                .map(|&value| {
                    insn(
                        value as i64 + 100,
                        Operation::Move,
                        "mov",
                        vec![narrow(value + 100)],
                        vec![narrow(value)],
                        vec![value + 100],
                        vec![value],
                    )
                })
                .collect();
            inside.push(sum);
            inside.push(jump(73));
            let exit = insn(72, Operation::Return, "ret", vec![], vec![], vec![], vec![2, 3]);
            let mut head = block(1, vec![2, 3], header);
            head.phis = vec![crate::model::lir::Phi { result: 5, incoming: vec![(0, 1), (2, 50)] }].into();
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    head.into(),
                    block(2, vec![1], inside).into(),
                    block(3, vec![], vec![exit]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1, 2, 3].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let mut classes = (*RegisterClasses::m16()).clone();
            classes.hoist_by_pressure = by_pressure;
            let done = hoisted(&body, &classes);
            done.blocks[0].insns.iter().any(|one| one.defines.contains(&50))
        };
        assert!(run(1, true), "a header that holds one value leaves room");
        assert!(!run(4, true), "four values of weight 2 and the sum reach the limit 10 of the set");
        assert!(run(1, false), "the count rule: a header that holds one value leaves room");
        assert!(
            !run(4, false),
            "the count rule passes (3 values named), but the sum is in a block that is not on every trip and the peak pressure is at the registers: no speculating hoist (#7821)"
        );
    }

    /// m16 gates hoists by pressure again: its address registers are their
    /// own class (ADDR16), so the pin's per-set limit answers.
    #[test]
    fn test_m16_gates_hoists_by_pressure() {
        assert!(crate::backend::classes::RegisterClasses::m16().hoist_by_pressure);
    }

    /// `EliminateCSE` renamed the readers of one duplicate at a time, each
    /// rename visiting every instruction of the function: 19 duplicates in a
    /// loop of 140 instructions made about 2,700 `rename_use` calls, and
    /// `lir machinelicm` of jpeg_single went from 8.7M to 51M instructions.
    /// Only a reader of a duplicate is renamed.
    #[test]
    fn test_the_readers_of_duplicates_are_renamed_without_visiting_the_whole_body() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let duplicates = 20u32;
        let mut inside = Vec::new();
        for number in 0..duplicates {
            let value = 10 + 2 * number;
            let at = i64::from(10 + 3 * number);
            inside.push(insn(
                at,
                Operation::Binary,
                "add",
                vec![held(value)],
                vec![held(1), held(2)],
                vec![value],
                vec![1, 2],
            ));
            inside.push(insn(
                at + 1,
                Operation::Compare,
                "cmp",
                vec![],
                vec![held(value), held(3)],
                vec![],
                vec![value, 3],
            ));
        }
        for number in 0..100u32 {
            let value = 200 + number;
            inside.push(insn(
                i64::from(100 + number),
                Operation::Move,
                "mov",
                vec![held(value)],
                vec![held(3)],
                vec![value],
                vec![3],
            ));
        }
        inside.push(insn(500, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]));
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![insn(0, Operation::Jump, "jmp", vec![], vec![], vec![], vec![])]).into(),
                block(1, vec![1, 2], inside).into(),
                block(2, vec![], vec![insn(501, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2, 3].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        RENAMES.with(|renames| renames.set(0));
        let done = hoisted(&body, &RegisterClasses::m16());
        let adds = done.blocks[0]
            .insns
            .iter()
            .filter(|one| one.what.as_ref().and_then(|w| w.name.as_deref()) == Some("add"))
            .count();
        assert_eq!(adds, 1, "the premise: the duplicates are made once");
        let renames = RENAMES.with(|renames| renames.get());
        assert!(renames <= 3 * duplicates as usize, "{renames}");
    }

    /// After a round of renames the sweep visited every instruction of the
    /// function, loop after loop: 2.9M `rename_uses` calls on
    /// memory_expansion -O2. Only a block that reads a renamed value is swept.
    #[test]
    fn test_blocks_that_read_no_duplicate_are_not_swept_for_renames() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let mut inside = Vec::new();
        for number in 0..5u32 {
            let value = 10 + 2 * number;
            let at = i64::from(10 + 3 * number);
            inside.push(insn(
                at,
                Operation::Binary,
                "add",
                vec![held(value)],
                vec![held(1), held(2)],
                vec![value],
                vec![1, 2],
            ));
            inside.push(insn(
                at + 1,
                Operation::Compare,
                "cmp",
                vec![],
                vec![held(value), held(3)],
                vec![],
                vec![value, 3],
            ));
        }
        inside.push(insn(500, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]));
        let tail = 200i64;
        let mut blocks = vec![
            block(0, vec![1], vec![insn(0, Operation::Jump, "jmp", vec![], vec![], vec![], vec![])]).into(),
            block(1, vec![1, 1000], inside).into(),
        ];
        for number in 0..tail {
            let at = 1000 + number;
            let next = if number + 1 == tail { vec![] } else { vec![at + 1] };
            let value = 5000 + number as u32;
            blocks.push(
                block(
                    at,
                    next,
                    vec![insn(at * 10, Operation::Move, "mov", vec![held(value)], vec![held(3)], vec![value], vec![3])],
                )
                .into(),
            );
        }
        let mut body = LirBody::new("loop", 0, blocks, IndexMap::default(), IndexMap::default());
        body.inputs = [1, 2, 3].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        crate::backend::licm_core::SWEPT.with(|swept| swept.set(0));
        let done = hoisted(&body, &RegisterClasses::m16());
        let adds = done.blocks[0]
            .insns
            .iter()
            .filter(|one| one.what.as_ref().and_then(|w| w.name.as_deref()) == Some("add"))
            .count();
        assert_eq!(adds, 1, "the premise: the duplicates are made once");
        let swept = crate::backend::licm_core::SWEPT.with(|swept| swept.get());
        assert!(swept <= 40, "{swept}");
    }

    /// The policy read every block of the function, filtered to the loop's,
    /// for each loop: with 400 loops of 3 blocks, memory_expansion's
    /// `lir machinelicm` spent 60M instructions in `begin`. It reads the
    /// loop's blocks only.
    #[test]
    fn test_a_loop_is_read_through_its_own_blocks_not_the_functions() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let loops = 60i64;
        let mut blocks: Vec<LirBlock> = Vec::new();
        for number in 0..loops {
            let (pre, body) = (10 * number, 10 * number + 1);
            let value = 100 + number as u32;
            blocks.push(block(
                pre,
                vec![body],
                vec![insn(pre, Operation::Jump, "jmp", vec![], vec![], vec![], vec![])],
            ));
            let next = if number + 1 == loops { 10 * loops } else { 10 * (number + 1) };
            blocks.push(block(
                body,
                vec![body, next],
                vec![
                    insn(
                        body * 10,
                        Operation::Binary,
                        "add",
                        vec![held(value)],
                        vec![held(1), held(2)],
                        vec![value],
                        vec![1, 2],
                    ),
                    insn(body * 10 + 1, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]),
                ],
            ));
        }
        blocks.push(block(
            10 * loops,
            vec![],
            vec![insn(99999, Operation::Return, "ret", vec![], vec![], vec![], vec![])],
        ));
        let mut body = LirBody::new(
            "loops",
            0,
            blocks.into_iter().map(crate::model::lir::Block::from).collect(),
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2, 3].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        crate::backend::licm_core::READ.with(|read| read.set(0));
        let done = hoisted(&body, &RegisterClasses::m16());
        let hoisted_adds = done
            .blocks
            .iter()
            .filter(|one| one.at % 10 == 0)
            .flat_map(|one| one.insns.iter())
            .filter(|one| one.what.as_ref().and_then(|w| w.name.as_deref()) == Some("add"))
            .count();
        assert_eq!(hoisted_adds as i64, loops, "the premise: every loop's add is made before it");
        let read = crate::backend::licm_core::READ.with(|read| read.get());
        assert!(read <= 8 * loops as usize, "{read}");
    }

    /// bintree's `insert` at -O1, a loop after `tailrecurse`: `t[3*node]` is
    /// read on every trip of a loop that stores nothing. The pin's
    /// EarlyMachineLICM hoists its `lea` and load (`AllowedToHoistLoads`);
    /// both stayed in the loop, 96366 clocks against 93475 hoisted. A store in
    /// the loop keeps the load in it.
    #[test]
    fn test_a_load_of_a_cell_a_loop_never_stores_is_made_before_the_loop() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let cell = |base: u32, index: u32, scale: i64| {
            Loc::Mem(Mem {
                base: Some(Held { value: base, width: 4, offset: 0 }),
                index: Some(Held { value: index, width: 4, offset: 0 }),
                scale,
                ..Mem::new(None, 4)
            })
        };
        let run = |stores: bool| {
            let lea = insn(10, Operation::Address, "lea", vec![held(10)], vec![cell(2, 2, 2)], vec![10], vec![2, 2]);
            let load = insn(11, Operation::Move, "mov", vec![held(11)], vec![cell(1, 10, 4)], vec![11], vec![1, 10]);
            let mut inside = vec![lea, load];
            if stores {
                inside.push(insn(
                    12,
                    Operation::Move,
                    "mov",
                    vec![cell(1, 3, 4)],
                    vec![held(11)],
                    vec![],
                    vec![1, 3, 11],
                ));
            }
            inside.push(branch(13));
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    block(1, vec![1, 2], inside).into(),
                    block(2, vec![], vec![insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1, 2, 3].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted(&body, &RegisterClasses::m16());
            let names = |at: usize| -> Vec<String> {
                done.blocks[at]
                    .insns
                    .iter()
                    .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                    .collect()
            };
            (names(0), names(1))
        };
        assert_eq!(run(false), (vec!["lea".to_owned(), "mov".into(), "jmp".into()], vec!["jcxz".to_owned()]));
        assert_eq!(
            run(true),
            (vec!["lea".to_owned(), "jmp".into()], vec!["mov".to_owned(), "mov".into(), "jcxz".into()])
        );
    }

    /// recfloat's `decay` at -O1: `fld1` was made on every trip of the loop
    /// (4 clk and an `fxch` a trip); the pin's EarlyMachineLICM hoists it, the
    /// value then stays on the x87 stack. A call in the loop keeps it there.
    #[test]
    fn test_an_x87_constant_load_is_made_before_a_loop_without_a_call() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 10, offset: 0 });
        let run = |call: bool, truncating: bool| {
            let one = insn(10, Operation::FloatLoad, "fld1", vec![wide(10)], vec![], vec![10], vec![]);
            let add =
                insn(11, Operation::Binary, "fadd", vec![wide(11)], vec![wide(1), wide(10)], vec![11], vec![1, 10]);
            let mut inside = vec![one, add];
            if call {
                inside.push(insn(12, Operation::Call, "call", vec![], vec![], vec![], vec![]));
            }
            inside.push(branch(13));
            let mut exit = vec![insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![])];
            if truncating {
                exit.insert(0, insn(15, Operation::FloatStore, "fisttp", vec![], vec![wide(11)], vec![], vec![11]));
            }
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    block(1, vec![1, 2], inside).into(),
                    block(2, vec![], exit).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().collect();
            body.float_stack = 8;
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted(&body, &RegisterClasses::m16());
            done.blocks[0]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(false, false), ["fld1", "jmp"]);
        assert_eq!(run(true, false), ["jmp"]);
        // recfloat -O2: the pin's `FLDCW` for the `fptosi` after the loop
        // makes `$fpcw` non-constant, so `fld1` stays in the loop (ML2 holds
        // it only to flip it: `test_a_flipped_x87_constant_is_held...`).
        assert_eq!(run(false, true), ["jmp"]);
    }

    /// FRAC.BAS `fracline2` (qb): two `fldz` made before the loop were one
    /// value after EliminateCSE, read by both loop-carried entries of the x87
    /// stack: "floating stack input is unavailable" (once "value#10 is read
    /// but never defined"). Each x87 value is its own stack entry, so two
    /// constant loads stay two values.
    #[test]
    fn test_two_hoisted_x87_constants_stay_two_values() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 10, offset: 0 });
        let first = insn(10, Operation::FloatLoad, "fldz", vec![wide(10)], vec![], vec![10], vec![]);
        let second = insn(11, Operation::FloatLoad, "fldz", vec![wide(11)], vec![], vec![11], vec![]);
        let read_first =
            insn(12, Operation::Binary, "fadd", vec![wide(12)], vec![wide(1), wide(10)], vec![12], vec![1, 10]);
        let read_second =
            insn(13, Operation::Binary, "fadd", vec![wide(13)], vec![wide(12), wide(11)], vec![13], vec![12, 11]);
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], vec![first, second, read_first, read_second, branch(14)]).into(),
                block(2, vec![], vec![insn(15, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1].into_iter().collect();
        body.float_stack = 8;
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::m16());
        let before: Vec<u32> = done.blocks[0]
            .insns
            .iter()
            .filter(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("fldz")))
            .flat_map(|one| one.defines.iter().copied())
            .collect();
        assert_eq!(before, [10, 11]);
    }

    /// x_ld_abs -O2 at m32: the unrolled copies of `x[i] - 16 - r` made `-1.0`
    /// (`fld1; fchs`, 10 clk) and added it (8) on every inner trip, 13,320 clk
    /// of gcc's 13,113. Held across the loop it is one `fadd st, st(i)`. A
    /// compare wants its constant on top, where holding it costs an `fxch`
    /// for the `fldz` it saves; a loop with a loop inside it holds the value
    /// across the inner loop for a saving once an outer trip.
    #[test]
    fn test_a_flipped_x87_constant_is_held_by_an_innermost_loop_that_adds_it() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 10, offset: 0 });
        // `reader`: what takes the flipped constant; `nested`: the loop has a
        // loop inside it.
        let run = |reader: &str, nested: bool, size: bool, extra: &str| {
            let one = insn(10, Operation::FloatLoad, "fld1", vec![wide(10)], vec![], vec![10], vec![]);
            let flip = insn(11, Operation::FloatUnary, "fchs", vec![wide(11)], vec![wide(10)], vec![11], vec![10]);
            let other = if extra == "mem" { Loc::Mem(Mem::new(None, 8)) } else { wide(1) };
            let read =
                insn(12, Operation::Binary, reader, vec![wide(12)], vec![other, wide(11)], vec![12], vec![1, 11]);
            let store = insn(
                17,
                Operation::FloatStore,
                "fstp",
                vec![Loc::Mem(Mem::new(None, 8))],
                vec![wide(12)],
                vec![],
                vec![12],
            );
            let mut blocks = vec![block(0, vec![1], vec![jump(0)]).into()];
            if nested {
                blocks.push(block(1, vec![2], vec![one, flip, read]).into());
                blocks.push(block(2, vec![2, 3], vec![branch(13)]).into());
                blocks.push(block(3, vec![1, 4], vec![branch(14)]).into());
            } else {
                let mut inside = vec![one, flip, read];
                if extra == "store" {
                    inside.push(store);
                }
                inside.push(branch(13));
                blocks.push(block(1, vec![1, 2], inside).into());
            }
            // The function writes the control word (`fisttp` after the loop).
            let exit = vec![
                insn(16, Operation::FloatStore, "fisttp", vec![], vec![wide(12)], vec![], vec![12]),
                insn(15, Operation::Return, "ret", vec![], vec![], vec![], vec![]),
            ];
            blocks.push(block(blocks.len() as i64, vec![], exit).into());
            let mut body = LirBody::new("loop", 0, blocks, IndexMap::default(), IndexMap::default());
            body.inputs = [1].into_iter().collect();
            body.float_stack = 8;
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted_sized(&body, &RegisterClasses::m16(), size, &crate::backend::target::BUILT_IN);
            done.blocks[0]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        assert_eq!(run("fadd", false, false, ""), ["fld1", "fchs", "jmp"]);
        assert_eq!(run("fcom", false, false, ""), ["jmp"]);
        assert_eq!(run("fadd", true, false, ""), ["jmp"]);
        assert_eq!(run("fadd", false, true, ""), ["jmp"], "-Os: the `fstp` that ends the carried value costs 2 B");
        assert_eq!(run("fadd", false, false, "store"), ["jmp"], "x_ld_to_float, 64-bit: `fxch`es around the store");
        assert_eq!(
            run("fadd", false, false, "mem"),
            ["jmp"],
            "x_ld_div, 64-bit: the held value is copied, `fld st(0)`"
        );
    }

    /// A loop-invariant `fld m80` of a constant-pool entry was made on every
    /// trip (6 clk and a popping `fmulp`; gcc keeps the value on the stack):
    /// x_ld_to_int -O2 +3,533 clk of 18,000 at m32. It is made once, ahead of
    /// the loop, where the function writes the control word too (`fisttp`),
    /// but not where a call is in the loop or the stack has no room left for
    /// the loop's own temporaries.
    #[test]
    fn test_an_x87_pool_constant_is_made_before_a_loop_that_leaves_stack_room() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 10, offset: 0 });
        // `others`: floating values the loop reads from outside it, besides the
        // accumulator.
        let run = |others: u32, call: bool, truncating: bool, width: u32, size: bool| {
            let pool = Loc::Mem(Mem::new(None, width));
            let load = Arc::new(Insn {
                invariant: true,
                ..(*insn(10, Operation::FloatLoad, "fld", vec![wide(10)], vec![pool], vec![10], vec![])).clone()
            });
            let mut inside = vec![load];
            let mut last = 1;
            for (at, value) in (0..others).zip(100..) {
                let sum = 200 + value;
                inside.push(insn(
                    20 + at as i64,
                    Operation::Binary,
                    "fadd",
                    vec![wide(sum)],
                    vec![wide(last), wide(value)],
                    vec![sum],
                    vec![last, value],
                ));
                last = sum;
            }
            inside.push(insn(
                11,
                Operation::Binary,
                "fmul",
                vec![wide(11)],
                vec![wide(last), wide(10)],
                vec![11],
                vec![last, 10],
            ));
            if call {
                inside.push(insn(12, Operation::Call, "call", vec![], vec![], vec![], vec![]));
            }
            inside.push(branch(13));
            let mut exit = vec![insn(14, Operation::Return, "ret", vec![], vec![], vec![], vec![])];
            if truncating {
                exit.insert(0, insn(15, Operation::FloatStore, "fisttp", vec![], vec![wide(11)], vec![], vec![11]));
            }
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    block(1, vec![1, 2], inside).into(),
                    block(2, vec![], exit).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().chain(100..100 + others).collect();
            body.float_stack = 8;
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted_sized(&body, &RegisterClasses::m16(), size, &crate::backend::target::BUILT_IN);
            done.blocks[0]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(0, false, false, 10, false), ["fld", "jmp"]);
        // The control word is written after the loop: `fld` does not read its
        // rounding.
        assert_eq!(run(0, false, true, 10, false), ["fld", "jmp"]);
        // Four values held across the loop and the constant leave three slots.
        assert_eq!(run(3, false, false, 10, false), ["fld", "jmp"]);
        // One more and the loop's own temporaries would not fit.
        assert_eq!(run(4, false, false, 10, false), ["jmp"]);
        assert_eq!(run(0, true, false, 10, false), ["jmp"]);
        // A dword or qword entry folds into its reader and stays there.
        assert_eq!(run(0, false, false, 4, false), ["jmp"]);
        assert_eq!(run(0, false, false, 8, false), ["jmp"]);
        // -Os: the `fstp` that ends the carried constant is a byte the loop did
        // not cost.
        assert_eq!(run(0, false, false, 10, true), ["jmp"]);
    }

    /// A constant made in the loop that a phi of the loop takes (the join of a
    /// state machine takes one from each case) stays where it is: the pin's
    /// `HasLoopPHIUse` keeps a cheap instruction from making a copy in the
    /// loop. Hoisted it was live across the whole loop, and the 262 of them in
    /// a 256-case state machine cost 29 times the pin's segment work in
    /// `LiveRangeCalc::extend` (206550 adds against 7056 at N=256).
    #[test]
    fn test_a_constant_a_loop_phi_takes_stays_in_the_loop() {
        use crate::model::lir::Phi;
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let imm = Loc::Imm(llrm_lir::Imm { value: 7, width: 4, address: None });
        let run = |phi_takes_it: bool| {
            let constant = insn(10, Operation::Move, "mov", vec![held(10)], vec![imm.clone()], vec![10], vec![]);
            let mut header = block(1, vec![1, 2], vec![constant, branch(11)]);
            let taken = if phi_takes_it { 10 } else { 1 };
            header.phis = vec![Phi { result: 20, incoming: vec![(0, 1), (1, taken)] }].into();
            let mut body = LirBody::new(
                "loop",
                0,
                vec![
                    block(0, vec![1], vec![jump(0)]).into(),
                    header.into(),
                    block(2, vec![], vec![insn(12, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            );
            body.inputs = [1].into_iter().collect();
            body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO));
            let done = hoisted(&body, &RegisterClasses::m16());
            (done.blocks[0].insns.len(), done.blocks[1].insns.len())
        };
        assert_eq!(run(true), (1, 2), "the phi's copy stays");
        assert_eq!(run(false), (2, 1), "no phi takes it: hoisted");
    }

    /// m64 nbody_single at -O1/-O2: `mov r64, offset sym` made on every trip of
    /// a loop was kept there, three per trip, against the pin's one before the
    /// loop (+18% executed): `floating` took every value wider than 4 bytes
    /// for a float, an 8-byte address among them.
    #[test]
    fn test_a_symbol_address_of_eight_bytes_is_made_before_the_loop() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 8, offset: 0 });
        let address = Loc::Imm(llrm_lir::Imm {
            value: 0,
            width: 8,
            address: Some(llrm_lir::Addr::new(llrm_lir::Space::Segment, 0)),
        });
        let constant = insn(10, Operation::Move, "mov", vec![wide(10)], vec![address], vec![10], vec![]);
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], vec![constant, branch(11)]).into(),
                block(2, vec![], vec![insn(12, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::of(&llrm_x86_m64::M64));
        assert_eq!((done.blocks[0].insns.len(), done.blocks[1].insns.len()), (2, 1));
    }

    /// m64 memory_expansion -O1 (+1125 B, 247 more `lea reg,[rip+sym]` than the
    /// pin): the address a call takes sat in the loop's preheader and the
    /// loop made it again; hoisting put a second value beside it, and the
    /// copy-only first one was never shared (MachineCSE heuristic 2) while the
    /// pin's `InitCSEMap(Preheader)` folds the hoisted one into it.
    #[test]
    fn test_an_address_the_preheader_already_makes_is_not_made_again_by_the_hoist() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let wide = |value: u32| Loc::Held(Held { value, width: 8, offset: 0 });
        let address = || {
            Loc::Imm(llrm_lir::Imm {
                value: 0,
                width: 8,
                address: Some(llrm_lir::Addr::new(llrm_lir::Space::Segment, 0)),
            })
        };
        let make = |at: i64, value: u32| {
            insn(at, Operation::Move, "mov", vec![wide(value)], vec![address()], vec![value], vec![])
        };
        let read = |at: i64, value: u32, result: u32| {
            insn(
                at,
                Operation::Binary,
                "add",
                vec![wide(result)],
                vec![wide(value), wide(1)],
                vec![result],
                vec![value, 1],
            )
        };
        let mut body = LirBody::new(
            "preheader",
            0,
            vec![
                block(0, vec![1], vec![make(0, 20), jump(1)]).into(),
                block(1, vec![1, 2], vec![make(10, 10), read(11, 10, 11), branch(12)]).into(),
                block(2, vec![], vec![insn(20, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::of(&llrm_x86_m64::M64));
        let made = done
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| one.what.as_ref().is_some_and(|what| matches!(what.sources.as_slice(), [Loc::Imm(_)])))
            .count();
        assert_eq!(made, 1, "{done:?}");
        let sum = done.blocks[0].insns.iter().find(|one| one.defines == [11]).expect("the sum left the loop");
        assert_eq!(sum.uses, [20, 1], "the sum reads the preheader's address");
    }

    /// `ExtractHoistableLoad`: a `mulss` that reads a constant-pool cell
    /// (`isDereferenceableInvariantLoad`) beside a value the loop changes was
    /// left whole in the loop, its load made on every trip (nbody_single -O3
    /// m64: three `mulss xmm, [rip+K]` in the loop where the pin has the
    /// constants in registers before it, +73 B). The load is hoisted and the
    /// `mulss` reads it.
    #[test]
    fn test_an_invariant_load_folded_into_an_instruction_of_the_loop_is_hoisted() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let cell = Loc::Mem(llrm_lir::Mem::new(None, 4));
        let step = insn(10, Operation::Binary, "addss", vec![held(2)], vec![held(2), held(1)], vec![2], vec![2, 1]);
        let mut folded = insn(11, Operation::Binary, "mulss", vec![held(3)], vec![held(2), cell], vec![3], vec![2]);
        Arc::make_mut(&mut folded).invariant = true;
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], vec![step, folded, branch(12)]).into(),
                block(2, vec![], vec![insn(13, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::of(&llrm_x86_m64::M64));
        let names = |at: usize| -> Vec<String> {
            done.blocks[at]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(names(0), ["movss", "jmp"], "the constant is loaded before the loop");
        assert_eq!(names(1), ["addss", "mulss", "jcxz"]);
        let mul = &done.blocks[1].insns[1];
        assert!(
            mul.what.as_ref().is_some_and(|what| what.sources.iter().all(|source| matches!(source, Loc::Held(_)))),
            "the mulss reads a register"
        );
    }

    /// x_hanoi2 -O1 (m32): a `shl` of an argument in the body of a loop that
    /// holds every register was made at the entry and spilled, 4096 leaf calls
    /// paying it; the pin's `AvoidSpeculation` leaves it where it runs only
    /// when the body does. Here the loop names 2 values but keeps 16 live.
    #[test]
    fn test_a_sum_in_a_conditionally_run_block_of_a_loop_holding_every_register_stays() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let mut body_insns: Vec<Arc<Insn>> = (0..20u32)
            .map(|n| {
                insn(
                    10 + n as i64,
                    Operation::Move,
                    "mov",
                    vec![held(100 + n)],
                    vec![Loc::Mem(llrm_lir::Mem::new(None, 4))],
                    vec![100 + n],
                    vec![],
                )
            })
            .collect();
        body_insns.push(insn(
            40,
            Operation::Binary,
            "add",
            vec![held(200)],
            vec![held(1), held(2)],
            vec![200],
            vec![1, 2],
        ));
        body_insns.extend((0..20u32).map(|n| {
            let sum = 300 + n;
            let before = if n == 0 { 200 } else { sum - 1 };
            insn(
                50 + n as i64,
                Operation::Binary,
                "add",
                vec![held(sum)],
                vec![held(before), held(100 + n)],
                vec![sum],
                vec![before, 100 + n],
            )
        }));
        body_insns.push(jump(90));
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![2, 3], vec![branch(5)]).into(),
                block(2, vec![1], body_insns).into(),
                block(3, vec![], vec![insn(99, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = [1, 2].into_iter().collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::of(&llrm_x86_m64::M64));
        let adds = |at: usize| done.blocks[at].insns.iter().filter(|one| one.defines.contains(&200)).count();
        assert_eq!((adds(0), adds(2)), (0, 1), "the sum stays in the block that runs when the loop goes on");
    }

    /// A loop that already names more values than the target has registers
    /// kept the load of an invariant cell in it (`CanCauseHighRegPressure`
    /// refused everything); the pin's high-pressure branch hoists an invariant
    /// load all the same. General registers only: a value no general register
    /// holds (`regclass::floating`) is not moved by `movable` at all.
    #[test]
    fn test_an_invariant_load_is_hoisted_out_of_a_loop_that_names_every_register() {
        let block =
            |at: i64, succ: Vec<i64>, insns: Vec<Arc<Insn>>| LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) };
        let jump = |at: i64| insn(at, Operation::Jump, "jmp", vec![], vec![], vec![], vec![]);
        let branch = |at: i64| insn(at, Operation::Branch, "jcxz", vec![], vec![], vec![], vec![]);
        let mut inside: Vec<Arc<Insn>> = (1..=20u32)
            .map(|n| {
                let sum = 100 + n;
                insn(
                    n as i64,
                    Operation::Binary,
                    "add",
                    vec![held(sum)],
                    vec![held(sum), held(n)],
                    vec![sum],
                    vec![sum, n],
                )
            })
            .collect();
        let mut load = insn(
            50,
            Operation::Move,
            "mov",
            vec![held(200)],
            vec![Loc::Mem(llrm_lir::Mem::new(None, 4))],
            vec![200],
            vec![],
        );
        Arc::make_mut(&mut load).invariant = true;
        inside.push(load);
        inside.push(insn(
            51,
            Operation::Binary,
            "add",
            vec![held(101)],
            vec![held(101), held(200)],
            vec![101],
            vec![101, 200],
        ));
        inside.push(branch(52));
        let mut body = LirBody::new(
            "loop",
            0,
            vec![
                block(0, vec![1], vec![jump(0)]).into(),
                block(1, vec![1, 2], inside).into(),
                block(2, vec![], vec![insn(60, Operation::Return, "ret", vec![], vec![], vec![], vec![])]).into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.inputs = (1..=20).chain(101..=120).collect();
        body.registers = Some(llrm_lir::registers::Regs(&llrm_x86_m64::REGISTER_INFO));
        let done = hoisted(&body, &RegisterClasses::of(&llrm_x86_m64::M64));
        let names = |at: usize| -> Vec<String> {
            done.blocks[at]
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect()
        };
        assert_eq!(names(0), ["mov", "jmp"], "the constant is loaded before the loop");
    }
}
