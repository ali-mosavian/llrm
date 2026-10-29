//! `MachineLICM` hoists a constant out of a loop (`IsProfitableToHoist`:
//! "Trivially rematerializable instructions should always be hoisted").

mod common;

/// bench/recsum at -O1: the multiplier of `n % 11` was made again every trip
/// (`mov edx, 780903145` ahead of the `imul`); the pin keeps it in a register
/// made once ahead of the loop (+1.7% clocks, 15 instructions a trip against
/// 14).
#[test]
fn a_constant_the_loop_reads_is_made_ahead_of_it() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/recsum/recsum.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "--isel=dag", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body: Vec<&str> =
        asm.lines().skip_while(|l| !l.contains("_sum proc")).take_while(|l| !l.contains("_sum endp")).collect();
    let made = body.iter().position(|l| l.contains("780903145")).unwrap_or_else(|| panic!("{asm}"));
    let top = body.iter().position(|l| l.trim_start().starts_with("align")).unwrap_or_else(|| panic!("{asm}"));
    assert!(made < top, "the constant is made inside the loop: {asm}");
}
