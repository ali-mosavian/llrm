//! x86-64: a loop of float lanes is vectorized where the pin prices it. `table[j] = (base + j) * 0.5` stayed scalar
//! (`cvtsi2sd` a lane) as no vector of float lanes had a cost: the init loops of object_extent_loop, +8.7% against
//! clang's `cvtdq2pd`/`mulpd` at -O2. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn compile(source: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-float-{}-{source}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

/// `BoUpSLP::getTreeCost` trims a subtree dearer than a gather of its root's scalars. A pair of `(i + k) * 0.5` kept
/// every lane scalar (`mulsd`): the pair's conversions made the tree cost +2 and nothing cut them.
#[test]
fn a_pair_of_converted_counters_is_one_packed_multiply_at_x86_64() {
    let text = compile("vector_float_trim.c");
    assert!(text.contains("mulpd"), "no packed multiply:\n{text}");
    assert!(!text.contains("mulsd"), "a scalar multiply is left:\n{text}");
}

/// object_extent_loop's init, +5.6% over clang 20 at -O2: the pin's InstCombine splits the 2-D address, so the 12-trip
/// loop fits the unroll threshold and SLP pairs its copies; clang 20 vectorizes the loop (VF 2, six trips unrolled).
/// The loop's `trunc <2 x i64>` is narrowed onto the splat: `paddd`, not `paddq` and a `pshufd` each.
#[test]
fn a_two_dimensional_fill_is_a_vector_loop_with_narrow_counters_at_x86_64() {
    let text = compile("vector_float_pairs.c");
    assert!(text.contains("cvtdq2pd"), "no packed conversion:\n{text}");
    assert!(!text.contains("cvtsi2sd"), "a scalar conversion is left:\n{text}");
    assert!(text.contains("paddd") && !text.contains("paddq"), "the counters are not narrow:\n{text}");
    // `trunc (build_vector x, x)` is the build of the truncated lanes: `movd`, `pshufd`, not `movq`, `punpcklqdq`.
    assert!(!text.contains("punpcklqdq"), "the 64-bit splat is truncated, not narrowed:\n{text}");
}

/// A splat of a counter into the low lanes of a `<4 x i32>` (the widened `<2 x i32>`) was built through the stack.
#[test]
fn a_splat_of_a_counter_is_movd_and_pshufd_at_x86_64() {
    let text = compile("vector_float_splat.c");
    assert!(text.contains("pshufd"), "no shuffle:\n{text}");
    assert!(!text.contains("[rsp"), "the splat goes through the stack:\n{text}");
}

#[test]
fn a_loop_filling_doubles_from_an_int_counter_is_vector_code_at_x86_64() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-float-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_float_init.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(text.contains("cvtdq2pd"), "no packed conversion:\n{text}");
    assert!(text.contains("mulpd"), "no packed multiply:\n{text}");
}

/// A run-time-length dot product is unrolled (two or more `mulsd` in the loop), as the pin's `UP.Runtime`, clang 20 and
/// gcc do; the recurrence veto kept one: +4.6% executed instructions on loop_countdown_sum, +1.8% on object_extent_loop.
#[test]
fn a_runtime_dot_product_is_unrolled_at_x86_64() {
    let text = compile("unroll_dot.c");
    assert!(text.matches("mulsd").count() >= 2, "the loop is not unrolled:\n{text}");
}
