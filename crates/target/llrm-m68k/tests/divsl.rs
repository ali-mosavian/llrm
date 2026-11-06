//! `a / b + a % b` divided twice on the 68020 (`divs.l`, then `muls.l` and
//! `sub` of the quotient to make the remainder): muldiv 42 B and 8 clk over
//! gcc's 28 B, which divides once with `divsl.l`. The pair is one
//! `divsl.l`/`divul.l`; the encoding against binutils, the selection, and
//! the values under qemu.

use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn pair(signed: bool) -> String {
    let (div, rem) = if signed { ("sdiv", "srem") } else { ("udiv", "urem") };
    format!(
        "define i32 @f(i32 %n) {{
  %m = and i32 %n, 7
  %d = or i32 %m, 1
  %q = {div} i32 %n, %d
  %r = {rem} i32 %n, %d
  %s = add i32 %q, %r
  ret i32 %s
}}
"
    )
}

fn text(
    ir: &str,
    cpu: &str,
) -> String {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    assembly(&ir, cpu_of(cpu).unwrap(), Level::O2).expect("compiles")
}

#[test]
fn a_quotient_and_its_remainder_are_one_divide_on_the_68020() {
    for (signed, one, other) in [(true, "divsl.l", "divs.l"), (false, "divul.l", "divu.l")] {
        let out = text(&pair(signed), "68020");
        assert!(out.contains(one) && !out.contains(&format!("{other}\t")), "{out}");
        assert!(!out.contains("muls.l") && !out.contains("mulu.l"), "the multiply back is gone:\n{out}");
    }
}

#[test]
fn a_68000_keeps_its_library_calls() {
    let out = text(&pair(true), "68000");
    assert!(!out.contains("divsl") && !out.contains("divul"), "{out}");
}

#[test]
fn a_remainder_alone_stays_the_pins_library_call() {
    let out = text(
        "define i32 @f(i32 %n) {\n  %m = and i32 %n, 7\n  %d = or i32 %m, 1\n  %r = srem i32 %n, %d\n  ret i32 %r\n}\n",
        "68020",
    );
    assert!(out.contains("__modsi3") && !out.contains("divsl"), "{out}");
}

fn binutils(tool: &str) -> Option<Command> {
    let root = support::oracle_root()?;
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn assembled(text: &str) -> Option<Vec<u8>> {
    let object = support::unique_path("divsl-o");
    let source = support::unique_path("divsl-s");
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let status = binutils("as")?.args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap();
    assert!(status.success(), "{text}");
    let flat = support::unique_path("divsl-bin");
    let status = binutils("objcopy")?.args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

/// A wrong remainder field would store the remainder in the wrong register.
#[test]
fn the_pair_encodes_as_binutils_does() {
    for name in ["SDIVREM32", "UDIVREM32"] {
        for (quotient, remainder, divisor) in [(reg::D0, reg::D1, reg::D2), (reg::D5, reg::D3, reg::D3)] {
            let inst = Inst {
                opcode: name,
                operands: vec![
                    Operand::Reg(quotient),
                    Operand::Reg(remainder),
                    Operand::Reg(quotient),
                    Operand::Reg(divisor),
                ],
            };
            let text = print(&inst).unwrap();
            let Some(want) = assembled(&text) else { return };
            assert_eq!(encode(&inst).unwrap().bytes, want, "{name}: {text}");
        }
    }
}

/// The pair at 68020, linked with a C `main` that compares it with C's own
/// division and remainder, run under qemu.
#[test]
fn the_quotient_and_remainder_are_the_divides_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    for (signed, kind) in [(true, "int"), (false, "unsigned")] {
        let out = text(&pair(signed), "68020");
        let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
        let main = format!(
            "extern {kind} f({kind});
static long sys3(long n,long a,long b,long c){{register long d0 __asm__(\"d0\")=n,d1 __asm__(\"d1\")=a,d2 __asm__(\"d2\")=b,d3 __asm__(\"d3\")=c;__asm__ volatile(\"trap #0\":\"+d\"(d0):\"d\"(d1),\"d\"(d2),\"d\"(d3):\"memory\");return d0;}}
int main(void){{
  static const {kind} values[] = {{ 0, 1, 7, 99, 100, 1000, 12345, -1, -7, -100, -12345, 2147483647, -2147483647 }};
  for (unsigned i = 0; i < sizeof values / sizeof values[0]; i++) {{
    {kind} n = values[i], d = (n & 7) | 1;
    if (f(n) != (n / d) + (n % d)) return 1 + i;
  }}
  return 0;
}}
void _start(void){{ int r = main(); sys3(1, r, 0, 0); for(;;); }}
"
        );
        let source = support::unique_path("divsl-s");
        let object = support::unique_path("divsl-o");
        let c = support::unique_path("divsl-c");
        let exe = support::unique_path("divsl-exe");
        std::fs::write(&source, format!("{out}\n")).unwrap();
        std::fs::write(&c, main).unwrap();
        assert!(assemble.arg("-m68020").arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{out}");
        let linked = link
            .args(["-msoft-float", "-m68020", "-O1", "-nostdlib", "-static", "-ffreestanding", "-x", "c"])
            .arg(&c)
            .args(["-x", "none"])
            .arg(&object)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(linked.success(), "{out}");
        let code = support::run(&qemu, &["-cpu", "m68020"], &exe);
        for path in [source, object, c, exe] {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(0), "signed={signed}: value number {code:?} differs:\n{out}");
    }
}
