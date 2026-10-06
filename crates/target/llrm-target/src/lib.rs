//! The target-generic layer: what a target description is, owned by no ISA.
//! A target crate supplies the data; the passes read the type.

pub mod machine;

/// The I/O ports of a PC: a `[[ports]]`-only description that a platform appends to
/// its own text.
pub const PC_PORTS: &str = include_str!("machines/pc-ports.toml");
