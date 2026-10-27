//! A jump table is dense enough at 40% when tuned for size, 10% otherwise.

mod common;

const KERNEL: &str = "
extern void a(void), b(void), c(void), d(void);
void f(int x)
{
    switch (x) {
    case 192: a(); break;
    case 194: b(); break;
    case 196: c(); break;
    case 219: d(); break;
    }
}
";

fn listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--isel=dag", "-m16", "-march=i386", level, "-S", "-o", "-", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The DAG's switch lowering never read `-Os`: four cases over 28 values (14%)
/// became a 28-word table at m16 -Os where the pin, at 40%, compares
/// (jpeg_fixed.c m16 -Os +153 bytes, one table of 28 `dw`).
#[test]
fn test_a_sparse_switch_is_no_jump_table_when_tuned_for_size() {
    assert!(!listing("-Os").contains("jmp word ptr"), "{}", listing("-Os"));
}

/// Not tuned for size, the same switch is a table: 14% is over 10%.
#[test]
fn test_a_sparse_switch_is_a_jump_table_at_o2() {
    assert!(listing("-O2").contains("jmp word ptr"), "{}", listing("-O2"));
}
