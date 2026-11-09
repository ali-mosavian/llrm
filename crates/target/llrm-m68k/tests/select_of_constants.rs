//! `(x < y) ? 5 : 4` is `5 + sext(!c)`: an inverted `Scc` and an add. It was
//! `zext(c) + 4` (the pin's `foldSelectOfConstants` arm), which on a target
//! whose true is all ones costs a mask: `slt; and.b #1; addq.b #4`, 2 B and
//! one instruction more than gcc's `sge; addq.b #5`; run/select_of_constants
//! at 68020 -Os was 20312 B against gcc's 16990.

use llrm_m68k::driver::{Level, assembly, cpu_of};

mod support;

const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

fn select_of(
    pred: &str,
    then: i32,
    other: i32,
) -> String {
    format!(
        r#"{DATALAYOUT}
define i32 @f(i32 %x, i32 %y) {{
  %c = icmp {pred} i32 %x, %y
  %r = select i1 %c, i32 {then}, i32 {other}
  ret i32 %r
}}
"#
    )
}

#[test]
fn a_select_of_constants_one_apart_needs_no_mask() {
    for name in ["68000", "68020", "68040"] {
        for level in [Level::O1, Level::Os, Level::O2] {
            let out = assembly(&select_of("slt", 5, 4), cpu_of(name).unwrap(), level).expect("compiles");
            assert!(!out.contains("and."), "{name} {level:?}: the boolean was masked:\n{out}");
            assert!(out.contains("sge") || out.contains("slt"), "{name} {level:?}:\n{out}");
        }
    }
}

/// Both arms, in both orders, at the edges of the compare.
#[test]
fn a_select_of_constants_one_apart_keeps_its_value() {
    use llrm_m68k::compile::{Options, items_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::object::{executable, start, write_executable};
    let Some(qemu) = support::qemu() else { return };
    for (pred, then, other) in [("slt", 5, 4), ("slt", 4, 5), ("eq", 5, 4), ("ne", 7, 6), ("sge", 9, 8), ("ult", 1, 0)]
    {
        let module = llrm_mir::parse::module(&select_of(pred, then, other)).expect("parses");
        for (x, y) in [(1, 2), (2, 1), (3, 3), (-1, 0), (0, -1)] {
            let (a, b) = (x as i64, y as i64);
            let holds = match pred {
                "slt" => x < y,
                "sge" => x >= y,
                "eq" => x == y,
                "ne" => x != y,
                _ => (x as u32) < (y as u32),
            };
            let want = if holds { then } else { other };
            let mut all = start("f", &[a, b]);
            all.extend(items_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles"));
            let path = support::unique_path("select-consts");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &[], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(want), "{pred} {then}/{other} on ({x}, {y})");
        }
    }
}

/// A 64-bit select keeps the `zext` form: its `sext(!c)` costs the high half
/// (`sge; ext.w; ext.l; ...; cmp; bcc`, 10 more bytes at 68000 -O1) where the
/// `zext` adds with carry.
#[test]
fn a_64_bit_select_of_constants_keeps_the_carry_add() {
    let ir = format!(
        r#"{DATALAYOUT}
define i64 @f(i32 %x, i32 %y) {{
  %c = icmp slt i32 %x, %y
  %r = select i1 %c, i64 5, i64 4
  ret i64 %r
}}
"#
    );
    let out = assembly(&ir, cpu_of("68000").unwrap(), Level::O1).expect("compiles");
    assert!(out.contains("addx"), "the 64-bit select lost its carry add:\n{out}");
    assert!(!out.contains("ext.w"), "the 64-bit select pays a sign extension:\n{out}");
}
