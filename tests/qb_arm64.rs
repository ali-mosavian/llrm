//! The host runtime on arm64 Linux: the headless tests of tests/qb_headless.rs
//! built with clang for aarch64 against a private sysroot
//! (tools/targets/build-arm64-sysroot.sh) and run under qemu-aarch64. A machine
//! without clang, lld or qemu fails here by name; it does not skip. An arm64
//! Mac runs the same C natively (docs/working/qb-host.md, "Mac session").

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "qbhost/lists.rs"]
mod lists;
use lists::{DEVICE, RUNTIME};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn sysroot() -> PathBuf {
    let made = Command::new(root().join("tools/targets/build-arm64-sysroot.sh"))
        .output()
        .expect("tools/targets/build-arm64-sysroot.sh runs");
    assert!(made.status.success(), "no arm64 sysroot: {}", String::from_utf8_lossy(&made.stderr));
    PathBuf::from(String::from_utf8_lossy(&made.stdout).trim())
}

/// Builds `sources` for aarch64 Linux, runs the program under qemu; its
/// standard output.
fn build_and_run(
    name: &str,
    sources: &[&str],
    arguments: &[&str],
) -> String {
    let out = std::env::temp_dir().join(format!("{name}_arm64_{}", std::process::id()));
    build_and_run_with(name, sources, arguments, None, &out, &[])
}

/// `build_and_run`, with an object to link and the program's path.
fn build_and_run_with(
    name: &str,
    sources: &[&str],
    arguments: &[&str],
    object: Option<&Path>,
    out: &Path,
    environment: &[(&str, &str)],
) -> String {
    let sysroot = sysroot();
    let libc = sysroot.join("usr/aarch64-linux-gnu");
    let gcc = sysroot.join("usr/lib/gcc-cross/aarch64-linux-gnu/15");
    let qb = root().join("runtime/qb");
    let mut build = Command::new("clang");
    build.args(["--target=aarch64-linux-gnu", "-fuse-ld=lld", "-static", "-std=gnu11", "-w", "-g", "-pthread"]);
    // the root, not the libc directory: libm.a is a linker script that names
    // /usr/aarch64-linux-gnu/lib/libm-2.42.a
    build.arg(format!("--sysroot={}", sysroot.display())).arg(format!("-B{}", gcc.display()));
    build
        .arg(format!("-L{}", gcc.display()))
        .arg(format!("-L{}", libc.join("lib").display()))
        .arg("-isystem")
        .arg(libc.join("include"))
        .arg("-D__cdecl=");
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
    let done = Command::new("qemu-aarch64")
        .arg(out)
        .args(arguments)
        .envs(environment.iter().copied())
        .output()
        .expect("qemu-aarch64 runs");
    std::fs::remove_file(out).ok();
    let printed = String::from_utf8_lossy(&done.stdout).into_owned();
    // 124: the headless driver ended a program that was still waiting, with the
    // screen it had (QB_HEADLESS_SECONDS)
    assert!(
        done.status.success() || done.status.code() == Some(124),
        "{name} failed:\n{printed}{}",
        String::from_utf8_lossy(&done.stderr)
    );
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

/// The dirty-line protocol under a writer and a presenter thread
/// (tests/qb_dirty.rs); qemu orders memory strongly on an x86 host, so a tear
/// shows only on arm64 hardware.
#[test]
fn the_presenter_never_sees_a_torn_line_on_arm64() {
    let sources = [
        "tests/qbhost/dirty_stress.c",
        "runtime/qb/dev/host/video.c",
        "runtime/qb/dev/host/vgadata.c",
        "runtime/qb/dev/host/tick.c",
        "runtime/shared/posix/os.c",
    ];
    assert_eq!(build_and_run("dirty_stress", &sources, &[]).trim(), "ok");
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

/// Compiles `source` for arm64 Linux (`llrm-qb --arm64=linux`), links it with
/// the whole host runtime on the headless device and runs it under qemu; the
/// text its screen shows.
fn run_basic(
    name: &str,
    source: &[u8],
) -> String {
    run_basic_with(name, source, &[])
}

/// `run_basic`, with environment variables for the headless driver
/// (runtime/qb/dev/headless/main.c).
fn run_basic_with(
    name: &str,
    source: &[u8],
    environment: &[(&str, &str)],
) -> String {
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
            "runtime/qb/posix/blockentries.c",
            "runtime/qb/posix/onerror_host.c",
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
    let printed = build_and_run_with(name, &sources, &[], Some(&object), &program, environment);
    std::fs::remove_dir_all(&dir).ok();
    printed
}

/// `PRINT "HELLO"` through llrm-qb's own arm64 code: the first QB program to
/// run on arm64. Before `--arm64` the compiler had no route to the arm64
/// backend at all.
#[test]
fn print_hello_runs_on_arm64() {
    assert_eq!(run_basic("hello", b"PRINT \"HELLO\"\nEND\n").trim_end(), "HELLO");
}

/// NIBBLES (QB 4.5's demo, from `QB45_DEMOS_DIR`) typed through its questions
/// on arm64: its READ/DATA colour table, the SINGLE and DOUBLE arithmetic of
/// its speed calibration, INPUT and the graphics-free screen. The level screen
/// it reaches says "Level 1, Push Space".
#[test]
fn nibbles_reaches_its_first_level_on_arm64() {
    let Some(dir) = std::env::var_os("QB45_DEMOS_DIR") else {
        eprintln!("QB45_DEMOS_DIR is not set: NIBBLES is not in the tree");
        return;
    };
    let source = std::fs::read(Path::new(&dir).join("NIBBLES.BAS")).expect("NIBBLES.BAS");
    let screen = run_basic_with(
        "nibbles",
        &source,
        &[("QB_HEADLESS_KEYS", " 1\r50\rY\rC\r"), ("QB_HEADLESS_GAP", "300"), ("QB_HEADLESS_SECONDS", "10")],
    );
    assert!(screen.contains("Level 1,  Push Space"), "{screen}");
}

/// GORILLA (QB 4.5's demo) typed through its intro and four questions on arm64:
/// SparklePause's INKEY$ loop, INPUT of SINGLE and DOUBLE values and the menu
/// text. A shot in the city (SCREEN 9, floats) was checked by eye, not here.
#[test]
fn gorilla_reaches_its_menu_on_arm64() {
    let Some(dir) = std::env::var_os("QB45_DEMOS_DIR") else {
        eprintln!("QB45_DEMOS_DIR is not set: GORILLA is not in the tree");
        return;
    };
    let source = std::fs::read(Path::new(&dir).join("GORILLA.BAS")).expect("GORILLA.BAS");
    let screen = run_basic_with(
        "gorilla",
        &source,
        &[("QB_HEADLESS_KEYS", "   \r\r\r\r\r\r"), ("QB_HEADLESS_GAP", "1200"), ("QB_HEADLESS_SECONDS", "30")],
    );
    assert!(screen.contains("Your Choice?"), "{screen}");
}

/// `ON ERROR GOTO` landed nowhere on arm64: `ERROR 5` ended the program (exit
/// 1) before the handler printed anything. The handler now runs, RESUME NEXT
/// continues after the faulting statement, and the module handler also takes an
/// error raised in a procedure that has none.
#[test]
fn on_error_lands_in_its_handler_on_arm64() {
    let module = b"ON ERROR GOTO h\r\nERROR 5\r\nPRINT \"after\"\r\nEND\r\nh:\r\nPRINT \"err\"; ERR\r\nRESUME NEXT\r\n";
    assert_eq!(run_basic("onerror", module).split_whitespace().collect::<Vec<_>>(), ["err", "5", "after"]);
    let procedure = b"DECLARE SUB s ()\r\nON ERROR GOTO h\r\ns\r\nPRINT \"back\"\r\nEND\r\nh:\r\nPRINT \"mod\"; ERR\r\nRESUME NEXT\r\nSUB s\r\nERROR 7\r\nPRINT \"in s\"\r\nEND SUB\r\n";
    assert_eq!(
        run_basic("onerror_sub", procedure).split_whitespace().collect::<Vec<_>>(),
        ["mod", "7", "in", "s", "back"]
    );
}

/// A handled function whose first block after the entry is a loop header has a
/// phi with an edge from the entry; the landing split the entry and the phi
/// named a block that no longer branched there ("a phi with no value for an
/// edge", GORILLA's PlaceGorillas).
#[test]
fn a_handled_function_may_start_with_a_loop_on_arm64() {
    let source = b"DEFINT A-Z\r\nON ERROR GOTO h\r\nFOR i = 1 TO 3\r\nPRINT i;\r\nNEXT\r\nERROR 9\r\nPRINT \"done\"\r\nEND\r\nh:\r\nRESUME NEXT\r\n";
    assert_eq!(run_basic("onerror_loop", source).split_whitespace().collect::<Vec<_>>(), ["1", "2", "3", "done"]);
}

/// Darwin's AAPCS64 has the caller extend narrow arguments, and `PRINT -1`
/// passed `i16 -1` bare: the clang-built runtime read 65535 and NIBBLES died on
/// VIEW PRINT with "Illegal function call" on the Mac. Only a Mac can run it;
/// this reads the text the backend gets: Darwin marks the runtime's `i16`
/// arguments `signext` in the declaration, Linux (callee extends) does not.
#[test]
fn darwin_callers_extend_narrow_arguments_to_the_runtime() {
    let dir = std::env::temp_dir().join(format!("signext_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bas = dir.join("v.bas");
    std::fs::write(&bas, b"DEFINT A-Z\r\nVIEW PRINT 1 TO 24\r\nPRINT -1\r\nEND\r\n").unwrap();
    let text = |platform: &str| {
        let path = dir.join(format!("{platform}.txt"));
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
            .args([&format!("--arm64={platform}"), "-O2"])
            .arg(&bas)
            .arg("-o")
            .arg(dir.join(format!("{platform}.o")))
            .env("LLRM_ARM64_TEXT", &path)
            .output()
            .expect("llrm-qb runs");
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        std::fs::read_to_string(path).unwrap()
    };
    // The caller extends a 16-bit argument wherever the word is wider (the HIR
    // marks the call); Darwin adds the mark to the declaration and to any
    // call the HIR left bare, and none twice.
    let darwin = text("darwin");
    assert!(darwin.contains("@B_PEI2(i16 signext -1)") && !darwin.contains("signext signext"), "{darwin}");
    assert!(darwin.contains("declare void @B_PEI2(i16 signext)"), "{darwin}");
    let linux = text("linux");
    assert!(linux.contains("declare void @B_PEI2(i16)"), "{linux}");
    std::fs::remove_dir_all(&dir).ok();
}

/// `INT((row + 1) / 2)` with a SINGLE `row` took the INTEGER shortcut for INT(n
/// / constant), set the `+ 1` aside and never put it back for a float `n`: `row
/// = 3` printed 1, not 2 (NIBBLES' arena rows were off, on every target).
#[test]
fn int_of_a_single_sum_over_a_constant_keeps_the_sum() {
    let source = b"row = 3\r\nPRINT INT((row + 1) / 2); INT((row - 1) / 2); INT((row + 1) / 4)\r\nEND\r\n";
    assert_eq!(run_basic("intsum", source).split_whitespace().collect::<Vec<_>>(), ["2", "1", "1"]);
}
