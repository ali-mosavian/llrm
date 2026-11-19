use llrm_z80::driver::Level;

/// `-Omax` was no level here: "unknown optimization level -Omax" (loop_fusion.c, `// flags: -Omax`, did not
/// build). It is the shared `Options::aggressive()` level, as for x86.
#[test]
fn omax_is_a_level() {
    assert_eq!(Level::parse("max"), Ok(Level::Omax));
    assert!(Level::parse("fast").is_err());
}
