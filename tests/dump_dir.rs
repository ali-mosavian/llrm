//! A stage dump on tmpfs ran /tmp out of inodes and stopped every session's
//! shell; the tools refuse it before writing anything (#1641).

mod common;

use std::process::Command;

fn shm_is_tmpfs() -> bool {
    std::fs::read_to_string("/proc/mounts").is_ok_and(|mounts| {
        mounts.lines().any(|line| line.split_whitespace().take(3).collect::<Vec<_>>()[1..] == ["/dev/shm", "tmpfs"])
    })
}

/// `--dump` and `LLRM_MIR_STAGES` into a tmpfs directory both stop the tool
/// with a message and make nothing there.
///
/// llrm-cc: the refusal was llrm-c's and went with it; llrm-cc calls the same
/// check, so a dump into tmpfs exited 0 and wrote there.
#[test]
fn a_dump_into_tmpfs_is_refused_by_each_tool() {
    if !shm_is_tmpfs() {
        return;
    }
    let source = std::env::temp_dir().join(format!("dump-dir-{}.c", std::process::id()));
    std::fs::write(&source, "int f(int a) { return a + 1; }\n").unwrap();
    let flag = format!("/dev/shm/dump-dir-flag-{}", std::process::id());
    let variable = format!("/dev/shm/dump-dir-env-{}", std::process::id());
    let object = std::env::temp_dir().join(format!("dump-dir-{}.obj", std::process::id()));
    let by_flag = common::cc::llrm_c().arg(&source).args(["--dump", &flag, "-o"]).arg(&object).output().unwrap();
    let by_variable =
        common::cc::llrm_c().arg(&source).arg("-o").arg(&object).env("LLRM_MIR_STAGES", &variable).output().unwrap();
    for (run, directory) in [(&by_flag, &flag), (&by_variable, &variable)] {
        let said = String::from_utf8_lossy(&run.stderr);
        assert_eq!(run.status.code(), Some(2), "{said}");
        assert!(said.contains("is on tmpfs"), "{said}");
        assert!(!std::path::Path::new(directory).exists(), "{directory} was made");
    }
    let _ = std::fs::remove_file(source);
}

/// `llrm-qb --dump` wrote the source and the HIR and no MIR stage at all: every
/// difftest and oracle run that read the stage dumps saw only C programs. A
/// BASIC program's dump has the stage files a C program's has, in `mir/`.
#[test]
fn a_basic_programs_dump_has_the_stage_files_a_c_programs_has() {
    let scratch = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let c = scratch.path().join("p.c");
    std::fs::write(&c, "int f(int a) { return a + 1; }\n").unwrap();
    let bas = scratch.path().join("p.bas");
    std::fs::write(&bas, "PRINT 1 + 2\n").unwrap();
    let names = |directory: std::path::PathBuf| {
        let mut found: Vec<String> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains('-') && name.ends_with(".ll"))
            .collect();
        found.sort();
        found
    };
    let in_c = scratch.path().join("dump-c");
    let in_bas = scratch.path().join("dump-bas");
    let done = common::cc::llrm_c()
        .arg(&c)
        .args(["-m16", "-O2", "--dump"])
        .arg(&in_c)
        .arg("-o")
        .arg(scratch.path().join("p.obj"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .arg(&bas)
        .args(["-O2", "--dump"])
        .arg(&in_bas)
        .arg("-o")
        .arg(scratch.path().join("b.obj"))
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    assert_eq!(names(in_bas.join("mir")), names(in_c), "the stage list of a BASIC program is a C program's");
    for file in ["listing.asm", "cost"] {
        assert!(in_bas.join("mir").join(file).exists(), "mir/{file} was not written");
    }
}
