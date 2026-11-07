//! `splat (fp128 2.0)` was refused ("expected a constant, found `splat`"), so clang IR holding a
//! vector constant of one repeated element failed to compile (wasm32 `long_double_layout` -O2).
//! It parses as the vector of that element, and the print parses again.

use llrm_mir::{parse, print};

const SOURCE: &str = "define <2 x fp128> @f(<2 x fp128> %x) {\nb:\n  %y = fmul <2 x fp128> %x, splat (fp128 0xL00000000000000004000000000000000)\n  ret <2 x fp128> %y\n}\n";

#[test]
fn test_splat_constant_parses_and_round_trips() {
    let module = parse::module(SOURCE).expect("splat parses");
    let text = print::module(&module);
    parse::module(&text).expect("the print parses again");
}

#[test]
fn test_splat_of_the_wrong_element_type_is_refused() {
    let bad = SOURCE.replace("splat (fp128", "splat (double").replace("0xL00000000000000004000000000000000", "2.0");
    assert!(parse::module(&bad).is_err());
}
