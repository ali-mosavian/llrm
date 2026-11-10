//! crc16's bit loop tested the sign of a value and then doubled it:
//! `move.l %d0,%d1; add.l %d0,%d0; tst.l %d1; bge`. The `add` leaves the bit it
//! shifted out in C, which is that sign, so the copy and the `tst` are not
//! needed: `add.l %d0,%d0; bcc`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn asm(
    body: &str,
    cpu: Cpu,
    reuse: bool,
) -> String {
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{body}")).expect("parses");
    text_with(&module, "k", cpu, Options { reuse_shift_out_flags: reuse, ..Options::default() }).expect("compiles")
}

/// The sign of `x` picks a constant, then `x` is doubled and returned with it.
const SIGN_THEN_DOUBLE: &str = "define i32 @k(i32 %x) {
entry:
  %neg = icmp slt i32 %x, 0
  %s = shl i32 %x, 1
  br i1 %neg, label %a, label %b
a:
  %r = xor i32 %s, 4129
  ret i32 %r
b:
  ret i32 %s
}";

/// The codes of `add` are not the codes of `tst` for these: a reader of Z
/// (`ble`, `bne`) or a `tst` of another register or at another size keeps the
/// `tst`.
const SIGN_OF_ANOTHER: &str = "define i32 @k(i32 %x, i32 %y) {
entry:
  %neg = icmp slt i32 %y, 0
  %s = shl i32 %x, 1
  br i1 %neg, label %a, label %b
a:
  %r = xor i32 %s, 4129
  ret i32 %r
b:
  ret i32 %s
}";

const ZERO_READER: &str = "define i32 @k(i32 %x) {
entry:
  %pos = icmp sgt i32 %x, 0
  %s = shl i32 %x, 1
  br i1 %pos, label %a, label %b
a:
  %r = xor i32 %s, 4129
  ret i32 %r
b:
  ret i32 %s
}";

const WORD_SIGN_OF_LONG_DOUBLE: &str = "define i32 @k(i32 %x) {
entry:
  %w = trunc i32 %x to i16
  %neg = icmp slt i16 %w, 0
  %s = shl i32 %x, 1
  br i1 %neg, label %a, label %b
a:
  %r = xor i32 %s, 4129
  ret i32 %r
b:
  ret i32 %s
}";

#[test]
fn the_carry_of_the_doubling_stands_in_for_the_sign_test() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let base = asm(SIGN_THEN_DOUBLE, cpu, false);
        assert!(base.contains("tst.l"), "premise: the sign test is a tst without the pass:\n{base}");
        let out = asm(SIGN_THEN_DOUBLE, cpu, true);
        assert!(!out.contains("tst.l") && out.contains("add.l") && out.contains("bcc"), "{cpu:?}:\n{out}");
        assert_eq!(out.matches("move.l").count(), 1, "the copy of the tested value goes with the tst:\n{out}");
    }
}

#[test]
fn a_test_of_another_register_stays() {
    let out = asm(SIGN_OF_ANOTHER, Cpu::M68000, true);
    assert!(out.contains("tst.l"), "{out}");
}

#[test]
fn a_reader_of_the_zero_flag_keeps_the_test() {
    let out = asm(ZERO_READER, Cpu::M68000, true);
    assert!(out.contains("tst.l") && !out.contains("bcc"), "{out}");
}

#[test]
fn a_word_test_of_a_long_doubling_stays() {
    let out = asm(WORD_SIGN_OF_LONG_DOUBLE, Cpu::M68000, true);
    assert!(out.contains("tst.w") && !out.contains("bcc") && !out.contains("bcs"), "{out}");
}

/// The edge values of the sign, executed with the pass on: the carry must say
/// what the sign said.
#[test]
fn the_carry_gives_the_same_answers_as_the_sign_on_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let values: [i32; 9] = [0, 1, -1, 0x7fff_ffff, i32::MIN, 0x4000_0000, 0x1234_5678, -0x6543_2110, 0x8000];
    let mut text = format!("{LAYOUT}{SIGN_THEN_DOUBLE}\ndefine i32 @f() {{\n  %a0 = add i32 0, 0\n");
    for (n, x) in values.iter().enumerate() {
        let doubled = (*x as u32) << 1;
        let expected = if *x < 0 { doubled ^ 4129 } else { doubled } as i32;
        text.push_str(&format!(
            "  %r{n} = call i32 @k(i32 {x})\n  %w{n} = icmp ne i32 %r{n}, {expected}\n  %z{n} = zext i1 %w{n} to i32\n  \
             %m{n} = mul i32 %z{n}, {}\n  %a{} = add i32 %a{n}, %m{n}\n",
            n + 1,
            n + 1
        ));
    }
    text.push_str(&format!(
        "  %ok = icmp eq i32 %a{}, 0\n  %s = select i1 %ok, i32 42, i32 1\n  ret i32 %s\n}}\n",
        values.len()
    ));
    let module = llrm_mir::parse::module(&text).expect("parses");
    for (cpu, name) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020"), (Cpu::M68060, "m68060")] {
        let options = Options { mid_end: false, reuse_shift_out_flags: true, ..Options::default() };
        let mut all = start("f", &[]);
        for function in ["f", "k"] {
            all.extend(items_with(&module, function, cpu, options).unwrap_or_else(|why| panic!("{cpu:?}: {why}")));
        }
        let path = support::unique_path("cmp_carry");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", name], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(42), "{cpu:?}");
    }
}
