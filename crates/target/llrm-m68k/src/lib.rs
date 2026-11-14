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
pub mod cmpcarry;
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
pub mod moveqremat;
pub mod narrow;
pub mod narrow_add;
pub mod negadd;
pub mod object;
pub mod opcosts;
pub mod pccall;
pub mod regalloc;
pub mod routine;
pub mod scaledindex;
pub mod sextacc;
pub mod shiftadd;
pub mod wordtable;
pub mod sizes;
pub mod slotreg;
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

/// Instruction effect rows (`@ties rows`): the pairs of source and dest an
/// instruction's description ties (`OperandDesc::tied_to`), not x86's rule that
/// an `Add` is tied. `LEA32p d, #n, b` is not; a write-back form such as
/// `ADD16do_WB` ties two, its data and its address.
fn rows(
    name: &str,
    _dests: usize,
    _sources: usize,
) -> &'static [Row] {
    static TIED: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static Row>> =
        std::sync::OnceLock::new();
    let tied = TIED.get_or_init(|| {
        (0..=u32::from(u16::MAX))
            .filter_map(|code| {
                let name = dagisel::local::name_of(code)?;
                let desc = dagisel::local::desc(code as u16)?;
                let defs = usize::from(desc.num_defs);
                let pairs: Vec<(usize, usize)> = desc
                    .operands
                    .iter()
                    .skip(defs)
                    .enumerate()
                    .filter_map(|(source, operand)| usize::try_from(operand.tied_to).ok().map(|dest| (source, dest)))
                    .collect();
                if pairs.is_empty() {
                    return None;
                }
                let ties: &'static [(usize, usize)] = Box::leak(pairs.into_boxed_slice());
                let row = Row {
                    reads: &[],
                    writes: &[],
                    kinds: &[],
                    width: 0,
                    ties,
                    pins: &[],
                    flags_read: 0,
                    flags_written: 0,
                };
                Some((name, &*Box::leak(Box::new(row))))
            })
            .collect()
    });
    match tied.get(name) {
        Some(row) => std::slice::from_ref(row),
        None => &[],
    }
}

include!(concat!(env!("OUT_DIR"), "/register_info.rs"));
