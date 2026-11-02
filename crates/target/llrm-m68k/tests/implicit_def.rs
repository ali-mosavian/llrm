//! A phi input that is poison or undef, at each integer width.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

fn loop_with_undef_input(
    width: &str,
    value: &str,
) -> String {
    let narrow = if width == "i32" { "%q = add i32 %i, 7".to_owned() } else { format!("%q = trunc i32 %i to {width}") };
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
define {width} @f(i32 %n) {{
entry:
  br label %loop
loop:
  %p = phi {width} [ {value}, %entry ], [ %q, %loop ]
  %i = phi i32 [ 0, %entry ], [ %j, %loop ]
  {narrow}
  %j = add i32 %i, 1
  %c = icmp ult i32 %j, %n
  br i1 %c, label %loop, label %out
out:
  ret {width} %p
}}"
    )
}

/// The pin emits `IMPLICIT_DEF` for the copy of an undefined value; the
/// emitter had no name for the node and refused the function ("a machine node
/// with no name: 10") at every level, so no function with an undef phi input
/// compiled.
#[test]
fn a_phi_input_that_is_poison_or_undef_compiles_at_every_width() {
    for (width, value) in [("i8", "poison"), ("i16", "undef"), ("i32", "poison")] {
        let ir = loop_with_undef_input(width, value);
        for cpu in [Cpu::M68000, Cpu::M68020] {
            for level in [Level::O2, Level::Os] {
                let text =
                    assembly(&ir, cpu, level).unwrap_or_else(|why| panic!("{width} {value} {cpu:?} {level:?}: {why}"));
                assert!(text.contains("rts"), "{text}");
            }
        }
    }
}
