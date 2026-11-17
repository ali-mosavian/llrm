//! An `i32` `or`, `and` and `xor` with a single bit was `or.l #imm,Dn` and
//! its siblings: 16 clocks in 6 bytes on the 68000, over the 12-14 clocks in 4
//! bytes of `bset`, `bclr` and `bchg` that gcc uses (`fix_si`, 2,084 clocks per
//! 3,000 vectors). Executed against C over every bit and the edge values.

use std::path::Path;
use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

mod support;

fn ir() -> String {
    let mut text = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n",
    );
    for (name, op, mask) in [("set", "or", 0), ("clr", "and", -1), ("chg", "xor", 0)] {
        for k in 0..32u32 {
            let bit = (1u32 << k) as i32;
            let constant = if mask == -1 { !bit } else { bit };
            text += &format!("define i32 @{name}{k}(i32 %x) {{\n  %r = {op} i32 %x, {constant}\n  ret i32 %r\n}}\n");
        }
    }
    text
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    text.split(&format!("{name}:")).nth(1).and_then(|rest| rest.split("rts").next()).expect("a function")
}

#[test]
fn a_single_bit_or_and_xor_is_one_bit_instruction_on_the_68000() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(&ir(), cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        for (name, mnemonic) in [("set20", "bset"), ("clr20", "bclr"), ("chg20", "bchg")] {
            let code = body(&text, name);
            assert!(code.contains(&format!("{mnemonic}\t#20")), "{level:?} {name}\n{code}");
        }
    }
}

/// `bset #0` took hanoi (+2 bytes, +52 clocks) and switch_table_in_code (+2
/// bytes) over the `addq #1` the pin selects for an `or` of a clear bit.
#[test]
fn a_disjoint_small_or_stays_an_addq() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @disjoint(i32 %x) {
  %s = shl i32 %x, 2
  %r = or i32 %s, 1
  ret i32 %r
}
define i32 @plain(i32 %x) {
  %r = or i32 %x, 1
  ret i32 %r
}";
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(ir, cpu_of("68000").unwrap(), level).unwrap();
        assert!(body(&text, "disjoint").contains("addq.l\t#1"), "{level:?}\n{}", body(&text, "disjoint"));
        assert!(body(&text, "plain").contains("bset\t#0"), "{level:?}\n{}", body(&text, "plain"));
    }
}

#[test]
fn the_68020_keeps_the_immediate_logic_instruction() {
    let text = assembly(&ir(), cpu_of("68020").unwrap(), Level::O2).unwrap();
    assert!(!body(&text, "set20").contains("bset"), "{}", body(&text, "set20"));
}

/// GNU as: `bset #20,%d1` is `08c1 0014`, `bclr #3,%d0` is `0880 0003`,
/// `bchg #31,%d7` is `0847 001f`.
#[test]
fn the_bit_rows_encode_as_gnu_as_does() {
    let inst = |opcode, dst, bit| Inst {
        opcode,
        operands: vec![Operand::Reg(dst), Operand::Reg(dst), Operand::Imm(bit)],
    };
    assert_eq!(encode(&inst("BSET32ti", reg::D1, 20)).unwrap().bytes, [0x08, 0xc1, 0x00, 0x14]);
    assert_eq!(encode(&inst("BCLR32ti", reg::D0, 3)).unwrap().bytes, [0x08, 0x80, 0x00, 0x03]);
    assert_eq!(encode(&inst("BCHG32ti", reg::D7, 31)).unwrap().bytes, [0x08, 0x47, 0x00, 0x1f]);
}

#[test]
fn every_bit_and_edge_value_computes_what_c_computes_when_run() {
    let (Some(root), Some(qemu)) = (support::oracle_root(), support::qemu()) else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let mut text = assembly(&ir(), cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        for name in ["set", "clr", "chg"] {
            for k in 0..32 {
                text = text.replace(&format!("{name}{k}:"), &format!(".globl {name}{k}\n{name}{k}:"));
            }
        }
        let source = support::unique_path("bitop-s");
        let object = support::unique_path("bitop-o");
        let exe = support::unique_path("bitop-exe");
        std::fs::write(&source, format!(".text\n{text}")).unwrap();
        assert!(tool("as").arg("-m68000").arg("-o").arg(&object).arg(&source).status().unwrap().success());
        let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/bitop_main.c");
        let mut link = tool("gcc");
        link.args(["-msoft-float", "-m68000", "-O1", "-nostdlib", "-static", "-ffreestanding"]);
        link.arg(main).arg(&object).arg("-o").arg(&exe);
        let linked = link.output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let code = support::run(&qemu, &["-cpu", "m68000"], &exe);
        for path in [source, object, exe] {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(0), "{level:?}: wrong result (1+k set, 33+k clr, 65+k chg)");
    }
}
