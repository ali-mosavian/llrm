//! The relocatable object of four functions against the bytes of `llc
//! -mtriple=wasm32 -O0 -fast-isel=false -filetype=obj` (system LLVM 20.1.8,
//! `tests/golden/four.o`).

mod common;

use llrm_wasm::object::{encode_function, write};

#[test]
fn four_functions_are_llcs_object_byte_for_byte() {
    let functions: Vec<_> = common::four().iter().map(encode_function).collect();

    let golden = std::fs::read(format!("{}/tests/golden/four.o", env!("CARGO_MANIFEST_DIR"))).expect("golden object");

    let ours = write(&functions, &[]);
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
    let bytes = write(&functions, &[]);
    if let Some(path) = std::env::var_os("WASM_DUMP_STACKIFIED") {
        std::fs::write(path, &bytes).expect("dump");
    }
    assert!(
        bytes.len()
            < std::fs::metadata(format!("{}/tests/golden/four.o", env!("CARGO_MANIFEST_DIR"))).unwrap().len() as usize
    );
}

/// `tests/obj/NAME.o` is `llc -mtriple=wasm32 -O0 -fast-isel=false
/// -filetype=obj` of `NAME.ll` at the pin.
fn check(name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/obj");
    let source = std::fs::read_to_string(dir.join(format!("{name}.ll"))).expect("the .ll");
    let golden = std::fs::read(dir.join(format!("{name}.o"))).expect("the .o");
    let got = llrm_wasm::driver::object(&source, llrm_wasm::Width::W32, llrm_wasm::dag::Options::default())
        .unwrap_or_else(|why| panic!("{name}: {why}"));
    if let Some(path) = std::env::var_os("WASM_DUMP_DIR") {
        std::fs::write(std::path::Path::new(&path).join(format!("{name}.o")), &got).expect("dump");
    }
    assert_eq!(got, golden, "{name}");
}

/// Calls to undefined functions wrote no import, `reloc.CODE` or symbol: the
/// object linked to nothing.
#[test]
fn calls_to_undefined_functions_import_and_relocate() {
    check("calls");
}

/// Imports, types and symbols follow the order the assembler first met each
/// name, not the declaration order.
#[test]
fn imports_follow_first_use() {
    check("calls_reordered");
}

/// A callee defined later is one symbol, defined, at its first use's position.
#[test]
fn a_callee_defined_later_is_not_an_import() {
    check("calls_defined");
}

/// A global's address was a panic ("needs a relocation"): no data section, segment info or address relocations.
#[test]
fn global_addresses_relocate_into_data_segments() {
    check("data_small");
}

/// Segments follow module order, symbols the order of first use then the unused ones; sections by kind
/// (`.rodata` for a constant, `.bss` for zeros), offsets aligned per object.
#[test]
fn segments_symbols_and_alignment_follow_the_assembler() {
    check("data");
}
