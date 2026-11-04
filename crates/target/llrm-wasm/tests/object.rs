//! The relocatable object of four functions against the bytes of `llc
//! -mtriple=wasm32 -O0 -fast-isel=false -filetype=obj` (system LLVM 20.1.8,
//! `tests/golden/four.wasm`).

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, object};

fn four(options: Options) -> Vec<u8> {
    let source =
        std::fs::read_to_string(format!("{}/tests/golden/four.ll", env!("CARGO_MANIFEST_DIR"))).expect("four.ll");
    object(&source, Width::W32, options).unwrap_or_else(|why| panic!("{why}"))
}

#[test]
fn four_functions_are_llcs_object_byte_for_byte() {
    let golden =
        std::fs::read(format!("{}/tests/golden/four.wasm", env!("CARGO_MANIFEST_DIR"))).expect("golden object");
    let ours = four(Options { stackify: false, ..Options::default() });
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
            < std::fs::metadata(format!("{}/tests/golden/four.wasm", env!("CARGO_MANIFEST_DIR"))).unwrap().len()
                as usize
    );
}

/// `tests/obj/NAME.wasm` is `llc -mtriple=wasm32 -O0 -fast-isel=false
/// -filetype=obj` of `NAME.ll` at the pin.
fn check(name: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/obj");
    let source = std::fs::read_to_string(dir.join(format!("{name}.ll"))).expect("the .ll");
    let golden = std::fs::read(dir.join(format!("{name}.wasm"))).expect("the .wasm");
    let got = llrm_wasm::driver::object(&source, llrm_wasm::Width::W32, llrm_wasm::dag::Options::default())
        .unwrap_or_else(|why| panic!("{name}: {why}"));
    if let Some(path) = std::env::var_os("WASM_DUMP_DIR") {
        std::fs::write(std::path::Path::new(&path).join(format!("{name}.wasm")), &got).expect("dump");
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

/// `tests/obj/main_alias.wasm` is the pin's object less its `producers`
/// section (the pin's version string). A C `main(void)` is `__original_main`
/// behind a `main(i32, i32)` wrapper, with a hidden `__main_void` alias of the
/// body: the parser stopped at "expected a type, found `hidden`", so no C
/// program reached the wasm backend.
#[test]
fn a_hidden_main_and_its_alias_are_the_pins_object() {
    check("main_alias");
}

const ALIAS_CALL: &str = "@other = alias i32 (i32), ptr @impl
define i32 @impl(i32 %x) {
  %y = add i32 %x, 1
  ret i32 %y
}
define i32 @user(i32 %x) {
  %r = call i32 @other(i32 %x)
  ret i32 %r
}
";

/// DEVIATE W23: the call names the alias, and ours relocates against `impl`'s
/// symbol (0), the pin against `other`'s (2). At -O0 the callee is not inlined.
#[test]
fn a_call_naming_an_alias_relocates_against_its_aliasee() {
    let options = Options { level: Level::O0, ..Options::default() };
    let bytes = object(ALIAS_CALL, Width::W32, options).unwrap();
    let at = bytes.windows(10).position(|w| w == b"reloc.CODE").expect("a code relocation section") + 10;
    // section index, one relocation: type FUNCTION_INDEX_LEB (0), offset,
    // symbol
    assert_eq!(bytes[at + 1], 1, "one relocation");
    assert_eq!((bytes[at + 2], bytes[at + 4]), (0, 0), "a function-index relocation against symbol 0, `impl`");
    assert!(bytes.windows(5).any(|w| w == b"other"), "the alias is a symbol");
}

/// Debug sections were dropped: `.debug_line` and its `reloc..debug_line`
/// never reached the object, so DWARF could not be linked.
#[test]
fn a_debug_section_and_its_relocations_are_written() {
    use llrm_wasm::object::{DebugReloc, DebugSection, DebugTarget};
    let source =
        std::fs::read_to_string(format!("{}/tests/golden/four.ll", env!("CARGO_MANIFEST_DIR"))).expect("four.ll");
    let first = source
        .lines()
        .find_map(|l| l.strip_prefix("define ").and_then(|l| l.split('@').nth(1)).map(|l| l.split('(').next().unwrap()))
        .expect("a function");
    let debug = [
        DebugSection {
            name: ".debug_line".into(),
            bytes: vec![0; 16],
            relocs: vec![
                DebugReloc { offset: 4, target: DebugTarget::Function(first.into()), addend: 2 },
                DebugReloc { offset: 8, target: DebugTarget::Section(1), addend: 0 },
            ],
        },
        DebugSection { name: ".debug_str".into(), bytes: vec![b'x', 0], relocs: vec![] },
    ];
    let bytes = llrm_wasm::driver::object_with_debug(&source, Width::W32, Options::default(), &debug).expect("object");
    let dir = std::env::temp_dir().join(format!("wasm-debug-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("d.wasm");
    std::fs::write(&path, &bytes).unwrap();
    let dump = |flags: &[&str]| {
        let out = std::process::Command::new("wasm-objdump").args(flags).arg(&path).output().expect("wasm-objdump");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let headers = dump(&["-h"]);
    for section in [".debug_line", ".debug_str", "reloc..debug_line"] {
        assert!(headers.contains(section), "{section} missing:\n{headers}");
    }
    let relocs = dump(&["-x", "-r"]);
    assert!(relocs.contains("R_WASM_FUNCTION_OFFSET_I32"), "{relocs}");
    assert!(relocs.contains("R_WASM_SECTION_OFFSET_I32"), "{relocs}");
    let linked = std::process::Command::new("wasm-ld")
        .args(["--no-entry", "--export-all", "--allow-undefined", "-o"])
        .arg(dir.join("linked.wasm"))
        .arg(&path)
        .output();
    if let Ok(linked) = linked {
        assert!(linked.status.success(), "wasm-ld: {}", String::from_utf8_lossy(&linked.stderr));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
