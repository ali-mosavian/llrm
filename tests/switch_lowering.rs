//! A switch is not a chain of compares: x_switch_dense was 11 in a row (445 B,
//! 1.23x gcc's clocks) and x_switch_ranges 58 (770 B, 3.2x).

mod common;

fn compares(
    kernel: &str,
    flags: &[&str],
) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    let source = format!("{}/crates/target/llrm-x86-m32/vsgcc/kernels/{kernel}/{kernel}.c", env!("CARGO_MANIFEST_DIR"));
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-fobject-format=elf", "-S", "-o", "a.s"])
        .args(flags)
        .arg(source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s"))
        .unwrap()
        .lines()
        .filter(|line| line.trim().starts_with("cmp"))
        .count()
}

/// x_switch_ranges: 58 case values over a span of 114 were a tree of compares
/// at 51% density, 2.09x clang's instructions; the table is priced under that
/// tree.
#[test]
fn a_character_class_at_half_density_is_a_lookup_table() {
    assert!(compares("x_switch_ranges", &["-O2"]) <= 3);
}

#[test]
fn a_switch_of_constants_is_a_load_and_a_character_class_a_few_ranges() {
    for flags in [["-O2"], ["-Os"]] {
        let (dense, ranges) = (compares("x_switch_dense", &flags), compares("x_switch_ranges", &flags));
        assert!(dense <= 4, "{flags:?}: x_switch_dense has {dense} compares");
        assert!(ranges <= 16, "{flags:?}: x_switch_ranges has {ranges} compares");
    }
}

/// Without `-ftree-switch-conversion` the same cases are a jump table, and
/// without `-fjump-tables` either a tree of compares.
#[test]
fn without_switch_conversion_the_constants_are_a_table_or_compares() {
    assert!(compares("x_switch_dense", &["-O2", "-fno-tree-switch-conversion"]) <= 4);
    assert!(compares("x_switch_dense", &["-O2", "-fno-tree-switch-conversion", "-fno-jump-tables"]) >= 11);
}

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/run/c/switch_jump_table.c");
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(flags)
        .args(["-march=i486", "-S", "-o", "a.s", source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// A dense switch whose cases each do their own work was a compare for each
/// case, in order; it is a jump through a table of code addresses, in 32-bit
/// code (a dword each, scaled) and in real mode (a word each, the index
/// doubled).
#[test]
fn a_dense_switch_is_a_jump_through_a_table() {
    let flat = listing(&["-m32", "-O2"]);
    assert!(flat.contains("jmp dword ptr $J"), "{flat}");
    assert!(flat.matches("    dd L0_").count() >= 10, "{flat}");
    let real = listing(&["-m16", "-O2"]);
    assert!(real.contains("jmp word ptr $J"), "{real}");
    assert!(real.matches("    dw L0_").count() >= 10, "{real}");
    // Tuned for size the table is still the smaller code.
    assert!(listing(&["-m32", "-Os"]).contains("jmp dword ptr $J"));
}

/// At -O0 the DAG still builds a jump table, as the pin does (`clang -O0
/// -target i486` emits `jmpl *%eax` for this switch); `-fno-jump-tables` stops
/// it.
#[test]
fn at_o0_a_dense_switch_is_a_table_unless_tables_are_off() {
    assert!(listing(&["-m32", "-O0"]).contains("$J"));
    assert!(!listing(&["-m32", "-O0", "-fno-jump-tables"]).contains("$J"));
}
