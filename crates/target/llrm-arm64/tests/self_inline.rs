//! A small tree recursion is copied into itself at -O2 (DEVIATE REC-1).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn instructions(
    source: &str,
    level: Level,
) -> usize {
    let asm = driver::assembly(source, Platform::Linux, level).expect("lowers");
    asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

/// rectwo -O2: `paths(n - 1, m) + paths(n, m - 1)` ran 777,916 instructions
/// where gcc's copies of it run 266,373. The copies are 74 instructions; the
/// module without them is 24. At -Os the pin's count stands.
#[test]
fn a_small_tree_recursion_is_copied_into_itself_at_o2_and_not_at_os() {
    let ir = include_str!("data/self_inline_paths.ll");
    let o2 = instructions(ir, Level::O2);
    assert!(o2 > 50, "{o2} instructions");
    assert!(instructions(ir, Level::Os) < 40);
}
