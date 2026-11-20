//! A 64-bit shift whose amount is a `zext` made on each arm of a branch kept
//! the casts apart from their uses: the pin's CodeGenPrepare sinks a cast to
//! its user's block. `pick` took 57 instructions and saved `d2-d5`; sunk it
//! takes 39 and saves `d2-d4` (u64_shift_select_arms: 574 -> 406 B at 68000
//! -O2, 8024 -> 7804 clk).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const PICK: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i64 @pick(i32 %a, i32 %e) {
  %and = and i32 %a, 8388607
  %m32 = or i32 %and, 8388608
  %m = zext i32 %m32 to i64
  %ge = icmp sge i32 %e, 23
  br i1 %ge, label %l, label %r
l:
  %s1 = sub nsw i32 %e, 23
  %z1 = zext i32 %s1 to i64
  %v1 = shl i64 %m, %z1
  br label %j
r:
  %s2 = sub nsw i32 23, %e
  %z2 = zext i32 %s2 to i64
  %v2 = lshr i64 %m, %z2
  br label %j
j:
  %p = phi i64 [ %v1, %l ], [ %v2, %r ]
  ret i64 %p
}";

#[test]
fn a_cast_is_made_in_the_block_that_uses_it() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let out = assembly(PICK, cpu_of(cpu).unwrap(), level).expect("compiles");
            let instructions = out.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
            assert!(instructions <= 45, "{cpu} {level:?}: {instructions} instructions (sunk: 39):\n{out}");
            assert!(!out.contains("%d5"), "{cpu} {level:?}: d5 saved (sunk: d2-d4):\n{out}");
        }
    }
}
