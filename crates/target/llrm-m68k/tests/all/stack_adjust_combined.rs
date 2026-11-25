//! The cleanup of a pushed call and the epilogue's frame release were two
//! adjacent stack adjustments (`lea (12,%sp), %sp; lea (64,%sp), %sp`), where
//! gcc's `combine-stack-adjustments` emits one. QuickrBasic `blok_stmt`:
//! 134 such pairs in the -Os 68020 corpus, gcc 1.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const FRAME_AND_CALL: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare void @g(ptr, i32, i32)
define void @f(i32 %c, i32 %d) {
  %b = alloca [16 x i32]
  call void @g(ptr %b, i32 %c, i32 %d)
  ret void
}"#;

fn adjusts_sp(line: &str) -> bool {
    let line = line.trim();
    (line.starts_with("lea\t(") || line.starts_with("adda") || line.starts_with("add.l\t#") || line.starts_with("addq"))
        && line.ends_with("%sp")
}

#[test]
fn a_call_cleanup_and_the_frame_release_are_one_adjustment() {
    for cpu in ["68000", "68020"] {
        let out = assembly(FRAME_AND_CALL, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let lines: Vec<&str> = out.lines().collect();
        let pairs = lines.windows(2).filter(|pair| adjusts_sp(pair[0]) && adjusts_sp(pair[1])).count();
        assert_eq!(pairs, 0, "{cpu}: two stack adjustments in a row:\n{out}");
    }
}
