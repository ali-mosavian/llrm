//! At -Os a loop is not replaced by its closed form when the formula is more
//! code than the loop.

mod common;

/// A sum the loop optimizer writes in closed form, widened to i64 for the
/// product: 35 B as a loop on main, 107 B as the formula.
const SUM: &str = r#"long b(unsigned short n)
{
    long s = 0;
    unsigned short i;
    for (i = 0; i < n && i < 10; i++) s += i;
    return s;
}
"#;

fn exit_value_listing(level: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SUM).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m16", "-march=i386", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("a.s")).unwrap()
}

/// Whether a jump goes to a label already defined above it: a loop.
fn has_backward_branch(text: &str) -> bool {
    let mut defined = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(label) = line.strip_suffix(':') {
            defined.push(label.to_owned());
        } else if let Some(target) = line.strip_prefix('j').and_then(|rest| rest.split_whitespace().nth(1)) {
            if defined.iter().any(|label| label == target) {
                return true;
            }
        }
    }
    false
}

/// The closed form took 107 B where the loop is under half of that.
#[test]
fn at_os_the_sum_stays_a_loop() {
    let text = exit_value_listing("-Os");
    assert!(has_backward_branch(&text), "no loop at -Os:\n{text}");
}

/// The sum is a product and a shift (`imul`; the width of the shift is the
/// instruction selector's business), the loop gone.
#[test]
fn at_o2_the_sum_is_still_the_closed_form() {
    let text = exit_value_listing("-O2");
    assert!(!has_backward_branch(&text) && text.contains("imul"), "no closed form at -O2:\n{text}");
}
