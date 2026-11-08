//! The m68k target (classic Mac, Amiga, Atari ST): spike, translated from
//! LLVM's M68k backend.

pub mod abi;
pub mod addrarith;
pub mod addrcmp;
pub mod addrroles;
pub mod amiga;
pub mod atari;
pub mod callpush;
pub mod calltarget;
pub mod ccr;
pub mod chain;
pub mod cmpdup;
pub mod cmpelim;
pub mod cmpshift;
pub mod commutative;
pub mod compile;
pub mod constreg;
pub mod consts;
pub mod dagisel;
pub mod data;
pub mod divide;
pub mod divrem;
pub mod driver;
pub mod elf;
pub mod encode;
pub mod expand;
pub mod flags;
pub mod foldarg;
pub mod foldload;
pub mod frame;
pub mod isa;
pub mod kernels;
pub mod loopinc;
pub mod mac;
pub mod machine;
pub mod mc;
pub mod mfunc;
pub mod mulprice;
pub mod narrow;
pub mod narrow_add;
pub mod negadd;
pub mod object;
pub mod opcosts;
pub mod regalloc;
pub mod scaledindex;
pub mod shiftadd;
pub mod sizes;
pub mod spillmove;
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
