//! -Os: an `inlinehint` callee the pin inlines (hint threshold 325) is inlined
//! only where the module gets smaller. The size bound stood down for the hint,
//! which the units cannot price: `h` was copied into three callers, +60 B on
//! this module; the compile that keeps the calls is kept when it is fewer bytes
//! (gcc `ipa-inline.cc` `want_inline_small_function_p`). QuickrBasic m68k Os
//! exec.c and operands.c shrink by 3,084 and 1,062 B with the yield; code.c and
//! suggest.c grew without the trial.

use std::process::Command;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
target triple = \"m68k-unknown-linux-gnu\"\n\
define internal i32 @h(i32 %x, i32 %y) inlinehint nounwind optsize {\n\
  %a = mul i32 %x, 7\n\
  %b = add i32 %a, %y\n\
  %c = xor i32 %b, 12345\n\
  %d = mul i32 %c, 11\n\
  %e = sub i32 %d, %x\n\
  %f = shl i32 %e, 3\n\
  %g = xor i32 %f, %y\n\
  %h = add i32 %g, 99999\n\
  %i = mul i32 %h, 5\n\
  ret i32 %i\n\
}\n\
define i32 @f1(i32 %p, i32 %q) nounwind optsize { %r = call i32 @h(i32 %p, i32 %q)  ret i32 %r }\n\
define i32 @f2(i32 %p, i32 %q) nounwind optsize { %r = call i32 @h(i32 %q, i32 %p)  %s = add i32 %r, 1 ret i32 %s }\n\
define i32 @f3(i32 %p, i32 %q) nounwind optsize { %r = call i32 @h(i32 %p, i32 7)  ret i32 %r }\n\
";

fn assembly(level: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, IR).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-m68k-llc"))
        .args(["-S", level, "-mcpu=68020", "-o", "-"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(said.stdout).unwrap()
}

#[test]
fn an_inlinehint_callee_that_grows_the_module_stays_a_call_at_size() {
    let text = assembly("-Os");
    assert!(text.contains("h:"), "{text}");
}

/// The trial is for -Os: -O2 takes the hint as the pin does.
#[test]
fn the_pins_hint_threshold_stands_at_speed() {
    let text = assembly("-O2");
    assert!(!text.contains("h:"), "{text}");
}
