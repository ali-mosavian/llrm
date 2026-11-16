//! A loop of bitwise operations over 16-bit elements runs two elements per
//! 32-bit register, as gcc does with no SSE (`ix86_preferred_simd_mode` word
//! mode, `tree-vect-generic.cc`). Ours xor'ed one element a trip: x_bigadd -O3
//! 336,072 clk against gcc's 286,005.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "unsigned short a[64], c[65];\n\
    void xr(long r) { long i; for (i = 0; i < 64; ++i) a[i] = (unsigned short)(c[i] ^ r); }\n";

fn xr(level: &str) -> Vec<String> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("word-vectors-{}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .env(
            "LLRM_LLC",
            std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
        )
        .args(["-m32", "-march=i486", level, "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(dir.join("c.s"))
        .unwrap()
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != "xr proc near")
        .take_while(|line| !line.ends_with("endp"))
        .map(str::to_owned)
        .collect()
}

fn xors(body: &[String]) -> (usize, usize) {
    let wide = body.iter().filter(|line| line.starts_with("xor e")).count();
    let narrow = body.iter().filter(|line| line.starts_with("xor ") && !line.starts_with("xor e")).count();
    (wide, narrow)
}

#[test]
fn test_m32_o2_a_xor_loop_over_shorts_runs_two_elements_a_register() {
    let body = xr("-O2");
    assert_eq!(xors(&body).1, 0, "a 16-bit xor is still in the loop: {body:#?}");
    assert!(xors(&body).0 >= 1, "no 32-bit xor: {body:#?}");
}

#[test]
fn test_m32_os_a_xor_loop_over_shorts_stays_element_wise() {
    let body = xr("-Os");
    assert!(xors(&body).1 >= 1, "-Os widened the loop (bytes rank first there): {body:#?}");
}
