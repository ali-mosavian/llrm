use llrm_elf::{Machine, Relocation, write};
use llrm_object::{Binding, Definition, Fix, FixupKind, Object, Reloc, Role, Section, Symbol, Target, Unsupported};

/// A kind no crate here knows: `FIRST_TARGET + 99`.
const MADE_UP: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 99);

struct MadeUp;

impl Machine for MadeUp {
    const BITS: u8 = 64;
    const WIDE: bool = true;
    const NUMBER: u16 = 183;
    const ADDEND: bool = true;

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        match reloc.kind {
            MADE_UP => Ok(Relocation { kind: 0x1234, width: 4, from: 0 }),
            other => Err(Unsupported(format!("{other:?}"))),
        }
    }
}

fn object(fix: Fix) -> Object {
    Object {
        name: "made.s".into(),
        bits: 64,
        sections: vec![Section {
            name: ".text".into(),
            role: Role::Text,
            image: vec![0; 4],
            relocs: vec![Reloc::new(0, fix, Target::Symbol(0), 0)],
            align: 4,
            near: true,
            spans: vec![[0, 4]],
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

/// A target's own fixup kind reached the ELF writer only through the target's
/// `Machine`; before, the writer's crate had to name it.
#[test]
fn a_kind_only_the_target_knows_gets_the_targets_relocation_type() {
    let bytes = write::<MadeUp>(&object(Fix::plain(MADE_UP))).unwrap();
    assert!(bytes.windows(4).any(|word| word == 0x1234u32.to_le_bytes()), "the r_info type is in the file");
    assert!(write::<MadeUp>(&object(Fix::plain(FixupKind(MADE_UP.0 + 1)))).is_err());
}
