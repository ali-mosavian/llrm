//! Ports of `qbopt/abi`.

pub use llrm_bcmachine::abi::{callsite, events, handlers, machine, runtime};

pub mod inputscan;
pub mod linkunit;
pub mod nib;
pub mod nativecalls;
pub mod profile;
pub mod qb;

#[cfg(test)]
mod tests {
    /// The machine layer cannot see the backend, so it lists the CPUs itself.
    #[test]
    fn test_machine_cpus_are_the_backend_profiles() {
        assert_eq!(crate::backend::cpu::names(), super::machine::CPUS);
    }
}
