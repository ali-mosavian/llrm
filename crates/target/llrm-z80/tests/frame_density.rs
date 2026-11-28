//! Frame objects past IX's reach are laid by density, as the pin's
//! `orderFrameObjects` does. By size alone a cold 100-byte array lay nearest
//! IX and eight stores to the hot 120-byte one went through `ld iy, #n; add iy,
//! sp`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_hot_array_lies_within_ixs_reach_of_a_colder_smaller_one() {
    let ir = include_str!("kernels/ir/frame_density.ll");
    let out = text(ir, "density", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let stores = out.lines().filter(|line| line.contains("(ix), ")).count();
    assert!(stores >= 8 && !out.contains("(iy)"), "{out}");
}
