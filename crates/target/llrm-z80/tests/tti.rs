//! The Z80 cost model's addressing modes: what one `ld` takes.

use llrm_mir::tti::Tti;
use llrm_z80::tti::Z80Tti;

/// A register alone and an absolute address are modes; a register plus a
/// displacement, two registers and a scaled index are not. With `Basic`'s
/// "nothing is legal", LSR panicked ("Formula is illegal") on any loop
/// over a global: 10 of the 23 bench kernels.
#[test]
fn a_register_alone_and_an_absolute_address_are_the_modes() {
    let tti = Z80Tti::new();
    // (gv, offset, base register, scale)
    assert!(tti.is_legal_addressing_mode(false, 0, true, 0), "ld a,(hl)");
    assert!(tti.is_legal_addressing_mode(false, 0, false, 1), "a lone index register is a register");
    assert!(tti.is_legal_addressing_mode(true, 0, false, 0), "ld a,(sym)");
    assert!(tti.is_legal_addressing_mode(true, 6, false, 0), "ld a,(sym+6)");
    assert!(!tti.is_legal_addressing_mode(false, 5, true, 0), "register plus displacement");
    assert!(!tti.is_legal_addressing_mode(true, 0, true, 0), "symbol plus register");
    assert!(!tti.is_legal_addressing_mode(false, 0, true, 1), "two registers");
    assert!(!tti.is_legal_addressing_mode(false, 0, false, 2), "scaled index");
}
