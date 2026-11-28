//! `p[0] * 10 + p[1]` on signed/unsigned bytes at m16 under `--isel=dag`
//! widened through the 32-bit form (`movsx eax, byte ptr`, the pin's
//! `EXTRACT_SUBREG (MOVSX32rm8), sub_16bit`): 0x66 prefix, +1 B per extend.
//! struct_odd_sizes -Os was 139 B against 129.

use std::path::Path;
use std::process::Command;

#[test]
fn a_byte_extended_to_a_word_uses_the_16_bit_form_at_m16() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ext16-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("c.c"),
        "int f(signed char *p, unsigned char *q) { return p[0] * 10 + p[1] * 100 + q[0] * 3 + q[1]; }\n",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_16-dos")
        .args(["-march=i386", "-Os", "--isel=dag", "-w", "-S", "-o"])
        .arg(dir.join("c.s"))
        .arg(dir.join("c.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(dir.join("c.s")).unwrap();
    assert!(!listing.contains("movsx e") && !listing.contains("movzx e"), "a 32-bit register extend:\n{listing}");
    assert!(
        listing.contains("movsx ax, byte") || listing.contains("movsx cx, byte"),
        "expected a 16-bit movsx:\n{listing}"
    );
}
