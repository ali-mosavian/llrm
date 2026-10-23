use std::collections::BTreeMap;

use llrm_dagisel::generated::x86_i486::MACHINE_OPCODES;
use llrm_target::Target;

use super::opcodes::{OpcodeMap, Unmapped};

fn map() -> OpcodeMap {
    let target = llrm_x86_m32::M32;
    let forms = llrm_instr::parse(&target.forms_text(), target.dialect()).expect("x86.instr parses");
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
/// `ADD32ri_DB` (an add of disjoint bits) is the form of `add`. It was
/// unmapped, and 5 m32 programs stopped at `Unmapped("ADD32ri_DB")`.
#[test]
fn an_add_of_disjoint_bits_is_an_add() {
    let map = map();
    for opcode in ["ADD32ri_DB", "ADD32rr_DB", "ADD16ri_DB", "ADD8rr_DB"] {
        assert_eq!(form_name(&map, opcode), "add", "{opcode}");
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
