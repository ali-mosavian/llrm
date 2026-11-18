use std::path::Path;
use std::process::Command;

/// `tests/ir/os_inline_growth.ll`: a ten-operation helper called from four
/// callers, as assembly at `level`.
fn assembly(level: &str) -> String {
    assembly_of("os_inline_growth.ll", level)
}

fn assembly_of(file: &str, level: &str) -> String {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(file);
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args([level, "-S", "-o", "-"])
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    String::from_utf8_lossy(&made.stdout).into_owned()
}

/// The pin's `OptSizeThreshold` 50 inlines a helper at every site that prices under it, here
/// four. The SZ1 size bound kept it a call at all four (`sf_cmp_bits` and 16 more -Os cells
/// above the pin on bytes or executed instructions).
#[test]
fn a_size_build_inlines_a_helper_the_pin_prices_under_the_threshold_at_every_site() {
    let text = assembly("-Os");
    assert_eq!(text.matches("call\thelper").count() + text.matches("call helper").count(), 0, "{text}");
}

/// A callee with one site is the pin's inline (cost 45 against `OptSizeThreshold` 50): the size
/// guard holds a callee copied into several callers only. Held at one site too, the once-called
/// `bench_X` of 12 corpus cells stayed a call from `main` at -Os, one call and return more than
/// the pin's executed instructions (recgcd +7, recchop +1), with the bytes below the pin's.
#[test]
fn a_size_build_inlines_a_helper_with_one_site() {
    let text = assembly_of("os_inline_single_site.ll", "-Os");
    assert_eq!(text.matches("call\thelper").count() + text.matches("call helper").count(), 0, "{text}");
}
