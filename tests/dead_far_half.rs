//! A far pointer's segment half carried round a loop that reads only its
//! offset.

mod common;

const KERNEL: &str = "
extern void use(char *p);
void walk(char far *p, int n)
{
    do {
        use((char *)p);
        p += 40;
    } while (--n);
}
";

/// The pointer is two halves in the loop's phis, and nothing reads the segment
/// half: its copies joined away and left a definition nothing reads with the
/// range of a live value, and `LLRM_CHECK_LIVEINTERVALS` stopped the compile
/// ("the edited interval of v16 differs from the one made afresh"; QCport
/// ui/menu.c at -Os). A dead phi cycle is removed before the coalescer.
#[test]
fn test_a_segment_half_nothing_reads_is_not_carried_round_the_loop() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .env("LLRM_CHECK_LIVEINTERVALS", "1")
        .args(["-m16", "-march=i386", "-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
