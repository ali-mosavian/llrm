//! Object-code instruction lengths and operand locations.
//!
//! Port of `qbopt/frontend/declen.py:{Insn,to_signed,stood_in_for,emulated,
//! decode,length,run}`. The Python decoder is the specification: this keeps
//! the iced instruction and records the object-file offsets that fixups use.

use std::ops::{Range, RangeInclusive};

use iced_x86::{
    Code, Decoder, DecoderOptions, FlowControl, Instruction, InstructionInfoFactory, OpAccess,
    OpKind, Register,
};

/// BC targets an 8086; rewritten instructions use 386 forms in a 16-bit segment.
pub const BITNESS: u32 = 16;
pub const MEMORY: OpKind = OpKind::Memory;
pub const NO_REGISTER: Register = Register::None;

/// Emulator interrupt numbers which stand in for x87 ESC opcodes.
pub const EMULATED: Range<u8> = 0x34..0x3C;

/// Emulator interrupts which stand in for a prefix or a complete WAIT.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Stands {
    Segmented = 0x3C,
    Fwait = 0x3D,
}

pub const WAIT: u8 = 0x9B;
pub const ESC: Range<u8> = 0xD8..0xE0;
pub const INTERRUPT: u8 = 0xCD;
pub const STANDS_IN: RangeInclusive<u8> = 0x34..=0x3D;

/// Iced accesses that write an operand, matching Python's `WRITES`.
pub const WRITES: [OpAccess; 4] = [
    OpAccess::Write,
    OpAccess::ReadWrite,
    OpAccess::CondWrite,
    OpAccess::ReadCondWrite,
];

/// Iced accesses that read an operand, matching Python's `READS`.
pub const READS: [OpAccess; 4] = [
    OpAccess::Read,
    OpAccess::ReadWrite,
    OpAccess::CondRead,
    OpAccess::ReadCondWrite,
];

/// Makes the scoped iced access-fact factory used by callers needing it.
///
/// Python keeps one mutable module-global factory. Rust deliberately makes its
/// ownership explicit: `InstructionInfoFactory::info()` mutates its reusable
/// buffers, and a global mutable instance would be unsynchronised.
#[must_use]
pub fn instruction_info_factory() -> InstructionInfoFactory {
    InstructionInfoFactory::new()
}

/// A decoded object-code instruction and the operand fields inside its bytes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Insn {
    pub at: usize,
    /// Kept independently from `insn.len()`: emulated sites differ from iced's bytes.
    pub length: usize,
    pub insn: Instruction,
    pub disp_at: Option<usize>,
    pub disp_len: usize,
    pub imm_at: Option<usize>,
    pub imm_len: usize,
}

impl Insn {
    #[must_use]
    pub const fn end(&self) -> usize {
        self.at + self.length
    }

    #[must_use]
    pub const fn code(&self) -> Code {
        self.insn.code()
    }

    #[must_use]
    pub fn flow(&self) -> FlowControl {
        self.insn.flow_control()
    }

    #[must_use]
    pub fn reads(&self) -> u32 {
        self.insn.rflags_read()
    }

    /// Every flag this leaves other than as it found it.
    #[must_use]
    pub fn writes(&self) -> u32 {
        self.insn.rflags_written()
            | self.insn.rflags_cleared()
            | self.insn.rflags_set()
            | self.insn.rflags_undefined()
    }

    /// Where a self-relative branch goes, or `None` if it is not one.
    #[must_use]
    pub fn target(&self) -> Option<u64> {
        match self.insn.op0_kind() {
            OpKind::NearBranch16 | OpKind::NearBranch32 => Some(self.insn.near_branch_target()),
            _ => None,
        }
    }

    #[must_use]
    pub const fn memory_base(&self) -> Register {
        self.insn.memory_base()
    }

    #[must_use]
    pub const fn memory_index(&self) -> Register {
        self.insn.memory_index()
    }

    /// The displacement as the instruction means it, sign and all.
    #[must_use]
    pub fn displacement(&self) -> i64 {
        if self.disp_len == 0 {
            0
        } else {
            to_signed(self.insn.memory_displacement64(), (BITNESS / 8) as usize)
        }
    }

    #[must_use]
    pub fn reads_memory(&self, operand: usize) -> bool {
        (if operand == 0 {
            self.insn.op0_kind()
        } else {
            self.insn.op1_kind()
        }) == MEMORY
    }

    #[must_use]
    pub fn segment_override(&self) -> Register {
        self.insn.segment_prefix()
    }

    #[must_use]
    pub fn has_segment_override(&self) -> bool {
        self.segment_override() != NO_REGISTER
    }

    #[must_use]
    pub fn register(&self, operand: usize) -> Register {
        if operand == 0 {
            self.insn.op0_register()
        } else {
            self.insn.op1_register()
        }
    }
}

/// Reads a `width`-byte raw field as two's complement.
#[must_use]
pub fn to_signed(raw: u64, width: usize) -> i64 {
    let bits = width * 8;
    let sign = 1_u64 << (bits - 1);
    if raw >= sign {
        (raw as i128 - (1_i128 << bits)) as i64
    } else {
        raw as i64
    }
}

/// Bytes an emulator interrupt means, and how many bytes it hides.
#[must_use]
pub fn stood_in_for(code: &[u8], at: usize) -> Option<(Vec<u8>, usize)> {
    let escape = *code.get(at + 1)?;
    let tail_end = (at + 2 + 15).min(code.len());
    let tail = &code[at + 2..tail_end];
    match escape {
        escape if EMULATED.contains(&escape) => {
            let mut stood_in = Vec::with_capacity(tail.len() + 1);
            stood_in.push(ESC.start + (escape - EMULATED.start));
            stood_in.extend_from_slice(tail);
            Some((stood_in, 1))
        }
        escape
            if escape == Stands::Segmented as u8
                && tail.first().is_some_and(|opcode| ESC.contains(opcode)) =>
        {
            Some((tail.to_vec(), 0))
        }
        escape if escape == Stands::Fwait as u8 => Some((vec![WAIT], 1)),
        _ => None,
    }
}

/// Decodes an x87 instruction wearing the emulator's interrupt as its first byte.
#[must_use]
pub fn emulated(code: &[u8], at: usize) -> Option<Insn> {
    let (stood_in, hidden) = stood_in_for(code, at)?;
    let mut decoder = Decoder::with_ip(BITNESS, &stood_in, 0, DecoderOptions::NONE);
    if !decoder.can_decode() {
        return None;
    }
    let insn = decoder.decode();
    if insn.is_invalid() {
        return None;
    }
    let length = 2 + insn.len() - hidden;
    if at.checked_add(length)? > code.len() {
        return None;
    }

    let operand = at + 2 - hidden;
    let where_ = decoder.get_constant_offsets(&insn);
    Some(Insn {
        at,
        length,
        insn,
        disp_at: where_
            .has_displacement()
            .then(|| operand + where_.displacement_offset()),
        disp_len: if where_.has_displacement() {
            where_.displacement_size()
        } else {
            0
        },
        imm_at: where_
            .has_immediate()
            .then(|| operand + where_.immediate_offset()),
        imm_len: if where_.has_immediate() {
            where_.immediate_size()
        } else {
            0
        },
    })
}

/// Decodes the instruction at `at`, if the bytes contain one.
#[must_use]
pub fn decode(code: &[u8], at: usize) -> Option<Insn> {
    if at >= code.len() {
        return None;
    }
    if code[at] == INTERRUPT
        && code
            .get(at + 1)
            .is_some_and(|stand_in| STANDS_IN.contains(stand_in))
    {
        if let Some(found) = emulated(code, at) {
            return Some(found);
        }
        // Only INT 3Ch can be ordinary: it represents a prefix only when an
        // ESC opcode actually follows it. The other emulator escapes hide an
        // operand, so treating them as INT would desynchronise the walk.
        if code[at + 1] != Stands::Segmented as u8 {
            return None;
        }
    }
    let mut decoder = Decoder::with_ip(BITNESS, &code[at..], at as u64, DecoderOptions::NONE);
    if !decoder.can_decode() {
        return None;
    }
    let insn = decoder.decode();
    if insn.is_invalid() || at.checked_add(insn.len())? > code.len() {
        return None;
    }

    let where_ = decoder.get_constant_offsets(&insn);
    Some(Insn {
        at,
        length: insn.len(),
        insn,
        disp_at: where_
            .has_displacement()
            .then(|| at + where_.displacement_offset()),
        disp_len: if where_.has_displacement() {
            where_.displacement_size()
        } else {
            0
        },
        imm_at: where_
            .has_immediate()
            .then(|| at + where_.immediate_offset()),
        imm_len: if where_.has_immediate() {
            where_.immediate_size()
        } else {
            0
        },
    })
}

/// The instruction length at `at`, if it decodes.
#[must_use]
pub fn length(code: &[u8], at: usize) -> Option<usize> {
    decode(code, at).map(|insn| insn.length)
}

/// Every instruction from `start`, and the offset where decoding gave up.
#[must_use]
pub fn run(code: &[u8], start: usize, end: usize) -> (Vec<Insn>, Option<usize>) {
    let mut found = Vec::new();
    let mut at = start;
    while at < end {
        let Some(insn) = decode(code, at) else {
            return (found, Some(at));
        };
        if insn.end() > end {
            return (found, Some(at));
        }
        at = insn.end();
        found.push(insn);
    }
    (found, None)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process::{Command, Stdio};

    use iced_x86::{Code, FlowControl, Register};

    use super::{
        BITNESS, EMULATED, ESC, INTERRUPT, MEMORY, NO_REGISTER, READS, STANDS_IN, Stands, WAIT,
        WRITES, decode, length, run,
    };

    fn hx(text: &str) -> Vec<u8> {
        text.split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }

    /// CPython's `random.seed(20260828); random.randrange(256)` stream.
    ///
    /// `randrange(256)` uses a nine-bit `getrandbits()` draw and rejects 256
    /// through 511; using a byte-oriented MT API would be a different corpus.
    struct PythonMt19937 {
        state: [u32; 624],
        index: usize,
    }

    impl PythonMt19937 {
        fn seeded(seed: u32) -> Self {
            let mut state = [0; 624];
            state[0] = 19650218;
            for index in 1..624 {
                state[index] = 1812433253_u32
                    .wrapping_mul(state[index - 1] ^ (state[index - 1] >> 30))
                    .wrapping_add(index as u32);
            }
            let key = [seed];
            let (mut index, mut key_index) = (1, 0);
            for _ in 0..624.max(key.len()) {
                state[index] = (state[index]
                    ^ (state[index - 1] ^ (state[index - 1] >> 30)).wrapping_mul(1664525))
                .wrapping_add(key[key_index])
                .wrapping_add(key_index as u32);
                index += 1;
                key_index += 1;
                if index == 624 {
                    state[0] = state[623];
                    index = 1;
                }
                if key_index == key.len() {
                    key_index = 0;
                }
            }
            for _ in 0..623 {
                state[index] = (state[index]
                    ^ (state[index - 1] ^ (state[index - 1] >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(index as u32);
                index += 1;
                if index == 624 {
                    state[0] = state[623];
                    index = 1;
                }
            }
            state[0] = 0x8000_0000;
            Self { state, index: 624 }
        }

        fn next_u32(&mut self) -> u32 {
            if self.index == 624 {
                for index in 0..624 {
                    let y = (self.state[index] & 0x8000_0000)
                        | (self.state[(index + 1) % 624] & 0x7FFF_FFFF);
                    self.state[index] = self.state[(index + 397) % 624]
                        ^ (y >> 1)
                        ^ if y & 1 == 0 { 0 } else { 0x9908_B0DF };
                }
                self.index = 0;
            }
            let mut value = self.state[self.index];
            self.index += 1;
            value ^= value >> 11;
            value ^= (value << 7) & 0x9D2C_5680;
            value ^= (value << 15) & 0xEFC6_0000;
            value ^ (value >> 18)
        }

        fn randrange_byte(&mut self) -> u8 {
            loop {
                let value = self.next_u32() >> 23; // Python's getrandbits(9)
                if value < 256 {
                    return value as u8;
                }
            }
        }
    }

    fn python_random_bytes() -> Vec<u8> {
        let mut random = PythonMt19937::seeded(20260828);
        (0..4000).map(|_| random.randrange_byte()).collect()
    }

    #[test]
    fn forms_bc_and_the_runtime_emit_have_the_right_lengths() {
        let forms = [
            ("A1 5E 00", 3),
            ("66 A1 5E 00", 4),
            ("8B 16 60 00", 4),
            ("8B 46 E8", 3),
            ("8B 86 00 01", 4),
            ("8B C1", 2),
            ("83 D2 00", 3),
            ("81 C2 00 01", 4),
            ("66 81 C2 00 01 00 00", 7),
            ("F7 D8", 2),
            ("F7 06 5E 00 34 12", 6),
            ("F6 06 5E 00 34", 5),
            ("0F 85 1A 16", 4),
            ("0F B6 C1", 3),
            ("0F A4 C1 04", 4),
            ("66 FF 36 5E 00", 5),
            ("9A 0F 00 15 00", 5),
            ("EA 0F 00 15 00", 5),
            ("C2 08 00", 3),
            ("C8 04 00 00", 4),
            ("C4 46 E8", 3),
            ("26 8A 07", 3),
            ("F3 A4", 2),
            ("67 66 8D 04 80", 5),
        ];
        for (enc, want) in forms {
            assert_eq!(length(&hx(enc), 0), Some(want), "{enc}");
        }
    }

    #[test]
    fn operand_fields_sit_at_the_iced_offsets() {
        for (enc, disp_at, disp_len, imm_at, imm_len) in [
            ("66 A1 5E 00", Some(2), 2, None, 0),
            ("8B 46 E8", Some(2), 1, None, 0),
            ("66 C7 06 00 00 78 56 34 12", Some(3), 2, Some(5), 4),
            ("7D 09", None, 0, Some(1), 1),
            ("8B C1", None, 0, None, 0),
        ] {
            let insn = decode(&hx(enc), 0).unwrap();
            assert_eq!(
                (insn.disp_at, insn.disp_len, insn.imm_at, insn.imm_len),
                (disp_at, disp_len, imm_at, imm_len)
            );
        }
    }

    #[test]
    fn flags_include_read_written_and_undefined() {
        for (enc, reads, writes) in [
            ("74 02", true, false),
            ("23 06 5A 00", false, true),
            ("8B C1", false, false),
            ("66 50", false, false),
            ("D1 E0", false, true),
        ] {
            let insn = decode(&hx(enc), 0).unwrap();
            assert_eq!(insn.reads() != 0, reads, "{enc}");
            assert_eq!(insn.writes() != 0, writes, "{enc}");
        }
        let shift = decode(&hx("D3 E0"), 0).unwrap();
        assert_ne!(shift.insn.rflags_undefined(), 0);
        assert_eq!(
            shift.writes() & shift.insn.rflags_undefined(),
            shift.insn.rflags_undefined()
        );
    }

    #[test]
    fn operand_access_sets_and_operand_helpers_match_python() {
        assert_eq!(BITNESS, 16);
        assert_eq!(MEMORY, iced_x86::OpKind::Memory);
        assert_eq!(NO_REGISTER, Register::None);
        assert_eq!(EMULATED, 0x34..0x3C);
        assert_eq!(Stands::Segmented as u8, 0x3C);
        assert_eq!(Stands::Fwait as u8, 0x3D);
        assert_eq!(WAIT, 0x9B);
        assert_eq!(ESC, 0xD8..0xE0);
        assert_eq!(INTERRUPT, 0xCD);
        assert_eq!(STANDS_IN, 0x34..=0x3D);
        assert_eq!(READS.len(), 4);
        assert_eq!(WRITES.len(), 4);
        let load = decode(&hx("26 8A 07"), 0).unwrap();
        assert!(load.reads_memory(1));
        assert!(!load.reads_memory(0));
        assert_eq!(load.register(0), Register::AL);
        assert_eq!(load.segment_override(), Register::ES);
        assert!(load.has_segment_override());
        let addressed = decode(&hx("8B 46 E8"), 0).unwrap();
        assert_eq!(addressed.memory_base(), Register::BP);
        assert_eq!(addressed.memory_index(), Register::None);
        assert_eq!(addressed.displacement(), -24);
    }

    #[test]
    fn invalid_encodings_decode_to_nothing() {
        for enc in ["FF FF", "0F FF", "C4 C0"] {
            assert_eq!(decode(&hx(enc), 0), None, "{enc}");
        }
    }

    #[test]
    fn truncation_never_runs_off_the_end() {
        for op in ["8B", "A1", "81", "9A", "0F", "C8", "F7"] {
            for count in 1..=5 {
                let code = hx(op).repeat(count);
                assert!(
                    length(&code, 0).is_none_or(|got| got <= code.len()),
                    "{op} x {count}"
                );
            }
        }
    }

    #[test]
    fn run_stops_where_it_cannot_go_on_or_crosses_its_requested_end() {
        let code = hx("90 90 FF FF 90");
        let (found, gave_up) = run(&code, 0, code.len());
        assert_eq!(found.iter().map(|insn| insn.at).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(gave_up, Some(2));

        let code = hx("90 B8 34 12");
        let (found, gave_up) = run(&code, 0, 3);
        assert_eq!(found.iter().map(|insn| insn.at).collect::<Vec<_>>(), [0]);
        assert_eq!(gave_up, Some(1));
    }

    #[test]
    fn code_flow_and_target_are_iced_facts_from_only_op0_near_branches() {
        let conditional = decode(&hx("0F 85 1A 16"), 0).unwrap();
        assert_eq!(conditional.code(), Code::Jne_rel16);
        assert_eq!(conditional.flow(), FlowControl::ConditionalBranch);
        assert_eq!(conditional.target(), Some(0x161E));
        assert_eq!(decode(&hx("EA 0F 00 15 00"), 0).unwrap().target(), None);
        assert_eq!(decode(&hx("FF E0"), 0).unwrap().target(), None);
    }

    #[test]
    fn emulator_interrupts_are_x87_instructions() {
        for (enc, want) in [
            ("CD 35 46 C8", 4),
            ("CD 34 4E C8", 4),
            ("CD 3A C1", 3),
            ("CD 36 06 5E 00", 5),
            ("CD 3C D9 07", 4),
            ("CD 3C D8 27", 4),
            ("CD 3C DF 06 12 00", 6),
            ("CD 3D", 2),
        ] {
            let insn = decode(&hx(enc), 0).unwrap();
            assert_eq!(insn.length, want, "{enc}");
            assert!(insn.disp_at.is_none_or(|at| at >= 2), "{enc}");
            assert_eq!(insn.writes() & 0x3F, 0, "{enc}");
        }
    }

    #[test]
    fn emulator_operand_offsets_rebase_to_the_interrupt_source_bytes() {
        let escaped = decode(&hx("CD 35 46 C8"), 0).unwrap();
        assert_eq!(
            (
                escaped.disp_at,
                escaped.disp_len,
                escaped.imm_at,
                escaped.imm_len
            ),
            (Some(3), 1, None, 0)
        );
        let segmented = decode(&hx("CD 3C DF 06 12 00"), 0).unwrap();
        assert_eq!(
            (
                segmented.disp_at,
                segmented.disp_len,
                segmented.imm_at,
                segmented.imm_len
            ),
            (Some(4), 2, None, 0)
        );
    }

    #[test]
    fn ordinary_and_truncated_int_3c_behavior_and_incomplete_escapes() {
        assert_eq!(decode(&hx("CD 3C 90"), 0).unwrap().length, 2);
        assert_eq!(decode(&hx("CD 3C D9"), 0).unwrap().length, 2);
        for enc in ["CD 34", "CD 35", "CD 3B"] {
            assert_eq!(decode(&hx(enc), 0), None, "{enc}");
        }
    }

    #[test]
    fn random_bytes_agree_with_ndisasm() {
        let mut child = match Command::new("ndisasm")
            .args(["-b16", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("could not run ndisasm: {error}"),
        };
        let blob = python_random_bytes();
        assert_eq!(
            &blob[..32],
            &hx(
                "BB A8 6A 8C EA 71 0E FC 8B C5 BE 3C 2C 6F 37 DC BF 7D 80 96 54 32 0A F6 A0 9F 0F 2D 59 13 0B 18"
            )
        );
        assert_eq!(
            &blob[3968..],
            &hx(
                "BE F2 10 EB 4C F4 6C B3 71 BA 8E BA 60 81 8C 48 E7 DE BD 73 DB E6 E6 68 6F 9E E1 BD 32 B2 62 0A"
            )
        );
        assert_eq!(
            blob.iter().map(|byte| usize::from(*byte)).sum::<usize>(),
            510526
        );
        child.stdin.as_mut().unwrap().write_all(&blob).unwrap();
        let output = child.wait_with_output().unwrap();
        let marks: Vec<(usize, String)> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let words: Vec<_> = line.split_whitespace().collect();
                let offset = usize::from_str_radix(words.first()?, 16).ok()?;
                (words.len() >= 3).then(|| (offset, words[2..].join(" ")))
            })
            .collect();
        let joined = [
            "wait", "lock", "rep", "repe", "repne", "repz", "repnz", "cs", "ds", "es", "ss", "fs",
            "gs", "a16", "a32", "o16", "o32",
        ];
        let (mut agree, mut wrong) = (0, Vec::new());
        for pair in marks.windows(2) {
            let (at, text) = (&pair[0].0, &pair[0].1);
            if text.starts_with("db 0x")
                || joined.contains(&text.split_whitespace().next().unwrap_or(""))
            {
                continue;
            }
            if blob[*at] == INTERRUPT
                && blob
                    .get(*at + 1)
                    .is_some_and(|byte| EMULATED.contains(byte))
            {
                continue;
            }
            let want = pair[1].0 - *at;
            let Some(got) = length(&blob, *at) else {
                continue;
            };
            if got == want {
                agree += 1;
            } else {
                wrong.push(format!("{at:04X} {text:?}: got {got}, ndisasm says {want}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        assert!(agree > 1000, "only {agree} instructions agreed");
    }
}
