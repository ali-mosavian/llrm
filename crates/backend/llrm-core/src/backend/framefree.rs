//! Decides, before allocation, whether a function can do without its frame
//! register, which is then one more value register.
//!
//! # How it works
//! This is LLVM's `hasFP`. The frame register is free when `masm` can address
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
//! inline assembly, a landing pad, `returns_twice`, or debug variables not
//! located by the CFA. It is kept where the frame register is read or written
//! as a value, or is a base or index of anything but a frame cell; the stack
//! pointer is written other than by `add`/`sub` with an immediate; there is an
//! indirect call, jump or branch, a `leave`, an escape or a barrier other than
//! `fnstcw`, `fldcw`, `fnstsw`, `in` and `out`, a push or pop of a width other
//! than 2 or 4, or two edges disagree on the depth.
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
use llrm_lir::registers::Regs;

use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};

/// The registers that are the frame register as LIR names it.
fn is_frame(
    register: RegId,
    pointer: RegId,
) -> bool {
    register != RegId::None && crate::model::ir::root(register) == crate::model::ir::root(pointer)
}

/// Whether the function described by `body` (its calls' popped bytes in `pops`)
/// can have no frame register.
pub fn without_frame_register(
    body: &LirBody,
    registers: &llrm_target::FrameRegisters,
    pops: &BTreeMap<i64, i64>,
    inline: bool,
    far: bool,
    landing: bool,
) -> bool {
    let regs = body.regs();
    let pointer = registers.pointer;
    if !registers.optional
        || body.bits != 32
        || far
        || inline
        || landing
        || body.returns_twice
        || (!body.variables.is_empty() && !body.cfa_variables)
    {
        return false;
    }
    // The depth at the top of each block, and each block's effect.
    let mut effects: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    // The depth within a block at which it returns.
    let mut leaves: BTreeMap<i64, i64> = BTreeMap::new();
    for block in &body.blocks {
        let mut depth = 0i64;
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            for place in what.dests.iter().chain(&what.sources) {
                match place {
                    Loc::Reg(reg) if is_frame(reg.register, pointer) => return false,
                    Loc::Mem(cell) if is_frame(cell.index_through, pointer) => return false,
                    Loc::Address(address) if is_frame(address.index_through, pointer) => return false,
                    // A register-based cell is fine; the frame register as a
                    // base is only a frame cell.
                    Loc::Mem(cell) if is_frame(cell.through, pointer) && !cell.in_frame() => return false,
                    Loc::Address(address)
                        if is_frame(address.through, pointer)
                            && address
                                .addr
                                .as_ref()
                                .is_none_or(|addr| addr.space != crate::model::ir::Space::Frame) =>
                    {
                        return false;
                    }
                    _ => {}
                }
            }
            match what.op {
                Operation::Push | Operation::Pop => {
                    let (places, sign) = if what.op == Operation::Push { (&what.sources, 1) } else { (&what.dests, -1) };
                    let [place] = &places[..] else { return false };
                    let width = match place {
                        Loc::Reg(reg) => reg.register.size() as i64,
                        Loc::Imm(imm) => i64::from(imm.width),
                        Loc::Mem(cell) => i64::from(cell.width),
                        _ => return false,
                    };
                    if width != 2 && width != 4 {
                        return false;
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
                Operation::Leave | Operation::Exchange | Operation::Escape | Operation::Barrier => return false,
                Operation::Nothing if what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")) => return false,
                Operation::Call => {
                    if what.indirect {
                        return false;
                    }
                    depth -= pops.get(&one.at).copied().unwrap_or(0);
                }
                Operation::Jump | Operation::Branch if what.indirect => return false,
                Operation::Return => leaves.insert(block.at, depth).map_or((), |_| ()),
                _ if what.dests.iter().any(|place| matches!(
                    place,
                    Loc::Reg(reg) if crate::model::ir::root(reg.register) == crate::model::ir::root(registers.stack)
                )) => {
                    // Only `add sp, n` and `sub sp, n` are understood.
                    let amount = match (&what.dests[..], &what.sources[..]) {
                        ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(imm)]) if dest.register == source.register && imm.address.is_none() => imm.value,
                        _ => return false,
                    };
                    match what.name.as_deref() {
                        Some("add") => depth -= amount,
                        Some("sub") => depth += amount,
                        _ => return false,
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
    while let Some((block, here)) = pending.pop() {
        match at.get(&block) {
            Some(&there) if there != here => return false,
            Some(_) => continue,
            None => {
                at.insert(block, here);
            }
        }
        seen.insert(block);
        if leaves.get(&block).is_some_and(|within| here + within != 0) {
            return false;
        }
        let Some(Some(effect)) = effects.get(&block) else { continue };
        let Some(found) = body.blocks.iter().find(|one| one.at == block) else { continue };
        for &next in &found.succ {
            pending.push((next, here + effect));
        }
    }
    llrm_support::debug!("frame", "{}: freed", body.name);
    true
}

/// Whether `one` needs the frame register to hold the frame: it names a frame
/// cell, the frame register or the stack pointer, or it is a return, a call,
/// or an instruction that may trap (a runtime built on the frame chain walks
/// it from the trap), so no value may be in the frame register across it.
pub fn needs_frame(
    one: &Insn,
    regs: Regs,
) -> bool {
    *one.frame.get_or_init(|| _needs_frame(one, regs))
}

fn _needs_frame(
    one: &Insn,
    regs: Regs,
) -> bool {
    let Some(what) = &one.what else { return true };
    if matches!(
        what.op,
        Operation::Call
            | Operation::Return
            | Operation::Leave
            | Operation::Divide
            | Operation::Escape
            | Operation::Barrier
            | Operation::Data
            | Operation::Restore
            | Operation::FloatLoad
            | Operation::FloatStore
            | Operation::FloatArith
            | Operation::FloatArithPop
            | Operation::FloatUnary
    ) || what.name.as_deref().is_some_and(|name| ["int", "into", "iret", "enter", "bound"].contains(&name))
        || one.clobbers.iter().chain(&one.clobbers_high).any(|register| regs.is_frame(*register))
    {
        return true;
    }
    let named = |register: RegId| register != RegId::None && (regs.is_frame(register) || regs.is_stack(register));
    what.dests
        .iter()
        .chain(&what.sources)
        .any(
            |place| place.in_frame()
                || match place {
                    Loc::Reg(reg) => named(reg.register),
                    Loc::Mem(cell) => {
                        named(cell.through)
                            || named(cell.index_through)
                            || cell.addr.is_some_and(|addr| addr.space == crate::model::ir::Space::Stack)
                    }
                    Loc::Address(address) => named(address.through) || named(address.index_through),
                    _ => false,
                },
        )
}

/// Whether a function may give its frame register to a value where nothing
/// touches the frame: it has no inline code, no landing pad, no second return
/// and is no interrupt handler (which may interrupt anything that reads the
/// chain of frames).
pub fn parkable(
    body: &LirBody,
    inline: bool,
    landing: bool,
    interrupt: bool,
) -> bool {
    !(inline || landing || interrupt || body.returns_twice)
}

/// What a body's parking must keep to (`LLRM_CHECK_PARKED`): the frame register
/// is saved on every way into a stretch and restored on every way out of it, no
/// instruction in a stretch needs the frame, and no return is inside one.
pub fn parking_holds(body: &LirBody) -> Result<(), String> {
    use std::collections::BTreeMap;
    let regs = body.regs();
    let parking = |one: &Insn, op: Operation| {
        one.frame_adjust
            && one.what.as_ref().is_some_and(|what| {
                what.op == op
                    && what.dests.iter().chain(&what.sources).any(|place| {
                        matches!(
                            place,
                            Loc::Reg(reg) if reg.register != RegId::None && regs.is_frame(reg.register)
                        )
                    })
            })
    };
    let mut at: BTreeMap<i64, bool> = BTreeMap::new();
    let mut pending = vec![(body.entry, false)];
    while let Some((block, mut parked)) = pending.pop() {
        // The instruction after a push may read the frame as it makes the value
        // that takes the register: it reads before it writes.
        let mut fresh = false;
        match at.get(&block) {
            Some(&there) if there != parked => {
                return Err(format!("{}: block {block} is entered both parked and not", body.name));
            }
            Some(_) => continue,
            None => {
                at.insert(block, parked);
            }
        }
        let Some(found) = body.blocks.iter().find(|one| one.at == block) else { continue };
        for one in &found.insns {
            if parking(one, Operation::Push) {
                if parked {
                    return Err(format!("{}: the frame register is saved twice in block {block}", body.name));
                }
                parked = true;
                fresh = true;
                continue;
            } else if parking(one, Operation::Pop) {
                if !parked {
                    return Err(format!("{}: the frame register is restored unsaved in block {block}", body.name));
                }
                parked = false;
            } else if parked && !fresh && needs_frame(one, regs) {
                return Err(format!(
                    "{}: an instruction at {} needs the frame inside a parked stretch",
                    body.name, one.at
                ));
            }
            fresh = false;
        }
        for next in &found.succ {
            pending.push((*next, parked));
        }
    }
    Ok(())
}
