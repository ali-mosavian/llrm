//! At -Os a loop of three trips is copied out where the copies are small:
//! x_life's 3x3 neighbour sum kept `inc; jne` per element (407 B against gcc's
//! 344).

mod common;

#[test]
fn a_loop_of_three_trips_is_copied_out_at_os() {
    let scratch = tempfile::tempdir().unwrap();
    let source = "int f(const unsigned char (*g)[24], int y, int x)\n{\n    int c = 0, dy, dx;\n    for (dy = -1; dy <= 1; ++dy)\n        for (dx = -1; dx <= 1; ++dx) c += g[y + dy][x + dx];\n    return c;\n}\n";
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-Os", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let backward =
        listing.lines().filter(|line| line.trim().starts_with("jne") || line.trim().starts_with("jl")).count();
    assert_eq!(backward, 0, "a loop is kept: {listing}");
}
