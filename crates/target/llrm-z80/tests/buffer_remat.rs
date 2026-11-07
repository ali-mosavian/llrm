//! The remainder of a divide by a constant comes back through a stack buffer
//! the divide routine fills. A use after the next call spilled the loaded
//! remainder into a fresh slot (24 B a site, 15 sites in div_constants_long):
//! the load is made again where it is read now.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_remainder_read_after_a_call_is_loaded_again_not_spilled() {
    let ir = include_str!("kernels/ir/rem_after_call.ll");
    let out = text(ir, "f", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    assert!(
        lines.iter().any(|line| line.starts_with("call __s_udivremsi4")),
        "premise: one divide with a remainder\n{out}"
    );
    let stores = lines.iter().filter(|line| line.starts_with("ld -") && line.contains("(ix),")).count();
    assert_eq!(stores, 4, "only the quotient (4 bytes) is stored\n{out}");
}
