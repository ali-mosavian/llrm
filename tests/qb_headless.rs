//! The QB device interface on the headless platform (runtime/qb/dev/headless),
//! through C programs built with the host's compiler: the device operations
//! alone, the same random graphics oracle the DOS device passes (tests/qbgfx/
//! fill_ops.c), and the runtime's own statements (PRINT, INKEY$, BEEP, SOUND,
//! PLAY, TIMER, TIME$, DATE$, SCREEN 13, LINE, CIRCLE, PAINT, GET, PUT) driven
//! as a compiled program drives them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Builds `sources` (paths under the root) into one program and runs it with
/// `arguments`; its standard output.
fn build_and_run(
    name: &str,
    sources: &[&str],
    defines: &[&str],
    arguments: &[&str],
) -> String {
    let out: PathBuf = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
    let qb = root().join("runtime/qb");
    let mut build = Command::new("cc");
    build.args(["-std=gnu11", "-w", "-g", "-pthread"]);
    for define in defines {
        build.arg(format!("-D{define}"));
    }
    for include in ["posix", ".", "dev", "dev/headless", "dev/host", "../shared/posix"] {
        build.arg("-I").arg(qb.join(include));
    }
    for source in sources {
        build.arg(root().join(source));
    }
    build.arg("-lm").arg("-o").arg(&out);
    let built = build.output().expect("cc runs");
    assert!(built.status.success(), "{name}: {}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new(&out).args(arguments).output().expect("the program runs");
    std::fs::remove_file(&out).ok();
    let printed = String::from_utf8_lossy(&done.stdout).into_owned();
    assert!(done.status.success(), "{name} failed:\n{printed}{}", String::from_utf8_lossy(&done.stderr));
    printed
}

#[path = "qbhost/lists.rs"]
mod lists;
use lists::{DEVICE, RUNTIME};

#[test]
fn the_headless_device_does_what_the_interface_says() {
    let dump = std::env::temp_dir().join(format!("headless_dump_{}.ppm", std::process::id()));
    let mut sources = vec!["tests/qbhost/headless_test.c"];
    sources.extend(DEVICE);
    let printed = build_and_run("headless_test", &sources, &[], &[dump.to_str().unwrap()]);
    assert_eq!(printed.trim(), "ok");
}

/// The DOS device passes this on the screen (tools/dosbatch/test_qbgfxfill.py):
/// every mode, operation, box, dot, line, circle step, search, run, GET and PUT
/// against the pixel at a time.
#[test]
fn the_headless_graphics_pass_the_oracle_the_dos_device_passes() {
    let mut sources = vec!["tests/qbgfx/fill_ops.c", "tests/qbhost/report.c"];
    sources.extend(DEVICE);
    let printed = build_and_run("fill_ops_host", &sources, &[], &[]);
    assert_eq!(printed.trim(), "0");
}

#[test]
fn the_runtimes_statements_run_on_the_headless_device() {
    let mut sources = vec!["tests/qbhost/program_test.c", "tests/qbhost/hostmodule.c"];
    sources.extend(DEVICE);
    sources.extend(RUNTIME);
    let printed = build_and_run("program_test", &sources, &["__cdecl="], &[]);
    assert_eq!(printed.trim(), "ok");
}
