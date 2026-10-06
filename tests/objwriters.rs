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
    let unknown = refused(&["-m32", "-fobject-format=coff"]);
    assert!(unknown.contains("-fobject-format=coff") && unknown.contains("\"macho\""), "{unknown}");
    let unwritten = refused(&["-m32", "-fobject-format=macho"]);
    assert!(unwritten.contains("cannot write macho"), "{unwritten}");
}
