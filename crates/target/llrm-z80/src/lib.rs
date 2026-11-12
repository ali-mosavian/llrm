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
pub mod clocks;
pub mod commute;
pub mod ldir;
pub mod long_shift;
pub mod mfunc;
pub mod narrow;
pub mod post_update;
pub mod prepare;
pub mod regalloc;
pub mod runtime_size;
pub mod sp_frame;
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

/// FNV-1a (64 bits) of [`RUNTIME`], as 16 hex digits: `llrm-z80-llc --runtime-stamp`. A sweep links the runtime from
/// the working tree, not from the compiler, so it compares this with the file it links (`tests/programs/run.py`).
pub fn runtime_stamp() -> String {
    let hash = RUNTIME.bytes().fold(0xcbf29ce484222325u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3));
    format!("{hash:016x}")
}

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

#[cfg(test)]
mod runtime_stamp_tests {
    /// `--runtime-stamp` is what `tests/programs/run.py` compares with the file it links: FNV-1a over the bytes, so a Python
    /// `fnv1a64` agrees (the same vectors are in `test_sweep_guard.py`).
    #[test]
    fn the_runtime_stamp_is_fnv1a_64_of_the_embedded_runtime() {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in super::RUNTIME.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        assert_eq!(super::runtime_stamp(), format!("{hash:016x}"));
        assert_eq!(super::runtime_stamp().len(), 16);
    }
}
