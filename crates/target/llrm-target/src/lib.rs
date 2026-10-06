//! The target-generic layer: what a target description is, owned by no ISA.
//! A target crate supplies the data; the passes read the type.

pub mod machine;

use machine::Machine;

/// A target, as the driver picks one by name: what a frontend of it starts
/// from. The passes' view of a target grows here as the backend stops naming
/// one (docs/targets.md).
pub trait Target {
    /// The name `--target` takes.
    fn name(&self) -> &'static str;

    /// The platform description a frontend of this target defaults to.
    fn machine(&self) -> Machine;

    /// The processors this target prices, as a platform description names them.
    fn cpus(&self) -> &'static [&'static str];
}

/// The I/O ports of a PC: a `[[ports]]`-only description that a platform appends to
/// its own text.
pub const PC_PORTS: &str = include_str!("machines/pc-ports.toml");
