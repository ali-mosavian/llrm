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

    /// The bytes an argument of `width` takes on the stack, and the least a
    /// stack cell holds.
    fn stack_slot_bytes(&self) -> i64;

    /// The register a frame's cells are addressed through.
    fn frame_register(&self) -> iced_x86::Register;

    /// Where the first argument lies from the frame register: past the saved
    /// frame register and the return address, which a far call makes longer.
    fn first_argument_offset(&self, far: bool) -> i64;

    /// The registers a result of `width` bytes leaves in, low part first.
    fn results(&self, width: u32) -> Vec<iced_x86::Register>;
}

/// The I/O ports of a PC: a `[[ports]]`-only description that a platform appends to
/// its own text.
pub const PC_PORTS: &str = include_str!("machines/pc-ports.toml");
