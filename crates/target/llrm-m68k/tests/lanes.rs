//! The m68k register file's lanes: a lane per byte of each root.

use llrm_codegen::backend::lanes::{Lanes, enter};
use llrm_codegen::backend::peephole::_lanes;
use llrm_lir::registers::Regs;
use llrm_m68k::{REGISTER_INFO, reg};

/// The file said `@lanes none`, and the shared prelude (`tolir`) entering the
/// lanes panicked in 8 tests.
#[test]
fn the_byte_and_word_views_of_a_data_register_are_its_low_lanes() {
    let regs = Regs(&REGISTER_INFO);
    let _guard = enter(regs);
    let (byte, word, long) = (_lanes(regs, reg::BD0), _lanes(regs, reg::WD0), _lanes(regs, reg::D0));
    assert!(byte != Lanes::new() && byte != word, "byte {byte:?} word {word:?}");
    assert_eq!(byte.or(&word), word);
    assert_eq!(word.or(&long), long);
}
