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
//! The second half of `getSuperRegDestIfDead` (an implicit-def of the super
//! register on a `MOV8rm`, `MOV16rm`, `MOV8rr` or `MOV16rr` whose upper bits
//! were undefined) is `Insn::undef_super`, which `rewriter::rewrite` sets on a
//! narrow definition of a value that is read wider and not live before it: a
//! 32-bit user of the loaded word reads the upper lanes, which `dead_after`
//! counts as live though nothing ever defined them.
//!
//! DEVIATE W1 (no number yet): in real mode the dword form takes the 0x66
//! prefix, so the pin's claim that the size is the same is false there; the
//! rewrite is made only if it is no longer. Exit: none (a fact of the
//! encoding). `movsx ax,al` stays `cbw`, as the pin leaves it.
//!
//! # Example
//! `unsigned char c = *p;` at `-m32 -O2`, where the rest of `eax` is dead:
//!
//! ```text
//! mov al, byte ptr [ebx]        ->    movzx eax, byte ptr [ebx]
//! mov cl, dl                    ->    mov ecx, edx
//! ```
//!
//! From `test_a_byte_load_into_a_dead_register_is_the_zero_extending_load` and
//! `test_a_byte_or_word_copy_is_the_dword_copy_when_the_rest_is_dead` in
//! `tests/fixup_bw.rs`.
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
use crate::model::ir::{Loc, Operation, Reg};
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
        undef_super: bool,
    ) -> Option<Reg> {
        if regs.is_segment(dest.register) {
            return None;
        }
        // The destination is the lowest part of its super register (not `ah`).
        if !_lanes(regs, dest.register).iter().all(|lane| lane.1 < dest.width as u32) {
            return None;
        }
        // The rest of the register is live afterwards but was not before: the
        // `implicit-def` of the super register on a `MOV8rm`, `MOV16rm`,
        // `MOV8rr` or `MOV16rr` with no implicit use (L254).
        (undef_super || make::rest_of_root(regs, dest).is_subset(dead)).then(|| {
            let Loc::Reg(wide) = make::dword_of(regs, dest) else { unreachable!("a register") };
            wide
        })
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
        let wide = Self::super_reg_dest_if_dead(regs, dest, dead, one.undef_super)?;
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
        let wide = Self::super_reg_dest_if_dead(regs, dest, dead, one.undef_super)?;
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
        let wide = Self::super_reg_dest_if_dead(regs, dest, dead, false)?;
        let name = one.what.as_ref()?.name.clone()?;
        // Don't interfere with the formation of `cbw`, a shorter encoding than
        // even the `movsx` to a dword.
        if &*name == "movsx"
            && dest.register == llrm_lir::registers::RegId::AX
            && src.is_some_and(|src| src.register == llrm_lir::registers::RegId::AL)
        {
            return None;
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
    what.name = Some(name.into());
    what.op = op;
    what.dests = vec![Loc::Reg(wide)];
    if let Some(source) = source {
        what.sources = vec![source];
    }
    new
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::*;
    use crate::model::ir::Semantics;
    use crate::model::lir::{Insn, LirBlock};
    use crate::support::hash::IndexMap;

    fn reg(
        register: RegId,
        width: u32,
    ) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    /// `movsx dest, src` alone in a body, `dest`'s register dead after it.
    fn extended(
        dest: Loc,
        src: Loc,
    ) -> (String, Loc) {
        let what = Semantics {
            name: Some("movsx".into()),
            dests: vec![dest],
            sources: vec![src],
            ..Semantics::new(Operation::Extend)
        };
        let insn = Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]));
        let ret = Semantics { name: Some("ret".into()), ..Semantics::new(Operation::Return) };
        let mut ret = Insn::new(1, Some((1, 1)), Some(ret), vec![], vec![]);
        // A return that reads nothing but what it requires and the epilogue's.
        ret.reads_complete = true;
        let ret = Arc::new(ret);
        let mut body = LirBody::new(
            "f",
            0,
            vec![LirBlock::new(0, vec![insn, ret]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.bits = 32;
        let done = FixupBw { size: false }.fixed(&body);
        let one = &done.blocks[0].insns[0];
        let what = one.what.as_ref().expect("semantics");
        (what.name.as_deref().expect("a name").to_owned(), what.dests[0].clone())
    }

    /// The pin leaves only `movsx ax, al` (`cbw`); `movsx cx, cl` is
    /// `movsx ecx, cl`. A `movsx` within one register was refused whatever its
    /// registers.
    #[test]
    fn only_movsx_ax_al_stays_as_cbw() {
        assert_eq!(extended(reg(RegId::AX, 2), reg(RegId::AL, 1)).1, reg(RegId::AX, 2));
        assert_eq!(extended(reg(RegId::CX, 2), reg(RegId::CL, 1)).1, reg(RegId::ECX, 4));
    }

    /// `mov ax, [esi]` then `reader`, whose result nobody reads: the name of
    /// the load after the pass.
    fn load_before(reader: Semantics) -> String {
        let cell = Loc::Mem(crate::model::ir::Mem { through: RegId::ESI, ..crate::model::ir::Mem::new(None, 2) });
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![reg(RegId::AX, 2)],
            sources: vec![cell],
            ..Semantics::new(Operation::Move)
        };
        let load = Arc::new(Insn::new(0, Some((0, 0)), Some(what), vec![], vec![]));
        let read = Arc::new(Insn::new(1, Some((1, 1)), Some(reader), vec![], vec![]));
        let ret = Semantics { name: Some("ret".into()), ..Semantics::new(Operation::Return) };
        let mut ret = Insn::new(2, Some((2, 2)), Some(ret), vec![], vec![]);
        ret.reads_complete = true;
        let mut body = LirBody::new(
            "f",
            0,
            vec![LirBlock::new(0, vec![load, read, Arc::new(ret)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.bits = 32;
        let done = FixupBw { size: false }.fixed(&body);
        done.blocks[0].insns[0].what.as_ref().expect("semantics").name.clone().expect("a name").to_string()
    }

    fn copy(
        dest: Loc,
        source: Loc,
    ) -> Semantics {
        Semantics {
            name: Some("mov".into()),
            dests: vec![dest],
            sources: vec![source],
            ..Semantics::new(Operation::Move)
        }
    }

    /// `mov ax,[esi]; mov ebx,eax`: the dword copy reads the upper half of EAX,
    /// which the load leaves as it was. Made `movzx eax,[esi]`, the copy read
    /// zeros.
    #[test]
    fn a_word_load_stays_where_the_dword_is_read_after_it() {
        assert_eq!(load_before(copy(reg(RegId::EBX, 4), reg(RegId::EAX, 4))), "mov");
        assert_eq!(load_before(copy(reg(RegId::BX, 2), reg(RegId::AX, 2))), "movzx");
    }
}
