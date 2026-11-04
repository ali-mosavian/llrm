//! A mutable variable of zeros is bss, not bytes of the image.

use std::process::Command;

fn assembly(
    name: &str,
    ir: &str,
) -> String {
    let dir = std::env::temp_dir().join(format!("llrm-bss-z80-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\ntarget triple = \"z80-unknown-none\"\n{ir}"),
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `static short a[32][32]` was 2048 `.db 0` bytes: tests/run/c/loop_interchange
/// measured 8210 B against SDCC's 1237 B, 6144 of them the three zero arrays.
#[test]
fn a_zero_initialised_array_is_reserved_not_emitted() {
    let text = assembly("array", "@a = global [1024 x i16] zeroinitializer\n");
    assert!(text.contains(".area _BSS"), "{text}");
    assert!(text.contains(".ds 2048"), "{text}");
    assert!(!text.contains(".db 0,0"), "{text}");
}

/// A variable with a non-zero byte keeps its bytes, and a constant of zeros
/// stays in the image: a ROM table has no start-up clear.
#[test]
fn initialised_and_constant_data_keep_their_bytes() {
    let text = assembly("kept", "@d = global [4 x i8] c\"\\00\\01\\00\\00\"\n@k = constant [4 x i8] zeroinitializer\n");
    assert!(text.contains(".db 0,1,0,0"), "{text}");
    assert!(text.contains(".db 0,0,0,0"), "{text}");
}
