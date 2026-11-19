//! jpeg_fixed -Os (68000/68020, +172 B against gcc): `put32`, a 4-trip store loop called from eight cold sites, was
//! copied into each. The units price a copy at 15 against a call at 18; the backend writes a copy bigger than the
//! call and the callee it keeps (outlined: -20 B). A cold site the units accept is built as a call too and the smaller
//! finished code is kept.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

const PUT32: &str = "@bmp = global [64 x i8] zeroinitializer\n\
@src = global i32 0\n\
define internal void @put32(i32 %at, i32 %value) {\n\
entry:\n\
  br label %loop\n\
loop:\n\
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]\n\
  %sh = shl i32 %i, 3\n\
  %v = lshr i32 %value, %sh\n\
  %b = trunc i32 %v to i8\n\
  %o = add i32 %at, %i\n\
  %p = getelementptr i8, ptr @bmp, i32 %o\n\
  store i8 %b, ptr %p\n\
  %n = add i32 %i, 1\n\
  %d = icmp eq i32 %n, 4\n\
  br i1 %d, label %done, label %loop\n\
done:\n\
  ret void\n\
}\n\
";

fn sites(count: usize) -> String {
    let calls: String = (0..count)
        .map(|k| format!("  %v{k} = load volatile i32, ptr @src\n  call void @put32(i32 {}, i32 %v{k})\n", k * 4))
        .collect();
    format!("define i32 @main() optsize nounwind {{\n{calls}  ret i32 0\n}}\n")
}

/// Eight copies of the loop are bigger than one loop and eight calls.
#[test]
fn a_callee_of_eight_cold_sites_stays_a_call_when_the_copies_are_bigger() {
    let ir = format!("{DATALAYOUT}{PUT32}{}", sites(8));
    for cpu in ["68000", "68020"] {
        let asm = assembly(&ir, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let calls = asm.lines().filter(|line| line.trim_start().starts_with("jsr")).count();
        assert_eq!(calls, 8, "{cpu}: the callee was copied into its sites:\n{asm}");
    }
}
