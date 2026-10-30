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
