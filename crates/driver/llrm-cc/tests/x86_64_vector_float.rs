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

/// `BoUpSLP::getTreeCost` trims a subtree dearer than a gather of its root's scalars. Pairs of `(i + j) * 0.5` kept
/// every lane scalar (`mulsd`, one per element: object_extent_loop's init, +5.6% over clang at -O2) as the pair's
/// conversions and truncates made the whole tree cost +12 and nothing cut them.
#[test]
fn a_pair_of_converted_counters_is_one_packed_multiply_at_x86_64() {
    let text = compile("vector_float_pairs.c");
    assert!(text.contains("mulpd"), "no packed multiply:\n{text}");
    assert!(!text.contains("mulsd"), "a scalar multiply is left:\n{text}");
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
