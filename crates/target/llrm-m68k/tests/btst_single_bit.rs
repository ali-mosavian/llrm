//! A test of one bit, `(x & 0x800000) != 0`, took a copy of the register, a
//! 6-byte `and.l #8388608` and a branch (10 B; gcc: `btst #23,%d2`, 4 B, no
//! copy, 10 clk against 18 on the 68000). The pin takes a constant mask to
//! BTST only when it does not fit a `TEST` (never at i32), so crc16's
//! `crc & 0x8000` (78 B against gcc's 76) and the soft-float normalizers kept
//! the `and`. BTST sets Z only: the rule is for conditions that read Z.

use llrm_m68k::dagisel::lowering::btst_beats_and_immediate;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::timing::cost;

mod support;

const CPUS: [&str; 6] = ["68000", "68010", "68020", "68030", "68040", "68060"];
const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

fn test_of(
    mask: i64,
    uses_the_and: bool,
) -> String {
    format!(
        r#"{DATALAYOUT}
declare void @a(i32)
declare void @b(i32)
define void @k(i32 %x) {{
  %m = and i32 %x, {mask}
  %t = icmp ne i32 %m, 0
  br i1 %t, label %yes, label %no
yes:
  call void @a(i32 {pass})
  ret void
no:
  call void @b(i32 %x)
  ret void
}}
"#,
        pass = if uses_the_and { "%m" } else { "1" }
    )
}

/// The premise, from the tables: at every CPU `btst #k,Dn` is no dearer than
/// `and.l #imm,Dn` in bytes (4 against 6) or clocks.
#[test]
fn btst_takes_no_more_bytes_or_clocks_than_and_immediate_on_every_cpu() {
    for name in CPUS {
        let cpu = cpu_of(name).unwrap();
        let btst = cost(cpu, "btst #23, %d0", 4, false).unwrap().clocks;
        let and = cost(cpu, "and.l #8388608, %d0", 6, false).unwrap().clocks;
        assert!(btst <= and, "{name}: btst {btst} clk against and.l {and} clk");
        assert!(btst_beats_and_immediate(cpu), "{name}");
    }
}

#[test]
fn a_one_bit_mask_against_zero_is_a_btst_not_a_copy_and_an_and() {
    for name in CPUS {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = assembly(&test_of(8388608, false), cpu_of(name).unwrap(), level).expect("compiles");
            assert!(out.contains("btst\t#23"), "{name} {level:?}: no btst #23:\n{out}");
            assert!(!out.contains("and.l\t#8388608"), "{name} {level:?}: the and stayed:\n{out}");
        }
    }
}

/// crc16's `crc & 0x8000` (bench/library/crc16.ll) is the sign of the low word:
/// `tst.w` (2 B, 4 clk on the 68000) where `btst #15` is 4 B, 10 clk and the
/// first form was `and.l #32768` (6 B, 16 clk). At O1 that was 4972 clk against
/// gcc's 4200 on the 68000. With the loop's `crc` carried as a word
/// (`narrowcycle`) the carry of `add.w %d0, %d0` is that bit and the `tst.w`
/// goes (`narrow_cycle.rs`): the bit is read by `tst.w` or by the carry, never
/// by `btst` or an `and`.
#[test]
fn crc16_tests_its_top_bit_with_tst_w_or_the_carry() {
    let ir = include_str!("../bench/library/crc16.ll");
    for name in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = assembly(ir, cpu_of(name).unwrap(), level).expect("compiles");
            assert!(out.contains("tst.w") || (out.contains("add.w\t") && out.contains("bcc")), "{name} {level:?}:\n{out}");
            assert!(!out.contains("btst\t#15") && !out.contains("and.l\t#32768"), "{name} {level:?}:\n{out}");
        }
    }
}

/// The sign bit of the low word or byte, `(x & 0x8000) != 0`, is
/// `tst.w`/`tst.b` and a sign branch on every CPU and level: no `btst`, no
/// `and`, no copy.
#[test]
fn the_top_bit_of_a_narrow_width_is_a_tst_of_that_width() {
    for (mask, tst) in [(0x8000, "tst.w"), (0x80, "tst.b")] {
        for name in CPUS {
            for level in [Level::O1, Level::O2, Level::Os] {
                let out = assembly(&test_of(mask, false), cpu_of(name).unwrap(), level).expect("compiles");
                assert!(out.contains(tst), "{name} {level:?} {mask:#x}: no {tst}:\n{out}");
                assert!(!out.contains("btst") && !out.contains("and.l"), "{name} {level:?} {mask:#x}:\n{out}");
            }
        }
    }
}

/// The premise, from the tables: at every CPU `tst.w`/`tst.b` Dn is no dearer
/// than `btst #k,Dn` in bytes (2 against 4) or clocks.
#[test]
fn tst_takes_no_more_bytes_or_clocks_than_btst_on_every_cpu() {
    for name in CPUS {
        let cpu = cpu_of(name).unwrap();
        let btst = cost(cpu, "btst #15, %d0", 4, false).unwrap().clocks;
        for tst in ["tst.w %d0", "tst.b %d0"] {
            let clocks = cost(cpu, tst, 2, false).unwrap().clocks;
            assert!(clocks <= btst, "{name}: {tst} {clocks} clk against btst {btst} clk");
        }
    }
}

/// An `and` whose value is read again is not a flags-only test: it stays an
/// `and`, which sets N and Z for its other user and is the value.
#[test]
fn an_and_whose_value_is_used_again_stays_an_and() {
    let out = assembly(&test_of(8388608, true), cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("btst"), "{out}");
}

/// Bit 31 is the sign: `tst.l`, 2 B, not a `btst`.
#[test]
fn the_sign_bit_stays_a_sign_test() {
    let out = assembly(&test_of(-2147483648, false), cpu_of("68020").unwrap(), Level::Os).expect("compiles");
    assert!(!out.contains("btst"), "{out}");
}

/// `((x >> 5) & 1) != 0` printed `move.w #5,%a1; btst %a1,%d0` (the pin's
/// `BTST32di` emits no `EmitConvertToTarget`; its own `llc` stops printing it):
/// 2 B and a register more than `btst #5,%d0`, and `btst` with an address
/// register is not an instruction.
#[test]
fn a_constant_bit_number_is_an_immediate_not_a_register() {
    let ir = format!(
        r#"{DATALAYOUT}
declare void @g()
define void @k(i32 %x) {{
  %s = lshr i32 %x, 5
  %a = and i32 %s, 1
  %c = icmp ne i32 %a, 0
  br i1 %c, label %t, label %e
t:
  call void @g()
  ret void
e:
  ret void
}}
"#
    );
    for name in CPUS {
        let out = assembly(&ir, cpu_of(name).unwrap(), Level::O1).expect("compiles");
        assert!(out.contains("btst\t#5, %d0"), "{name}:\n{out}");
    }
}

/// The `btst` form runs: with the bit set the program exits 7, with it clear 9
/// (the 8-bit exit status of `qemu-m68k`), at the edges of the mask.
#[test]
fn a_btst_tests_the_bit_it_names() {
    use llrm_m68k::compile::{Options, items_with, text_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};
    let ir = format!(
        r#"{DATALAYOUT}
define i32 @f(i32 %x) {{
  %m = and i32 %x, 8388608
  %c = icmp eq i32 %m, 0
  %r = select i1 %c, i32 9, i32 7
  ret i32 %r
}}
"#
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let asm = text_with(&module, "f", Cpu::M68000, Options::default()).unwrap();
    assert!(asm.contains("btst\t#23"), "{asm}");
    let Some(qemu) = support::qemu() else { return };
    for (x, want) in [(0x800000, 7), (0x7FFFFF, 9), (0, 9), (0xFFFF_FFFFu32 as i64, 7), (0x7F7FFFFF, 9), (0x1000000, 9)]
    {
        let mut all = start("f", &[x]);
        all.extend(items_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles"));
        let path = support::unique_path("btst");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(want), "x = {x:#x}\n{asm}");
    }
}

/// `tst.w` reads only the low word: with bit 15 set and the bits above set or
/// clear the test is true, with bit 15 clear (bit 16 or 31 set) it is false.
#[test]
fn a_tst_w_tests_the_bit_15_and_nothing_above_it() {
    use llrm_m68k::compile::{Options, items_with, text_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};
    let ir = format!(
        r#"{DATALAYOUT}
define i32 @f(i32 %x) {{
  %m = and i32 %x, 32768
  %c = icmp eq i32 %m, 0
  %r = select i1 %c, i32 9, i32 7
  ret i32 %r
}}
"#
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let asm = text_with(&module, "f", Cpu::M68000, Options::default()).unwrap();
    assert!(asm.contains("tst.w"), "{asm}");
    let Some(qemu) = support::qemu() else { return };
    for (x, want) in [
        (0x8000, 7),
        (0x7FFF, 9),
        (0, 9),
        (0xFFFF_FFFFu32 as i64, 7),
        (0x10000, 9),
        (0x8000_8000u32 as i64, 7),
        (0x7FFF_7FFF, 9),
    ] {
        let mut all = start("f", &[x]);
        all.extend(items_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles"));
        let path = support::unique_path("tstw");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(want), "x = {x:#x}\n{asm}");
    }
}

/// crc16 with its `tst.w` still returns the result worked out by hand
/// (`library_table`: 152 over 9 bytes of the data block).
#[test]
fn crc16_with_tst_w_returns_the_hand_worked_result() {
    use llrm_m68k::compile::{Options, items_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::kernels::{Arg, program_with_data};
    use llrm_m68k::object::{executable, write_executable};
    let Some(qemu) = support::qemu() else { return };
    let ir = include_str!("../bench/library/crc16.ll");
    let module = llrm_mir::parse::module(ir).expect("parses");
    let code = items_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    let program = program_with_data(&[Arg::Data(224), Arg::Imm(9)], code, &llrm_m68k::kernels::library_data());
    let path = support::unique_path("btst-crc16");
    write_executable(&path, &executable(&program).unwrap()).unwrap();
    let status = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(status, Some(152));
}
