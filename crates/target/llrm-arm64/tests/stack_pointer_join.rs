//! A restore of a stack pointer nothing else moves writes nothing
//! (`joinReservedPhysReg`): the call area stays reserved in the frame.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn function(
    asm: &str,
    name: &str,
) -> String {
    let asm = format!("\n{asm}");
    let start = asm.find(&format!("\n{name}:")).expect("the function") + 1;
    let rest = &asm[start..];
    let end = rest[1..]
        .match_indices('\n')
        .map(|(at, _)| at + 2)
        .find(|&at| {
            rest[at..].lines().next().is_some_and(|line| line.ends_with(':') && !line.starts_with(['.', ' ', '\t']))
        })
        .unwrap_or(rest.len());
    rest[..end].to_owned()
}

/// vla_callee_constant arm64 -O2: a `stackrestore` of an unmoved stack
/// pointer made each call bracket the stack by hand and kept every cell
/// off the frame register: 424 B against 400.
#[test]
fn a_restore_of_an_unmoved_stack_pointer_writes_nothing() {
    let asm = driver::assembly(include_str!("data/stack_pointer_join.ll"), Platform::Linux, Level::O2).expect("lowers");
    let f = function(&asm, "f");
    assert!(!f.contains("sp, x"), "{f}");
    assert!(!f.contains("x29"), "{f}");
}

/// A dynamic allocation between the save and the restore is a write: the
/// restore stays.
#[test]
fn a_restore_after_a_dynamic_allocation_is_kept() {
    let asm = driver::assembly(include_str!("data/stack_pointer_join.ll"), Platform::Linux, Level::O2).expect("lowers");
    let g = function(&asm, "g");
    assert!(g.contains("sp, x"), "{g}");
}
