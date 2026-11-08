//! A variable-length `memcpy` inlined as `rep movsd; rep movsb` (m32 -O2,
//! `--isel=dag`): the byte tail recomputed `dst`/`src` + `n - n % 4` and kept
//! them live across the dword copy, which cost two stack spills and 46 B over
//! the old selector (loop_idiom_variable_copy, 136 B against 90 B). The tail
//! goes on from the registers the dword copy leaves.

use std::path::Path;
use std::process::Command;

#[test]
fn test_m32_dag_the_byte_tail_of_an_inline_copy_spills_nothing() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("copy-tail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "extern unsigned char src[12000];\nunsigned char dst[512];\nlong pos;\n\
         int rd(int count)\n{\n    int n = 0;\n    while (n < count && pos < 12000) dst[n++] = src[pos++];\n    return n;\n}\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-linux")
        .args(["-m32", "-march=i486", "-O2", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    let copy: Vec<&str> = listing
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != "rep movsd")
        .take_while(|line| *line != "rep movsb")
        .collect();
    assert!(!copy.is_empty(), "no rep movsd .. rep movsb in:\n{listing}");
    assert!(
        copy.iter().all(|line| !line.contains("[esp") && !line.starts_with("mov edi, offset")),
        "the tail reloads or spills between the two copies:\n{}",
        copy.join("\n")
    );
}
