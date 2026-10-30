//! x86-64 for the Mach-O writer, as LLVM's `X86MachObjectWriter`.
//!
//! # Runs
//! Once per Mach-O object of x86-64, from `objbuild::serialized`.
//!
//! # Example
//! `llrm_macho::write(&object, &X8664MachO)`.

use llrm_macho::{Described, MachTargetWriter};
use llrm_object::{FixupKind, Reloc, Unsupported};

use super::fixup;

const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const CPU_SUBTYPE_X86_64_ALL: u32 = 3;
const X86_64_RELOC_UNSIGNED: u32 = 0;
const X86_64_RELOC_SIGNED: u32 = 1;
const X86_64_RELOC_BRANCH: u32 = 2;
const X86_64_RELOC_SIGNED_1: u32 = 6;
const X86_64_RELOC_SIGNED_2: u32 = 7;
const X86_64_RELOC_SIGNED_4: u32 = 8;

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

pub struct X8664MachO;

impl MachTargetWriter for X8664MachO {
    fn cpu(&self) -> (u32, u32) {
        (CPU_TYPE_X86_64, CPU_SUBTYPE_X86_64_ALL)
    }

    fn lists_locals(&self) -> bool {
        false
    }

    fn relocation(
        &self,
        reloc: &Reloc,
    ) -> Result<Described, Unsupported> {
        let from = reloc.pc_bias;
        let (kind, length, pcrel, beyond) = match (reloc.kind, reloc.pc_rel) {
            (FixupKind::DATA_8, false) => (X86_64_RELOC_UNSIGNED, 3, false, 0),
            (FixupKind::DATA_4, false) => (X86_64_RELOC_UNSIGNED, 2, false, 0),
            (fixup::BRANCH_4, true) => (X86_64_RELOC_BRANCH, 2, true, 0),
            // SIGNED_n is chosen by the total addend, `-(addend - beyond + 4)`
            // in X86MachObjectWriter (n is 1, 2 or 4), not by the distance
            // alone: equal totals are one object.
            (FixupKind::DATA_4, true) if from >= 4 => {
                let beyond = reloc.beyond(4);
                let kind = match beyond - reloc.addend {
                    1 => X86_64_RELOC_SIGNED_1,
                    2 => X86_64_RELOC_SIGNED_2,
                    4 => X86_64_RELOC_SIGNED_4,
                    _ => X86_64_RELOC_SIGNED,
                };
                (kind, 2, true, beyond)
            }
            (fixup::SEGMENT_BASE, _) => return Err(unsupported("a segment selector has no Mach-O relocation")),
            (fixup::FAR_POINTER, _) => return Err(unsupported("a 16:16 far pointer has no Mach-O relocation")),
            other => return Err(unsupported(format!("Mach-O x86-64 has no relocation for {other:?}"))),
        };
        Ok(Described { kind, length, pcrel, beyond, addend_in_field: true })
    }
}

#[cfg(test)]
mod tests {
    use llrm_object::{Binding, Definition, Fix, Object, Reloc, Role, Section, Symbol, Target};

    use super::*;

    /// (type, bytes past the field's end the field compensates) of a
    /// pc-relative 32-bit field `from` bytes short of the place and `addend`
    /// past the symbol.
    fn described(
        from: usize,
        addend: i64,
    ) -> (u32, i64) {
        let reloc = Reloc::new(2, Fix::pc_rel(4, from), Target::Symbol(0), addend);
        let one = X8664MachO.relocation(&reloc).unwrap();
        (one.kind, one.beyond)
    }

    /// X86MachObjectWriter picks SIGNED_n from `-(total + 4)`: `cmpb $1,
    /// x(%rip)` (5 short, addend 0) and `lea x-1(%rip)` (4 short, addend -1)
    /// share the total -5 and write SIGNED_1; llrm chose by the distance
    /// alone and wrote SIGNED_1 and SIGNED.
    #[test]
    fn two_instructions_with_the_same_total_addend_write_one_signed_n() {
        assert_eq!(described(5, 0), (X86_64_RELOC_SIGNED_1, 1));
        assert_eq!(described(4, -1), (X86_64_RELOC_SIGNED_1, 0));
        assert_eq!(described(6, 0).0, described(4, -2).0);
        assert_eq!(described(8, 0).0, described(5, -3).0);
        assert_eq!(described(4, 0).0, X86_64_RELOC_SIGNED);
        assert_eq!(described(7, 0).0, X86_64_RELOC_SIGNED);
    }

    /// The field `write` leaves in an instruction: the 4 bytes after `prefix`
    /// in the object, for the instruction `image` with a relocation `from`
    /// bytes short of the place and `addend` past the symbol.
    fn written_field(
        image: Vec<u8>,
        prefix: &[u8],
        from: usize,
        addend: i64,
    ) -> i32 {
        let at = prefix.len();
        let reloc = Reloc::new(at, Fix::pc_rel(4, from), Target::Symbol(0), addend);
        let spans = vec![[0, image.len()]];
        let text = Section {
            name: "A_TEXT".into(),
            role: Role::Text,
            near: true,
            align: 16,
            image: image.clone(),
            spans,
            relocs: vec![reloc],
        };
        let symbol = Symbol {
            name: "x".into(),
            binding: Binding::Public,
            definition: Definition::Undefined,
            size: 0,
            group: None,
        };
        let object = Object {
            name: "a.c".into(),
            bits: 64,
            sections: vec![text],
            symbols: vec![symbol],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        };
        let bytes = llrm_macho::write(&object, &X8664MachO).unwrap();
        let tail = &image[at + 4..];
        let found = bytes.windows(image.len()).position(|w| w.starts_with(prefix) && w.ends_with(tail)).unwrap();
        i32::from_le_bytes(bytes[found + at..found + at + 4].try_into().unwrap())
    }

    /// The same two instructions write the same field, -1, as the pin clang's
    /// assembler leaves for both.
    #[test]
    fn two_instructions_with_the_same_total_addend_write_the_same_field() {
        let cmpb = written_field(vec![0x80, 0x3D, 0, 0, 0, 0, 1], &[0x80, 0x3D], 5, 0);
        let lea = written_field(vec![0x48, 0x8D, 0x05, 0, 0, 0, 0], &[0x48, 0x8D, 0x05], 4, -1);
        assert_eq!((cmpb, lea), (-1, -1));
    }
}
