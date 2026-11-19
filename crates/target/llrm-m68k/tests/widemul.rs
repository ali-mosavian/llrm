//! `(long long)a * b` of two `i32` took about 30 instructions on the 68020
//! (the 64-bit product split into 16-bit pieces) where gcc emits one
//! `mulu.l`/`muls.l` with a register pair. The pin has no such row and calls
//! `__muldi3`. The selection, the encoding against binutils, and the values
//! under qemu.

use std::process::Command;

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn product(extend: &str) -> String {
    format!(
        "define i64 @f(i32 %a, i32 %b) {{
  %x = {extend} i32 %a to i64
  %y = {extend} i32 %b to i64
  %m = mul i64 %x, %y
  ret i64 %m
}}
"
    )
}

fn text(
    ir: &str,
    cpu: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    assembly(&ir, cpu_of(cpu).unwrap(), level).expect("compiles")
}

#[test]
fn a_widening_multiply_is_one_instruction_from_the_68020_to_the_68040() {
    for cpu in ["68020", "68030", "68040"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            for (extend, one) in [("zext", "mulu.l"), ("sext", "muls.l")] {
                let out = text(&product(extend), cpu, level);
                let lines = out.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
                assert!(
                    out.lines().any(|line| line.contains(one) && line.contains(':')),
                    "{cpu} {level:?} {extend}: no register-pair {one}:\n{out}"
                );
                assert!(!out.contains("jsr") && lines <= 6, "{cpu} {level:?} {extend}: {lines} instructions:\n{out}");
            }
        }
    }
}

/// The 68000 has no 32-bit multiply and the 68060 does not implement the
/// 64-bit result (it traps to software), so both keep the expansion.
#[test]
fn the_68000_and_the_68060_keep_the_expansion() {
    for cpu in ["68000", "68010", "68060"] {
        let out = text(&product("zext"), cpu, Level::O2);
        assert!(!out.lines().any(|line| line.contains("mulu.l") && line.contains(':')), "{cpu}:\n{out}");
    }
}

fn binutils(tool: &str) -> Option<Command> {
    let root = support::oracle_root()?;
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn assembled(text: &str) -> Option<Vec<u8>> {
    let object = support::unique_path("widemul-o");
    let source = support::unique_path("widemul-s");
    std::fs::write(&source, format!("{text}\n")).unwrap();
    let status = binutils("as")?.args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap();
    assert!(status.success(), "{text}");
    let flat = support::unique_path("widemul-bin");
    let status = binutils("objcopy")?.args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
    assert!(status.success());
    let bytes = std::fs::read(&flat).unwrap();
    for path in [object, source, flat] {
        std::fs::remove_file(path).ok();
    }
    Some(bytes)
}

/// A swapped high and low field would return the halves the wrong way round.
#[test]
fn the_pair_encodes_as_binutils_does() {
    for name in ["UMUL64", "SMUL64"] {
        for (low, high, source) in
            [(reg::D0, reg::D1, reg::D2), (reg::D5, reg::D3, reg::D3), (reg::D7, reg::D0, reg::D4)]
        {
            let inst = Inst {
                opcode: name,
                operands: vec![Operand::Reg(low), Operand::Reg(high), Operand::Reg(low), Operand::Reg(source)],
            };
            let text = print(&inst).unwrap();
            let Some(want) = assembled(&text) else { return };
            assert_eq!(encode(&inst).unwrap().bytes, want, "{name}: {text}");
        }
    }
}

/// Both products at 68020, linked with a C `main` that compares them with C's
/// own, run under qemu.
#[test]
fn the_product_is_c_s_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    for (extend, kind, wide) in [("zext", "unsigned", "unsigned long long"), ("sext", "int", "long long")] {
        let out = text(&product(extend), "68020", Level::O2);
        let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
        let main = format!(
            "extern {wide} f({kind}, {kind});
static long sys3(long n,long a,long b,long c){{register long d0 __asm__(\"d0\")=n,d1 __asm__(\"d1\")=a,d2 __asm__(\"d2\")=b,d3 __asm__(\"d3\")=c;__asm__ volatile(\"trap #0\":\"+d\"(d0):\"d\"(d1),\"d\"(d2),\"d\"(d3):\"memory\");return d0;}}
int main(void){{
  static const {kind} values[] = {{ 0, 1, 2, 7, 99, 65535, 65536, 12345678, -1, -2, -7, -65536, 2147483647, -2147483647, 2147483648u }};
  unsigned n = sizeof values / sizeof values[0];
  for (unsigned i = 0; i < n; i++)
    for (unsigned j = 0; j < n; j++) {{
      {wide} want = ({wide}) values[i] * ({wide}) values[j];
      if (f(values[i], values[j]) != want) return 1 + i * n + j;
    }}
  return 0;
}}
void _start(void){{ int r = main(); sys3(1, r > 255 ? 255 : r, 0, 0); for(;;); }}
"
        );
        let source = support::unique_path("widemul-s");
        let object = support::unique_path("widemul-o");
        let c = support::unique_path("widemul-c");
        let exe = support::unique_path("widemul-exe");
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
        assert_eq!(code, Some(0), "{extend}: pair number {code:?} differs:\n{out}");
    }
}

/// The product sets the codes, so a compare whose result is read after it is
/// not left to the codes: `a < b` is decided, then the product is made, then
/// the branch on `a < b` is taken. The row defines the codes like the pin's.
const COMPARE_ACROSS: &str = "define i64 @f(i32 %a, i32 %b, i32 %c, i32 %d) {
  %lt = icmp ult i32 %a, %b
  %x = zext i32 %c to i64
  %y = zext i32 %d to i64
  %m = mul i64 %x, %y
  %k = select i1 %lt, i64 1, i64 2
  %r = add i64 %m, %k
  ret i64 %r
}
";

#[test]
fn a_compare_read_after_the_product_survives_it_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let out = text(COMPARE_ACROSS, "68020", Level::O2);
    let (Some(mut assemble), Some(mut link)) = (binutils("as"), binutils("gcc")) else { return };
    let main = "extern unsigned long long f(unsigned, unsigned, unsigned, unsigned);
static long sys3(long n,long a,long b,long c){register long d0 __asm__(\"d0\")=n,d1 __asm__(\"d1\")=a,d2 __asm__(\"d2\")=b,d3 __asm__(\"d3\")=c;__asm__ volatile(\"trap #0\":\"+d\"(d0):\"d\"(d1),\"d\"(d2),\"d\"(d3):\"memory\");return d0;}
int main(void){
  static const unsigned v[] = { 0, 1, 2, 65536, 0x7fffffff, 0x80000000u, 0xffffffffu };
  unsigned n = sizeof v / sizeof v[0];
  for (unsigned i = 0; i < n; i++) for (unsigned j = 0; j < n; j++) for (unsigned k = 0; k < n; k++) for (unsigned l = 0; l < n; l++) {
      unsigned long long want = (unsigned long long) v[k] * v[l] + (v[i] < v[j] ? 1 : 2);
      if (f(v[i], v[j], v[k], v[l]) != want) return 1;
  }
  return 0;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
";
    let source = support::unique_path("widemul-s");
    let object = support::unique_path("widemul-o");
    let c = support::unique_path("widemul-c");
    let exe = support::unique_path("widemul-exe");
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
    assert_eq!(code, Some(0), "a compare read after the product differs:\n{out}");
}

/// The 64-bit product sets Z from all 64 bits: a `tst` of the low half is kept.
#[test]
fn a_test_of_the_low_half_is_not_dropped_for_the_products_codes() {
    let out = text(
        "declare void @g()
define i32 @f(i32 %a, i32 %b) {
  %x = zext i32 %a to i64
  %y = zext i32 %b to i64
  %m = mul i64 %x, %y
  %lo = trunc i64 %m to i32
  %h = lshr i64 %m, 32
  %hi = trunc i64 %h to i32
  %c = icmp ne i32 %lo, 0
  br i1 %c, label %t, label %e
t:
  call void @g()
  ret i32 %hi
e:
  ret i32 %hi
}
",
        "68020",
        Level::O2,
    );
    assert!(out.contains("mulu.l") && out.contains("tst.l"), "{out}");
}
