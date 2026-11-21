//! `long double` is binary128 on arm64 Linux: operations are `tf` calls with
//! `q` register arguments. Each kernel is built by this compiler from the
//! pinned clang's un-optimized IR, linked statically with the cross gcc (its
//! libgcc has the `tf` routines) and run under `qemu-aarch64`.

mod support;

fn output(kernel: &str) -> Option<String> {
    support::output(kernel, "-O2")
}

/// Register-to-register moves of an `f128` were `fmov d`: 64 of its 128 bits.
/// The kernel printed `0 0` for `28 773`.
#[test]
fn the_long_double_kernel_prints_what_gcc_prints() {
    if let Some(said) = output("long_double") {
        assert_eq!(said, "28 773\n");
    }
}

/// `sitofp i32 7 to fp128` folded to a double's bits: `mix(7, x, 2, y, 100)`
/// printed 6 for 111.
#[test]
fn integers_converted_to_long_double_keep_their_value() {
    if let Some(said) = output("long_double_mix") {
        assert_eq!(said, "111\n");
    }
}

/// A call's `f128` result that the allocator spilled from `v0` was stored with
/// `str d0`, 64 of its 128 bits (the width came from the register, and a
/// physical one is never wide). QuickrBasic's `pfpu.c` computed 2^3 as 4, and
/// `power(2, n)` was 1 for every `n`.
#[test]
fn a_spilled_long_double_call_result_keeps_all_its_bits() {
    if let Some(said) = output("long_double_spill") {
        assert_eq!(said, "1 2 4 8 16 32 64 128 256 \n");
    }
}
