//! The m68k target (classic Mac, Amiga, Atari ST): spike, translated from
//! LLVM's M68k backend.

pub mod abi;
pub mod addrarith;
pub mod amiga;
pub mod atari;
pub mod ccr;
pub mod chain;
pub mod cmpelim;
pub mod compile;
pub mod dagisel;
pub mod divide;
pub mod driver;
pub mod elf;
pub mod encode;
pub mod expand;
pub mod flags;
pub mod foldload;
pub mod frame;
pub mod isa;
pub mod kernels;
pub mod mac;
pub mod machine;
pub mod mc;
pub mod mfunc;
pub mod narrow;
pub mod negadd;
pub mod object;
pub mod regalloc;
pub mod target;
pub mod timing;

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
