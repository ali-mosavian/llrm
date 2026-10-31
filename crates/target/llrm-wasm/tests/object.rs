//! The relocatable object of four functions against the bytes of `llc
//! -mtriple=wasm32 -O0 -fast-isel=false -filetype=obj` (system LLVM 20.1.8,
//! `tests/golden/four.o`).

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::object;

fn four(options: Options) -> Vec<u8> {
    let source =
        std::fs::read_to_string(format!("{}/tests/golden/four.ll", env!("CARGO_MANIFEST_DIR"))).expect("four.ll");
    object(&source, Width::W32, options).unwrap_or_else(|why| panic!("{why}"))
}

#[test]
fn four_functions_are_llcs_object_byte_for_byte() {
    let golden = std::fs::read(format!("{}/tests/golden/four.o", env!("CARGO_MANIFEST_DIR"))).expect("golden object");
    let ours = four(Options { stackify: false });
    if let Some(path) = std::env::var_os("WASM_DUMP") {
        std::fs::write(path, &ours).expect("dump");
    }
    assert_eq!(ours, golden);
}

/// No golden: the pin's -O0 order (RegStackify first) has no oracle here. The
/// dump is for `wasm-validate`, `wasm-ld` and `node` by hand.
#[test]
fn the_stackified_object_is_written() {
    let bytes = four(Options::default());
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

/// A global's address was a panic ("needs a relocation"): no data section,
/// segment info or address relocations.
#[test]
fn global_addresses_relocate_into_data_segments() {
    check("data_small");
}

/// Segments follow module order, symbols the order of first use then the unused
/// ones; sections by kind (`.rodata` for a constant, `.bss` for zeros), offsets
/// aligned per object.
#[test]
fn segments_symbols_and_alignment_follow_the_assembler() {
    check("data");
}

/// A private string, an internal variable, a pointer in data, an undefined
/// variable and a stack frame wrote no `.L` name, local binding, `reloc.DATA`,
/// undefined data symbol or `__stack_pointer` import: each was a panic.
#[test]
fn linkage_pointers_and_the_stack_pointer_import() {
    check("linkage");
}

/// A weak function and variable, a `linkonce_odr` function, an internal and a
/// global one: the symbol table wrote local binding for each of the weak ones,
/// so the strong `music_tick` of another object never replaced the
/// runtime's no-op and the tick played nothing (Gorillas' PLAY hung).
#[test]
fn weak_symbols_bind_weak() {
    check("weak");
}

/// The whole object of each `tests/ir` fixture. The encoder wrote only the
/// first block of a function, so any loop or branch lost its code after the
/// first `block`; an internal function was written as a global.
#[test]
fn every_fixture_object_is_the_pins() {
    let mut wrong = Vec::new();
    for name in [
        "internal",
        "obj_arith",
        "obj_branch",
        "obj_call",
        "obj_cmp",
        "obj_frame",
        "obj_floatcalls",
        "obj_ctors",
        "obj_funcdata",
        "obj_funcptr",
        "obj_global",
        "obj_indirect",
        "obj_mem",
        "obj_memops",
        "obj_phis",
        "obj_shapes",
        "obj_shifts",
        "obj_switch",
    ] {
        if std::panic::catch_unwind(|| check(name)).is_err() {
            wrong.push(name);
        }
    }
    assert!(wrong.is_empty(), "objects that differ from the pin's: {wrong:?}");
}

/// An `llvm.global_ctors` entry without a function made the object fail ("an
/// entry without a function"): the driver counted entries itself. The shared
/// reader drops it, as `emitXXStructorList` does.
#[test]
fn a_constructor_entry_without_a_function_is_dropped() {
    let text = "define void @init() {\n  ret void\n}\n\
        @llvm.global_ctors = appending global [2 x { i32, ptr, ptr }] \
        [{ i32, ptr, ptr } { i32 65535, ptr @init, ptr null }, { i32, ptr, ptr } { i32 65535, ptr null, ptr null }]\n";
    let bytes = llrm_wasm::driver::object(text, llrm_wasm::Width::W32, llrm_wasm::dag::Options::default());
    let bytes = bytes.unwrap_or_else(|why| panic!("{why}"));
    assert!(bytes.windows(4).any(|w| w == b"init"), "the init function is in the object");
}
