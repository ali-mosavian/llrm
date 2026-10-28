//! `-g` as DWARF in an ELF object: llvm-dwarfdump accepts every bench C
//! program's, and gdb, driven by a script, stops where it is told in a linked
//! and running program and reads its values.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A test that cannot run says so on stderr, and fails where
/// `LLRM_REQUIRE_DWARF` is set, so a gate that has the tools cannot pass by
/// skipping.
fn skipped(reason: &str) {
    // Written to the stderr itself, which the harness does not capture: seen
    // when the test passes.
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("SKIPPED: {reason}\n").as_bytes());
    assert!(!llrm_codegen::support::env_set("LLRM_REQUIRE_DWARF"), "LLRM_REQUIRE_DWARF is set, and: {reason}");
}

/// `skipped`, for a tool that has its own requirement variable: the gate unsets
/// `var` where it says the tool is not on the host (a runner has no Open Watcom
/// install), so the test names what it lacked and skips; a host that has it
/// sets `var` and fails without it.
fn skipped_for(
    var: &str,
    reason: &str,
) {
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("SKIPPED: {reason}\n").as_bytes());
    assert!(std::env::var_os(var).is_none(), "{var} is set, and: {reason}");
}

/// Open Watcom's wdump: `WDUMP`, else the development hosts' install.
fn wdump() -> Option<PathBuf> {
    std::env::var_os("WDUMP")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join("dos/devtools/dev/c/watcom/binl/wdump")))
        .filter(|path| path.exists())
}

fn tool(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.push("/usr/lib/llvm-20/bin".into());
    dirs.iter().map(|dir| dir.join(name)).find(|path| path.exists())
}

fn dwarfdump() -> Option<PathBuf> {
    tool("llvm-dwarfdump").or_else(|| tool("llvm-dwarfdump-20"))
}

fn compile(
    source: &Path,
    arguments: &[&str],
    out: &Path,
) -> std::process::Output {
    common::cc::llrm_c().args(arguments).arg(source).arg("-o").arg(out).output().unwrap()
}

fn bench_programs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench");
    let mut found: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(|one| {
            let dir = one.unwrap().path();
            let source = dir.join(format!("{}.c", dir.file_name()?.to_str()?));
            source.exists().then_some(source)
        })
        .collect();
    found.sort();
    found
}

/// llvm-dwarfdump --verify finds no error and no warning in the DWARF 4 and 5
/// of every bench C program at -O0 and -O2, and the unit has code, functions
/// and lines: a verifier passes an empty one.
#[test]
fn dwarfdump_verifies_every_bench_program_at_both_ends_of_the_optimiser() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let programs = bench_programs();
    assert!(programs.len() > 20, "{programs:?}");
    for source in &programs {
        for level in ["-O0", "-O2"] {
            for flag in ["-gdwarf-5", "-gdwarf-4"] {
                let object = scratch.path().join("x.o");
                let made = compile(source, &["-m32", level, "-fobject-format=elf", flag], &object);
                assert!(
                    made.status.success(),
                    "{} {level} {flag}: {}",
                    source.display(),
                    String::from_utf8_lossy(&made.stderr)
                );
                let said = Command::new(&dump).arg("--verify").arg(&object).output().unwrap();
                let text =
                    format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
                assert!(
                    said.status.success() && text.trim_end().ends_with("No errors.") && !text.contains("warning"),
                    "{} {level} {flag}:\n{text}",
                    source.display()
                );
                let shown = Command::new(&dump).args(["--debug-info", "--debug-line"]).arg(&object).output().unwrap();
                let shown = String::from_utf8_lossy(&shown.stdout);
                assert!(
                    shown.contains("DW_TAG_subprogram") && shown.contains("DW_AT_low_pc") && shown.contains("is_stmt"),
                    "{} {level} {flag}: an empty unit",
                    source.display()
                );
                // Every function has call frame information: one the code could
                // not be followed through has none.
                let frames = Command::new(&dump).arg("--debug-frame").arg(&object).output().unwrap();
                let (fdes, functions) = (
                    String::from_utf8_lossy(&frames.stdout).matches(" FDE ").count(),
                    // An inlined function's abstract subprogram has no code.
                    shown.matches("DW_TAG_subprogram").count() - shown.matches("DW_AT_inline\t").count(),
                );
                assert_eq!(fdes, functions, "{} {level} {flag}: frame rules for every function", source.display());
            }
        }
    }
}

/// gdb on a -m32 ELF program linked by ld and run under Linux: it stops after
/// the prologue of a function at the line the body starts on, and prints a
/// parameter, a local, a struct field through a pointer, the function's type
/// and the value it returns.
#[test]
fn gdb_stops_at_a_line_and_reads_a_parameter_a_local_a_struct_field_and_the_return_type() {
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        skipped("needs gdb, GNU ld and as");
        return;
    };
    if cfg!(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "x86")))) {
        skipped("needs an x86 Linux host that runs i386 programs");
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let (start, object, program) =
        (scratch.path().join("start.o"), scratch.path().join("gdb.o"), scratch.path().join("gdb"));
    let made =
        Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("gdb.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked =
        Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(12) {
        skipped("this host does not run i386 programs");
        return;
    }
    let script = scratch.path().join("script.gdb");
    std::fs::write(
        &script,
        "set width 0\nbreak add\nrun\nprint a\nprint p->y\nprint *p\nnext\nprint l\nptype add\nfinish\n",
    )
    .unwrap();
    let said = Command::new(gdb)
        .args(["-batch", "-nx", "-x"])
        .arg(&script)
        .arg(&program)
        .current_dir(&fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    // The stop is the first statement of the body, not the one after it.
    assert!(
        text.contains("add (a=1, p=0x") && text.contains("gdb.c:7") && text.contains("int l = a + p->x;"),
        "{text}"
    );
    for expected in [
        "$1 = 1",
        "$2 = 4",
        "$3 = {x = 3, y = 4}",
        "$4 = 4",
        "type = int (int, struct pt *)",
        "Value returned is $5 = 12",
    ] {
        assert!(text.contains(expected), "no {expected:?} in:\n{text}");
    }
}

/// `-g` takes the object format's own debug format; a flavor the format cannot
/// carry is an error that says so, never an object with another flavor in it.
#[test]
fn the_flavor_asked_for_is_the_formats_or_an_error() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/gdb.c");
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("x.o");
    let refused = |arguments: &[&str]| {
        let made = compile(&source, arguments, &object);
        assert!(!made.status.success(), "{arguments:?} was written");
        String::from_utf8_lossy(&made.stderr).into_owned()
    };
    assert!(refused(&["-m32", "-gcodeview", "-fobject-format=elf"]).contains("cannot carry CodeView"));
    assert!(refused(&["-m32", "-gtd", "-fobject-format=elf"]).contains("Turbo Debugger"));
    assert!(refused(&["-m32", "-gdwarf-1", "-fobject-format=elf"]).contains("unrecognized"));
    // Borland's records are 16-bit.
    assert!(refused(&["-m32", "-gtd"]).contains("16-bit"));
    let made = compile(&source, &["-m16", "-gtd"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let records = llrm_codegen::objectfile::omf::parse(&std::fs::read(&object).unwrap()).unwrap();
    let classes: BTreeSet<u8> = records
        .iter()
        .filter(|one| one.r#type == llrm_codegen::objectfile::omf::COMENT)
        .filter_map(|one| one.body.get(1).copied())
        .collect();
    assert!(
        classes.contains(&0xE3) && classes.contains(&0xE5) && !classes.contains(&0xA1),
        "-gtd on OMF is Borland's: {classes:x?}"
    );
    let made = compile(&source, &["-m32", "-g", "-fobject-format=elf"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    assert!(std::fs::read(&object).unwrap().windows(11).any(|one| one == b".debug_info"), "-g on ELF is DWARF");
    let made = compile(&source, &["-m32", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let bytes = std::fs::read(&object).unwrap();
    assert!(
        bytes.windows(6).any(|one| one == b"DEBSYM") && !bytes.windows(11).any(|one| one == b".debug_info"),
        "-g on OMF is CodeView"
    );
}

/// The values gdb reads of a program's variables at a line it stops on, as
/// `-O0` and `-O2` leave them; with `hits` over one, at each of the first
/// that many times it stops there (an unrolled loop's first stop on its line
/// may be any iteration's).
fn stopped_at(
    gdb: &Path,
    program: &Path,
    line: u32,
    fixtures: &Path,
    hits: usize,
) -> Option<(u32, Vec<String>)> {
    let mut args = vec![
        "-batch".to_owned(),
        "-nx".to_owned(),
        "-ex".to_owned(),
        format!("{} observed.c:{line}", if hits > 1 { "break" } else { "tbreak" }),
        "-ex".to_owned(),
        "run".to_owned(),
    ];
    for hit in 0..hits {
        if hit > 0 {
            args.extend(["-ex".to_owned(), "continue".to_owned()]);
        }
        args.extend(["-ex".to_owned(), "info locals".to_owned(), "-ex".to_owned(), "info args".to_owned()]);
    }
    let said = Command::new(gdb).args(args).arg(program).current_dir(fixtures).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let at =
        text.split("observed.c:").nth(1)?.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()?;
    let values = text
        .lines()
        .filter(|one| {
            one.split_once(" = ").is_some_and(|(name, _)| name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        })
        .map(str::to_owned)
        .collect();
    Some((at, values))
}

/// What gdb reads of a program's variables at -O2 is never what -O0 does not:
/// where -O2 says a value it is the value -O0 has at that line or at the next
/// one it stops at (a line's first instruction may come after its first
/// assignment at -O2), and where it says none (`<optimized out>`) it says no
/// wrong one. Before `-g` described values over the code, a store to a declared
/// variable stayed where the source wrote it for gdb to read the cell, and -g
/// changed the code at -O2 by every one of those stores; a value the optimiser
/// keeps nowhere is now said to be nowhere.
#[test]
#[ignore = "D-row: expression-carrying debug records + salvageDebugInfo"]
fn a_variable_reads_the_same_at_o2_as_at_o0_on_every_line_both_stop_at() {
    reads_the_same_as_at_o0("-O2");
}

/// -O3 unrolls the loops of `observed.c` whole: the pieces of `v` the stores
/// said stood after the code that reads them once the blocks were merged, and
/// gdb read {0, 0, 0, 0} where -O0 has {0, 3, 6, 9} on line 30.
#[test]
#[ignore = "D-row: expression-carrying debug records + salvageDebugInfo"]
fn a_variable_reads_the_same_at_o3_as_at_o0_on_every_line_both_stop_at() {
    reads_the_same_as_at_o0("-O3");
}

fn reads_the_same_as_at_o0(optimised: &str) {
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        skipped("needs gdb, GNU ld and as");
        return;
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let start = scratch.path().join("start.o");
    let made =
        Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let mut programs = Vec::new();
    for level in ["-O0", optimised] {
        let (object, program) =
            (scratch.path().join(format!("observed{level}.o")), scratch.path().join(format!("observed{level}")));
        let made = compile(&fixtures.join("observed.c"), &["-m32", level, "-fobject-format=elf", "-g"], &object);
        assert!(made.status.success(), "{level}: {}", String::from_utf8_lossy(&made.stderr));
        let linked =
            Command::new(&ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        programs.push(program);
    }
    if Command::new(&programs[0]).output().ok().and_then(|ran| ran.status.code()) != Some(73) {
        skipped("this host does not run i386 programs");
        return;
    }
    assert_eq!(
        Command::new(&programs[1]).output().unwrap().status.code(),
        Some(73),
        "{optimised} computes what -O0 does"
    );
    let mut slow_stops = Vec::new();
    let mut fast_stops = Vec::new();
    for line in 10..=48 {
        let (Some(slow), Some(fast)) =
            (stopped_at(&gdb, &programs[0], line, &fixtures, 6), stopped_at(&gdb, &programs[1], line, &fixtures, 1))
        else {
            continue;
        };
        // A line with no code stops at the next one that has some: compare
        // where both stopped alike.
        if slow.0 == fast.0 {
            slow_stops.push(slow);
            fast_stops.push(fast);
        }
    }
    let value = |stop: &(u32, Vec<String>), name: &str| -> Vec<String> {
        stop.1.iter().filter_map(|one| one.strip_prefix(&format!("{name} = ")).map(str::to_owned)).collect()
    };
    let (mut compared, mut wrong) = (0, Vec::new());
    for (at, fast) in fast_stops.iter().enumerate() {
        for one in &fast.1 {
            let Some((name, said)) = one.split_once(" = ") else { continue };
            if said.contains("<optimized out>") {
                continue;
            }
            let allowed = [
                value(&slow_stops[at], name),
                slow_stops.get(at + 1).map(|next| value(next, name)).unwrap_or_default(),
            ];
            if allowed.iter().flatten().any(|slow| slow == said) {
                compared += 1;
            } else {
                wrong.push(format!("line {}: {name} is {said} at {optimised}, and {allowed:?} at -O0", fast.0));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(compared >= COMPARED_FLOOR, "only {compared} values were compared: -O2 says more of them than that");
}

/// `mix` of observed.c at -O2 keeps its parameter `n` in a register to its last
/// use, yet every record of `n` had drifted past that use as the blocks merged
/// and gdb read `n` as `<optimized out>` at every line. LLVM says a parameter
/// whose records name its argument from the entry (`EmitFuncArgumentDbgValue`).
#[test]
fn a_parameter_the_optimiser_keeps_in_a_register_has_a_location_at_o2() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("observed.o");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/observed.c");
    let made = compile(&source, &["-m32", "-O2", "-gdwarf", "-fobject-format=elf"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let said = Command::new(dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let n = text.split("DW_AT_name\t(\"n\")").nth(1).expect("n is described");
    let n = n.split("DW_TAG_").next().unwrap();
    assert!(n.contains("DW_AT_location") && n.contains("DW_OP_reg"), "{n}");
}

/// How many (line, variable) values of `observed.c` -O2 says, and -O0 has: the
/// pin's own count, clang-20 `-m32 -O2 -g` on this fixture read the same way
/// (18 compared, none wrong, 14 `<optimized out>`). llrm says 2: its records
/// carry no expression, so a folded value is lost
/// (docs/working/above-main-rows.md, AM-DBG-1).
const COMPARED_FLOOR: usize = 18;

/// `struct node { struct node *next; int v; }`: a struct that holds a pointer
/// to itself kept only `v` in the debug information, whichever format wrote it:
/// asked for while it was being built, it answered "none" and the member naming
/// it was dropped. gdb follows the list; CodeView keeps the field.
#[test]
fn a_struct_that_names_itself_keeps_the_member_that_does_in_both_formats() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    // CodeView, in an OMF object.
    let object = scratch.path().join("list.obj");
    let made = compile(&fixtures.join("list.c"), &["-m16", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let records = llrm_codegen::objectfile::omf::parse(&std::fs::read(&object).unwrap()).unwrap();
    let shape = llrm_codegen::objectfile::cv4info::shape(&records);
    let head = shape.iter().find(|one| one.starts_with("DATA head")).unwrap_or_else(|| panic!("{shape:#?}"));
    assert!(head.contains("next +0") && head.contains("v +2"), "{head}");
    // DWARF, in an ELF object, and read by gdb.
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        skipped("needs gdb, GNU ld and as");
        return;
    };
    let (start, object, program) =
        (scratch.path().join("start.o"), scratch.path().join("list.o"), scratch.path().join("list"));
    let made =
        Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("list.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked =
        Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(10) {
        skipped("this host does not run i386 programs");
        return;
    }
    let said = Command::new(gdb)
        .args([
            "-batch",
            "-nx",
            "-ex",
            "break sum",
            "-ex",
            "run",
            "-ex",
            "print *n",
            "-ex",
            "print n->next->v",
            "-ex",
            "print *n->next",
        ])
        .arg(&program)
        .current_dir(&fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    assert!(text.contains("$1 = {next = 0x") && text.contains(", v = 3}"), "{text}");
    assert!(text.contains("$2 = 7") && text.contains("$3 = {next = 0x0, v = 7}"), "{text}");
}

/// A 64-bit integer was no type of the debug model (CodeView 4 had none) and
/// every variable of one was dropped before any format saw it: `sum`'s `a`, `t`
/// and `u` and the global `total` were in no DWARF. They are types
/// now; gdb reads their values past 32 bits, and CodeView, which has no record
/// for one, still writes the object and what is beside them.
#[test]
fn a_64_bit_variable_is_in_dwarf_with_its_value_and_codeview_still_writes() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    // CodeView: the object is written, and `b`, an int, is in it beside what is
    // left out.
    let object = scratch.path().join("wide.obj");
    let made = compile(&fixtures.join("wide.c"), &["-m32", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    // DWARF, read by gdb.
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        skipped("needs gdb, GNU ld and as");
        return;
    };
    let (start, object, program) =
        (scratch.path().join("start.o"), scratch.path().join("wide.o"), scratch.path().join("wide"));
    let made =
        Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("wide.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked =
        Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(11) {
        skipped("this host does not run i386 programs");
        return;
    }
    let said = Command::new(gdb)
        .args([
            "-batch",
            "-nx",
            "-ex",
            "break sum",
            "-ex",
            "run",
            "-ex",
            "print a",
            "-ex",
            "print b",
            "-ex",
            "print total",
            "-ex",
            "next",
            "-ex",
            "next",
            "-ex",
            "print t",
            "-ex",
            "print u",
            "-ex",
            "ptype sum",
        ])
        .arg(&program)
        .current_dir(&fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    for expected in [
        "$1 = 5000000000",
        "$2 = 3",
        "$3 = 4294967297",
        "$4 = 5000000003",
        "$5 = 7",
        "type = long long (long long, int)",
    ] {
        assert!(text.contains(expected), "no {expected:?} in:\n{text}");
    }
}

/// `sum(long long a, int b)` listed `b` before `a`: `a` arrives in two
/// registers, so it was located after the others from its declared cell, and
/// gdb showed `sum (b=3, a=5000000000)` and `long long (int, long long)`.
#[test]
fn a_parameter_in_two_registers_keeps_its_place_among_the_parameters() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("wide.o");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/wide.c");
    let made = compile(&source, &["-m32", "-O0", "-gdwarf", "-fobject-format=elf"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let said = Command::new(dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let sum = text.split("DW_AT_name\t(\"sum\")").nth(1).expect("sum is described");
    let names: Vec<&str> = sum
        .split("DW_TAG_formal_parameter")
        .skip(1)
        .take(2)
        .filter_map(|one| one.split("DW_AT_name\t(\"").nth(1)?.split('"').next())
        .collect();
    assert_eq!(names, ["a", "b"], "{text}");
}

/// A function with no variable has no frame register, and its stack pointer
/// moves with each argument it pushes; the callee that pops them (`ret 8`)
/// moves it back with no instruction to say so. gdb walks out of the callee and
/// through that function to `main` only with call frame information: without it
/// the backtrace stopped at `middle`, one frame short.
#[test]
fn gdb_backtraces_through_a_function_with_no_frame_register_at_o2() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        skipped("needs gdb, GNU ld and as");
        return;
    };
    let (start, object, program) =
        (scratch.path().join("start.o"), scratch.path().join("frames.o"), scratch.path().join("frames"));
    let made =
        Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("frames.c"), &["-m32", "-O2", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked =
        Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(48) {
        skipped("this host does not run i386 programs");
        return;
    }
    let said = Command::new(gdb)
        .args(["-batch", "-nx", "-ex", "break leaf", "-ex", "run", "-ex", "bt", "-ex", "continue", "-ex", "bt"])
        .arg(&program)
        .current_dir(&fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let frames: Vec<&str> = text.lines().filter(|line| line.starts_with('#')).collect();
    // The second call is entered after the first one popped its arguments: its
    // frame rule depends on that.
    assert!(
        frames.len() == 6
            && frames[0].contains("leaf (a=1, b=2, c=3, d=4, e=5, f=6)")
            && frames[3].contains("leaf (a=2, b=3, c=4, d=5, e=6, f=7)"),
        "{text}"
    );
    for at in [1, 4] {
        assert!(frames[at].contains("in middle ()") && frames[at + 1].contains("in main ()"), "{text}");
    }
}

/// A parameter arrives in a register and the function stores it into its frame
/// cell a few instructions in; until then the cell holds nothing and the frame
/// register is the caller's. gdb stopped at the first instruction of `add` read
/// `a` from the cell (garbage) where it is in `eax`: the location is the
/// register until the store and the cell after it.
#[test]
fn gdb_reads_a_parameter_at_the_first_instruction_from_the_register_it_arrived_in() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        skipped("needs gdb, GNU ld and as");
        return;
    };
    let (start, object, program) =
        (scratch.path().join("start.o"), scratch.path().join("gdb.o"), scratch.path().join("gdb"));
    let made =
        Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("gdb.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked =
        Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(12) {
        skipped("this host does not run i386 programs");
        return;
    }
    let said = Command::new(gdb)
        .args([
            "-batch",
            "-nx",
            "-ex",
            "break *add",
            "-ex",
            "run",
            "-ex",
            "print a",
            "-ex",
            "print *p",
            "-ex",
            "stepi",
            "-ex",
            "stepi",
            "-ex",
            "stepi",
            "-ex",
            "stepi",
            "-ex",
            "stepi",
            "-ex",
            "print a",
        ])
        .arg(&program)
        .current_dir(&fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    assert!(text.contains("$1 = 1") && text.contains("$2 = {x = 3, y = 4}"), "at the entry:\n{text}");
    assert!(text.contains("$3 = 1"), "after the store, from the cell:\n{text}");
}

/// `-g` kept the frame register for every function with a variable: `push ebp;
/// mov ebp, esp` and every cell through `ebp`, where the same build without
/// `-g` addressed them through `esp`. DWARF finds a cell from the canonical
/// frame address, so the code is the code.
#[test]
fn an_elf_function_with_variables_has_the_code_it_has_without_g() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let assembly = |source: &Path, flags: &[&str]| {
        let out = scratch.path().join("x.s");
        let made = compile(source, &[&["-m32", "-O0", "-fobject-format=elf", "-S"], flags].concat(), &out);
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        std::fs::read_to_string(&out).unwrap()
    };
    let gdb = root.join("tests/fixtures/dwarf/gdb.c");
    assert!(!assembly(&gdb, &[]).contains("ebp"), "the instrument: without -g the function keeps no frame register");
    // `crc` bumps a local in place (`add dword ptr [ebp-n], 1`), which `-g`
    // split into a load, an add and a store while it marked the store
    // volatile for the optimiser, which -O0 does not run.
    for source in [gdb, root.join("bench/crc/crc.c")] {
        assert_eq!(assembly(&source, &[]), assembly(&source, &["-g"]), "{}", source.display());
    }
}

/// The targets and levels whose bench programs' code `-g` must leave alone, all
/// of them: DWARF on ELF at every level, and the other formats (CodeView on
/// COFF and OMF, 16-bit OMF) at the two ends of the optimiser.
const IDENTICAL: &[(&str, &str, &[&str])] = &[
    ("-m32", "elf", &["-O0", "-O1", "-O2", "-Os"]),
    ("-m32", "coff", &["-O0", "-O2"]),
    ("-m32", "omf", &["-O0", "-O2"]),
    ("-m16", "omf", &["-O0", "-O2"]),
];

/// `-g` changed the code of every bench program: a frame register kept for its
/// variables, a volatile on their stores, an entry store for a register
/// parameter, a global kept. No bench program's assembly differs with and
/// without `-g`.
#[test]
fn g_leaves_the_code_of_the_bench_programs_alone() {
    let scratch = tempfile::tempdir().unwrap();
    let programs = bench_programs();
    for &(machine, format, levels) in IDENTICAL {
        for &level in levels {
            let format_flag = format!("-fobject-format={format}");
            let assembly = |source: &Path, debug: bool| {
                let out = scratch.path().join("x.s");
                let mut flags = vec![machine, format_flag.as_str(), level, "-S"];
                if debug {
                    flags.push("-g");
                }
                let made = compile(source, &flags, &out);
                assert!(made.status.success(), "{}: {}", source.display(), String::from_utf8_lossy(&made.stderr));
                std::fs::read_to_string(&out).unwrap()
            };
            let differing: Vec<String> = programs
                .iter()
                .filter(|source| assembly(source, false) != assembly(source, true))
                .map(|source| source.file_name().unwrap().to_string_lossy().into_owned())
                .collect();
            assert!(differing.is_empty(), "{machine} {format} {level}: -g changed the code of {differing:?}");
        }
    }
}

/// A string literal's label was numbered by the front end's own handle, which
/// `-g` moves: two literals passed to a call were `L_b222` and `L_b232` without
/// `-g` and `L_b245` and `L_b255` with it, and the listings differed by the
/// names.
#[test]
fn g_leaves_the_labels_of_string_literals_alone() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("literals.c");
    std::fs::write(&source, "extern int open_it(const char *name, int mode);\nint main(void) { return open_it(\"IN\", 0) + open_it(\"OUT\", 1); }\n").unwrap();
    for machine in ["-m32", "-m16"] {
        let assembly = |debug: bool| {
            let out = scratch.path().join("x.s");
            let flags: Vec<&str> = [machine, "-O0", "-S"].into_iter().chain(debug.then_some("-g")).collect();
            let made = compile(&source, &flags, &out);
            assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
            std::fs::read_to_string(&out).unwrap()
        };
        assert_eq!(assembly(false), assembly(true), "{machine}: -g changed the listing");
    }
}

/// A global nothing reads was kept in the data when `-g` described it, "for a
/// debugger to read at any time", where the build without `-g` drops it: the
/// object differed by the global, and its neighbours in the layout by the room
/// it took. A debugger is told of what the program keeps, and of nothing else.
#[test]
fn a_global_the_optimiser_drops_is_dropped_with_g_too() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("unused.c");
    std::fs::write(
        &source,
        "static int unused[4] = { 1, 2, 3, 4 };\nint kept[2] = { 5, 6 };\nint main(void) { return kept[1]; }\n",
    )
    .unwrap();
    let assembly = |debug: bool| {
        let out = scratch.path().join("x.s");
        let flags: Vec<&str> =
            ["-m32", "-O2", "-fobject-format=elf", "-S"].into_iter().chain(debug.then_some("-g")).collect();
        let made = compile(&source, &flags, &out);
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        std::fs::read_to_string(&out).unwrap()
    };
    let plain = assembly(false);
    assert!(!plain.contains("unused"), "the instrument: without -g the unused static is dropped\n{plain}");
    assert_eq!(plain, assembly(true));
}

/// The same for the BASIC and Nib bench programs, whose compilers share the
/// backend.
#[test]
fn g_leaves_the_code_of_the_basic_and_nib_bench_programs_alone() {
    let scratch = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench");
    let qb = PathBuf::from(env!("CARGO_BIN_EXE_llrm-qb"));
    let nib = qb.parent().unwrap().join("llrm-nib");
    let mut differing = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let directory = entry.unwrap().path();
        for (compiler, extension) in [(&qb, "bas"), (&nib, "nib")] {
            let source = directory.join(format!("{}.{extension}", directory.file_name().unwrap().to_string_lossy()));
            if !source.exists() {
                continue;
            }
            for level in ["-O0", "-O2"] {
                let assembly = |debug: bool| {
                    let out = scratch.path().join("x.s");
                    let flags: Vec<&str> = [level, "-S"].into_iter().chain(debug.then_some("-g")).collect();
                    let made = Command::new(compiler).args(&flags).arg(&source).arg("-o").arg(&out).output().unwrap();
                    made.status.success().then(|| std::fs::read_to_string(&out).unwrap())
                };
                if assembly(false) != assembly(true) {
                    differing.push(format!("{} {level}", source.display()));
                }
            }
        }
    }
    assert!(differing.is_empty(), "-g changed the code of {differing:?}");
}

/// The front end lists, for `-g`, every extern a header declares, used or not;
/// each was an `extern` in the object (a symbol it asks the linker for), so a
/// program built with `-g` differed from one built without by the symbols it
/// asked for. gcc's undefined symbols are the ones the code names, whatever
/// `-g` describes.
#[test]
fn an_extern_nothing_names_is_asked_of_no_linker_with_g_or_without() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("ext.c");
    std::fs::write(
        &source,
        "extern int unused_one;\nextern char unused_two[4];\nextern int used;\nint main(void) { return used; }\n",
    )
    .unwrap();
    for machine in ["-m16", "-m32"] {
        let assembly = |debug: bool| {
            let out = scratch.path().join("x.s");
            let flags: Vec<&str> =
                [machine, "-fobject-format=omf", "-O2", "-S"].into_iter().chain(debug.then_some("-g")).collect();
            let made = compile(&source, &flags, &out);
            assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
            std::fs::read_to_string(&out).unwrap()
        };
        let plain = assembly(false);
        assert!(
            plain.contains("used") && !plain.contains("unused"),
            "the instrument: without -g only the used extern is declared\n{plain}"
        );
        assert_eq!(plain, assembly(true), "{machine}");
    }
}

/// `llrm-nib -m32 -fobject-format=elf -g` left out every local that lives in
/// a frame cell (`sum`, `total` of tests/fixtures/matrix/known.nib): the
/// driver's options kept the default object format (OMF), whose writer wants
/// a frame register, so the ELF writer was handed the frameless function's
/// cells as ones to drop.
#[test]
fn nib_in_elf_keeps_the_locals_that_live_in_frame_cells() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("known.o");
    let nib = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-nib");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/known.nib");
    let made = Command::new(nib)
        .args(["-m32", "-O0", "-g", "-fobject-format=elf"])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let said = Command::new(dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout);
    for name in ["sum", "total"] {
        assert!(text.contains(&format!("DW_AT_name\t(\"{name}\")")), "no {name}:\n{text}");
    }
}

/// CodeView 4 in a 32-bit OMF object left out every local of a function that
/// keeps no frame register (`sum` of tests/fixtures/matrix/known.c): its
/// BP-relative record has no EBP to be relative to, and wdump/jwlink images
/// of -m32 programs showed no locals. The stack pointer is the same distance
/// from the frame address over the whole body, so a register-relative record
/// names the cell.
#[test]
fn cv4_names_a_local_of_a_function_without_a_frame_register_off_the_stack_pointer() {
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("known.obj");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/known.c");
    let made = compile(&source, &["-m32", "-O0", "-g", "-fobject-format=omf"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let records = llrm_codegen::objectfile::omf::parse(&std::fs::read(&object).unwrap()).unwrap();
    let shape = llrm_codegen::objectfile::cv4info::shape(&records);
    assert!(shape.iter().any(|one| one.starts_with("REGREL add.sum:")), "{shape:#?}");
}

/// DWARF in an OMF object, linked by jwlink into an LE image: the sections
/// ride in segments of the class DWARF, as Open Watcom's `-hd` writes them, and
/// the linker's "flat addresses" directive makes their addresses the image's.
/// Without the directive a variable's address was its offset in its segment
/// (`counter` at 0, not 0x10000). llvm-dwarfdump reads the sections that jwlink
/// puts at the end of the image.
#[test]
fn dwarf_in_an_omf_object_links_into_an_le_image_that_llvm_reads() {
    let (Some(dwarfdump), jwlink) =
        (dwarfdump(), Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("jwlink"))
    else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    if !jwlink.exists() {
        skipped("needs jwlink beside llrm");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/known.c");
    let made = compile(&source, &["-m32", "-O0", "-gdwarf", "-fobject-format=omf"], &dir.join("known.obj"));
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new(&jwlink)
        .args(["format", "os2", "le", "debug", "dwarf", "file", "known.obj", "name", "known.exe"])
        .args(["option", "quiet,start=main_,nodefaultlibs"])
        .current_dir(dir)
        .output()
        .unwrap();
    let image =
        std::fs::read(dir.join("known.exe")).unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&linked.stdout)));
    let elf = image.windows(4).rposition(|four| four == b"\x7fELF").expect("jwlink appends the DWARF as an ELF image");
    std::fs::write(dir.join("known.elf"), &image[elf..]).unwrap();
    let ran = |args: &[&str]| Command::new(&dwarfdump).args(args).arg(dir.join("known.elf")).output().unwrap();
    let verified = ran(&["--verify"]);
    assert!(verified.status.success(), "{}", String::from_utf8_lossy(&verified.stdout));
    let text = String::from_utf8_lossy(&ran(&["--debug-info"]).stdout).into_owned();
    for name in ["add", "main", "first", "second", "sum", "counter", "p", "pt"] {
        assert!(text.contains(&format!("DW_AT_name\t(\"{name}\")")), "no {name}:\n{text}");
    }
    assert!(text.contains("DW_OP_addr 0x10000"), "the global is not at the image's address of the data:\n{text}");
    // `-g` changes no code or data: the pages of the image are those of the
    // same program linked without it (the header's debug-information pointers
    // differ).
    let made = compile(&source, &["-m32", "-O0", "-fobject-format=omf"], &dir.join("plain.obj"));
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new(&jwlink)
        .args(["format", "os2", "le", "file", "plain.obj", "name", "plain.exe"])
        .args(["option", "quiet,start=main_,nodefaultlibs"])
        .current_dir(dir)
        .output()
        .unwrap();
    let plain =
        std::fs::read(dir.join("plain.exe")).unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&linked.stdout)));
    let le = image.windows(4).position(|four| four == b"LE\0\0").expect("an LE header");
    let pages = u32::from_le_bytes(image[le + 0x80..le + 0x84].try_into().unwrap()) as usize;
    assert_eq!(image[pages..elf], plain[pages..plain.len()], "the pages of the image differ with -g");
}

/// DWARF in a 16-bit OMF object, linked by jwlink into an MZ image, in Open
/// Watcom's own convention for segmented code: address size 2, a
/// `DW_AT_segment` on each function and data symbol, the segment of a line
/// program (extended opcode 4) and of an arange. Open Watcom's wdump is the
/// reader (llvm-dwarfdump has no 16-bit addresses); it read none of it before
/// the segment forms, and the offsets were unrelocated zeros without
/// the linker's directive.
#[test]
fn dwarf_in_a_16_bit_omf_object_links_into_an_mz_image_that_wdump_reads() {
    let Some(wdump) = wdump() else {
        skipped_for("LLRM_REQUIRE_WDUMP", "needs Open Watcom's wdump (WDUMP)");
        return;
    };
    let jwlink = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("jwlink");
    if !jwlink.exists() {
        skipped("needs jwlink beside llrm");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/known.c");
    let made = compile(&source, &["-m16", "-O0", "-gdwarf-2", "-fobject-format=omf"], &dir.join("known.obj"));
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new(&jwlink)
        .args(["format", "dos", "debug", "dwarf", "file", "known.obj", "name", "known.exe"])
        .args(["option", "quiet,start=main_,nodefaultlibs,map=known.map"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(dir.join("known.exe").exists(), "{}", String::from_utf8_lossy(&linked.stdout));
    let said = Command::new(wdump).args(["-q", "-d", "-Dx"]).arg(dir.join("known.exe")).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    for wanted in [
        "Address Size 02",
        "Segment Size 02",
        "DW_AT_name                    counter",
        "DW_AT_name                    add",
        "DW_AT_name                    first",
        "DW_AT_name                    sum",
        "DW_AT_segment",
        "SET_SEGMENT",
        // The frame description: without it a debugger cannot know the frame
        // address its DW_OP_call_frame_cfa and DW_OP_fbreg start from.
        "\".debug_frame\"",
    ] {
        assert!(text.contains(wanted), "no {wanted:?}:\n{text}");
    }
    // The global is where the linker put it: its offset in the data group, not
    // zero, and its segment the group's.
    let map = std::fs::read_to_string(dir.join("known.map")).unwrap();
    let place = map.lines().find(|line| line.contains("_counter")).expect("the map names counter");
    let offset =
        place.split_whitespace().next().unwrap().split(':').nth(1).unwrap().trim_end_matches(['*', '+']).to_uppercase();
    let wanted = format!("Loc expr: addr {}", &offset[offset.len() - 4..]);
    assert!(text.contains(&wanted), "{wanted:?} not in the dump:\n{text}");
    // -O2 keeps values in registers, whose DWARF numbers 16-bit code lacked:
    // `register ax has no DWARF number` refused the compile.
    let optimised = compile(&source, &["-m16", "-O2", "-gdwarf-2", "-fobject-format=omf"], &dir.join("fast.obj"));
    assert!(optimised.status.success(), "{}", String::from_utf8_lossy(&optimised.stderr));
}

/// A `long long` held in two registers on a 32-bit target (`wide` of
/// tests/fixtures/matrix/opt.c at -O2) had no location: the value the record
/// names is one the code never holds whole. It is the two pieces it is, EAX and
/// EDX, each four bytes.
#[test]
fn a_value_in_two_registers_is_said_as_two_pieces() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("opt.o");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/opt.c");
    let made = compile(&source, &["-m32", "-O2", "-gdwarf", "-fobject-format=elf"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let said = Command::new(dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let wide = text.split("DW_AT_name\t(\"wide\")").nth(1).expect("wide is described");
    let wide = wide.split("DW_TAG_").next().unwrap();
    // The pair may move registers, so there can be several ranges: each is two
    // pieces of four bytes.
    let ranges: Vec<&str> = wide.lines().filter(|line| line.contains("DW_OP_")).collect();
    assert!(
        !ranges.is_empty()
            && ranges
                .iter()
                .all(|line| line.matches("DW_OP_piece 0x4").count() == 2 && line.matches("DW_OP_piece").count() == 2),
        "{wide}"
    );
}

/// Nib's DWARF in OMF, linked by jwlink, read back: the 32-bit LE image's DWARF
/// by llvm-dwarfdump (verified), the 16-bit MZ image's by wdump. A bare
/// `-gdwarf` at -m16 wrote version 5, which wdump read none of (no address
/// size, no names); Open Watcom's convention is version 2.
#[test]
fn nib_dwarf_in_omf_links_into_images_that_independent_readers_read() {
    let Some(wdump) = wdump() else {
        skipped_for("LLRM_REQUIRE_WDUMP", "needs Open Watcom's wdump (WDUMP)");
        return;
    };
    let bins = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().to_path_buf();
    let (Some(dwarfdump), true) = (dwarfdump(), bins.join("jwlink").exists()) else {
        skipped("needs llvm-dwarfdump and jwlink beside llrm");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/known.nib");
    let link = |bits: &str, format: &[&str]| {
        let made = Command::new(bins.join("llrm-nib"))
            .args([&format!("-m{bits}"), "-O0", "-gdwarf", "-fobject-format=omf"])
            .arg(&source)
            .arg("-o")
            .arg(dir.join(format!("n{bits}.obj")))
            .output()
            .unwrap();
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        let linked = Command::new(bins.join("jwlink"))
            .args(format)
            .args(["debug", "dwarf", "file"])
            .arg(format!("n{bits}.obj"))
            .args(["name"])
            .arg(format!("n{bits}.exe"))
            .args(["option", "quiet,start=_main,nodefaultlibs"])
            .current_dir(dir)
            .output()
            .unwrap();
        std::fs::read(dir.join(format!("n{bits}.exe")))
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&linked.stdout)))
    };
    let image = link("32", &["format", "os2", "le"]);
    let elf = image.windows(4).rposition(|four| four == b"\x7fELF").expect("jwlink appends the DWARF as an ELF image");
    std::fs::write(dir.join("n32.elf"), &image[elf..]).unwrap();
    let ran = |args: &[&str]| Command::new(&dwarfdump).args(args).arg(dir.join("n32.elf")).output().unwrap();
    let verified = ran(&["--verify"]);
    assert!(verified.status.success(), "{}", String::from_utf8_lossy(&verified.stdout));
    let text = String::from_utf8_lossy(&ran(&["--debug-info"]).stdout).into_owned();
    for name in ["add", "main", "first", "second", "sum", "total"] {
        assert!(text.contains(&format!("DW_AT_name\t(\"{name}\")")), "no {name}:\n{text}");
    }
    link("16", &["format", "dos"]);
    let said = Command::new(wdump).args(["-q", "-d", "-Dx"]).arg(dir.join("n16.exe")).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    for wanted in [
        "Address Size 02",
        "DW_AT_name                    add",
        "DW_AT_name                    sum",
        "\".debug_frame\"",
    ] {
        assert!(text.contains(wanted), "no {wanted:?}:\n{text}");
    }
}

/// A 16-bit function is far by default (`retf`), its frame address `bp+6` in
/// the body, so `add(int first, int second)` with `sum`, kept at `[bp-2]`,
/// `[bp-4]` and `[bp-8]`, is `fbreg -8`, `-10` and `-14`. They were -6, -8 and
/// -12, the near function's, and a debugger read `first` from the saved BP: the
/// offsets dosbox-harness measured in a real run (Open Watcom's wdump is the
/// reader).
#[test]
fn a_far_16_bit_functions_cells_are_found_from_its_longer_frame_address() {
    let Some(wdump) = wdump() else {
        skipped_for("LLRM_REQUIRE_WDUMP", "needs Open Watcom's wdump (WDUMP)");
        return;
    };
    let jwlink = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("jwlink");
    if !jwlink.exists() {
        skipped("needs jwlink beside llrm");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matrix/known.c");
    let made = compile(&source, &["-m16", "-O0", "-gdwarf-2", "-fobject-format=omf"], &dir.join("known.obj"));
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    Command::new(&jwlink)
        .args(["format", "dos", "debug", "dwarf", "file", "known.obj", "name", "known.exe"])
        .args(["option", "quiet,start=main_,nodefaultlibs"])
        .current_dir(dir)
        .output()
        .unwrap();
    let said = Command::new(wdump).args(["-q", "-d", "-Dx"]).arg(dir.join("known.exe")).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    // first, second, sum: 2-byte cells at bp-2, bp-4, bp-6, the canonical frame
    // address 6 above bp (llrm-c laid `sum` at -14).
    for wanted in ["Loc expr: fbreg -8", "Loc expr: fbreg -10", "Loc expr: fbreg -12"] {
        assert!(text.contains(wanted), "no {wanted:?}:\n{text}");
    }
}

/// The tag of the DIE that holds the variable `name`, from `llvm-dwarfdump`'s
/// indented output.
fn parent_of_variable(
    dump: &str,
    name: &str,
) -> Option<String> {
    // (indent, tag, name) of each DIE; a DIE's name is its first `DW_AT_name`.
    let mut dies: Vec<(usize, String, Option<String>)> = Vec::new();
    for line in dump.lines() {
        if let Some(at) = line.find("DW_TAG_") {
            dies.push((at, line[at + 7..].trim().to_owned(), None));
        } else if line.contains("DW_AT_name") {
            if let (Some(die), Some(open)) = (dies.last_mut(), line.find("(\"")) {
                die.2.get_or_insert_with(|| line[open + 2..].trim_end_matches(")").trim_end_matches('"').to_owned());
            }
        }
    }
    let at = dies.iter().position(|(_, tag, one)| tag == "variable" && one.as_deref() == Some(name))?;
    let indent = dies[at].0;
    dies[..at].iter().rev().find(|(outer, ..)| *outer < indent).map(|(_, tag, _)| tag.clone())
}

/// A local declared in a nested `{}` was written in the function itself: the
/// writers had blocks and the backend never made one, so `t` and `u` were
/// visible (and wrong) all through `f`. They are in lexical blocks now.
#[test]
fn a_local_in_a_nested_scope_is_in_a_lexical_block() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("blocks.o");
    let made = compile(&fixtures.join("blocks.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let dumped = Command::new(dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let dump = String::from_utf8_lossy(&dumped.stdout).into_owned();
    let tag = |name| parent_of_variable(&dump, name);
    assert_eq!(tag("r").as_deref(), Some("subprogram"), "{dump}");
    assert_eq!(tag("t").as_deref(), Some("lexical_block"), "{dump}");
    assert_eq!(tag("u").as_deref(), Some("lexical_block"), "{dump}");
}

/// A function inlined twice left no trace in DWARF: the body's code was in the
/// caller's lexical block and a debugger could not tell the two bodies, or the
/// calls, apart. `inlined.ll` is `f` of `{ sq(t); } sq(a)` after the pin's
/// inliner: each body is a `DW_TAG_inlined_subroutine` of `sq` with the line
/// and column of its call.
#[test]
fn a_function_inlined_twice_has_two_inlined_subroutines_with_their_calls() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("inlined.o");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/inlined.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-O2", "-march=i486", "-fobject-format=elf", "-mabi=sysv", "-o"])
        .arg(&object)
        .arg(&source)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let dumped = Command::new(dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let dump = String::from_utf8_lossy(&dumped.stdout).into_owned();
    assert_eq!(dump.matches("DW_TAG_inlined_subroutine").count(), 2, "{dump}");
    assert!(dump.contains("DW_AT_call_line\t(2)") && dump.contains("DW_AT_call_column\t(48)"), "{dump}");
    assert!(dump.contains("DW_AT_call_column\t(68)"), "{dump}");
    assert_eq!(dump.matches("DW_AT_inline\t(DW_INL_inlined)").count(), 1, "one abstract sq: {dump}");
}

/// llrm's inliner kept the callee's locations unchanged: an inlined `sq` was
/// `sq`'s lines in `f`, with no `DW_TAG_inlined_subroutine` and no call line.
#[test]
fn a_call_the_optimizer_inlines_becomes_an_inlined_subroutine_at_its_call() {
    let Some(dwarfdump) = dwarfdump() else {
        skipped("needs llvm-dwarfdump");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("candidate.o");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/inline_candidate.ll");
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-O2", "-march=i486", "-fobject-format=elf", "-mabi=sysv", "-o"])
        .arg(&object)
        .arg(&source)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let dumped = Command::new(&dwarfdump).arg("--debug-info").arg(&object).output().unwrap();
    let dump = String::from_utf8_lossy(&dumped.stdout).into_owned();
    assert_eq!(dump.matches("DW_TAG_inlined_subroutine").count(), 2, "{dump}");
    assert!(dump.contains("DW_AT_call_line\t(2)") && dump.contains("DW_AT_call_column\t(48)"), "{dump}");
    assert!(dump.contains("DW_AT_call_column\t(68)"), "{dump}");
    let verified = Command::new(dwarfdump).arg("--verify").arg(&object).output().unwrap();
    assert!(verified.status.success(), "{}", String::from_utf8_lossy(&verified.stdout));
}
