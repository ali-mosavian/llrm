//! `goto *table[i]` and `&&label`: the address of a block is a symbol, an
//! `indirectbr` jumps to a register and keeps its targets as successors.
//! Before, `llrm-arm64-llc` refused every function that took a label's
//! address (`@exec_loop: aggregate constant`, QuickrBasic's VM loop).

mod support;

/// The bytecode loop dispatches through a static table of label addresses;
/// `jump_to` through a pointer chosen by a condition.
#[test]
fn computed_goto_runs_at_every_level() {
    for level in ["-O1", "-O2", "-Os"] {
        if let Some(said) = support::output("computed_goto", level) {
            assert_eq!(said, "0 27 11 20\n", "{level}");
        }
    }
}
