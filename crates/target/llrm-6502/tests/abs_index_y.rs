//! A table access takes its index in `x` or in `y` (`abs,x`, `abs,y`), as
//! llvm-mos' `LDAAbsIdx`/`STAbsIdx` (index class `XY`) do. The index was always
//! copied to `x`, so a loop that kept the high byte of its 16-bit counter in
//! `x` parked it in a cell around the table access (`stx rc3` ... `lda rc3`)
//! and counted the high byte of the element in `y`: split_loops -2 B,
//! memmove_overlap -11 B (6502) with the index in `y`. The assertion is on the
//! index register of the loop's table accesses. Needs `llrm-cc` built
//! (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(
    source: &str,
    level: &str,
) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("abs-index-y");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(source);
    let out = dir.join(format!("{source}.{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-fno-inline-functions", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// `counts[j & 63]++` for `j += i`: the loop's element is read and written
/// through `y`, leaving `x` to the counter's high byte (the base used `x`).
#[test]
fn a_16_bit_table_element_is_indexed_by_y_where_x_holds_the_counter() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = assembly("counts_bump.c", level);
        let body = &text[text.find("\nbump_b2:").expect("loop")..];
        let body = body.split("\nbump_b3:").next().unwrap();
        assert!(body.contains("counts,"), "{level}: no indexed access (premise):\n{body}");
        assert!(!body.contains("counts,x"), "{level}: the element is indexed by x:\n{body}");
        assert!(body.contains("counts,y"), "{level}: the element is not indexed by y:\n{body}");
    }
}
