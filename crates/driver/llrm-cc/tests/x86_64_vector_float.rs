//! x86-64: a loop of float lanes is vectorized where the pin prices it.
//! `table[j] = (base + j) * 0.5` stayed scalar (`cvtsi2sd` a lane) as no vector
//! of float lanes had a cost: the init loops of object_extent_loop, +8.7%
//! against clang's `cvtdq2pd`/`mulpd` at -O2. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

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

/// A vector `fcmp` was `CannotSelect("SetCC")` (and a `<4 x float>` value had
/// no register class): `cmpps`/`cmppd` now make the mask of all six orders over
/// NaN lanes, and the mask select is `pand`/`por`. The program runs and prints
/// what gcc -O0 prints (`vector_float_compare.out`). Needs `LLRM_LLC` and a
/// host `cc` for the link.
#[test]
fn a_vector_float_compare_makes_the_mask_gcc_makes() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-compare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c");
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-fno-pic", "-O2", "-w", "-c", "-o"])
        .arg(&object)
        .arg(tests.join("vector_float_compare.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let exe = dir.join("a.out");
    let link =
        Command::new("cc").args(["-static", "-no-pie", "-o"]).arg(&exe).arg(&object).arg("-lm").output().unwrap();
    assert!(link.status.success(), "{}", String::from_utf8_lossy(&link.stderr));
    let out = Command::new(&exe).output().unwrap();
    let expected = std::fs::read_to_string(tests.join("vector_float_compare.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
    let asm = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", std::env::var_os("LLRM_LLC").unwrap())
        .args(["-O2", "-w", "-S", "-o", "-"])
        .arg(tests.join("vector_float_compare.c"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&asm.stdout);
    assert!(text.contains("cmpps") && text.contains("cmppd"), "no packed compare:\n{text}");
}

/// sfm16_float_cmp at -O2 stayed scalar on x86-64 (12,036 executed instructions
/// a call against clang's 3,848): the vector plan needed a priced float
/// compare, mask extension, lane insert and pointer extract, and the pin's rows
/// were not there. Now the kernel is `cmpps` over four lanes. Needs `LLRM_LLC`.
#[test]
fn the_sfm16_float_cmp_kernel_is_cmpps_at_o2() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sfm16-cmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-fno-pic", "-O2", "-w", "-S", "-Dmain=bench_main_", "-I"])
        .arg(root.join("bench"))
        .arg("-I")
        .arg(root.join("runtime/shared/posix"))
        .arg("-o")
        .arg(&asm)
        .arg(root.join("bench/sfm16_float_cmp/sfm16_float_cmp.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    let body = text
        .split("bench_sfm16_float_cmp proc near")
        .nth(1)
        .and_then(|rest| rest.split("bench_sfm16_float_cmp endp").next())
        .expect("the kernel");
    assert!(body.contains("cmpps"), "the compare loop stayed scalar:\n{body}");
}

/// An init loop's `uitofp nneg <4 x i32>` was not selectable and an `i32` lane
/// gather was unpriced, so the SLP planner left the loop scalar (`cvtsi2ss` a
/// lane, never `cvtdq2ps`). The loop's `i64` index trees stay scalar (their
/// gathers are not priced: row X64-SLP-I64-GATHER). Needs `LLRM_LLC`.
#[test]
fn a_float_init_loop_from_an_int_counter_is_cvtdq2ps_at_o2() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-float-f32-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/vector_float_init_f32.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(&asm).unwrap();
    assert!(text.contains("cvtdq2ps"), "no packed conversion:\n{text}");
}

/// A `<4 x i32>` to `<4 x float>` unsigned convert had no selection (`UIntToFp`
/// of a vector with no register class). Every lane, from 0 to 2^32 - 1, must
/// round as gcc -O0's scalar convert does. Needs `LLRM_LLC` and a host `cc`.
#[test]
fn an_unsigned_vector_convert_rounds_as_the_scalar_one_does() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("vector-uitofp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c");
    let object = dir.join("a.o");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-m64", "-fno-pic", "-O2", "-w", "-c", "-o"])
        .arg(&object)
        .arg(tests.join("vector_uint_to_float.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let exe = dir.join("a.out");
    let link =
        Command::new("cc").args(["-static", "-no-pie", "-o"]).arg(&exe).arg(&object).arg("-lm").output().unwrap();
    assert!(link.status.success(), "{}", String::from_utf8_lossy(&link.stderr));
    let out = Command::new(&exe).output().unwrap();
    let expected = std::fs::read_to_string(tests.join("vector_uint_to_float.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}
