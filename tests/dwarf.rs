//! `-g` as DWARF in an ELF object: llvm-dwarfdump accepts every bench C program's, and gdb, driven
//! by a script, stops where it is told in a linked and running program and reads its values.

use std::path::{Path, PathBuf};
use std::process::Command;

fn llrm_c() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-c")
}

fn tool(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.push("/usr/lib/llvm-20/bin".into());
    dirs.iter().map(|dir| dir.join(name)).find(|path| path.exists())
}

fn dwarfdump() -> Option<PathBuf> {
    tool("llvm-dwarfdump").or_else(|| tool("llvm-dwarfdump-20"))
}

fn compile(source: &Path, arguments: &[&str], out: &Path) -> std::process::Output {
    Command::new(llrm_c()).args(arguments).arg(source).arg("-o").arg(out).output().unwrap()
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

/// llvm-dwarfdump --verify finds no error and no warning in the DWARF 4 and 5 of every bench C
/// program at -O0 and -O2, and the unit has code, functions and lines: a verifier passes an empty one.
#[test]
fn dwarfdump_verifies_every_bench_program_at_both_ends_of_the_optimiser() {
    let Some(dump) = dwarfdump() else {
        eprintln!("skipped: llvm-dwarfdump is not installed");
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
                assert!(made.status.success(), "{} {level} {flag}: {}", source.display(), String::from_utf8_lossy(&made.stderr));
                let said = Command::new(&dump).arg("--verify").arg(&object).output().unwrap();
                let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
                assert!(said.status.success() && text.trim_end().ends_with("No errors.") && !text.contains("warning"), "{} {level} {flag}:\n{text}", source.display());
                let shown = Command::new(&dump).args(["--debug-info", "--debug-line"]).arg(&object).output().unwrap();
                let shown = String::from_utf8_lossy(&shown.stdout);
                assert!(shown.contains("DW_TAG_subprogram") && shown.contains("DW_AT_low_pc") && shown.contains("is_stmt"), "{} {level} {flag}: an empty unit", source.display());
            }
        }
    }
}

/// gdb on a -m32 ELF program linked by ld and run under Linux: it stops after the prologue of a
/// function at the line the body starts on, and prints a parameter, a local, a struct field through a
/// pointer, the function's type and the value it returns.
#[test]
fn gdb_stops_at_a_line_and_reads_a_parameter_a_local_a_struct_field_and_the_return_type() {
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        eprintln!("skipped: needs gdb, GNU ld and as");
        return;
    };
    if cfg!(not(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "x86")))) {
        eprintln!("skipped: needs an x86 Linux host that runs i386 programs");
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let (start, object, program) = (scratch.path().join("start.o"), scratch.path().join("gdb.o"), scratch.path().join("gdb"));
    let made = Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("gdb.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(12) {
        eprintln!("skipped: this host does not run i386 programs");
        return;
    }
    let script = scratch.path().join("script.gdb");
    std::fs::write(
        &script,
        "set width 0\nbreak add\nrun\nprint a\nprint p->y\nprint *p\nnext\nprint l\nptype add\nfinish\n",
    )
    .unwrap();
    let said = Command::new(gdb).args(["-batch", "-nx", "-x"]).arg(&script).arg(&program).current_dir(&fixtures).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    // The stop is the first statement of the body, not the one after it.
    assert!(text.contains("add (a=1, p=0x") && text.contains("gdb.c:7") && text.contains("int l = a + p->x;"), "{text}");
    for expected in ["$1 = 1", "$2 = 4", "$3 = {x = 3, y = 4}", "$4 = 4", "type = int32_t (int32_t, struct pt *)", "Value returned is $5 = 12"] {
        assert!(text.contains(expected), "no {expected:?} in:\n{text}");
    }
}

/// `-g` takes the object format's own debug format; a flavor the format cannot carry is an error
/// that says so, never an object with another flavor in it.
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
    assert!(refused(&["-m32", "-gdwarf"]).contains("OMF cannot carry DWARF"));
    assert!(refused(&["-m32", "-gdwarf-4", "-fobject-format=omf"]).contains("OMF cannot carry DWARF"));
    assert!(refused(&["-m32", "-gcodeview", "-fobject-format=elf"]).contains("cannot carry CodeView"));
    assert!(refused(&["-m32", "-gtd", "-fobject-format=elf"]).contains("Turbo Debugger"));
    assert!(refused(&["-m32", "-gdwarf-3", "-fobject-format=elf"]).contains("unrecognized"));
    let made = compile(&source, &["-m32", "-g", "-fobject-format=elf"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    assert!(std::fs::read(&object).unwrap().windows(11).any(|one| one == b".debug_info"), "-g on ELF is DWARF");
    let made = compile(&source, &["-m32", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let bytes = std::fs::read(&object).unwrap();
    assert!(bytes.windows(6).any(|one| one == b"DEBSYM") && !bytes.windows(11).any(|one| one == b".debug_info"), "-g on OMF is CodeView");
}

/// The values gdb reads of a program's variables at a line it stops on, as `-O0` and `-O2` leave them.
fn stopped_at(gdb: &Path, program: &Path, line: u32, fixtures: &Path) -> Option<(u32, Vec<String>)> {
    let said = Command::new(gdb)
        .args(["-batch", "-nx", "-ex", &format!("tbreak observed.c:{line}"), "-ex", "run", "-ex", "info locals", "-ex", "info args"])
        .arg(program)
        .current_dir(fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    let at = text.split("observed.c:").nth(1)?.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()?;
    let values = text.lines().filter(|one| one.split_once(" = ").is_some_and(|(name, _)| name.chars().all(|c| c.is_alphanumeric() || c == '_'))).map(str::to_owned).collect();
    Some((at, values))
}

/// A debugger reads a variable from its frame cell, so at -O2 the cell holds what -O0's holds at every
/// line both stop on: before a store to a declared variable stayed where the source wrote it, the
/// optimiser kept `a` and `i` in registers and wrote their cells once at the exit, and gdb read
/// `a = 0, i = 0` through the loop where -O0 reads `a = 20, i = 4`.
#[test]
fn a_variable_reads_the_same_at_o2_as_at_o0_on_every_line_both_stop_at() {
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        eprintln!("skipped: needs gdb, GNU ld and as");
        return;
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let start = scratch.path().join("start.o");
    let made = Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let mut programs = Vec::new();
    for level in ["-O0", "-O2"] {
        let (object, program) = (scratch.path().join(format!("observed{level}.o")), scratch.path().join(format!("observed{level}")));
        let made = compile(&fixtures.join("observed.c"), &["-m32", level, "-fobject-format=elf", "-g"], &object);
        assert!(made.status.success(), "{level}: {}", String::from_utf8_lossy(&made.stderr));
        let linked = Command::new(&ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        programs.push(program);
    }
    if Command::new(&programs[0]).output().ok().and_then(|ran| ran.status.code()) != Some(73) {
        eprintln!("skipped: this host does not run i386 programs");
        return;
    }
    assert_eq!(Command::new(&programs[1]).output().unwrap().status.code(), Some(73), "-O2 computes what -O0 does");
    let (mut same, mut wrong) = (0, Vec::new());
    for line in 10..=48 {
        let (Some(slow), Some(fast)) = (stopped_at(&gdb, &programs[0], line, &fixtures), stopped_at(&gdb, &programs[1], line, &fixtures)) else { continue };
        // A line with no code stops at the next one that has some: compare where both stopped alike.
        if slow.0 != fast.0 {
            continue;
        }
        if slow.1 == fast.1 {
            same += 1;
        } else {
            wrong.push(format!("line {}: -O0 {:?}, -O2 {:?}", slow.0, slow.1, fast.1));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(same >= 20, "only {same} lines were compared");
}

/// `struct node { struct node *next; int v; }`: a struct that holds a pointer to itself kept only `v`
/// in the debug information, whichever format wrote it: asked for while it was being built, it
/// answered "none" and the member naming it was dropped. gdb follows the list; CodeView keeps the field.
#[test]
fn a_struct_that_names_itself_keeps_the_member_that_does_in_both_formats() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    // CodeView, in an OMF object.
    let object = scratch.path().join("list.obj");
    let made = compile(&fixtures.join("list.c"), &["-m16", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let records = llrm_core::objectfile::omf::parse(&std::fs::read(&object).unwrap()).unwrap();
    let shape = llrm_core::objectfile::cvinfo::parse(&records).shape();
    let head = shape.iter().find(|one| one.starts_with("DATA head")).unwrap_or_else(|| panic!("{shape:#?}"));
    assert!(head.contains("next +0") && head.contains("v +2"), "{head}");
    // DWARF, in an ELF object, and read by gdb.
    let (Some(gdb), Some(ld), Some(assembler)) = (tool("gdb"), tool("ld"), tool("as")) else {
        eprintln!("skipped: needs gdb, GNU ld and as");
        return;
    };
    let (start, object, program) = (scratch.path().join("start.o"), scratch.path().join("list.o"), scratch.path().join("list"));
    let made = Command::new(assembler).arg("--32").arg("-o").arg(&start).arg(fixtures.join("start.s")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let made = compile(&fixtures.join("list.c"), &["-m32", "-O0", "-fobject-format=elf", "-g"], &object);
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new(ld).args(["-m", "elf_i386", "-o"]).arg(&program).arg(&start).arg(&object).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    if Command::new(&program).output().ok().and_then(|ran| ran.status.code()) != Some(10) {
        eprintln!("skipped: this host does not run i386 programs");
        return;
    }
    let said = Command::new(gdb)
        .args(["-batch", "-nx", "-ex", "break sum", "-ex", "run", "-ex", "print *n", "-ex", "print n->next->v", "-ex", "print *n->next"])
        .arg(&program)
        .current_dir(&fixtures)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&said.stdout).into_owned();
    assert!(text.contains("$1 = {next = 0x") && text.contains(", v = 3}"), "{text}");
    assert!(text.contains("$2 = 7") && text.contains("$3 = {next = 0x0, v = 7}"), "{text}");
}
