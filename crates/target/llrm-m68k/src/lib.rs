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
pub mod classcheck;
pub mod classsplit;
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
pub mod frameindexed;
pub mod isa;
pub mod kernels;
pub mod libcalls;
pub mod loopinc;
pub mod lowering;
pub mod mac;
pub mod machine;
pub mod mc;
pub mod mfunc;
pub mod narrow;
pub mod narrow_add;
pub mod negadd;
pub mod object;
pub mod opcosts;
pub mod pccall;
pub mod regalloc;
pub mod scaledindex;
pub mod sextacc;
pub mod shiftadd;
pub mod routine;
pub mod sizes;
pub mod spillmove;
pub mod target;
pub mod timing;
pub mod trial;

use llrm_lir::registers::{RegId, Row};

/// The register ids, numbered by `registers.regs`'s lines: `reg::D0`.
pub mod reg {
    use super::RegId;
    include!(concat!(env!("OUT_DIR"), "/register_ids.rs"));
}

/// The row of an instruction whose first source is its destination's register
/// (`@ties rows`): what the pin's description says (`OperandDesc::tied_to`),
/// not x86's rule that an `Add` is tied. `LEA32p d, #n, b` is not.
const TIED: [Row; 1] =
    [Row { reads: &[], writes: &[], kinds: &[], width: 0, ties: &[(0, 0)], pins: &[], flags_read: 0, flags_written: 0 }];

/// Instruction effect rows: the tie of each instruction the description ties,
/// nothing else until m68k's other effects are described.
fn rows(
    name: &str,
    _dests: usize,
    _sources: usize,
) -> &'static [Row] {
    static TIED_NAMES: std::sync::OnceLock<std::collections::HashSet<&'static str>> = std::sync::OnceLock::new();
    let tied = TIED_NAMES.get_or_init(|| {
        (0..=u32::from(u16::MAX))
            .filter_map(|code| {
                let name = dagisel::local::name_of(code)?;
                let desc = dagisel::local::desc(code as u16)?;
                let first_source = usize::from(desc.num_defs);
                desc.operands.get(first_source).is_some_and(|operand| operand.tied_to == 0).then_some(name)
            })
            .collect()
    });
    if tied.contains(name) { &TIED } else { &[] }
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));
