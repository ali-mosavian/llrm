//! A long shifted left by one in a loop keeps its words in place.

use std::process::Command;

fn assembly(ir: &str) -> String {
    let guard = tempfile::tempdir().unwrap();
    let dir = guard.path().to_path_buf();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n{ir}"),
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O1", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `while (!(x & 256)) x <<= 1` on a `u32` was 36 instructions and an `ix`
/// frame (`add hl, hl` / `adc hl, de` pinned both words to hl: four live pairs,
/// the low word spilled), SDCC's is 7: `sla e; rl d; adc hl, hl` in place.
#[test]
fn a_long_doubled_in_a_loop_needs_no_frame() {
    let text = assembly(
        "define i32 @g(i32 %x) {\nentry:\n  br label %l\nl:\n  %p = phi i32 [ %x, %entry ], [ %s, %l ]\n  \
         %a = and i32 %p, 256\n  %z = icmp eq i32 %a, 0\n  %s = shl i32 %p, 1\n  br i1 %z, label %l, label %e\ne:\n  ret i32 %p\n}\n",
    );
    let body = text.lines().filter(|l| l.starts_with('\t') && !l.starts_with("\t.")).count();
    assert!(!text.contains("(ix"), "{text}");
    assert!(body <= 18, "{body} instructions:\n{text}");
}
