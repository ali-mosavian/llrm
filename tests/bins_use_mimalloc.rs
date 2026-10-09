//! glibc's malloc, free and memmove were a quarter of a compile's instructions;
//! mimalloc takes 7 to 10% of them. The binaries that compile say so.

#[test]
fn every_compiler_binary_uses_mimalloc() {
    for bin in ["llrm-c", "llrm-qb", "llrm-nib", "llrm-mir"] {
        let source = std::fs::read_to_string(format!("{}/src/bin/{bin}.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(
            source.contains("#[global_allocator]") && source.contains("mimalloc::MiMalloc"),
            "{bin} does not use mimalloc"
        );
    }
}
