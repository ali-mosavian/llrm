//! The 6502 target (C64 first): spike, modelled on LLVM's AVR backend.

pub mod dagisel;

/// `registers.regs`: A, X, Y and the zero-page bytes used as registers.
pub const REGISTERS: &str = include_str!("registers.regs");
/// `machines/datalayout.toml`: 8-bit native, 16-bit pointers.
pub const LAYOUT: &str = include_str!("machines/datalayout.toml");
/// `machines/calling.toml`: arguments in rc2.., results in A:X.
pub const CALLING: &str = include_str!("machines/calling.toml");
