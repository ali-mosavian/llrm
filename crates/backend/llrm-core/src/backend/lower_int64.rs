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
