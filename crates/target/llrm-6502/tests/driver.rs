//! The pipeline of `llrm-6502-llc` (`driver::assembly`): the request's platform and level reach the allocator.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn module(function: &str) -> String {
    format!("target datalayout = \"{}\"\n{function}\n", datalayout())
}

/// Ten sums are live at once. Compiled for sim65 (26 zero-page bytes) they fit in scratch cells; with 16 the
/// allocator spills. A driver that kept `select_function`'s own 16 spilled on every platform: 8% more bytes
/// over the hand corpus.
#[test]
fn the_platforms_zero_page_reaches_the_allocator() {
    let pairs = [("a", "b"), ("b", "c"), ("c", "d"), ("d", "e"), ("e", "f"), ("a", "c"), ("b", "d"), ("c", "e"), ("d", "f"), ("a", "f")];
    let mut ir = String::from("define i8 @many(i8 %a, i8 %b, i8 %c, i8 %d, i8 %e, i8 %f) {\n");
    for (n, (x, y)) in pairs.iter().enumerate() {
        ir += &format!("  %t{n} = add i8 %{x}, %{y}\n");
    }
    ir += "  %x0 = xor i8 %t0, %t1\n";
    for n in 2..pairs.len() {
        ir += &format!("  %x{} = xor i8 %x{}, %t{n}\n", n - 1, n - 2);
    }
    ir += &format!("  ret i8 %x{}\n}}", pairs.len() - 2);
    let mut request = Request::new(Level::O0);
    let wide = assembly(&module(&ir), &request).unwrap();
    request.platform.zero_page = 16;
    let narrow = assembly(&module(&ir), &request).unwrap();
    assert!(!wide.contains("s_many_") && !wide.contains("pha"), "sim65 still spills or saves:\n{wide}");
    assert!(narrow.contains("s_many_0") || narrow.contains("pha"), "no pressure at 16:\n{narrow}");
}

/// A word shifted by 5 is a counted loop at the size level and `asl`/`rol` pairs at speed. A driver that kept
/// `select_function`'s speed tuning gave `-Os` the same text as `-O2`.
#[test]
fn the_size_level_is_priced_in_bytes() {
    let ir = module("define i16 @shl5(i16 %x) {\n  %p = shl i16 %x, 5\n  %q = add i16 %p, %x\n  ret i16 %q\n}");
    let size = assembly(&ir, &Request::new(Level::Os)).unwrap();
    let speed = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(size.contains("dex") || size.contains("dey"), "-Os:\n{size}");
    assert!(!speed.contains("dex") && !speed.contains("dey"), "-O2:\n{speed}");
}

/// `LLRM_6502_ZERO_PAGE` sets the platform's zero page, whichever way the platform is named. It was read before
/// the row's own `--platform=sim65`, which put the 26 back: a sweep of 16..26 read flat, 4362 bytes at every size.
#[test]
fn the_zero_page_override_survives_the_platform_flag() {
    let dir = std::env::temp_dir().join(format!("llrm6502-zp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = dir.join("f.ll");
    std::fs::write(&ir, module("define i8 @f(i8 %a) {\n  ret i8 %a\n}")).unwrap();
    let equates = |bytes: &str| {
        let out = dir.join(format!("f{bytes}.s"));
        let ran = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
            .args(["--platform=sim65", "-S", "-O0", "-o", out.to_str().unwrap(), ir.to_str().unwrap()])
            .env("LLRM_6502_ZERO_PAGE", bytes)
            .status()
            .unwrap();
        assert!(ran.success());
        std::fs::read_to_string(out).unwrap().lines().filter(|line| line.starts_with("rc")).count()
    };
    assert_eq!((equates("16"), equates("26")), (16, 26));
    std::fs::remove_dir_all(dir).ok();
}

/// `-Os` holds an inline to the size threshold: a callee of about forty instructions called three times stays
/// a call, where `-O2` copies it in. The hand corpus and bench tables have no such callee, so their `-Os` rows
/// equal their `-O2` rows and cannot show whether `-Os` reaches the mid-end.
#[test]
fn the_size_level_keeps_a_call_that_speed_inlines() {
    let ir = module(
        "@sink = external global [8 x i8]
define internal i8 @mix(i8 %x, i8 %y) {
  %a3 = mul i8 %x, 3
  %a = add i8 %a3, %y
  %xy = xor i8 %x, %y
  %b = add i8 %xy, 5
  %c = sub i8 %a, %b
  %ab = and i8 %a, %b
  %cx = xor i8 %c, %x
  %d = or i8 %ab, %cx
  %p0 = getelementptr i8, ptr @sink, i16 0
  store i8 %a, ptr %p0
  %p1 = getelementptr i8, ptr @sink, i16 1
  store i8 %b, ptr %p1
  %p2 = getelementptr i8, ptr @sink, i16 2
  store i8 %c, ptr %p2
  %p3 = getelementptr i8, ptr @sink, i16 3
  store i8 %d, ptr %p3
  %e = add i8 %a, %d
  %p4 = getelementptr i8, ptr @sink, i16 4
  store i8 %e, ptr %p4
  %f = sub i8 %b, %c
  %p5 = getelementptr i8, ptr @sink, i16 5
  store i8 %f, ptr %p5
  %ad = xor i8 %a, %d
  %g = add i8 %ad, %x
  %p6 = getelementptr i8, ptr @sink, i16 6
  store i8 %g, ptr %p6
  %bc = or i8 %b, %c
  %h = sub i8 %bc, %y
  %p7 = getelementptr i8, ptr @sink, i16 7
  store i8 %h, ptr %p7
  %s1 = add i8 %a, %b
  %s2 = add i8 %s1, %c
  %s3 = add i8 %s2, %d
  ret i8 %s3
}
define i8 @f(i8 %p, i8 %q) {
  %m1 = call i8 @mix(i8 %p, i8 %q)
  %m2 = call i8 @mix(i8 %q, i8 %p)
  %m3 = call i8 @mix(i8 %p, i8 9)
  %r1 = add i8 %m1, %m2
  %r2 = add i8 %r1, %m3
  ret i8 %r2
}",
    );
    let calls = |level| assembly(&ir, &Request::new(level)).unwrap().matches("jsr mix").count();
    assert_eq!((calls(Level::O2), calls(Level::Os)), (0, 3));
}

/// The pin's `opt -O2` makes an eight-trip CRC loop a byte-wise table lookup (`LoopIdiomRecognize`, priced by the
/// target's cost model). Compiled with a machine that had no cost model the recognizer never ran: crc16 kept its
/// bit loop (25 instructions, 6497 clocks) where the pin has 19.
#[test]
fn a_crc_loop_becomes_a_table_lookup() {
    let ir = module("define i16 @crc16(ptr %0, i8 %1) {\n\
  %.not = icmp eq i8 %1, 0\n\
  br i1 %.not, label %._crit_edge, label %.lr.ph\n\
\n\
.lr.ph:                                           \n\
  %.01118 = phi i8 [ %15, %14 ], [ 0, %2 ]\n\
  %.01217 = phi i16 [ %12, %14 ], [ -1, %2 ]\n\
  %3 = zext i8 %.01118 to i16\n\
  %4 = getelementptr i8, ptr %0, i16 %3\n\
  %5 = load i8, ptr %4, align 1\n\
  %6 = zext i8 %5 to i16\n\
  %7 = shl i16 %6, 8\n\
  %8 = xor i16 %7, %.01217\n\
  br label %9\n\
\n\
9:                                                \n\
  %.016 = phi i8 [ 0, %.lr.ph ], [ %13, %9 ]\n\
  %.115 = phi i16 [ %8, %.lr.ph ], [ %12, %9 ]\n\
  %10 = shl i16 %.115, 1\n\
  %11 = xor i16 %10, 4129\n\
  %.not14 = icmp slt i16 %.115, 0\n\
  %12 = select i1 %.not14, i16 %11, i16 %10\n\
  %13 = add i8 %.016, 1\n\
  %exitcond.not = icmp eq i8 %13, 8\n\
  br i1 %exitcond.not, label %14, label %9\n\
\n\
14:                                               \n\
  %15 = add i8 %.01118, 1\n\
  %exitcond19.not = icmp eq i8 %15, %1\n\
  br i1 %exitcond19.not, label %._crit_edge, label %.lr.ph\n\
\n\
._crit_edge:                                      \n\
  %.012.lcssa = phi i16 [ -1, %2 ], [ %12, %14 ]\n\
  ret i16 %.012.lcssa\n\
}\n\
");
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains(".segment \"RODATA\"") && text.matches(".byte").count() >= 32, "no table:\n{text}");
    assert!(!text.contains("cmp #8"), "the bit loop is still there:\n{text}");
}
