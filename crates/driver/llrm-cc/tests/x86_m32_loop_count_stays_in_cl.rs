//! A shift count the loops of an outer iteration share stays in `cl`. The pin's
//! IndVarSimplify `sinkUnusedInvariants` moves `pass * 8` past the `memset` the
//! zeroing loop became; left before it, the count lived across the call in a
//! callee-saved register and was copied into `ecx` on each trip of both loops
//! (x_radix -Os: 1,191 extra `mov`, 44,059 clk against clang's 43,041).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "typedef unsigned long u32;\n\
    static u32 a[256], b[256];\n\
    long f(long n) {\n\
        u32 s = 31337; long i, pass; long sum = 0;\n\
        for (i = 0; i < n; ++i) { s = s * 1103515245ul + 12345ul; a[i] = s >> 8; }\n\
        for (pass = 0; pass < 3; ++pass) {\n\
            u32 count[256]; long shift = pass * 8, d;\n\
            for (d = 0; d < 256; ++d) count[d] = 0;\n\
            for (i = 0; i < n; ++i) ++count[(a[i] >> shift) & 255];\n\
            { u32 t = 0; for (d = 0; d < 256; ++d) { u32 c = count[d]; count[d] = t; t += c; } }\n\
            for (i = 0; i < n; ++i) b[count[(a[i] >> shift) & 255]++] = a[i];\n\
            for (i = 0; i < n; ++i) a[i] = b[i];\n\
        }\n\
        for (i = 0; i < n; ++i) sum = sum * 7 + (long)(a[i] & 0xFFFF);\n\
        return sum;\n\
    }\n";

fn count_copies() -> usize {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cl-count-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-Os", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    let lines: Vec<&str> = listing.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|pair| pair[0].starts_with("mov ecx, e") && pair[1].starts_with("shr ") && pair[1].ends_with(", cl"))
        .count()
}

#[test]
fn test_m32_os_loops_share_the_shift_count_in_cl() {
    assert_eq!(count_copies(), 0, "the shift count was copied into ecx inside a loop");
}
