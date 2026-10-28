use std::collections::BTreeMap;

use llrm_dagisel::generated::x86_i486::MACHINE_OPCODES;
use llrm_target::Target;

use super::opcodes::{OpcodeMap, Unmapped};

fn map() -> OpcodeMap {
    let target = llrm_x86_m32::M32;
    let forms = llrm_target::instr::parse(&target.forms_text(), target.dialect()).expect("x86.instr parses");
    OpcodeMap::new(forms)
}

fn form_name(
    map: &OpcodeMap,
    opcode: &str,
) -> String {
    map.forms()[map.form(opcode).unwrap_or_else(|e| panic!("{opcode}: {e:?}"))].name.clone()
}

#[test]
fn common_integer_opcodes_are_their_forms() {
    let map = map();
    for (opcode, name) in [
        ("ADD32rr", "add"),
        ("ADD32ri8", "add"),
        ("ADD32mr", "add"),
        ("ADD32rm", "add"),
        ("SUB8mi", "sub"),
        ("MOV32rm", "mov"),
        ("MOV32mr", "mov"),
        ("MOV32ri", "mov"),
        ("MOVZX32rr8", "movzx"),
        ("IMUL32rr", "imul"),
        ("IMUL32rri", "imul"),
        ("SHL32rCL", "shl"),
        ("SHL32ri", "shl"),
    ] {
        assert_eq!(form_name(&map, opcode), name, "{opcode}");
    }
}

/// `IMUL32rri` (two untied sources) is the three-operand form, not the tied
/// two-operand one: the tie is in the form, so the letters count differently.
/// `ADD32ri_DB` (an add of disjoint bits) is the form of `or`, as the pin
/// prints it. It was unmapped, and 5 m32 programs stopped at
/// `Unmapped("ADD32ri_DB")`; then it was an `add`, where the pin emits `orl`.
#[test]
fn an_add_of_disjoint_bits_is_an_or() {
    let map = map();
    for opcode in ["ADD32ri_DB", "ADD32rr_DB", "ADD16ri_DB", "ADD8rr_DB"] {
        assert_eq!(form_name(&map, opcode), "or", "{opcode}");
    }
}

#[test]
fn a_tied_source_has_no_letter() {
    let map = map();
    let two = map.form("IMUL32rr").unwrap();
    let three = map.form("IMUL32rri").unwrap();
    assert_ne!(two, three);
    assert_eq!(map.forms()[three].sources[0].tied, None);
}

/// A guess at a condition-coded opcode would pick one of sixteen forms.
#[test]
fn condition_coded_opcodes_are_unmapped() {
    assert_eq!(map().form("CMOV32rr"), Err(Unmapped::Condition));
}

/// Every opcode of the pruned i486 set is one form or unmapped for a named
/// reason, and none is ambiguous; the unmapped names are in
/// `opcodes_unmapped.txt`, so a new one is a change someone reads.
#[test]
fn every_opcode_of_the_i486_set_is_one_form_or_listed() {
    let map = map();
    let mut mapped = 0;
    let mut listed: BTreeMap<String, String> = BTreeMap::new();
    for &(_, name) in MACHINE_OPCODES {
        match map.form(name) {
            Ok(_) => mapped += 1,
            Err(Unmapped::Ambiguous(rows)) => panic!("{name} is {} forms: {rows:?}", rows.len()),
            Err(Unmapped::NoForm | Unmapped::Name) => {
                listed.insert(name.to_owned(), "other".to_owned());
            }
            Err(_) => {}
        }
    }
    assert!(mapped > 100, "{mapped} opcodes map");
    let text: String = listed.keys().map(|n| format!("{n}\n")).collect();
    let expected = include_str!("opcodes_unmapped.txt");
    assert_eq!(text, expected, "unmapped opcodes changed ({mapped} map)");
}

/// `MOVZX32rr16` is the form of `movzx`, 16 to 32: its `16` is the source
/// size, not a `1` and a `6`. It was read as register, register, immediate
/// and refused (18 + 11 + 1 functions of the corpora).
#[test]
fn a_size_after_the_letters_is_not_an_operand() {
    let map = map();
    for name in ["MOVZX32rr16", "MOVSX32rr16", "MOVZX32rm16", "MOVSX32rm16"] {
        assert_eq!(
            map.forms()[map.form(name).unwrap_or_else(|e| panic!("{name}: {e:?}"))].name,
            name[..5].to_lowercase()
        );
    }
    assert_eq!(map.forms()[map.form("SHL32r1").unwrap()].name, "shl", "a lone 1 is still the shift by one");
}

/// A store of a word is a word-wide cell: `mov [base+index], v` took the
/// width of the node's first value, the chain, and wrote four bytes from a
/// 16-bit register (two modules of the vsgcc corpus stopped compiling).
#[test]
fn a_memory_operand_has_the_width_the_name_gives() {
    use super::opcodes::memory_bytes;
    assert_eq!(
        [("MOV16mr", 2), ("MOV8mi", 1), ("MOV32rm", 4), ("ADD32mi8", 4), ("MOVZX32rm8", 1), ("MOVSX32rm16", 2)]
            .map(|(name, _)| memory_bytes(name)),
        [Some(2), Some(1), Some(4), Some(4), Some(1), Some(2)]
    );
}

fn map64() -> OpcodeMap {
    let target = llrm_x86_m64::M64;
    OpcodeMap::new(llrm_target::instr::parse(&target.forms_text(), target.dialect()).expect("x86.instr parses"))
}

/// The pin names the scalar SSE instructions `ADDSDrr`, `MOVSDrm_alt`,
/// `MOVSSmr`: the width is in the mnemonic (`SS` 32, `SD` 64), `_alt` marks the
/// load that may be rematerialised, and `r` also names an xmm register. Every
/// one stopped at `Unmapped("MOVSDrm_alt")` (the argument and constant loads),
/// so no floating-point function got through emit at m64; `MOVSD` is the string
/// copy in our table, the scalar move is `movsd_x`.
#[test]
fn the_pins_scalar_sse_opcodes_are_their_forms() {
    let map = map64();
    for (opcode, name) in [
        ("ADDSDrr", "addsd"),
        ("ADDSSrm", "addss"),
        ("SUBSDrr", "subsd"),
        ("MULSSrr", "mulss"),
        ("DIVSDrm", "divsd"),
        ("MINSDrr", "minsd"),
        ("MAXSSrr", "maxss"),
        ("SQRTSDr", "sqrtsd"),
        ("MOVSDrm_alt", "movsd_x"),
        ("MOVSDrm", "movsd_x"),
        ("MOVSDrr", "movsd_x"),
        ("MOVSDmr", "movsd_x"),
        ("MOVSSrm_alt", "movss"),
        ("MOVSSmr", "movss"),
        ("UCOMISDrr", "ucomisd"),
        ("UCOMISSrm", "ucomiss"),
        ("CVTSS2SDrr", "cvtss2sd"),
        ("CVTSD2SSrm", "cvtsd2ss"),
        ("CVTSI2SDrr", "cvtsi2sd"),
        ("XORPSrr", "xorps"),
        ("ANDPSrm", "andps"),
        ("ANDNPSrr", "andnps"),
        ("ORPSrr", "orps"),
        ("MOVAPSrr", "movaps"),
    ] {
        assert_eq!(form_name(&map, opcode), name, "{opcode}");
    }
}

/// `CVTTSD2SIrr` and `CVTTSD2SI64rr` are the 32- and 64-bit result of one
/// mnemonic; both rows state the source's width (64), so the name was
/// `Ambiguous` and every float-to-integer conversion at m64 was refused.
#[test]
fn a_truncating_conversion_picks_its_result_width_by_the_name() {
    let map = map64();
    for (opcode, iced) in [
        ("CVTTSD2SIrr", "Cvttsd2si_r32_xmmm64"),
        ("CVTTSD2SI64rr", "Cvttsd2si_r64_xmmm64"),
        ("CVTTSD2SIrm", "Cvttsd2si_r32_xmmm64"),
        ("CVTTSS2SIrr", "Cvttss2si_r32_xmmm32"),
        ("CVTTSS2SI64rm", "Cvttss2si_r64_xmmm32"),
    ] {
        let form = &map.forms()[map.form(opcode).unwrap_or_else(|e| panic!("{opcode}: {e:?}"))];
        assert_eq!(form.iced.as_deref(), Some(iced), "{opcode}");
    }
}

/// The memory operand of a conversion is its source's size: `CVTTSD2SIrm` reads
/// 8 bytes, `CVTSI2SDrm` 4, `CVTSI642SDrm` 8, `ANDPSrm` the 16 of a register.
#[test]
fn a_conversions_memory_operand_is_its_sources_size() {
    use super::opcodes::memory_bytes;
    assert_eq!(
        [
            "CVTTSD2SIrm",
            "CVTTSS2SI64rm",
            "CVTSS2SDrm",
            "CVTSD2SSrm",
            "CVTSI2SDrm",
            "CVTSI642SDrm",
            "ANDPSrm",
            "MOVSDrm"
        ]
        .map(memory_bytes),
        [Some(8), Some(4), Some(4), Some(8), Some(4), Some(8), Some(16), Some(8)]
    );
}

/// The second of two byte stores of `o` and `o >> 8` is `MOV8mr_NOREX` (the
/// store of `AH`); its name has no width the parser reads, so its cell was a
/// dword and the encoder refused a byte register stored into it.
#[test]
fn a_norex_byte_store_is_one_byte() {
    assert_eq!(super::opcodes::memory_bytes("MOV8mr_NOREX"), Some(1));
    assert_eq!(super::opcodes::memory_bytes("MOV8mr"), Some(1));
}
