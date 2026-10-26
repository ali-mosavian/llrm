//! The five Borland memory models through clang and llrm-llc: code is near or
//! far (`proc near`/`retf`), and a data pointer is near (`[bx]`) or far
//! (`es:[bx]`). Needs the i8086 clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

fn listing(model: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let out = std::env::temp_dir().join(format!("llrm-cc-model-{model}-{}.s", std::process::id()));
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .env("LLRM_LLC", root.join("target/release/llrm-llc"))
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
                .env("LLRM_LLC", root.join("target/release/llrm-llc"))
                .args(["-S", "-O1", &format!("-mcmodel={model}"), "-o", "/dev/null"])
                .arg(root.join("tests/cc/models").join(format!("{probe}.c")))
                .output()
                .unwrap();
            assert!(run.status.success(), "{probe} in {model}: {}", String::from_utf8_lossy(&run.stderr));
        }
    }
}
