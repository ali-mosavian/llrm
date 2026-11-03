//! m68k read through `llrm_target::Target`, against the pin's m68k facts.

use llrm_m68k::reg;
use llrm_m68k::target::M68k;
use llrm_target::Target;

/// Results leave in `d0` by width, the frame register is `a6` and the stack
/// pointer `sp` (`M68kRegisterInfo`), and `d2` is kept for the caller
/// (`CSR_STD`).
#[test]
fn the_target_states_the_pins_register_roles() {
    let target = M68k;
    assert_eq!(target.results(4), [reg::D0]);
    assert_eq!(target.results(2), [reg::WD0]);
    assert_eq!(target.results(1), [reg::BD0]);
    assert_eq!(target.frame_register(), reg::A6);
    assert_eq!(target.stack_pointer(), reg::SP);
    assert!(target.callee_saved().iter().any(|&(full, _)| full == reg::D2));
    assert!(!target.callee_saved().iter().any(|&(full, _)| full == reg::D0));
}

/// The pin's datalayout for m68k: big-endian, 4-byte pointers.
#[test]
fn the_layout_is_big_endian_with_four_byte_pointers() {
    let target = M68k;
    assert!(target.layout().datalayout.starts_with("E-"));
    assert_eq!(target.object().bitness, 32);
    assert_eq!(target.stack_slot_bytes(), 4);
}

/// After the object refactor `Target` asked for `objects` and `debug_target`;
/// m68k answered neither and the crate did not build. It writes ELF now
/// (3f55493bf5) and no other format; the test still said it had no writer.
#[test]
fn the_target_writes_elf_and_no_other_object_format() {
    assert!(M68k.objects().asm_info("elf").is_some());
    assert!(M68k.objects().asm_info("coff").is_none());
    assert!(M68k.objects().asm_info("omf").is_none());
    assert_eq!(M68k.debug_target().address_bytes, 4);
}
