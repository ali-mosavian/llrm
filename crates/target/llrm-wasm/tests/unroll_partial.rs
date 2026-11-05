use std::path::Path;
use std::process::Command;

/// `LoopUnrollPass` (wasm: partial threshold 30) unrolls the 40-trip inner
/// loop of bench/tile five times. The full unroller left it rolled: `tile`
/// ran 94725 instructions to the pin's 57229.
#[test]
fn a_loop_of_forty_trips_and_six_instructions_is_unrolled_by_five() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_partial.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let loads = String::from_utf8_lossy(&out.stdout).matches("i32.load16_s").count();
    assert_eq!(loads, 5, "{loads} loads in the inner loop");
}

/// A dot loop with a run-time trip count was left rolled (`runtime unroll is
/// not translated`); wasm's `dot` ran one multiply per trip where the pin
/// runs four, with a remainder loop.
#[test]
fn a_loop_of_unknown_trips_is_unrolled_by_four_with_a_remainder() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_runtime.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let muls = String::from_utf8_lossy(&out.stdout).matches("f64.mul").count();
    assert_eq!(muls, 5, "{muls} multiplies: four in the body, one in the remainder");
    // The loop keeps its back edge and tests the exit once per four copies; a
    // complete unroll (no back edge) mis-summed every n but 4
    // (object_extent_loop).
    let text = String::from_utf8_lossy(&out.stdout);
    let body = text.split("\tloop").nth(1).and_then(|rest| rest.split("end_loop").next()).expect("a loop");
    assert_eq!((body.matches("f64.mul").count(), body.matches("br_if").count()), (4, 1), "{body}");
}

/// The epilog loop of a run-time unroll was unrolled again by two (the pin
/// marks it `setLoopAlreadyUnrolled`): `object_extent_loop` at wasm O2 was
/// 1680 B where the pin's is 1489.
#[test]
fn the_remainder_of_a_run_time_unroll_is_not_unrolled_again() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_runtime_nested.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let muls = String::from_utf8_lossy(&out.stdout).matches("f64.mul").count();
    assert_eq!(muls, 5, "{muls} multiplies: four in the body, one in the remainder");
}

/// A loop of 101 trips unrolled by two got an epilog loop for the odd trip
/// (a run-time unroll of a known trip count): bench/particle's sum loop was
/// 12 B over what the pin's `UP.Runtime &= TripCount == 0` leaves, a copy of
/// the body after the loop. The pin keeps the exit in the first copy.
#[test]
fn a_loop_of_an_odd_known_trip_count_has_no_epilog() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_odd_trips.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .env("LLRM_WASM_IR", "1")
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let mid_end = String::from_utf8_lossy(&out.stderr);
    assert!(!mid_end.contains(".epil"), "{mid_end}");
}

/// The late `LoopRotate` (PassBuilderPipelines.cpp#L1621) ran after the unroll,
/// where the pin runs it before: the 101-trip loop unrolled by two, its exit
/// in the first copy, was rotated into a copied first iteration before the
/// loop (particle -O2: 21 B over the pin).
#[test]
fn a_loop_unrolled_with_its_exit_in_the_middle_is_not_rotated_afterwards() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_odd_trips.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let before_loop = text.split("\tloop").next().expect("a loop");
    assert!(!before_loop.contains("i32.load16_s"), "a copy of the first iteration before the loop: {text}");
}

fn mid_end_ir(ir: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(ir);
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .env("LLRM_WASM_IR", "1")
        .arg(path)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// bench/recfloat at O2 was 17 B over the pin: no header phi starts at an
/// integer constant, so the pin's `isEpilogProfitable` is false and the extra
/// iterations run first (`.prol`); ours always made an epilog (`.epil`).
#[test]
fn a_run_time_unroll_with_no_integer_start_runs_its_extra_iterations_first() {
    let mid_end = mid_end_ir("unroll_prolog.ll");
    assert!(mid_end.contains(".prol"), "{mid_end}");
    assert!(!mid_end.contains(".epil"), "{mid_end}");
}

/// The other side of the choice: a phi that starts at `0` keeps the epilog
/// (`isEpilogProfitable`), as the dot loop does.
#[test]
fn a_run_time_unroll_with_an_integer_start_keeps_its_epilog() {
    let mid_end = mid_end_ir("unroll_runtime.ll");
    assert!(mid_end.contains(".epil"), "{mid_end}");
    assert!(!mid_end.contains(".prol"), "{mid_end}");
}

/// A run-time unroll's prolog leaves a phi from poison beside one from 0.0,
/// both the prolog's value; the pin's coalescer erases the `implicit_def`
/// (`ErasableImplicitDef`) and the two share a local. bench/recfloat at O2 was
/// 512 B to the pin's 487 with a local and a copy per prolog iteration more.
#[test]
fn the_poison_phi_of_a_prolog_shares_the_local_of_its_neighbour() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_prolog.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    let locals = text.lines().find(|line| line.trim_start().starts_with(".local")).expect("a .local line");
    assert_eq!(locals.split_whitespace().skip(1).count(), 4, "{locals}");
}

/// A prolog's exit phis take `poison` from the skip edge. The wasm coalescer
/// erased their `IMPLICIT_DEF` but the instruction stayed, and its zero was
/// written into the local a join had given the live value: `run(n)` was wrong
/// for every n divisible by four from 4 up (n = 4 gave 12.625 for -165.625).
#[test]
fn a_prolog_whose_trip_count_divides_by_four_gives_the_right_sum() {
    let dir = std::env::temp_dir().join(format!("llrm-wasm-prolog-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/unroll_prolog_run.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-O2", "-filetype=obj", "-o"])
        .arg(dir.join("f.o"))
        .arg(&ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new("wasm-ld")
        .args(["--no-entry", "--export-all"])
        .arg(dir.join("f.o"))
        .arg("-o")
        .arg(dir.join("f.wasm"))
        .output()
        .expect("wasm-ld");
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let script = "const m = new WebAssembly.Module(require('fs').readFileSync(process.env.MODULE));
         const run = new WebAssembly.Instance(m, {}).exports.run;
         const f32 = Math.fround;
         const bad = [];
         for (let n = 0; n <= 17; n++) {
           let a = 100, b = f32(3), c = 7, d = f32(0.25);
           for (let i = 0; i < n; i++) { const a1 = a * 0.5 + 1; b = f32(b + 1); c = c - a; d = f32(d + d); a = a1; }
           const want = a + b + c + d;
           const got = run(100, 3, n, 7, 0.25);
           if (got !== want) bad.push(n + ': ' + got + ' != ' + want);
         }
         console.log(bad.length ? bad.join('; ') : 'ok');";
    let ran = Command::new("node").args(["-e", script]).env("MODULE", dir.join("f.wasm")).output().expect("node");
    std::fs::remove_dir_all(&dir).ok();
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "ok");
}
