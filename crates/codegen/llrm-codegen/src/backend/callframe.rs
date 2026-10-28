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
//! The input exists in LIR only as hand-built tests: selection pushes the
//! arguments itself (DEVIATE CG6 of row 50, `pushfold.rs`), and the pass finds
//! no setup in a body of ours. Its producer is the call lowering of the
//! SelectionDAG port (`LowerCall`: `ADJCALLSTACKDOWN`, stores to `[esp+k]`);
//! owner sdag-a, with the reserved call frame that prologue insertion (row 80,
//! 2026-11-13) lays out. Until then the pass does nothing on the corpus.
//!
//! N/A: the Darwin and Win64 conditions of `isLegal`, the stack probe, the
//! 64-bit forms (`PUSH64`, `INSERT_SUBREG`) and `BuildCFI` (LIR has no CFI
//! instruction). DEVIATE CF2 (no number yet): `hasVarSizedObjects` is not in
//! LIR; the pass is given it (false until row 80). Owner w-5, review
//! 2026-11-13.
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
        let probe =
            CallFrameOpt { slot: 0, align: 0, stack: self.stack, has_var_sized: false, slow_two_mem_ops: false };
        let mut lowering = LowerCallFrame { stack: self.stack, width: self.width };
        let Some(frame) = self.frame.as_ref().filter(|_| !pushed && !probe.allocates_dynamically(&body)) else {
            return lowering.transform(body);
        };
        let area = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|one| name_of(one) == Some("call_frame_setup"))
            .filter_map(|one| imm_of(one, 0))
            .max()
            .unwrap_or(0);
        // The destroys that re-reserve what a callee popped become `sub`.
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
                    Some("call_frame_setup") => ("sub", imm_of(one, 0).unwrap_or(0) - imm_of(one, 2).unwrap_or(0)),
                    Some("call_frame_destroy") => ("add", imm_of(one, 0).unwrap_or(0) - imm_of(one, 1).unwrap_or(0)),
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
            out.blocks[at] = block.with_insns(insns);
        }
        Ok(out)
    }
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
        if !self.is_profitable(&contexts, self.has_var_sized || self.allocates_dynamically(body)) {
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

    /// `MFI.hasVarSizedObjects()`: a `sub sp, reg`, as `framefree` reads it.
    fn allocates_dynamically(
        &self,
        body: &LirBody,
    ) -> bool {
        body.blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter_map(|one| one.what.as_ref())
            .any(
                |what| matches!(what.name.as_deref(), Some("add" | "sub"))
                    && matches!(
                        (&what.dests[..], &what.sources[..]),
                        ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Reg(_)])
                            if dest.register == self.stack && source.register == self.stack
                    ),
            )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86CallFrameOptimization.cpp#L205
    /// `isProfitable`: the bytes saved against those of the adjustments the
    /// pushes need, over every call site.
    fn is_profitable(
        &self,
        contexts: &[CallContext],
        has_var_sized: bool,
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
                if context.expected_dist % self.align != 0 {
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
        if let [Loc::Mem(_)] = what.dests.as_slice() {
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
        insns[setup_at] = Arc::new(setup);
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
                (Operation::Move, [Loc::Held(held)]) => (Loc::Held(*held), vec![held.value]),
                _ => unreachable!("Unexpected Opcode!"),
            };
            let mut pushed = Insn::new(at, Some((at, at)), Some(push_what(source_loc.clone())), vec![], extra_uses);
            // If a push from memory is not slow here, fold the argument's load.
            if let Loc::Held(held) = source_loc
                && !self.slow_two_mem_ops
                && let Some(def_at) = self.can_fold_into_reg_push(body, &insns, context, held.value)
            {
                let def = Arc::clone(&insns[def_at]);
                let [Loc::Mem(cell)] = def.what.as_ref().expect("semantics").sources.as_slice() else { unreachable!() };
                pushed.what = Some(push_what(Loc::Mem(cell.clone())));
                pushed.uses = def.uses.clone();
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
        context: &CallContext,
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
        let setup_at = insns.iter().position(|one| Arc::ptr_eq(one, &context.setup))?;
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
        CallFrameOpt { slot: 4, align: 16, stack: RegId::SP, has_var_sized: false, slow_two_mem_ops: false }
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
        Loc::Held(Held { value, width: 4 })
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
        let mut pass = ReserveCallFrame { stack: RegId::SP, width: 4, frame: Some(frame.clone()) };
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

    fn lower(insns: Vec<Arc<Insn>>) -> Vec<String> {
        let mut lowering = LowerCallFrame { stack: RegId::SP, width: 4 };
        let after = lowering.transform(body(insns)).expect("lowers");
        names(&after)
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
