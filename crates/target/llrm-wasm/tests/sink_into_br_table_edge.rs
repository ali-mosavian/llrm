use std::path::Path;
use std::process::Command;

/// The marker loop of switch_header_marker_loop: `112` is the phi input of the
/// case `194` of a switch that became a `BR_TABLE`. MachineSink split that edge
/// (the pin cannot: `analyzeBranch` fails on a `BR_TABLE`, so "Not legal to
/// break critical edge") and sank the constant into the new block, which cost
/// two nested blocks and a wider table: 261 B of code against the pin's 253 (+8
/// at -O2, -O3 and -Os).
#[test]
fn a_constant_stays_in_the_br_table_block_it_is_a_phi_input_of() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/sink_into_br_table_edge.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    let blocks = text.lines().filter(|line| line.trim() == "block").count();
    assert_eq!(blocks, 7, "{blocks} blocks:\n{text}");
}
