//! `-g`: the DWARF sections of an object.

use llrm_wasm::Width;
use llrm_wasm::dag::{Options, compile_function};
use llrm_wasm::object::{DebugTarget, encode_function};

/// `.debug_line` carried no function relocation: its one sequence's address
/// was a bare 0, so a linked object put every row at the code section's start.
#[test]
fn the_line_program_addresses_its_function_by_a_relocation() {
    let source =
        std::fs::read_to_string(format!("{}/tests/ir/debug_lines.ll", env!("CARGO_MANIFEST_DIR"))).expect("fixture");
    let mut module = llrm_wasm::driver::module(&source, Width::W32).expect("module");
    llrm_mir::dbgimport::types_and_globals(&mut module);
    let f = compile_function(&module, "f", Width::W32, Options::default()).expect("compiles");
    let sections = llrm_wasm::debug::sections(&module, Width::W32, &[encode_function(&f)], "d.c").expect("sections");
    let line = sections.iter().find(|one| one.name == ".debug_line").expect(".debug_line");
    assert!(line.relocs.iter().any(|r| r.target == DebugTarget::Function("f".into())), "{:?}", line.relocs);
}

/// A linked `-g` object put every row at the code section's start, or none: the
/// line table is read back by `llvm-dwarfdump` against the code `wasm-objdump`
/// lists.
#[test]
fn a_linked_g_object_has_the_rows_of_its_source_lines() {
    let source =
        std::fs::read_to_string(format!("{}/tests/ir/debug_lines.ll", env!("CARGO_MANIFEST_DIR"))).expect("fixture");
    let bytes = llrm_wasm::driver::object_g(&source, Width::W32, Options::default(), "d.c").expect("object");
    let dir = std::env::temp_dir().join(format!("wasm-g-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (object, linked) = (dir.join("d.o"), dir.join("l.wasm"));
    std::fs::write(&object, &bytes).unwrap();
    let run = |program: &str, args: &[&std::ffi::OsStr]| {
        let out = std::process::Command::new(program).args(args).output().expect(program);
        assert!(out.status.success(), "{program}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let os = |s: &'static str| std::ffi::OsStr::new(s);
    run(
        "wasm-ld",
        &[
            os("--no-entry"),
            os("--export-all"),
            os("--allow-undefined"),
            os("-o"),
            linked.as_os_str(),
            object.as_os_str(),
        ],
    );
    let rows = run("llvm-dwarfdump-20", &[os("--debug-line"), linked.as_os_str()]);
    let lines: Vec<u32> = rows
        .lines()
        .filter(|l| l.starts_with("0x") && !l.contains("end_sequence"))
        .filter_map(|l| l.split_whitespace().nth(1)?.parse().ok())
        .collect();
    // The scope line's row at the function's start, then one row where the
    // line changes (`DwarfDebug::beginInstruction`); the repeats were rows too.
    assert_eq!(lines, [1, 11, 12, 13, 14], "{rows}");
    let verified = run("llvm-dwarfdump-20", &[os("--verify"), linked.as_os_str()]);
    assert!(verified.contains("No errors"), "{verified}");
    let info = run("llvm-dwarfdump-20", &[os("--debug-info"), linked.as_os_str()]);
    assert!(info.contains("DW_AT_name\t(\"f\")"), "{info}");
    if std::env::var_os("WASM_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}
