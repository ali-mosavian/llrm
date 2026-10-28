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
        frames: Vec::new(),
    }
}

/// The `.xdata` bytes of a function of `code`.
fn xdata(code: &[u8]) -> Vec<u8> {
    let object = expanded(&with(code)).unwrap();
    let section = object.sections.iter().find(|one| one.name == ".xdata").expect("an .xdata");
    section.image.clone()
}

/// Function bodies and the `.xdata` that the pin's clang
/// (`clang -target x86_64-pc-windows-msvc -O1 [-fno-omit-frame-pointer] -c`)
/// writes for them: `(name, code, xdata)`. A decoder that read only
/// `[rsp+d]` saves stopped at the first `movaps [rbp-0x10]` of a function with
/// a frame register and wrote its unwind data without the xmm saves.
const PIN: &[(&str, &str, &str)] = &[
    ("plain_f", "564883ec4089ce488d4c2420e80000000089f04883c4405ec3", "0105020005720160"),
    ("fp_frame", "55564883ec48488d6c244089ce488d4de8e80000000089f04883c4485e5dc3", "010b04450b03068202600150"),
    (
        "fp_big",
        "55564881ec78020000488dac248000000089ce488d4da0e80000000089f04881c4780200005e5dc3",
        "01110585110309014f00026001500000",
    ),
    (
        "fp_saves",
        "5541565657534883ec30488d6c24304489c689d789cb4c8d75f04c89f1e8000000004c89f1e80000000001fb01f30fb645f00fbec001d84883c4305b5f5e415e5dc3",
        "010f07350f030a5206300570046003e001500000",
    ),
    (
        "fp_xmm",
        "554883ec60488d6c2460440f294df0440f2945e00f297dd00f2975c0660f28f1660f28f8e80000000066440f28c0660f28c6e80000000066440f28c8660f28c7f20f58c6e800000000f2450f59c8f2410f58c1f20f58c7f20f58c60f2875c00f287dd0440f2845e0440f284df04883c4605dc3",
        "011c0b651c68020018780300148804000f9805000a0305b201500000",
    ),
    ("nofp_big", "564881ec8002000089ce488d4c2428e80000000089f04881c4800200005ec3", "010803000801500001600000"),
    (
        "nofp_saves",
        "41565657534883ec384489c689d789cb4c8d7424284c89f1e8000000004c89f1e80000000001fb01f30fb64424280fbec001d84883c4385b5f5e415ec3",
        "01090500096205300470036002e00000",
    ),
    (
        "nofp_xmm",
        "4883ec68440f294c2450440f294424400f297c24300f29742420660f28f1660f28f8e80000000066440f28c0660f28c6e80000000066440f28c8660f28c7f20f58c6e800000000f2450f59c8f2410f58c1f20f58c7f20f58c60f287424200f287c2430440f28442440440f284c24504883c468c3",
        "011a09001a68020015780300108804000a98050004c20000",
    ),
];

fn bytes(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
}

#[test]
fn the_unwind_data_is_what_the_pins_clang_writes_for_the_same_code() {
    for (name, code, expected) in PIN {
        assert_eq!(xdata(&bytes(code)), bytes(expected), "{name}");
    }
}

/// The builder takes codes whoever made them. Bytes by hand from the Microsoft
/// x64 unwind spec (learn.microsoft.com/cpp/build/exception-handling-x64): the
/// brief's `push rbp; sub rsp,32`, a two-slot large allocation, and a far save.
#[test]
fn a_list_of_codes_is_encoded_last_first_with_operand_slots_after_theirs() {
    let info = |codes: Vec<(u8, Op)>, frame| Info {
        prolog: codes.last().map_or(0, |one| one.0),
        frame,
        codes: codes.into_iter().map(|(offset, op)| Code { offset, op }).collect(),
    };
    assert_eq!(
        info(vec![(1, Op::PushNonvol(5)), (5, Op::Alloc(32))], None).bytes().unwrap(),
        [0x01, 0x05, 0x02, 0x00, 0x05, 0x32, 0x01, 0x50]
    );
    // sub rsp,0x20000: 0x4000 qwords, one operand slot.
    assert_eq!(
        info(vec![(7, Op::Alloc(0x20000))], None).bytes().unwrap(),
        [0x01, 0x07, 0x02, 0x00, 0x07, 0x01, 0x00, 0x40]
    );
    // sub rsp,0x1000000: past 512K, two operand slots (low, high), padded.
    assert_eq!(
        info(vec![(7, Op::Alloc(0x1000000))], None).bytes().unwrap(),
        [0x01, 0x07, 0x03, 0x00, 0x07, 0x11, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]
    );
    // rbx saved at rsp+0x80000: past 512K, a far save.
    assert_eq!(
        info(vec![(9, Op::SaveNonvol(3, 0x80000))], None).bytes().unwrap(),
        [0x01, 0x09, 0x03, 0x00, 0x09, 0x35, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00]
    );
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

/// Directives a producer declares give the `.xdata` the pin's assembler writes
/// for the same `.seh_*`: a pushed register and a stack allocation, a frame
/// register with a push of the frame's own, and xmm saves below an allocation.
#[test]
fn declared_directives_are_encoded_as_the_pins_assembler_does() {
    use llrm_object::Seh::*;
    let cases: [(&str, Vec<(usize, llrm_object::Seh)>, &str); 3] = [
        ("plain", vec![(1, PushReg(6)), (5, StackAlloc(64))], "0105020005720160"),
        (
            "frame",
            vec![(1, PushReg(5)), (2, PushReg(6)), (6, StackAlloc(72)), (0xb, SetFrame(5, 64))],
            "010b04450b03068202600150",
        ),
        (
            "xmm",
            vec![(7, StackAlloc(168)), (0x10, SaveXmm(7, 0x90)), (0x19, SaveXmm(6, 0x80))],
            "01190600196808001078090007011500",
        ),
    ];
    for (name, directives, expected) in cases {
        let info = from_seh(name, &directives).unwrap();
        assert_eq!(info.bytes().unwrap(), bytes(expected), "{name}");
    }
}

/// A declared prologue wins over the bytes: code whose bytes say nothing
/// (here a lone `ret`) still gets the declared codes.
#[test]
fn a_declared_prologue_is_not_read_back_from_the_bytes() {
    use llrm_object::{Frame, Seh};
    let mut object = with(&[0xC3]);
    object.frames.push(Frame { section: 0, offset: 0, length: 1, rows: Vec::new(), seh: vec![(1, Seh::PushReg(6))] });
    let written = expanded(&object).unwrap();
    let xdata = written.sections.iter().find(|one| one.name == ".xdata").unwrap();
    assert_eq!(xdata.image, bytes("0101010001600000"));
}
