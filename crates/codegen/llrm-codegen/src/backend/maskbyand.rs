//! `movzx r32, r8|r16` of a register into itself as `and r32, mask`, where the
//! CPU prices the extension above the `and` and the block runs often enough
//! to pay the extra bytes.
//!
//! # How it works
//! The pin's table selects `(and x, 0xff)` as `MOVZX32rr8` with no CPU gate
//! (X86InstrCompiler.td#L1773-L1777): its CPUs extend in one clock. The 486
//! and the Pentium take three clocks for it and one for `and r,imm`, which is
//! two to three bytes longer. After allocation the block frequency is known,
//! so the rewrite is made where `frequency x clocks saved` is at least `rate x
//! bytes added` (the target's speed rate; none at -O1 and for size).
//!
//! Only `movzx r,low(r)` is rewritten: the `and` is two-address, so any other
//! source would cost a `mov`. The flags the `and` writes must be dead.
//!
//! DEVIATE MA1 (owner x86-m32; exit: the rule moves into the generated table
//! when it takes a block-frequency predicate; review 2026-12-10). Replaces
//! the instruction selector's SD460, which could not see how often a block
//! runs: cold sites paid bytes for no clocks.
//!
//! # Example
//! `movzx eax, al` in a loop body of frequency 8: `and eax, 255`.
//!
//! # Runs
//! After `FixupBW`, at -O2 and -O3.

use std::sync::Arc;

use iced_x86::RflagsBits;

use crate::backend::lanes::Lanes;
use crate::backend::liveness;
use crate::backend::peep::guards::free_fields;
use crate::backend::peep::{field, make};
use crate::backend::peephole::id;
use crate::backend::regthrash::_dead_after;
use crate::liveness::blockfrequency::MachineBlockFrequencyInfo;
use crate::model::ir::{Imm, Loc, Operation, Reg};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

/// What an instruction may carry and still be replaced.
const REPLACEABLE: u32 = field::CLOBBERS
    | field::CLOBBERS_HIGH
    | field::REQUIRES
    | field::DELIVERS
    | field::SPREAD
    | field::GROUP
    | field::FRAME_ADJUST
    | field::SPILL_RELOAD
    | field::SPILL_STORE;

/// The prices of the rewrite, from the CPU's table.
pub struct MaskByAnd {
    /// Clocks the `and` saves over the `movzx`.
    pub clocks_saved: i64,
    /// Thousandths of a clock a byte of code is worth (`speed_milliclocks_per_byte`).
    pub milliclocks_per_byte: i64,
}

impl MaskByAnd {
    /// Whether a site executed `frequency` times per entry pays for `bytes`
    /// more code.
    fn pays(
        &self,
        frequency: f64,
        bytes: i64,
    ) -> bool {
        self.clocks_saved > 0
            && frequency * self.clocks_saved as f64 * 1000.0 >= (self.milliclocks_per_byte * bytes) as f64
    }
}

impl LIRTransform for MaskByAnd {
    fn class_name(&self) -> &'static str {
        "MaskByAnd"
    }

    fn name(&self) -> &str {
        "maskbyand"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if self.milliclocks_per_byte <= 0 || self.clocks_saved <= 0 {
            return Ok(body);
        }
        let (regs, bits) = (body.regs(), body.bits);
        let exits = liveness::dead_at_exit(&body);
        let frequencies = MachineBlockFrequencyInfo::of(&body);
        let flags = Lanes::flags(
            RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF,
        );
        let mut out = body.clone();
        for (index, block) in out.blocks.iter_mut().enumerate() {
            let frequency = frequencies.get_block_freq_relative_to_entry_block(index);
            let dead = _dead_after(regs, bits, block, exits[&block.at].clone());
            let insns = block
                .insns
                .iter()
                .map(|one| {
                    let Some(mask) = extension_into_itself(regs, one) else { return Arc::clone(one) };
                    let bytes = if matches!(mask.0.register, llrm_lir::registers::RegId::EAX) { 2 } else { 3 };
                    let after = &dead[&id(one)];
                    if bits != 32 || !flags.is_subset(after) || !free_fields(one, REPLACEABLE) || !self.pays(frequency, bytes)
                    {
                        return Arc::clone(one);
                    }
                    let mut new = (**one).clone();
                    let what = new.what.as_mut().expect("semantics");
                    what.name = Some("and".into());
                    what.op = Operation::Binary;
                    what.dests = vec![Loc::Reg(mask.0)];
                    what.sources = vec![Loc::Reg(mask.0), Loc::Imm(Imm { value: mask.1, width: 4, address: None })];
                    Arc::new(new)
                })
                .collect();
            *block = block.with_insns(insns);
        }
        Ok(out)
    }
}

/// The destination and the mask of `movzx r32, low byte or word of r32`.
fn extension_into_itself(
    regs: llrm_lir::registers::Regs,
    one: &Arc<Insn>,
) -> Option<(Reg, i64)> {
    let what = one.what.as_ref()?;
    if what.name.as_deref() != Some("movzx") {
        return None;
    }
    let ([Loc::Reg(dest)], [Loc::Reg(src)]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    if dest.width != 4 || !matches!(src.width, 1 | 2) {
        return None;
    }
    let Loc::Reg(wide) = make::dword_of(regs, *src) else { return None };
    // A low view only: `ah` is not the low byte of `eax`.
    let low = crate::backend::peephole::_lanes(regs, src.register).iter().all(|lane| lane.1 < src.width as u32);
    (wide.register == dest.register && low).then_some((*dest, if src.width == 1 { 0xff } else { 0xffff }))
}
