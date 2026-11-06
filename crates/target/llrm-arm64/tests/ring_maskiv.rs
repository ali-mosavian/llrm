//! ARM64-MASKIV: `total += buf[(i*5+3) & 1023]` recomputed `i*5+3` and the mask
//! every trip (8 instructions per trip against gcc's 5). A masked induction
//! variable `p = (p+5) & 1023` needs no multiply. Red until the LSR generator lands.

use std::process::Command;

fn loop_instructions() -> usize {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("t.s");
    let ir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/ring_maskiv.ll");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(ir)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let body = &asm[asm.find("_ring:").expect("the function is there")..];
    // the last block that branches to its own label is the summing loop
    let mut blocks: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in body.lines().skip(1) {
        if let Some(label) = line.strip_suffix(':') {
            blocks.push((label, Vec::new()));
        } else if let Some((_, lines)) = blocks.last_mut() {
            lines.push(line);
        }
    }
    let (_, lines) = blocks
        .iter()
        .rev()
        .find(|(label, lines)| lines.iter().any(|l| l.ends_with(*label)))
        .expect("a loop");
    lines.iter().filter(|l| l.starts_with('\t') && !l.starts_with("\t.")).count()

}

#[test]
fn a_masked_ring_index_costs_five_instructions_a_trip() {
    let n = loop_instructions();
    assert!(n <= 5, "{n} instructions per trip");
}
