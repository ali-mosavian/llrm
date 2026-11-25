//! m68k never ran `LiveOutRegInfo`: a bool read in the block after the one
//! that loaded it (`select` of two bool tests becomes two branches) lost its
//! `!range`, so the branch tested `and.b #1, %d0` where `tst.b` is enough
//! (QuickrBasic `sugg_program` and others: 2 B per test). Os.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

#[test]
fn a_ranged_bool_read_in_a_later_block_is_not_masked() {
    let body = r#"
@flag = global i8 0
declare void @g()
define void @f(i32 %n) optsize {
  %z = icmp eq i32 %n, 0
  %w = load i8, ptr @flag, align 1, !range !0
  %u = trunc nuw i8 %w to i1
  %s = select i1 %z, i1 %u, i1 false
  br i1 %s, label %a, label %b
a:
  call void @g()
  ret void
b:
  ret void
}
!0 = !{i8 0, i8 2}"#;
    for cpu in ["68000", "68020"] {
        let out = assembly(&format!("{DATALAYOUT}\n{body}"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let masks = out.lines().filter(|line| line.trim().starts_with("and.b")).count();
        assert_eq!(masks, 0, "{cpu}: a bool known to be 0 or 1 is masked again:\n{out}");
    }
}
