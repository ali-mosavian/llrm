//! `-fno-inline=NAME` reaches `llrm-wasm-llc` (wasm drops the pass switches but not this one) and was refused as an unrecognized
//! argument: tail_calls_edges was refused at -O0 on wasm32.

#[test]
fn the_flags_llrm_cc_passes_on_are_taken() {
    for flag in ["-fno-inline=sum5", "-fexcess-precision=standard"] {
        assert!(llrm_wasm::driver::pass_flag(flag), "{flag}");
    }
    assert!(!llrm_wasm::driver::pass_flag("-fnot-a-flag"));
}
