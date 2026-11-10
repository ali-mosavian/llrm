//! The Z80 target, a sibling of the 6502 and m68k spikes.

pub mod abi;
pub mod compile;
pub mod dagisel;
pub mod data;
pub mod driver;
pub mod expand;
pub mod forward;
pub mod frequency;
pub mod inline_rt;
pub mod bump;
pub mod carry;
pub mod commute;
pub mod ldir;
pub mod mfunc;
pub mod narrow;
pub mod prepare;
pub mod regalloc;
pub mod runtime_size;
pub mod spilled_args;
pub mod step;
pub mod target;
pub mod timing;
pub mod tti;

use llrm_lir::registers::{RegId, Row};

/// `registers.regs`: A, the pairs BC DE HL IX IY, SP and the bytes B C D E H L.
pub const REGISTERS: &str = include_str!("registers.regs");
/// `machines/datalayout.toml`: 8-bit native, 16-bit pointers.
pub const LAYOUT: &str = include_str!("machines/datalayout.toml");

/// The alignment of a stack object the frames keep: the datalayout's `S8`, one byte. Nothing realigns SP.
pub const KEPT_STACK_ALIGN: u64 = 1;

/// The largest alignment an `alloca` may ask (the widest scalar, 8 bytes): the Z80 has no alignment
/// requirement, so one asking up to this is clamped to `KEPT_STACK_ALIGN`; more is refused by name.
pub const MAX_ALLOCA_ALIGN: u64 = 8;
/// `machines/calling.toml`: SDCC's register convention.
pub const CALLING: &str = include_str!("machines/calling.toml");

/// `runtime/z80_rt.s`: multiply, divide, remainder and shifts by a variable, in
/// sdasz80 syntax.
pub const RUNTIME: &str = include_str!("../runtime/z80_rt.s");

/// The register ids, numbered by `registers.regs`'s lines: `reg::HL`.
pub mod reg {
    use super::RegId;
    include!(concat!(env!("OUT_DIR"), "/register_ids.rs"));
}

/// Instruction effect rows: none until the Z80's forms are described.
fn rows(
    _name: &str,
    _dests: usize,
    _sources: usize,
) -> &'static [Row] {
    &[]
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));

#[cfg(test)]
mod stack_natural_alignment_tests {
    /// The z80 datalayout named no stack alignment, so a pass raising an alloca to a type's preferred alignment had no bound.
    #[test]
    fn the_datalayout_states_the_stack_alignment_the_frames_keep() {
        llrm_target::testing::assert_stack_facts(
            "z80",
            super::LAYOUT.lines().find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"')).unwrap(),
            llrm_target::Target::stack_alignment(&super::target::Z80) as u64,
            None,
        );
    }
}
