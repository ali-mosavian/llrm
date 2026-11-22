//! The byte table is the one fitted to the bytes written (`size_fit.txt`, `tools/z80_size_fit.py`). With the
//! forms' bytes frame_address_stack_args -O1 inlined a callee whose copy the byte model read as shrinking the
//! code: 474 B / 9060 T, against 463 B / 455 T with the fitted table.

use llrm_z80::compile::{effective, module_priced};
use llrm_z80::driver::{Level, Request};

#[test]
fn frame_address_stack_args_o1_takes_the_inline_the_fitted_table_prices() {
    let ir = include_str!("data/frame_address_stack_args_o1.ll");
    let request = effective(ir, &Request::new(Level::O1)).expect("chooses");
    let (bytes, clocks) = module_priced(ir, &request).expect("prices");
    assert!(bytes <= 470 && clocks <= 5000, "-O1 {bytes} B / {clocks} T");
}
