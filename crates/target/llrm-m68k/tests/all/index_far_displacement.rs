//! `(base + index) + 272` has no `(d8,An,Xn)` form: the constant became the
//! index register (`move.w #272,%a1; move.l (0,%a0,%a1),%d0`, 8 B) where
//! `add.l` and `(272,%a0)` is 6 B and one fewer register.

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_far_constant_after_base_and_index_is_a_displacement() {
    let source = include_str!("../data/index_far_displacement.ll");
    let text = assembly(source, cpu_of("68000").unwrap(), Level::Os).unwrap();
    assert!(text.contains("(272,"), "{text}");
}
