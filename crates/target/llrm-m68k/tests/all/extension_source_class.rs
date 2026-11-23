//! The source of `MOVZXd32d16` and `MOVSXd32d16` is the pin's `XR16`, a word.
//! It was a long register (`Dr32`), so each extension of a word value read a
//! copy of its own (`Copy Dr16 -> Dr32`, per use and per block): three uses of
//! one call result made three different operands, which no later pass could
//! share.

use llrm_m68k::mfunc::{Class, operand_class};

#[test]
fn the_source_of_a_word_extension_is_a_word_register() {
    for name in ["MOVZXd32d16", "MOVSXd32d16"] {
        assert_eq!(operand_class(name, 1), Some(Class::Dr16), "{name} source");
    }
}

/// The result is still a long data register.
#[test]
fn the_result_of_a_word_extension_is_a_data_long() {
    for name in ["MOVZXd32d16", "MOVSXd32d16"] {
        assert_eq!(operand_class(name, 0), Some(Class::Dr32), "{name} result");
    }
}
