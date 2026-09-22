//! Source occurrences recognised as halves of a BC long operation.
//!
//! Direct port of `qbopt.legacy.lift.py:{Kind,Decoded,FIXUP}`.  These are
//! deliberately decoded OMF-source facts, not generic machine instructions.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use iced_x86::{Code, Register};

use crate::objectfile::module::{Addr, Space, far_pointer, frame_relative, literal_only};


use crate::frontend::declen::Insn;

/// The seven single-half forms `lift.classify()` recognises.
///
/// Direct port of `qbopt.legacy.lift.py:Kind`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Load,
    Store,
    Alu,
    Move,
    RegAlu,
    Not,
    AluImm,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load => "ld",
            Self::Store => "st",
            Self::Alu => "op",
            Self::Move => "mv",
            Self::RegAlu => "rr",
            Self::Not => "not",
            Self::AluImm => "opi",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Ord for Kind {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for Kind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// One classified half of a BC long operation.
///
/// Direct port of `qbopt.legacy.lift.py:Decoded`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Decoded {
    pub kind: Kind,
    pub pair: usize,
    pub half: usize,
    pub length: usize,
    pub src_pair: usize,
    pub alu: Option<Code>,
    pub mem: Option<Addr>,
    pub dlen: usize,
    pub disp_at: Option<usize>,
    pub imm: Option<i64>,
}

impl Decoded {
    #[must_use]
    pub const fn new(kind: Kind, pair: usize, half: usize, length: usize) -> Self {
        Self {
            kind,
            pair,
            half,
            length,
            src_pair: 0,
            alu: None,
            mem: None,
            dlen: 0,
            disp_at: None,
            imm: None,
        }
    }
}

/// The byte-identical restore sequences emitted after a widened operation.
///
/// Direct port of `qbopt.legacy.lift.py:FIXUP`.
pub static FIXUP: LazyLock<BTreeMap<usize, [u8; 4]>> = LazyLock::new(|| {
    BTreeMap::from([(0, [0x66, 0x50, 0x58, 0x5A]), (1, [0x66, 0x51, 0x59, 0x5B])])
});

/// The five operations, low half -> (high half, widened mnemonic).
///
/// Direct port of `qbopt.legacy.lift.py:PAIRED`.
pub static PAIRED: LazyLock<BTreeMap<Code, (Code, &'static str)>> = LazyLock::new(|| {
    BTreeMap::from([
        (Code::And_r16_rm16, (Code::And_r16_rm16, "and")),
        (Code::Or_r16_rm16, (Code::Or_r16_rm16, "or")),
        (Code::Xor_r16_rm16, (Code::Xor_r16_rm16, "xor")),
        (Code::Add_r16_rm16, (Code::Adc_r16_rm16, "add")),
        (Code::Sub_r16_rm16, (Code::Sbb_r16_rm16, "sub")),
    ])
});

/// Direct port of `qbopt.legacy.lift.py:HIGH_HALVES`.
pub static HIGH_HALVES: LazyLock<BTreeSet<Code>> =
    LazyLock::new(|| PAIRED.values().map(|(high, _)| *high).collect());

/// The immediate encodings of the low-half ALU operations.
///
/// Direct port of `qbopt.legacy.lift.py:IMM_FAMILY`.
pub static IMM_FAMILY: LazyLock<BTreeMap<Code, &'static str>> = LazyLock::new(|| {
    BTreeMap::from([
        (Code::Add_AX_imm16, "add"),
        (Code::Add_rm16_imm16, "add"),
        (Code::Add_rm16_imm8, "add"),
        (Code::Sub_AX_imm16, "sub"),
        (Code::Sub_rm16_imm16, "sub"),
        (Code::Sub_rm16_imm8, "sub"),
        (Code::And_AX_imm16, "and"),
        (Code::And_rm16_imm16, "and"),
        (Code::And_rm16_imm8, "and"),
        (Code::Or_AX_imm16, "or"),
        (Code::Or_rm16_imm16, "or"),
        (Code::Or_rm16_imm8, "or"),
        (Code::Xor_AX_imm16, "xor"),
        (Code::Xor_rm16_imm16, "xor"),
        (Code::Xor_rm16_imm8, "xor"),
    ])
});

/// The immediate encodings that can supply an ALU high half.
///
/// Direct port of `qbopt.legacy.lift.py:IMM_HIGH_FAMILY`.
pub static IMM_HIGH_FAMILY: LazyLock<BTreeMap<&'static str, BTreeSet<Code>>> =
    LazyLock::new(|| {
        BTreeMap::from([
            (
                "add",
                BTreeSet::from([
                    Code::Adc_AX_imm16,
                    Code::Adc_rm16_imm16,
                    Code::Adc_rm16_imm8,
                ]),
            ),
            (
                "sub",
                BTreeSet::from([
                    Code::Sbb_AX_imm16,
                    Code::Sbb_rm16_imm16,
                    Code::Sbb_rm16_imm8,
                ]),
            ),
            (
                "and",
                BTreeSet::from([
                    Code::And_AX_imm16,
                    Code::And_rm16_imm16,
                    Code::And_rm16_imm8,
                ]),
            ),
            (
                "or",
                BTreeSet::from([Code::Or_AX_imm16, Code::Or_rm16_imm16, Code::Or_rm16_imm8]),
            ),
            (
                "xor",
                BTreeSet::from([
                    Code::Xor_AX_imm16,
                    Code::Xor_rm16_imm16,
                    Code::Xor_rm16_imm8,
                ]),
            ),
        ])
    });

/// Direct port of `qbopt.legacy.lift.py:IMM_HIGH_HALVES`.
pub static IMM_HIGH_HALVES: LazyLock<BTreeSet<Code>> =
    LazyLock::new(|| IMM_HIGH_FAMILY.values().flatten().copied().collect());

/// Direct port of `qbopt.legacy.lift.py:LOADS`.
pub static LOADS: LazyLock<BTreeSet<Code>> =
    LazyLock::new(|| BTreeSet::from([Code::Mov_r16_rm16, Code::Mov_AX_moffs16]));

/// Direct port of `qbopt.legacy.lift.py:STORES`.
pub static STORES: LazyLock<BTreeSet<Code>> =
    LazyLock::new(|| BTreeSet::from([Code::Mov_rm16_r16, Code::Mov_moffs16_AX]));

/// Register -> (pair, half), direct port of `qbopt.legacy.lift.py:HALF_OF`.
pub static HALF_OF: LazyLock<BTreeMap<Register, (usize, usize)>> = LazyLock::new(|| {
    BTreeMap::from([
        (Register::AX, (0, 0)),
        (Register::DX, (0, 1)),
        (Register::CX, (1, 0)),
        (Register::BX, (1, 1)),
    ])
});

/// Direct port of `qbopt.legacy.lift.py:REDUNDANT_DS`.
pub static REDUNDANT_DS: LazyLock<BTreeSet<Register>> =
    LazyLock::new(|| BTreeSet::from([Register::None, Register::SI, Register::DI]));

/// The OMF field resolver passed by the production module decoder.
///
/// Python's resolver receives the raw iced displacement, not the signed
/// addressing displacement.
pub type Resolver<'a> = dyn Fn(i64, i64) -> Addr + 'a;

/// Where this instruction's memory operand points, or `None` if it has none.
///
/// Direct port of `qbopt.legacy.lift.py:operand`.
#[must_use]
pub fn operand(insn: &Insn, resolve: &Resolver) -> Option<Addr> {
    let base = insn.memory_base();
    if insn.memory_index() != Register::None {
        return None;
    }
    let override_ = insn.segment_override();
    if override_ != Register::None && override_ != Register::DS {
        if !matches!(base, Register::BX | Register::SI | Register::DI) {
            return None;
        }
        if let Some(disp_at) = insn.disp_at {
            let mut resolved = resolve(disp_at as i64, insn.insn.memory_displacement64() as i64);
            if resolved.space == Space::Group {
                return None;
            }
            if matches!(resolved.space, Space::Segment | Space::External) {
                resolved.base = base;
                resolved.segment = override_;
                return Some(resolved);
            }
        }
        return Some(far_pointer(
            insn.displacement(),
            base,
            override_,
        ));
    }
    if override_ == Register::DS && !REDUNDANT_DS.contains(&base) {
        return None;
    }
    let Some(disp_at) = insn.disp_at else {
        return match base {
            Register::SI | Register::DI => {
                let mut resolved = literal_only(0, 0);
                resolved.base = base;
                Some(resolved)
            }
            _ => None,
        };
    };
    match base {
        Register::None => resolved(resolve(disp_at as i64, insn.insn.memory_displacement64() as i64)),
        Register::BP => Some(frame_relative(insn.displacement())),
        Register::BX | Register::SI | Register::DI => {
            let mut resolved = resolved(resolve(disp_at as i64, insn.insn.memory_displacement64() as i64))?;
            resolved.base = base;
            Some(resolved)
        }
        _ => None,
    }
}

fn resolved(address: Addr) -> Option<Addr> {
    (address.space != Space::Group).then_some(address)
}

/// What one instruction is in long terms, or `None`.
///
/// Direct port of Python's `classify(insn, resolve=literal_only)` default.
#[must_use]
pub fn classify(insn: &Insn) -> Option<Decoded> {
    classify_with(insn, &literal_only)
}

/// What one instruction is in long terms under an OMF field resolver, or `None`.
///
/// Production module decoding must call this explicit-resolver form rather
/// than treating a fixup-backed field as a literal.
#[must_use]
pub fn classify_with(insn: &Insn, resolve: &Resolver) -> Option<Decoded> {
    let code = insn.code();
    if LOADS.contains(&code) || STORES.contains(&code) {
        let loading = LOADS.contains(&code);
        let register = insn.register(if loading { 0 } else { 1 });
        let &(pair, half) = HALF_OF.get(&register)?;
        let kind = if loading { Kind::Load } else { Kind::Store };
        if !insn.reads_memory(if loading { 1 } else { 0 }) {
            let source = insn.register(if loading { 1 } else { 0 });
            let &(src_pair, source_half) = HALF_OF.get(&source)?;
            if !loading || source_half != half {
                return None;
            }
            let mut decoded = Decoded::new(Kind::Move, pair, half, insn.length);
            decoded.src_pair = src_pair;
            return Some(decoded);
        }
        let where_ = operand(insn, resolve)?;
        let mut decoded = Decoded::new(kind, pair, half, insn.length);
        decoded.mem = Some(where_);
        decoded.dlen = insn.disp_len;
        decoded.disp_at = insn.disp_at;
        return Some(decoded);
    }

    if code == Code::Not_rm16 && !insn.reads_memory(0) {
        let &(pair, half) = HALF_OF.get(&insn.register(0))?;
        return Some(Decoded::new(Kind::Not, pair, half, insn.length));
    }

    if IMM_FAMILY.contains_key(&code) || IMM_HIGH_HALVES.contains(&code) {
        let &(pair, half) = HALF_OF.get(&insn.register(0))?;
        let mut decoded = Decoded::new(Kind::AluImm, pair, half, insn.length);
        decoded.alu = Some(code);
        decoded.imm = Some((insn.insn.immediate(1) & 0xFFFF) as i64);
        return Some(decoded);
    }

    if !PAIRED.contains_key(&code) && !HIGH_HALVES.contains(&code) {
        return None;
    }
    let &(pair, half) = HALF_OF.get(&insn.register(0))?;
    if !insn.reads_memory(1) {
        let &(src_pair, source_half) = HALF_OF.get(&insn.register(1))?;
        if source_half != half {
            return None;
        }
        let mut decoded = Decoded::new(Kind::RegAlu, pair, half, insn.length);
        decoded.src_pair = src_pair;
        decoded.alu = Some(code);
        return Some(decoded);
    }
    let where_ = operand(insn, resolve)?;
    let mut decoded = Decoded::new(Kind::Alu, pair, half, insn.length);
    decoded.alu = Some(code);
    decoded.mem = Some(where_);
    decoded.dlen = insn.disp_len;
    decoded.disp_at = insn.disp_at;
    Some(decoded)
}

#[cfg(test)]
mod tests {
    use super::{Decoded, FIXUP, Kind, classify, operand};
    use crate::frontend::declen::{Insn, decode};
    use crate::objectfile::module::{Addr, Space, far_pointer, literal_only};
    

    fn insn(bytes: &[u8]) -> Insn {
        decode(bytes, 0).unwrap()
    }

    fn classified(bytes: &[u8]) -> Option<Decoded> {
        classify(&insn(bytes))
    }

    #[test]
    fn omf_source_nodes_kind_spelling_and_order_are_python_strenum_order() {
        let mut kinds = vec![
            Kind::Load,
            Kind::Store,
            Kind::Alu,
            Kind::Move,
            Kind::RegAlu,
            Kind::Not,
            Kind::AluImm,
        ];
        kinds.sort();
        assert_eq!(
            kinds.into_iter().map(Kind::as_str).collect::<Vec<_>>(),
            ["ld", "mv", "not", "op", "opi", "rr", "st"]
        );
        assert_eq!(Kind::AluImm.to_string(), "opi");
    }

    #[test]
    fn omf_source_nodes_decoded_defaults_and_fixups_match_lift() {
        let decoded = Decoded::new(Kind::Load, 1, 0, 3);
        assert_eq!(decoded.kind, Kind::Load);
        assert_eq!(decoded.pair, 1);
        assert_eq!(decoded.half, 0);
        assert_eq!(decoded.length, 3);
        assert_eq!(decoded.src_pair, 0);
        assert_eq!(decoded.alu, None);
        assert_eq!(decoded.mem, None);
        assert_eq!(decoded.dlen, 0);
        assert_eq!(decoded.disp_at, None);
        assert_eq!(decoded.imm, None);
        assert_eq!(FIXUP.get(&0).unwrap(), &[0x66, 0x50, 0x58, 0x5A]);
        assert_eq!(FIXUP.get(&1).unwrap(), &[0x66, 0x51, 0x59, 0x5B]);
    }

    #[test]
    fn omf_lift_classify_operand_refuses_a_segment_override() {
        // `es: mov ax,[0x1234]` must not conflate es:[x] and ds:[x].
        let decoded = insn(&[0x26, 0x8B, 0x06, 0x34, 0x12]);
        assert_eq!(operand(&decoded, &literal_only), None);
    }

    #[test]
    fn omf_lift_classify_operand_resolves_the_same_field_without_the_override() {
        let decoded = insn(&[0x8B, 0x06, 0x34, 0x12]);
        assert_eq!(
            operand(&decoded, &literal_only),
            Some(Addr::new(Space::Literal, 0x1234))
        );
    }

    #[test]
    fn omf_lift_classify_resolver_sees_raw_displacement_while_bp_is_signed() {
        let based = insn(&[0x8B, 0x87, 0xE8, 0xFF]);
        let resolver = |field_offset: i64, literal: i64| {
            assert_eq!((field_offset, literal), (2, 0xFFE8));
            Addr::new(Space::Literal, literal as i64)
        };
        let mut based_wanted = Addr::new(Space::Literal, 0xFFE8);
        based_wanted.base = iced_x86::Register::BX;
        assert_eq!(operand(&based, &resolver), Some(based_wanted));

        let frame = insn(&[0x8B, 0x46, 0xE8]);
        let no_resolver = |_field_offset: i64, _literal: i64| -> Addr {
            panic!("bp-relative displacement is not a relocation")
        };
        assert_eq!(
            operand(&frame, &no_resolver),
            Some(Addr::new(Space::Frame, -24))
        );
    }

    #[test]
    fn omf_lift_classify_redundant_ds_forms_are_ordinary_and_bp_is_refused() {
        let direct = insn(&[0x3E, 0x8B, 0x06, 0x34, 0x12]);
        let direct_resolver = |field_offset: i64, literal: i64| {
            assert_eq!((field_offset, literal), (3, 0x1234));
            Addr::new(Space::Literal, literal as i64)
        };
        assert_eq!(
            operand(&direct, &direct_resolver),
            Some(Addr::new(Space::Literal, 0x1234))
        );

        for (bytes, base) in [
            (&[0x3E, 0x8B, 0x04][..], iced_x86::Register::SI),
            (&[0x3E, 0x8B, 0x05][..], iced_x86::Register::DI),
        ] {
            let decoded = insn(bytes);
            let no_resolver = |_field_offset: i64, _literal: i64| -> Addr {
                panic!("an absent displacement cannot have a relocation")
            };
            let mut wanted = Addr::new(Space::Literal, 0);
            wanted.base = base;
            assert_eq!(operand(&decoded, &no_resolver), Some(wanted));
        }

        let bp = insn(&[0x3E, 0x8B, 0x46, 0xE8]);
        let no_resolver = |_field_offset: i64, _literal: i64| -> Addr {
            panic!("ds:[bp+disp] is refused before resolution")
        };
        assert_eq!(operand(&bp, &no_resolver), None);
    }

    #[test]
    fn omf_lift_classify_operand_refuses_a_group_relative_address() {
        let decoded = insn(&[0x8B, 0x06, 0x34, 0x12]);
        let group_only = |_field_offset: i64, literal: i64| Addr {
            space: Space::Group,
            disp: literal as i64,
            index: 1,
            base: iced_x86::Register::None,
            segment: iced_x86::Register::None,
        };
        assert_eq!(operand(&decoded, &group_only), None);
    }

    #[test]
    fn omf_lift_classify_operand_keeps_an_es_bx_pointer_far() {
        let decoded = insn(&[0x26, 0x8B, 0x07]);
        assert_eq!(
            operand(&decoded, &literal_only),
            Some(far_pointer(
                0,
                iced_x86::Register::BX,
                iced_x86::Register::ES,
            ))
        );
    }

    #[test]
    fn omf_lift_classify_operand_resolver_segment_and_external_keep_override() {
        for (space, index) in [(Space::Segment, 5), (Space::External, 8)] {
            let decoded = insn(&[0x26, 0x8B, 0x87, 0x34, 0x12]);
            let resolver = move |field_offset: i64, literal: i64| {
                assert_eq!((field_offset, literal), (3, 0x1234));
                Addr {
                    space,
                    disp: 0x40,
                    index,
                    base: iced_x86::Register::None,
                    segment: iced_x86::Register::None,
                }
            };
            assert_eq!(
                operand(&decoded, &resolver),
                Some(Addr {
                    space,
                    disp: 0x40,
                    index,
                    base: iced_x86::Register::BX,
                    segment: iced_x86::Register::ES,
                })
            );
        }
    }

    #[test]
    fn omf_lift_classify_every_register_half_and_addressing_form() {
        let registers = [
            ("ax", 0_u8, 0_usize, 0_usize),
            ("cx", 1_u8, 1_usize, 0_usize),
            ("dx", 2_u8, 0_usize, 1_usize),
            ("bx", 3_u8, 1_usize, 1_usize),
        ];
        let bases = [(0x06_u8, 2_usize), (0x46_u8, 1_usize), (0x86_u8, 2_usize)];
        for (opcode, kind) in [(0x8B, Kind::Load), (0x89, Kind::Store)] {
            for (name, register, pair, half) in registers {
                for (base, dlen) in bases {
                    let mut bytes = vec![opcode, base | (register << 3)];
                    if dlen == 1 {
                        bytes.push(0xE8);
                    } else {
                        bytes.extend([0x5E, 0x00]);
                    }
                    let decoded =
                        classified(&bytes).unwrap_or_else(|| panic!("{name} base {base:02x}"));
                    assert_eq!(decoded.kind, kind);
                    assert_eq!((decoded.pair, decoded.half), (pair, half));
                    assert_eq!(decoded.dlen, dlen);
                }
            }
        }
    }

    #[test]
    fn omf_lift_classify_every_alu_operation() {
        let registers = [(0_u8, 0, 0), (1_u8, 1, 0), (2_u8, 0, 1), (3_u8, 1, 1)];
        for (low, high) in [(0x23, 0x23), (0x03, 0x13), (0x2B, 0x1B)] {
            for half in [0, 1] {
                for (register, _, _) in registers {
                    let opcode = if half == 0 { low } else { high };
                    let decoded =
                        classified(&[opcode, 0x06 | (register << 3), 0x5E, 0x00]).unwrap();
                    assert_eq!(decoded.kind, Kind::Alu);
                    assert!(decoded.alu.is_some());
                }
            }
        }
    }

    #[test]
    fn omf_lift_classify_register_to_register_requires_matching_halves() {
        let registers = [
            (0_u8, 0_usize, 0_usize),
            (1_u8, 1_usize, 0_usize),
            (2_u8, 0_usize, 1_usize),
            (3_u8, 1_usize, 1_usize),
        ];
        for (destination, destination_pair, destination_half) in registers {
            for (source, source_pair, source_half) in registers {
                let decoded = classified(&[0x8B, 0xC0 | (destination << 3) | source]);
                if destination_half == source_half {
                    let decoded = decoded.unwrap();
                    assert_eq!(decoded.kind, Kind::Move);
                    assert_eq!(
                        (decoded.pair, decoded.src_pair),
                        (destination_pair, source_pair)
                    );
                } else {
                    assert_eq!(decoded, None);
                }
            }
        }
    }

    #[test]
    fn omf_lift_classify_recognises_every_immediate_alu_encoding() {
        for (bytes, wanted) in [
            (&[0x05, 0x34, 0x12][..], 0x1234_i64),
            (&[0x81, 0xC0, 0x34, 0x12][..], 0x1234_i64),
            (&[0x83, 0xC0, 0x04][..], 4_i64),
            (&[0x83, 0xC0, 0xFF][..], 0xFFFF_i64),
        ] {
            let decoded = classified(bytes).unwrap();
            assert_eq!(decoded.kind, Kind::AluImm);
            assert_eq!(decoded.imm, Some(wanted));
        }
    }

    #[test]
    fn omf_lift_classify_refuses_the_python_table() {
        for bytes in [
            &[0x8B, 0x07][..],
            &[0x8B, 0x20][..],
            &[0x8B, 0x36, 0x5E, 0x00][..],
            &[0x87, 0x06, 0x5E, 0x00][..],
            &[0x8B, 0xC4][..],
        ] {
            assert_eq!(classified(bytes), None);
        }
    }

    #[test]
    fn omf_lift_classify_refuses_an_operand_size_prefixed_half() {
        assert!(classified(&[0x8B, 0x06, 0x5E, 0x00]).is_some());
        assert_eq!(classified(&[0x66, 0x8B, 0x06, 0x5E, 0x00]), None);
    }

    /// The resolver was `dyn Fn + 'static`, so a module's own `resolve` --
    /// a closure borrowing the module -- could not be passed, and production
    /// decoding could only ever classify against `literal_only`.
    #[test]
    fn omf_lift_classify_takes_a_resolver_borrowing_its_module() {
        let operands = std::collections::BTreeMap::from([(2_i64, Addr::new(Space::Segment, 0x40))]);
        let resolve = |field_offset: i64, literal: i64| {
            operands.get(&field_offset).copied().unwrap_or(Addr::new(Space::Literal, literal))
        };
        let decoded = super::classify_with(&insn(&[0x8B, 0x06, 0x00, 0x00]), &resolve).unwrap();
        assert_eq!(decoded.mem, Some(Addr::new(Space::Segment, 0x40)));
    }

    #[test]
    fn omf_lift_classify_refuses_a_bp_si_address() {
        // [bp+si+8] cannot be silently reclassified as the plain [bp+8] local.
        assert_eq!(classified(&[0x8B, 0x42, 0x08]), None);
    }

    #[test]
    fn omf_lift_classify_implicit_zero_displacement_keeps_si_or_di() {
        for (bytes, base) in [
            (&[0x8B, 0x04][..], iced_x86::Register::SI),
            (&[0x8B, 0x05][..], iced_x86::Register::DI),
            (&[0x89, 0x04][..], iced_x86::Register::SI),
            (&[0x89, 0x05][..], iced_x86::Register::DI),
        ] {
            let decoded = insn(bytes);
            let no_field = |_field_offset: i64, _literal: i64| -> Addr {
                panic!("an absent displacement cannot have a relocation")
            };
            let mut wanted = Addr::new(Space::Literal, 0);
            wanted.base = base;
            assert_eq!(operand(&decoded, &no_field), Some(wanted));
        }
    }
}

/// Port of `qbopt/legacy/lift.py:Emitted`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Emitted {
    pub code: Vec<u8>,
    /// (offset within `code`, where the operand's field was) for each
    /// displacement that has to be relocated.
    pub relocations: Vec<(usize, usize)>,
}

/// Port of `qbopt/legacy/lift.py:relocated_memory`: a relocated address,
/// always emitted as zero.
pub fn relocated_memory(base: Register, segment: Register) -> iced_x86::MemoryOperand {
    iced_x86::MemoryOperand::new(base, Register::None, 1, 0, 2, false, segment)
}
