//! An `adc` takes the carry of the `add` before it, never the flags of a
//! `mul` or `or` the scheduler hoisted in between (a 128-bit product lost
//! carries at -O2: x_m64, softfloat). Needs the Open Watcom tree
//! (tools/ow-fetch.sh) and the m16 clang (`LLRM_CLANG_M16`).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "typedef unsigned int u32; typedef unsigned long long u64;\nstatic inline void m32(u32 a, u32 b, u32 *hi, u32 *lo) { u64 p = (u64)a * b; *hi = (u32)(p >> 32); *lo = (u32)p; }\nstatic inline void mul64(u64 a, u64 b, u64 *hi, u64 *lo)\n{\n    u32 h, l; u64 ll, lh, hl, hh, mid;\n    m32((u32)a, (u32)b, &h, &l); ll = (u64)h << 32 | l;\n    m32((u32)a, (u32)(b >> 32), &h, &l); lh = (u64)h << 32 | l;\n    m32((u32)(a >> 32), (u32)b, &h, &l); hl = (u64)h << 32 | l;\n    m32((u32)(a >> 32), (u32)(b >> 32), &h, &l); hh = (u64)h << 32 | l;\n    mid = (ll >> 32) + (lh & 0xFFFFFFFFull) + (hl & 0xFFFFFFFFull);\n    *lo = (ll & 0xFFFFFFFFull) | (mid << 32);\n    *hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);\n}\nu64 g(u64 s, u64 t, u64 *lo)\n{\n    u64 x = s | 0x4000000000000000ull, B = t | 0x8000000000000000ull, hi, l2;\n    mul64(x, B, &hi, lo);\n    mul64(x, 0 - hi, &hi, &l2);\n    return hi + l2;\n}\n";

#[test]
fn an_adc_never_reads_the_flags_of_a_mul_or_logical_op() {
    assert!(
        llrm_cc::ow_present(),
        "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip"
    );
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("carrymul-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-wat")
        .args(["-O2", "-w", "-S", "-march=i486", "-o"])
        .arg(dir.join("d.s"))
        .arg(dir.join("d.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("d.s")).unwrap();
    let mut last = "";
    for line in text.lines() {
        let Some(mnemonic) = line.split_whitespace().next() else { continue };
        if matches!(mnemonic, "adc" | "sbb") {
            assert!(
                matches!(last, "" | "add" | "adc" | "sub" | "sbb" | "cmp" | "neg"),
                "{mnemonic} follows {last}:\n{text}"
            );
        }
        if matches!(
            mnemonic,
            "add"
                | "adc"
                | "sub"
                | "sbb"
                | "cmp"
                | "neg"
                | "mul"
                | "imul"
                | "or"
                | "and"
                | "xor"
                | "test"
                | "shl"
                | "shr"
                | "sar"
                | "shld"
                | "shrd"
                | "lea"
                | "inc"
                | "dec"
        ) {
            last = mnemonic;
        }
    }
}
