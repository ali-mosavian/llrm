//! `movzx r,m8` is three clocks on a 486 and `xor r,r; mov rl,m8` two: the
//! target's own prices decide, so a CPU where the extension is one clock, or
//! that stalls on the partial register, keeps `movzx`.

mod common;

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "unsigned h(unsigned char *p, unsigned char *q, int n) { unsigned s = 0; int i; for (i = 0; i < n; i++) s += p[i] * q[i]; return s; }\n",
    )
    .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32"])
        .args(flags)
        .args(["-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// The pin extends a byte load with `movzx` on a 486 and a Pentium alike
/// (clang -O2 -m32 -march=i486: `movzbl (%esi,%edi), %ebx`); the `xor r,r; mov
/// rl,m8` pair that x_life used was a tuning of the old selector, not the pin's.
#[test]
fn a_486_extends_a_byte_load_with_movzx_as_the_pin_does() {
    for cpu in ["-march=i486", "-march=pentium"] {
        let text = listing(&["-O2", cpu]);
        assert!(text.contains("movzx") && !text.contains("mov cl, byte ptr"), "{cpu}: {text}");
    }
}

/// Tuned for size the pair is more bytes: `movzx` stays.
#[test]
fn a_size_build_keeps_movzx() {
    let text = listing(&["-Os", "-march=i486"]);
    assert!(text.contains("movzx") && !text.contains("mov cl, byte ptr"), "{text}");
}
