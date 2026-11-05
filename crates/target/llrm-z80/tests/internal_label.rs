//! An `internal` function is the module's own: a symbol of another module's
//! may share its name.

use std::process::Command;

/// `static inline` `sf_clz64` was `.globl _sf_clz64` in both the soft-float f64
/// mul and div: float_bits and float_in_place_update failed to link ("Multiple
/// definition of _sf_clz64").
#[test]
fn an_internal_function_is_not_exported_by_the_driver() {
    let dir = std::env::temp_dir().join("llrm-internal-label-z80");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
         define internal i16 @helper(i16 %a) noinline {\n  %r = add i16 %a, 1\n  ret i16 %r\n}\n\
         define i16 @api(i16 %a) {\n  %r = call i16 @helper(i16 %a)\n  ret i16 %r\n}\n",
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O1", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    assert!(!text.contains("_helper::") && !text.contains(".globl _helper"), "{text}");
    assert!(text.contains("_helper:\n") && text.contains("_api::"), "{text}");
}
