//! The pin runs the greedy allocator for every x86 target; a target that says
//! nothing gets it.

use llrm_target::{Allocator, Target};
use llrm_x86_m16::M16;
use llrm_x86_m32::M32;
use llrm_x86_m64::M64;

#[test]
fn test_the_x86_targets_run_the_greedy_allocator() {
    assert_eq!(M16.register_allocator(), Allocator::Greedy);
    assert_eq!(M32.register_allocator(), Allocator::Greedy);
    assert_eq!(M64.register_allocator(), Allocator::Greedy);
}
