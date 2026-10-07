//! The object writers: what `-fobject-format=` picks, and what readelf and objdump make of the
//! objects it writes for every bench C program.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn llrm_c() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-c")
}

fn compile(source: &Path, arguments: &[&str], out: &Path) -> Output {
    Command::new(llrm_c()).args(arguments).arg(source).arg("-o").arg(out).output().unwrap()
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").output().is_ok()
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

/// readelf and objdump accept the ELF object of every bench C program, at both sizes.
#[test]
fn readelf_and_objdump_accept_every_bench_objects() {
    if !have("readelf") || !have("objdump") {
        eprintln!("skipped: binutils is not installed");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let programs = bench_programs();
    assert!(programs.len() > 20, "{programs:?}");
    for source in &programs {
        for level in ["-O2", "-Os"] {
            let object = scratch.path().join("x.o");
            let made = compile(source, &["-m32", level, "-fobject-format=elf"], &object);
            assert!(made.status.success(), "{} {level}: {}", source.display(), String::from_utf8_lossy(&made.stderr));
            for (tool, arguments) in [("readelf", &["-a", "--wide"][..]), ("objdump", &["-dr"][..])] {
                let said = Command::new(tool).args(arguments).arg(&object).output().unwrap();
                let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
                assert!(said.status.success() && !text.contains("Warning") && !text.contains("Error") && !text.contains("not recognized"), "{tool} {} {level}:\n{text}", source.display());
            }
        }
    }
}

fn llvm(tool: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.push("/usr/lib/llvm-20/bin".into());
    dirs.iter().map(|dir| dir.join(tool)).find(|path| path.exists())
}

/// llvm-readobj and llvm-objdump accept the COFF object of every bench C program, at both ends of
/// the optimiser.
#[test]
fn llvm_accepts_every_bench_coff_object() {
    let (Some(readobj), Some(objdump)) = (llvm("llvm-readobj"), llvm("llvm-objdump")) else {
        eprintln!("skipped: LLVM's object tools are not installed");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    for source in &bench_programs() {
        for level in ["-O0", "-O2"] {
            let object = scratch.path().join("x.obj");
            let made = compile(source, &["-m32", level, "-fobject-format=coff"], &object);
            assert!(made.status.success(), "{} {level}: {}", source.display(), String::from_utf8_lossy(&made.stderr));
            for (tool, arguments) in [(&readobj, &["--file-headers", "--sections", "--symbols", "--relocations"][..]), (&objdump, &["-d", "-r"][..])] {
                let said = Command::new(tool).args(arguments).arg(&object).output().unwrap();
                let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
                assert!(said.status.success() && !text.contains("warning") && !text.contains("error"), "{} {} {level}:\n{text}", tool.display(), source.display());
            }
        }
    }
}

/// `-g`: llvm-readobj reads the C13 of every bench C program's COFF object and llvm-objdump its
/// code, at both ends of the optimiser. Each object names its functions.
#[test]
fn llvm_reads_the_codeview_of_every_bench_coff_object() {
    let (Some(readobj), Some(objdump)) = (llvm("llvm-readobj"), llvm("llvm-objdump")) else {
        eprintln!("skipped: LLVM's object tools are not installed");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    for source in &bench_programs() {
        for level in ["-O0", "-O2"] {
            let object = scratch.path().join("x.obj");
            let made = compile(source, &["-m32", level, "-g", "-fobject-format=coff"], &object);
            assert!(made.status.success(), "{} {level}: {}", source.display(), String::from_utf8_lossy(&made.stderr));
            for (tool, arguments) in [(&readobj, &["--codeview"][..]), (&objdump, &["-d", "-r"][..])] {
                let said = Command::new(tool).args(arguments).arg(&object).output().unwrap();
                let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
                assert!(said.status.success() && !text.contains("warning") && !text.contains("error"), "{} {} {level}:\n{text}", tool.display(), source.display());
                if tool == &readobj {
                    assert!(text.contains("S_GPROC32") && text.contains("FunctionLineTable"), "{} {level}: no function or lines:\n{text}", source.display());
                }
            }
        }
    }
}

/// `-g` through the whole path: lld-link links the C program's object with /debug, and the PDB holds
/// its function with both parameters, its struct, its global and its lines. The same program
/// as OMF CodeView names the same function, parameters and struct.
#[test]
fn lld_link_makes_a_pdb_of_a_c_program() {
    let (Some(link), Some(pdbutil)) = (llvm("lld-link"), llvm("llvm-pdbutil")) else {
        eprintln!("skipped: needs lld-link and llvm-pdbutil");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    std::fs::write(dir.join("p.c"), "struct S { int a; char b; };\nint g = 3;\nint twice(int a, struct S *p)\n{\n    int y = a + p->a;\n    return y + y + g;\n}\n").unwrap();
    let made = compile(&dir.join("p.c"), &["-m32", "-O0", "-g", "-fobject-format=coff"], &dir.join("p.obj"));
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let linked = Command::new(link).args(["/machine:x86", "/subsystem:console", "/entry:twice", "/nodefaultlib", "/debug", "/pdb:p.pdb", "/out:p.exe", "p.obj"]).current_dir(dir).output().unwrap();
    assert!(linked.status.success(), "{}{}", String::from_utf8_lossy(&linked.stdout), String::from_utf8_lossy(&linked.stderr));
    let said = Command::new(pdbutil).args(["dump", "-l", "--symbols", "--types", "--globals"]).arg(dir.join("p.pdb")).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout);
    for wanted in ["S_GPROC32", "`twice`", "S_LOCAL", "`a`", "`p`", "`y`", "flags = param", "LF_STRUCTURE", "`S`", "sizeof 8", "S_GDATA32", "`g`", "line/addr entries"] {
        assert!(text.contains(wanted), "no {wanted}:\n{text}");
    }
}

const SOURCE: &str = "int twice(int a) { return a + a; }\n";

/// The default format is the target's own, OMF: THEADR opens the file.
#[test]
fn without_a_format_the_target_writes_its_default() {
    let scratch = tempfile::tempdir().unwrap();
    let (source, object) = (scratch.path().join("t.c"), scratch.path().join("t.obj"));
    std::fs::write(&source, SOURCE).unwrap();
    for mode in ["-m16", "-m32"] {
        assert!(compile(&source, &[mode, "-O2"], &object).status.success());
        assert_eq!(std::fs::read(&object).unwrap()[0], 0x80, "{mode}");
    }
}

/// COFF's i386 machine number opens the file, and its symbols carry the target's `coff` decoration.
#[test]
fn coff_is_a_coff_object_for_a_target_that_lists_it() {
    let scratch = tempfile::tempdir().unwrap();
    let (source, object) = (scratch.path().join("t.c"), scratch.path().join("t.obj"));
    std::fs::write(&source, SOURCE).unwrap();
    assert!(compile(&source, &["-m32", "-O2", "-fobject-format=coff"], &object).status.success());
    let bytes = std::fs::read(&object).unwrap();
    assert_eq!(&bytes[..2], [0x4C, 0x01]);
    assert!(bytes.windows(7).any(|window| window == b"_twice\0"), "no _twice in the string table or symbols");
}

#[test]
fn elf_is_an_elf_object_for_a_target_that_lists_it() {
    let scratch = tempfile::tempdir().unwrap();
    let (source, object) = (scratch.path().join("t.c"), scratch.path().join("t.o"));
    std::fs::write(&source, SOURCE).unwrap();
    assert!(compile(&source, &["-m32", "-O2", "-fobject-format=elf"], &object).status.success());
    assert_eq!(&std::fs::read(&object).unwrap()[..4], b"\x7fELF");
}

/// A format a target cannot write was never refused: the request fell through to its default or to a
/// wrong file. It names what the target writes.
#[test]
fn a_format_the_target_does_not_write_is_refused() {
    let scratch = tempfile::tempdir().unwrap();
    let (source, object) = (scratch.path().join("t.c"), scratch.path().join("t.o"));
    std::fs::write(&source, SOURCE).unwrap();
    let refused = |arguments: &[&str]| {
        let done = compile(&source, arguments, &object);
        assert!(!done.status.success());
        String::from_utf8_lossy(&done.stderr).into_owned()
    };
    let real = refused(&["-m16", "-fobject-format=elf"]);
    assert!(real.contains("cannot write elf") && real.contains("it writes omf"), "{real}");
    let unlisted = refused(&["-m16", "-fobject-format=coff"]);
    assert!(unlisted.contains("cannot write coff") && unlisted.contains("it writes omf"), "{unlisted}");
    let unknown = refused(&["-m32", "-fobject-format=pe"]);
    assert!(unknown.contains("-fobject-format=pe") && unknown.contains("\"coff\""), "{unknown}");
    let unwritten = refused(&["-m32", "-fobject-format=macho"]);
    assert!(unwritten.contains("cannot write macho"), "{unwritten}");
}
