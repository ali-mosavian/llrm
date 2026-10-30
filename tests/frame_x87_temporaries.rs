//! Each x87 rounding store and reload had a frame object of its own and the
//! pin shares none (no lifetime markers), so frames grew with the roundings:
//! bench/nbody_single.bas m16 -Os 4 -> 72 bytes. They are spill slots now,
//! shared by `slots_colored` where their live ranges do not overlap.

mod common;

const ROUNDINGS: &str = "float g(double a, double b, double c) {
  float s = (float)(a * b);
  s = (float)(s * c);
  s = (float)(s + a);
  s = (float)(s * b);
  return (float)(s + c);
}
";

#[test]
fn test_roundings_through_memory_share_one_slot() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("g.c"), ROUNDINGS).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m16", "-march=i486", "-Os", "--isel=dag", "-S", "-o", "g.s", "g.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("g.s")).unwrap();
    let stores = text.matches("fstp dword ptr [bp-").count();
    assert!(stores >= 3, "premise: the roundings go through memory\n{text}");
    let reserved: i64 = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("sub sp, "))
        .map(|bytes| bytes.trim().parse::<i64>().unwrap())
        .sum();
    assert!(reserved <= 4, "{stores} roundings reserved {reserved} bytes (one slot is 4)\n{text}");
}
