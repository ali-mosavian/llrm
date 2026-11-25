//! At -Os a callee-saved register is priced in bytes.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// rectwo -Os: `n - 1` was spilled to save the third callee-saved register,
/// but it reads `n`, which is gone at its later uses, so the spill was real;
/// the stack slot also cost the frame its pre- and post-indexed `stp`/`ldp`
/// (+4 B). A value whose definition reads a register is not free to make
/// again.
#[test]
fn a_value_made_from_a_register_is_not_priced_as_remade() {
    let asm = driver::assembly(include_str!("data/paths_os.ll"), Platform::Linux, Level::Os).expect("lowers");
    let paths: Vec<&str> = asm.lines().skip_while(|line| *line != "paths:").skip(1).take_while(|line| !line.ends_with(':') || line.starts_with(".L")).collect();
    let spills = paths.iter().filter(|line| line.trim().starts_with("str ") && line.contains("[sp]")).count();
    assert_eq!(spills, 0, "{asm}");
}
