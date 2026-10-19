//! The machine code of arm64: an instruction as data (`inst`), its encoding
//! (`encode`), its text (`text`) and the immediates the encodings code
//! (`immediate`). Translated from LLVM's `AArch64` MC layer.

pub mod encode;
pub mod immediate;
pub mod inst;
mod text;
pub mod visit;

#[cfg(test)]
pub(crate) mod tests;
