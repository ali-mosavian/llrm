//! Frame lowering: the frame's layout, the prologue and epilogue, and the
//! frame objects turned into stack-pointer offsets
//! (`AArch64FrameLowering::emitPrologue`, `emitEpilogue`,
//! `eliminateFrameIndex`). The frame, from the stack pointer up: outgoing
//! arguments, frame objects, the callee-saved registers in pairs, and the frame
//! record (`x29`, `x30`) at the top. Offsets are from the stack pointer, which
//! does not move in the body.
//!
//! A frame of only saved registers moves the stack pointer in the first pair's
//! store and last load (`stp [sp, #-N]!`, `ldp [sp], #N`). A `Return` that
//! carries a callee is a sibling call ([`crate::sibcall`]): the epilogue, then
//! `b`.
//!
//! Shrink wrapping: where the prologue and epilogue go is
//! [`crate::shrinkwrap`]'s.

use llrm_object::assembler::Cfi;
use rustc_hash::FxHashSet as HashSet;

use crate::cfi::Unwind;
use crate::mc::inst::*;
use crate::mc::unwind;
use crate::mc::visit::Reg;
use crate::mfunc::{Callee, INCOMING_ARGUMENTS, MFunc, Op};
use crate::regalloc::Allocation;

/// A block of real instructions.
#[derive(Clone, Debug)]
pub struct Lowered {
    pub label: String,
    pub insts: Vec<Inst>,
    /// The source line and column each instruction is from, parallel to
    /// `insts`.
    pub locs: Vec<Option<(u32, u32)>>,
}

#[derive(Debug)]
struct Layout {
    size: u64,
    /// The stack pointer moves in the body: objects are addressed from `x29`.
    dynamic: bool,
    /// Each frame object's offset from the stack pointer.
    slots: Vec<u64>,
    /// The register pairs saved and the frame record, lowest address first.
    saves: Vec<(Save, u64)>,
    frame_record: Option<u64>,
    /// The bytes of the frame below the saved registers (the locals).
    locals: u64,
}

/// One store of the callee-saved area: the register at the lower address
/// first.
#[derive(Clone, Copy, Debug)]
enum Save {
    GprPair(u32, Option<u32>),
    FprPair(u32, Option<u32>),
    /// `x29` and `x30`.
    Record,
}

/// A callee-saved register, as the pin's list (`CalleeSavedInfo`) names it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Csr {
    Lr,
    Fp,
    X(u32),
    D(u32),
}

/// The registers `allocation` saves, the pin's list order, which is filled
/// from the top of the area down. Darwin's list starts with the frame record
/// (`CSR_Darwin_AArch64_AAPCS`), Linux's has it after `x28`
/// (`CSR_AArch64_AAPCS`). Mach-O saves only pairs, so a register whose partner
/// is not saved takes the partner too (`determineCalleeSaves`).
fn saved_list(
    allocation: &Allocation,
    record: bool,
    link: bool,
    darwin: bool,
) -> Vec<Csr> {
    let pad = |list: &[u32], base: u32| -> Vec<u32> {
        let mut all = list.to_vec();
        if darwin {
            all.extend(list.iter().map(|&n| base + ((n - base) ^ 1)));
        }
        all.sort_unstable();
        all.dedup();
        all
    };
    let gprs = pad(&allocation.saved_gprs, 19).into_iter().map(Csr::X).chain(link.then_some(Csr::X(30)));
    let fprs = pad(&allocation.saved_fprs, 8).into_iter().map(Csr::D);
    let frame = [Csr::Lr, Csr::Fp].into_iter().filter(|_| record);
    if darwin { frame.chain(gprs).chain(fprs).collect() } else { gprs.chain(frame).chain(fprs).collect() }
}

/// The pairs and singles `list` is stored as, top first
/// (`computeCalleeSaveRegisterPairs`): neighbours of one class pair, but
/// nothing but `x29` pairs with `x30`.
fn paired(list: &[Csr]) -> Vec<(Csr, Option<Csr>)> {
    let class = |register: Csr| match register {
        Csr::X(_) => 0,
        Csr::D(_) => 1,
        Csr::Lr | Csr::Fp => 2,
    };
    let mut out = Vec::new();
    let mut at = 0;
    while at < list.len() {
        let first = list[at];
        let second = list
            .get(at + 1)
            .copied()
            .filter(|&next| next != Csr::Lr && (class(first) == class(next) || (first, next) == (Csr::Lr, Csr::Fp)));
        out.push((first, second));
        at += 1 + usize::from(second.is_some());
    }
    out
}

/// How many operations name each frame object.
fn object_uses(function: &MFunc) -> Vec<u64> {
    let mut uses = vec![0; function.slots.len()];
    for block in &function.blocks {
        for op in &block.ops {
            match op {
                Op::I(
                    Inst::Load { addr: Addr::Slot { slot, .. }, .. }
                    | Inst::Store { addr: Addr::Slot { slot, .. }, .. },
                )
                | Op::FrameAddress { slot, .. } => {
                    if let Some(count) = uses.get_mut(*slot as usize) {
                        *count += 1;
                    }
                }
                _ => {}
            }
        }
    }
    uses
}

/// Each frame object's offset from the stack pointer, and the locals' size.
///
/// The objects are in the order of `X86FrameLowering::orderFrameObjects`
/// ([permalink](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FrameLowering.cpp#L4301)),
/// which AArch64's own does not have: the densest (uses per byte) nearest the
/// register they are addressed from, so that the offsets are the small ones
/// and the first access can be the one at `[sp]` that the frame's allocation
/// folds into (`str x, [sp, #-N]!`), the one the entry block touches first.
/// From the stack pointer that means the alignment slack goes at the top of the
/// locals; from the frame pointer, at the bottom, as
/// `calculateFrameObjectOffsets` fills from the top. DEVIATE: the pin's AArch64
/// order is the index order, the first object at the top, and its `sub sp; str
/// [sp, #8]` is two instructions the fold makes one (exit: none, the rule is
/// the better one).
/// The frame's size by `per_register` bytes for each register, as a bound on
/// what spilling adds: locals, the registers, the saves.
pub fn frame_bound(
    function: &MFunc,
    per_register: u64,
) -> u64 {
    let (_, locals) = place_objects(function);
    locals + per_register * u64::from(function.next_register) + 512
}

/// The frame size at which an offset may need a scratch register
/// (`x16`/`x17`, the lowerer's pair): below it, no offset does.
pub const SCRATCH_FREE_BELOW: u64 = 4000;

/// Whether the frame may stay below `SCRATCH_FREE_BELOW` (so `x16`/`x17` can
/// be allocated): `(tight, loose)` for 8 and 16 B per register.
pub fn scratch_estimate(function: &MFunc) -> (bool, bool) {
    if UNSURE.load(std::sync::atomic::Ordering::Relaxed) {
        return (true, false);
    }
    let fits = |per_register| frame_bound(function, per_register) < SCRATCH_FREE_BELOW;
    (fits(8), fits(16))
}

/// Tests set this to make every function "maybe small": the pair is allocated
/// and the real frame decides whether the allocation stands.
pub static UNSURE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn place_objects(function: &MFunc) -> (Vec<u64>, u64) {
    let uses = object_uses(function);
    let size = |at: usize| function.slots[at].size.max(1);
    // Densest last, as the pin's list: ties go to the larger alignment.
    let mut order: Vec<usize> = (0..function.slots.len()).collect();
    order.sort_by(|&a, &b| {
        (uses[a] * size(b)).cmp(&(uses[b] * size(a))).then(function.slots[a].align.cmp(&function.slots[b].align))
    });
    let mut slots = vec![0; function.slots.len()];
    if function.dynamic {
        // From the frame pointer, at the top: the densest first.
        let mut down = 0;
        let mut from_top = vec![0; slots.len()];
        for &at in order.iter().rev() {
            let slot = &function.slots[at];
            down = (down + slot.size).next_multiple_of(slot.align.max(1));
            from_top[at] = down;
        }
        let locals_end = (down + function.outgoing).next_multiple_of(16);
        for (at, from) in from_top.iter().enumerate() {
            slots[at] = locals_end - from;
        }
        return (slots, locals_end);
    }
    // The object the entry block touches first, at offset 0 of it, goes at
    // `[sp]`: that access folds into the frame's allocation.
    let first = function
        .blocks
        .first()
        .and_then(
            |block| block
                .ops
                .iter()
                .find_map(
                    |op| match op {
                        Op::I(
                            Inst::Load { addr: Addr::Slot { slot, offset }, .. }
                            | Inst::Store { addr: Addr::Slot { slot, offset }, .. },
                        ) => Some((*slot as usize, *offset)),
                        _ => None,
                    },
                ),
        );
    if let Some((first, 0)) = first
        && let Some(at) = order.iter().position(|&one| one == first)
    {
        order.remove(at);
        order.push(first);
    }
    let mut at = function.outgoing;
    for &index in order.iter().rev() {
        let slot = &function.slots[index];
        at = at.next_multiple_of(slot.align.max(1));
        slots[index] = at;
        at += slot.size;
    }
    // The dense order can put an object past what a pair reaches
    // (`ldp`/`stp` of an 8-byte register: 504 bytes from the base). Ordered
    // by the pairs per byte instead, the pairs are the ones near the base; it
    // is taken where it leaves fewer of them out of reach.
    let reach = unreachable_pairs(function, &slots);
    if reach > 0 {
        let pairs = pair_uses(function);
        let mut by_pairs: Vec<usize> = (0..function.slots.len()).collect();
        by_pairs.sort_by(|&a, &b| {
            (pairs[a] * size(b)).cmp(&(pairs[b] * size(a))).then(function.slots[a].align.cmp(&function.slots[b].align))
        });
        let mut paired = vec![0; slots.len()];
        let mut end = function.outgoing;
        for &index in by_pairs.iter().rev() {
            let slot = &function.slots[index];
            end = end.next_multiple_of(slot.align.max(1));
            paired[index] = end;
            end += slot.size;
        }
        if formable_pairs(function, &paired) > formable_pairs(function, &slots) {
            (slots, at) = (paired, end);
        }
    }
    (slots, at.next_multiple_of(16))
}

/// How many adjacent same-kind access pairs (`ldp`/`stp` candidates) start in
/// each frame object.
fn pair_uses(function: &MFunc) -> Vec<u64> {
    let mut uses = vec![0; function.slots.len()];
    for block in &function.blocks {
        let mut seen: HashSet<(bool, u8, u32, i64)> = HashSet::default();
        for op in &block.ops {
            let (load, mem, slot, offset) = match op {
                Op::I(Inst::Load { mem, addr: Addr::Slot { slot, offset }, .. }) => (true, mem, slot, offset),
                Op::I(Inst::Store { mem, addr: Addr::Slot { slot, offset }, .. }) => (false, mem, slot, offset),
                _ => continue,
            };
            let (Memory::Int { bytes, .. } | Memory::Fp { bytes, .. }) = mem;
            if matches!(bytes, 4 | 8 | 16) {
                seen.insert((load, *bytes, *slot, *offset));
            }
        }
        for &(load, bytes, slot, offset) in &seen {
            if seen.contains(&(load, bytes, slot, offset + i64::from(bytes)))
                && let Some(count) = uses.get_mut(slot as usize)
            {
                *count += 1;
            }
        }
    }
    uses
}

/// The adjacent same-kind accesses in a block that one pair instruction can
/// take with the objects at `slots`: the lower address a multiple of the size
/// and within what `ldp`/`stp` reaches from the stack pointer.
fn formable_pairs(
    function: &MFunc,
    slots: &[u64],
) -> usize {
    pairs_at(function, slots, true)
}

/// The same adjacent accesses that a pair cannot take.
fn unreachable_pairs(
    function: &MFunc,
    slots: &[u64],
) -> usize {
    pairs_at(function, slots, false)
}

fn pairs_at(
    function: &MFunc,
    slots: &[u64],
    formable: bool,
) -> usize {
    let mut count = 0;
    for block in &function.blocks {
        let mut seen: HashSet<(bool, u8, i64)> = HashSet::default();
        for op in &block.ops {
            let (load, mem, slot, offset) = match op {
                Op::I(Inst::Load { mem, addr: Addr::Slot { slot, offset }, .. }) => (true, mem, slot, offset),
                Op::I(Inst::Store { mem, addr: Addr::Slot { slot, offset }, .. }) => (false, mem, slot, offset),
                _ => continue,
            };
            let (Memory::Int { bytes, .. } | Memory::Fp { bytes, .. }) = mem;
            if let Some(&base) = slots.get(*slot as usize)
                && matches!(bytes, 4 | 8 | 16)
            {
                seen.insert((load, *bytes, base as i64 + offset));
            }
        }
        count += seen
            .iter()
            .filter(|&&(load, bytes, at)| {
                let reachable = at <= 63 * i64::from(bytes) && at % i64::from(bytes) == 0;
                seen.contains(&(load, bytes, at + i64::from(bytes))) && reachable == formable
            })
            .count();
    }
    count
}

/// The pin's `DefaultSafeSPDisplacement`.
pub const SAFE_SP_DISPLACEMENT: u64 = 255;

fn layout(
    function: &MFunc,
    allocation: &Allocation,
    darwin: bool,
) -> Layout {
    let (mut slots, locals_end) = place_objects(function);
    // `AArch64FrameLowering::hasFP`: `disableFramePointerElim` (the attribute)
    // or a variable-sized object keeps the frame record. A call without one
    // saves the link register alone, with its neighbours in the list.
    let keeps =
        llrm_codegen::backend::framefree::keeps_frame_pointer(function.frame_pointer.as_deref(), function.calls);
    // `hasFP` also holds for a call frame over `DefaultSafeSPDisplacement`
    // (255 bytes): the scavenger's emergency slot is reached from the frame
    // pointer. DEVIATE FP-ALLOC-1 (owner w15, exit 2026-11-16): x29 joins
    // the allocation pool when no record is kept. stft_fixed -O1/-O2
    // (bench), +1 executed insn in a 78M run, -4 B: the greedy gives x29 to
    // one global's base and rematerializes another global (`adrp`/`add`
    // into x20) at the outer loop head, where the old code folded the
    // address into x9; read from both assemblies.
    // DEVIATE FP-DARWIN (owner w15, exit 2026-11-30): the pin's
    // `producePairRegisters` (Mach-O compact unwind) saves the frame
    // pointer beside the link register on Darwin whatever `hasFP` says,
    // without setting it up; here a Darwin call keeps the whole record.
    // Unreachable from clang, which gives Darwin `"frame-pointer"="all"`.
    let record = function.dynamic || keeps || function.outgoing > SAFE_SP_DISPLACEMENT || (darwin && function.calls);
    let pairs = paired(&saved_list(allocation, record, function.calls && !record, darwin));
    let registers: u64 = pairs.iter().map(|(_, second)| 1 + u64::from(second.is_some())).sum();
    let area = (8 * registers).next_multiple_of(16);
    // Filled from the top; where the area has a spare word, the first single
    // register that would leave a half pair below takes the gap under it.
    let mut gap = area != 8 * registers;
    let mut top = area;
    // DEVIATE FR-LOW (beyond the pin; owner w15, exit 2026-11-30): on ELF a
    // frame that has locals and saved registers and fits one `stp [sp, #-N]!`
    // puts the saved registers lowest and the locals above them, as gcc does:
    // the stack pointer moves in the first store and the last load, not in a
    // separate `sub sp` and `add sp` (queens -Os: `bench_queens` 44 B, gcc 36).
    // The pin's frame has the saved registers on top. Mach-O keeps it: its
    // compact unwind names the saves below the frame record.
    let record_low = !darwin
        && !function.dynamic
        && function.outgoing == 0
        && locals_end > 0
        && area > 0
        // Below 512: from there `split_locals` pushes the saves before the
        // locals, as the pin.
        && locals_end + area < 512;
    let shift = if record_low { 0 } else { locals_end };
    let mut saves = Vec::new();
    let mut frame_record = None;
    for (first, second) in pairs {
        top -= if second.is_some() { 16 } else { 8 };
        if second.is_none() && gap && top % 16 != 0 {
            top -= 8;
            gap = false;
        }
        let at = shift + top;
        // The register at the higher number goes at the lower address.
        let save = match (first, second) {
            (Csr::Lr, Some(Csr::Fp)) => {
                frame_record = Some(at);
                Save::Record
            }
            (Csr::X(low), Some(Csr::X(high))) => Save::GprPair(high, Some(low)),
            (Csr::X(only), None) => Save::GprPair(only, None),
            (Csr::D(low), Some(Csr::D(high))) => Save::FprPair(high, Some(low)),
            (Csr::D(only), None) => Save::FprPair(only, None),
            _ => unreachable!("a register pair of the frame"),
        };
        saves.push((save, at));
    }
    saves.reverse();
    if record_low {
        for slot in &mut slots {
            *slot += area;
        }
    }
    Layout { size: locals_end + area, dynamic: function.dynamic, slots, saves, frame_record, locals: locals_end }
}

impl Layout {
    /// The bytes the first save takes off the stack pointer (`stp [sp, #-N]!`)
    /// where the frame is only saved registers and `N` is in range of the
    /// access (`convertCalleeSaveRestoreToSPPrePostIncDec`: the pin checks the
    /// offset against the opcode's, -512..504 for a pair and -256..255 for a
    /// single register, and makes a separate `sub`/`add` when it is out). The
    /// prologue subtracts and the epilogue adds, so one frame can fold in one
    /// and not the other.
    fn folded_bump(
        &self,
        store: bool,
    ) -> Option<i64> {
        let (first, pair) = match self.saves.first() {
            Some((Save::GprPair(_, second) | Save::FprPair(_, second), at)) => (*at, second.is_some()),
            Some((Save::Record, at)) => (*at, true),
            None => (self.frame_record?, true),
        };
        let (least, most) = if pair { (-512, 504) } else { (-256, 255) };
        let by = self.size as i64;
        (first == 0 && (least..=most).contains(&if store { -by } else { by })).then_some(by)
    }

    /// The locals' bytes where the saved registers are pushed first and the
    /// locals taken after, as the pin when `CombineSPBump` is off
    /// (`shouldCombineCSRLocalStackBump`: a frame of 512 bytes or more).
    /// DEVIATE FS1: a frame with a moving stack pointer stays combined (the
    /// pin does not combine it either). Exit: 11-12.
    fn split_locals(&self) -> Option<u64> {
        let saved = !self.saves.is_empty() || self.frame_record.is_some();
        (saved && !self.dynamic && self.locals > 0 && self.size >= 512).then_some(self.locals)
    }

    /// The register and the offset from it that address byte `offset` of frame
    /// object `slot` (or of the caller's argument area): the stack pointer, or
    /// the frame pointer where the stack pointer moves.
    fn place(
        &self,
        slot: u32,
        offset: i64,
    ) -> (Gpr, i64) {
        let from_sp = if slot == INCOMING_ARGUMENTS { self.size } else { self.slots[slot as usize] };
        if !self.dynamic {
            return (Gpr::R31, from_sp as i64 + offset);
        }
        // `x29` is where the frame record is: that many bytes above the stack
        // pointer the frame was made with.
        let record = self.frame_record.expect("a moving stack has a frame record");
        (Gpr::FP, from_sp as i64 + offset - record as i64)
    }
}

struct Emitter {
    insts: Vec<Inst>,
    /// Parallel to `insts`, filled as each operation is done.
    locs: Vec<Option<(u32, u32)>>,
    /// The saves' `.cfi_offset`s, stated once the prologue has set the frame
    /// register up (`AArch64PrologueEmitter::emitCalleeSavedGPRLocations`
    /// follows `emitFramePointerSetup`).
    saved: Vec<Cfi>,
    /// The restores' `.cfi_restore`s, stated together before the terminator
    /// (`AArch64EpilogueEmitter::finalizeEpilogue`:
    /// `emitCalleeSavedGPRRestores` at `getFirstTerminator`).
    restored: Vec<Cfi>,
    /// Which `.cfi_*` the frame states: all of them (`Async`), or the frame
    /// as the prologue leaves it and nothing for the epilogues (`Sync`).
    unwind: Unwind,
}

impl Emitter {
    fn new(unwind: Unwind) -> Self {
        Self { insts: Vec::new(), locs: Vec::new(), saved: Vec::new(), restored: Vec::new(), unwind }
    }

    /// The instructions made since the last call are from `loc`.
    fn from_place(
        &mut self,
        loc: Option<(u32, u32)>,
    ) {
        self.locs.resize(self.insts.len(), loc);
    }
}

impl Emitter {
    /// A step of the frame's construction or removal, stated where it
    /// happens: only an asynchronous table has them
    /// (`AArch64FunctionInfo::needsAsyncDwarfUnwindInfo`).
    fn cfi(
        &mut self,
        cfi: Cfi,
    ) {
        if self.unwind == Unwind::Async {
            self.insts.push(Inst::Cfi(cfi));
        }
    }

    /// `sp = sp + delta` (a multiple of 16).
    fn adjust_sp(
        &mut self,
        delta: i64,
    ) {
        if delta == 0 {
            return;
        }
        let subtract = delta < 0;
        let magnitude = delta.unsigned_abs();
        self.add_to(Gpr::R31, Gpr::R31, subtract, magnitude, Gpr(16));
    }

    /// `rd = rn +- magnitude`, through `scratch` where one add cannot do it.
    fn add_to(
        &mut self,
        rd: Gpr,
        rn: Gpr,
        subtract: bool,
        magnitude: u64,
        scratch: Gpr,
    ) {
        let op = AddSub { subtract, set_flags: false };
        if magnitude < 4096 {
            self.insts.push(Inst::AddSubImm { op, width: Width::X, rd, rn, imm12: magnitude as u16, shift12: false });
        } else if magnitude & 0xFFF == 0 && magnitude >> 12 < 4096 {
            self.insts.push(Inst::AddSubImm {
                op,
                width: Width::X,
                rd,
                rn,
                imm12: (magnitude >> 12) as u16,
                shift12: true,
            });
        } else {
            self.insts.extend(crate::expand_imm::mov_imm(magnitude, 64, scratch));
            // The register form takes the stack pointer through the extended
            // form.
            self.insts.push(Inst::AddSubExtended {
                op,
                width: Width::X,
                rd,
                rn,
                rm: scratch,
                extend: Extend::Uxtx,
                amount: 0,
            });
        }
    }
}

fn accessed_bytes(mem: &Memory) -> i64 {
    match mem {
        Memory::Int { bytes, .. } => i64::from(*bytes),
        Memory::Fp { bytes, .. } => i64::from(*bytes),
    }
}

/// `function`, its registers physical, as real instructions.
/// Where each source variable that lives in a frame object is, in bytes from
/// the canonical frame address (the stack pointer once the prologue has run,
/// plus the frame's size), below it; none where the stack pointer moves in the
/// body.
pub fn variable_offsets(
    function: &MFunc,
    allocation: &Allocation,
    darwin: bool,
) -> Vec<(llrm_mir::MetadataId, i64)> {
    let frame = layout(function, allocation, darwin);
    if frame.dynamic {
        return Vec::new();
    }
    function
        .declared
        .iter()
        .map(|&(variable, slot)| (variable, frame.slots[slot as usize] as i64 - frame.size as i64))
        .collect()
}

pub fn lower(
    function: &MFunc,
    allocation: &Allocation,
    darwin: bool,
) -> Result<Vec<Lowered>, String> {
    let frame = layout(function, allocation, darwin);
    let wrapped = crate::shrinkwrap::place(function).filter(|_| frame.size > 0);
    let save = wrapped.as_ref().map_or(0, |one| one.save);
    let mut out = Vec::new();
    for (index, block) in function.blocks.iter().enumerate() {
        let mut emitter = Emitter::new(function.unwind);
        if index == save {
            prologue(&frame, &mut emitter);
        }
        // The entry block is where the source begins, whichever block the
        // frame is built in.
        if index == 0 {
            emitter.insts.push(Inst::Mark(FrameMark::PrologueEnd));
        }
        emitter.from_place(None);
        // Without a placement every return takes the frame down; with one, the
        // Restore's return does, or the blocks that end the frame at
        // their end.
        let framed = wrapped.as_ref().is_none_or(|one| !one.at_end && one.restores.contains(&index));
        for (at, op) in block.ops.iter().enumerate() {
            let place = block.ops.loc(at).map(|one| one.row());
            match op {
                Op::I(inst) => lower_inst(inst, &frame, &mut emitter)?,
                Op::Copy { dst, src } => match (dst, src) {
                    (Reg::Gpr(rd), Reg::Gpr(rm)) => emitter.insts.push(Inst::LogicShifted {
                        op: Logic::Orr,
                        invert: false,
                        width: Width::X,
                        rd: *rd,
                        rn: Gpr::R31,
                        rm: *rm,
                        shift: Shift::Lsl,
                        amount: 0,
                    }),
                    (Reg::Fpr(rd), Reg::Fpr(rn)) => {
                        emitter.insts.push(Inst::FpUnary { op: FpUnary::Mov, size: FpSize::D, rd: *rd, rn: *rn })
                    }
                    _ => return Err("a copy between register classes".into()),
                },
                Op::Imm { dst, bits, value } => emitter.insts.extend(crate::expand_imm::mov_imm(*value, *bits, *dst)),
                Op::Phi { .. } => return Err("a phi after phi elimination".into()),
                Op::DbgValue { variable, reg } => {
                    let reg = match reg {
                        Some(Reg::Gpr(gpr)) => Some(*gpr),
                        _ => None,
                    };
                    emitter.insts.push(Inst::Mark(FrameMark::Value { variable: *variable, reg }));
                }
                Op::Call { callee, .. } => emitter.insts.push(match callee {
                    Callee::Symbol(name) => {
                        Inst::Branch { link: true, target: Target::Symbol { name: name.clone(), addend: 0 } }
                    }
                    Callee::Register(register) => Inst::BranchReg { link: true, rn: *register },
                }),
                Op::Return { tail, .. } => {
                    if framed {
                        epilogue(&frame, &mut emitter);
                    }
                    emitter.insts.push(match tail {
                        None => Inst::Ret { rn: Gpr::LR },
                        Some(Callee::Symbol(name)) => {
                            Inst::Branch { link: false, target: Target::Symbol { name: name.clone(), addend: 0 } }
                        }
                        Some(Callee::Register(register)) => Inst::BranchReg { link: false, rn: *register },
                    });
                }
                Op::JumpTable { dst, scratch, table, index, id, .. } => {
                    emitter.insts.extend(crate::jumptable::dest32(&function.name, *id, *dst, *scratch, *table, *index));
                }
                Op::FrameAddress { dst, slot, offset } => {
                    let (base, total) = frame.place(*slot, *offset);
                    emitter.add_to(*dst, base, total < 0, total.unsigned_abs(), Gpr(16));
                }
            }
            emitter.from_place(place);
        }
        if wrapped.as_ref().is_some_and(|one| one.at_end && one.restores.contains(&index)) {
            // Before the branches that leave.
            let at = emitter
                .insts
                .iter()
                .rposition(|inst| {
                    !matches!(
                        inst,
                        Inst::Branch { link: false, .. }
                            | Inst::BranchCond { .. }
                            | Inst::CompareBranch { .. }
                            | Inst::TestBranch { .. }
                    )
                })
                .map_or(0, |i| i + 1);
            let mut tail = Emitter::new(function.unwind);
            epilogue(&frame, &mut tail);
            emitter.locs.splice(at..at, vec![None; tail.insts.len()]);
            emitter.insts.splice(at..at, tail.insts);
        }
        emitter.from_place(None);
        out.push(Lowered { label: block.label.clone(), insts: emitter.insts, locs: emitter.locs });
    }
    Ok(out)
}

fn prologue(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    build_frame(frame, emitter);
    state_frame(frame, emitter);
}

/// A synchronous table states the finished frame once, after the prologue's
/// last instruction (`AArch64PrologueEmitter::emitPrologue`, `EmitCFI &&
/// !EmitAsyncCFI`): the frame address, then the saves.
fn state_frame(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    if emitter.unwind != Unwind::Sync {
        return;
    }
    let cfa = match frame.frame_record {
        Some(at) => Some(Cfi::DefCfa { register: unwind::FP, offset: (frame.size - at) as i64 }),
        None if frame.size > 0 => Some(Cfi::DefCfaOffset { offset: frame.size as i64 }),
        None => None,
    };
    let saves = sorted_saves(emitter);
    emitter.insts.extend(cfa.into_iter().chain(saves).map(Inst::Cfi));
}

fn build_frame(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    if let Some(locals) = frame.split_locals() {
        // The saved registers first, the locals after them.
        let area = (frame.size - locals) as i64;
        save_registers(frame, emitter, true, Some(area), locals);
        if let Some(at) = frame.frame_record {
            emitter.add_to(Gpr::FP, Gpr::R31, false, at - locals, Gpr(16));
            emitter.cfi(Cfi::DefCfa { register: unwind::FP, offset: (frame.size - at) as i64 });
        }
        flush_saved(emitter);
        emitter.adjust_sp(-(locals as i64));
        if frame.frame_record.is_none() {
            emitter.cfi(Cfi::DefCfaOffset { offset: frame.size as i64 });
        }
        return;
    }
    let bump = frame.folded_bump(true);
    if bump.is_none() && frame.size > 0 {
        emitter.adjust_sp(-(frame.size as i64));
        emitter.cfi(Cfi::DefCfaOffset { offset: frame.size as i64 });
    }
    save_registers(frame, emitter, true, bump, 0);
    if let Some(at) = frame.frame_record {
        emitter.add_to(Gpr::FP, Gpr::R31, false, at, Gpr(16));
        emitter.cfi(Cfi::DefCfa { register: unwind::FP, offset: (frame.size - at) as i64 });
    }
    flush_saved(emitter);
}

/// The saves' `.cfi_offset`s, highest address first: the order of the pin's
/// frame indices.
fn sorted_saves(emitter: &mut Emitter) -> Vec<Cfi> {
    let mut saved = std::mem::take(&mut emitter.saved);
    saved.sort_by_key(|cfi| match cfi {
        Cfi::Offset { offset, .. } => std::cmp::Reverse(*offset),
        _ => std::cmp::Reverse(0),
    });
    saved
}

/// The saves, stated as the frame register is set up; a synchronous table
/// states them with the finished frame ([`state_frame`]).
fn flush_saved(emitter: &mut Emitter) {
    if emitter.unwind == Unwind::Sync {
        return;
    }
    for cfi in sorted_saves(emitter) {
        emitter.cfi(cfi);
    }
}

fn epilogue(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    let first = emitter.insts.len();
    take_down(frame, emitter);
    // The restores in the pin's callee-saved order: x19..x28, then x30 and x29
    // (the record, link register first), then d8..d15.
    let mut restores = std::mem::take(&mut emitter.restored);
    restores.sort_by_key(|cfi| match cfi {
        Cfi::Restore { register: 30 } => (1, 0),
        Cfi::Restore { register: 29 } => (1, 1),
        Cfi::Restore { register } if *register >= 64 => (2, *register),
        Cfi::Restore { register } => (0, *register),
        _ => (3, 0),
    });
    for cfi in restores {
        emitter.cfi(cfi);
    }
    // Only an epilogue that does something is marked.
    if emitter.insts.len() > first {
        emitter.insts.insert(first, Inst::Mark(FrameMark::EpilogueBegin));
    }
}

fn take_down(
    frame: &Layout,
    emitter: &mut Emitter,
) {
    // A moving stack pointer is put back from the frame pointer first.
    if let (true, Some(record)) = (frame.dynamic, frame.frame_record) {
        emitter.add_to(Gpr::R31, Gpr::FP, true, record, Gpr(16));
    }
    if let Some(locals) = frame.split_locals() {
        // The locals first, then the registers pushed ahead of them.
        emitter.adjust_sp(locals as i64);
        let area = (frame.size - locals) as i64;
        if frame.frame_record.is_some() {
            emitter.cfi(Cfi::DefCfa { register: unwind::SP, offset: area });
        } else {
            emitter.cfi(Cfi::DefCfaOffset { offset: area });
        }
        save_registers(frame, emitter, false, Some(area), locals);
        return;
    }
    // The frame address is the stack pointer's again, before the registers
    // come back from under it.
    if frame.frame_record.is_some() {
        emitter.cfi(Cfi::DefCfa { register: unwind::SP, offset: frame.size as i64 });
    }
    let bump = frame.folded_bump(false);
    save_registers(frame, emitter, false, bump, 0);
    if bump.is_none() && frame.size > 0 {
        emitter.adjust_sp(frame.size as i64);
        emitter.cfi(Cfi::DefCfaOffset { offset: 0 });
    }
}

/// The stores (`store`) or loads of the saved pairs and the frame record.
fn save_registers(
    frame: &Layout,
    emitter: &mut Emitter,
    store: bool,
    bump: Option<i64>,
    below: u64,
) {
    let int = |reg: Gpr| Memory::Int { width: Width::X, bytes: 8, sign: false, reg };
    let mut accesses: Vec<(Memory, Option<Memory>, u64, i64)> = Vec::new();
    for (save, offset) in &frame.saves {
        accesses.push(match *save {
            Save::GprPair(a, b) => (int(Gpr(a)), b.map(|b| int(Gpr(b))), *offset, 8),
            Save::FprPair(a, b) => {
                (Memory::Fp { bytes: 8, reg: Vreg(a) }, b.map(|b| Memory::Fp { bytes: 8, reg: Vreg(b) }), *offset, 8)
            }
            Save::Record => (int(Gpr::FP), Some(int(Gpr::LR)), *offset, 8),
        });
    }
    // Lowest address first to store; the loads go from the highest down, so
    // that the one that also moves the stack pointer comes last (`ldp [sp],
    // #N`), as the pin's.
    if !store {
        accesses.reverse();
    }
    for (first, second, absolute, _) in accesses {
        // From the stack pointer as it stands: `below` bytes of locals are not
        // taken yet where the registers go first.
        let offset = absolute - below;
        let moves = bump.filter(|_| offset == 0);
        let addr = match moves {
            Some(by) if store => Addr::Pre { base: Gpr::R31, imm: -by },
            Some(by) => Addr::Post { base: Gpr::R31, imm: by },
            None => Addr::Offset { base: Gpr::R31, imm: offset as i64 },
        };
        if moves.is_some() {
            let inst = match (second, store) {
                (Some(second), true) => Inst::StorePair { first, second, addr },
                (Some(second), false) => Inst::LoadPair { first, second, addr },
                (None, true) => Inst::Store { mem: first, addr, mmo: MemOperand::default() },
                (None, false) => Inst::Load { mem: first, addr, mmo: MemOperand::default() },
            };
            emitter.insts.push(inst);
            declare(frame, emitter, store, &first, second.as_ref(), absolute, bump.filter(|_| moves.is_some()));
            continue;
        }
        // A pair reaches 504 bytes from the base; further, the address is made.
        let (base, imm) = if offset <= 504 {
            (Gpr::R31, offset as i64)
        } else {
            emitter.add_to(Gpr(16), Gpr::R31, false, offset, Gpr(17));
            (Gpr(16), 0)
        };
        let addr = if offset <= 504 { addr } else { Addr::Offset { base, imm } };
        let _ = imm;
        let inst = match (second, store) {
            (Some(second), true) => Inst::StorePair { first, second, addr },
            (Some(second), false) => Inst::LoadPair { first, second, addr },
            (None, true) => Inst::Store { mem: first, addr, mmo: MemOperand::default() },
            (None, false) => Inst::Load { mem: first, addr, mmo: MemOperand::default() },
        };
        emitter.insts.push(inst);
        declare(frame, emitter, store, &first, second.as_ref(), absolute, None);
    }
}

/// The call frame information of a save (`store`) or a restore of `first` and
/// `second` at `offset` from the bottom of the frame; `moves` where the access
/// also moved the stack pointer.
fn declare(
    frame: &Layout,
    emitter: &mut Emitter,
    store: bool,
    first: &Memory,
    second: Option<&Memory>,
    offset: u64,
    moves: Option<i64>,
) {
    let number = |memory: &Memory| match memory {
        Memory::Int { reg, .. } => reg.0 as u16,
        Memory::Fp { reg, .. } => 64 + reg.0 as u16,
    };
    let size = frame.size as i64;
    if let (true, Some(by)) = (store, moves) {
        // The bytes the stack pointer has moved by this far.
        emitter.cfi(Cfi::DefCfaOffset { offset: by });
    }
    for (index, memory) in std::iter::once(first).chain(second).enumerate() {
        let register = number(memory);
        if store {
            emitter.saved.push(Cfi::Offset { register, offset: offset as i64 + 8 * index as i64 - size });
        } else {
            emitter.restored.push(Cfi::Restore { register });
        }
    }
    if !store && moves.is_some() {
        emitter.cfi(Cfi::DefCfaOffset { offset: 0 });
    }
}

/// `inst` with a frame object address turned into an offset from the stack
/// pointer, through a scratch register where the offset does not encode.
fn lower_inst(
    inst: &Inst,
    frame: &Layout,
    emitter: &mut Emitter,
) -> Result<(), String> {
    let mut inst = inst.clone();
    match &mut inst {
        Inst::Load { mem, addr, .. } | Inst::Store { mem, addr, .. } => {
            if let Addr::Slot { slot, offset } = *addr {
                let bytes = accessed_bytes(mem);
                let (base, total) = frame.place(slot, offset);
                let scaled = total >= 0 && total % bytes == 0 && total / bytes < 4096;
                if scaled || (-256..256).contains(&total) {
                    *addr = Addr::Offset { base, imm: total };
                } else {
                    emitter.add_to(Gpr(16), base, total < 0, total.unsigned_abs(), Gpr(17));
                    *addr = Addr::Offset { base: Gpr(16), imm: 0 };
                }
            }
        }
        Inst::LoadPair { addr, .. } | Inst::StorePair { addr, .. } => {
            if let Addr::Slot { .. } = addr {
                return Err("a frame object in a register pair".into());
            }
        }
        _ => {}
    }
    emitter.insts.push(inst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        size: u64,
        save: Save,
    ) -> Layout {
        Layout { size, dynamic: false, slots: Vec::new(), saves: vec![(save, 0)], frame_record: None, locals: 0 }
    }

    /// A pair's pre-index reaches -512 and its post-index 504: a 512 byte
    /// frame folds the prologue's update and not the epilogue's, as the pin's
    /// range check per opcode. One rule for both made the epilogue's `ldp
    /// [sp], #512` fail to encode.
    #[test]
    fn a_pair_folds_512_bytes_going_down_and_not_coming_up() {
        let pair = frame(512, Save::GprPair(19, Some(20)));
        assert_eq!((pair.folded_bump(true), pair.folded_bump(false)), (Some(512), None));
        let near = frame(496, Save::GprPair(19, Some(20)));
        assert_eq!((near.folded_bump(true), near.folded_bump(false)), (Some(496), Some(496)));
    }

    /// A single register reaches -256 and 255.
    #[test]
    fn a_single_register_folds_at_most_255_bytes_coming_up() {
        let single = frame(256, Save::GprPair(30, None));
        assert_eq!((single.folded_bump(true), single.folded_bump(false)), (Some(256), None));
        assert_eq!(frame(272, Save::GprPair(30, None)).folded_bump(true), None);
        // A lone `d8` is a single access too (`ldr d8, [sp], #256`).
        let float = frame(256, Save::FprPair(8, None));
        assert_eq!((float.folded_bump(true), float.folded_bump(false)), (Some(256), None));
    }
}
