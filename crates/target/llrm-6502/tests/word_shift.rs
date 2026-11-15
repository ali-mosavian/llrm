//! A 16-bit shift by a constant is one chain through both bytes.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// Bytes of `f` for `x <op> k`, from its label to its `rts`.
fn bytes(
    level: Level,
    op: &str,
    k: u32,
) -> (u32, String) {
    let name = "f";
    let ir = format!(
        "target datalayout = \"{}\"\ndefine i16 @{name}(i16 %x) {{\n  %r = {op} i16 %x, {k}\n  ret i16 %r\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("no `{name}:` in\n{text}"));
    let body: String = text[from..].split("  rts").next().unwrap().to_owned() + "  rts\n";
    (estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes, body)
}

/// The chain through A, by hand under the ABI (A:X in, A:X out):
/// shl `stx rc3; (asl a; rol rc3)*k; ldx rc3; rts`, shr
/// `sta rc2; txa; (lsr a; ror rc2)*k; tax; lda rc2; rts`, sar the same with
/// `cmp #$80; ror a; ror rc2`.
fn target(
    op: &str,
    k: u32,
) -> u32 {
    match op {
        "shl" => 3 * k + 5,
        "lshr" => 3 * k + 7,
        _ => 5 * k + 7,
    }
}

fn within_target(op: &str) {
    let mut over = Vec::new();
    for level in [Level::O2, Level::Os] {
        for k in 1..=4 {
            let (got, body) = bytes(level, op, k);
            let allowed = target(op, k);
            if got > allowed {
                over.push(format!("{level:?} {op} {k}: {got} B against {allowed}\n{body}"));
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}

/// `x << 1` was 10 bytes against 8: the low byte of the shift was computed
/// again on its own beside the chain that had it.
#[test]
fn a_word_shift_left_by_one_to_four_is_the_chain_through_a() {
    within_target("shl");
}

/// `x >> 1` was 15 bytes against 9 and `x >> 3` 24 against 15: the high byte
/// was shifted again on its own, and the chain went through memory.
#[test]
fn a_word_logical_shift_right_by_one_to_four_is_the_chain_through_a() {
    within_target("lshr");
}

/// An arithmetic shift right took 8 bytes a step against 5: the high byte was
/// loaded and stored back each step.
#[test]
fn a_word_arithmetic_shift_right_by_one_to_four_is_the_chain_through_a() {
    within_target("ashr");
}

/// Bytes of `x <op> k` for k = 5..=15 as they were before shared word shifts:
/// the chain is not wanted there (a byte shift or a bit trick is shorter), so
/// keeping the inner shift of `(x >> 7) >> 8` for the other half made `x >> 7`
/// seven `lsr; ror` pairs, 18 bytes against 12.
///
/// 6502-AX-WORD-OPS: shl 5..7 take +2 B, the A:X entry stored to the cells the
/// shift runs on.
const CEILING: [(&str, [u32; 11], [u32; 11]); 3] = [
    // O2, Os
    ("shl", [24, 21, 15, 5, 7, 8, 9, 10, 11, 12, 10], [21, 21, 15, 5, 7, 8, 9, 10, 11, 12, 10]),
    ("lshr", [28, 24, 18, 5, 6, 7, 8, 9, 10, 11, 9], [24, 24, 18, 5, 6, 7, 8, 9, 10, 11, 9]),
    ("ashr", [45, 53, 61, 15, 18, 21, 24, 27, 30, 33, 22], [45, 53, 61, 15, 18, 21, 24, 27, 30, 33, 22]),
];

#[test]
fn a_word_shift_by_five_to_fifteen_is_no_longer_than_it_was() {
    let mut over = Vec::new();
    for (op, o2, os) in CEILING {
        for (level, ceilings) in [(Level::O2, o2), (Level::Os, os)] {
            for (at, ceiling) in ceilings.into_iter().enumerate() {
                let k = 5 + at as u32;
                let (got, body) = bytes(level, op, k);
                if got > ceiling {
                    over.push(format!("{level:?} {op} {k}: {got} B against {ceiling}\n{body}"));
                }
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}

fn exit_code_of(
    text: &str,
    function: &str,
) -> i32 {
    let dir = std::env::temp_dir().join(format!("llrm6502-word-shift-{}-{function}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let main = format!(
        ".export _main\n_main:\n  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n  jsr {function}\n  ldx #0\n  rts\n"
    );
    std::fs::write(dir.join("t.s"), format!("{text}\n{main}")).unwrap();
    common::pipeline::tool(&dir, "cl65", &["-t", "sim6502", "-o", "t.bin", "t.s"])
        .unwrap_or_else(|e| panic!("{e}\n{text}"));
    let code = std::process::Command::new("sim65").arg(dir.join("t.bin")).status().unwrap().code().unwrap();
    std::fs::remove_dir_all(dir).ok();
    code
}

/// Every shift of a word by 1 to 15, left, logical right and arithmetic
/// right, gives what the shift gives in Rust: the exit code is the number of
/// the first one that does not (0: all agree).
#[test]
fn every_word_shift_gives_the_value_of_the_shift() {
    for level in [Level::O2, Level::Os] {
        for value in [0xA5C3u16, 0x1234, 0x8000, 0x00FF] {
            let mut ir = format!("target datalayout = \"{}\"\n@x = global i16 {}\n", datalayout(), value as i16);
            let mut cases = Vec::new();
            for op in ["shl", "lshr", "ashr"] {
                for k in 1..=15u32 {
                    let want = match op {
                        "shl" => value << k,
                        "lshr" => value >> k,
                        _ => ((value as i16) >> k) as u16,
                    };
                    ir += &format!(
                        "define i16 @f{}(i16 %x) noinline {{\n  %r = {op} i16 %x, {k}\n  ret i16 %r\n}}\n",
                        cases.len()
                    );
                    cases.push(want);
                }
            }
            ir += "define i8 @probe() {\n  %v = load volatile i16, ptr @x\n";
            let mut previous = "0".to_owned();
            for (n, want) in cases.iter().enumerate() {
                ir += &format!(
                    "  %r{n} = call i16 @f{n}(i16 %v)\n  %ok{n} = icmp eq i16 %r{n}, {}\n  %a{n} = select i1 %ok{n}, i8 {previous}, i8 {}\n",
                    *want as i16,
                    n + 1
                );
                previous = format!("%a{n}");
            }
            ir += &format!("  ret i8 {previous}\n}}\n");
            let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
            let failed = exit_code_of(&text, "probe");
            assert_eq!(failed, 0, "{level:?} x={value:#06x}: shift #{failed} (of shl, lshr, ashr by 1..15) is wrong");
        }
    }
}
