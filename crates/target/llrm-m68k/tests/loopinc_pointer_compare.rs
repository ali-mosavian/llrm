//! `partition`'s loop with the exit test on the incremented pointer
//! (`icmp eq ptr %p.next, %end`, the shape the exit-value fold leaves) lost its
//! `(An)+`: `address_adds` kept the step in a data register because the compare
//! read its result, so `loopinc` found no address-register step and the loop
//! paid `move; addq; move; cmp` (42 instructions, 39 with the fix; +2/+6 B on
//! partition, +8 memset Os, +12 crc16 O2 and memcpy once the exit-value fold
//! is on). A counter compared with an integer bound must stay a data register
//! (`addr_roles`, `compare_in_address_register`, `cmpa_post_increment`).

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const IR: &str = include_str!("fixtures/partition_fold_isel.ll");

/// The premise: the pointer is read in the header, stepped in the latch, and
/// the latch compares the stepped pointer.
#[test]
fn the_fixture_compares_the_incremented_pointer() {
    assert!(IR.contains("icmp eq ptr"), "{IR}");
    assert!(IR.contains("getelementptr i8, ptr %19, i32 4"), "{IR}");
}

#[test]
fn a_pointer_stepped_in_the_latch_and_compared_is_a_post_increment_load() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let asm = text_with(&module, "k", Cpu::M68000, Options { mid_end: false, ..Options::default() }).expect("compiles");
    // a load through `(An)+`; the epilogue's `movem.l (%sp)+` is not one
    let loads = asm.lines().filter(|line| line.starts_with("\tmove.l\t(%a") && line.contains(")+,")).count();
    assert_eq!(loads, 1, "no (An)+ load in the loop:\n{asm}");
}
