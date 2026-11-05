//! `memcpy(dst, src, 22)` stored its arguments to the call area in the order
//! offset 4, 0, 8, which `callpush` read as "not a push sequence": -Os kept
//! `lea (-16,%sp),%sp`, three stores and a `lea (16,%sp),%sp`, 54 B against
//! gcc's 42. The stores may come in any order; the pushes go before the call.
//! `structcopy_raw.ll` is clang's output with no passes, what `llrm-cc` hands
//! the driver.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const STRUCTCOPY: &str = include_str!("fixtures/structcopy_raw.ll");

#[test]
fn memcpys_arguments_are_pushed_whatever_order_they_were_stored_in() {
    let out = assembly(STRUCTCOPY, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    assert_eq!(out.matches("-(%sp)").count(), 4, "a2 and three pushes expected:\n{out}");
    assert!(!out.contains("lea\t(-"), "a call area is still reserved:\n{out}");
    let before_call = out.split("jsr").next().unwrap();
    let pushed: Vec<&str> = before_call.lines().filter(|l| l.ends_with("-(%sp)")).collect();
    assert!(pushed[1].contains("#22"), "the length goes first (last argument):\n{out}");
}
