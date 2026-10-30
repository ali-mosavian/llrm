//! The relocatable object of four functions against the bytes of `llc
//! -mtriple=wasm32 -O0 -fast-isel=false -filetype=obj` (system LLVM 20.1.8,
//! `tests/golden/four.o`).

mod common;

use llrm_wasm::object::{encode_function, write};

#[test]
fn four_functions_are_llcs_object_byte_for_byte() {
    let functions: Vec<_> = common::four().iter().map(encode_function).collect();

    let golden = std::fs::read(format!("{}/tests/golden/four.o", env!("CARGO_MANIFEST_DIR"))).expect("golden object");

    let ours = write(&functions);
    if let Some(path) = std::env::var_os("WASM_DUMP") {
        std::fs::write(path, &ours).expect("dump");
    }
    assert_eq!(ours, golden);
}

/// No golden: the pin's -O0 order (RegStackify first) has no oracle here. The
/// dump is for `wasm-validate`, `wasm-ld` and `node` by hand.
#[test]
fn the_stackified_object_is_written() {
    let functions: Vec<_> = common::four_with(true).iter().map(encode_function).collect();
    let bytes = write(&functions);
    if let Some(path) = std::env::var_os("WASM_DUMP_STACKIFIED") {
        std::fs::write(path, &bytes).expect("dump");
    }
    assert!(
        bytes.len()
            < std::fs::metadata(format!("{}/tests/golden/four.o", env!("CARGO_MANIFEST_DIR"))).unwrap().len() as usize
    );
}
