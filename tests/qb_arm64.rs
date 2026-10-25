//! The host runtime on arm64 Linux: the headless tests of tests/qb_headless.rs built with clang for aarch64 against a
//! private sysroot (tools/build-arm64-sysroot.sh) and run under qemu-aarch64. A machine without clang, lld or qemu fails
//! here by name; it does not skip. An arm64 Mac runs the same C natively (docs/working/qb-host.md, "Mac session").

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "qbhost/lists.rs"]
mod lists;
use lists::{DEVICE, RUNTIME};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn sysroot() -> PathBuf {
    let made = Command::new(root().join("tools/build-arm64-sysroot.sh")).output().expect("tools/build-arm64-sysroot.sh runs");
    assert!(made.status.success(), "no arm64 sysroot: {}", String::from_utf8_lossy(&made.stderr));
    PathBuf::from(String::from_utf8_lossy(&made.stdout).trim())
}

/// Builds `sources` for aarch64 Linux, runs the program under qemu; its standard output.
fn build_and_run(name: &str, sources: &[&str], arguments: &[&str]) -> String {
    let out = std::env::temp_dir().join(format!("{name}_arm64_{}", std::process::id()));
    build_and_run_with(name, sources, arguments, None, &out)
}

/// `build_and_run`, with an object to link and the program's path.
fn build_and_run_with(name: &str, sources: &[&str], arguments: &[&str], object: Option<&Path>, out: &Path) -> String {
    let sysroot = sysroot();
    let libc = sysroot.join("usr/aarch64-linux-gnu");
    let gcc = sysroot.join("usr/lib/gcc-cross/aarch64-linux-gnu/15");
    let qb = root().join("runtime/qb");
    let mut build = Command::new("clang");
    build.args(["--target=aarch64-linux-gnu", "-fuse-ld=lld", "-static", "-std=gnu11", "-w", "-g", "-pthread"]);
    // the root, not the libc directory: libm.a is a linker script that names /usr/aarch64-linux-gnu/lib/libm-2.42.a
    build.arg(format!("--sysroot={}", sysroot.display())).arg(format!("-B{}", gcc.display()));
    build.arg(format!("-L{}", gcc.display())).arg(format!("-L{}", libc.join("lib").display())).arg("-isystem").arg(libc.join("include")).arg("-D__cdecl=");
    for include in ["posix", ".", "dev", "dev/headless", "dev/host", "../shared/posix"] {
        build.arg("-I").arg(qb.join(include));
    }
    for source in sources {
        build.arg(root().join(source));
    }
    build.args(object);
    build.arg("-lm").arg("-o").arg(out);
    let built = build.output().expect("clang runs");
    assert!(built.status.success(), "{name}: {}", String::from_utf8_lossy(&built.stderr));
    let done = Command::new("qemu-aarch64").arg(out).args(arguments).output().expect("qemu-aarch64 runs");
    std::fs::remove_file(out).ok();
    let printed = String::from_utf8_lossy(&done.stdout).into_owned();
    assert!(done.status.success(), "{name} failed:\n{printed}{}", String::from_utf8_lossy(&done.stderr));
    printed
}

#[test]
fn the_headless_device_does_what_the_interface_says_on_arm64() {
    let dump = std::env::temp_dir().join(format!("headless_dump_arm64_{}.ppm", std::process::id()));
    let mut sources = vec!["tests/qbhost/headless_test.c"];
    sources.extend(DEVICE);
    let printed = build_and_run("headless_test", &sources, &[dump.to_str().unwrap()]);
    assert_eq!(printed.trim(), "ok");
}

#[test]
fn the_headless_graphics_pass_the_oracle_on_arm64() {
    let mut sources = vec!["tests/qbgfx/fill_ops.c", "tests/qbhost/report.c"];
    sources.extend(DEVICE);
    assert_eq!(build_and_run("fill_ops_arm64", &sources, &[]).trim(), "0");
}

#[test]
fn the_runtimes_statements_run_on_arm64() {
    let mut sources = vec!["tests/qbhost/program_test.c", "tests/qbhost/hostmodule.c"];
    sources.extend(DEVICE);
    sources.extend(RUNTIME);
    assert_eq!(build_and_run("program_test_arm64", &sources, &[]).trim(), "ok");
}

/// Compiles `source` for arm64 Linux (`llrm-qb --arm64=linux`), links it with the whole host runtime on the headless
/// device and runs it under qemu; the text its screen shows.
fn run_basic(name: &str, source: &str) -> String {
    let dir = std::env::temp_dir().join(format!("{name}_arm64_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bas = dir.join(format!("{name}.bas"));
    std::fs::write(&bas, source).unwrap();
    let object = dir.join(format!("{name}.o"));
    let compiled = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .args(["--arm64=linux", "-O2"])
        .arg(&bas)
        .arg("-o")
        .arg(&object)
        .output()
        .expect("llrm-qb runs");
    assert!(compiled.status.success(), "llrm-qb: {}", String::from_utf8_lossy(&compiled.stderr));

    let qb = root().join("runtime/qb");
    let mut sources: Vec<String> = std::fs::read_dir(&qb)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .map(|path| path.strip_prefix(root()).unwrap().to_string_lossy().into_owned())
        .collect();
    sources.extend(
        [
            "runtime/qb/posix/startup.c",
            "runtime/qb/posix/block.c",
            "runtime/qb/posix/ad.c",
            "runtime/qb/posix/mathops.c",
            "runtime/qb/posix/paintq.c",
            "runtime/qb/posix/module_exports.c",
            "runtime/qb/posix/program.c",
            "runtime/qb/posix/real.c",
            "runtime/qb/dev/headless/headless.c",
            "runtime/qb/dev/headless/main.c",
            "runtime/qb/dev/host/video.c",
            "runtime/qb/dev/host/vgadata.c",
            "runtime/qb/dev/host/tick.c",
            "runtime/shared/posix/os.c",
        ]
        .map(str::to_owned),
    );
    let program = dir.join(name);
    let sources: Vec<&str> = sources.iter().map(String::as_str).collect();
    let printed = build_and_run_with(name, &sources, &[], Some(&object), &program);
    std::fs::remove_dir_all(&dir).ok();
    printed
}

/// `PRINT "HELLO"` through llrm-qb's own arm64 code: the first QB program to run on arm64. Before `--arm64` the compiler
/// had no route to the arm64 backend at all.
#[test]
fn print_hello_runs_on_arm64() {
    assert_eq!(run_basic("hello", "PRINT \"HELLO\"\nEND\n").trim_end(), "HELLO");
}
