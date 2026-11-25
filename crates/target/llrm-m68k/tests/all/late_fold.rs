//! QuickrBasic `arena_grow` closed twice (`movem; rts` on each return); a
//! call whose paths shared its tail kept the arguments as stores in the call
//! area, because the tail ended an area the stores began; and two paths that
//! made the same call and met after it made it twice, because only blocks that
//! return were folded. The tails are shared after the frame is made
//! (`BranchFolder` after prologue/epilogue insertion). All at `-Os`.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

fn at_os(
    body: &str,
    cpu: &str,
) -> String {
    assembly(&format!("{DATALAYOUT}\n{body}"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles")
}

fn count(
    out: &str,
    start: &str,
) -> usize {
    out.lines().filter(|line| line.trim().starts_with(start)).count()
}

const ARENA_GROW: &str = r#"declare ptr @arena_alloc(ptr, i32)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define ptr @arena_grow(ptr %0, ptr %1, i32 %2, i32 %3) optsize {
  %5 = icmp eq ptr %1, null
  br i1 %5, label %6, label %8
6:
  %7 = tail call ptr @arena_alloc(ptr %0, i32 %3)
  br label %34
8:
  %9 = add i32 %2, 7
  %10 = and i32 %9, -8
  %11 = getelementptr inbounds i8, ptr %1, i32 %10
  %12 = getelementptr inbounds i8, ptr %0, i32 8
  %13 = load ptr, ptr %12, align 2
  %14 = icmp eq ptr %11, %13
  br i1 %14, label %15, label %30
15:
  %16 = getelementptr inbounds i8, ptr %0, i32 4
  %17 = load ptr, ptr %16, align 2
  %18 = getelementptr inbounds i8, ptr %17, i32 4
  %19 = load ptr, ptr %18, align 4
  %20 = ptrtoint ptr %19 to i32
  %21 = ptrtoint ptr %1 to i32
  %22 = sub i32 %20, %21
  %23 = add i32 %3, 7
  %24 = and i32 %23, -8
  %25 = icmp ult i32 %22, %24
  br i1 %25, label %30, label %26
26:
  %27 = getelementptr inbounds i8, ptr %1, i32 %24
  store ptr %27, ptr %12, align 2
  %28 = getelementptr inbounds i8, ptr %1, i32 %2
  %29 = sub i32 %3, %2
  tail call void @llvm.memset.p0.i32(ptr align 1 %28, i8 0, i32 %29, i1 false)
  br label %34
30:
  %31 = tail call ptr @arena_alloc(ptr %0, i32 %3)
  %32 = icmp eq ptr %31, null
  br i1 %32, label %34, label %33
33:
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 1 %31, ptr align 1 %1, i32 %2, i1 false)
  br label %34
34:
  %35 = phi ptr [ %1, %26 ], [ %31, %33 ], [ null, %30 ], [ %7, %6 ]
  ret ptr %35
}"#;

#[test]
fn two_returns_share_one_closing() {
    for cpu in ["68000", "68020"] {
        let out = at_os(ARENA_GROW, cpu);
        assert_eq!(count(&out, "rts"), 1, "{cpu}: the closing is repeated per return:\n{out}");
    }
}

#[test]
fn a_call_on_two_paths_pushes_its_arguments() {
    let body = r#"
declare void @f(ptr, i32)
define void @two(i1 %c, ptr %p) optsize {
  br i1 %c, label %a, label %b
a:
  call void @f(ptr %p, i32 3)
  br label %e
b:
  call void @f(ptr %p, i32 13)
  br label %e
e:
  ret void
}"#;
    for cpu in ["68000", "68020"] {
        let out = at_os(body, cpu);
        let stores = out
            .lines()
            .filter(|line| {
                let line = line.trim();
                line.starts_with("move.l") && (line.ends_with(", (%sp)") || line.ends_with(", (4,%sp)"))
            })
            .count();
        assert_eq!(stores, 0, "{cpu}: {stores} arguments kept as stores in the call area:\n{out}");
    }
}

#[test]
fn two_paths_that_meet_share_a_call() {
    let body = r#"
declare void @f(ptr, i32)
declare i32 @g(ptr)
define i32 @meet(i1 %c, ptr %p) optsize {
  br i1 %c, label %a, label %b
a:
  call void @f(ptr %p, i32 3)
  br label %e
b:
  call void @f(ptr %p, i32 13)
  br label %e
e:
  %r = call i32 @g(ptr %p)
  %s = add i32 %r, 1
  ret i32 %s
}"#;
    for cpu in ["68000", "68020"] {
        let out = at_os(body, cpu);
        assert_eq!(count(&out, "jsr\tf"), 1, "{cpu}: the call is made on both paths:\n{out}");
    }
}

/// Every merge rebuilds the block graph; the scan of a block's edges read all
/// its instructions, so a function of long blocks took time in proportion to
/// merges x instructions (`synth.c` at 68000 -O1 with `-funroll-loops`: over
/// 8 minutes, 60 % of the first 100 s in `latefold`). The scan reads the
/// block's last instructions only.
#[test]
fn the_edge_scan_reads_a_blocks_terminators_not_its_body() {
    let mut body =
        String::from("declare void @sink(ptr)\ndefine void @f(ptr %p, i32 %n) optsize {\nentry:\n  br label %b0\n");
    for i in 0..40 {
        body += &format!("b{i}:\n  %c{i} = icmp eq i32 %n, {i}\n  br i1 %c{i}, label %a{i}, label %b{}\n", i + 1);
        body += &format!("a{i}:\n");
        for j in 0..24 {
            body += &format!(
                "  %g{i}_{j} = getelementptr i32, ptr %p, i32 {}\n  store i32 {}, ptr %g{i}_{j}\n",
                j,
                i * 100 + j
            );
        }
        body += "  call void @sink(ptr %p)\n  br label %end\n";
    }
    body += "b40:\n  br label %end\nend:\n  ret void\n}\n";
    let before = llrm_m68k::latefold::edge_scans();
    let _ = at_os(&body, "68020");
    let (instructions, blocks) = llrm_m68k::latefold::edge_scans();
    let (instructions, blocks) = (instructions - before.0, blocks - before.1);
    assert!(blocks > 0, "the late fold scanned nothing");
    assert!(instructions <= 4 * blocks, "{instructions} instructions read for {blocks} block scans");
}
