//! `(add (add x, c), y)` is `(add (add x, y), c)`, so the constant lands in the
//! last `lea`.

mod common;

const KERNEL: &str = "
int f(int a, int b, unsigned s)
{
    int x = (int)((s >> 20) & 0x3FF) - 512;
    return (x * 1024 + a * 1600 - b * 700) >> 10;
}
";

/// The constant stayed in the inner sum (`lea eax, [eax+edx-524288]; add eax,
/// ecx; sar`): `reassociateOps` was not translated, so the pin's `add edx, ecx;
/// lea eax, [eax+edx-524288]; sar` had a different shape and, in x_iir's loop,
/// one more copy (+1.1% clocks m32 -O2).
#[test]
fn test_a_constant_is_added_in_the_last_lea() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--isel=dag", "-m32", "-mabi=sysv", "-march=i486", "-O2", "-S", "-o", "-", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let sar = lines.iter().position(|line| line.starts_with("sar")).expect("a sar");
    assert!(lines[sar - 1].starts_with("lea") && lines[sar - 1].contains("524288"), "{asm}");
}
