//! `llrm-wasm-llc` runs under glibc's malloc unless it says otherwise: malloc,
//! free and realloc were 23% of a compile's instructions (arm64, queens -O3),
//! the pin's 10%. `tests/bins_use_the_slab.rs` at the root checks every
//! crate-local llc; this names this crate's.

#[test]
fn the_wasm_llc_uses_the_slab_allocator() {
    let source = std::fs::read_to_string(format!("{}/src/bin/llc.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(
        source.contains("#[global_allocator]") && source.contains("llrm_alloc::Slab"),
        "llrm-wasm-llc does not use the slab allocator"
    );
}
