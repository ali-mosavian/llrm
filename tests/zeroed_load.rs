//! `movzx r,m8` is three clocks on a 486 and `xor r,r; mov rl,m8` two: the
//! target's own prices decide, so a CPU where the extension is one clock, or
//! that stalls on the partial register, keeps `movzx`.

use std::process::Command;

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "unsigned h(unsigned char *p, unsigned char *q, int n) { unsigned s = 0; int i; for (i = 0; i < n; i++) s += p[i] * q[i]; return s; }\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .args(["-m32"])
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// x_life has fewer instructions than gcc's but more clocks: every byte load
/// was `movzx` (3 clocks on the 486) where `xor ecx,ecx; mov cl,[m]` is 2.
#[test]
fn a_486_clears_the_register_and_loads_its_low_byte() {
    for cpu in ["-march=i486", "-march=pentium"] {
        let text = listing(&["-O2", cpu]);
        assert!(text.contains("xor ecx, ecx\n    mov cl, byte ptr"), "{cpu}: {text}");
    }
}

/// Tuned for size the pair is more bytes: `movzx` stays.
#[test]
fn a_size_build_keeps_movzx() {
    let text = listing(&["-Os", "-march=i486"]);
    assert!(text.contains("movzx") && !text.contains("mov cl, byte ptr"), "{text}");
}
