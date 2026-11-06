//! A copy that puts a value back where it already is is dropped.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The argument was copied into the callee-saved `x19` and straight back into
/// `x0` for the call: one `mov` too many in every recursion of recpow's
/// `power` (the pin's MachineCopyPropagation drops it).
#[test]
fn a_value_copied_out_and_straight_back_is_copied_once() {
    let asm = driver::assembly(include_str!("data/shrink_wrap_loop.ll"), Platform::Linux, Level::O2).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let out = lines.iter().position(|line| *line == "orr x19, xzr, x0, lsl #0").expect("the copy out");
    assert_ne!(lines[out + 1], "orr x0, xzr, x19, lsl #0", "{asm}");
}

/// Fourteen accumulators start at 0 across calls. Their phi copies read one
/// shared zero, so `orr x9, xzr, x10` chains were kept (and spilled) where the
/// pin makes the zero again at each copy (`reMaterializeTrivialDef`): +9
/// instructions in jpeg_single -Os.
#[test]
fn a_copy_of_the_zero_register_is_made_again_not_copied_from_one_shared_zero() {
    let asm = driver::assembly(include_str!("data/zero_spill.ll"), Platform::Linux, Level::O1).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let first = lines.iter().find_map(|line| line.strip_prefix("orr ")?.strip_suffix(", xzr, xzr, lsl #0")).expect("a zero");
    let shared = format!(", xzr, {first}, lsl #0");
    assert!(!lines.iter().any(|line| line.starts_with("orr ") && line.ends_with(&shared)), "{asm}");
}

/// A zero accumulator that spills before it is first used was made in a
/// register and stored (`orr x9, xzr, xzr; str x9, [sp, N]`): the pin folds the
/// spill of a copy from `xzr` into `str xzr` (`foldMemoryOperandImpl`); two of them pair into `stp`.
#[test]
fn a_spilled_zero_is_stored_from_the_zero_register() {
    let asm = driver::assembly(include_str!("data/zero_spill.ll"), Platform::Linux, Level::O1).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    assert!(lines.iter().any(|line| line.starts_with("str xzr, [sp") || line.starts_with("stp xzr, xzr, [sp")), "{asm}");
    let zeroed = lines.windows(2).any(|pair| {
        pair[0].strip_prefix("orr ").and_then(|rest| rest.strip_suffix(", xzr, xzr, lsl #0")).is_some_and(|reg| {
            pair[1].starts_with(&format!("str {reg}, [sp"))
        })
    });
    assert!(!zeroed, "{asm}");
}

/// The spilled-zero store kept the remat flag of the definition it was cloned
/// from, greedy refused the function ("no definition of the function") and the
/// linear scan left `add x9, x10, #1; mov x10, x9` in jpeg_single's inner
/// loops: +19% instructions at -Os.
#[test]
fn a_function_with_a_spilled_zero_stays_on_greedy() {
    let before = driver::greedy_fallbacks();
    driver::assembly(include_str!("data/zero_spill_os.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert_eq!(driver::greedy_fallbacks(), before, "fell back to the linear scan");
}
