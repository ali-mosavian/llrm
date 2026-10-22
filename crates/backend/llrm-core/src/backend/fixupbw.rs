//! A byte or word load, copy or extension is the dword form when the rest of
//! its register is dead: `X86FixupBWInsts`. Translated from
//! [X86FixupBWInsts.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L165)
//! (`runOnMachineFunction`), function by function:
//! [`getSuperRegDestIfDead`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L190)
//! is `super_reg_dest_if_dead`,
//! [`tryReplaceLoad`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L285),
//! [`tryReplaceCopy`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L316)
//! and
//! [`tryReplaceExtend`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L353)
//! keep their names,
//! [`tryReplaceInstr`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L388)
//! chooses by opcode, and
//! [`processBasicBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupBWInsts.cpp#L434)
//! decides every instruction of a block against the liveness of the block as it
//! was, then replaces them.
//!
//! # How it works
//! After allocation, `mov al,[m]` becomes `movzx eax,[m]`, `mov ax,[m]`
//! `movzx eax,[m]`, `mov al,cl` `mov eax,ecx`, and `movsx`/`movzx` to a word
//! the dword form, when no other byte of the destination's dword register is
//! live afterwards. A zero-extending load needs no merge with the old upper
//! bytes and a dword copy no partial-register write. `ah` and the other high
//! views are never the destination; a byte load is left alone when the code is
//! for size (`OptForSize`, L455).
//!
//! DEVIATE W1 (no number yet): in real mode the dword form takes the 0x66
//! prefix, so the pin's claim that the size is the same is false there; the
//! rewrite is made only if it is no longer. Exit: none (a fact of the
//! encoding). DEVIATE W3 (no number yet): the pin refuses `movsx ax,al` only as
//! the registers AX and AL; here a `movsx` within one register is refused
//! whatever the registers. Owner w-5.
//!
//! # Runs
//! After `Peephole`, optimizing levels only.

use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::liveness;
use crate::backend::peep::guards::{free_fields, short_in_real_mode_at};
use crate::backend::peep::{field, make};
use crate::backend::peephole::{_lanes, id};
use crate::backend::regthrash::_dead_after;
use crate::model::ir::{self, Loc, Operation, Reg};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

/// What an instruction may carry and still widen: the requirement of a
/// register (`widening`), or of a load's segment (`widening_load`).
const WIDENING: u32 = field::CLOBBERS
    | field::CLOBBERS_HIGH
    | field::REQUIRES
    | field::DELIVERS
    | field::SPREAD
    | field::GROUP
    | field::FRAME_ADJUST
    | field::SPILL_RELOAD
    | field::SPILL_STORE;
const WIDENING_LOAD: u32 = WIDENING & !field::REQUIRES;

pub struct FixupBw {
    /// The code is for size (`shouldOptimizeForSize`).
    pub size: bool,
}

impl LIRTransform for FixupBw {
    fn class_name(&self) -> &'static str {
        "FixupBW"
    }

    fn name(&self) -> &str {
        "fixupbw"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(self.fixed(&body))
    }
}

impl FixupBw {
    /// `runOnMachineFunction`: every block.
    fn fixed(
        &self,
        body: &LirBody,
    ) -> LirBody {
        let (regs, bits) = (body.regs(), body.bits);
        let exits = liveness::dead_at_exit(body);
        let mut out = body.clone();
        for block in &mut out.blocks {
            let dead = _dead_after(regs, bits, block, exits[&block.at].clone());
            let insns: Vec<Arc<Insn>> = self.process_basic_block(regs, bits, block.insns.iter(), &dead);
            *block = block.with_insns(insns);
        }
        out
    }

    /// `processBasicBlock`: the replacements are decided against the block as
    /// it was and made after.
    fn process_basic_block<'a>(
        &self,
        regs: Regs,
        bits: u32,
        insns: impl Iterator<Item = &'a Arc<Insn>>,
        dead: &crate::backend::peephole::DeadAfter,
    ) -> Vec<Arc<Insn>> {
        insns
            .map(|one| match self.try_replace_instr(regs, bits, one, &dead[&id(one)]) {
                Some(new) => Arc::new(new),
                None => Arc::clone(one),
            })
            .collect()
    }

    /// `tryReplaceInstr`: the form chosen by the opcode.
    fn try_replace_instr(
        &self,
        regs: Regs,
        bits: u32,
        one: &Arc<Insn>,
        dead: &crate::backend::peephole::Lanes,
    ) -> Option<Insn> {
        let what = one.what.as_ref()?;
        let name = what.name.as_deref()?;
        let ([Loc::Reg(dest)], [source]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
        match (name, dest.width, source) {
            // MOV8rm: not for size, the extending form takes a byte more.
            ("mov", 1, Loc::Mem(_)) if !self.size => self.try_replace_load(regs, bits, one, *dest, dead),
            // MOV16rm
            ("mov", 2, Loc::Mem(_)) => self.try_replace_load(regs, bits, one, *dest, dead),
            // MOV8rr, MOV16rr
            ("mov", 1 | 2, Loc::Reg(src)) if src.width == dest.width => {
                self.try_replace_copy(regs, bits, one, *dest, *src, dead)
            }
            // MOVSX16rr8, MOVZX16rr8
            ("movsx" | "movzx", 2, Loc::Reg(src)) if src.width == 1 => {
                self.try_replace_extend(regs, bits, one, *dest, Some(*src), dead)
            }
            // MOVSX16rm8, MOVZX16rm8
            ("movsx" | "movzx", 2, Loc::Mem(_)) => self.try_replace_extend(regs, bits, one, *dest, None, dead),
            _ => None,
        }
    }

    /// `getSuperRegDestIfDead`: the dword register of `dest` if no other byte
    /// of it is live after `one`.
    fn super_reg_dest_if_dead(
        regs: Regs,
        dest: Reg,
        dead: &crate::backend::peephole::Lanes,
    ) -> Option<Reg> {
        if regs.is_segment(dest.register) {
            return None;
        }
        // The destination is the lowest part of its super register (not `ah`).
        if !_lanes(regs, dest.register).iter().all(|lane| lane.1 < dest.width as u32) {
            return None;
        }
        make::rest_of_root(regs, dest)
            .is_subset(dead)
            .then(
                || {
                    let Loc::Reg(wide) = make::dword_of(regs, dest) else { unreachable!("a register") };
                    wide
                },
            )
    }

    /// `tryReplaceLoad`: `mov` from memory to `movzx` into the dword register.
    fn try_replace_load(
        &self,
        regs: Regs,
        bits: u32,
        one: &Arc<Insn>,
        dest: Reg,
        dead: &crate::backend::peephole::Lanes,
    ) -> Option<Insn> {
        let wide = Self::super_reg_dest_if_dead(regs, dest, dead)?;
        let new = rebuilt(one, "movzx", Operation::Extend, wide, None);
        (free_fields(one, WIDENING_LOAD) && short_in_real_mode_at(bits, one, &new)).then_some(new)
    }

    /// `tryReplaceCopy`: a byte or word copy is the dword copy.
    fn try_replace_copy(
        &self,
        regs: Regs,
        bits: u32,
        one: &Arc<Insn>,
        dest: Reg,
        src: Reg,
        dead: &crate::backend::peephole::Lanes,
    ) -> Option<Insn> {
        let wide = Self::super_reg_dest_if_dead(regs, dest, dead)?;
        // The source must be the same part of its register as the destination
        // is of its own: `movb %ah, %al` is not `movl %eax, %eax`.
        if regs.is_segment(src.register) || !_lanes(regs, src.register).iter().all(|lane| lane.1 < src.width as u32) {
            return None;
        }
        let Loc::Reg(wide_src) = make::dword_of(regs, src) else { unreachable!("a register") };
        let new = rebuilt(one, "mov", Operation::Move, wide, Some(Loc::Reg(wide_src)));
        (free_fields(one, WIDENING) && short_in_real_mode_at(bits, one, &new)).then_some(new)
    }

    /// `tryReplaceExtend`: a byte extended to a word is extended to a dword.
    fn try_replace_extend(
        &self,
        regs: Regs,
        bits: u32,
        one: &Arc<Insn>,
        dest: Reg,
        src: Option<Reg>,
        dead: &crate::backend::peephole::Lanes,
    ) -> Option<Insn> {
        let wide = Self::super_reg_dest_if_dead(regs, dest, dead)?;
        let name = one.what.as_ref()?.name.clone()?;
        if let Some(src) = src {
            // Do not interfere with the formation of `cbw` (DEVIATE W3: any
            // extension within one register).
            if regs.is_segment(src.register)
                || !_lanes(regs, src.register).iter().all(|lane| lane.1 < src.width as u32)
                || (name == "movsx" && ir::root(src.register) == ir::root(dest.register))
            {
                return None;
            }
        }
        let new = rebuilt(one, &name, Operation::Extend, wide, None);
        let fields = if src.is_some() { WIDENING } else { WIDENING_LOAD };
        (free_fields(one, fields) && short_in_real_mode_at(bits, one, &new)).then_some(new)
    }
}

/// `one` with the opcode `name`, the destination `wide` and, if given, the
/// source `source`.
fn rebuilt(
    one: &Arc<Insn>,
    name: &str,
    op: Operation,
    wide: Reg,
    source: Option<Loc>,
) -> Insn {
    let mut new = (**one).clone();
    let what = new.what.as_mut().expect("semantics");
    what.name = Some(name.to_owned());
    what.op = op;
    what.dests = vec![Loc::Reg(wide)];
    if let Some(source) = source {
        what.sources = vec![source];
    }
    new
}
