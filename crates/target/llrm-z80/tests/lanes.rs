//! The z80 register file's lanes: a lane per byte of each root.

use llrm_codegen::backend::lanes::{Lanes, enter};
use llrm_codegen::backend::peephole::_lanes;
use llrm_lir::registers::Regs;
use llrm_z80::{REGISTER_INFO, reg};

/// The file stated no `@lanes` line and read x86's table: `h` and `l` were two
/// unrelated x86 registers, every pinned z80 instruction had no effect (94 of
/// 94 in `three`) and the scheduler ordered nothing.
#[test]
fn the_two_bytes_of_a_pair_are_its_two_lanes() {
    let regs = Regs(&REGISTER_INFO);
    let _guard = enter(regs);
    let (h, l, hl) = (_lanes(regs, reg::H), _lanes(regs, reg::L), _lanes(regs, reg::HL));
    assert!(h != Lanes::new() && l != Lanes::new() && h != l, "h {h:?} l {l:?}");
    assert_eq!(h.or(&l), hl);
}
