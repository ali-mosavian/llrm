//! `AArch64ConditionalCompares` takes a chain by its code-size delta at
//! `minsize`; at `optsize` (-Os) too, as gcc's `ccmp` does. The trace model
//! alone decided at -Os and left `cmp; b.cond; cmp; b.cond` (QuickrBasic
//! `sugg_paren`, 148 B against 144 B, and 73 chains over the product).

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

fn ccmps(name: &str) -> usize {
    let asm =
        driver::assembly(include_str!("data/conditional_compare_size.ll"), Platform::Darwin, true).expect("lowers");
    let start = asm.find(&format!("_{name}:")).expect("the function is there");
    let rest = &asm[start..];
    let text = &rest[..rest[1..].find("\n_").map_or(rest.len(), |end| end + 1)];
    text.matches("ccmp").count()
}

#[test]
fn a_chain_the_trace_model_refuses_is_a_ccmp_at_optsize() {
    let (size, speed) = (ccmps("paren_size"), ccmps("paren_speed"));
    assert!(size > speed, "optsize {size} ccmp, speed {speed}");
}

#[test]
fn the_same_function_at_speed_is_left_to_the_trace_model() {
    assert_eq!(ccmps("paren_speed"), 1);
}
