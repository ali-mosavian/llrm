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
