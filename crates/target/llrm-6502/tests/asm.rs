//! The 6502 emitter's output assembled by ca65, linked for cc65's `sim65` and run. Each program
//! returns a byte in A, which is the exit code; the expected values are worked out by hand from the
//! 6502 instruction set (ADC adds the carry in, SBC clears C on a borrow), not from the emitter.

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::dagisel::generated::ids::opcode as op;
use llrm_6502::emit::{MInst, Operand, equates, function};
use llrm_6502::platform::{Platform, SIM65};

type Body = Vec<Result<MInst, &'static str>>;

fn reg(name: &str) -> Operand {
    Operand::reg(name)
}

fn i(
    opcode: u16,
    operands: Vec<Operand>,
) -> Result<MInst, &'static str> {
    Ok(MInst::new(opcode, operands))
}

fn imm(v: i64) -> Operand {
    Operand::Imm(v)
}

/// Assembles `functions` after the platform's equates, runs `_main` and returns its exit code.
fn run(
    name: &str,
    functions: &[String],
) -> i32 {
    let platform = Platform::parse(SIM65).expect("sim65 row");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("asm-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("p.s");
    std::fs::write(&source, format!("{}{}", equates(&platform), functions.concat())).unwrap();
    let binary = dir.join("p.bin");
    let built = Command::new("cl65").args(["-t", "sim6502", "-o"]).arg(&binary).arg(&source).output().expect("cl65");
    assert!(built.status.success(), "cl65: {}", String::from_utf8_lossy(&built.stderr));
    Command::new("sim65").arg(&binary).status().expect("sim65").code().expect("exit code")
}

fn main_returning(
    mut body: Body,
    byte: &str,
) -> String {
    body.push(i(op::COPY8, vec![reg("a"), reg(byte)]));
    body.push(i(op::RET, vec![]));
    function("_main", &body).expect("expands")
}

/// 0x12F0 + 0x0020 = 0x1310: the low byte 0x10, the high byte 0x13 (the carry from the low bytes
/// is 0, so the high bytes add to 0x13).
#[test]
fn a_word_add_gives_both_bytes() {
    let prefix = || -> Body {
        vec![
            i(op::LDI16, vec![reg("rs1"), imm(0x12F0)]),
            i(op::LDI16, vec![reg("rs2"), imm(0x0020)]),
            i(op::ADD16, vec![reg("rs3"), reg("rs1"), reg("rs2")]),
        ]
    };
    assert_eq!(run("add16-lo", &[main_returning(prefix(), "rc6")]), 0x10);
    assert_eq!(run("add16-hi", &[main_returning(prefix(), "rc7")]), 0x13);
}

/// An `i32` add as two words joined by the carry: 0x0001FFFF + 0x00000001 = 0x00020000. The low
/// words 0xFFFF + 1 wrap to 0 with a carry out; the high words 1 + 0 + carry = 2.
#[test]
fn a_dword_add_carries_between_words() {
    let body: Body = vec![
        i(op::LDI16, vec![reg("rs1"), imm(0xFFFF)]),
        i(op::LDI16, vec![reg("rs2"), imm(1)]),
        i(op::LDI16, vec![reg("rs4"), imm(1)]),
        i(op::LDI16, vec![reg("rs5"), imm(0)]),
        i(op::ADDO16, vec![reg("rs3"), reg("c"), reg("rs1"), reg("rs2")]),
        i(op::ADDOC16, vec![reg("rs6"), reg("c"), reg("rs4"), reg("rs5")]),
    ];
    assert_eq!(run("add32-hi", &[main_returning(body.clone(), "rc12")]), 2);
    assert_eq!(run("add32-lo", &[main_returning(body, "rc6")]), 0);
}

/// 0x00020000 - 1 = 0x0001FFFF: the low words 0 - 1 borrow and wrap to 0xFFFF, the high words
/// 2 - 0 - borrow = 1.
#[test]
fn a_dword_subtract_borrows_between_words() {
    let body: Body = vec![
        i(op::LDI16, vec![reg("rs1"), imm(0)]),
        i(op::LDI16, vec![reg("rs2"), imm(1)]),
        i(op::LDI16, vec![reg("rs4"), imm(2)]),
        i(op::LDI16, vec![reg("rs5"), imm(0)]),
        i(op::SUBO16, vec![reg("rs3"), reg("c"), reg("rs1"), reg("rs2")]),
        i(op::SUBOC16, vec![reg("rs6"), reg("c"), reg("rs4"), reg("rs5")]),
    ];
    assert_eq!(run("sub32-lo", &[main_returning(body.clone(), "rc6")]), 0xFF);
    assert_eq!(run("sub32-hi", &[main_returning(body, "rc12")]), 1);
}

/// The sum 1 + 2 + .. + 10 = 55 in a counted loop.
#[test]
fn a_counted_loop_sums_to_55() {
    let body: Body = vec![
        i(op::LDI8, vec![reg("rc2"), imm(0)]),
        i(op::LDI8, vec![reg("rc3"), imm(1)]),
        i(op::LDI8, vec![reg("rc4"), imm(11)]),
        i(op::LDI8, vec![reg("rc5"), imm(1)]),
        Err("loop"),
        i(op::ADD8, vec![reg("rc2"), reg("rc2"), reg("rc3")]),
        i(op::ADD8, vec![reg("rc3"), reg("rc3"), reg("rc5")]),
        i(op::CMP8, vec![reg("p"), reg("rc3"), reg("rc4")]),
        i(op::BNE, vec![Operand::Label("loop".into()), reg("p")]),
    ];
    assert_eq!(run("loop", &[main_returning(body, "rc2")]), 55);
}

/// `f(7, 5)` with the arguments in rc2 and rc3 and the result in A: 12.
#[test]
fn a_call_passes_arguments_in_rc2_and_rc3_and_returns_in_a() {
    let main: Body = vec![
        i(op::LDI8, vec![reg("rc2"), imm(7)]),
        i(op::LDI8, vec![reg("rc3"), imm(5)]),
        i(op::JSR, vec![Operand::Label("add_bytes".into())]),
        i(op::RET, vec![]),
    ];
    let callee: Body = vec![
        i(op::ADD8, vec![reg("rc4"), reg("rc2"), reg("rc3")]),
        i(op::COPY8, vec![reg("a"), reg("rc4")]),
        i(op::RET, vec![]),
    ];
    let functions = [function("_main", &main).unwrap(), function("add_bytes", &callee).unwrap()];
    assert_eq!(run("call", &functions), 12);
}

fn compare(
    a: i64,
    b: i64,
) -> Body {
    vec![
        i(op::LDI8, vec![reg("rc2"), imm(a)]),
        i(op::LDI8, vec![reg("rc3"), imm(b)]),
        i(op::CMP8, vec![reg("p"), reg("rc2"), reg("rc3")]),
    ]
}

/// `3 <u 5` is C clear after `cmp`: SETCC with code CC (2) gives 1; `5 <u 3` gives 0.
#[test]
fn setcc_materializes_an_unsigned_less_than() {
    let setcc = |mut body: Body| {
        body.push(i(op::SETCC8, vec![reg("rc4"), imm(2), reg("p")]));
        body
    };
    assert_eq!(run("setcc-lt", &[main_returning(setcc(compare(3, 5)), "rc4")]), 1);
    assert_eq!(run("setcc-ge", &[main_returning(setcc(compare(5, 3)), "rc4")]), 0);
}

/// `-1 <s 1` as the lowering writes it: both sign bits flipped (0xFF -> 0x7F, 1 -> 0x81), then
/// unsigned: 0x7F < 0x81, so CC holds and the result is 1.
#[test]
fn a_signed_compare_through_flipped_sign_bits() {
    let mut body = compare(0x7F, 0x81);
    body.push(i(op::SETCC8, vec![reg("rc4"), imm(2), reg("p")]));
    assert_eq!(run("signed-lt", &[main_returning(body, "rc4")]), 1);
}

/// CMOV with EQ: equal operands take the true value (20), unequal ones the false value (10).
#[test]
fn cmov_picks_the_true_value_only_when_the_condition_holds() {
    // The values are loaded first: a load sets N and Z, so none may sit between the compare and the CMOV.
    let cmov = |a, b| {
        let mut body: Body = vec![i(op::LDI8, vec![reg("rc5"), imm(10)]), i(op::LDI8, vec![reg("rc6"), imm(20)])];
        body.extend(compare(a, b));
        body.push(i(op::CMOV8, vec![reg("rc4"), reg("rc5"), reg("rc6"), imm(0), reg("p")]));
        body
    };
    assert_eq!(run("cmov-eq", &[main_returning(cmov(4, 4), "rc4")]), 20);
    assert_eq!(run("cmov-ne", &[main_returning(cmov(4, 5), "rc4")]), 10);
}

fn word_compare(
    compare: u16,
    a: i64,
    b: i64,
    code: i64,
) -> Body {
    vec![
        i(op::LDI16, vec![reg("rs1"), imm(a)]),
        i(op::LDI16, vec![reg("rs2"), imm(b)]),
        i(compare, vec![reg("p"), reg("rs1"), reg("rs2")]),
        i(op::SETCC8, vec![reg("rc8"), imm(code), reg("p")]),
    ]
}

/// 0x1234 <u 0x1300 (the high bytes differ: 0x12 < 0x13) is 1; 0x1300 <u 0x1234 is 0; 0x12FF <u
/// 0x1300 (the low bytes alone say otherwise, the borrow decides) is 1.
#[test]
fn a_word_compare_orders_by_the_high_byte_then_the_low() {
    let ult = |a, b| main_returning(word_compare(op::CMP16, a, b, 2), "rc8");
    assert_eq!(run("cmp16-hi-lt", &[ult(0x1234, 0x1300)]), 1);
    assert_eq!(run("cmp16-hi-ge", &[ult(0x1300, 0x1234)]), 0);
    assert_eq!(run("cmp16-borrow", &[ult(0x12FF, 0x1300)]), 1);
}

/// Equal words give Z; words that differ in only the high byte, or in only the low byte, do not.
#[test]
fn a_word_equality_needs_both_bytes() {
    let eq = |a, b| main_returning(word_compare(op::CMPZ16, a, b, 0), "rc8");
    assert_eq!(run("cmpz16-same", &[eq(0x1234, 0x1234)]), 1);
    assert_eq!(run("cmpz16-hi", &[eq(0x1234, 0x1334)]), 0);
    assert_eq!(run("cmpz16-lo", &[eq(0x1234, 0x1235)]), 0);
}

/// `-1 <s 1` on words as lowered: bit 15 flipped, 0xFFFF -> 0x7FFF and 1 -> 0x8001; 0x7FFF <u 0x8001.
#[test]
fn a_signed_word_compare_through_flipped_bit_15() {
    assert_eq!(run("cmp16-signed", &[main_returning(word_compare(op::CMP16, 0x7FFF, 0x8001, 2), "rc8")]), 1);
}

/// A word stored at `$4000 + 2` lands little-endian: the low byte 0xEF at +2, the high byte 0xBE at +3;
/// loaded back as a word it is the same.
#[test]
fn word_memory_is_little_endian_through_a_pointer_and_offset() {
    let stored = || -> Body {
        vec![
            i(op::LDI16, vec![reg("rs1"), imm(0x4000)]),
            i(op::LDI16, vec![reg("rs2"), imm(0xBEEF)]),
            i(op::STP16, vec![reg("rs2"), reg("rs1"), imm(2)]),
        ]
    };
    let byte_at = |offset| {
        let mut body = stored();
        body.push(i(op::LDP8, vec![reg("rc8"), reg("rs1"), imm(offset)]));
        main_returning(body, "rc8")
    };
    assert_eq!(run("mem-lo", &[byte_at(2)]), 0xEF);
    assert_eq!(run("mem-hi", &[byte_at(3)]), 0xBE);
    let mut body = stored();
    body.push(i(op::LDP16, vec![reg("rs3"), reg("rs1"), imm(2)]));
    assert_eq!(run("mem-word-hi", &[main_returning(body.clone(), "rc7")]), 0xBE);
    assert_eq!(run("mem-word-lo", &[main_returning(body, "rc6")]), 0xEF);
}

fn shifted(
    opcode: u16,
    value: i64,
    k: i64,
    wide: bool,
) -> Body {
    if wide {
        vec![
            i(op::LDI16, vec![reg("rs1"), imm(value)]),
            i(opcode, vec![reg("rs2"), reg("rs1"), imm(k)]),
        ]
    } else {
        vec![i(op::LDI8, vec![reg("rc2"), imm(value)]), i(opcode, vec![reg("rc3"), reg("rc2"), imm(k)])]
    }
}

/// 0x81 << 1 = 0x02 (the top bit falls off); 0x05 << 3 = 0x28; 0x81 >>u 1 = 0x40; 0x81 >>s 1 = 0xC0
/// (the sign is shifted in).
#[test]
fn byte_shifts_by_constants() {
    let r = |name: &str, opcode, value, k| run(name, &[main_returning(shifted(opcode, value, k, false), "rc3")]);
    assert_eq!(r("shl8-1", op::SHL8, 0x81, 1), 0x02);
    assert_eq!(r("shl8-3", op::SHL8, 0x05, 3), 0x28);
    assert_eq!(r("srl8", op::SRL8, 0x81, 1), 0x40);
    assert_eq!(r("sra8", op::SRA8, 0x81, 1), 0xC0);
}

/// 0x1234 << 4 = 0x2340 (the bits cross the byte boundary); 0x8002 >>u 1 = 0x4001; 0x8001 >>s 1 =
/// 0xC000.
#[test]
fn word_shifts_by_constants() {
    let hi_lo = |name: &str, opcode, value, k| {
        let hi = run(&format!("{name}-hi"), &[main_returning(shifted(opcode, value, k, true), "rc5")]);
        let lo = run(&format!("{name}-lo"), &[main_returning(shifted(opcode, value, k, true), "rc4")]);
        (hi, lo)
    };
    assert_eq!(hi_lo("shl16", op::SHL16, 0x1234, 4), (0x23, 0x40));
    assert_eq!(hi_lo("srl16", op::SRL16, 0x8002, 1), (0x40, 0x01));
    assert_eq!(hi_lo("sra16", op::SRA16, 0x8001, 1), (0xC0, 0x00));
}

use llrm_6502::frame::{Function, layout};
use llrm_codegen::backend::frameobjects::Object;

/// A function with a 10-byte local, a saved pair and a 4-byte call area (16 bytes), entered with
/// `rs0 = $4100`. It opens the frame (`rs0` = $4100 - 16), stores 0x5A in local byte 1 and reads it back,
/// clobbers `rs4`, and closes the frame. After the return `rs0` is $4100 again, `rs4` is what the
/// caller had, and the byte is in memory at $4100 - 16 + 4 + 1.
fn framed_call() -> (Body, String) {
    let frame = layout(&Function {
        objects: vec![Object { size: 10, align: 1, offset: 0, callee_saved: false, dead: false }],
        saved: vec!["rs4".into()],
        max_call_frame: 4,
        adjusts_stack: true,
        ..Function::default()
    });
    let mut f: Body = frame.prologue().into_iter().map(Ok).collect();
    let local = |opcode: u16, byte: &str| {
        frame
            .eliminate(&MInst::new(opcode, vec![reg(byte), Operand::FrameIndex(0), imm(1)]))
            .map(Ok)
            .expect("resolves")
    };
    f.push(i(op::LDI8, vec![reg("rc2"), imm(0x5A)]));
    f.push(local(op::STP8, "rc2"));
    f.push(local(op::LDP8, "rc3"));
    f.push(i(op::LDI16, vec![reg("rs2"), imm(0x1234)]));
    f.push(i(op::COPY16, vec![reg("rs4"), reg("rs2")]));
    f.extend(frame.epilogue().into_iter().map(Ok));
    f.push(i(op::COPY8, vec![reg("a"), reg("rc3")]));
    f.push(i(op::RET, vec![]));
    let callee = function("framed", &f).expect("expands");
    let main: Body = vec![
        i(op::LDI16, vec![reg("rs0"), imm(0x4100)]),
        i(op::LDI16, vec![reg("rs4"), imm(0xABCD)]),
        i(op::JSR, vec![Operand::Label("framed".into())]),
        i(op::COPY8, vec![reg("rc12"), reg("a")]),
        // the byte, read from where the frame put it
        i(op::LDI16, vec![reg("rs1"), imm(0x4100 - 16 + 4)]),
        i(op::LDP8, vec![reg("rc10"), reg("rs1"), imm(1)]),
    ];
    (main, callee)
}

fn after_framed_call(
    name: &str,
    byte: &str,
) -> i32 {
    let (main, callee) = framed_call();
    run(name, &[main_returning(main, byte), callee])
}

#[test]
fn a_frame_restores_rs0_and_the_saved_pair_and_keeps_its_locals() {
    // the function returned the byte it stored; rs0 is $4100 again; rs4 is $ABCD; memory holds 0x5A
    for (name, register, want) in [
        ("frame-ret", "rc12", 0x5A),
        ("frame-mem", "rc10", 0x5A),
        ("frame-rs0-hi", "rc1", 0x41),
        ("frame-rs0-lo", "rc0", 0x00),
        ("frame-rs4-hi", "rc9", 0xAB),
        ("frame-rs4-lo", "rc8", 0xCD),
    ] {
        assert_eq!(after_framed_call(name, register), want, "{name}");
    }
}
