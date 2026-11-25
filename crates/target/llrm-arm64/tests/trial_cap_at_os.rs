//! A module with a very large function is built once at -Os: each trial builds
//! the whole module again (one QB module took 18% of all -Os compile
//! instructions, and the cap costs 0.019% of the bytes).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// Functions priced while compiling `particle_os.ll` (it notes a merge, so it
/// is priced and tried) beside a function of `stores` volatile stores.
fn priced_with(stores: usize) -> usize {
    let mut text = include_str!("data/particle_os.ll").to_owned();
    text.push_str("\ndefine void @wide(ptr %p) optsize {\n");
    for i in 0..stores {
        text.push_str(&format!("  store volatile i32 {}, ptr %p\n", 1000 + 7 * i));
    }
    text.push_str("  ret void\n}\n");
    let before = llrm_arm64::trial::priced();
    driver::assembly(&text, Platform::Linux, Level::Os).expect("lowers");
    llrm_arm64::trial::priced() - before
}

/// 2,400 instructions in one function: the first build stands, each function
/// priced once; 50 stores: the merge is tried, a second build.
#[test]
fn a_function_over_the_bound_keeps_the_first_build_at_os() {
    let small = priced_with(50);
    let wide = priced_with(1_200);
    assert_eq!(wide * 2, small, "{small} priced with the trial, {wide} for one build");
}
