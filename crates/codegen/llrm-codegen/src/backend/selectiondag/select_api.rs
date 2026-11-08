//! What the compile hands the x86 function selector besides the module.

/// What the program's ABI says about its runtime routines: a QB routine's
/// contract names the registers it destroys and how it returns, which its
/// declared convention does not.
pub struct RuntimeContracts<'a> {
    pub abi: &'a dyn crate::backend::assemble::Abi,
    pub segments: &'a crate::backend::target::Segments,
    /// Whether the program's runtime routines have contracts of their own
    /// (QB's); a call of any other routine reads the ABI's for what it
    /// destroys.
    pub runtime: bool,
    /// Whether the function's frame is zero-filled by the runtime that builds
    /// it (QB's `B$ENRA`): the entry block's zero writes to a fresh local are
    /// then dropped.
    pub frame_zeroed: bool,
    /// What the module's finished callees wrote, which a call of one destroys
    /// and no more (`isel`'s `written`).
    pub facts: Option<&'a crate::backend::calleefacts::CalleeFacts>,
    /// Bytes below BP left free above the allocas, for spill slots
    /// (`relayout`'s second layout): the frame's objects start under them.
    pub hole: i64,
}
