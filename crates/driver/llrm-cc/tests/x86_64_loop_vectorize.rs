//! x86-64 -O2: LoopVectorize runs on the integer loops isel can select (`paddd`, `movups`), because the target now
//! says it has a 128-bit vector register and prices the add, the subtract, the logic ops and the contiguous access
//! (X86TTIImpl rows); every op it does not price keeps its loop scalar (a `<4 x i32>` multiply has no SSE2 form).
//! Before, `getRegisterBitWidth` was 0 and no loop was widened (copy_runs, loop_fusion, fill_nest: 15-59% more
//! instructions than clang). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(target: &str, level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    // Tests run in parallel: a directory of its own for each call.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-{}-{call}-{target}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lv_loops.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(text: &'a str, name: &str) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

#[test]
fn an_integer_add_loop_is_four_lanes_wide_at_o2() {
    let text = assembly("x86_64-linux", "-O2");
    let add = body(&text, "add_arrays");
    assert!(add.contains("paddd") && (add.contains("movups") || add.contains("movaps")), "{add}");
}

/// An unknown trip count and pointers that may overlap: the remainder loop and the runtime check are emitted, and the
/// listing still assembles (no op without a form).
#[test]
fn a_runtime_trip_count_loop_has_a_vector_body_and_a_scalar_remainder() {
    let text = assembly("x86_64-linux", "-O2");
    let copy = body(&text, "copy_plus_one");
    assert!(copy.contains("paddd"), "{copy}");
    assert!(copy.contains("add ") || copy.contains("inc "), "the scalar remainder: {copy}");
}

/// A `<4 x i32>` multiply is not priced (no SSE2 `pmulld`): the loop stays scalar until the next step prices it.
#[test]
fn a_loop_with_a_vector_multiply_stays_scalar_until_it_is_priced() {
    let text = assembly("x86_64-linux", "-O2");
    let multiply = body(&text, "multiply_arrays");
    assert!(!multiply.contains("xmm"), "{multiply}");
}

#[test]
fn the_32_bit_target_and_o1_keep_every_loop_scalar() {
    assert!(!assembly("x86_32-linux", "-O2").contains("xmm"));
    assert!(!assembly("x86_64-linux", "-O1").contains("xmm"));
}

/// A sum reached `llvm.vector.reduce.add` and the backend refused it (`CannotSelect("VecReduceAdd")`, no output at
/// all), so reductions were kept scalar. The total is now folded from the spilled lanes (`expandVecReduce`).
#[test]
fn a_sum_loop_is_vector_with_a_folded_total() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "sum_array");
    assert!(sum.contains("paddd"), "{sum}");
}

/// The widened loops compute what the scalar ones do: sums and xors folded from vector lanes, and forward and backward copies that overlap by 1 to 5 bytes, every
/// length under 100, run on the host. The count-down copy sat 31 bytes low at IC 2 (`bad 340`).
#[test]
fn overlapping_forward_and_backward_copies_keep_their_bytes_at_o2() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("a.o");
    let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set"))
        .args(["-O2", "-w", "-c", "-o"])
        .arg(&object)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lv_run.c"))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let exe = dir.join("a.out");
    let linked = Command::new("gcc").args(["-static", "-no-pie", "-o"]).arg(&exe).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let ran = Command::new(&exe).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "bad 0");
}

/// The vector loop of a 64-lane row (VF 8, IC 2: four trips) was kept as a loop: the unroller found no size for a
/// `<8 x i16>` store ("no cost for store") and never fully unrolled it. The pin unrolls it to one trip of eight
/// stores per row.
#[test]
fn a_vector_loop_of_four_trips_is_fully_unrolled() {
    let text = assembly("x86_64-linux", "-O2");
    let fill = body(&text, "fill_rows");
    let stores = fill.matches("movaps xmmword ptr [").count() + fill.matches("movups xmmword ptr [").count();
    assert!(stores >= 8, "{fill}");
}
