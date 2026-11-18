//! x86-64 -O2: float compares, selects and conversions are vector loops. The
//! compare, the conversions and the float loads of a strided access had no
//! vector price (and `cmpps`, `cvttps2dq`, a `pandn` select had no selection),
//! so sfm16_float_cmp, _f2i and sfm16_float ran 1.5x to 2.6x clang's
//! instructions. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly() -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    // Tests run in parallel: a directory of its own for each call.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-compare-{}-{call}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_float_compare.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

#[test]
fn a_float_compare_loop_is_cmpps() {
    let text = assembly();
    assert!(body(&text, "compare_lt").contains("cmpps"), "{}", body(&text, "compare_lt"));
}

#[test]
fn a_select_of_floats_by_a_compare_is_and_andn_or() {
    let text = assembly();
    let select = body(&text, "select_min");
    assert!(select.contains("cmpps") && select.contains("pandn") && select.contains("por"), "{select}");
}

#[test]
fn float_and_int_conversion_loops_are_four_lanes_wide() {
    let text = assembly();
    assert!(body(&text, "float_to_int").contains("cvttps2dq"), "{}", body(&text, "float_to_int"));
    assert!(body(&text, "int_to_float").contains("cvtdq2ps"), "{}", body(&text, "int_to_float"));
}

/// The gather of lanes 44 bytes apart: `movss` and `unpcklps`, never the stack.
#[test]
fn a_strided_float_loop_gathers_in_registers() {
    let text = assembly();
    let loop_body = body(&text, "strided_sum");
    assert!(loop_body.contains("unpcklps") && loop_body.contains("cvttps2dq"), "{loop_body}");
    assert!(!loop_body.contains("[rsp"), "the vector is built through the stack:\n{loop_body}");
}
