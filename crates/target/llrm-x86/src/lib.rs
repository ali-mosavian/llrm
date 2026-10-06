//! What every x86 target shares. Each target's `x86.instr` holds only the forms
//! it adds to `instructions::FAMILY`.

pub mod instructions {
    /// The forms every x86 target has, `x86.instr`.
    pub const FAMILY: &str = include_str!("instructions/x86.instr");

    /// The family's forms, then `own`: a target's description is the two joined.
    pub fn joined(own: &str) -> String {
        format!("{FAMILY}\n{own}")
    }
}
