//! At m16 a jump table is dense enough at 40% at every level (DEVIATE
//! M16-JT-DENSITY); the pin asks 40% only tuned for size.

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

/// DEVIATE M16-JT-DENSITY: not tuned for size the pin's 10% made the same four
/// cases over 28 values a 56-byte table (jpeg_fixed and jpeg_single m16 -O1:
/// data +58 B and +60 B over bcc); at m16 it compares at -O1 and -O2 too.
#[test]
fn test_a_sparse_switch_is_no_jump_table_at_o1_and_o2() {
    for level in ["-O1", "-O2"] {
        let text = listing(level);
        assert!(!text.contains("jmp word ptr"), "{level}: {text}");
    }
}

/// The bar moved, jump tables did not go: ten cases over ten values are a table
/// at -O2.
#[test]
fn test_a_dense_switch_is_still_a_jump_table_at_o2() {
    let scratch = tempfile::tempdir().unwrap();
    let cases: String = (0..10).map(|n| format!("case {n}: a{n}(); break;\n")).collect();
    let decls: String = (0..10).map(|n| format!("extern void a{n}(void);\n")).collect();
    std::fs::write(scratch.path().join("a.c"), format!("{decls}void f(int x)\n{{\nswitch (x) {{\n{cases}}}\n}}\n"))
        .unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["--isel=dag", "-m16", "-march=i386", "-O2", "-S", "-o", "-", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("jmp word ptr"), "{text}");
}
