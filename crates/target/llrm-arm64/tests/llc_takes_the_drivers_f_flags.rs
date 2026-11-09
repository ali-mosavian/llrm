//! `llrm-cc` hands `-fexcess-precision=standard` and `-fno-inline=NAME` to every backend's `llc`. arm64's refused both ("unrecognized
//! argument"), so int_compared_with_float and tail_calls_edges were refused at -O0 on arm64-linux; m68k took them.

#[test]
fn the_flags_llrm_cc_passes_on_are_taken() {
    for flag in ["-fexcess-precision=standard", "-fexcess-precision=fast", "-fno-inline=sum5", "-ffloat-store", "-fno-unroll-loops"] {
        assert!(llrm_arm64::driver::pass_flag(flag), "{flag}");
    }
    assert!(!llrm_arm64::driver::pass_flag("-fnot-a-flag"));
    assert!(!llrm_arm64::driver::pass_flag("-O2"));
}
