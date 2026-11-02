//! The Z80 read through `llrm_target::Target`.

use llrm_target::Target;
use llrm_z80::reg;
use llrm_z80::target::Z80;

/// A byte result leaves in `a`, a word in `de`; `ix` is the frame register and
/// `sp` the stack.
#[test]
fn the_target_states_the_register_roles() {
    let target = Z80;
    assert_eq!(target.results(1), [reg::A]);
    assert_eq!(target.results(2), [reg::E, reg::D]);
    assert_eq!(target.stack_pointer(), reg::SP);
    assert_eq!(target.frame_register(), reg::IX);
    assert!(target.layout().datalayout.starts_with("e-p:16"));
}
