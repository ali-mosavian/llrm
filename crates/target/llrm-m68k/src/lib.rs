//! The m68k target (classic Mac, Amiga, Atari ST): spike, translated from
//! LLVM's M68k backend.

pub mod abi;
pub mod amiga;
pub mod atari;
pub mod compile;
pub mod dagisel;
pub mod driver;
pub mod encode;
pub mod expand;
pub mod flags;
pub mod frame;
pub mod kernels;
pub mod mac;
pub mod machine;
pub mod mc;
pub mod mfunc;
pub mod object;
pub mod overlay;
pub mod regalloc;
pub mod target;

use llrm_lir::registers::{RegId, Row};

/// The register ids, numbered by `registers.regs`'s lines: `reg::D0`.
pub mod reg {
    use super::RegId;
    include!(concat!(env!("OUT_DIR"), "/register_ids.rs"));
}

/// Instruction effect rows: none until m68k's forms are described.
fn rows(
    _name: &str,
    _dests: usize,
    _sources: usize,
) -> &'static [Row] {
    &[]
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));
