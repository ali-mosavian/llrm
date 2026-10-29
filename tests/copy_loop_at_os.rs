//! At -Os a copy loop of a length known at run time stays a loop where the
//! `rep movs` that would replace it is more bytes.

mod common;

fn os_copy_listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "extern int a[], b[];\nvoid copy(int lo, int mid, int hi)\n{\n    int i = mid, k = lo;\n    while (i < hi) b[k++] = a[i++];\n}\n",
    )
    .unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        .args(flags)
        .args(["-m16", "-march=i386", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// x_mergesort's three copy loops at m16 -Os were `rep movsd` and `rep movsb`
/// with the count split between them: 29 bytes more than the loops.
#[test]
fn at_os_a_copy_of_a_run_time_length_is_not_rep_movs() {
    let text = os_copy_listing(&["-Os"]);
    assert!(!text.contains("rep movs"), "{text}");
}

#[test]
fn at_o2_the_same_copy_is_rep_movs() {
    let text = os_copy_listing(&["-O2"]);
    assert!(text.contains("rep movs"), "{text}");
}
