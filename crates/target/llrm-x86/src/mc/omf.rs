//! x86's OMF fixups: the kinds the OMF writer cannot name.

use llrm_object::{FixupKind, FixupKindInfo};
use llrm_omf::write::OmfTargetWriter;

use super::fixup;

pub struct X86Omf;

impl OmfTargetWriter for X86Omf {
    fn backend(&self) -> &dyn llrm_object::assembler::AsmBackend {
        &super::backend::FIELDS
    }

    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo {
        fixup::kind_info(kind)
    }

    fn is_branch(
        &self,
        kind: FixupKind,
    ) -> bool {
        matches!(kind, fixup::BRANCH_2 | fixup::BRANCH_4)
    }

    fn debug_target(
        &self,
        bits: u8,
    ) -> llrm_debug::DebugTarget {
        fixup::debug_target(bits)
    }
}

#[cfg(test)]
mod tests {
    use llrm_object::{Binding, Definition, Fix, Object, Reloc, Role, Section, Symbol, Target};

    use super::*;
    use crate::mc::fixup;

    fn object(fix: Fix) -> Object {
        Object {
            name: "t.asm".into(),
            bits: 32,
            sections: vec![Section {
                name: "_TEXT".into(),
                role: Role::Text,
                near: true,
                align: 1,
                image: vec![0xE8, 0, 0, 0, 0],
                spans: vec![[0, 5]],
                relocs: vec![Reloc::new(1, fix, Target::Symbol(0), 0)],
            }],
            symbols: vec![Symbol {
                name: "f".into(),
                binding: Binding::Public,
                definition: Definition::Undefined,
                size: 0,
                group: None,
            }],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// The OMF writer knew no x86 branch once the kind left the object crate,
    /// and refused every call: a call writes, a GOT-relative field (no OMF
    /// fixup) is refused.
    #[test]
    fn a_call_writes_and_a_got_field_is_refused() {
        assert!(llrm_omf::write::write(&object(fixup::branch(4)), None, &X86Omf).is_ok());
        assert!(llrm_omf::write::write(&object(fixup::got_pc_rel(4)), None, &X86Omf).is_err());
    }
}
