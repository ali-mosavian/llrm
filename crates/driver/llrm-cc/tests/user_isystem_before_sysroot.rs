//! A project header named like a glibc one (`error.h`) found through the
//! user's `-isystem` must win over the arm64 sysroot. llrm-cc put the sysroot
//! first, so the QuickrBasic runtime read glibc's `error.h` and failed on
//! `b_errnum` and `b_inonerr`.

use std::process::Command;

#[test]
fn a_user_isystem_header_beats_the_arm64_sysroot_header() {
    if std::env::var_os("LLRM_PIN_CLANG").is_none() {
        eprintln!("skipped: LLRM_PIN_CLANG names no pinned clang");
        return;
    }
    let work = std::env::temp_dir().join(format!("llrm-cc-isystem-{}", std::process::id()));
    let sysroot_include = work.join("sysroot/usr/aarch64-linux-gnu/include");
    let project = work.join("project");
    std::fs::create_dir_all(&sysroot_include).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(sysroot_include.join("error.h"), "int from_sysroot;\n").unwrap();
    std::fs::write(project.join("error.h"), "int from_project;\n").unwrap();
    std::fs::write(work.join("main.c"), "#include \"error.h\"\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "arm64-glibc")
        .env("LLRM_ARM64_SYSROOT", work.join("sysroot"))
        .arg("-E")
        .arg("-isystem")
        .arg(&project)
        .arg(work.join("main.c"))
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&work);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains("from_project") && !text.contains("from_sysroot"), "{text}");
}
