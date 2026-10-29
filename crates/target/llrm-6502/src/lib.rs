//! The 6502 target (C64 first): spike, modelled on LLVM's AVR backend.

pub mod abi;
pub mod dagisel;
pub mod emit;
pub mod frame;
pub mod peep;
pub mod platform;
pub mod select;
pub mod target;

use llrm_lir::registers::{RegId, Row};

/// `registers.regs`: A, X, Y and the zero-page bytes used as registers.
pub const REGISTERS: &str = include_str!("registers.regs");
/// `machines/datalayout.toml`: 8-bit native, 16-bit pointers.
pub const LAYOUT: &str = include_str!("machines/datalayout.toml");
/// `machines/calling.toml`: arguments in rc2.., results in A:X.
pub const CALLING: &str = include_str!("machines/calling.toml");

/// `runtime/mos_rt.s`: multiply, divide, remainder and variable shifts, for
/// ca65.
pub const RUNTIME: &str = include_str!("../runtime/mos_rt.s");

/// The register ids, numbered by `registers.regs`'s lines: `reg::RC2`.
pub mod reg {
    use super::RegId;
    include!(concat!(env!("OUT_DIR"), "/register_ids.rs"));
}

/// Instruction effect rows: none until the 6502's forms are described.
fn rows(
    _name: &str,
    _dests: usize,
    _sources: usize,
) -> &'static [Row] {
    &[]
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));
