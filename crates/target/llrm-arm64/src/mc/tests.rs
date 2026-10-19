//! The encoder against the pinned `llvm-mc`: every instruction below is
//! printed in LLVM syntax, assembled by the pinned tool, and the word it writes
//! must be the one `encode` gives. The premise is checked first: the tool must
//! be the pinned build and must answer once per instruction.

use std::path::Path;
use std::process::Command;

use super::encode::encode;
use super::inst::*;

fn pinned_mc() -> Option<String> {
    let build = std::env::var("LLVM_BUILD")
        .unwrap_or_else(|_| format!("{}/scratch/llvm-build-aarch64", std::env::var("HOME").unwrap_or_default()));
    let path = format!("{build}/bin/llvm-mc");
    if !Path::new(&path).exists() {
        return None;
    }
    let version = Command::new(&path).arg("--version").output().ok()?;
    assert!(String::from_utf8_lossy(&version.stdout).contains("24.0.0git"), "not the pinned LLVM");
    Some(path)
}

/// A byte as `-show-encoding` spells it: `0x91`, `0x90'A'` and `0b100101AA`
/// where a fixup fills bits (`A` stands for a bit left zero), or `A` alone.
fn fixed_byte(text: &str) -> u8 {
    if text == "A" {
        0
    } else if let Some(bits) = text.strip_prefix("0b") {
        u8::from_str_radix(&bits.replace('A', "0"), 2).unwrap()
    } else {
        u8::from_str_radix(text.trim_start_matches("0x").split('\'').next().unwrap(), 16).unwrap()
    }
}

/// The words the pinned `llvm-mc` assembles `lines` to, one per line.
fn assembled(
    mc: &str,
    lines: &[String],
) -> Vec<u32> {
    let source = lines.join("\n") + "\n";
    let mut child = Command::new(mc)
        .args(["-triple", "arm64-apple-macos", "-show-encoding"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), source.as_bytes()).unwrap();
    let said = child.wait_with_output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let text = String::from_utf8_lossy(&said.stdout);
    let words: Vec<u32> = text
        .lines()
        .filter_map(|line| line.split("encoding: [").nth(1))
        .map(|rest| {
            let bytes: Vec<u8> = rest.trim_end_matches(']').split(',').map(|byte| fixed_byte(byte.trim())).collect();
            u32::from_le_bytes(bytes.try_into().unwrap())
        })
        .collect();
    assert_eq!(words.len(), lines.len(), "the tool answered {} of {} lines:\n{text}", words.len(), lines.len());
    words
}

fn check(insts: &[Inst]) {
    let Some(mc) = pinned_mc() else {
        eprintln!("skipped: no pinned LLVM with AArch64");
        return;
    };
    let lines: Vec<String> = insts.iter().map(|one| one.to_string()).collect();
    let theirs = assembled(&mc, &lines);
    let mine: Vec<Result<u32, String>> =
        insts.iter().map(|one| encode(one).map(|done| done.word).map_err(|why| why.0)).collect();
    let bad: Vec<String> = lines
        .iter()
        .zip(mine.iter().zip(&theirs))
        .filter(|(_, (mine, theirs))| mine.as_ref().ok() != Some(*theirs))
        .map(|(line, (mine, theirs))| format!("{line}: ours {mine:08x?}, llvm-mc {theirs:08x}"))
        .collect();
    assert!(bad.is_empty(), "{} of {} differ:\n{}", bad.len(), lines.len(), bad.join("\n"));
}

const WIDTHS: [Width; 2] = [Width::W, Width::X];
const REGS: [u8; 6] = [0, 1, 7, 18, 29, 30];

#[test]
fn add_and_subtract() {
    let mut all = Vec::new();
    for (subtract, set_flags) in [(false, false), (false, true), (true, false), (true, true)] {
        let op = AddSub { subtract, set_flags };
        for width in WIDTHS {
            for (rd, rn) in [(0, 1), (30, 29), (31, 3), (4, 31)] {
                for (imm12, shift12) in [(0, false), (1, false), (4095, false), (1, true), (4095, true)] {
                    all.push(Inst::AddSubImm { op, width, rd: Gpr(rd), rn: Gpr(rn), imm12, shift12 });
                }
                for (shift, amount) in [(Shift::Lsl, 0), (Shift::Lsl, 3), (Shift::Lsr, 1), (Shift::Asr, 31)] {
                    if rd != 31 && rn != 31 {
                        all.push(Inst::AddSubShifted {
                            op,
                            width,
                            rd: Gpr(rd),
                            rn: Gpr(rn),
                            rm: Gpr(2),
                            shift,
                            amount,
                        });
                    }
                }
            }
            for extend in [Extend::Uxtb, Extend::Uxth, Extend::Uxtw, Extend::Sxtw, Extend::Sxtb, Extend::Sxth] {
                for amount in [0, 2, 4] {
                    all.push(Inst::AddSubExtended { op, width, rd: Gpr(1), rn: Gpr(2), rm: Gpr(3), extend, amount });
                }
            }
        }
        for extend in [Extend::Uxtx, Extend::Sxtx] {
            all.push(Inst::AddSubExtended {
                op,
                width: Width::X,
                rd: Gpr(1),
                rn: Gpr(31),
                rm: Gpr(3),
                extend,
                amount: 1,
            });
        }
    }
    check(&all);
}

#[test]
fn logical() {
    let mut all = Vec::new();
    for op in [Logic::And, Logic::Orr, Logic::Eor, Logic::Ands] {
        for width in WIDTHS {
            let values: &[u64] = match width {
                Width::W => &[1, 0xFF, 0xFFFF, 0x5555_5555, 0x0F0F_0F0F, 0x8000_0000, 0xFFFF_FFFE, 0x3FFF_FFFF],
                Width::X => &[
                    1,
                    0xFF,
                    0x1_0000_0000,
                    0xFFFF_0000_FFFF_0000,
                    0x7FFF_FFFF_FFFF_FFFF,
                    0x5555_5555_5555_5555,
                    0x00FF_00FF_00FF_00FF,
                    0xFFFF_FFFF_FFFF_FFFE,
                    0x0000_FFFF_FFFF_0000,
                ],
            };
            for &imm in values {
                all.push(Inst::LogicImm { op, width, rd: Gpr(0), rn: Gpr(1), imm });
            }
            for invert in [false, true] {
                for (shift, amount) in [(Shift::Lsl, 0), (Shift::Lsr, 7), (Shift::Asr, 1), (Shift::Ror, 31)] {
                    all.push(Inst::LogicShifted {
                        op,
                        invert,
                        width,
                        rd: Gpr(4),
                        rn: Gpr(5),
                        rm: Gpr(6),
                        shift,
                        amount,
                    });
                }
            }
        }
    }
    check(&all);
}

#[test]
fn moves_and_bitfields() {
    let mut all = Vec::new();
    for op in [Wide::Not, Wide::Zero, Wide::Keep] {
        for width in WIDTHS {
            for lsl in [0, 16, 32, 48] {
                if width == Width::W && lsl > 16 {
                    continue;
                }
                for imm16 in [0, 1, 0xFFFF] {
                    all.push(Inst::MoveWide { op, width, rd: Gpr(9), imm16, lsl });
                }
            }
        }
    }
    for op in [Bitfield::Sbfm, Bitfield::Bfm, Bitfield::Ubfm] {
        for width in WIDTHS {
            for (immr, imms) in [(0, 7), (0, 15), (3, 9), (31, 31), (1, 0)] {
                all.push(Inst::BitfieldMove { op, width, rd: Gpr(1), rn: Gpr(2), immr, imms });
            }
        }
    }
    for width in WIDTHS {
        all.push(Inst::Extract { width, rd: Gpr(1), rn: Gpr(2), rm: Gpr(3), lsb: 5 });
    }
    all.push(Inst::BitfieldMove { op: Bitfield::Sbfm, width: Width::X, rd: Gpr(1), rn: Gpr(2), immr: 0, imms: 31 });
    check(&all);
}

#[test]
fn arithmetic_classes() {
    let mut all = Vec::new();
    for width in WIDTHS {
        for op in [Div::Udiv, Div::Sdiv, Div::Lslv, Div::Lsrv, Div::Asrv, Div::Rorv] {
            all.push(Inst::Divide { op, width, rd: Gpr(1), rn: Gpr(2), rm: Gpr(3) });
        }
        for op in [Unary::Rbit, Unary::Rev16, Unary::Rev, Unary::Clz, Unary::Cls] {
            all.push(Inst::OneSource { op, width, rd: Gpr(1), rn: Gpr(2) });
        }
        for op in [Mul3::Madd, Mul3::Msub] {
            all.push(Inst::ThreeSource { op, width, rd: Gpr(1), rn: Gpr(2), rm: Gpr(3), ra: Gpr(4) });
        }
        for op in [Select::Csel, Select::Csinc, Select::Csinv, Select::Csneg] {
            for cond in [Cond::Eq, Cond::Ne, Cond::Lt, Cond::Hi, Cond::Al] {
                all.push(Inst::CondSelect { op, width, rd: Gpr(1), rn: Gpr(2), rm: Gpr(3), cond });
            }
        }
        for negate in [false, true] {
            all.push(Inst::CondCompare {
                negate,
                width,
                rn: Gpr(1),
                rm_or_imm: CcOperand::Reg(Gpr(2)),
                nzcv: 4,
                cond: Cond::Ge,
            });
            all.push(Inst::CondCompare {
                negate,
                width,
                rn: Gpr(1),
                rm_or_imm: CcOperand::Imm5(31),
                nzcv: 0,
                cond: Cond::Lo,
            });
        }
    }
    for op in [Mul3::Smaddl, Mul3::Smsubl, Mul3::Umaddl, Mul3::Umsubl, Mul3::Smulh, Mul3::Umulh] {
        let ra = if matches!(op, Mul3::Smulh | Mul3::Umulh) { Gpr(31) } else { Gpr(4) };
        all.push(Inst::ThreeSource { op, width: Width::X, rd: Gpr(1), rn: Gpr(2), rm: Gpr(3), ra });
    }
    check(&all);
}

#[test]
fn branches() {
    let mut all = Vec::new();
    for offset in [0, 4, -4, 128, -1024, 0x7FF_FFFC, -0x800_0000] {
        for link in [false, true] {
            all.push(Inst::Branch { link, target: Target::Offset(offset) });
        }
    }
    for offset in [0, 8, -8, 0xF_FFFC, -0x10_0000] {
        for cond in [Cond::Eq, Cond::Ne, Cond::Ge, Cond::Hi, Cond::Nv] {
            all.push(Inst::BranchCond { cond, target: Target::Offset(offset) });
        }
        for (nonzero, width) in [(false, Width::W), (true, Width::W), (false, Width::X), (true, Width::X)] {
            all.push(Inst::CompareBranch { nonzero, width, rt: Gpr(3), target: Target::Offset(offset) });
        }
    }
    for offset in [0, 4, -4, 0x7FFC, -0x8000] {
        for (bit, rt) in [(0, 1), (5, 2), (31, 3), (32, 4), (63, 5)] {
            for nonzero in [false, true] {
                all.push(Inst::TestBranch { nonzero, rt: Gpr(rt), bit, target: Target::Offset(offset) });
            }
        }
    }
    for link in [false, true] {
        all.push(Inst::BranchReg { link, rn: Gpr(16) });
    }
    all.push(Inst::Ret { rn: Gpr::LR });
    all.push(Inst::Ret { rn: Gpr(5) });
    all.push(Inst::Svc { imm16: 0x80 });
    all.push(Inst::Brk { imm16: 1 });
    all.push(Inst::Nop);
    for (page, offset) in [(false, 0), (false, 1023), (false, -1048576), (true, 0), (true, 4096), (true, -8192)] {
        all.push(Inst::Adr { page, rd: Gpr(1), target: Target::Offset(offset) });
    }
    check(&all);
}

#[test]
fn symbolic_targets_leave_zero_fields() {
    let symbol = |name: &str, addend| Target::Symbol { name: name.into(), addend };
    check(&[
        Inst::Branch { link: true, target: symbol("_puts", 0) },
        Inst::Adr { page: true, rd: Gpr(0), target: symbol("_msg", 0) },
        Inst::Adr { page: true, rd: Gpr(0), target: symbol("_msg", 16) },
        Inst::AddPageOffset { rd: Gpr(0), rn: Gpr(0), target: symbol("_msg", 0) },
        Inst::Load {
            mem: Memory::Int { width: Width::W, bytes: 4, sign: false, reg: Gpr(2) },
            addr: Addr::PageOffset { base: Gpr(1), target: symbol("_count", 0), got: false },
        },
        Inst::Load {
            mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(3) },
            addr: Addr::PageOffset { base: Gpr(3), target: symbol("_ext", 0), got: true },
        },
    ]);
}

#[test]
fn loads_and_stores() {
    let mut all = Vec::new();
    let ints = [
        (Width::W, 1, false),
        (Width::W, 2, false),
        (Width::W, 4, false),
        (Width::X, 8, false),
        (Width::W, 1, true),
        (Width::X, 1, true),
        (Width::W, 2, true),
        (Width::X, 2, true),
        (Width::X, 4, true),
    ];
    let addrs = |bytes: i64| {
        vec![
            Addr::Offset { base: Gpr(1), imm: 0 },
            Addr::Offset { base: Gpr(31), imm: bytes * 4095 },
            Addr::Offset { base: Gpr(1), imm: bytes * 3 },
            Addr::Offset { base: Gpr(1), imm: -1 },
            Addr::Offset { base: Gpr(1), imm: 255 },
            Addr::Offset { base: Gpr(1), imm: -256 },
            Addr::Pre { base: Gpr(1), imm: -16 },
            Addr::Post { base: Gpr(31), imm: 8 },
            Addr::Register { base: Gpr(1), index: Gpr(2), extend: Extend::Uxtx, shifted: false },
            Addr::Register { base: Gpr(1), index: Gpr(2), extend: Extend::Uxtx, shifted: true },
            Addr::Register { base: Gpr(1), index: Gpr(2), extend: Extend::Sxtw, shifted: true },
            Addr::Register { base: Gpr(1), index: Gpr(2), extend: Extend::Uxtw, shifted: false },
            Addr::Register { base: Gpr(1), index: Gpr(2), extend: Extend::Sxtx, shifted: false },
        ]
    };
    for (width, bytes, sign) in ints {
        let mem = Memory::Int { width, bytes, sign, reg: Gpr(5) };
        for addr in addrs(i64::from(bytes)) {
            all.push(Inst::Load { mem, addr: addr.clone() });
            if !sign {
                all.push(Inst::Store { mem, addr });
            }
        }
    }
    for bytes in [1u8, 2, 4, 8, 16] {
        let mem = Memory::Fp { bytes, reg: Vreg(6) };
        for addr in addrs(i64::from(bytes)) {
            all.push(Inst::Load { mem, addr: addr.clone() });
            all.push(Inst::Store { mem, addr });
        }
    }
    for (first, second, bytes) in [
        (
            Memory::Int { width: Width::W, bytes: 4, sign: false, reg: Gpr(1) },
            Memory::Int { width: Width::W, bytes: 4, sign: false, reg: Gpr(2) },
            4,
        ),
        (
            Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(29) },
            Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(30) },
            8,
        ),
        (Memory::Fp { bytes: 4, reg: Vreg(1) }, Memory::Fp { bytes: 4, reg: Vreg(2) }, 4),
        (Memory::Fp { bytes: 8, reg: Vreg(8) }, Memory::Fp { bytes: 8, reg: Vreg(9) }, 8),
        (Memory::Fp { bytes: 16, reg: Vreg(8) }, Memory::Fp { bytes: 16, reg: Vreg(9) }, 16),
    ] {
        for addr in [
            Addr::Offset { base: Gpr(31), imm: 0 },
            Addr::Offset { base: Gpr(31), imm: 63 * bytes },
            Addr::Offset { base: Gpr(31), imm: -64 * bytes },
            Addr::Pre { base: Gpr(31), imm: -16 * bytes / bytes * 2 },
            Addr::Post { base: Gpr(31), imm: 2 * bytes },
        ] {
            all.push(Inst::LoadPair { first, second, addr: addr.clone() });
            all.push(Inst::StorePair { first, second, addr });
        }
    }
    check(&all);
}

#[test]
fn floating_point() {
    let mut all = Vec::new();
    for size in [FpSize::S, FpSize::D] {
        for op in [FpUnary::Mov, FpUnary::Abs, FpUnary::Neg, FpUnary::Sqrt] {
            all.push(Inst::FpUnary { op, size, rd: Vreg(1), rn: Vreg(2) });
        }
        for rounding in [
            Rounding::Nearest,
            Rounding::Up,
            Rounding::Down,
            Rounding::Zero,
            Rounding::Away,
            Rounding::Exact,
            Rounding::Current,
        ] {
            all.push(Inst::FpUnary { op: FpUnary::Rint(rounding), size, rd: Vreg(1), rn: Vreg(2) });
        }
        for op in [
            FpBinary::Mul,
            FpBinary::Div,
            FpBinary::Add,
            FpBinary::Sub,
            FpBinary::Max,
            FpBinary::Min,
            FpBinary::MaxNm,
            FpBinary::MinNm,
            FpBinary::Nmul,
        ] {
            all.push(Inst::FpBinary { op, size, rd: Vreg(1), rn: Vreg(2), rm: Vreg(3) });
        }
        for op in [FpFused::Madd, FpFused::Msub, FpFused::Nmadd, FpFused::Nmsub] {
            all.push(Inst::FpFused { op, size, rd: Vreg(1), rn: Vreg(2), rm: Vreg(3), ra: Vreg(4) });
        }
        for signalling in [false, true] {
            all.push(Inst::FpCompare { signalling, size, rn: Vreg(1), rm: Some(Vreg(2)) });
            all.push(Inst::FpCompare { signalling, size, rn: Vreg(1), rm: None });
        }
        all.push(Inst::FpSelect { size, rd: Vreg(1), rn: Vreg(2), rm: Vreg(3), cond: Cond::Mi });
        for imm8 in [0x00, 0x70, 0x80, 0xFF, 0x2A] {
            all.push(Inst::FpMoveImm { size, rd: Vreg(1), imm8 });
        }
        for width in WIDTHS {
            for op in [
                FpConvert::ToFp { signed: true },
                FpConvert::ToFp { signed: false },
                FpConvert::ToIntZero { signed: true },
                FpConvert::ToIntZero { signed: false },
            ] {
                all.push(Inst::FpInt { op, width, size, gpr: Gpr(1), fpr: Vreg(2) });
            }
        }
        all.push(Inst::FpInt {
            op: FpConvert::Bits { to_fp: true },
            width: if size == FpSize::S { Width::W } else { Width::X },
            size,
            gpr: Gpr(1),
            fpr: Vreg(2),
        });
        all.push(Inst::FpInt {
            op: FpConvert::Bits { to_fp: false },
            width: if size == FpSize::S { Width::W } else { Width::X },
            size,
            gpr: Gpr(1),
            fpr: Vreg(2),
        });
    }
    all.push(Inst::FpToFp { to: FpSize::D, from: FpSize::S, rd: Vreg(1), rn: Vreg(2) });
    all.push(Inst::FpToFp { to: FpSize::S, from: FpSize::D, rd: Vreg(1), rn: Vreg(2) });
    let _ = REGS;
    check(&all);
}
