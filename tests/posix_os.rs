//! The POSIX OS layer (`runtime/shared/posix/os.c`) against its own header:
//! tests/posix_os/os_test.c, built with the host's C compiler and run with an
//! empty standard input.

use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn the_posix_os_layer_does_what_the_interface_says() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = std::env::temp_dir().join(format!("posix_os_test_{}", std::process::id()));
    let built = Command::new("cc")
        .args(["-O1", "-g", "-Wall", "-Werror", "-I"])
        .arg(root.join("runtime/shared/posix"))
        .arg(root.join("tests/posix_os/os_test.c"))
        .arg(root.join("runtime/shared/posix/os.c"))
        .arg("-o")
        .arg(&out)
        .output()
        .expect("cc runs");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new(&out).args(["one", "two"]).stdin(Stdio::null()).output().expect("the test runs");
    std::fs::remove_file(&out).ok();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stdout));
    assert_eq!(String::from_utf8_lossy(&done.stdout).trim(), "ok");
}
