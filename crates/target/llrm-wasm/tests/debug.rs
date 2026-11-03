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

/// The object of `fixture` (`-g`), linked, and what `tool` says of it.
fn linked(
    fixture: &str,
    tool: &[&str],
) -> String {
    linked_at(fixture, llrm_wasm::driver::Level::O0, tool)
}

fn linked_at(
    fixture: &str,
    level: llrm_wasm::driver::Level,
    tool: &[&str],
) -> String {
    let source =
        std::fs::read_to_string(format!("{}/tests/ir/{fixture}", env!("CARGO_MANIFEST_DIR"))).expect("fixture");
    let options = Options { level, ..Options::default() };
    let bytes = llrm_wasm::driver::object_g(&source, Width::W32, options, "d.c").expect("object");
    let dir = std::env::temp_dir().join(format!("wasm-g-{}-{}", std::process::id(), fixture));
    std::fs::create_dir_all(&dir).unwrap();
    let (object, linked) = (dir.join("d.o"), dir.join("l.wasm"));
    std::fs::write(&object, &bytes).unwrap();
    let run = |program: &str, args: &[&std::ffi::OsStr]| {
        let out = std::process::Command::new(program).args(args).output().expect(program);
        assert!(out.status.success(), "{program}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let os = std::ffi::OsStr::new;
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
    let mut args: Vec<&std::ffi::OsStr> = tool.iter().map(|one| os(one)).collect();
    args.push(linked.as_os_str());
    let said = run("llvm-dwarfdump-20", &args);
    if std::env::var_os("WASM_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    said
}

/// A linked `-g` object put every row at the code section's start, or none: the
/// line table is read back by `llvm-dwarfdump` against the code `wasm-objdump`
/// lists.
#[test]
fn a_linked_g_object_has_the_rows_of_its_source_lines() {
    let rows = linked("debug_lines.ll", &["--debug-line"]);
    let lines: Vec<u32> = rows
        .lines()
        .filter(|l| l.starts_with("0x") && !l.contains("end_sequence"))
        .filter_map(|l| l.split_whitespace().nth(1)?.parse().ok())
        .collect();
    // The scope line's row at the function's start, then one row where the
    // line changes (`DwarfDebug::beginInstruction`); the repeats were rows too.
    assert_eq!(lines, [1, 11, 12, 13, 14], "{rows}");
    let verified = linked("debug_lines.ll", &["--verify"]);
    assert!(verified.contains("No errors"), "{verified}");
    let info = linked("debug_lines.ll", &["--debug-info"]);
    assert!(info.contains("DW_AT_name\t(\"f\")"), "{info}");
}

/// `-O0` variables had no location: `x` and `y` were in the frame and the
/// debugger could not read them. Clang 20 writes them `DW_OP_fbreg +12` and
/// `+8` from a frame base that is a wasm local.
#[test]
fn an_o0_frame_variable_is_an_fbreg_from_a_wasm_local_frame_base() {
    let info = linked("debug_vars_o0.ll", &["--debug-info"]);
    assert!(info.contains("DW_AT_frame_base\t(DW_OP_WASM_location 0x0 "), "{info}");
    assert!(info.contains("DW_OP_fbreg +12"), "{info}");
    assert!(info.contains("DW_OP_fbreg +8"), "{info}");
    let verified = linked("debug_vars_o0.ll", &["--verify"]);
    assert!(verified.contains("No errors"), "{verified}");
}

/// Two functions with frame pointers in different locals shared the first
/// one's `DW_AT_frame_base`, so the second function's `fbreg` variables read
/// the wrong local. Clang writes each subprogram its own.
#[test]
fn each_function_states_its_own_frame_base() {
    let info = linked("debug_two_o0.ll", &["--debug-info"]);
    let bases: Vec<&str> = info.lines().filter(|l| l.contains("DW_AT_frame_base")).collect();
    assert_eq!(bases.len(), 2, "{info}");
    assert_ne!(bases[0], bases[1], "{info}");
    for want in ["fbreg +12", "fbreg +8", "fbreg +4", "fbreg +0"] {
        assert!(info.contains(want), "{want} missing:\n{info}");
    }
}

/// An `-O2` parameter had no location: its `#dbg_value` of the argument was
/// dropped, so the debugger showed `x` as optimized out. Clang 20 puts it in
/// its argument's local for the whole function.
#[test]
fn an_o2_parameter_is_its_argument_local() {
    let info = linked_at("debug_vars_o2.ll", llrm_wasm::driver::Level::O2, &["--debug-info"]);
    let at = info.find("DW_AT_name\t(\"x\")").unwrap_or_else(|| panic!("no x:\n{info}"));
    let location = info[at..].split("\n\n").next().unwrap();
    assert!(location.contains("DW_AT_location\t(DW_OP_WASM_location 0x0 0x0, DW_OP_stack_value)"), "{info}");
}
