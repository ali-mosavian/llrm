//! The 6502 read through `llrm_target::Target`.

use llrm_6502::reg;
use llrm_6502::target::Mos;
use llrm_target::Target;

/// A byte result leaves in `a`, a word in `a:x`; `rs0` is the frame and stack
/// register.
#[test]
fn the_target_states_the_register_roles() {
    let target = Mos;
    assert_eq!(target.results(1), [reg::A]);
    assert_eq!(target.results(2), [reg::A, reg::X]);
    assert_eq!(target.stack_pointer(), reg::RS0);
    assert_eq!(target.frame_register(), reg::RS0);
    assert!(target.layout().datalayout.starts_with("e-p:16"));
}
