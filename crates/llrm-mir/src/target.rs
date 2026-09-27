//! What analyses may ask of the target, which MIR does not state: the
//! driver hands one to the pass manager (`PassManager::target`), as LLVM's
//! `TargetMachine` gives its analyses `TargetTransformInfo`.

/// Where the target keeps no program data, as linear addresses: old
/// `abi::machine::Machine::foreign_span`. A real-mode target has some (its
/// video memory and ROM); any other none.
pub trait Machine {
    /// The linear bytes that `width`-byte accesses at `selectors` and
    /// `offsets` (unsigned words) reach, where foreign memory holds them all.
    fn foreign_span(&self, selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)>;
}
