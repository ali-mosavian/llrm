//! A stage dump on tmpfs ran /tmp out of inodes and stopped every session's
//! shell; the tools refuse it before writing anything (#1641).

use std::process::Command;

fn shm_is_tmpfs() -> bool {
    std::fs::read_to_string("/proc/mounts").is_ok_and(|mounts| {
        mounts.lines().any(|line| line.split_whitespace().take(3).collect::<Vec<_>>()[1..] == ["/dev/shm", "tmpfs"])
    })
}

/// `--dump` and `LLRM_MIR_STAGES` into a tmpfs directory both stop the tool
/// with a message and make nothing there.
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
    let by_flag = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .arg(&source)
        .args(["--dump", &flag, "-o"])
        .arg(&object)
        .output()
        .unwrap();
    let by_variable = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .env("LLRM_MIR_STAGES", &variable)
        .output()
        .unwrap();
    for (run, directory) in [(&by_flag, &flag), (&by_variable, &variable)] {
        let said = String::from_utf8_lossy(&run.stderr);
        assert_eq!(run.status.code(), Some(2), "{said}");
        assert!(said.contains("is on tmpfs"), "{said}");
        assert!(!std::path::Path::new(directory).exists(), "{directory} was made");
    }
    let _ = std::fs::remove_file(source);
}
