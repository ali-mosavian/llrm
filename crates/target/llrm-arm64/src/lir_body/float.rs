//! Scalar floating-point and vector-register forms. A name is the mnemonic,
//! the size letters (`S`, `D`, `H`; `W`, `X` for the general register of a
//! conversion) and the form (`r` registers, `i` an immediate).

use super::*;
use crate::mc::inst::Cond;
use crate::mc::inst::{
    Arrangement, Fill, FpBinary, FpConvert, FpFused, FpSize, FpUnary, ImmShape, NarrowLane, Rounding, VectorFma,
    VectorFpOp, VectorFpUnary, VectorLongOp, VectorOp, VectorReduce, VectorShift,
};

const UNARY: [(FpUnary, &str); 11] = [
    (FpUnary::Mov, "FMOV"),
    (FpUnary::Abs, "FABS"),
    (FpUnary::Neg, "FNEG"),
    (FpUnary::Sqrt, "FSQRT"),
    (FpUnary::Rint(Rounding::Nearest), "FRINTN"),
    (FpUnary::Rint(Rounding::Up), "FRINTP"),
    (FpUnary::Rint(Rounding::Down), "FRINTM"),
    (FpUnary::Rint(Rounding::Zero), "FRINTZ"),
    (FpUnary::Rint(Rounding::Away), "FRINTA"),
    (FpUnary::Rint(Rounding::Exact), "FRINTX"),
    (FpUnary::Rint(Rounding::Current), "FRINTI"),
];
const BINARY: [(FpBinary, &str); 9] = [
    (FpBinary::Mul, "FMUL"),
    (FpBinary::Div, "FDIV"),
    (FpBinary::Add, "FADD"),
    (FpBinary::Sub, "FSUB"),
    (FpBinary::Max, "FMAX"),
    (FpBinary::Min, "FMIN"),
    (FpBinary::MaxNm, "FMAXNM"),
    (FpBinary::MinNm, "FMINNM"),
    (FpBinary::Nmul, "FNMUL"),
];
const FUSED: [(FpFused, &str); 4] =
    [(FpFused::Madd, "FMADD"), (FpFused::Msub, "FMSUB"), (FpFused::Nmadd, "FNMADD"), (FpFused::Nmsub, "FNMSUB")];
/// The conversions between a general and a float register (`FMOV` is by
/// direction, the letter order tells it).
const CONVERT: [(FpConvert, &str); 4] = [
    (FpConvert::ToFp { signed: true }, "SCVTF"),
    (FpConvert::ToFp { signed: false }, "UCVTF"),
    (FpConvert::ToIntZero { signed: true }, "FCVTZS"),
    (FpConvert::ToIntZero { signed: false }, "FCVTZU"),
];

fn bits(size: FpSize) -> u32 {
    match size {
        FpSize::S => 32,
        FpSize::D => 64,
        FpSize::H => 16,
    }
}

fn size_letter(size: FpSize) -> char {
    match size {
        FpSize::S => 'S',
        FpSize::D => 'D',
        FpSize::H => 'H',
    }
}

fn size_named(letter: char) -> FpSize {
    match letter {
        'S' => FpSize::S,
        'D' => FpSize::D,
        _ => FpSize::H,
    }
}

fn f(
    register: Vreg,
    size: FpSize,
) -> Loc {
    vreg_loc(register, bits(size))
}

fn named<T: Copy + PartialEq>(
    table: &[(T, &'static str)],
    one: T,
) -> &'static str {
    table.iter().find(|(each, _)| *each == one).expect("a listed mnemonic").1
}

/// The entry of `table` whose mnemonic is the longest prefix of `name`
/// (`FMAXNM` over `FMAX`), and what follows it.
fn prefix<'a, T: Copy>(
    table: &[(T, &'static str)],
    name: &'a str,
) -> Option<(T, &'a str)> {
    table
        .iter()
        .filter(|(_, mnemonic)| name.starts_with(mnemonic))
        .max_by_key(|(_, mnemonic)| mnemonic.len())
        .map(|(one, mnemonic)| (*one, &name[mnemonic.len()..]))
}

/// `rest` as its size letters and form: `Dr` is ("D", "r").
fn letters(rest: &str) -> (&str, &str) {
    rest.split_at(rest.find(|c: char| c.is_ascii_lowercase()).unwrap_or(rest.len()))
}

fn to_fp(op: FpConvert) -> bool {
    matches!(op, FpConvert::ToFp { .. } | FpConvert::Bits { to_fp: true })
}

pub(super) fn to(inst: &Inst) -> Option<Semantics> {
    Some(match inst {
        Inst::FpUnary { op, size, rd, rn } => semantics(
            if *op == FpUnary::Mov { Operation::Move } else { Operation::Unary },
            format!("{}{}r", named(&UNARY, *op), size_letter(*size)),
            vec![f(*rd, *size)],
            vec![f(*rn, *size)],
            None,
        ),
        Inst::VectorMov { rd, rn } => {
            semantics(Operation::Move, "ORRv16b", vec![vreg_loc(*rd, 128)], vec![vreg_loc(*rn, 128)], None)
        }
        Inst::VectorBinary { op, lanes, rd, rn, rm } => semantics(
            Operation::Binary,
            vector_op_name(*op, *lanes),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), vreg_loc(*rm, 128)],
            None,
        ),
        Inst::VectorShift { op, lanes, rd, rn, amount } => semantics(
            Operation::Binary,
            format!(
                "{}v{}_shift",
                match op {
                    VectorShift::Shl => "SHL",
                    VectorShift::Ushr => "USHR",
                    VectorShift::Sshr => "SSHR",
                },
                lane_name(*lanes)
            ),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), imm(i64::from(*amount))],
            None,
        ),
        Inst::VectorLongShift { op, lane, high, rd, rn, amount } => semantics(
            Operation::Binary,
            long_name(*op, *lane, *high),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, if *high { 128 } else { 64 }), imm(i64::from(*amount))],
            None,
        ),
        Inst::VectorLong { op, lane, high, rd, rn, rm } => {
            let narrow = if *high { 128 } else { 64 };
            let mut sources = vec![vreg_loc(*rn, if op.adds_wide() { 128 } else { narrow }), vreg_loc(*rm, narrow)];
            if op.accumulates() {
                sources.insert(0, vreg_loc(*rd, 128));
            }
            semantics(
                if op.accumulates() { Operation::FloatMultiplyAdd } else { Operation::Binary },
                long_name(*op, *lane, *high),
                vec![vreg_loc(*rd, 128)],
                sources,
                None,
            )
        }
        Inst::VectorNot { rd, rn } => {
            semantics(Operation::Unary, "NOTv16i8", vec![vreg_loc(*rd, 128)], vec![vreg_loc(*rn, 128)], None)
        }
        Inst::VectorFma { op, lanes, rd, rn, rm } => semantics(
            Operation::FloatMultiplyAdd,
            vector_fp_name(op.mnemonic(), *lanes),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rd, 128), vreg_loc(*rn, 128), vreg_loc(*rm, 128)],
            None,
        ),
        Inst::VectorInsGpr { lanes, rd, rn, index } => {
            let width = if *lanes == Arrangement::D2 { Width::X } else { Width::W };
            semantics(
                Operation::Move,
                format!("INSv{}gpr", ins_lane(*lanes)),
                vec![vreg_loc(*rd, 128)],
                vec![vreg_loc(*rd, 128), imm(i64::from(*index)), gpr_loc(*rn, width)],
                None,
            )
        }
        Inst::VectorFmaLane { op, lanes, rd, rn, rm, index } => semantics(
            Operation::FloatMultiplyAdd,
            format!(
                "{}v{}_indexed",
                op.mnemonic().to_uppercase(),
                if *lanes == Arrangement::D2 { "2i64" } else { "4i32" }
            ),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rd, 128), vreg_loc(*rn, 128), vreg_loc(*rm, 128), imm(i64::from(*index))],
            None,
        ),
        Inst::VectorUnzip { second, lanes, rd, rn, rm } => semantics(
            Operation::Binary,
            unzip_name(*second, *lanes),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), vreg_loc(*rm, 128)],
            None,
        ),
        Inst::VectorNarrow { high, wide, rd, rn } => semantics(
            if *high { Operation::Binary } else { Operation::Unary },
            format!("XTN{}", narrow_name(*wide, *high)),
            vec![vreg_loc(*rd, if *high { 128 } else { 64 })],
            if *high { vec![vreg_loc(*rd, 128), vreg_loc(*rn, 128)] } else { vec![vreg_loc(*rn, 128)] },
            None,
        ),
        Inst::VectorFmulLane { lanes, rd, rn, rm, index } => semantics(
            Operation::Binary,
            format!("FMULv{}_indexed", if *lanes == Arrangement::D2 { "2i64" } else { "4i32" }),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), vreg_loc(*rm, 128), imm(i64::from(*index))],
            None,
        ),
        Inst::VectorMulLane { rd, rn, rm, index } => semantics(
            Operation::Binary,
            "MULv4i32_indexed".to_string(),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), vreg_loc(*rm, 128), imm(i64::from(*index))],
            None,
        ),
        Inst::VectorUmov { lanes, rd, rn, index } => {
            let width = if *lanes == Arrangement::D2 { Width::X } else { Width::W };
            semantics(
                Operation::Move,
                format!("UMOVv{}", ins_lane(*lanes)),
                vec![gpr_loc(*rd, width)],
                vec![vreg_loc(*rn, 128), imm(i64::from(*index))],
                None,
            )
        }
        Inst::VectorInsLane { lanes, rd, rn, index, src } => semantics(
            Operation::Move,
            format!("INSv{}lane", ins_lane(*lanes)),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rd, 128), imm(i64::from(*index)), vreg_loc(*rn, 128), imm(i64::from(*src))],
            None,
        ),
        Inst::VectorDupScalar { lanes, rd, rn, index } => semantics(
            Operation::Move,
            format!("DUP{}", ins_lane(*lanes)),
            vec![vreg_loc(*rd, 64)],
            vec![vreg_loc(*rn, 128), imm(i64::from(*index))],
            None,
        ),
        Inst::VectorReduce { op, lanes, rd, rn } => semantics(
            Operation::Unary,
            reduce_name(*op, *lanes),
            vec![vreg_loc(*rd, lanes.lane_bits())],
            vec![vreg_loc(*rn, 128)],
            None,
        ),
        Inst::VectorDupLane { lanes, rd, rn, index } => semantics(
            Operation::Move,
            format!("DUPv{}lane", lane_name(*lanes)),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), imm(i64::from(*index))],
            None,
        ),
        Inst::VectorFpBinary { op, lanes, rd, rn, rm } => semantics(
            Operation::Binary,
            vector_fp_name(op.mnemonic(), *lanes),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128), vreg_loc(*rm, 128)],
            None,
        ),
        Inst::VectorFpUnary { op, lanes, rd, rn } => semantics(
            Operation::Unary,
            vector_fp_name(op.mnemonic(), *lanes),
            vec![vreg_loc(*rd, 128)],
            vec![vreg_loc(*rn, 128)],
            None,
        ),
        Inst::FpBinary { op, size, rd, rn, rm } => semantics(
            Operation::Binary,
            format!("{}{}rr", named(&BINARY, *op), size_letter(*size)),
            vec![f(*rd, *size)],
            vec![f(*rn, *size), f(*rm, *size)],
            None,
        ),
        Inst::FpFused { op, size, rd, rn, rm, ra } => semantics(
            Operation::FloatMultiplyAdd,
            format!("{}{}rrr", named(&FUSED, *op), size_letter(*size)),
            vec![f(*rd, *size)],
            vec![f(*rn, *size), f(*rm, *size), f(*ra, *size)],
            None,
        ),
        Inst::FpCompare { signalling, size, rn, rm } => {
            let mut sources = vec![f(*rn, *size)];
            sources.extend(rm.map(|rm| f(rm, *size)));
            semantics(
                Operation::Compare,
                format!(
                    "{}{}{}",
                    if *signalling { "FCMPE" } else { "FCMP" },
                    size_letter(*size),
                    if rm.is_some() { "rr" } else { "rz" }
                ),
                vec![],
                sources,
                None,
            )
        }
        Inst::FpSelect { size, rd, rn, rm, cond } => semantics(
            Operation::Select,
            format!("FCSEL{}rrr", size_letter(*size)),
            vec![f(*rd, *size)],
            vec![f(*rn, *size), f(*rm, *size), imm(position(&super::integer::CONDS, *cond))],
            None,
        ),
        Inst::FpMoveImm { size, rd, imm8 } => semantics(
            Operation::Move,
            format!("FMOV{}i", size_letter(*size)),
            vec![f(*rd, *size)],
            vec![imm(i64::from(*imm8))],
            None,
        ),
        Inst::FpLaneToFp { signed, size, rd, rn } => semantics(
            Operation::Unary,
            format!("{}CVTFv1i{}", if *signed { 'S' } else { 'U' }, if *size == FpSize::D { 64 } else { 32 }),
            vec![f(*rd, *size)],
            vec![f(*rn, *size)],
            None,
        ),
        Inst::FpToFp { to, from, rd, rn } => semantics(
            Operation::Unary,
            format!("FCVT{}{}r", size_letter(*to), size_letter(*from)),
            vec![f(*rd, *to)],
            vec![f(*rn, *from)],
            None,
        ),
        Inst::FpInt { op, width, size, gpr, fpr } => {
            let (mnemonic, letters) = convert_name(*op, *width, *size);
            let (g, fp) = (gpr_loc(*gpr, *width), f(*fpr, *size));
            let (dest, source) = if to_fp(*op) { (fp, g) } else { (g, fp) };
            semantics(Operation::Unary, format!("{mnemonic}{letters}r"), vec![dest], vec![source], None)
        }
        Inst::FpFixed { op, width, size, gpr, fpr, fbits } => {
            let (mnemonic, letters) = convert_name(*op, *width, *size);
            let (g, fp) = (gpr_loc(*gpr, *width), f(*fpr, *size));
            let (dest, source) = if to_fp(*op) { (fp, g) } else { (g, fp) };
            semantics(
                Operation::Unary,
                format!("{mnemonic}{letters}ri"),
                vec![dest],
                vec![source, imm(i64::from(*fbits))],
                None,
            )
        }
        Inst::VectorFill { rd, fill } => {
            let (name, sources) = match fill {
                Fill::Zero => ("MOVIv2d".to_string(), vec![]),
                Fill::Byte(byte) => ("MOVIv16b".to_string(), vec![imm(i64::from(*byte))]),
                Fill::Dup(arrangement, register) => {
                    let width = if matches!(arrangement, Arrangement::D2) { Width::X } else { Width::W };
                    (dup_name(*arrangement).to_string(), vec![gpr_loc(*register, width)])
                }
                Fill::Imm { invert, shape, imm8 } => {
                    let (lanes, shift) = match shape {
                        ImmShape::H8(shift) => ("8i16", shift),
                        ImmShape::S4(shift) => ("4i32", shift),
                        ImmShape::S2(shift) => ("2i32", shift),
                        ImmShape::S4Msl(shift) => ("4s_msl", shift),
                    };
                    let stem = if *invert { "MVNI" } else { "MOVI" };
                    (format!("{stem}v{lanes}"), vec![imm(i64::from(*imm8)), imm(i64::from(*shift))])
                }
                Fill::Mask(imm8) => ("MOVIv2d_ns".to_string(), vec![imm(i64::from(*imm8))]),
                Fill::MaskD(imm8) => ("MOVID".to_string(), vec![imm(i64::from(*imm8))]),
            };
            let narrow = matches!(
                fill,
                Fill::MaskD(_) | Fill::Imm { shape: ImmShape::S2(_), .. }
            );
            let width = if narrow { 64 } else { 128 };
            semantics(Operation::Move, name, vec![vreg_loc(*rd, width)], sources, None)
        }
        _ => return None,
    })
}

/// The mnemonic of a conversion and its size letters in the order its name
/// states them: the destination's first for `FMOV`, else the general
/// register's width then the float size.
fn convert_name(
    op: FpConvert,
    width: Width,
    size: FpSize,
) -> (&'static str, String) {
    let (w, s) = (width_letter(width), size_letter(size));
    match op {
        FpConvert::Bits { to_fp: true } => ("FMOV", format!("{s}{w}")),
        FpConvert::Bits { to_fp: false } => ("FMOV", format!("{w}{s}")),
        other => (named(&CONVERT, other), format!("{w}{s}")),
    }
}

/// The form of `dup v.<arrangement>, w`: the pin's name for it (the
/// sixteen-byte one is the producer's older spelling).
fn dup_name(arrangement: Arrangement) -> &'static str {
    match arrangement {
        Arrangement::B16 => "DUPv16b",
        Arrangement::H8 => "DUPv8i16gpr",
        Arrangement::S4 => "DUPv4i32gpr",
        Arrangement::D2 => "DUPv2i64gpr",
    }
}

/// `v16i8`, `v8i16`, `v4i32`, `v2i64`: the lanes as the pin's opcode names
/// spell them.
fn lane_name(lanes: Arrangement) -> &'static str {
    match lanes {
        Arrangement::B16 => "16i8",
        Arrangement::H8 => "8i16",
        Arrangement::S4 => "4i32",
        Arrangement::D2 => "2i64",
    }
}

/// `USHLLv8i8_shift`, `UADDWv16i8_v8i16`, `SMULLv4i16_v4i32`: the narrow
/// operand's arrangement, and the result's for all but the shift.
fn long_name(
    op: VectorLongOp,
    lane: NarrowLane,
    high: bool,
) -> String {
    let narrow = match (lane, high) {
        (NarrowLane::B, false) => "8i8",
        (NarrowLane::B, true) => "16i8",
        (NarrowLane::H, false) => "4i16",
        (NarrowLane::H, true) => "8i16",
        (NarrowLane::S, false) => "2i32",
        (NarrowLane::S, true) => "4i32",
    };
    let stem = op.mnemonic().to_uppercase();
    if op.is_shift() {
        return format!("{stem}v{narrow}_shift");
    }
    let wide = match lane {
        NarrowLane::B => "8i16",
        NarrowLane::H => "4i32",
        NarrowLane::S => "2i64",
    };
    format!("{stem}v{narrow}_v{wide}")
}

const LONG_OPS: [VectorLongOp; 8] = [
    VectorLongOp::Ushll,
    VectorLongOp::Sshll,
    VectorLongOp::Uaddw,
    VectorLongOp::Saddw,
    VectorLongOp::Umull,
    VectorLongOp::Smull,
    VectorLongOp::Umlal,
    VectorLongOp::Smlal,
];

/// `FADDv4f32`, `FNEGv2f64`.
fn vector_fp_name(
    mnemonic: &str,
    lanes: Arrangement,
) -> String {
    let lanes = if lanes == Arrangement::D2 { "2f64" } else { "4f32" };
    format!("{}v{lanes}", mnemonic.to_uppercase())
}

/// `UZP1v8i16`: the pin's name of an unzip.
fn unzip_name(
    second: bool,
    lanes: Arrangement,
) -> String {
    let lanes = match lanes {
        Arrangement::B16 => "v16i8",
        Arrangement::H8 => "v8i16",
        Arrangement::S4 => "v4i32",
        Arrangement::D2 => "v2i64",
    };
    format!("UZP{}{lanes}", if second { 2 } else { 1 })
}

/// `v8i8`, `v16i8`, `v4i16`, ...: the narrow arrangement of an `xtn` or
/// the source of a `sshll`, as the pin names it.
fn narrow_name(
    wide: Arrangement,
    high: bool,
) -> &'static str {
    match (wide, high) {
        (Arrangement::H8, false) => "v8i8",
        (Arrangement::H8, true) => "v16i8",
        (Arrangement::S4, false) => "v4i16",
        (Arrangement::S4, true) => "v8i16",
        (Arrangement::D2, false) => "v2i32",
        (Arrangement::D2, true) => "v4i32",
        (Arrangement::B16, _) => unreachable!("no lane is narrower than a byte"),
    }
}

/// `i8`, `i16`, `i32`, `i64`: the lane of an `INSvi32lane`.
fn ins_lane(lanes: Arrangement) -> &'static str {
    match lanes {
        Arrangement::B16 => "i8",
        Arrangement::H8 => "i16",
        Arrangement::S4 => "i32",
        Arrangement::D2 => "i64",
    }
}

/// `ADDVv4i32v`, `SMAXVv16i8v`, `ADDPv2i64p`.
fn reduce_name(
    op: VectorReduce,
    lanes: Arrangement,
) -> String {
    match op {
        VectorReduce::Addp => format!("ADDPv{}p", lane_name(lanes)),
        _ => format!("{}Vv{}v", op.mnemonic()[..op.mnemonic().len() - 1].to_uppercase(), lane_name(lanes)),
    }
}

const VECTOR_REDUCES: [VectorReduce; 6] = [
    VectorReduce::Addv,
    VectorReduce::Smaxv,
    VectorReduce::Sminv,
    VectorReduce::Umaxv,
    VectorReduce::Uminv,
    VectorReduce::Addp,
];

const VECTOR_FP_OPS: [VectorFpOp; 4] = [VectorFpOp::Add, VectorFpOp::Sub, VectorFpOp::Mul, VectorFpOp::Div];
const VECTOR_FP_UNARY: [VectorFpUnary; 6] = [
    VectorFpUnary::Neg,
    VectorFpUnary::Abs,
    VectorFpUnary::Scvtf,
    VectorFpUnary::Ucvtf,
    VectorFpUnary::Fcvtzs,
    VectorFpUnary::Fcvtzu,
];

/// `ADDv8i16`, `ANDv16i8`, `USHLv4i32`.
fn vector_op_name(
    op: VectorOp,
    lanes: Arrangement,
) -> String {
    let stem = op.mnemonic().to_uppercase();
    let lanes = if op.is_logical() { "16i8" } else { lane_name(lanes) };
    format!("{stem}v{lanes}")
}

const VECTOR_OPS: [VectorOp; 10] = [
    VectorOp::Add,
    VectorOp::Sub,
    VectorOp::Mul,
    VectorOp::And,
    VectorOp::Bic,
    VectorOp::Orr,
    VectorOp::Orn,
    VectorOp::Eor,
    VectorOp::Ushl,
    VectorOp::Sshl,
];
const ARRANGEMENTS: [Arrangement; 4] = [Arrangement::B16, Arrangement::H8, Arrangement::S4, Arrangement::D2];

pub(super) fn back(what: &Semantics) -> Option<Inst> {
    let name = what.name.as_deref()?;
    let (dest, s) = (what.dests.first(), &what.sources);
    if name == "NOTv16i8" {
        return Some(Inst::VectorNot { rd: vreg_of(dest?), rn: vreg_of(&s[0]) });
    }
    for lanes in [Arrangement::S4, Arrangement::D2] {
        for op in [VectorFma::Add, VectorFma::Sub] {
            if name == vector_fp_name(op.mnemonic(), lanes) {
                return Some(Inst::VectorFma { op, lanes, rd: vreg_of(dest?), rn: vreg_of(&s[1]), rm: vreg_of(&s[2]) });
            }
        }
    }
    for (lanes, op) in [Arrangement::S4, Arrangement::D2]
        .into_iter()
        .flat_map(|lanes| [VectorFma::Add, VectorFma::Sub].map(|op| (lanes, op)))
    {
        let lane = if lanes == Arrangement::D2 { "2i64" } else { "4i32" };
        if name == format!("{}v{lane}_indexed", op.mnemonic().to_uppercase()) {
            return Some(Inst::VectorFmaLane {
                op,
                lanes,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[1]),
                rm: vreg_of(&s[2]),
                index: value_of(&s[3]) as u8,
            });
        }
    }
    for (wide, high) in [
        (Arrangement::H8, false),
        (Arrangement::H8, true),
        (Arrangement::S4, false),
        (Arrangement::S4, true),
        (Arrangement::D2, false),
        (Arrangement::D2, true),
    ] {
        let narrow = narrow_name(wide, high);
        if name == format!("XTN{narrow}") {
            return Some(Inst::VectorNarrow { high, wide, rd: vreg_of(dest?), rn: vreg_of(s.last()?) });
        }
    }
    for (second, lanes) in ARRANGEMENTS.into_iter().flat_map(|lanes| [(false, lanes), (true, lanes)]) {
        if name == unzip_name(second, lanes) {
            return Some(Inst::VectorUnzip {
                second,
                lanes,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[0]),
                rm: vreg_of(&s[1]),
            });
        }
    }
    if name == "MULv4i32_indexed" {
        return Some(Inst::VectorMulLane {
            rd: vreg_of(dest?),
            rn: vreg_of(&s[0]),
            rm: vreg_of(&s[1]),
            index: value_of(&s[2]) as u8,
        });
    }
    for lanes in [Arrangement::S4, Arrangement::D2] {
        let lane = if lanes == Arrangement::D2 { "2i64" } else { "4i32" };
        if name == format!("FMULv{lane}_indexed") {
            return Some(Inst::VectorFmulLane {
                lanes,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[0]),
                rm: vreg_of(&s[1]),
                index: value_of(&s[2]) as u8,
            });
        }
    }
    for (op, lanes) in VECTOR_REDUCES.into_iter().flat_map(|op| ARRANGEMENTS.map(|lanes| (op, lanes))) {
        if name == reduce_name(op, lanes) {
            return Some(Inst::VectorReduce { op, lanes, rd: vreg_of(dest?), rn: vreg_of(&s[0]) });
        }
    }
    for lanes in ARRANGEMENTS {
        if name == format!("INSv{}gpr", ins_lane(lanes)) {
            return Some(Inst::VectorInsGpr {
                lanes,
                rd: vreg_of(dest?),
                rn: gpr_of(&s[2]),
                index: value_of(&s[1]) as u8,
            });
        }
        if name == format!("UMOVv{}", ins_lane(lanes)) {
            return Some(Inst::VectorUmov {
                lanes,
                rd: gpr_of(dest?),
                rn: vreg_of(&s[0]),
                index: value_of(&s[1]) as u8,
            });
        }
        if name == format!("INSv{}lane", ins_lane(lanes)) {
            return Some(Inst::VectorInsLane {
                lanes,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[2]),
                index: value_of(&s[1]) as u8,
                src: value_of(&s[3]) as u8,
            });
        }
        if name == format!("DUP{}", ins_lane(lanes)) {
            return Some(Inst::VectorDupScalar {
                lanes,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[0]),
                index: value_of(&s[1]) as u8,
            });
        }
        if name == format!("DUPv{}lane", lane_name(lanes)) {
            return Some(Inst::VectorDupLane {
                lanes,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[0]),
                index: value_of(&s[1]) as u8,
            });
        }
    }
    for lanes in [Arrangement::S4, Arrangement::D2] {
        for op in VECTOR_FP_OPS {
            if name == vector_fp_name(op.mnemonic(), lanes) {
                return Some(Inst::VectorFpBinary {
                    op,
                    lanes,
                    rd: vreg_of(dest?),
                    rn: vreg_of(&s[0]),
                    rm: vreg_of(&s[1]),
                });
            }
        }
        for op in VECTOR_FP_UNARY {
            if name == vector_fp_name(op.mnemonic(), lanes) {
                return Some(Inst::VectorFpUnary { op, lanes, rd: vreg_of(dest?), rn: vreg_of(&s[0]) });
            }
        }
    }
    for op in VECTOR_OPS {
        for lanes in ARRANGEMENTS {
            if name == vector_op_name(op, lanes)
                && (!op.is_logical() || lanes == Arrangement::B16)
                && !(op == VectorOp::Mul && lanes == Arrangement::D2)
            {
                return Some(Inst::VectorBinary {
                    op,
                    lanes,
                    rd: vreg_of(dest?),
                    rn: vreg_of(&s[0]),
                    rm: vreg_of(&s[1]),
                });
            }
        }
    }
    for op in LONG_OPS {
        for lane in [NarrowLane::B, NarrowLane::H, NarrowLane::S] {
            for high in [false, true] {
                if name != long_name(op, lane, high) {
                    continue;
                }
                return Some(if op.is_shift() {
                    Inst::VectorLongShift {
                        op,
                        lane,
                        high,
                        rd: vreg_of(dest?),
                        rn: vreg_of(&s[0]),
                        amount: value_of(&s[1]) as u8,
                    }
                } else {
                    // The accumulating forms list the tied `rd` first.
                    let at = usize::from(op.accumulates());
                    Inst::VectorLong {
                        op,
                        lane,
                        high,
                        rd: vreg_of(dest?),
                        rn: vreg_of(&s[at]),
                        rm: vreg_of(&s[at + 1]),
                    }
                });
            }
        }
    }
    for (stem, op) in [("SHL", VectorShift::Shl), ("USHR", VectorShift::Ushr), ("SSHR", VectorShift::Sshr)] {
        for lanes in ARRANGEMENTS {
            if name == format!("{stem}v{}_shift", lane_name(lanes)) {
                return Some(Inst::VectorShift {
                    op,
                    lanes,
                    rd: vreg_of(dest?),
                    rn: vreg_of(&s[0]),
                    amount: value_of(&s[1]) as u8,
                });
            }
        }
    }
    match name {
        "ORRv16b" => return Some(Inst::VectorMov { rd: vreg_of(dest?), rn: vreg_of(&s[0]) }),
        "MOVIv2d" => return Some(Inst::VectorFill { rd: vreg_of(dest?), fill: Fill::Zero }),
        "MOVIv16b" => return Some(Inst::VectorFill { rd: vreg_of(dest?), fill: Fill::Byte(value_of(&s[0]) as u8) }),
        "MOVIv2d_ns" => return Some(Inst::VectorFill { rd: vreg_of(dest?), fill: Fill::Mask(value_of(&s[0]) as u8) }),
        "MOVID" => return Some(Inst::VectorFill { rd: vreg_of(dest?), fill: Fill::MaskD(value_of(&s[0]) as u8) }),
        "SCVTFv1i32" | "UCVTFv1i32" | "SCVTFv1i64" | "UCVTFv1i64" => {
            let size = if name.ends_with("64") { FpSize::D } else { FpSize::S };
            return Some(Inst::FpLaneToFp {
                signed: name.starts_with('S'),
                size,
                rd: vreg_of(dest?),
                rn: vreg_of(&s[0]),
            });
        }
        _ => {}
    }
    if let Some(arrangement) = [Arrangement::B16, Arrangement::H8, Arrangement::S4, Arrangement::D2]
        .into_iter()
        .find(|one| name == dup_name(*one))
    {
        return Some(Inst::VectorFill { rd: vreg_of(dest?), fill: Fill::Dup(arrangement, gpr_of(&s[0])) });
    }
    for (stem, invert) in [("MOVI", false), ("MVNI", true)] {
        let Some(lanes) = name.strip_prefix(stem).and_then(|rest| rest.strip_prefix('v')) else { continue };
        let (imm8, shift) = (value_of(s.first()?) as u8, s.get(1).map_or(0, value_of) as u8);
        let shape = match lanes {
            "8i16" => ImmShape::H8(shift),
            "4i32" => ImmShape::S4(shift),
            "2i32" => ImmShape::S2(shift),
            "4s_msl" => ImmShape::S4Msl(shift),
            _ => continue,
        };
        return Some(Inst::VectorFill { rd: vreg_of(dest?), fill: Fill::Imm { invert, shape, imm8 } });
    }
    let at = |i: usize| vreg_of(&s[i]);
    let first = |text: &str| size_named(text.chars().next().unwrap_or('H'));
    if let Some((op, rest)) = prefix(&UNARY, name) {
        let (size, form) = letters(rest);
        return match (op, size.len(), form) {
            (FpUnary::Mov, 1, "i") => {
                Some(Inst::FpMoveImm { size: first(size), rd: vreg_of(dest?), imm8: value_of(&s[0]) as u8 })
            }
            (_, 1, "r") => Some(Inst::FpUnary { op, size: first(size), rd: vreg_of(dest?), rn: at(0) }),
            (FpUnary::Mov, 2, form) => {
                let op = FpConvert::Bits { to_fp: matches!(size.as_bytes()[0], b'S' | b'D' | b'H') };
                let (width, size) = if to_fp(op) {
                    (width_named(size.as_bytes()[1] as char), first(size))
                } else {
                    (width_named(size.as_bytes()[0] as char), size_named(size.as_bytes()[1] as char))
                };
                let (gpr, fpr) = if to_fp(op) { (gpr_of(&s[0]), vreg_of(dest?)) } else { (gpr_of(dest?), at(0)) };
                (form == "r").then_some(Inst::FpInt { op, width, size, gpr, fpr })
            }
            _ => None,
        };
    }
    if let Some((op, rest)) = prefix(&BINARY, name) {
        let (size, _) = letters(rest);
        return Some(Inst::FpBinary { op, size: first(size), rd: vreg_of(dest?), rn: at(0), rm: at(1) });
    }
    if let Some((op, rest)) = prefix(&FUSED, name) {
        let (size, _) = letters(rest);
        return Some(Inst::FpFused { op, size: first(size), rd: vreg_of(dest?), rn: at(0), rm: at(1), ra: at(2) });
    }
    if let Some((signalling, rest)) = prefix(&[(false, "FCMP"), (true, "FCMPE")], name) {
        let (size, form) = letters(rest);
        return Some(Inst::FpCompare { signalling, size: first(size), rn: at(0), rm: (form == "rr").then(|| at(1)) });
    }
    if let Some(rest) = name.strip_prefix("FCSEL") {
        let (size, _) = letters(rest);
        return Some(Inst::FpSelect {
            size: first(size),
            rd: vreg_of(dest?),
            rn: at(0),
            rm: at(1),
            cond: member::<Cond>(&super::integer::CONDS, &s[2]),
        });
    }
    if let Some((op, rest)) = prefix(&CONVERT, name) {
        let (pair, form) = letters(rest);
        let width = width_named(pair.as_bytes()[0] as char);
        let size = size_named(pair.as_bytes()[1] as char);
        let (gpr, fpr) = if to_fp(op) { (gpr_of(&s[0]), vreg_of(dest?)) } else { (gpr_of(dest?), at(0)) };
        return Some(if form == "ri" {
            Inst::FpFixed { op, width, size, gpr, fpr, fbits: value_of(&s[1]) as u8 }
        } else {
            Inst::FpInt { op, width, size, gpr, fpr }
        });
    }
    if let Some(rest) = name.strip_prefix("FCVT") {
        let (pair, _) = letters(rest);
        let (to, from) = (size_named(pair.as_bytes()[0] as char), size_named(pair.as_bytes()[1] as char));
        return Some(Inst::FpToFp { to, from, rd: vreg_of(dest?), rn: at(0) });
    }
    None
}
