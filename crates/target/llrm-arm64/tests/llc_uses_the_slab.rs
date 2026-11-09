//! `llrm-arm64-llc` runs the arm64 compile under glibc's malloc unless it says
//! otherwise: malloc, free and realloc were 23% of its instructions on queens
//! -O3 (16.7 M of 71.6 M; the pin's 10%), the root binaries' slab
//! (`llrm-alloc`) cuts that. `tests/bins_use_the_slab.rs` at the root checks
//! the root binaries; this checks the one this crate builds.

#[test]
fn the_arm64_llc_uses_the_slab_allocator() {
    let source = std::fs::read_to_string(format!("{}/src/bin/llc.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(
        source.contains("#[global_allocator]") && source.contains("llrm_alloc::Slab"),
        "llrm-arm64-llc does not use the slab allocator"
    );
}
