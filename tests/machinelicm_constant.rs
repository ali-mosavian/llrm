//! `MachineLICM` hoists a constant out of a loop (`IsProfitableToHoist`:
//! "Trivially rematerializable instructions should always be hoisted").

mod common;

/// bench/recsum at -O1: a trip is 14 instructions, the pin's count with the
/// multiplier hoisted (15 with it made beside the `imul` and a copy of the
/// dividend as well, +1.7% clocks). On a 486 the multiplier is the r/m operand
/// (early-out reads it), so the constant is the accumulator: `mov eax, K` ahead
/// of `imul` replaces the copy of the dividend and cannot be hoisted, `imul`
/// overwrites EAX. The property is the trip count, not where the constant is.
#[test]
fn a_trip_of_a_magic_multiply_loop_has_no_extra_copy() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/recsum/recsum.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O1", "--isel=dag", "-S", "-o", "a.s"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body: Vec<&str> =
        asm.lines().skip_while(|l| !l.contains("_sum proc")).take_while(|l| !l.contains("_sum endp")).collect();
    let top = body.iter().position(|l| l.trim_start().starts_with("align")).unwrap_or_else(|| panic!("{asm}"));
    let trip =
        body[top + 1..].iter().filter(|l| !l.ends_with(':')).take_while(|l| !l.trim_start().starts_with("jne")).count()
            + 1;
    assert!(trip <= 14, "a trip is {trip} instructions: {asm}");
}
