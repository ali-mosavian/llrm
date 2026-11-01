//! The Z80 target, a sibling of the 6502 and m68k spikes.

pub mod abi;
pub mod dagisel;
pub mod driver;
pub mod prepare;
pub mod regalloc;
pub mod target;
pub mod timing;
pub mod tti;

use llrm_lir::registers::{RegId, Row};

/// `registers.regs`: A, the pairs BC DE HL IX IY, SP and the bytes B C D E H L.
pub const REGISTERS: &str = include_str!("registers.regs");
/// `machines/datalayout.toml`: 8-bit native, 16-bit pointers.
pub const LAYOUT: &str = include_str!("machines/datalayout.toml");
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
