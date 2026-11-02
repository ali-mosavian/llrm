//! A crc loop ended `eori.l #4129, %d0`, 6 bytes where `eori.w` is 4 and the
//! upper half is the same. The word form where the upper half of the
//! immediate changes nothing and no branch reads the codes; the values under
//! qemu, since a word row that touched the upper half or the codes would
//! still assemble.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// `x op imm` stored whole, then one byte of it read back: the volatile pair
/// keeps the mid-end from seeing that an immediate with a zero upper half
/// cannot change the bits read at offset 1.
fn byte_of(
    op: &str,
    imm: i64,
    offset: i32,
) -> String {
    format!(
        "define i32 @f(i32 %x) {{
  %y = {op} i32 %x, {imm}
  %slot = alloca [4 x i8], align 4
  store volatile i32 %y, ptr %slot, align 4
  %p = getelementptr i8, ptr %slot, i32 {offset}
  %b = load volatile i8, ptr %p, align 1
  %r = zext i8 %b to i32
  ret i32 %r
}}"
    )
}

#[test]
fn an_immediate_with_an_unchanged_upper_half_is_a_word_operation() {
    for (op, imm, word) in
        [("xor", 4129, "eori.w\t#4129"), ("or", 4129, "ori.w\t#4129"), ("and", -65281, "andi.w\t#255")]
    {
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let out = text_with(&module(&byte_of(op, imm, 1)), "f", cpu, Options::default()).expect("compiles");
            assert!(out.contains(word) || out.contains(&word.replace('i', "")), "{cpu:?} {op}: no word form:\n{out}");
            assert!(!out.contains(&format!(".l\t#{imm}")), "{cpu:?} {op}: a long immediate is left:\n{out}");
        }
    }
}

/// The upper half of `x` through the word form is `x`'s, and the lower half
/// is the operation's.
#[test]
fn the_word_form_keeps_the_upper_half_and_does_the_operation_below() {
    let Some(qemu) = support::qemu() else { return };
    let x: i64 = 0x00AB_C3D5;
    for (op, imm, apply) in [
        ("xor", 4129_i64, (|x: i64| x ^ 4129) as fn(i64) -> i64),
        ("or", 4129, |x| x | 4129),
        ("and", -65281, |x| x & 0xFFFF_00FF),
    ] {
        for offset in [1, 3] {
            let mut all = start("f", &[x]);
            all.extend(items_with(&module(&byte_of(op, imm, offset)), "f", Cpu::M68000, Options::default()).unwrap());
            let path = support::unique_path("narrow");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", "m68000"], &path);
            std::fs::remove_file(&path).ok();
            let want = (apply(x) >> (8 * (3 - offset))) & 255;
            assert_eq!(code, Some(want as i32), "{op} byte {offset}");
        }
    }
}

/// `x ^ 32768 < 0`: the long result of 0 ^ 0x8000 is positive, and the word
/// form sets N. The branch reads the codes the long form set.
const SIGN: &str = "
define i32 @f(i32 %x) {
  %y = xor i32 %x, 32768
  %c = icmp slt i32 %y, 0
  %r = zext i1 %c to i32
  ret i32 %r
}";

#[test]
fn a_reader_of_the_codes_keeps_the_long_form() {
    let out = text_with(&module(SIGN), "f", Cpu::M68000, Options::default()).expect("compiles");
    assert!(!out.contains("eori.w"), "the codes of the word form are read:\n{out}");
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[0]);
    all.extend(items_with(&module(SIGN), "f", Cpu::M68000, Options::default()).unwrap());
    let path = support::unique_path("narrow-sign");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
}
