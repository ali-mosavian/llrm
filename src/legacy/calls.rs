//! Port of `qbopt/legacy/calls.py`: so far only the fixed registers an
//! absorbed long divide uses.

use iced_x86::Register;

/// The routine that answers a long remainder.
pub const REMAINDER: &str = "B$RMI4";
/// What the runtime returns a long in, as ax:dx.
pub const RESULT: Register = Register::EAX;
pub const OTHER: Register = Register::EBX;
pub const DIVISOR: Register = Register::ECX;
