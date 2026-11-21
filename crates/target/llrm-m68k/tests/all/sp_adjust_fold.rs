//! variadic.c -Os (68000/68020, +60 B against gcc): `main` ended with the cleanup of its last call and the frame's
//! release as two adjusts of `%sp` (`lea (100,%sp),%sp` then `lea (24,%sp),%sp`, 4 B each). They are one adjust.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// A local the call cannot see (volatile) keeps a frame; five pushed arguments leave a cleanup owed at the return.
const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
declare void @f(i32, i32, i32, i32, i32)\n\
define void @g() nounwind optsize {\n  %a = alloca [24 x i8]\n  store volatile i32 1, ptr %a\n\
  call void @f(i32 1, i32 2, i32 3, i32 4, i32 5)\n  ret void\n}\n";

fn adjusts_sp(line: &str) -> bool {
    let line = line.trim();
    line.ends_with(", %sp") && (line.starts_with("lea\t") || line.starts_with("add") || line.starts_with("sub"))
        && !line.contains("%a") || line.starts_with("lea\t(") && line.ends_with("), %sp")
}

/// Two adjacent adjusts of `%sp` at the return are one.
#[test]
fn the_cleanup_of_the_last_call_and_the_frame_release_are_one_adjust() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let lines: Vec<&str> = asm.lines().collect();
        let doubled = lines.windows(2).filter(|pair| adjusts_sp(pair[0]) && adjusts_sp(pair[1])).count();
        assert_eq!(doubled, 0, "{cpu}: two adjusts of %sp in a row:\n{asm}");
    }
}
