use llrm_object::{Binding, Definition, Fix, Object, ObjectTarget, Role, Section, Symbol, Target};

use super::*;

/// An object with one function `f` of `code`.
fn with(code: &[u8]) -> Object {
    Object {
        name: "a.c".into(),
        bits: 64,
        sections: vec![Section {
            name: "A_TEXT".into(),
            role: Role::Text,
            near: true,
            align: 16,
            image: code.to_vec(),
            spans: vec![[0, code.len()]],
            relocs: Vec::new(),
        }],
        symbols: vec![Symbol {
            name: "f".into(),
            binding: Binding::Public,
            definition: Definition::Defined { section: 0, offset: 0 },
            size: code.len(),
            group: None,
        }],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        debug: None,
    }
}

/// The `.xdata` bytes of a function of `code`.
fn xdata(code: &[u8]) -> Vec<u8> {
    let object = expanded(&with(code)).unwrap();
    let section = object.sections.iter().find(|one| one.name == ".xdata").expect("an .xdata");
    section.image.clone()
}

/// A COFF x64 object had no `.pdata` or `.xdata`, so the Windows unwinder
/// found no frame to walk; the bytes below are derived by hand from the
/// UNWIND_INFO layout (version, prolog size, slot count, frame; the codes last
/// executed first, padded to an even slot count).
#[test]
fn push_and_a_small_allocation_are_two_codes_last_first() {
    // push rbp; sub rsp,32; ret
    assert_eq!(xdata(&[0x55, 0x48, 0x83, 0xEC, 0x20, 0xC3]), [0x01, 0x05, 0x02, 0x00, 0x05, 0x32, 0x01, 0x50]);
}

#[test]
fn a_frame_register_and_an_odd_slot_count_is_padded() {
    // push rbp; sub rsp,48; lea rbp,[rsp+32]; ret
    let code = [0x55, 0x48, 0x83, 0xEC, 0x30, 0x48, 0x8D, 0x6C, 0x24, 0x20, 0xC3];
    assert_eq!(xdata(&code), [0x01, 0x0A, 0x03, 0x25, 0x0A, 0x03, 0x05, 0x52, 0x01, 0x50, 0x00, 0x00]);
}

#[test]
fn a_large_allocation_and_a_register_save_take_an_operand_slot() {
    // sub rsp,4096; ret
    assert_eq!(
        xdata(&[0x48, 0x81, 0xEC, 0x00, 0x10, 0x00, 0x00, 0xC3]),
        [0x01, 0x07, 0x02, 0x00, 0x07, 0x01, 0x00, 0x02]
    );
    // mov [rsp+8],rbx; ret
    assert_eq!(xdata(&[0x48, 0x89, 0x5C, 0x24, 0x08, 0xC3]), [0x01, 0x05, 0x02, 0x00, 0x05, 0x34, 0x01, 0x00]);
}

#[test]
fn a_function_without_a_prologue_has_an_empty_unwind_info() {
    assert_eq!(xdata(&[0xC3]), [0x01, 0x00, 0x00, 0x00]);
}

#[test]
fn a_prologue_the_codes_cannot_say_is_refused() {
    // sub rsp,12
    let error = expanded(&with(&[0x48, 0x83, 0xEC, 0x0C, 0xC3])).err().expect("refused");
    assert!(error.0.contains("multiple of 8"), "{error}");
}

/// `.pdata` is one RUNTIME_FUNCTION per function: the begin, the end and the
/// unwind info as image-relative fields.
#[test]
fn pdata_names_the_function_and_its_unwind_info_by_image_relative_fields() {
    let object = expanded(&with(&[0x55, 0x48, 0x83, 0xEC, 0x20, 0xC3])).unwrap();
    let pdata = object.sections.iter().find(|one| one.name == ".pdata").expect("a .pdata");
    assert_eq!(pdata.image.len(), 12);
    let fields: Vec<_> = pdata.relocs.iter().map(|one| (one.at, one.spec, one.addend)).collect();
    assert_eq!(fields, [(0, Fix::S_IMGREL, 0), (4, Fix::S_IMGREL, 6), (8, Fix::S_IMGREL, 0)]);
    assert!(matches!(pdata.relocs[0].target, Target::Symbol(0)));
}

/// The written file names the sections `.pdata` and `.xdata`, and the three
/// fields are ADDR32NB relocations (type 3): a `.rdata$.pdata` name or an
/// ADDR32 type leaves the linker with no table, or one of unrebased
/// addresses.
#[test]
fn the_object_file_carries_pdata_and_xdata_with_addr32nb_relocations() {
    let bytes = crate::mc::X86Objects.write_object("coff", &with(&[0x55, 0x48, 0x83, 0xEC, 0x20, 0xC3])).unwrap();
    let has = |name: &[u8]| bytes.windows(name.len()).any(|window| window == name);
    assert!(has(b".pdata") && has(b".xdata") && !has(b".rdata$"));
    let sections = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    let pdata = (0..sections)
        .map(|index| 20 + 40 * index)
        .find(|&at| bytes[at..at + 6] == *b".pdata")
        .expect("a .pdata header");
    let relocs = u32::from_le_bytes(bytes[pdata + 24..pdata + 28].try_into().unwrap()) as usize;
    let types: Vec<u16> =
        (0..3).map(|n| u16::from_le_bytes([bytes[relocs + 10 * n + 8], bytes[relocs + 10 * n + 9]])).collect();
    assert_eq!(types, [3, 3, 3]);
}
