//! `a >= c ? a - c : a` is one `sub` whose flags the branch reads.

mod common;

const KERNEL: &str = "
extern unsigned total;
void adler(const unsigned char *p, unsigned n)
{
    unsigned a = total;
    while (n--) {
        a += *p++;
        if (a >= 65521) a -= 65521;
    }
    total = a;
}
";

/// The difference was a `lea` and the compare a separate `cmp`
/// (`lea edx, [eax+ebx-65521]; cmp edi, esi`), two more instructions than the
/// pin's `sub ecx, esi; jae` in every Adler-32 loop (jpeg_fixed `emit`, m16
/// -Os +9 instructions): `combineX86AddSub` was not translated, so the
/// generic `add x, -c` stayed apart from the `X86ISD::SUB` of the compare.
#[test]
fn test_a_conditional_subtract_reuses_the_compare_flags() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--isel=dag", "-m32", "-mabi=sysv", "-march=i486", "-Os", "-S", "-o", "-", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    assert!(!asm.contains("lea") && !asm.contains("cmp"), "{asm}");
    assert!(asm.contains("sub") && asm.contains("jae"), "{asm}");
}
