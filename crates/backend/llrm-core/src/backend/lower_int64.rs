//! Port of `qbopt/backend/lower_int64.py`: legalize MIR's whole 64-bit
//! integers for the 386 machine boundary.
//!
//! MIR keeps a C `long long` as one eight-byte value; this first
//! target-specific step gives the 16-bit ABI its two dword halves.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::abi::runtime::{self, Reg};

pub(crate) fn _helper(name: &str, inputs: BTreeSet<Reg>, clobbers: BTreeSet<Reg>) -> runtime::Contract {
    runtime::Contract {
        name: name.to_owned(),
        cleanup: Some(0),
        control: runtime::Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: runtime::Memory::None,
        reads: runtime::Memory::None,
        clobbers,
        established: true,
        evidence: "qbopt's inline 386 int64 helper; operands and results follow Open Watcom's register ABI".to_owned(),
        documented: None,
        inputs: Some(inputs),
        direct_inputs: None,
        clobbers_reached: true,
        caller_cleanup: 0,
        // Not a separately called 386 routine: `clobbers` describes the inline bytes exactly.
        i386: false,
        direct_writes: None,
        flags_result: false,
        direct_reads: None,
    }
}

/// `bytes.fromhex(text)`.
fn fromhex(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|byte| u8::from_str_radix(byte, 16).expect("a hex byte")).collect()
}

/// EDX:EAX / ECX:EBX, using the Open Watcom runtime's leading-bit division.
pub(crate) static _UDIV: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 09 c9 75 2a 66 4b 0f 84 c2 00 66 43 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 ",
        "66 f7 f3 66 91 66 f7 f3 66 89 d3 66 89 ca 66 31 c9 e9 9e 00 66 39 d1 72 28 75 19 ",
        "66 39 c3 77 14 66 29 d8 66 89 c3 66 31 c9 66 31 d2 66 b8 01 00 00 00 eb 7e 66 31 ",
        "c9 66 31 db 66 93 66 87 d1 eb 71 66 55 66 56 66 57 66 31 f6 66 89 f7 66 89 f5 ",
        "66 d1 e3 66 d1 d1 72 19 66 45 66 39 d1 72 f1 77 05 66 39 c3 76 ea f8 66 d1 d6 ",
        "66 d1 d7 66 4d 78 2f 66 d1 d9 66 d1 db 66 29 d8 66 19 ca f5 72 e7 66 d1 e6 66 d1 ",
        "d7 66 4d 78 10 66 d1 e9 66 d1 db 66 01 d8 66 11 ca 73 e8 eb cd 66 01 d8 66 11 ca ",
        "66 89 c3 66 89 d1 66 89 f0 66 89 fa 66 5f 66 5e 66 5d",
    ))
});

pub(crate) static _SDIV: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 09 d2 78 25 66 09 c9 78 06 e8 60 00 e9 27 01 66 f7 d9 66 f7 db 66 83 d9 00 e8 ",
        "50 00 66 f7 da 66 f7 d8 66 83 da 00 e9 0d 01 66 f7 da 66 f7 d8 66 83 da 00 66 09 c9 ",
        "79 1a 66 f7 d9 66 f7 db 66 83 d9 00 e8 27 00 66 f7 d9 66 f7 db 66 83 d9 00 e9 e4 ",
        "00 e8 17 00 66 f7 d9 66 f7 db 66 83 d9 00 66 f7 da 66 f7 d8 66 83 da 00 e9 ca 00 ",
        "66 09 c9 75 28 66 4b 0f 84 be 00 66 43 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 ",
        "d2 66 f7 f3 66 91 66 f7 f3 66 89 d3 66 89 ca 66 31 c9 c3 66 39 d1 72 26 75 18 ",
        "66 39 c3 77 13 66 29 d8 66 89 c3 66 31 c9 66 31 d2 66 b8 01 00 00 00 c3 66 31 ",
        "c9 66 31 db 66 93 66 87 d1 c3 66 55 66 56 66 57 66 31 f6 66 89 f7 66 89 f5 ",
        "66 d1 e3 66 d1 d1 72 19 66 45 66 39 d1 72 f1 77 05 66 39 c3 76 ea f8 66 d1 d6 ",
        "66 d1 d7 66 4d 78 2f 66 d1 d9 66 d1 db 66 29 d8 66 19 ca f5 72 e7 66 d1 e6 66 ",
        "d1 d7 66 4d 78 10 66 d1 e9 66 d1 db 66 01 d8 66 11 ca 73 e8 eb cd 66 01 d8 66 ",
        "11 ca 66 89 c3 66 89 d1 66 89 f0 66 89 fa 66 5f 66 5e 66 5d c3",
    ))
});

/// A compile-time dword divisor: at most two hardware divisions.
pub(crate) static _UDIV_CONST32: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex("66 31 c9 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 66 f7 f3 66 91 66 f7 f3 66 89 d3 66 89 ca 66 31 c9")
});

pub(crate) static _SDIV_CONST32: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 09 d2 78 24 66 31 c9 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 66 f7 f3 66 91 ",
        "66 f7 f3 66 89 d3 66 89 ca 66 31 c9 eb 40 66 f7 da 66 f7 d8 66 83 da 00 66 31 c9 ",
        "66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 66 f7 f3 66 91 66 f7 f3 66 89 d3 66 ",
        "89 ca 66 31 c9 66 f7 d9 66 f7 db 66 83 d9 00 66 f7 da 66 f7 d8 66 83 da 00",
    ))
});

pub(crate) fn _four_inputs() -> BTreeSet<Reg> {
    BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx])
}

pub(crate) fn _four_clobbers() -> BTreeSet<Reg> {
    let mut out = _four_inputs();
    out.insert(Reg::Flags);
    out
}

/// `bytes`, 386 code for a 16-bit segment, as the same instructions in a 32-bit one: each `66` operand-size prefix gone with
/// the instruction's own operand size, a `ret` and a near jump or call taking the wider forms, every branch aimed again.
/// Run as they were in a 32-bit segment the prefixes made 16-bit operations of them: a 64-bit remainder was garbage.
pub(crate) fn flat(bytes: &[u8]) -> Vec<u8> {
    use iced_x86::{BlockEncoder, BlockEncoderOptions, Code, Decoder, DecoderOptions, FlowControl, Instruction, InstructionBlock};
    let wide: crate::support::hash::HashMap<String, Code> = Code::values().map(|code| (format!("{code:?}"), code)).collect();
    let mut decoder = Decoder::with_ip(16, bytes, 0, DecoderOptions::NONE);
    let mut found = Vec::new();
    while decoder.can_decode() {
        let one = decoder.decode();
        assert!(!one.is_invalid(), "an instruction of the helper at {}", one.ip());
        let name = format!("{:?}", one.code());
        let flat = match one.flow_control() {
            FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch | FlowControl::Call if one.near_branch16() != 0 || name.contains("rel") => {
                let target = name.replace("_rel16", "_rel32_32").replace("_rel8_16", "_rel8_32");
                let mut branch = Instruction::with_branch(wide[&target], one.near_branch_target()).expect("a branch");
                branch.set_ip(one.ip());
                branch
            }
            FlowControl::Return => {
                let mut again = one;
                again.set_code(wide[&name.replace("Retnw", "Retnd")]);
                again
            }
            _ => {
                // The operand size is in the code: the same instruction encodes without the prefix at 32 bits.
                one
            }
        };
        found.push(flat);
    }
    // A branch to the end of the helper aims at what follows it: a nop there is the target, and is cut off after.
    let mut end = Instruction::with(Code::Nopd);
    end.set_ip(bytes.len() as u64);
    found.push(end);
    let block = InstructionBlock::new(&found, 0);
    let mut code = BlockEncoder::encode(32, block, BlockEncoderOptions::NONE).expect("the helper in 32 bits").code_buffer;
    code.pop();
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_x86::{Decoder, DecoderOptions, Formatter, NasmFormatter};

    /// The instructions of `bytes`, a branch's target as the index of the instruction it aims at (or their count, for the end).
    fn text(bytes: &[u8], bits: u32) -> Vec<String> {
        let mut decoder = Decoder::with_ip(bits, bytes, 0, DecoderOptions::NONE);
        let mut formatter = NasmFormatter::new();
        let mut found = Vec::new();
        while decoder.can_decode() {
            found.push(decoder.decode());
        }
        let index = |ip: u64| {
            let at = found.iter().position(|one| one.ip() == ip);
            assert!(at.is_some() || ip == bytes.len() as u64, "a branch aims at {ip:#x}, inside no instruction of {} bytes", bytes.len());
            at.unwrap_or(found.len())
        };
        found
            .iter()
            .map(|one| {
                let mut line = String::new();
                formatter.format(one, &mut line);
                match one.flow_control() {
                    iced_x86::FlowControl::UnconditionalBranch | iced_x86::FlowControl::ConditionalBranch | iced_x86::FlowControl::Call if one.near_branch_target() != 0 || line.contains("short") || line.contains("near") => {
                        format!("{} -> {}", line.split_whitespace().next().unwrap(), index(one.near_branch_target()))
                    }
                    _ => line,
                }
            })
            .collect()
    }

    /// Run in a 32-bit segment, the helpers' `66` prefixes made 16-bit operations of them: a 64-bit remainder by a variable
    /// was wrong at every level (torture 920501-2). Each helper's instructions are the same ones, without prefixes.
    #[test]
    fn test_the_division_helpers_are_the_same_instructions_at_32_bits() {
        for blob in [&*_UDIV, &*_SDIV, &*_UDIV_CONST32, &*_SDIV_CONST32] {
            let flat = flat(blob);
            assert_eq!(text(&flat, 32), text(blob, 16));
        }
    }
}
