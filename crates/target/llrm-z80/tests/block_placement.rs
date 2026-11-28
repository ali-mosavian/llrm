//! `MachineBlockPlacement` runs after the last branch fold.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `fcmp_all` -O2 `main` held 36 more unconditional `jp` than it needed (+108
/// B): blocks stayed in selection order, one jump per edge that did not follow
/// it. Placed, this reduction of its chains of loops has one (O1 -O3 were 3, 5
/// and 5).
#[test]
fn blocks_are_placed_so_edges_fall_through() {
    for level in [Level::O1, Level::O2, Level::O3] {
        let asm = text(include_str!("kernels/ir/block_placement.ll"), "f", 0, &Request::new(level)).unwrap();
        let jumps = asm.lines().filter(|line| line.trim().starts_with("jp ") && !line.contains(',')).count();
        assert!(jumps <= 2, "{level:?}: {jumps} unconditional jumps\n{asm}");
    }
}
