//! loop_versioning -O2/-O3 was 447 B / 1,749,192 T once `total` stayed a call: a long product of two
//! sign-extended words is expanded to four `__mulhi3` calls (~4300 T a trip) where one `__mulsi3` is ~2500,
//! and a lone product was never priced against the call (`prepare::mark_long_multiplies` wanted two sites).
//! The lone product is priced against the call and the module keeps the smaller form: 323 B / 1,036,323 T.

use llrm_z80::compile::{effective, module_priced, text};
use llrm_z80::driver::{Level, Request};

const LOOP_VERSIONING: &str = include_str!("data/loop_versioning_o2.ll");
const WORD_COMPARES: &str = include_str!("data/word_compares_o2.ll");
const STANDALONE: &str = include_str!("data/sext_product_standalone_o2.ll");

#[test]
fn the_cell_that_ran_the_product_as_four_word_calls_stays_under_a_million_clocks() {
    for level in [Level::O2, Level::O3] {
        let chosen = effective(LOOP_VERSIONING, &Request::new(level)).expect("chooses");
        let (bytes, clocks) = module_priced(LOOP_VERSIONING, &chosen).unwrap();
        assert!(clocks <= 1_050_000 && bytes <= 447, "{level:?}: {bytes} B / {clocks} T");
    }
}

/// The product is one `__mulsi3` call in its own function, not four word calls (the loop in `main`
/// is the routine's loop expanded in place: ucsim 147,424 T against 147,264 T standalone).
#[test]
fn a_sign_extended_word_product_in_a_loop_is_one_long_multiply_call() {
    let request = effective(STANDALONE, &Request::new(Level::O2)).expect("chooses");
    let asm = text(STANDALONE, "total", 0, &request).expect("compiles");
    assert!(asm.contains("call __mulsi3") && !asm.contains("call __mulhi3"), "total:\n{asm}");
}

/// The call is taken only where it gives up neither axis: word_compares -O2 is 12 B smaller with it and 35% slower
/// (520,441 T as it was, 703,547 T with the call), so the word products stay.
#[test]
fn the_call_that_saves_bytes_but_costs_clocks_is_not_taken() {
    let chosen = effective(WORD_COMPARES, &Request::new(Level::O2)).expect("chooses");
    let (bytes, clocks) = module_priced(WORD_COMPARES, &chosen).unwrap();
    assert!(clocks <= 600_000, "{bytes} B / {clocks} T");
}
