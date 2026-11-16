//! A size build on x86-m32 copies no loop latch to thread a jump. The pin's
//! JumpThreading copied the outer loop's latch of x_prefix for its `n <= 0`
//! guard: a second `inc; cmp 4; jne` and a second exit, 163 B against gcc's
//! 157 B (gcc Os never rotates these loops).

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "static long a[1024];\n\
    long f(long n) {\n\
        long i, r, sum = 0;\n\
        for (i = 0; i < n; ++i) a[i] = (i * 5) & 15;\n\
        for (r = 0; r < 4; ++r) {\n\
            for (i = 1; i < n; ++i) a[i] += a[i - 1];\n\
            for (i = 0; i < n; ++i) a[i] &= 0xFFFF;\n\
        }\n\
        for (i = 0; i < n; ++i) sum += a[i];\n\
        return sum;\n\
    }\n";

fn latch_tests(level: &str) -> usize {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("os-jt{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("c.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", level, "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    listing.lines().filter(|line| line.trim_start().starts_with("cmp ") && line.trim_end().ends_with(", 4")).count()
}

#[test]
fn test_m32_os_tests_the_outer_trip_count_once() {
    assert_eq!(latch_tests("-Os"), 1, "-Os copied the outer latch to thread the n <= 0 guard");
}
