//! The copies between x87 registers are propagated before the stack form.

mod common;

use std::path::Path;

/// nbody at -O2 under RFP kept every `fp := fp` RegAlloc made, and
/// the stackifier turned each into a `fld st(i)` and a pop: 101 floating
/// instructions where copy propagation leaves 95, 97 since the pin's order
/// (clang's whole listing has 108; the 200-trip position block is 34
/// instructions, clang's 36).
///
/// llrm-cc: the 97 was llrm-c's; the pin's `llc` on the same IR holds 130
/// (it stores with `fst`, this allocator copies with `fld st(i)`), so the bar
/// is the pin's count (pin-shape, on observable output).
#[test]
fn test_nbody_keeps_no_copy_between_x87_registers() {
    let scratch = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/nbody/nbody.c");
    let flags = ["-m32", "-O2", "-march=i486"];
    let listing = scratch.path().join("nbody.s");
    let output = common::cc::llrm_c().args(flags).args(["-S", "-o"]).arg(&listing).arg(&source).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let listing = std::fs::read_to_string(listing).unwrap();
    let floating =
        |text: &str| common::pin::instructions(text, None).iter().filter(|line| line.starts_with('f')).count();
    let pin = common::pin::listing(&source, &flags);
    let (ours, pins) = (floating(&listing), floating(&pin));
    assert!(pins > 0 && ours > 0, "premise: both listings hold x87 code: {pin}");
    assert!(ours <= pins, "{ours} floating instructions, the pin's llc has {pins}: {listing}");
}
