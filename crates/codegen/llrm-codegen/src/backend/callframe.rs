//! Stores into a reserved call frame become pushes: `X86CallFrameOptimization`.
//! Translated from
//! [X86CallFrameOptimization.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp),
//! function by function in the pin's order: `isLegal` ([L147]), `isProfitable`
//! ([L205]), `runOnMachineFunction` ([L248]), `classifyInstruction` ([L282]),
//! `collectCallInfo` ([L338]), `adjustCallSequence` ([L505]) and
//! `canFoldIntoRegPush` ([L605]).
//!
//! # How it works
//! A call sequence is `call_frame_setup N, 0, 0` (`ADJCALLSTACKDOWN`: the size,
//! a second amount, the adjustment the pass sets), stores of the arguments into
//! the frame above the stack pointer (`mov [sp+k], r|imm`, `and [sp+k], 0`,
//! `or [sp+k], -1`), the `call`, and `call_frame_destroy N, M`. Where the
//! stores fill the slots from 0 up with no gap, are all the instructions
//! between the setup and the call but others that touch neither memory nor the
//! stack pointer, and the sequence is profitable by the pin's size heuristic,
//! each store becomes a `push` placed before the call, in reverse order, the
//! setup's adjustment is the bytes pushed, and a once-read load of the argument
//! is folded into `push [m]` unless `slowTwoMemOps`.
//!
//! The producer is the call lowering of the SelectionDAG port: `LowerCall`
//! stores the arguments to `[sp+k]` between `ADJCALLSTACKDOWN` and
//! `ADJCALLSTACKUP`. `emit.rs` turns the pair into `call_frame_setup` /
//! `call_frame_destroy` at every call at m32 and m64 (DEVIATE CF4), also the
//! zero-byte ones, as the pin's; the pass is in the flow at every x86 width. At
//! m16 (`Real16`) `emit.rs` makes `sub sp, N` / `add sp, N - M` itself (none
//! for a call of no bytes), so the pass finds no setup and changes nothing
//! there. A zero-byte pair at m16 moved no cell of the cause-A list (0 B on 14
//! cells, jpeg_fixed -Os +4 B). The m16 steps left: emit the pseudos for
//! `Real16` (`slot` 2, 16-bit `Held` cells), let `ReserveCallFrame` lay the
//! reserved area out in the m16 prologue, and re-check the readers of the old
//! `sub`/`add` (`cfi.rs`, `framefree`, `call_pops`) and the m16 byte and clock
//! rows. Pushes made at m32 selection time (DEVIATE CG6 of row 50,
//! `pushfold.rs`) are not this pass's input.
//!
//! N/A: the Darwin and Win64 conditions of `isLegal`, the stack probe, the
//! 64-bit forms (`PUSH64`, `INSERT_SUBREG`) and `BuildCFI` (LIR has no CFI
//! instruction). DEVIATE CF2 (no number yet): `hasVarSizedObjects` is not in
//! LIR; the pass is given it (false until row 80). Owner llrm-target-x86-m16,
//! review 2026-11-20.
//!
//! DEVIATE CF3 (owner x86-m32, exit: never; upstream-report candidate): as
//! `pushfold.rs`'s CF2, a volatile load is not folded into `push [m]`, nor is
//! one moved past another volatile access. The pin's `canFoldIntoRegPush` asks
//! only `isLoadFoldBarrier`, so it loads `p[0]`, `p[1]` (the IR's last reads)
//! before `pushl 28(%esp)` six times (the first reads). Cost:
//! os_locals_between_pushes -O2 m32 +29 B (226 vs 197).
//!
//! [L147]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L147
//! [L205]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L205
//! [L248]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L248
//! [L282]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L282
//! [L338]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L338
//! [L505]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L505
//! [L605]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L605
//!
//! # Example
//! ```lir
//! call_frame_setup 8, 0, 0
//! mov [sp+4], v2
//! mov [sp+0], 7
//! call f
//! call_frame_destroy 8, 0
//! ```
//! becomes
//! ```lir
//! call_frame_setup 8, 0, 8
//! push v2
//! push 7
//! call f
//! call_frame_destroy 8, 0
//! ```
//!
//! # Runs
//! After `FixupSetCC`, before `FlagsCopy` (the pin's pre-allocation order).
//! Optimizing levels only, as the pin (`addPreRegAlloc`).

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::liveness;
use crate::backend::storedhomes::may_write;
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct CallFrameOpt {
    /// `SlotSize`: the bytes a push moves the stack pointer by.
    pub slot: i64,
    /// `TFL->getStackAlign()`.
    pub align: i64,
    /// `RegInfo.getStackRegister()`.
    pub stack: RegId,
    /// `MFI.hasVarSizedObjects()`.
    pub has_var_sized: bool,
    /// `STI->slowTwoMemOps()`.
    pub slow_two_mem_ops: bool,
    /// The bytes every call finds the stack a multiple of
    /// (`Profile::call_alignment`); 1 where none.
    pub call_alignment: i64,
    /// The widest alignment of a frame object: the post-prologue stack keeps
    /// it.
    pub phase_step: i64,
}

impl LIRTransform for CallFrameOpt {
    fn class_name(&self) -> &'static str {
        "CallFrameOpt"
    }

    fn name(&self) -> &str {
        "callframe"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(self.run(&body))
    }
}

/// Where the frame pseudos go, after allocation and before the prologue is
/// made (the pin's `eliminateCallFramePseudoInstr`, from PEI). A function with
/// no variable-sized object and no pushed call reserves its largest call area
/// in the prologue (`hasReservedCallFrame`): the pseudos go, and what a callee
/// popped is reserved again with `sub sp, M`. Any other function adjusts for
/// every call (`LowerCallFrame`). Runs at every level.
pub struct ReserveCallFrame {
    /// The bytes every call finds the stack a multiple of; 1 where none.
    pub call_alignment: i64,
    /// The widest alignment of a frame object: the phase is a multiple of it.
    pub phase_step: i64,
    /// The callee-saved registers the prologue may push (whole registers).
    pub saved: Vec<RegId>,
    /// `RegInfo.getStackRegister()`.
    pub stack: RegId,
    /// The width of the stack pointer.
    pub width: u32,
    /// The frame the prologue reserves; a function with none lowers every call.
    pub frame: Option<std::rc::Rc<std::cell::RefCell<crate::backend::frame::Frame>>>,
}

impl LIRTransform for ReserveCallFrame {
    fn class_name(&self) -> &'static str {
        "ReserveCallFrame"
    }

    fn name(&self) -> &str {
        "reservecallframe"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let pseudos = |one: &Arc<Insn>| matches!(name_of(one), Some("call_frame_setup" | "call_frame_destroy"));
        if !body.blocks.iter().flat_map(|block| block.insns.iter()).any(pseudos) {
            return Ok(body);
        }
        let pushed = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Push));
        let probe = CallFrameOpt {
            slot: 0,
            align: 0,
            stack: self.stack,
            has_var_sized: false,
            slow_two_mem_ops: false,
            call_alignment: 1,
            phase_step: 1,
        };
        let dynamic = probe.allocates_dynamically(&body);
        let phase = if dynamic { 0 } else { self.call_phase(&body) };
        let mut lowering = LowerCallFrame {
            stack: self.stack,
            width: self.width,
            call_alignment: self.call_alignment,
            phase,
            defer: self.call_alignment > i64::from(self.width),
        };
        let Some(frame) = self.frame.as_ref().filter(|_| !pushed && !dynamic) else {
            let mut out = lowering.transform(body)?;
            out.call_phase = phase;
            return Ok(out);
        };
        let area = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| name_of(one) == Some("call_frame_setup"))
            .filter_map(|one| imm_of(one, 0))
            .max()
            .unwrap_or(0);
        // The area is not padded: the frame's own size is aligned to the call
        // alignment past the return address (a 40-byte Win64 area is
        // `sub rsp, 40`, not 48 + 8). The destroys that re-reserve what
        // a callee popped become `sub`.
        let mut out = body.clone();
        for block in &mut out.blocks {
            let insns: Vec<Arc<Insn>> = block
                .insns
                .iter()
                .filter_map(|one| match name_of(one) {
                    Some("call_frame_setup") => None,
                    Some("call_frame_destroy") => {
                        let popped = imm_of(one, 1).filter(|&popped| popped != 0)?;
                        let sp = Loc::Reg(Reg { register: self.stack, width: self.width });
                        let what = Semantics {
                            name: Some("sub".into()),
                            dests: vec![sp.clone()],
                            sources: vec![sp, Loc::Imm(Imm { value: popped, width: self.width, address: None })],
                            ..Semantics::new(Operation::Binary)
                        };
                        Some(Arc::new(Insn::new(one.at, one.covers, Some(what), vec![], vec![])))
                    }
                    _ => Some(Arc::clone(one)),
                })
                .collect();
            *block = block.with_insns(insns);
        }
        frame.borrow_mut().call_area = area;
        Ok(out)
    }
}

impl ReserveCallFrame {
    /// Where the frame leaves the stack for the calls that adjust it, in bytes
    /// past a multiple of `call_alignment` (`LirBody::call_phase`): the one
    /// whose pads cost the fewest bytes, the frame's own pad (the prologue'
    /// s `sub`, or a `push`) and each call's. A call of `n` bytes is aligned
    /// where the frame leaves `-n` past a multiple, so a function whose
    /// calls all push alike pads neither (GCC's `expand_call` pads the
    /// argument block against the stack's depth and `ix86_compute_frame_layout`
    /// the frame to it; the pin pads every block to the alignment, and the
    /// frame besides). A frame object keeps its alignment, so the phase is
    /// a multiple of the widest. 0 where the stack moves under the frame or
    /// no call needs a pad.
    fn call_phase(
        &self,
        body: &LirBody,
    ) -> i64 {
        let align = self.call_alignment;
        let slot = i64::from(self.width);
        if align <= slot || slot != 4 {
            return 0;
        }
        let sizes: Vec<i64> = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| name_of(one) == Some("call_frame_setup"))
            .filter_map(|one| imm_of(one, 0))
            .collect();
        if sizes.is_empty() {
            return 0;
        }
        // The stack at the first local: the return address, what the prologue
        // pushes (estimated from the registers the body writes: the
        // prologue's own loop keeps the phase whatever this gets wrong).
        let regs = body.regs();
        let written: BTreeSet<RegId> = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter_map(|one| one.what.as_ref())
            .flat_map(|what| what.dests.iter())
            .filter_map(|dest| match dest {
                Loc::Reg(reg) => Some(regs.root(reg.register)),
                _ => None,
            })
            .collect();
        let pushed = self.saved.iter().filter(|whole| written.contains(whole)).count() as i64;
        // The frame's own size is the spill slots below the floor; the objects
        // the body addresses itself (static allocas) lie above it, and the
        // phase counts them too: a four-byte local read as no local put the
        // phase four bytes off and padded the frame and every call.
        let locals = self
            .frame
            .as_ref()
            .map_or(
                0,
                |frame| {
                    let frame = frame.borrow();
                    frame.size() - frame.floor
                },
            );
        let base = (slot + slot * pushed + (locals + slot - 1) / slot * slot) % align;
        best_phase(&sizes, Some(base), align, self.phase_step.max(slot), locals > 0)
    }
}

/// The phase (see `ReserveCallFrame::call_phase`) that pads the fewest bytes of
/// code: the frame's pad is a `push` (a word) or a `sub` and an `add`; a call's
/// is a `sub` (its `add` is the one it has), or both for a call of no bytes.
fn best_phase(
    sizes: &[i64],
    base: Option<i64>,
    align: i64,
    step: i64,
    has_locals: bool,
) -> i64 {
    let cost = |phase: i64| -> i64 {
        // A frame that has locals has its `sub` already: the pad is a larger
        // immediate, no code.
        let frame = match base.map_or(0, |base| (phase - base).rem_euclid(align)) {
            0 => 0,
            _ if has_locals => 0,
            4 => 2,
            _ => 6,
        };
        let calls: i64 = sizes
            .iter()
            .map(|&bytes| match (padded(bytes, phase, align) != bytes, bytes) {
                (false, _) => 0,
                (true, 0) => 6,
                (true, _) => 3,
            })
            .sum();
        frame + calls
    };
    (0..align).step_by(step.max(1) as usize).min_by_key(|&phase| (cost(phase), phase)).unwrap_or(0)
}

/// What the pin's `eliminateCallFramePseudoInstr` makes of the frame pseudos
/// where no call frame is reserved: `call_frame_setup N, 0, A` is `sub sp, N -
/// A` (the pushes made A of it) and `call_frame_destroy N, M` is `add sp, N -
/// M` (the callee popped M). [`ReserveCallFrame`] runs it for the functions
/// that cannot reserve one.
struct LowerCallFrame {
    /// `RegInfo.getStackRegister()`.
    pub stack: RegId,
    /// The width of the stack pointer.
    pub width: u32,
    /// The bytes every call finds the stack a multiple of; 1 where none.
    pub call_alignment: i64,
    /// `LirBody::call_phase`.
    pub phase: i64,
    /// Whether the cleanup of one call is deferred into the padding of the
    /// next ([`defer_cleanup`]).
    pub defer: bool,
}

impl LIRTransform for LowerCallFrame {
    fn class_name(&self) -> &'static str {
        "LowerCallFrame"
    }

    fn name(&self) -> &str {
        "lowercallframe"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let mut out = body.clone();
        for (at, block) in body.blocks.iter().enumerate() {
            if !block.insns.iter().any(|one| matches!(name_of(one), Some("call_frame_setup" | "call_frame_destroy"))) {
                continue;
            }
            let mut insns: Vec<Arc<Insn>> = Vec::with_capacity(block.insns.len());
            // The indirect call of the frame open now, to be told what its
            // callee pops.
            let mut open_call: Option<i64> = None;
            for one in block.insns.iter() {
                if one.what.as_ref().is_some_and(|what| what.op == Operation::Call && what.indirect) {
                    open_call = Some(one.at);
                }
                if name_of(one) == Some("call_frame_destroy")
                    && let Some(call) = open_call.take()
                {
                    Arc::make_mut(&mut out.call_pops).insert(call, imm_of(one, 1).unwrap_or(0));
                }
                let (verb, count) = match name_of(one) {
                    Some("call_frame_setup") => (
                        "sub",
                        padded(imm_of(one, 0).unwrap_or(0), self.phase, self.call_alignment)
                            - imm_of(one, 2).unwrap_or(0),
                    ),
                    Some("call_frame_destroy") => (
                        "add",
                        padded(imm_of(one, 0).unwrap_or(0), self.phase, self.call_alignment)
                            - imm_of(one, 1).unwrap_or(0),
                    ),
                    _ => {
                        insns.push(Arc::clone(one));
                        continue;
                    }
                };
                if count != 0 {
                    let sp = Loc::Reg(Reg { register: self.stack, width: self.width });
                    let amount = Loc::Imm(Imm { value: count, width: self.width, address: None });
                    let what = Semantics {
                        name: Some(verb.into()),
                        dests: vec![sp.clone()],
                        sources: vec![sp, amount],
                        ..Semantics::new(Operation::Binary)
                    };
                    insns.push(Arc::new(Insn::new(one.at, one.covers, Some(what), vec![], vec![])));
                }
            }
            if self.defer && body.over_aligned.is_empty() {
                insns = defer_cleanup(insns, self.stack);
            }
            out.blocks[at] = block.with_insns(insns);
        }
        Ok(out)
    }
}

/// The amount of a `sub sp, n` (negative) or `add sp, n` of the stack pointer
/// `stack` that lowering made, else none.
fn sp_step(
    one: &Insn,
    stack: RegId,
) -> Option<i64> {
    let what = one.what.as_ref()?;
    let [Loc::Reg(dest)] = what.dests.as_slice() else { return None };
    let (name, [Loc::Reg(source), Loc::Imm(amount)]) = (what.name.as_deref()?, what.sources.as_slice()) else {
        return None;
    };
    if dest.register != stack || source.register != stack {
        return None;
    }
    match name {
        "add" => Some(amount.value),
        "sub" => Some(-amount.value),
        _ => None,
    }
}

/// Whether an instruction between two stack steps pins them apart: a call (it
/// finds the stack aligned), a push or a pop, or anything that names the stack
/// pointer but as the base of a memory operand, whose displacement the frame
/// readers keep right whatever the steps before it.
fn pins_stack(
    one: &Insn,
    stack: RegId,
) -> bool {
    let Some(what) = one.what.as_ref() else { return true };
    // Only what computes, loads and stores moves past a stack step: anything
    // else (a call, a push or pop, a jump, inline assembly, a frame pseudo)
    // may look at the stack pointer or leave it in a place of its own.
    let plain = matches!(
        what.op,
        Operation::Move
            | Operation::Exchange
            | Operation::Address
            | Operation::Binary
            | Operation::Multiply
            | Operation::Divide
            | Operation::Compare
            | Operation::Unary
            | Operation::Funnel
            | Operation::Extend
            | Operation::Select
            | Operation::BitField
            | Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    );
    if !plain {
        return true;
    }
    let names = |loc: &Loc| matches!(loc, Loc::Reg(reg) if reg.register == stack);
    what.dests.iter().chain(&what.sources).any(names) || !one.requires.is_empty() || !one.delivers.is_empty()
}

/// GCC's deferred stack adjustment (`expr.c`: `pending_stack_adjust`,
/// `do_pending_stack_adjust`; `compute_argument_block_size` pads a call's
/// arguments against the bytes still pending). The bytes a call's cleanup
/// frees are not freed until something needs the stack pointer where it was:
/// the next call's pad is taken out of them. Here: an `add sp, A` and a later
/// `sub sp, B` of one block with nothing between that pins the stack
/// ([`pins_stack`]) become the one step `A - B`, where the later of the two
/// was. Every call keeps the alignment it had: the stack at the `sub` is the
/// same either way, and no call lies between the two.
fn defer_cleanup(
    mut insns: Vec<Arc<Insn>>,
    stack: RegId,
) -> Vec<Arc<Insn>> {
    let mut at = 0;
    while at < insns.len() {
        let Some(freed) = sp_step(&insns[at], stack).filter(|&step| step > 0) else {
            at += 1;
            continue;
        };
        let mut next = at + 1;
        while next < insns.len() && sp_step(&insns[next], stack).is_none() && !pins_stack(&insns[next], stack) {
            next += 1;
        }
        let Some(taken) = insns.get(next).and_then(|one| sp_step(one, stack)).filter(|&step| step < 0) else {
            at += 1;
            continue;
        };
        let net = freed + taken;
        let (keep, drop) = if net >= 0 { (at, next) } else { (next, at) };
        if net == 0 {
            insns.remove(next);
            insns.remove(at);
            continue;
        }
        let one = &insns[keep];
        let what = one.what.as_ref().expect("a stack step");
        let (name, count) = if net > 0 { ("add", net) } else { ("sub", -net) };
        let amount = Loc::Imm(Imm { value: count, width: stack_width(what), address: None });
        let what = Semantics {
            name: Some(name.into()),
            dests: what.dests.clone(),
            sources: vec![what.sources[0].clone(), amount],
            ..Semantics::new(Operation::Binary)
        };
        insns[keep] = Arc::new(Insn::new(one.at, one.covers, Some(what), vec![], vec![]));
        insns.remove(drop);
        // The merged step can take the next one too.
        at = keep.min(drop);
    }
    insns
}

/// The width of the stack pointer a step of `what` was written for.
fn stack_width(what: &Semantics) -> u32 {
    match what.dests.first() {
        Some(Loc::Reg(reg)) => reg.width,
        _ => 4,
    }
}

/// The call area of `bytes` with the pad that leaves the stack a multiple of
/// `align` at the call, the frame having left it `phase` past one
/// (`X86FrameLowering::eliminateCallFramePseudoInstr`'s `alignSPAdjust`, which
/// pads to a multiple of the stack alignment: here from the frame's phase).
fn padded(
    bytes: i64,
    phase: i64,
    align: i64,
) -> i64 {
    if align <= 1 { bytes } else { bytes + (align - (phase + bytes) % align) % align }
}

fn name_of(one: &Insn) -> Option<&str> {
    one.what.as_ref()?.name.as_deref()
}

fn imm_of(
    one: &Insn,
    at: usize,
) -> Option<i64> {
    match one.what.as_ref()?.sources.get(at)? {
        Loc::Imm(imm) => Some(imm.value),
        _ => None,
    }
}

/// What `classifyInstruction` says of an instruction between setup and call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    Convert,
    Skip,
    Exit,
}

/// One call sequence (`CallContext`), its instructions held by identity.
struct CallContext {
    block: usize,
    setup: Arc<Insn>,
    call: Option<Arc<Insn>>,
    sp_copy: Option<Arc<Insn>>,
    expected_dist: i64,
    stores: Vec<Option<Arc<Insn>>>,
    no_stack_params: bool,
    use_push: bool,
}

impl CallFrameOpt {
    fn run(
        &self,
        body: &LirBody,
    ) -> LirBody {
        // `runOnMachineFunction`.
        if !self.is_legal(body) {
            return body.clone();
        }
        let mut contexts = Vec::new();
        for (block, one) in body.blocks.iter().enumerate() {
            for (at, insn) in one.insns.iter().enumerate() {
                if name_of(insn) == Some("call_frame_setup") {
                    contexts.push(self.collect_call_info(body, block, at));
                }
            }
        }
        let dynamic = self.has_var_sized || self.allocates_dynamically(body);
        let phase = self.call_phase(&contexts, dynamic);
        if !self.is_profitable(&contexts, dynamic, phase) {
            return body.clone();
        }
        let mut out = body.clone();
        let mut changed = false;
        for context in contexts.iter().filter(|c| c.use_push) {
            self.adjust_call_sequence(&mut out, context);
            changed = true;
        }
        if changed { out } else { body.clone() }
    }

    /// The phase `isProfitable` prices the pads at: the calls' alone
    /// (`ReserveCallFrame::call_phase` adds the frame's own and chooses it
    /// for good).
    fn call_phase(
        &self,
        contexts: &[CallContext],
        dynamic: bool,
    ) -> i64 {
        let align = self.call_alignment;
        if align <= self.slot || dynamic || self.slot != 4 {
            return 0;
        }
        let sizes: Vec<i64> = contexts.iter().map(|one| imm_of(&one.setup, 0).unwrap_or(0)).collect();
        best_phase(&sizes, None, align, self.phase_step.max(self.slot), false)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L147
    /// `isLegal`: every frame is closed in its block, none nested.
    fn is_legal(
        &self,
        body: &LirBody,
    ) -> bool {
        for block in &body.blocks {
            let mut inside = false;
            for one in block.insns.iter() {
                match name_of(one) {
                    Some("call_frame_setup") => {
                        if inside {
                            return false;
                        }
                        inside = true;
                    }
                    Some("call_frame_destroy") => {
                        if !inside {
                            return false;
                        }
                        inside = false;
                    }
                    _ => {}
                }
            }
            if inside {
                return false;
            }
        }
        true
    }

    /// `MFI.hasVarSizedObjects()`: a `sub sp, reg`, as `framefree` reads it, or
    /// a `mov sp, reg|mem`: an alloca kept to a 16-byte call alignment computes
    /// the new `sp` in a register (`sub`, `and`) and copies it back, and a
    /// `stackrestore` loads it.
    fn allocates_dynamically(
        &self,
        body: &LirBody,
    ) -> bool {
        body.blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter_map(|one| one.what.as_ref())
            .any(
                |what| (matches!(what.name.as_deref(), Some("add" | "sub"))
                    && matches!(
                        (&what.dests[..], &what.sources[..]),
                        ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Reg(_)])
                            if dest.register == self.stack && source.register == self.stack
                    ))
                    || (what.name.as_deref() == Some("mov")
                        && matches!(
                            (&what.dests[..], &what.sources[..]),
                            ([Loc::Reg(dest)], [Loc::Reg(_) | Loc::Mem(_)]) if dest.register == self.stack
                        )),
            )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L205
    /// `isProfitable`: the bytes saved against those of the adjustments the
    /// pushes need, over every call site.
    fn is_profitable(
        &self,
        contexts: &[CallContext],
        has_var_sized: bool,
        phase: i64,
    ) -> bool {
        // Always a win when no call frame can be reserved.
        if has_var_sized {
            return true;
        }
        let mut advantage: i64 = 0;
        for context in contexts {
            // No stack parameters: no adjustment either way.
            if context.no_stack_params {
                continue;
            }
            if !context.use_push {
                // Not reserving the frame costs a sub/add pair of about 3 bytes
                // each.
                advantage -= 6;
            } else {
                // The add after the call.
                advantage -= 3;
                // A sub before it, if the stack must be realigned.
                if (phase + context.expected_dist) % self.align != 0 {
                    advantage -= 3;
                }
                // Each push saves about 3 bytes.
                advantage += (context.expected_dist / self.slot) * 3;
            }
        }
        advantage >= 0
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L282
    /// `classifyInstruction`.
    fn classify_instruction(
        &self,
        body: &LirBody,
        one: Option<&Arc<Insn>>,
        used: &[RegId],
    ) -> Class {
        let Some(one) = one else { return Class::Exit };
        let Some(what) = one.what.as_ref() else { return Class::Skip };
        let name = what.name.as_deref().unwrap_or_default();
        // The stores of the frame, and the constant stores written as `and` and
        // `or`.
        if let [Loc::Mem(cell)] = what.dests.as_slice() {
            // `MOV32mi`/`MOV32mr` (and the 64-bit forms): a store narrower than
            // the slot is no push (a one-byte struct passed by value stored a
            // byte register, which `push` cannot take).
            if i64::from(cell.width) != self.slot {
                return Class::Exit;
            }
            match (what.op, name) {
                (Operation::Binary, "and") => {
                    return if imm_of(one, 1) == Some(0) { Class::Convert } else { Class::Exit };
                }
                (Operation::Binary, "or") => {
                    return if imm_of(one, 1) == Some(-1) { Class::Convert } else { Class::Exit };
                }
                (Operation::Move, "mov") => return Class::Convert,
                _ => {}
            }
        }
        // Any instruction that writes no memory, makes no call, and touches
        // neither the stack pointer nor a register an earlier store reads.
        if what.op == Operation::Call || what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_))) {
            return Class::Exit;
        }
        let (regs, bits) = (body.regs(), body.bits);
        let Some(effect) = liveness::effect(regs, bits, one) else { return Class::Exit };
        let touches = |lanes: &crate::backend::lanes::Lanes, register: RegId| {
            lanes.iter().any(|&(lane, _)| crate::model::ir::root(lane) == crate::model::ir::root(register))
        };
        if touches(&effect.all_reads(), self.stack) || touches(&effect.writes, self.stack) {
            return Class::Exit;
        }
        if used.iter().any(|&register| touches(&effect.writes, register)) {
            return Class::Exit;
        }
        Class::Skip
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L338
    /// `collectCallInfo`: whether the sequence at `at` is a run of stores that
    /// push the slots from 0 up, and the call that ends it.
    fn collect_call_info(
        &self,
        body: &LirBody,
        block: usize,
        setup_at: usize,
    ) -> CallContext {
        let insns: &[Arc<Insn>] = &body.blocks[block].insns;
        let setup = Arc::clone(&insns[setup_at]);
        let mut context = CallContext {
            block,
            setup: Arc::clone(&setup),
            call: None,
            sp_copy: None,
            expected_dist: 0,
            stores: vec![None; 4],
            no_stack_params: false,
            use_push: false,
        };
        // The upper bound of the stack parameters.
        let max_adjust = imm_of(&setup, 0).unwrap_or(0) / self.slot;
        if max_adjust == 0 {
            context.no_stack_params = true;
            return context;
        }
        let mut at = setup_at + 1;
        // The copy of the stack pointer selection makes, if it is there.
        let mut stack_base: Option<u32> = None;
        let mut sp_copy_at = None;
        let mut scan = at;
        while scan < insns.len() && !insns[scan].what.as_ref().is_some_and(|w| w.op == Operation::Call) {
            let what = insns[scan].what.as_ref();
            if let Some(what) = what
                && what.op == Operation::Move
                && matches!(
                    what.sources.as_slice(),
                    [Loc::Reg(register)] if register.register == self.stack
                )
                && let [Loc::Held(into)] = what.dests.as_slice()
            {
                stack_base = Some(into.value);
                sp_copy_at = Some(scan);
                context.sp_copy = Some(Arc::clone(&insns[scan]));
                break;
            }
            scan += 1;
        }
        if max_adjust > 4 {
            context.stores.resize(max_adjust as usize, None);
        }
        let mut used: Vec<RegId> = Vec::new();
        loop {
            if Some(at) == sp_copy_at {
                at += 1;
                continue;
            }
            let class = self.classify_instruction(body, insns.get(at), &used);
            if class == Class::Exit {
                break;
            }
            if class == Class::Convert {
                let one = &insns[at];
                let what = one.what.as_ref().expect("semantics");
                let Some(Loc::Mem(cell)) = what.dests.first() else { return context };
                // Only `mov imm/reg, k(%StackPtr)`: no index, no selector,
                // scale 1.
                let based = match stack_base {
                    Some(value) => cell.base.is_some_and(|base| base.value == value) && cell.through == RegId::None,
                    None => cell.base.is_none() && cell.through == self.stack,
                };
                if !based || cell.index.is_some() || cell.selector.is_some() || cell.scale != 1 || cell.addr.is_some() {
                    return context;
                }
                let disp = cell.offset;
                assert!(disp >= 0, "Negative stack displacement when passing parameters");
                // The unaligned case is not considered.
                if disp & (self.slot - 1) != 0 {
                    return context;
                }
                let slot = (disp / self.slot) as usize;
                assert!(
                    slot < context.stores.len(),
                    "Function call has more parameters than the stack is adjusted for."
                );
                // The same slot filled twice: fishy.
                if context.stores[slot].is_some() {
                    return context;
                }
                context.stores[slot] = Some(Arc::clone(one));
                // The registers the store reads.
                let (regs, bits) = (body.regs(), body.bits);
                if let Some(effect) = liveness::effect(regs, bits, one) {
                    used.extend(effect.all_reads().iter().map(|&(lane, _)| lane));
                }
            }
            at += 1;
        }
        // We now expect the end of the sequence: the call, then the destroy.
        let Some(call) = insns.get(at) else { return context };
        if !call.what.as_ref().is_some_and(|w| w.op == Operation::Call) {
            return context;
        }
        context.call = Some(Arc::clone(call));
        if insns.get(at + 1).and_then(|one| name_of(one)) != Some("call_frame_destroy") {
            return context;
        }
        // No gap: the stores are a prefix.
        let filled = context.stores.iter().take_while(|one| one.is_some()).count();
        context.expected_dist = filled as i64 * self.slot;
        // No parameters: nothing to do.
        if filled == 0 {
            return context;
        }
        // At the last parameter, or at a gap: no store may follow a gap.
        if context.stores[filled..].iter().any(Option::is_some) {
            return context;
        }
        context.use_push = true;
        context
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L505
    /// `adjustCallSequence`: the stores become pushes before the call, last
    /// slot first; the setup's adjustment is what is pushed.
    fn adjust_call_sequence(
        &self,
        body: &mut LirBody,
        context: &CallContext,
    ) {
        let mut insns: Vec<Arc<Insn>> = body.blocks[context.block].insns.iter().cloned().collect();
        let position = |insns: &[Arc<Insn>], one: &Arc<Insn>| {
            insns.iter().position(|other| Arc::ptr_eq(other, one)).expect("an instruction of the sequence")
        };
        // `setFrameAdjustment`.
        let setup_at = position(&insns, &context.setup);
        let mut setup = (*insns[setup_at]).clone();
        if let Some(what) = setup.what.as_mut() {
            if let Some(Loc::Imm(adjustment)) = what.sources.get_mut(2) {
                adjustment.value = context.expected_dist;
            }
        }
        // The fold looks the setup up by identity: keep the replacement.
        let setup = Arc::new(setup);
        insns[setup_at] = Arc::clone(&setup);
        let call = context.call.as_ref().expect("a call");
        for slot in (0..(context.expected_dist / self.slot) as usize).rev() {
            let store = Arc::clone(context.stores[slot].as_ref().expect("a store"));
            let what = store.what.as_ref().expect("semantics");
            let at = store.at;
            let push_what = |source: Loc| Semantics {
                name: Some("push".into()),
                sources: vec![source],
                ..Semantics::new(Operation::Push)
            };
            let (source_loc, extra_uses) = match (what.op, what.sources.as_slice()) {
                // `and [m], 0` and `or [m], -1` push their immediate.
                (Operation::Binary, [_, Loc::Imm(imm)]) => (Loc::Imm(imm.clone()), vec![]),
                (Operation::Move, [Loc::Imm(imm)]) => (Loc::Imm(imm.clone()), vec![]),
                // The push keeps the store's operand flags: a value the store
                // reads undefined (`ProcessImplicitDefs` erased its definition)
                // is read undefined by the push too.
                (Operation::Move, [Loc::Held(held)]) if store.undefs.contains(&held.value) => {
                    (Loc::Held(*held), vec![])
                }
                (Operation::Move, [Loc::Held(held)]) => (Loc::Held(*held), vec![held.value]),
                _ => unreachable!("Unexpected Opcode!"),
            };
            let mut pushed = Insn::new(at, Some((at, at)), Some(push_what(source_loc.clone())), vec![], extra_uses);
            if let Loc::Held(held) = &source_loc
                && store.undefs.contains(&held.value)
            {
                pushed.undefs = vec![held.value];
            }
            // If a push from memory is not slow here, fold the argument's load.
            if let Loc::Held(held) = source_loc
                && !self.slow_two_mem_ops
                && let Some(def_at) = self.can_fold_into_reg_push(body, &insns, &setup, held.value)
            {
                let def = Arc::clone(&insns[def_at]);
                let [Loc::Mem(cell)] = def.what.as_ref().expect("semantics").sources.as_slice() else { unreachable!() };
                pushed.what = Some(push_what(Loc::Mem(cell.clone())));
                pushed.uses = def.uses.clone();
                pushed.undefs = def.undefs.clone();
                insns.remove(def_at);
            }
            let before_call = position(&insns, call);
            insns.insert(before_call, Arc::new(pushed));
            let store_at = position(&insns, &store);
            insns.remove(store_at);
        }
        // The stack-pointer copy is no longer used in the sequence.
        if let Some(copy) = &context.sp_copy {
            let value = copy.defines.first().copied();
            let read = insns.iter().any(|one| !Arc::ptr_eq(one, copy) && value.is_some_and(|v| one.uses.contains(&v)));
            if !read {
                let at = position(&insns, copy);
                insns.remove(at);
            }
        }
        body.blocks[context.block] = body.blocks[context.block].with_insns(insns);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L605
    /// `canFoldIntoRegPush`: the load of `value`, if it is read once, made by
    /// a plain `mov` from memory before the setup in the same block, and
    /// nothing from there to the setup may write its cell.
    fn can_fold_into_reg_push(
        &self,
        body: &LirBody,
        insns: &[Arc<Insn>],
        setup: &Arc<Insn>,
        value: u32,
    ) -> Option<usize> {
        // Make sure this is the only use of the value.
        let reads: usize = body
            .blocks
            .iter()
            .map(|block| {
                block.insns.iter().filter(|one| one.uses.contains(&value)).count()
                    + block.phis.iter().filter(|phi| phi.incoming.iter().any(|&(_, v)| v == value)).count()
            })
            .sum();
        if reads != 1 {
            return None;
        }
        let setup_at = insns.iter().position(|one| Arc::ptr_eq(one, setup))?;
        let def_at = insns.iter().position(|one| {
            one.what.as_ref().is_some_and(|what| matches!(what.dests.as_slice(), [Loc::Held(h)] if h.value == value))
        })?;
        // The def is a `mov` from memory in this block, before the frame.
        if def_at >= setup_at {
            return None;
        }
        let def = &insns[def_at];
        let what = def.what.as_ref()?;
        let [Loc::Mem(cell)] = what.sources.as_slice() else { return None };
        if what.op != Operation::Move || def.volatile || cell.width != self.slot as u32 {
            return None;
        }
        // No instruction between the load and the frame may make folding
        // illegal.
        let barrier = insns[def_at + 1..setup_at]
            .iter()
            .any(
                |between| between.volatile
                    || between.call.is_some()
                    || may_write(between, cell, false)
                    || between
                        .what
                        .as_ref()
                        .is_none_or(|w| matches!(w.op, Operation::Call | Operation::Barrier | Operation::Escape)),
            );
        (!barrier).then_some(def_at)
    }
}

#[cfg(test)]
mod tests {
    use llrm_lir::{Held, Imm, Mem};

    use super::*;
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn pass() -> CallFrameOpt {
        CallFrameOpt {
            slot: 4,
            align: 16,
            stack: RegId::SP,
            has_var_sized: false,
            slow_two_mem_ops: false,
            call_alignment: 1,
            phase_step: 4,
        }
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
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }),
            defines,
            uses,
        ))
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    fn setup(size: i64) -> Arc<Insn> {
        insn(0, Operation::Nothing, "call_frame_setup", vec![], vec![imm(size), imm(0), imm(0)], vec![], vec![])
    }

    fn destroy(size: i64) -> Arc<Insn> {
        insn(9, Operation::Nothing, "call_frame_destroy", vec![], vec![imm(size), imm(0)], vec![], vec![])
    }

    fn call() -> Arc<Insn> {
        insn(8, Operation::Call, "call", vec![], vec![], vec![], vec![])
    }

    /// `mov [sp+offset], source`.
    fn store(
        offset: i64,
        source: Loc,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let cell = Mem { through: RegId::SP, offset, ..Mem::new(None, 4) };
        insn(1 + offset, Operation::Move, "mov", vec![Loc::Mem(cell)], vec![source], vec![], uses)
    }

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
    }

    fn body(insns: Vec<Arc<Insn>>) -> LirBody {
        let mut body =
            LirBody::new("g", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default());
        body.bits = 32;
        body
    }

    fn names(body: &LirBody) -> Vec<String> {
        body.blocks[0].insns.iter().map(|one| name_of(one).unwrap().to_owned()).collect()
    }

    /// A byte store into the frame (the copy of a one-byte struct passed by
    /// value) became `push r8`, which does not encode: `struct T { char x; }`
    /// passed uninitialised failed in the backend at -O1/-O2 (wc_mixed).
    #[test]
    fn a_store_narrower_than_a_slot_stays_a_store() {
        let cell = Mem { through: RegId::SP, offset: 0, ..Mem::new(None, 1) };
        let byte = insn(
            1,
            Operation::Move,
            "mov",
            vec![Loc::Mem(cell)],
            vec![Loc::Held(Held { value: 1, width: 1, offset: 0 })],
            vec![],
            vec![1],
        );
        // Two slots, as one push alone is never worth the adjustments.
        let before = body(vec![setup(8), store(4, imm(4), vec![]), byte, call(), destroy(8)]);
        let after = pass().run(&before);
        assert_eq!(names(&after), ["call_frame_setup", "mov", "mov", "call", "call_frame_destroy"]);
    }

    /// The pinned `llc -run-pass=x86-cf-opt` on a frame of two stores, `mov
    /// [esp+4], 4` and `mov [esp], %1`: `PUSH32i 4` then `PUSH32r %1`, the
    /// setup kept with its adjustment 8, the stores gone.
    #[test]
    fn two_stores_become_two_pushes_last_slot_first() {
        let before = body(vec![setup(8), store(4, imm(4), vec![]), store(0, held(1), vec![1]), call(), destroy(8)]);
        let after = pass().run(&before);
        assert_eq!(names(&after), ["call_frame_setup", "push", "push", "call", "call_frame_destroy"]);
        let pushes: Vec<_> =
            after.blocks[0].insns[1..3].iter().map(|one| one.what.as_ref().unwrap().sources.clone()).collect();
        assert_eq!(pushes, [vec![imm(4)], vec![held(1)]]);
        assert_eq!(imm_of(&after.blocks[0].insns[0], 2), Some(8));
    }

    /// `printf("%d", undef)` at -O1: the store read its value undefined, the
    /// push built from it read it as a real use of a value nothing defines,
    /// and the allocator panicked "a unique value" (bits.c).
    #[test]
    fn a_push_made_from_an_undef_read_reads_it_undefined() {
        let mut undef_store = (*store(0, held(1), vec![])).clone();
        undef_store.undefs = vec![1];
        // Two stores: one push is not worth the adjustments.
        let after =
            pass().run(&body(vec![setup(8), store(4, imm(4), vec![]), Arc::new(undef_store), call(), destroy(8)]));
        let push = &after.blocks[0].insns[2];
        assert_eq!(name_of(push), Some("push"));
        assert_eq!((push.uses.clone(), push.undefs.clone()), (vec![], vec![1]), "{push:?}");
    }

    /// `printf(0, g)` at m32 -O2 kept `mov eax, [g]; push eax`: the fold
    /// looked the setup up by an `Arc` it had already replaced, so the load
    /// was never folded (the pin emits `push [g]`). The push also keeps the
    /// load's address operands, undef flags included.
    #[test]
    fn a_once_read_load_is_folded_into_its_push_keeping_its_operand_flags() {
        let cell = Mem { through: RegId::DI, offset: 0, ..Mem::new(None, 4) };
        let mut load =
            (*insn(0, Operation::Move, "mov", vec![held(1)], vec![Loc::Mem(cell)], vec![1], vec![8])).clone();
        load.undefs = vec![7];
        let after = pass().run(&body(vec![
            Arc::new(load),
            setup(8),
            store(4, imm(4), vec![]),
            store(0, held(1), vec![1]),
            call(),
            destroy(8),
        ]));
        assert_eq!(names(&after), ["call_frame_setup", "push", "push", "call", "call_frame_destroy"]);
        let push = &after.blocks[0].insns[2];
        assert!(matches!(push.what.as_ref().unwrap().sources.as_slice(), [Loc::Mem(_)]), "{push:?}");
        assert_eq!((push.uses.clone(), push.undefs.clone()), (vec![8], vec![7]), "{push:?}");
    }

    /// `f(a, v)` with `v` volatile folded `mov eax,[v]` into `push [v]` and
    /// moved the access past the other loads: the program's volatile order
    /// changed (CF3). A volatile load, or any volatile access between the load
    /// and the frame, keeps the load where it is.
    #[test]
    fn a_volatile_access_is_not_moved_into_a_push() {
        let cell = Mem { through: RegId::DI, offset: 0, ..Mem::new(None, 4) };
        let plain = || {
            (*insn(0, Operation::Move, "mov", vec![held(1)], vec![Loc::Mem(cell.clone())], vec![1], vec![8])).clone()
        };
        let frame = |first: Arc<Insn>, between: Option<Arc<Insn>>| {
            let mut insns = vec![first];
            insns.extend(between);
            insns.extend([setup(8), store(4, imm(4), vec![]), store(0, held(1), vec![1]), call(), destroy(8)]);
            names(&pass().run(&body(insns)))
        };
        let kept = ["mov", "call_frame_setup", "push", "push", "call", "call_frame_destroy"];
        let mut volatile_load = plain();
        volatile_load.volatile = true;
        assert_eq!(frame(Arc::new(volatile_load), None), kept);
        let mut volatile_between =
            (*insn(5, Operation::Move, "mov", vec![held(2)], vec![Loc::Mem(cell.clone())], vec![2], vec![8])).clone();
        volatile_between.volatile = true;
        assert_eq!(
            frame(Arc::new(plain()), Some(Arc::new(volatile_between))),
            ["mov", "mov", "call_frame_setup", "push", "push", "call", "call_frame_destroy"]
        );
    }

    /// A gap in the slots (slot 1 stored, slot 0 not) is left as stores.
    #[test]
    fn a_gap_leaves_the_stores() {
        let before = body(vec![setup(8), store(4, imm(4), vec![]), call(), destroy(8)]);
        assert_eq!(names(&pass().run(&before)), ["call_frame_setup", "mov", "call", "call_frame_destroy"]);
    }

    /// The same slot filled twice is fishy: no change.
    #[test]
    fn a_slot_stored_twice_is_left() {
        let before = body(vec![setup(4), store(0, imm(1), vec![]), store(0, imm(2), vec![]), call(), destroy(4)]);
        assert_eq!(names(&pass().run(&before)), ["call_frame_setup", "mov", "mov", "call", "call_frame_destroy"]);
    }

    /// A frame left open (no destroy in its block) makes the pass refuse the
    /// function (`isLegal`).
    #[test]
    fn an_unclosed_frame_is_illegal() {
        let before = body(vec![setup(4), store(0, imm(1), vec![]), call()]);
        assert_eq!(names(&pass().run(&before)), ["call_frame_setup", "mov", "call"]);
    }

    /// Four bytes pushed at an alignment of 16 cost an add and a sub, and save
    /// one push: 3 - 3 - 3 < 0, so it stays stores unless no frame can be
    /// reserved (`isProfitable`).
    #[test]
    fn one_push_is_not_worth_the_adjustments_but_two_are() {
        let one = body(vec![setup(4), store(0, imm(1), vec![]), call(), destroy(4)]);
        assert_eq!(names(&pass().run(&one)), ["call_frame_setup", "mov", "call", "call_frame_destroy"]);
        let var_sized = CallFrameOpt { has_var_sized: true, ..pass() };
        assert_eq!(names(&var_sized.run(&one)), ["call_frame_setup", "push", "call", "call_frame_destroy"]);
    }

    /// A function that allocates dynamically reserves no frame, so the pin
    /// pushes its area whatever the heuristic says: one push stays a store in
    /// a function without
    /// (`one_push_is_not_worth_the_adjustments_but_two_are`).
    #[test]
    fn a_function_with_a_dynamic_allocation_pushes_every_area() {
        let sp = || Loc::Reg(Reg { register: RegId::SP, width: 4 });
        let alloca = insn(
            0,
            Operation::Binary,
            "sub",
            vec![sp()],
            vec![sp(), Loc::Reg(Reg { register: RegId::AX, width: 4 })],
            vec![],
            vec![],
        );
        let one = body(vec![alloca, setup(4), store(0, imm(1), vec![]), call(), destroy(4)]);
        assert_eq!(names(&pass().run(&one)), ["sub", "call_frame_setup", "push", "call", "call_frame_destroy"]);
    }

    fn reserve(insns: Vec<Arc<Insn>>) -> (Vec<String>, i64) {
        let frame = std::rc::Rc::new(std::cell::RefCell::new(crate::backend::frame::Frame::new(0)));
        let mut pass = ReserveCallFrame {
            call_alignment: 1,
            phase_step: 4,
            saved: Vec::new(),
            stack: RegId::SP,
            width: 4,
            frame: Some(frame.clone()),
        };
        let after = pass.transform(body(insns)).expect("reserves");
        let area = frame.borrow().call_area;
        (names(&after), area)
    }

    /// Calls that push nothing left a `sub`/`add` pair each; a function with
    /// none pushed reserves the largest area once, in the prologue.
    #[test]
    fn a_function_with_no_pushed_call_reserves_its_largest_area() {
        let first = vec![setup(8), store(0, held(1), vec![1]), store(4, held(2), vec![2]), call(), destroy(8)];
        let second = vec![setup(16), store(0, held(1), vec![1]), call(), destroy(16)];
        let (after, area) = reserve([first, second].concat());
        assert_eq!(after, ["mov", "mov", "call", "mov", "call"]);
        assert_eq!(area, 16);
    }

    /// One pushed call un-reserves the function (`hasPushSequences`): every
    /// call adjusts for itself and the frame grows by nothing.
    #[test]
    fn a_pushed_call_keeps_the_per_call_adjustments() {
        let pushed = insn(0, Operation::Push, "push", vec![], vec![imm(1)], vec![], vec![]);
        let plain = vec![setup(8), store(0, held(1), vec![1]), call(), destroy(8)];
        let made =
            insn(0, Operation::Barrier, "call_frame_setup", vec![], vec![imm(4), imm(0), imm(4)], vec![], vec![]);
        let (after, area) = reserve([vec![made, pushed, call(), destroy(4)], plain].concat());
        assert_eq!(after, ["push", "call", "add", "sub", "mov", "call", "add"]);
        assert_eq!(area, 0);
    }

    /// An alloca kept to a 16-byte call alignment is `mov r, sp; sub r, n; and
    /// r, -16; mov sp, r`, no `sub sp, reg`: the frame was reserved, and a
    /// call's `mov [esp], arg` then overwrote the array it
    /// sat on (vla_loop_stack_restored m32 reported 90, not 93).
    #[test]
    fn an_alloca_that_copies_the_stack_pointer_back_reserves_no_frame() {
        let sp = || Loc::Reg(Reg { register: RegId::SP, width: 4 });
        let cx = || Loc::Reg(Reg { register: RegId::CX, width: 4 });
        let alloca = insn(0, Operation::Move, "mov", vec![sp()], vec![cx()], vec![], vec![]);
        let (after, area) = reserve(vec![alloca, setup(4), store(0, held(1), vec![1]), call(), destroy(4)]);
        assert_eq!(after, ["mov", "sub", "mov", "call", "add"]);
        assert_eq!(area, 0);
    }

    fn lower(insns: Vec<Arc<Insn>>) -> Vec<String> {
        lower_aligned(insns, 1, 0).iter().map(|(name, _)| name.clone()).collect()
    }

    /// The lowered instructions, each with its `sub`/`add` amount (0 for any
    /// other).
    fn lower_aligned(
        insns: Vec<Arc<Insn>>,
        call_alignment: i64,
        phase: i64,
    ) -> Vec<(String, i64)> {
        let mut lowering = LowerCallFrame { stack: RegId::SP, width: 4, call_alignment, phase, defer: false };
        let after = lowering.transform(body(insns)).expect("lowers");
        after.blocks[0]
            .insns
            .iter()
            .map(|one| (name_of(one).unwrap().to_owned(), imm_of(one, 1).unwrap_or(0)))
            .collect()
    }

    /// A 16-aligned call padded an 8-byte push block by 8 whatever the frame
    /// left (`sub esp, 8` before the pushes, twice in a frame that had
    /// itself been padded to 16: QuickrBasic m32 -Os +3.04%); a frame that
    /// leaves the stack 8 past a multiple of 16 needs none.
    #[test]
    fn a_pad_is_what_the_frames_phase_leaves_the_call_short_of() {
        let pushed =
            |n| insn(0, Operation::Barrier, "call_frame_setup", vec![], vec![imm(n), imm(0), imm(n)], vec![], vec![]);
        let body = |n| vec![pushed(n), call(), destroy(n)];
        assert_eq!(lower_aligned(body(8), 16, 0), [("sub".into(), 8), ("call".into(), 0), ("add".into(), 16)]);
        assert_eq!(lower_aligned(body(8), 16, 8), [("call".into(), 0), ("add".into(), 8)]);
        let none = vec![setup(0), call(), destroy(0)];
        assert_eq!(lower_aligned(none.clone(), 16, 8), [("sub".into(), 8), ("call".into(), 0), ("add".into(), 8)]);
        assert_eq!(lower_aligned(none, 16, 0), [("call".into(), 0)]);
    }

    /// Three calls push 8 bytes and one pushes none: the frame that leaves the
    /// stack 8 past a multiple of 16 pads the one, where 0 pads the three;
    /// and a frame that is itself 8 short pays for its own pad at
    /// 0 (QuickrBasic's `ast_new`: `sub esp, 8` in the prologue and again at
    /// its call, +3.04% Os).
    #[test]
    fn the_frame_takes_the_phase_that_pads_the_fewest_bytes() {
        // calls of 8, 8, 8 and 0 bytes
        assert_eq!(best_phase(&[8, 8, 8, 0], Some(8), 16, 4, false), 8);
        // two calls of 0 bytes and one of 8: no pad at 0, and the frame is
        // aligned already
        assert_eq!(best_phase(&[8, 0, 0], Some(0), 16, 4, false), 0);
        // one call of 8 bytes from a frame whose own pad is 8 at phase 0: phase
        // 8 pays neither
        assert_eq!(best_phase(&[8], Some(8), 16, 4, false), 8);
        // a frame object 8-aligned keeps the phase a multiple of 8
        assert_eq!(best_phase(&[4], Some(4), 16, 8, false) % 8, 0);
        // `isProfitable` prices the calls alone: a one-argument call is aligned
        // at phase 12, not 0
        assert_eq!(best_phase(&[4, 4], None, 16, 4, false), 12);
    }

    fn lower_deferred(
        insns: Vec<Arc<Insn>>,
        over_aligned: bool,
    ) -> Vec<(String, i64)> {
        let mut lowering = LowerCallFrame { stack: RegId::SP, width: 4, call_alignment: 16, phase: 0, defer: true };
        let mut input = body(insns);
        if over_aligned {
            input.over_aligned.push((0, 16));
        }
        let after = lowering.transform(input).expect("lowers");
        after.blocks[0]
            .insns
            .iter()
            .map(|one| (name_of(one).unwrap().to_owned(), imm_of(one, 1).unwrap_or(0)))
            .collect()
    }

    fn pushed_call(n: i64) -> Vec<Arc<Insn>> {
        let pushed =
            insn(0, Operation::Barrier, "call_frame_setup", vec![], vec![imm(n), imm(0), imm(n)], vec![], vec![]);
        vec![pushed, call(), destroy(n)]
    }

    /// Two calls of 4 bytes each freed 16 and padded 12 again: the stack where
    /// the second finds it is the same, with one step of 4 between them
    /// (GCC's `pending_stack_adjust`; a block of 4 bytes needs a pad of 12 at
    /// phase 0).
    #[test]
    fn the_cleanup_of_one_call_is_the_pad_of_the_next() {
        let calls: Vec<_> = pushed_call(4).into_iter().chain(pushed_call(4)).collect();
        assert_eq!(
            lower_deferred(calls, false),
            [("sub".into(), 12), ("call".into(), 0), ("add".into(), 4), ("call".into(), 0), ("add".into(), 16)]
        );
    }

    /// Nothing is deferred across what pins the stack: a push, a pop, a call
    /// that is not between the two steps' own, a use of the stack pointer as a
    /// register (an `alloca`), or a function that has an over-aligned object
    /// addressed from the stack pointer.
    #[test]
    fn nothing_is_deferred_across_what_pins_the_stack() {
        let cx = || Loc::Reg(Reg { register: RegId::CX, width: 4 });
        let sp = || Loc::Reg(Reg { register: RegId::SP, width: 4 });
        let kept = [("sub".into(), 12), ("call".into(), 0), ("add".into(), 16)];
        let two = |between: Arc<Insn>| -> Vec<Arc<Insn>> {
            pushed_call(4).into_iter().chain([between]).chain(pushed_call(4)).collect()
        };
        for between in [
            insn(0, Operation::Push, "push", vec![], vec![cx()], vec![], vec![]),
            insn(0, Operation::Pop, "pop", vec![cx()], vec![], vec![], vec![]),
            insn(0, Operation::Binary, "sub", vec![sp()], vec![sp(), cx()], vec![], vec![]),
            insn(0, Operation::Escape, "asm", vec![], vec![], vec![], vec![]),
            call(),
        ] {
            let after = lower_deferred(two(between), false);
            assert_eq!(&after[..3], &kept, "{after:?}");
            let steps = |name: &str, amount: i64| after.iter().filter(|one| **one == (name.to_owned(), amount)).count();
            assert_eq!((steps("sub", 12), steps("add", 16)), (2, 2), "both pads and both cleanups stay: {after:?}");
        }
        let calls: Vec<_> = pushed_call(4).into_iter().chain(pushed_call(4)).collect();
        let after = lower_deferred(calls, true);
        assert_eq!(after.iter().map(|(_, amount)| *amount).collect::<Vec<_>>(), [12, 0, 16, 12, 0, 16], "{after:?}");
    }

    /// The frame pseudos that no push replaced were `sub`/`add` in the DAG
    /// emitter; at -O0 nothing converts them, and every level lowers them.
    #[test]
    fn a_frame_nothing_pushed_is_a_sub_and_an_add() {
        let before = vec![setup(8), store(0, held(1), vec![1]), store(4, held(2), vec![2]), call(), destroy(8)];
        assert_eq!(lower(before), ["sub", "mov", "mov", "call", "add"]);
    }

    /// Pushed bytes are not subtracted again, and what the callee pops is not
    /// added back.
    #[test]
    fn what_the_pushes_made_and_the_callee_pops_is_not_adjusted_again() {
        let pushed =
            insn(0, Operation::Barrier, "call_frame_setup", vec![], vec![imm(8), imm(0), imm(8)], vec![], vec![]);
        let popped = insn(9, Operation::Barrier, "call_frame_destroy", vec![], vec![imm(8), imm(8)], vec![], vec![]);
        assert_eq!(lower(vec![pushed, call(), popped]), ["call"]);
    }
}
