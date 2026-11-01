//! Decides, before allocation, whether a function can do without its frame
//! register, which is then one more value register.
//!
//! # How it works
//! `has_fp` is LLVM's `X86FrameLowering::hasFPImpl`: a function keeps its
//! frame register when `MachineFrameInfo` says so, and `pinned_by` names which
//! fact does ([`Pinned`]): the frame address is taken, there are variable
//! sized objects, or the stack adjustment is opaque. The rest of the
//! function's facts (`far`, inline code, a landing pad) are read by `has_fp`.
//! The register is free when `masm` can address
//! every frame cell through the stack pointer, which needs the stack depth at
//! every instruction. The depth is computed here from the LIR before
//! allocation, reading the same things `masm::stack_addressed` reads; spill
//! code adds cells and moves but no push or pop, so the depth does not change.
//!
//! Per block the walk sums pushes and pops (word or dword), `add sp, n` and
//! `sub sp, n`, and the bytes each call pops. Depths are then propagated along
//! the CFG from the entry:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   e["<b>entry</b><br/><small>depth 0</small>"]:::mid
//!   a["<b>A: +4</b>"]:::mid
//!   b["<b>B: +4</b>"]:::mid
//!   j["<b>join</b><br/><small>depth 4 on both edges: fine<br/>4 and 6: keep the<br/>frame register</small>"]:::mid
//!   e --> a
//!   e --> b
//!   a --> j
//!   b --> j
//! ```
//!
//! *Frame depth at each block; every way into a block must agree, every return
//! at depth 0*
//!
//! The frame register is kept where any of these holds: the target does not
//! allow freeing it (`registers.optional`), the body is not 32-bit, or it has
//! inline code (bytes of x87 register operations, `fsin` for one, touch no
//! stack cell and do not count), a landing pad, `returns_twice`, or debug
//! variables not located by the CFA. It is kept where the frame register is
//! read or written as a value, or is a base or index of anything but a frame
//! cell; the stack pointer is written other than by `add`/`sub` with an
//! immediate; there is an indirect call, jump or branch, a `leave`, an escape
//! or a barrier other than `fnstcw`, `fldcw`, `fnstsw`, `in` and `out`, a push
//! or pop of a width other than 2 or 4, or two edges disagree on the depth.
//!
//! When freed, the frame register joins the allocatable set and the saved
//! registers (the prologue pushes it where it is used).
//!
//! # Example
//! `mix(a, b, c, d, n)` on `-m32 -O2` updates four accumulators in a loop. The
//! body has no push, pop or indirect call, so the stack depth is known at every
//! instruction and the frame register is free: the prologue saves `ebp` with
//! the other callee-saved registers, the loop uses it as a value register, and
//! the argument is read through `esp`. The decision itself is not printed; this
//! is the listing it leads to.
//!
//! ```asm
//! _mix proc near
//!     push esi
//!     push edi
//!     push ebp
//! L0_0:
//!     mov esi, dword ptr [esp+16]
//!     ; ...
//! L0_8:
//!     mov ebp, edx
//!     imul ebp, edi
//!     add eax, ebp
//!     ; ...
//!     lea ebp, [ecx+edx]
//!     xor eax, ebp
//!     inc edi
//! ```
//!
//! From `-S` of a nib function with four accumulators updated in a `for` loop,
//! with `-m32 -O2 -fno-inline-functions -fno-inline-functions-called-once`.
//!
//! # Runs
//! In `assemble::staged`, after isel and before `frame::of`, once per function.
//! Never for a BASIC module, whose frames are chained through BP.
//!
//! # Needs
//! [`crate::backend::isel`] (`pops`, `inline`, `landing`).
//!
//! # See also
//! LLVM `X86FrameLowering::hasFP`; `masm::stack_addressed`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_lir::registers::RegId;
use llrm_mir::opcode::Attribute;

use crate::model::ir::{Loc, Operation};
use crate::model::lir::LirBody;

/// The registers that are the frame register as LIR names it.
fn is_frame(
    register: RegId,
    pointer: RegId,
) -> bool {
    register != RegId::None && crate::model::ir::root(register) == crate::model::ir::root(pointer)
}

/// Whether `bytes` are x87 register operations only: pairs of an escape opcode
/// (`D8`..`DF`) and a register-form ModRM (`C0` and up), as the sequences a
/// frontend spells as bytes (`fsin`, `fatan`, `fexp2`). Such code reads no
/// memory, so it cannot name the stack or the frame.
pub fn x87_registers_only(bytes: &[u8]) -> bool {
    bytes.len() % 2 == 0 && bytes.chunks(2).all(|pair| (0xd8..=0xdf).contains(&pair[0]) && pair[1] >= 0xc0)
}

thread_local! {
    /// Blocks `without_frame_register` has looked over to find the blocks it
    /// follows, for a test of its slope.
    pub static LOOKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `MachineFunction::disableFramePointerElim`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineFunction.cpp#L750>):
/// the function's `"frame-pointer"` attribute keeps the frame register:
/// `all` always, `non-leaf` and `non-leaf-no-reserve` where it calls.
///
/// DEVIATE FP1: `reserved` (the register is kept out of allocation without a
/// frame) is read as `none`.
pub fn disable_frame_pointer_elim(
    attrs: &[Attribute],
    has_calls: bool,
) -> bool {
    let policy = attrs
        .iter()
        .find_map(
            |one| match one {
                Attribute::Str(key, Some(value)) if key == "frame-pointer" => Some(value.as_str()),
                _ => None,
            },
        );
    match policy {
        Some("all") => true,
        Some("non-leaf" | "non-leaf-no-reserve") => has_calls,
        _ => false,
    }
}

/// What keeps a function's frame register, as `MachineFrameInfo` names it:
/// the facts `X86FrameLowering::hasFPImpl` reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pinned {
    /// `isFrameAddressTaken`: the frame register is read or written as a
    /// value, is the base or index of anything but a frame cell, or a `leave`
    /// reads it.
    FrameAddressTaken,
    /// `hasVarSizedObjects`: the stack pointer moves by an amount that is not
    /// a constant.
    VarSizedObjects,
    /// `hasOpaqueSPAdjustment`: the stack depth at some instruction cannot be
    /// modelled: the stack pointer is written another way, a push or pop is
    /// of a width but 2 or 4, an indirect call's callee may pop, an escape or
    /// barrier may move it, or two edges into a block disagree on the depth.
    OpaqueSpAdjustment,
}

/// `X86FrameLowering::hasFPImpl`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86FrameLowering.cpp#L124-L134>):
/// whether the function described by `body` (its calls' popped bytes in
/// `pops`) keeps a dedicated frame register.
///
/// DEVIATE F1: the target allows freeing the register only in 32-bit code
/// (`registers.optional`, `body.bits`): real mode has no stack-pointer
/// addressing. DEVIATE F2: `returns_twice` keeps it, as `longjmp` restores the
/// register as a value. DEVIATE F3: debug variables not located by the CFA
/// keep it. `inline` code is `hasInlineAsm`'s opaque stack effect; `landing`
/// is `callsUnwindInit`.
pub fn has_fp(
    body: &LirBody,
    registers: &llrm_target::FrameRegisters,
    pops: &BTreeMap<i64, i64>,
    inline: bool,
    far: bool,
    landing: bool,
) -> bool {
    if !registers.optional
        || body.bits < 32
        || far
        || inline
        || landing
        || body.returns_twice
        || !body.over_aligned.is_empty()
        || (!body.variables.is_empty() && !body.cfa_variables)
    {
        return true;
    }
    let pinned = pinned_by(body, registers, pops);
    llrm_support::debug!(
        "frame",
        "{}: {}",
        body.name,
        pinned.map_or_else(|| "freed".to_string(), |why| format!("kept: {why:?}"))
    );
    pinned.is_some()
}

/// What `MachineFrameInfo` would say pins the frame register of `body`, read
/// from its instructions: the first fact found.
pub fn pinned_by(
    body: &LirBody,
    registers: &llrm_target::FrameRegisters,
    pops: &BTreeMap<i64, i64>,
) -> Option<Pinned> {
    let regs = body.regs();
    let pointer = registers.pointer;
    // The depth at the top of each block, and each block's effect.
    let mut effects: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    // The depth within a block at which it returns.
    let mut leaves: BTreeMap<i64, i64> = BTreeMap::new();
    for block in &body.blocks {
        let mut depth = 0i64;
        // Between `call_frame_setup` and `call_frame_destroy`, whose operands
        // give the bytes a callee pops.
        let mut in_call_frame = false;
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            for place in what.dests.iter().chain(&what.sources) {
                match place {
                    Loc::Reg(reg) if is_frame(reg.register, pointer) => return Some(Pinned::FrameAddressTaken),
                    Loc::Mem(cell) if is_frame(cell.index_through, pointer) => return Some(Pinned::FrameAddressTaken),
                    Loc::Address(address) if is_frame(address.index_through, pointer) => {
                        return Some(Pinned::FrameAddressTaken);
                    }
                    // A register-based cell is fine; the frame register as a
                    // base is only a frame cell.
                    Loc::Mem(cell) if is_frame(cell.through, pointer) && !cell.in_frame() => {
                        return Some(Pinned::FrameAddressTaken);
                    }
                    Loc::Address(address)
                        if is_frame(address.through, pointer)
                            && address
                                .addr
                                .as_ref()
                                .is_none_or(|addr| addr.space != crate::model::ir::Space::Frame) =>
                    {
                        return Some(Pinned::FrameAddressTaken);
                    }
                    _ => {}
                }
            }
            match what.op {
                Operation::Push | Operation::Pop => {
                    let (places, sign) = if what.op == Operation::Push { (&what.sources, 1) } else { (&what.dests, -1) };
                    let [place] = &places[..] else { return Some(Pinned::OpaqueSpAdjustment) };
                    let width = match place {
                        Loc::Reg(reg) => reg.register.size() as i64,
                        Loc::Held(held) => i64::from(held.width),
                        Loc::Imm(imm) => i64::from(imm.width),
                        Loc::Mem(cell) => i64::from(cell.width),
                        _ => return Some(Pinned::OpaqueSpAdjustment),
                    };
                    if !matches!(width, 2 | 4 | 8) {
                        return Some(Pinned::OpaqueSpAdjustment);
                    }
                    depth += sign * width;
                }
                // As `masm::stack_addressed`: an exchange of x87 registers,
                // memory or general registers and the x87 and port instructions
                // leave the stack pointer alone.
                Operation::Exchange if what.dests.iter().chain(&what.sources).all(|place| (crate::backend::target::positional_place(regs, place) || matches!(place, Loc::Mem(_))) || matches!(
                    place,
                    Loc::Reg(reg) if crate::model::ir::root(reg.register) != crate::model::ir::root(registers.stack)
                )) => {}
                Operation::Barrier if matches!(
                    what.name.as_deref(),
                    Some("fnstcw" | "fldcw" | "fnstsw" | "in" | "out")
                ) => {}
                // `X86CallFrameOptimization` and the call-frame lowering turn
                // these into pushes, `sub sp` and `add sp`: the bytes they
                // move are `N - A` and `N - M`, as `ADJCALLSTACKDOWN/UP`.
                Operation::Barrier if matches!(
                    what.name.as_deref(),
                    Some("call_frame_setup" | "call_frame_destroy")
                ) => {
                    let imm = |at: usize| match what.sources.get(at) {
                        Some(Loc::Imm(imm)) => imm.value,
                        _ => 0,
                    };
                    in_call_frame = what.name.as_deref() == Some("call_frame_setup");
                    if in_call_frame {
                        depth += imm(0) - imm(2);
                    } else {
                        depth -= imm(0) - imm(1);
                    }
                }
                Operation::Leave => return Some(Pinned::FrameAddressTaken),
                Operation::Exchange | Operation::Escape | Operation::Barrier => return Some(Pinned::OpaqueSpAdjustment),
                Operation::Nothing if what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")) => return Some(Pinned::OpaqueSpAdjustment),
                Operation::Call => {
                    if what.indirect && !in_call_frame {
                        return Some(Pinned::OpaqueSpAdjustment);
                    }
                    depth -= pops.get(&one.at).copied().unwrap_or(0);
                }
                // A jump table's targets are the blocks it names, at the depth
                // of the jump.
                Operation::Jump | Operation::Branch if what.indirect && what.table.is_empty() => {
                    return Some(Pinned::OpaqueSpAdjustment);
                }
                Operation::Return => leaves.insert(block.at, depth).map_or((), |_| ()),
                _ if what.dests.iter().any(|place| matches!(
                    place,
                    Loc::Reg(reg) if crate::model::ir::root(reg.register) == crate::model::ir::root(registers.stack)
                )) => {
                    // Only `add sp, n` and `sub sp, n` are understood.
                    let amount = match (&what.dests[..], &what.sources[..]) {
                        ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(imm)]) if dest.register == source.register && imm.address.is_none() => imm.value,
                        // `sub sp, reg`: stack the size of which is run time.
                        ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Reg(_)])
                            if dest.register == source.register
                                && matches!(what.name.as_deref(), Some("add" | "sub")) =>
                        {
                            return Some(Pinned::VarSizedObjects);
                        }
                        _ => return Some(Pinned::OpaqueSpAdjustment),
                    };
                    match what.name.as_deref() {
                        Some("add") => depth -= amount,
                        Some("sub") => depth += amount,
                        _ => return Some(Pinned::OpaqueSpAdjustment),
                    }
                }
                _ => {}
            }
        }
        effects.insert(block.at, Some(depth));
    }
    // Every way into a block agrees on the depth.
    let mut at: BTreeMap<i64, i64> = BTreeMap::new();
    let mut pending = vec![(body.entry, 0i64)];
    let mut seen = BTreeSet::new();
    // A block is found by its number once, not by a search of the body each
    // time a way reaches it.
    let by_number: BTreeMap<i64, &crate::model::lir::Block> = body.blocks.iter().map(|one| (one.at, one)).collect();
    while let Some((block, here)) = pending.pop() {
        match at.get(&block) {
            Some(&there) if there != here => return Some(Pinned::OpaqueSpAdjustment),
            Some(_) => continue,
            None => {
                at.insert(block, here);
            }
        }
        seen.insert(block);
        if leaves.get(&block).is_some_and(|within| here + within != 0) {
            return Some(Pinned::OpaqueSpAdjustment);
        }
        let Some(Some(effect)) = effects.get(&block) else { continue };
        LOOKED.with(|looked| looked.set(looked.get() + 1));
        let Some(found) = by_number.get(&block) else { continue };
        for &next in &found.succ {
            pending.push((next, here + effect));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::x87_registers_only;

    /// QB's `fsin`, `fatan` and `fexp2` are bytes the assembler has no
    /// mnemonic for. Any inline code kept the frame register, and a function
    /// that took a sine (qbdemo's PLASMA) could not hold a value in it. A byte
    /// string of x87 register operations touches no stack cell; one with a
    /// memory operand (`fld dword ptr [esp]`, ModRM below C0) or other code
    /// may.
    #[test]
    fn test_x87_register_operations_touch_no_stack_cell() {
        assert!(x87_registers_only(&[0xd9, 0xfe]));
        assert!(x87_registers_only(&[0xd9, 0xe8, 0xd9, 0xf3]));
        assert!(!x87_registers_only(&[0xd9, 0x04, 0x24]), "fld dword ptr [esp]");
        assert!(!x87_registers_only(&[0xd9, 0x44, 0x24, 0x04]), "fld dword ptr [esp+4]");
        assert!(!x87_registers_only(&[0x90, 0x90]), "nops are not x87");
        assert!(!x87_registers_only(&[0xd9]), "half an instruction");
    }
}
