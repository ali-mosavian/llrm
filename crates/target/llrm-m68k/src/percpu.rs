//! A value computed for the CPU that asks, once.

use std::sync::OnceLock;

use crate::dagisel::hooks::Cpu;

/// One slot per CPU, filled by the first ask for that CPU: a `LazyLock` over
/// all six would read the timing tables of five CPUs the compile never
/// targets.
pub struct PerCpu<T>([OnceLock<T>; 6]);

impl<T> PerCpu<T> {
    pub const fn new() -> Self {
        Self([const { OnceLock::new() }; 6])
    }

    pub fn get(
        &self,
        cpu: Cpu,
        make: impl FnOnce(Cpu) -> T,
    ) -> &T {
        self.0[cpu as usize].get_or_init(|| make(cpu))
    }
}

impl<T> Default for PerCpu<T> {
    fn default() -> Self {
        Self::new()
    }
}
