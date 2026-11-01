//! A `tst` of a register that the instruction before it just set the codes
//! for is dropped: `move.l (4,%sp), %d1; tst.l %d1; bne` is `move.l (4,%sp),
//! %d1; bne`.
//!
//! # How it works
//! For each `tst` of a virtual register, [`eliminate_tests`] finds the
//! instruction in the block that defines it with no other setter of the codes
//! between, and the readers of the codes that follow, up to the next setter or
//! the end of the block (the codes do not cross a block before the select
//! diamonds are made). The `tst` goes when the defining instruction leaves
//! every flag those readers test as `tst` of its result would ([`crate::ccr`]),
//! at the same operand size; a reader the pass cannot read keeps it.
//!
//! DEVIATE: the pin has no `optimizeCompareInstr` for m68k and keeps the
//! compare. Reason: best of gcc and clang is the goal, and both reuse the
//! codes of the instruction that made the value. Exit: none.
//!
//! # See also
//! `X86InstrInfo::optimizeCompareInstr`.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::ccr;
use crate::dagisel::local;
use crate::mfunc::MFunction;

/// The mnemonic and operand size letter of the instruction `name`, from its
/// assembly text: `move.l` is `("move", Some('l'))`.
fn mnemonic(name: &str) -> Option<(&'static str, Option<char>)> {
    let text = local::text(name)?.1.split('\t').next()?;
    Some(match text.split_once('.') {
        Some((mnemonic, size)) => (mnemonic, size.chars().next()),
        None => (text, None),
    })
}

/// The register a `tst` of a register reads.
fn tested(inst: &Inst) -> Option<(u32, Option<char>)> {
    let Kind::Op(name) = inst.kind else { return None };
    let (mnemonic, size) = mnemonic(name)?;
    match (mnemonic, &inst.operands[..]) {
        ("tst", [Operand::Use(Reg::Virt(v))]) => Some((*v, size)),
        _ => None,
    }
}

/// The flags a reader of the codes tests, or `None` when it is not one the
/// pass reads: a branch or set on a condition, or a `CMOV`.
fn read_by(inst: &Inst) -> Option<u32> {
    let Kind::Op(name) = inst.kind else { return None };
    if name.starts_with("CMOV") {
        return match inst.operands[..] {
            [.., Operand::Imm(code)] => ccr::tested_by(code),
            _ => None,
        };
    }
    let (mnemonic, _) = mnemonic(name)?;
    let condition = mnemonic.strip_prefix('b').or_else(|| mnemonic.strip_prefix('s'))?;
    ccr::tested_by_name(condition)
}

/// Whether `def` leaves the codes `tst` of its result at `size` would give,
/// for every flag in `needed`.
fn leaves_codes_of_test(
    def: &Inst,
    v: u32,
    size: Option<char>,
    needed: u32,
) -> bool {
    let Kind::Op(name) = def.kind else { return false };
    let Some((mnemonic, def_size)) = mnemonic(name) else { return false };
    let result = matches!(
        def.operands.first(),
        Some(Operand::Def(Reg::Virt(d))) if *d == v
    );
    // `swap` has no size letter and works on the long
    let same_size = def_size.or(Some('l')) == size;
    result && same_size && ccr::like_test(mnemonic).is_some_and(|same| needed & !same == 0)
}

/// Drops each `tst` the instruction that defined its register already stands
/// in for.
pub fn eliminate_tests(function: &mut MFunction) {
    for block in &mut function.blocks {
        let mut at = 0;
        while at < block.insts.len() {
            let Some((v, size)) = tested(&block.insts[at]) else {
                at += 1;
                continue;
            };
            if droppable(&block.insts, at, v, size) {
                block.insts.remove(at);
            } else {
                at += 1;
            }
        }
    }
}

fn droppable(
    insts: &[Inst],
    at: usize,
    v: u32,
    size: Option<char>,
) -> bool {
    // the defining instruction, with nothing that sets the codes after it
    let mut def = None;
    for before in insts[..at].iter().rev() {
        if before.operands.iter().any(|one| matches!(one, Operand::Def(Reg::Virt(d)) if *d == v)) {
            def = Some(before);
            break;
        }
        if before.sets_flags || before.call {
            return false;
        }
    }
    let Some(def) = def.filter(|def| def.sets_flags) else { return false };
    // the flags the readers test, until the next setter
    let mut needed = 0;
    for after in &insts[at + 1..] {
        if after.sets_flags || after.call {
            break;
        }
        if after.reads_flags {
            match read_by(after) {
                Some(flags) => needed |= flags,
                None => return false,
            }
        }
    }
    leaves_codes_of_test(def, v, size, needed)
}
