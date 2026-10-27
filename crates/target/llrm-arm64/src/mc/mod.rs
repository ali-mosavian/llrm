//! The machine code of arm64: an instruction as data (`inst`), its encoding
//! (`encode`), its text (`text`) and the immediates the encodings code
//! (`immediate`). Translated from LLVM's `AArch64` MC layer.

pub mod elf;
pub mod encode;
pub mod immediate;
pub mod inst;
pub mod macho;
mod text;
pub mod visit;

#[cfg(test)]
pub(crate) mod tests;

use llrm_object::{Object, ObjectTarget, Unsupported};

/// arm64's object writers.
pub struct Arm64Objects;

impl ObjectTarget for Arm64Objects {
    fn write_object(
        &self,
        format: &str,
        object: &Object,
    ) -> Result<Vec<u8>, Unsupported> {
        match format {
            "macho" => llrm_macho::write(object, &macho::Arm64MachO),
            "elf" => elf::write(object),
            _ => Err(Unsupported(format!("no {format} writer for {:?} yet", object.arch))),
        }
    }
}
