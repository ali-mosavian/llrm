//! The five Borland memory models through clang and llrm-llc: code is near or
//! far (`proc near`/`retf`), and a data pointer is near (`[bx]`) or far
//! (`es:[bx]`). Needs the i8086 clang (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn listing(model: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = std::env::temp_dir().join(format!("llrm-cc-model-{model}-{}.s", std::process::id()));
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-S", "-O1", &format!("-mcmodel={model}"), "-o"])
        .arg(&out)
        .arg(root.join("tests/cc/models/models.c"))
        .output()
        .unwrap();
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    assert!(run.status.success(), "{model}: {}", String::from_utf8_lossy(&run.stderr));
    text
}

/// `-mcmodel=` once stopped at the driver ("unsupported argument") and every
/// model compiled as medium.
#[test]
fn each_model_picks_the_distance_of_its_code_and_its_data() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    for (model, far_code, far_data) in [
        ("small", false, false),
        ("medium", true, false),
        ("compact", false, true),
        ("large", true, true),
        ("huge", true, true),
    ] {
        let text = listing(model);
        assert_eq!(text.contains("proc far"), far_code, "{model} code:\n{text}");
        assert_eq!(text.contains("retf"), far_code, "{model} return:\n{text}");
        assert_eq!(text.contains("es:[bx]"), far_data, "{model} data:\n{text}");
    }
}

/// Borland's pointer probes (arithmetic, comparison and conversion of each
/// distance, declaration lists, conventions) once came from i86clang; every
/// model must still compile them. `llrm.ia16.ptrdiff` was missing, so a huge
/// difference was refused.
#[test]
fn the_borland_probes_compile_in_every_model() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for probe in ["conventions", "defaults", "lists", "pointers"] {
        for model in ["small", "medium", "compact", "large", "huge"] {
            let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
                .env("LLRM_CC_TARGET", "x86_16-dos")
                .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
                .args(["-S", "-O1", &format!("-mcmodel={model}"), "-o", "/dev/null"])
                .arg(root.join("tests/cc/models").join(format!("{probe}.c")))
                .output()
                .unwrap();
            assert!(run.status.success(), "{probe} in {model}: {}", String::from_utf8_lossy(&run.stderr));
        }
    }
}

/// `#pragma aux NAME "LINK"` and the segment pragmas were ignored: the
/// runtime's `B$` routines were `_name` references and every function and datum
/// went to the module's own segments.
#[test]
fn watcom_pragmas_name_symbols_and_segments() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = std::env::temp_dir().join(format!("llrm-cc-pragmas-{}.obj", std::process::id()));
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-c", "-O1", "-o"])
        .arg(&out)
        .arg(root.join("tests/cc/pragmas/pragmas.c"))
        .output()
        .unwrap();
    let object = std::fs::read(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let has = |text: &str| object.windows(text.len()).any(|window| window == text.as_bytes());
    assert!(has("B$SINCOS") && !has("_B$SINCOS"), "link name");
    assert!(has("CNT$") && !has("_CNT$"), "link name of a variable");
    assert!(has("HOT_TEXT"), "code_seg");
    assert!(has("XIB"), "data_seg");
}

/// A constant offset on a huge pointer folded as the integer `6000:0000 +
/// 70000` = `6001:1170`, one paragraph in the selector where a 64K carry is
/// 0x1000: every element past 64K of a huge buffer was read from the wrong
/// place.
#[test]
fn a_constant_huge_offset_carries_a_64k_step_into_the_selector() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = std::env::temp_dir().join(format!("llrm-cc-hugeconst-{}.s", std::process::id()));
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&out)
        .arg(root.join("tests/cc/models/hugeconst.c"))
        .output()
        .unwrap();
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(!text.contains("24577"), "{text}");
}

/// Small and compact (near code) crashed on start-up: the runtime called `main`
/// and `report` far, and a program in any model but medium and large never ran.
/// Each model's sieve prints 1028, the primes below 8192.
#[test]
fn a_program_runs_in_each_model() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for model in ["small", "medium", "compact", "large", "huge"] {
        let exe = std::env::temp_dir().join(format!("llrm-cc-run-{model}-{}.exe", std::process::id()));
        let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", "x86_16-dos")
            .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
            .args(["-w", "-O2", &format!("-mcmodel={model}"), "-o"])
            .arg(&exe)
            .arg(root.join("tests/cc/dos/sieve.c"))
            .output()
            .unwrap();
        assert!(built.status.success(), "{model}: {}", String::from_utf8_lossy(&built.stderr));
        let ran = Command::new("python3")
            .args(["-I"])
            .arg(root.join("tools/dosbatch/run_exe.py"))
            .arg("x86-m16")
            .arg(&exe)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&exe);
        assert_eq!(
            String::from_utf8_lossy(&ran.stdout).trim(),
            "1028",
            "{model}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }
}

/// A static huge object past 64K stopped llrm-llc with a Rust panic (`'H'
/// format requires 0 <= number <= 65535`) from the OMF writer. It is now
/// consecutive 64K segments, and a huge pointer walks all of its bytes (sum,
/// edges and a second object).
#[test]
fn a_static_huge_object_past_a_segment_is_consecutive_segments() {
    if std::env::var_os("LLRM_CLANG_M16").is_none() {
        eprintln!("skipped: LLRM_CLANG_M16 names no i8086 clang");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let compile = |extra: &[&str], output: &Path| {
        Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
            .env("LLRM_CC_TARGET", "x86_16-dos")
            .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
            .args(["-w", "-O2"])
            .args(extra)
            .arg("-o")
            .arg(output)
            .arg(root.join("tests/cc/dos/bigstatic.c"))
            .output()
            .unwrap()
    };
    let listing = std::env::temp_dir().join(format!("llrm-cc-big-{}.s", std::process::id()));
    let built = compile(&["-S"], &listing);
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let text = std::fs::read_to_string(&listing).unwrap_or_default();
    let _ = std::fs::remove_file(&listing);
    let segments: Vec<&str> = text.lines().filter(|line| line.contains(" segment para public 'FAR_DATA'")).collect();
    assert!(
        segments.iter().any(|line| line.contains("big$0")) && segments.iter().any(|line| line.contains("big$1")),
        "{segments:?}"
    );
    let exe = std::env::temp_dir().join(format!("llrm-cc-big-{}.exe", std::process::id()));
    let built = compile(&[], &exe);
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let ran = Command::new("python3")
        .args(["-I"])
        .arg(root.join("tools/dosbatch/run_exe.py"))
        .arg("x86-m16")
        .arg(&exe)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&exe);
    let said = String::from_utf8_lossy(&ran.stdout).replace("\r\n", "\n");
    assert_eq!(said.trim().replace('\n', " "), "12748993 249 0 89 9");
}

/// tests/run/c programs written for llrm-c use `__far`/`__huge` at m32 (one
/// address space: they name nothing) and the OS header's
/// `PHYSICAL_TEXT_SCREEN`; the x86_32-dos row had neither, so 8 of them
/// stopped at "expected ';' after top level declarator".
#[test]
fn the_flat_dos_row_reads_far_and_huge_and_names_the_text_screen() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("llrm-cc-flat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.c"),
        "short __far a[4];\nlong __huge *p;\nint f(void) { volatile unsigned char *s = (unsigned char *)PHYSICAL_TEXT_SCREEN; return a[1] + (p != 0) + *s; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_32-dos")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-O1", "-w", "-S", "-o"])
        .arg(dir.join("a.s"))
        .arg(dir.join("a.c"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

/// Plain `char` is signed on the DOS rows and unsigned under `-mabi=watcom`, as
/// llrm-c had it: a stored 251 read -5 where watcom_far_sum.c wants 251 (it
/// printed 1229 for 1485), and copy_runs.c wants the signed sum.
#[test]
fn plain_char_is_unsigned_only_under_the_watcom_abi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("llrm-cc-char-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.c"), "int f(char *p) { return *p; }\n").unwrap();
    for (row, plain) in [("x86_16-dos", "-mabi=cdecl"), ("x86_32-dos", "-mabi=sysv")] {
        if row == "x86_16-dos" && std::env::var_os("LLRM_CLANG_M16").is_none() {
            eprintln!("skipped {row}: LLRM_CLANG_M16 names no i8086 clang");
            continue;
        }
        for (abi, unsigned) in [(plain, false), ("-mabi=watcom", true)] {
            let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
                .env("LLRM_CC_TARGET", row)
                .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
                .args(["-O1", "-w", abi, "-S", "-o"])
                .arg(dir.join("a.s"))
                .arg(dir.join("a.c"))
                .output()
                .unwrap();
            assert!(run.status.success(), "{row} {abi}: {}", String::from_utf8_lossy(&run.stderr));
            let listing = std::fs::read_to_string(dir.join("a.s")).unwrap();
            assert_eq!(listing.contains("movzx"), unsigned, "{row} {abi}: {listing}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_llc_is_looked_up_in_the_cargo_target_dir_when_one_is_set() {
    let root = Path::new("/repo");
    assert_eq!(target_dir::from(root, Some("/elsewhere".into())), Path::new("/elsewhere"));
    assert_eq!(target_dir::from(root, None), Path::new("/repo/target"));
}
