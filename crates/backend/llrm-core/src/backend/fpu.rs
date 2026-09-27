//! The emulator's interrupt, replaced by the instruction it stands for.
//!
//! Port of `qbopt/backend/fpu.py`. Under /FPi BC emits Open Watcom's emulator
//! interrupts, and the real opcode lives inside them:
//!
//! ```text
//! cd 35 46 c8     int 35h, operand inline   ->   d9 46 c8   fld dword [bp-38h]
//! ```
//!
//! `wrapped` restores the protocol after selecting new operands; `native`
//! refuses int 3Ch, whose override is not in the bytes it can see.

use crate::backend::select::Emitted;
use crate::frontends::bc::declen::{EMULATED, ESC, INTERRUPT, Insn, Stands, stood_in_for};

/// Reapply an existing emulator protocol to newly selected x87 bytes.
pub fn wrapped(made: &Emitted, protocol: u8) -> Option<Emitted> {
    let code = &made.code;
    if code.is_empty() {
        return None;
    }
    let (prefix, tail, shift): ([u8; 2], &[u8], usize);
    if protocol == Stands::Fwait as u8 {
        if code.as_slice() != [0x9B] {
            return None;
        }
        (prefix, tail, shift) = ([INTERRUPT, Stands::Fwait as u8], &[], 1);
    } else if protocol == Stands::Segmented as u8 {
        if code[0] == 0x26 && code.len() > 1 && ESC.contains(&code[1]) {
            (prefix, tail, shift) = ([INTERRUPT, Stands::Segmented as u8], &code[1..], 1);
        } else if ESC.contains(&code[0]) {
            (prefix, tail, shift) = ([INTERRUPT, Stands::Segmented as u8], &code[..], 2);
        } else {
            return None;
        }
    } else if EMULATED.contains(&protocol) && code[0] == 0x26 && code.len() > 1 && ESC.contains(&code[1]) {
        // A runtime conversion has no instruction-site protocol to preserve;
        // an ES-relative operand takes the 3Ch prefix protocol.
        (prefix, tail, shift) = ([INTERRUPT, Stands::Segmented as u8], &code[1..], 1);
    } else if EMULATED.contains(&protocol) && ESC.contains(&code[0]) {
        (prefix, tail, shift) = ([INTERRUPT, EMULATED.start + (code[0] - ESC.start)], &code[1..], 1);
    } else {
        return None;
    }
    Some(Emitted {
        code: [&prefix[..], tail].concat(),
        displacement_at: made.displacement_at.map(|at| at + shift),
        immediate_at: made.immediate_at.map(|at| at + shift),
        fields: made.fields.iter().map(|field| field + shift).collect(),
        ..made.clone()
    })
}

/// Whether `at` is an emulator site at all.
pub fn emulated_at(code: &[u8], at: usize) -> bool {
    code.get(at) == Some(&INTERRUPT) && code.len() > at + 1
}

/// The real instruction this emulated site stands for, or None.
///
/// None where the site is not emulated, where it is the segment-override
/// form, or where the bytes do not decode back to the same instruction.
pub fn native(code: &[u8], insn: &Insn) -> Option<Vec<u8>> {
    let at = insn.at;
    if !emulated_at(code, at) || !EMULATED.contains(&code[at + 1]) && code[at + 1] != Stands::Fwait as u8 {
        return None;
    }
    if code[at + 1] == Stands::Segmented as u8 {
        return None;
    }

    let (stood_in, hidden) = stood_in_for(code, at)?;
    if hidden != 1 {
        return None;
    }

    // The decoded instruction's own length, out of the buffer stood_in_for
    // built, which carries trailing bytes so iced can decode.
    let wanted = insn.length - 2 + hidden;
    let out = &stood_in[..wanted.min(stood_in.len())];
    if out.len() != wanted || out.is_empty() || !ESC.contains(&out[0]) && out[0] != 0x9B {
        return None;
    }
    Some(out.to_vec())
}

#[cfg(test)]
mod tests {
    //! Port of tests/test_fpu.py's tests that need no corpus. A `str(insn)`
    //! check reads iced's decoded fields: iced is built without a formatter.

    use super::*;
    use crate::frontends::bc::declen::decode;
    use iced_x86::{Code, Mnemonic, Register};

    const WAIT: u8 = 0x9B;

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex")).collect()
    }

    #[test]
    fn test_emulator_reencoding_moves_relocation_fields() {
        for (native, protocol, wanted, shift) in [
            ("d9860000", 0x35, "cd35860000", 1),
            ("dd860000", 0x39, "cd39860000", 1),
            ("d9860000", 0x3c, "cd3cd9860000", 2),
            ("26d9860000", 0x3c, "cd3cd9860000", 1),
        ] {
            let displacement = bytes(native).len() - 2;
            let emitted =
                Emitted { displacement_at: Some(displacement), fields: vec![displacement], ..Emitted::new(bytes(native)) };
            let made = wrapped(&emitted, protocol).expect("wrapped");
            assert_eq!(made.code, bytes(wanted));
            assert_eq!(made.displacement_at, Some(displacement + shift));
            assert_eq!(made.fields, [displacement + shift]);
        }
    }

    #[test]
    fn test_emulator_wait_remains_an_emulator_wait() {
        assert_eq!(wrapped(&Emitted::new(vec![0x9b]), 0x3d).expect("wrapped").code, [0xcd, 0x3d]);
        assert!(wrapped(&Emitted::new(vec![0x67, 0xd9, 0x00]), 0x35).is_none());
        assert!(wrapped(&Emitted::new(vec![0x36, 0xd9, 0x07]), 0x3c).is_none());
    }

    #[test]
    fn test_a_hand_built_site_converts_to_the_documented_bytes() {
        let sites: [(Vec<u8>, Option<(Mnemonic, Code)>, Vec<u8>); 3] = [
            // "fld dword ptr [bp-38h]"
            (vec![INTERRUPT, 0x35, 0x46, 0xC8], Some((Mnemonic::Fld, Code::Fld_m32fp)), vec![0xD9, 0x46, 0xC8]),
            (vec![INTERRUPT, 0x39, 0x04], None, vec![0xDD, 0x04]),
            // "wait"
            (vec![INTERRUPT, Stands::Fwait as u8], Some((Mnemonic::Wait, Code::Wait)), vec![WAIT]),
        ];
        for (raw, text, want) in sites {
            let found = decode(&raw, 0).expect("declen decoded something");
            assert_eq!(native(&raw, &found), Some(want));
            if let Some((mnemonic, code)) = text {
                assert_eq!((found.insn.mnemonic(), found.insn.code()), (mnemonic, code));
                if code == Code::Fld_m32fp {
                    let displacement = found.insn.memory_displacement64() as i16;
                    assert_eq!((found.insn.memory_base(), displacement), (Register::BP, -0x38));
                }
            }
        }
    }

    #[test]
    fn test_a_hand_built_segment_override_is_refused() {
        let raw = [INTERRUPT, Stands::Segmented as u8, 0xD9, 0x06, 0x00, 0x00];
        let found = decode(&raw, 0).expect("decodes");
        assert!(native(&raw, &found).is_none());
    }

    #[test]
    fn test_an_ordinary_interrupt_is_refused() {
        let raw = [INTERRUPT, 0x21, 0x90, 0x90];
        let found = decode(&raw, 0).expect("decodes");
        assert!(native(&raw, &found).is_none());
    }
}
