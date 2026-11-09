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

/// A crate-local `llc` (`crates/target/*/src/bin/llc.rs`) runs the whole
/// compile, so it declares the slab too: the five of them had not, and a new
/// target's would not either.
#[test]
fn every_crate_local_llc_uses_the_slab_allocator() {
    let targets = std::fs::read_dir(format!("{}/crates/target", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut seen = 0;
    for target in targets {
        let path = target.unwrap().path().join("src/bin/llc.rs");
        let Ok(source) = std::fs::read_to_string(&path) else { continue };
        seen += 1;
        assert!(
            source.contains("#[global_allocator]") && source.contains("llrm_alloc::Slab"),
            "{} does not use the slab allocator",
            path.display()
        );
    }
    assert!(seen >= 5, "the premise: {seen} crate-local llc binaries found");
}
