//! glibc's malloc, free and memmove were a quarter of a compile's instructions;
//! mimalloc took 7 to 10% of them and the size-class slab over it
//! (`llrm-alloc`) a further 4.6%. The binaries that compile say so.

#[test]
fn every_compiler_binary_uses_the_slab_allocator() {
    for bin in ["llrm-llc", "llrm-qb", "llrm-nib", "llrm-mir"] {
        let source = std::fs::read_to_string(format!("{}/src/bin/{bin}.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert!(
            source.contains("#[global_allocator]") && source.contains("llrm_alloc::Slab"),
            "{bin} does not use the slab allocator"
        );
    }
}
