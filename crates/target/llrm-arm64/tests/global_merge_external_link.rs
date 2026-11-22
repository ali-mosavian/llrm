//! A merged external keeps its name and address for other objects: a second
//! object (gcc's) stores through the names and compares the address of one
//! across objects. Skipped where there is no aarch64 gcc and `qemu-aarch64`.

use std::process::Command;

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").output().is_ok_and(|said| said.status.success())
}

const FIRST: &str = include_str!("data/global_merge_external.ll");

const SECOND: &str = r#"
extern int a, b, c, d;
extern void f(int);
extern int *address_of_c(void);
int main(void) {
    f(10);
    int bad = (address_of_c() != &c) | (a != 10) | (b != 10) | (c != 10) | (d != 10);
    c = 99;
    bad |= (*address_of_c() != 99);
    return bad;
}
"#;

#[test]
fn merged_externals_link_with_another_object_and_keep_their_addresses() {
    let (gcc, qemu) = ("aarch64-linux-gnu-gcc", "qemu-aarch64");
    if !have(gcc) || !have(qemu) {
        eprintln!("skipped: no {gcc} or {qemu}");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let first = format!("{FIRST}\ndefine ptr @address_of_c() nounwind {{ ret ptr @c }}\n");
    let object = driver::object_file(&first, Platform::Linux, "first", Level::Os).expect("lowers");
    std::fs::write(dir.path().join("first.o"), object).unwrap();
    std::fs::write(dir.path().join("second.c"), SECOND).unwrap();
    let (second, program) = (dir.path().join("second.o"), dir.path().join("program"));
    for (args, what) in [
        (
            vec![
                "-O1".into(),
                "-c".into(),
                dir.path().join("second.c").display().to_string(),
                "-o".into(),
                second.display().to_string(),
            ],
            "compile",
        ),
        (
            vec![
                dir.path().join("first.o").display().to_string(),
                second.display().to_string(),
                "-o".into(),
                program.display().to_string(),
            ],
            "link",
        ),
    ] {
        let said = Command::new(gcc).args(&args).output().unwrap();
        assert!(said.status.success(), "{what}: {}", String::from_utf8_lossy(&said.stderr));
    }
    let ran = Command::new(qemu).args(["-L", "/usr/aarch64-linux-gnu"]).arg(&program).status().unwrap();
    assert!(ran.success(), "the second object saw other values or addresses");
}
