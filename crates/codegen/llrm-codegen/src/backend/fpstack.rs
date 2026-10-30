//! The x87 stackifier: flat floating values in stack form, as LLVM's
//! `X86FloatingPoint` does it.
//!
//! # How it works
//! A translation of
//! [`X86FloatingPoint.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86FloatingPoint.cpp)
//! (`FPS`). `floatassign` leaves every floating value in a flat register while
//! it is live; this tracks which value sits in which stack slot and emits what
//! the instruction needs: `fxch` to bring an operand up (`moveToTop`), `fld
//! st(i)` to copy one that stays (`duplicateToTop`), the popping form of the
//! instruction where an operand dies (`popStackAfter`), `fstp st(i)` for a
//! value that dies below the top (`freeStackSlotBefore`).
//!
//! Liveness is the pass's own (`setKillFlags`): scanning a block backwards from
//! its live-out values marks an operand killed where nothing reads it after,
//! and a result dead where nothing does. Across a border the values live at
//! it are a bundle (`spillplacement::bundles`, LLVM's `EdgeBundles`): the first
//! exit to reach one fixes the order of its values, every other exit shuffles
//! to it (`shuffleStackTop`), and a block that holds more than it reads kills
//! the rest at its entry (`adjustLiveRegs`). No critical edge is split. Blocks
//! go in depth-first order, so one predecessor of each reachable block has
//! fixed its entry first.
//!
//! DEVIATE 1: a floating value is a value number, not `FP0`..`FP6`: the stack
//! model keeps the numbers in `values`, top first, and a scratch register is a
//! fresh number. Registers become values first (`fpregs`). More than
//! `float_stack` live values at once is refused where LLVM's `pushReg` is
//! fatal.
//!
//! DEVIATE 2: LLVM has no simultaneous copies at this stage (`PHIElimination`
//! writes them one at a time through temporaries); a group of phi copies here
//! is one step, as gcc's `move_for_stack_reg` takes it (`copy_group`).
//!
//! DEVIATE 3: a call, a barrier or an instruction naming a stack position
//! needs the stack empty. `handleCall` resets `StackTop` because the register
//! allocator spilled everything; a value live across one is an error here, as
//! `floatassign` does not spill across one.
//!
//! DEVIATE 5 (temporary,
//! [#2337](https://github.com/ali-mosavian/llrm/issues/2337)): `v op v`
//! where `v` stays puts the result below the copy when `v` is read next, so
//! no `fxch` follows.
//!
//! DEVIATE 6 (temporary,
//! [#2338](https://github.com/ali-mosavian/llrm/issues/2338)): a bundle still
//! to be fixed takes the order a fixed bundle ahead needs, through blocks that
//! only rename.
//!
//! DEVIATE 7 (temporary,
//! [#2339](https://github.com/ali-mosavian/llrm/issues/2339)): critical forward
//! edges into a block that reads fewer floating values than its bundle holds
//! are split first, as `floatalloc` does (`_split_for_the_stack`): the values
//! are popped on that edge only.
//!
//! DEVIATE 4: an unreachable block with live floating values has no predecessor
//! to fix its entry; its live values are taken in ascending order (LLVM
//! asserts).
//!
//! # Runs
//! As the body of `floatalloc` (slot `floatalloc@8`), after `floatassign`, once
//! per function with floating values.
//!
//! # Example
//! `a * b` where `a` is read again afterwards. Before:
//!
//! ```lir
//! v1 := fld [a]
//! v2 := fld [b]
//! v3 := fmul v1, v2        ; v2 dies, v1 stays
//! ```
//!
//! After: `fld [a]; fld [b]; fmul st(0), st(1)` leaves the product on top and
//! `a` under it.
//!
//! # Needs
//! [`crate::backend::floatassign`], `live`, `spillplacement::bundles`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::classes::RegisterClasses;
use crate::backend::floatalloc;
use crate::backend::floatalloc::{
    _float_copy, _floating, _reversed, _terminators, _two_values, inserted, name_is, semantics, st, unlowered, unnamed,
};
use crate::backend::floatassign;
use crate::backend::floatregions::{Raised, clears_stack};
use crate::backend::liverows::live_among;
use crate::backend::spillplacement;
use crate::model::ir::{Held, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Block, Insn, LirBody};
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// A store to a cell with no non-popping form (`fstp tbyte`), the first of a
/// pair `swapped_with` may join.
fn _unkept_store(what: Option<&Semantics>) -> bool {
    what.is_some_and(|what| {
        what.op == Operation::FloatStore
            && matches!(what.name.as_deref(), Some("fst" | "fstp"))
            && matches!(
                what.dests.as_slice(),
                [Loc::Mem(cell)] if !matches!(cell.width, 4 | 8)
            )
    })
}

/// The two cells share no byte: the same kind of address, a displacement
/// range apart. Anything else may overlap.
fn _apart(
    one: &llrm_lir::Mem,
    other: &llrm_lir::Mem,
) -> bool {
    let (Some(a), Some(b)) = (one.addr, other.addr) else { return false };
    a.space == b.space
        && a.base == b.base
        && a.segment == b.segment
        && one.base == other.base
        && one.index == other.index
        && one.selector == other.selector
        && one.scale == other.scale
        && (a.space == llrm_lir::Space::Frame || a.index == b.index)
        && (a.disp + i64::from(one.width) <= b.disp || b.disp + i64::from(other.width) <= a.disp)
}

/// What one step (an instruction, or a group of copies taken together) reads
/// and writes of the floating values, and what is live after it.
struct Step {
    /// The positions in the block: one, or the members of a group of copies.
    positions: Vec<usize>,
    uses: Vec<u32>,
    defs: Vec<u32>,
    live_after: HashSet<u32>,
    /// A later member of a group of copies taken at its first.
    absorbed: bool,
    /// For each value read after this step before it is written, the step that
    /// reads it first.
    next_read: IndexMap<u32, usize>,
}

impl Step {
    /// `MO.setIsKill()`: an operand nothing reads after, or one the step
    /// writes.
    fn kills(
        &self,
        value: u32,
    ) -> bool {
        self.defs.contains(&value) || !self.live_after.contains(&value)
    }

    /// `MO.setIsDead()`: a result nothing reads.
    fn dead(
        &self,
        value: u32,
    ) -> bool {
        !self.live_after.contains(&value)
    }
}

/// `LiveBundle`: the values live across a set of edges, and the order they
/// are in on the stack once the first exit has fixed it.
#[derive(Default)]
struct LiveBundle {
    mask: BTreeSet<u32>,
    /// `FixStack`: `st(0)` first.
    fixed: Option<Vec<u32>>,
}

struct Fps<'a> {
    floating: &'a HashSet<u32>,
    /// `Stack[]`, top first: `values[i]` is in `st(i)`.
    values: Vec<u32>,
    depth: usize,
    out: Vec<Arc<Insn>>,
    one: Option<Arc<Insn>>,
    /// The next number no instruction names.
    fresh: u32,
    status_word: RegId,
    /// Pops after a comparison that no popping form takes: they wait for the
    /// status word to be read.
    deferred: Option<Vec<usize>>,
    /// The step after the one converted, when a store to a cell without a
    /// non-popping form may swap with it: its position, what is live after it
    /// and what it writes.
    next: Option<(usize, HashSet<u32>, Vec<u32>)>,
    /// Positions of stores already emitted with the one before them.
    swallowed: BTreeSet<usize>,
}

/// `getNextFPInstruction`-free `PopTable` of the names this port emits: the
/// popping form of an instruction that acts on the top of the stack.
fn popping(what: &Semantics) -> Option<Semantics> {
    let name = what.name.as_deref()?;
    let renamed = |name: &str| Semantics { name: Some(name.into()), ..what.clone() };
    match (what.op, name) {
        (Operation::FloatStore, "fst") => Some(renamed("fstp")),
        (Operation::FloatStore, "fist") => Some(renamed("fistp")),
        (Operation::FloatStore, "fistt") => Some(renamed("fisttp")),
        (Operation::Compare, "fcom") => Some(renamed("fcomp")),
        (Operation::Compare, "fcomp") if what.sources == [st(0), st(1)] => Some(renamed("fcompp")),
        (Operation::Compare, "ficom") => Some(renamed("ficomp")),
        // `ST(i) = ST(i) op ST(0)` pops as `op p`.
        (Operation::FloatArith, _)
            if matches!(what.dests.as_slice(), [dest] if *dest != st(0))
                && matches!(
                    what.sources.as_slice(),
                    [first, second] if *first == what.dests[0] && *second == st(0)
                ) =>
        {
            Some(Semantics { op: Operation::FloatArithPop, name: Some(format!("{name}p").into()), ..what.clone() })
        }
        _ => None,
    }
}

impl Fps<'_> {
    fn one(&self) -> &Arc<Insn> {
        self.one.as_ref().expect("an instruction is being converted")
    }

    /// `pushReg`.
    fn push(
        &mut self,
        value: u32,
    ) -> Result<(), Raised> {
        if self.values.len() >= self.depth {
            return Err(unlowered("floating instruction requires too many stack operands"));
        }
        self.values.insert(0, value);
        Ok(())
    }

    fn slot(
        &self,
        value: u32,
    ) -> Result<usize, Raised> {
        self.values.iter().position(|one| *one == value).ok_or_else(|| unlowered("floating stack input is unavailable"))
    }

    /// An inserted instruction, before the one being converted.
    fn insert(
        &mut self,
        what: Semantics,
    ) {
        let at = self.one().at;
        self.out.push(Arc::new(inserted(at, what)));
    }

    /// The instruction being converted, with `what` in place of its semantics
    /// and its floating values gone from its bookkeeping.
    fn emit(
        &mut self,
        what: Semantics,
    ) {
        let one = self.one();
        let mut made = (**one).clone();
        made.what = Some(what);
        made.uses = one.uses.iter().copied().filter(|value| !self.floating.contains(value)).collect();
        made.defines = one.defines.iter().copied().filter(|value| !self.floating.contains(value)).collect();
        made.widths = one.widths.iter().copied().filter(|pair| !self.floating.contains(&pair.0)).collect();
        self.out.push(Arc::new(made));
    }

    /// Nothing is emitted for the instruction, and its bytes are still
    /// accounted for.
    fn vacate(&mut self) {
        let one = self.one();
        if one.covers.is_some_and(|(start, end)| start != end) {
            let mut made = (**one).clone();
            made.what = Some(semantics(Operation::Nothing, "", Vec::new(), Vec::new()));
            made.uses = Vec::new();
            made.defines = Vec::new();
            made.widths = Vec::new();
            made.requires = Vec::new();
            self.out.push(Arc::new(made));
        }
    }

    /// `moveToTop`: `fxch st(i)`.
    fn move_to_top(
        &mut self,
        value: u32,
    ) -> Result<(), Raised> {
        let slot = self.slot(value)?;
        if slot != 0 {
            let operands = vec![st(0), st(slot)];
            self.insert(semantics(Operation::Exchange, "fxch", operands.clone(), operands));
            self.values.swap(0, slot);
        }
        Ok(())
    }

    /// `duplicateToTop`: `fld st(i)`, the copy named `as_value`.
    fn duplicate_to_top(
        &mut self,
        value: u32,
        as_value: u32,
    ) -> Result<(), Raised> {
        let slot = self.slot(value)?;
        self.push(as_value)?;
        self.insert(semantics(Operation::FloatLoad, "fld", vec![st(0)], vec![st(slot)]));
        Ok(())
    }

    /// `popStackAfter`: the top is popped after the last instruction emitted,
    /// by its popping form where it has one, else by `fstp st(0)`.
    fn pop_stack_after(&mut self) {
        self.values.remove(0);
        if let Some(last) = self.out.last() {
            if let Some(folded) = last.what.as_ref().and_then(popping) {
                let mut made = (**last).clone();
                made.what = Some(folded);
                *self.out.last_mut().expect("an instruction") = Arc::new(made);
                return;
            }
        }
        self.insert_pop(0);
    }

    /// `fstp st(i)`: the top over slot `i`, popped.
    fn insert_pop(
        &mut self,
        slot: usize,
    ) {
        let pop = semantics(Operation::FloatStore, "fstp", vec![st(slot)], vec![st(0)]);
        match &mut self.deferred {
            Some(deferred) => deferred.push(slot),
            None => self.insert(pop),
        }
    }

    /// `freeStackSlotBefore`: `value` leaves the stack by the top being stored
    /// over it.
    fn free_stack_slot_before(
        &mut self,
        value: u32,
    ) -> Result<(), Raised> {
        let slot = self.slot(value)?;
        let top = self.values.remove(0);
        if slot > 0 {
            self.values[slot - 1] = top;
        }
        self.insert_pop(slot);
        Ok(())
    }

    /// `freeStackSlotAfter`.
    fn free_stack_slot_after(
        &mut self,
        value: u32,
    ) -> Result<(), Raised> {
        if self.values.first() == Some(&value) {
            self.pop_stack_after();
            Ok(())
        } else {
            self.free_stack_slot_before(value)
        }
    }

    /// `adjustLiveRegs`: exactly the values of `wanted` live. A killed value
    /// is given the number of a wanted one the stack lacks (free: the slot
    /// holds what the successors will not read); the rest are popped, after
    /// the last instruction where the stack top is one, else by `fstp
    /// st(i)`; a wanted value still missing is loaded as zero.
    fn adjust_live_regs(
        &mut self,
        wanted: &BTreeSet<u32>,
    ) -> Result<(), Raised> {
        let mut defs: BTreeSet<u32> = wanted.clone();
        let mut kills: BTreeSet<u32> = BTreeSet::new();
        for value in &self.values {
            if !defs.remove(value) {
                kills.insert(*value);
            }
        }
        while let (Some(&kill), Some(&def)) = (kills.first(), defs.first()) {
            let slot = self.slot(kill)?;
            self.values[slot] = def;
            kills.remove(&kill);
            defs.remove(&def);
        }
        if !kills.is_empty() && !self.out.is_empty() {
            while let Some(&top) = self.values.first() {
                if !kills.contains(&top) {
                    break;
                }
                self.pop_stack_after();
                kills.remove(&top);
            }
        }
        for kill in kills {
            self.free_stack_slot_before(kill)?;
        }
        for def in defs {
            self.insert(semantics(Operation::FloatLoad, "fldz", vec![st(0)], Vec::new()));
            self.push(def)?;
        }
        debug_assert_eq!(self.values.len(), wanted.len(), "live count mismatch");
        Ok(())
    }

    /// `shuffleStackTop`: the stack in the order `fixed` says, `st(0)` first.
    fn shuffle_stack_top(
        &mut self,
        fixed: &[u32],
    ) -> Result<(), Raised> {
        for count in (0..fixed.len()).rev() {
            let old = self.values[count];
            let wanted = fixed[count];
            if old == wanted {
                continue;
            }
            self.move_to_top(wanted)?;
            if count > 0 {
                self.move_to_top(old)?;
            }
        }
        Ok(())
    }

    /// `setupBlockStack`.
    fn setup_block_stack(
        &mut self,
        bundle: &mut LiveBundle,
        live_in: &BTreeSet<u32>,
    ) -> Result<(), Raised> {
        self.values.clear();
        if !bundle.mask.is_empty() {
            let order = bundle.fixed.get_or_insert_with(|| bundle.mask.iter().copied().collect());
            self.values = order.clone();
        }
        self.adjust_live_regs(live_in)
    }

    /// `finishBlockStack` for a block with successors.
    fn finish_block_stack(
        &mut self,
        bundle: &mut LiveBundle,
        hint: Option<Vec<u32>>,
    ) -> Result<(), Raised> {
        self.adjust_live_regs(&bundle.mask.clone())?;
        if bundle.mask.is_empty() {
            return Ok(());
        }
        if bundle.fixed.is_none() {
            bundle.fixed = hint.filter(|order| order.len() == self.values.len());
        }
        match bundle.fixed.clone() {
            Some(fixed) => self.shuffle_stack_top(&fixed)?,
            None => bundle.fixed = Some(self.values.clone()),
        }
        Ok(())
    }

    /// A step whose operands are floating values: `FPInstClass` and the
    /// handler.
    fn convert(
        &mut self,
        step: &Step,
        block: &Block,
    ) -> Result<(), Raised> {
        let one = Arc::clone(&block.insns[step.positions[0]]);
        self.one = Some(Arc::clone(&one));
        let mut what = one.what.clone().expect("a floating instruction has semantics");
        // A float copied to or from a memory cell (a spilled phi copy) is a
        // store or a load of it.
        if what.op == Operation::Move && what.name.as_deref() == Some("mov") {
            let floating = |loc: &Loc| matches!(loc, Loc::Held(held) if held.width == 10);
            match (what.dests.as_slice(), what.sources.as_slice()) {
                ([Loc::Mem(_)], [source]) if floating(source) => {
                    what = Semantics { op: Operation::FloatStore, name: Some("fstp".into()), ..what };
                }
                ([dest], [Loc::Mem(_)]) if floating(dest) => {
                    what = Semantics { op: Operation::FloatLoad, name: Some("fld".into()), ..what };
                }
                _ => {}
            }
            if one.what.as_ref() != Some(&what) {
                let mut changed = (*one).clone();
                changed.what = Some(what.clone());
                self.one = Some(Arc::new(changed));
            }
        }
        let held = |args: &[Loc]| -> Vec<u32> {
            args.iter()
                .filter_map(|arg| match arg {
                    Loc::Held(held) if held.width == 10 => Some(held.value),
                    _ => None,
                })
                .collect()
        };
        let operands = held(&what.sources);
        let results = held(&what.dests);
        if results.len() > 1 {
            return Err(unlowered("floating stack result is not a fresh value"));
        }
        let two = _two_values(Some(&what));
        if step.positions.len() > 1 {
            self.copy_group(step, block)?;
        } else if let (Some((name, left, right)), false) = (&two, results.is_empty()) {
            self.handle_two_arg_fp(step, name, *left, *right, results[0])?;
        } else if what.op == Operation::FloatLoad && unnamed(&what) && what.sources.is_empty() && !results.is_empty() {
            // `handleCall`'s STReturns: the callee left the result in st(0).
            // The stack was empty at the call (`flush`), so the result is the
            // bottom entry: a value the allocator reloaded after
            // the call (a `fld` ahead of this pseudo) was pushed on top of it.
            if self.values.len() >= self.depth {
                return Err(unlowered("floating instruction requires too many stack operands"));
            }
            self.values.push(results[0]);
            self.vacate();
        } else if what.op == Operation::FloatStore && unnamed(&what) && what.dests.is_empty() && operands.len() == 1 {
            self.handle_return(operands[0])?;
        } else if what.op == Operation::Compare && operands.len() == 2 && results.is_empty() {
            self.handle_compare_fp(step, operands[0], operands[1])?;
        } else if what.op == Operation::Compare
            && operands.len() == 1
            && results.is_empty()
            && matches!(what.sources.as_slice(), [Loc::Held(_), Loc::Mem(_)])
        {
            self.handle_compare_memory(step, operands[0], what.sources[1].clone())?;
        } else if what.op == Operation::FloatLoad && operands.is_empty() && !results.is_empty() {
            // `handleZeroArgFP`.
            self.emit(Semantics { dests: vec![st(0)], ..what });
            self.push(results[0])?;
        } else if matches!(what.op, Operation::FloatLoad | Operation::Move)
            && operands.len() == 1
            && !results.is_empty()
        {
            self.copy_group(step, block)?;
        } else if what.op == Operation::FloatStore && operands.len() == 1 && results.is_empty() {
            self.handle_one_arg_fp(step, block, operands[0])?;
        } else if what.op == Operation::FloatUnary && operands.len() == 1 && !results.is_empty() {
            let what = Semantics { dests: vec![st(0)], sources: vec![st(0)], ..what };
            self.handle_one_arg_fprw(step, operands[0], what, results[0])?;
        } else if what.op == Operation::FloatArith
            && !operands.is_empty()
            && _floating(&what.sources[0])
            && !results.is_empty()
        {
            let Loc::Held(kept) = what.sources[0] else { unreachable!("checked above") };
            let sources = std::iter::once(st(0)).chain(what.sources[1..].iter().cloned()).collect();
            self.handle_one_arg_fprw(step, kept.value, Semantics { dests: vec![st(0)], sources, ..what }, results[0])?;
        } else {
            return Err(unlowered(&format!(
                "floating instruction has no conversion rule: {:?} {:?} of {} operands, {} results ({:?} <- {:?})",
                what.op,
                what.name,
                operands.len(),
                results.len(),
                what.dests,
                what.sources
            )));
        }
        // A result nothing reads is popped where it is defined.
        for &result in &step.defs {
            if step.dead(result) && self.values.contains(&result) {
                self.free_stack_slot_after(result)?;
            }
        }
        Ok(())
    }

    /// `handleReturn`: the returned value is st(0) and the stack is otherwise
    /// empty.
    fn handle_return(
        &mut self,
        value: u32,
    ) -> Result<(), Raised> {
        self.adjust_live_regs(&BTreeSet::from([value]))?;
        if self.values != [value] {
            return Err(unlowered("a returned float leaves other values on the stack"));
        }
        self.values.clear();
        self.vacate();
        Ok(())
    }

    /// The next instruction as `fst` (a store of `source` that kills it, to a
    /// 4 or 8 byte cell apart from `first`'s), which then emits in the place of
    /// both; marks it swallowed. None when the pair does not fit.
    fn swapped_with(
        &mut self,
        block: &Block,
        source: u32,
        first: &Semantics,
    ) -> Option<Semantics> {
        let (position, live_after, defs) = self.next.take()?;
        if defs.contains(&source) || live_after.contains(&source) {
            return None;
        }
        let next = block.insns[position].what.as_ref()?;
        let stores_source = next.op == Operation::FloatStore
            && matches!(
                next.sources.as_slice(),
                [Loc::Held(held)] if held.value == source && held.width == 10
            );
        let (Some("fst" | "fstp"), [Loc::Mem(to)], [Loc::Mem(from)]) =
            (next.name.as_deref(), next.dests.as_slice(), first.dests.as_slice())
        else {
            return None;
        };
        if !stores_source || !matches!(to.width, 4 | 8) || !_apart(to, from) {
            return None;
        }
        self.swallowed.insert(position);
        Some(Semantics { name: Some("fst".into()), sources: vec![st(0)], ..next.clone() })
    }

    /// `handleOneArgFP`: a store of the top.
    fn handle_one_arg_fp(
        &mut self,
        step: &Step,
        block: &Block,
        source: u32,
    ) -> Result<(), Raised> {
        let what = self.one().what.clone().expect("a floating instruction has semantics");
        self.move_to_top(source)?;
        let name = if name_is(&what, "fst") { Some("fstp".into()) } else { what.name.clone() };
        if !step.kills(source) {
            let sized = |widths: &[u32]| {
                what.dests
                    .iter()
                    .all(
                        |dest| matches!(
                            dest,
                            Loc::Mem(dest) if widths.contains(&u32::from(dest.width))
                        ),
                    )
            };
            // The forms that leave the value: `fst`, `fist`, `fistt` (the 387
            // has `fist` for a word and a dword only). The rest have only a
            // popping form: the copy is stored (`ISTT_Fp..`, `IST_Fp64m`).
            let kept = match name.as_deref() {
                Some("fstp") if sized(&[4, 8]) => Some("fst"),
                Some("fistp") if sized(&[2, 4]) => Some("fist"),
                Some("fisttp") if sized(&[2, 4]) => Some("fistt"),
                _ => None,
            };
            if let Some(kept) = kept {
                self.emit(Semantics { name: Some(kept.into()), sources: vec![st(0)], ..what });
                return Ok(());
            }
            // DEVIATE 8: a store with no non-popping form (`fstp tbyte`) of a
            // value the very next instruction stores and kills: that store
            // keeps the value (`fst`) and this one pops it, where `fld st(0)`
            // would copy the value first. The cells must be apart.
            if let Some(next) = self.swapped_with(block, source, &what) {
                self.emit(next);
                self.emit(Semantics { name, sources: vec![st(0)], ..what });
                self.values.remove(0);
                return Ok(());
            }
            let scratch = self.take_fresh();
            self.duplicate_to_top(source, scratch)?;
        }
        self.emit(Semantics { name, sources: vec![st(0)], ..what });
        self.values.remove(0);
        Ok(())
    }

    /// `handleOneArgFPRW`: the top replaced by a value computed from it.
    fn handle_one_arg_fprw(
        &mut self,
        step: &Step,
        source: u32,
        what: Semantics,
        result: u32,
    ) -> Result<(), Raised> {
        if step.kills(source) {
            self.move_to_top(source)?;
            self.values[0] = result;
        } else {
            self.duplicate_to_top(source, result)?;
        }
        self.emit(what);
        Ok(())
    }

    /// `handleTwoArgFP`: `result := left op right`.
    fn handle_two_arg_fp(
        &mut self,
        step: &Step,
        name: &str,
        left: Held,
        right: Held,
        dest: u32,
    ) -> Result<(), Raised> {
        let (mut op0, op1) = (left.value, right.value);
        let (mut kills_op0, kills_op1) = (step.kills(op0), step.kills(op1));
        self.slot(op0)?;
        self.slot(op1)?;
        let mut tos = self.values[0];
        if op0 != tos && op1 != tos {
            if kills_op0 {
                self.move_to_top(op0)?;
                tos = op0;
            } else if kills_op1 {
                self.move_to_top(op1)?;
                tos = op1;
            } else {
                if self.square_keeps_the_next_read_on_top(step, name, op0, op1, dest)? {
                    return Ok(());
                }
                self.duplicate_to_top(op0, dest)?;
                op0 = dest;
                tos = dest;
                kills_op0 = true;
            }
        } else if !kills_op0 && !kills_op1 {
            if self.square_keeps_the_next_read_on_top(step, name, op0, op1, dest)? {
                return Ok(());
            }
            self.duplicate_to_top(op0, dest)?;
            op0 = dest;
            tos = dest;
            kills_op0 = true;
        }
        debug_assert!((tos == op0 || tos == op1) && (kills_op0 || kills_op1), "stack conditions not set up right");
        let is_forward = tos == op0;
        let update_st0 = (tos == op0 && !kills_op1) || (tos == op1 && !kills_op0);
        let not_tos = if tos == op0 { op1 } else { op0 };
        let other = self.slot(not_tos)?;
        // The four tables: ST(0) = ST(0) op ST(i), ST(0) = ST(i) op ST(0),
        // ST(i) = ST(0) op ST(i), ST(i) = ST(i) op ST(0).
        let (operation, target) = match (update_st0, is_forward) {
            (true, true) => (name.to_owned(), 0),
            (true, false) => (_reversed(name).to_owned(), 0),
            (false, true) => (_reversed(name).to_owned(), other),
            (false, false) => (name.to_owned(), other),
        };
        let sources = if target == 0 { vec![st(0), st(other)] } else { vec![st(other), st(0)] };
        self.insert_arith(operation, target, sources);
        if kills_op0 && kills_op1 && op0 != op1 {
            debug_assert!(!update_st0, "should have updated the other operand");
            self.pop_stack_after();
        }
        // The destination takes the slot that was updated.
        let slot = self.slot(if update_st0 { tos } else { not_tos })?;
        self.values[slot] = dest;
        Ok(())
    }

    /// DEVIATE 5: `v op v` where `v` stays. `handleTwoArgFP` copies `v` to the
    /// top and puts the result there. When `v` is read again before the result
    /// is, the copy keeps the name `v` and the result takes the slot below it
    /// (`fmul st(1), st(0)`), so the next read finds `v` on top and no `fxch`
    /// follows (the old pass's rule, FPDEEP's `p*p` then `p+p`). True when it
    /// emitted the instruction.
    fn square_keeps_the_next_read_on_top(
        &mut self,
        step: &Step,
        name: &str,
        op0: u32,
        op1: u32,
        dest: u32,
    ) -> Result<bool, Raised> {
        if op0 != op1 {
            return Ok(false);
        }
        let (later, product) = (step.next_read.get(&op0), step.next_read.get(&dest));
        if !matches!(
            (later, product),
            (Some(later), Some(product)) if later < product
        ) {
            return Ok(false);
        }
        self.duplicate_to_top(op0, op0)?;
        // The copy is `v` on top; the result goes in the slot below, which the
        // original `v` held.
        let below =
            self.values[1..].iter().position(|value| *value == op0).map(|slot| slot + 1).expect("v is below its copy");
        self.insert_arith(name.to_owned(), below, vec![st(below), st(0)]);
        self.values[below] = dest;
        Ok(true)
    }

    fn insert_arith(
        &mut self,
        operation: String,
        target: usize,
        sources: Vec<Loc>,
    ) {
        let what = semantics(Operation::FloatArith, &operation, vec![st(target)], sources);
        // The converted instruction stands for the pseudo: it carries its
        // bookkeeping.
        self.emit(what);
    }

    /// `handleCompareFP`: `left` against `right` where it lies, the answer
    /// moved from the status word into the flags.
    fn handle_compare_fp(
        &mut self,
        step: &Step,
        left: u32,
        right: u32,
    ) -> Result<(), Raised> {
        self.slot(right)?;
        self.move_to_top(left)?;
        let slot = self.slot(right)?;
        self.emit(semantics(Operation::Compare, "fcom", Vec::new(), vec![st(0), st(slot)]));
        self.deferred = Some(Vec::new());
        let (kills_left, kills_right) = (step.kills(left), step.kills(right));
        let result: Result<(), Raised> = (|| {
            if kills_left {
                self.free_stack_slot_after(left)?;
            }
            if kills_right && left != right {
                self.free_stack_slot_after(right)?;
            }
            Ok(())
        })();
        let deferred = self.deferred.take().unwrap_or_default();
        result?;
        self.status()?;
        for slot in deferred {
            self.insert(semantics(Operation::FloatStore, "fstp", vec![st(slot)], vec![st(0)]));
        }
        Ok(())
    }

    /// `left` against a cell: `fcom m`.
    fn handle_compare_memory(
        &mut self,
        step: &Step,
        left: u32,
        cell: Loc,
    ) -> Result<(), Raised> {
        self.move_to_top(left)?;
        let what = self.one().what.clone().expect("a floating instruction has semantics");
        let integer = what.name.as_deref().is_some_and(|name| name.starts_with("fi"));
        let name = if integer { "ficom" } else { "fcom" };
        self.emit(Semantics { name: Some(name.into()), dests: Vec::new(), sources: vec![st(0), cell], ..what });
        if step.kills(left) {
            self.pop_stack_after();
        }
        self.status()
    }

    /// Simultaneous copies, `(result, source)`: `COPY` for one, gcc's
    /// `move_for_stack_reg` for a group (DEVIATE 2).
    fn copy_group(
        &mut self,
        step: &Step,
        block: &Block,
    ) -> Result<(), Raised> {
        let pairs: Vec<(u32, u32)> =
            step.positions.iter().filter_map(|&position| _float_copy(&block.insns[position])).collect();
        let results: HashSet<u32> = pairs.iter().map(|(result, _)| *result).collect();
        if let Some((_, missing)) = pairs.iter().find(|(_, source)| !self.values.contains(source)) {
            return Err(unlowered(&format!("floating copy source {missing} is not on the stack")));
        }
        let mut copied = Vec::new();
        for slot in 0..self.values.len() {
            let value = self.values[slot];
            let mut wanted = pairs.iter().filter(|(_, source)| *source == value).map(|(result, _)| *result);
            let kept = !results.contains(&value) && step.live_after.contains(&value);
            if !kept {
                self.values[slot] = wanted.next().unwrap_or(if results.contains(&value) { u32::MAX } else { value });
            }
            copied.extend(wanted.map(|result| (self.values[slot], result)));
        }
        for (from, result) in copied {
            self.duplicate_to_top(from, result)?;
        }
        // A value a copy overwrote is dead: popped.
        while let Some(slot) = self.values.iter().position(|value| *value == u32::MAX) {
            self.values[slot] = self.values[0];
            self.values.remove(0);
            if slot == 0 {
                self.insert_pop(0);
            } else {
                self.insert_pop(slot);
            }
        }
        self.vacate();
        Ok(())
    }

    fn take_fresh(&mut self) -> u32 {
        self.fresh += 1;
        self.fresh - 1
    }

    /// The comparison defines flags through SAHF: `fnstsw ax; sahf`.
    fn status(&mut self) -> Result<(), Raised> {
        let one = Arc::clone(self.one());
        let produced: HashSet<u32> = one.defines.iter().copied().collect();
        let comparison = self.out.last().expect("a comparison was emitted");
        let mut made = (**comparison).clone();
        made.defines = comparison.defines.iter().copied().filter(|value| !produced.contains(value)).collect();
        made.delivers =
            comparison.delivers.iter().copied().filter(|(held, _)| !produced.contains(&held.value)).collect();
        made.widths = comparison.widths.iter().copied().filter(|pair| !produced.contains(&pair.0)).collect();
        *self.out.last_mut().expect("a comparison was emitted") = Arc::new(made);
        let at = one.at;
        let status = semantics(
            Operation::Barrier,
            "fnstsw",
            vec![Loc::Reg(Reg { register: self.status_word, width: 2 })],
            Vec::new(),
        );
        let mut word = Insn::new(at, Some((at, at)), Some(status), one.defines.clone(), Vec::new());
        word.clobbers = BTreeSet::from([self.status_word]);
        word.delivers = one.delivers.clone();
        word.widths = one.widths.iter().copied().filter(|pair| produced.contains(&pair.0)).collect();
        let held = match word.delivers.iter().find(|(_, register)| *register == self.status_word) {
            Some(&(held, _)) => held,
            None => {
                let held = Held { value: self.take_fresh(), width: 2 };
                word.defines.push(held.value);
                word.delivers.push((held, self.status_word));
                word.widths.push((held.value, 2));
                held
            }
        };
        self.out.push(Arc::new(word));
        let mut sahf = inserted(at, semantics(Operation::Nothing, "sahf", Vec::new(), Vec::new()));
        sahf.uses = vec![held.value];
        sahf.requires = vec![(held, self.status_word)];
        self.out.push(Arc::new(sahf));
        Ok(())
    }
}

/// `setKillFlags`: the steps of `block` with what each reads, writes and leaves
/// live, scanning backwards from `live_out`.
fn steps(
    block: &Block,
    cut: usize,
    live_out: &BTreeSet<u32>,
) -> Vec<Step> {
    let mut found: Vec<Step> = Vec::new();
    let mut absorbed: HashSet<usize> = HashSet::default();
    let float = |args: &[Loc]| -> Vec<u32> {
        args.iter()
            .filter_map(|arg| match arg {
                Loc::Held(held) if held.width == 10 => Some(held.value),
                _ => None,
            })
            .collect()
    };
    for position in 0..cut {
        let one = &block.insns[position];
        if absorbed.contains(&position) {
            // A later copy of a group already taken: nothing is emitted for it.
            found.push(Step {
                positions: vec![position],
                uses: Vec::new(),
                defs: Vec::new(),
                live_after: HashSet::default(),
                absorbed: true,
                next_read: IndexMap::default(),
            });
            continue;
        }
        let mut positions = vec![position];
        // A group's copies are simultaneous: all read at its first, all
        // written at its last, and taken together at the first.
        if let (Some(group), Some(_)) = (one.group, _float_copy(one)) {
            for later in position + 1..cut {
                let other = &block.insns[later];
                if other.group == Some(group) && _float_copy(other).is_some() {
                    positions.push(later);
                    absorbed.insert(later);
                }
            }
        }
        let mut uses = Vec::new();
        let mut defs = Vec::new();
        for &at in &positions {
            if let Some(what) = &block.insns[at].what {
                uses.extend(float(&what.sources));
                defs.extend(float(&what.dests));
            }
        }
        found.push(Step {
            positions,
            uses,
            defs,
            live_after: HashSet::default(),
            absorbed: false,
            next_read: IndexMap::default(),
        });
    }
    let mut live: HashSet<u32> = live_out.iter().copied().collect();
    let mut next_read: IndexMap<u32, usize> = IndexMap::default();
    for (index, step) in found.iter_mut().enumerate().rev() {
        step.live_after = live.clone();
        step.next_read = next_read.clone();
        for def in &step.defs {
            live.remove(def);
            next_read.shift_remove(def);
        }
        live.extend(step.uses.iter().copied());
        for used in &step.uses {
            next_read.insert(*used, index);
        }
    }
    found
}

/// DEVIATE 6: the order a bundle still to be fixed should take.
/// `finishBlockStack` lets the first exit to reach a bundle choose its order.
/// A block that only passes the stack on (no floating instruction, or only
/// copies, which rename) leads on to a bundle that may already be fixed (a
/// loop's header, reached by the back edge): the order that bundle has, seen
/// back through the renames, is taken, so the exit that fixes it shuffles once
/// there and no later edge of the loop shuffles again. Followed through at most
/// `DEPTH` such blocks.
const DEPTH: usize = 3;

thread_local! {
    /// The blocks `Ahead::renames` has scanned, counted for the test that fixes
    /// it at one scan a block.
    pub(super) static RENAME_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What `Ahead::renames` found, by block.
type Renamed = RefCell<HashMap<i64, Option<Vec<(u32, u32)>>>>;

struct Ahead<'a> {
    renamed: &'a Renamed,
    at_of: &'a IndexMap<i64, &'a Block>,
    bundles: &'a spillplacement::Bundles,
    live_bundles: &'a IndexMap<usize, LiveBundle>,
    floating: &'a HashSet<u32>,
}

impl Ahead<'_> {
    /// `(result, source)` of the copies of a block that does nothing else with
    /// a floating value.
    fn renames(
        &self,
        block: &Block,
    ) -> Option<Vec<(u32, u32)>> {
        if let Some(found) = self.renamed.borrow().get(&block.at) {
            return found.clone();
        }
        let found = self.scanned(block);
        self.renamed.borrow_mut().insert(block.at, found.clone());
        found
    }

    /// `renames`, worked out: asked once a block, since it reads every
    /// instruction and `order` asks it of each block ahead again for every
    /// block behind it.
    fn scanned(
        &self,
        block: &Block,
    ) -> Option<Vec<(u32, u32)>> {
        RENAME_SCANS.with(|scans| scans.set(scans.get() + 1));
        let mut pairs = Vec::new();
        for one in &block.insns {
            let floats = one.what.as_ref().is_some_and(|what| what.sources.iter().chain(&what.dests).any(_floating));
            if !floats {
                if one.uses.iter().chain(&one.defines).any(|value| self.floating.contains(value)) {
                    return None;
                }
            } else if let Some(pair) = _float_copy(one) {
                pairs.push(pair);
            } else {
                return None;
            }
        }
        Some(pairs)
    }

    /// The order the values entering `at` should be in, from what lies ahead.
    fn order(
        &self,
        at: i64,
        depth: usize,
    ) -> Option<Vec<u32>> {
        let block = self.at_of.get(&at)?;
        let pairs = self.renames(block)?;
        let exit = &self.live_bundles.get(&self.bundles.of[&at].1)?;
        let after = match &exit.fixed {
            Some(fixed) => fixed.clone(),
            None if depth > 0 => block.succ.iter().find_map(|next| self.order(*next, depth - 1))?,
            None => return None,
        };
        // Back through the copies: a value a copy wrote was the copy's source.
        Some(
            after
                .iter()
                .map(|value| pairs.iter().find(|(result, _)| result == value).map_or(*value, |(_, source)| *source))
                .collect(),
        )
    }
}

fn successor_order(
    ahead: &Ahead,
    block: &Block,
    mask: &BTreeSet<u32>,
) -> Option<Vec<u32>> {
    block
        .succ
        .iter()
        .find_map(|next| ahead.order(*next, DEPTH))
        .filter(|order| order.len() == mask.len() && order.iter().all(|value| mask.contains(value)))
}

/// Blocks in depth-first preorder from the entry, then the unreachable ones.
fn depth_first(body: &LirBody) -> Vec<i64> {
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut seen: HashSet<i64> = HashSet::default();
    let mut order = Vec::new();
    let mut pending: Vec<(i64, usize)> = vec![(body.entry, 0)];
    if at_of.contains_key(&body.entry) {
        seen.insert(body.entry);
        order.push(body.entry);
    }
    while let Some((at, next)) = pending.pop() {
        let successors = at_of.get(&at).map_or(&[][..], |block| block.succ.as_slice());
        if let Some(&successor) = successors.get(next) {
            pending.push((at, next + 1));
            if at_of.contains_key(&successor) && seen.insert(successor) {
                order.push(successor);
                pending.push((successor, 0));
            }
        }
    }
    order.extend(body.blocks.iter().map(|block| block.at).filter(|at| !seen.contains(at)));
    order
}

/// The whole function in stack form.
pub(super) fn converted(
    body: &LirBody,
    classes: &RegisterClasses,
) -> Result<LirBody, Raised> {
    let valued = crate::backend::fpregs::valued(body);
    let body = valued.as_ref().unwrap_or(body);
    let regs = body.regs();
    let floating = floatassign::_floating_values(body);
    if floating.is_empty() {
        return Ok(body.clone());
    }
    let (live_in, live_out) = live_among(body, |value| floating.contains(&value));
    let bundles = spillplacement::bundles(body);
    // DEVIATE 7: a block of its own on each critical forward edge into a block
    // that reads fewer floating values than its bundle holds, where the
    // values it does not read are popped. FPS leaves such an edge alone and the
    // block pops them at its entry, which makes every other way in define them
    // first (`fldz`, then the pop): nbody's inner loop exit ran 4 instructions
    // more a trip (+5.2% instructions at -O2).
    let split = floatalloc::_split_for_the_stack(body, &floating, (&live_in, &live_out), &bundles);
    let (body, live_in, live_out, bundles) = match split {
        Some(body) => {
            // A block made on an edge holds no instruction: what is live at its
            // entry and its exit is what is live into the block it
            // leads to, and no other block's rows change.
            let (mut live_in, mut live_out) = (live_in, live_out);
            for block in &body.blocks {
                if !live_in.contains_key(&block.at) {
                    let into = live_in[&block.succ[0]].clone();
                    live_out.insert(block.at, into.clone());
                    live_in.insert(block.at, into);
                }
            }
            let bundles = spillplacement::bundles(&body);
            (std::borrow::Cow::Owned(body), live_in, live_out, bundles)
        }
        None => (std::borrow::Cow::Borrowed(body), live_in, live_out, bundles),
    };
    let body = &*body;
    let floats = |set: &llrm_mir::dense::IdSet<u32>| -> BTreeSet<u32> {
        set.iter().filter(|value| floating.contains(value)).collect()
    };
    let mut live_bundles: IndexMap<usize, LiveBundle> = IndexMap::default();
    for block in &body.blocks {
        let (entry, exit) = bundles.of[&block.at];
        live_bundles.entry(entry).or_default().mask.extend(floats(&live_in[&block.at]));
        live_bundles.entry(exit).or_default().mask.extend(floats(&live_out[&block.at]));
    }
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let status_word = classes.status_word().ok_or_else(|| unlowered("the target has no x87 status word register"))?;
    let fresh =
        body.blocks
            .iter()
            .flat_map(|block| {
                block
                    .insns
                    .iter()
                    .flat_map(|one| one.defines.iter().chain(&one.uses))
                    .copied()
                    .chain(
                        block
                            .phis
                            .iter()
                            .flat_map(
                                |phi| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)),
                            ),
                    )
            })
            .max()
            .map_or(0, |most| most + 1);
    let mut fps = Fps {
        floating: &floating,
        values: Vec::new(),
        depth: body.float_stack,
        out: Vec::new(),
        one: None,
        fresh,
        status_word,
        deferred: None,
        next: None,
        swallowed: BTreeSet::new(),
    };
    let border = |at: i64| Arc::new(inserted(at, semantics(Operation::Nothing, "", Vec::new(), Vec::new())));
    let mut made: IndexMap<i64, Block> = IndexMap::default();
    let renamed: Renamed = RefCell::new(HashMap::default());
    for at in depth_first(body) {
        let block = at_of[&at];
        let (entry, exit) = bundles.of[&at];
        fps.out = Vec::new();
        fps.one = Some(border(block.insns.first().map_or(block.at, |first| first.at)));
        let mut bundle = std::mem::take(live_bundles.get_mut(&entry).expect("a bundle"));
        let entered = fps.setup_block_stack(&mut bundle, &floats(&live_in[&at]));
        live_bundles.insert(entry, bundle);
        entered?;
        let cut = _terminators(block);
        let steps = steps(block, cut, &floats(&live_out[&at]));
        for (at_step, step) in steps.iter().enumerate() {
            let one = &block.insns[step.positions[0]];
            if step.absorbed || fps.swallowed.contains(&step.positions[0]) {
                fps.one = Some(Arc::clone(one));
                fps.vacate();
                continue;
            }
            let floats_here =
                one.what.as_ref().is_some_and(|what| what.sources.iter().chain(&what.dests).any(_floating));
            if !floats_here {
                if one.uses.iter().chain(&one.defines).any(|value| floating.contains(value)) {
                    return Err(unlowered("floating value used by an unmodelled instruction"));
                }
                if clears_stack(regs, one) {
                    fps.one = Some(Arc::clone(one));
                    // `handleCall`: nothing may be on the stack across it.
                    if !fps.values.is_empty() {
                        return Err(unlowered("a floating value is live across a boundary"));
                    }
                }
                fps.out.push(Arc::clone(one));
            } else {
                fps.next = steps
                    .get(at_step + 1)
                    .filter(|_| _unkept_store(one.what.as_ref()))
                    .map(|next| (next.positions[0], next.live_after.clone(), next.defs.clone()));
                fps.convert(step, block)?;
            }
        }
        fps.one = Some(border(block.insns.get(cut).or(block.insns.last()).map_or(block.at, |one| one.at)));
        if block.succ.iter().any(|successor| at_of.contains_key(successor)) {
            let mut bundle = std::mem::take(live_bundles.get_mut(&exit).expect("a bundle"));
            let ahead = Ahead {
                renamed: &renamed,
                at_of: &at_of,
                bundles: &bundles,
                live_bundles: &live_bundles,
                floating: &floating,
            };
            let hint = successor_order(&ahead, block, &bundle.mask);
            let finished = fps.finish_block_stack(&mut bundle, hint);
            live_bundles.insert(exit, bundle);
            finished?;
        } else if !fps.values.is_empty() {
            return Err(unlowered("floating value on the stack where the function leaves"));
        }
        fps.out.extend(block.insns[cut..].iter().cloned());
        let mut one = block.clone();
        one.insns = std::mem::take(&mut fps.out).into();
        made.insert(at, one.into_inner().into());
    }
    let mut out = body.clone();
    out.blocks = body.blocks.iter().map(|block| made.shift_remove(&block.at).expect("every block converted")).collect();
    Ok(out)
}

#[cfg(test)]
#[path = "fpstack_tests.rs"]
mod tests;
