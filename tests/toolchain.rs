//! The tools build.rs puts beside llrm's binaries.

use std::path::Path;
use std::process::Command;

/// A fresh checkout had no jwasm or jwlink; then cargo's DEBUG made their
/// makefiles build into GccUnixD and the build script failed.
#[test]
fn test_jwasm_and_jwlink_are_built_beside_llrm() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let start = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/frontends/llrm-nib/src/runtime/start.asm");
    let assembled = Command::new(bin.join("jwasm"))
        .args(["-q", "-c", "-Cp", "-Zg", "-omf"])
        .arg(format!("-Fo{}", scratch.path().join("START.OBJ").display()))
        .arg(start)
        .status()
        .expect("jwasm is in target/<profile>");
    assert!(assembled.success() && scratch.path().join("START.OBJ").exists());
    let linker = Command::new(bin.join("jwlink")).stdin(std::process::Stdio::null()).output().expect("jwlink is in target/<profile>");
    assert!(String::from_utf8_lossy(&linker.stdout).contains("JWlink"));
}

/// The dosrun DOSBox-X only built on macOS (pthread_threadid_np, a missing
/// headless SDL_SetWindowIcon, archive order GNU ld rejects), so no e2e test
/// could run elsewhere.
#[test]
fn test_a_jwlink_exe_runs_under_the_built_dosbox() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    std::fs::write(
        dir.join("HELLO.ASM"),
        ".model small\n.stack 256\n.data\nmsg db 'dosrun', 13, 10, '$'\n.code\nstart: mov ax, @data\n\
         mov ds, ax\nmov dx, offset msg\nmov ah, 9\nint 21h\nmov ax, 4C00h\nint 21h\nend start\n",
    )
    .unwrap();
    let run = |program: &str, args: &[&str]| {
        assert!(Command::new(bin.join(program)).args(args).current_dir(dir).status().unwrap().success(), "{program}");
    };
    run("jwasm", &["-q", "-omf", "-FoHELLO.OBJ", "HELLO.ASM"]);
    run("jwlink", &["format", "dos", "file", "HELLO.OBJ", "name", "HELLO.EXE", "op", "quiet"]);
    let conf = format!("[autoexec]\nmount c {}\nc:\nHELLO > OUT.TXT\nexit\n", dir.display());
    std::fs::write(dir.join("dosbox.conf"), conf).unwrap();
    let conf = dir.join("dosbox.conf");
    let conf = conf.to_str().unwrap();
    run("dosbox-x", &["-nolog", "-exit", "-conf", conf]);
    assert_eq!(std::fs::read_to_string(dir.join("OUT.TXT")).unwrap(), "dosrun\r\n");
}

/// nib-build.sh named the generated header `main.nbl.h`, stripping `.mod`,
/// so geometry.c's `#include "main.h"` found no CPoint and the interop
/// example did not build.
#[test]
fn test_nib_build_links_the_interop_example_with_its_c_library() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-nib")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let exe = scratch.path().join("INTEROP.EXE");
    let example = root.join("examples/interop");
    let status = Command::new(root.join("tools/nib-build.sh"))
        .arg(example.join("main.nib"))
        .arg(&exe)
        .arg("-O2")
        .arg(example.join("geometry.c"))
        .env("TOOLCHAIN", bin)
        .status()
        .unwrap();
    assert!(status.success(), "nib-build.sh");
    assert!(exe.exists());
}

/// start.asm left SS at the STACK segment, not DGROUP, though the machine
/// says the stack is data: isel's code reached a frame array's cells
/// through DS, and summing a slice of one returned 0, not 5.
#[test]
fn test_nib_start_puts_the_stack_in_dgroup() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-nib")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let run = |program: &Path, args: &[&str]| {
        assert!(Command::new(program).args(args).current_dir(dir).status().unwrap().success(), "{}", program.display());
    };
    let runtime = root.join("crates/frontends/llrm-nib/src/runtime");
    for part in ["start", "dos"] {
        run(&bin.join("jwasm"), &["-q", "-c", "-Cp", "-Zg", "-omf", &format!("-Fo{part}.obj"), runtime.join(format!("{part}.asm")).to_str().unwrap()]);
    }
    // The divide fault's handler, which runtime.nib otherwise supplies.
    std::fs::write(dir.join("fault.asm"), ".model medium\n.code\npublic N$EDIV\nN$EDIV proc far\nmov ax, 4c63h\nint 21h\nN$EDIV endp\nend\n").unwrap();
    run(&bin.join("jwasm"), &["-q", "-c", "-Cp", "-omf", "-Fofault.obj", "fault.asm"]);
    let slice = root.join("tests/fixtures/nib/port/c7e7588fa1/slice.nib");
    run(&bin.join("llrm-nib"), &[slice.to_str().unwrap(), "-o", "slice.obj"]);
    run(&bin.join("jwlink"), &["format", "dos", "name", "SLICE.EXE", "file", "start.obj", "file", "slice.obj", "file", "dos.obj", "file", "fault.obj", "op", "quiet"]);
    let conf = format!(
        "[autoexec]\nmount c {}\nc:\nSLICE\nif errorlevel 6 goto other\nif errorlevel 5 goto five\n:other\necho other > OUT.TXT\ngoto end\n:five\necho 5 > OUT.TXT\n:end\nexit\n",
        dir.display()
    );
    std::fs::write(dir.join("dosbox.conf"), conf).unwrap();
    run(&bin.join("dosbox-x"), &["-nolog", "-exit", "-conf", dir.join("dosbox.conf").to_str().unwrap()]);
    assert_eq!(std::fs::read_to_string(dir.join("OUT.TXT")).unwrap(), "5\r\n");
}

/// start.asm's stack was 512 bytes: bench matmul's three 256-byte arrays
/// ran below it, over DGROUP's data, and the program hung or printed
/// garbage on either code path.
#[test]
fn test_nib_start_leaves_a_kilobyte_frame_room() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-nib")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let run = |program: &Path, args: &[&str]| {
        assert!(Command::new(program).args(args).current_dir(dir).status().unwrap().success(), "{}", program.display());
    };
    let runtime = root.join("crates/frontends/llrm-nib/src/runtime");
    for part in ["start", "dos"] {
        run(&bin.join("jwasm"), &["-q", "-c", "-Cp", "-Zg", "-omf", &format!("-Fo{part}.obj"), runtime.join(format!("{part}.asm")).to_str().unwrap()]);
    }
    std::fs::write(dir.join("fault.asm"), ".model medium\n.code\npublic N$EDIV\nN$EDIV proc far\nmov ax, 4c63h\nint 21h\nN$EDIV endp\nend\n").unwrap();
    run(&bin.join("jwasm"), &["-q", "-c", "-Cp", "-omf", "-Fofault.obj", "fault.asm"]);
    std::fs::write(
        dir.join("frame.nib"),
        "var marker: u16 = 5\n\nfn main() -> i16:\n    let mut cells: u16[600] = [0] * 600\n    for at in 0..600:\n        cells[at] = u16(at)\n    \
         let mut total: u16 = 0\n    for at in 0..600:\n        total += cells[599 - at]\n    return total == 48628 ? i16(marker) : 99\n",
    )
    .unwrap();
    run(&bin.join("llrm-nib"), &["frame.nib", "-o", "frame.obj"]);
    run(&bin.join("jwlink"), &["format", "dos", "name", "FRAME.EXE", "file", "start.obj", "file", "frame.obj", "file", "dos.obj", "file", "fault.obj", "op", "quiet"]);
    let conf = format!(
        "[autoexec]\nmount c {}\nc:\nFRAME\nif errorlevel 6 goto other\nif errorlevel 5 goto five\n:other\necho other > OUT.TXT\ngoto end\n:five\necho 5 > OUT.TXT\n:end\nexit\n",
        dir.display()
    );
    std::fs::write(dir.join("dosbox.conf"), conf).unwrap();
    run(&bin.join("dosbox-x"), &["-nolog", "-exit", "-conf", dir.join("dosbox.conf").to_str().unwrap()]);
    assert_eq!(std::fs::read_to_string(dir.join("OUT.TXT")).unwrap(), "5\r\n");
}

/// llrm-c's two routes, the old MIR's (--opt) and the rich MIR's through
/// isel (the default), run each parity fixture to bench/parity/expected.json's
/// value: the gate for retiring the old route.
#[test]
fn test_c_parity_fixtures_agree_on_both_routes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let run = |program: &str, args: &[&str]| {
        let done = Command::new(bin.join(program)).args(args).current_dir(dir).output().unwrap();
        assert!(done.status.success(), "{program} {args:?}: {}", String::from_utf8_lossy(&done.stderr));
    };
    let parity = root.join("tests/fixtures/c/parity");
    let expected: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join("bench/parity/expected.json")).unwrap()).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&parity)
        .unwrap()
        .filter_map(|one| Some(one.unwrap().path().to_str()?.strip_suffix(".cgs")?.rsplit('/').next()?.to_owned()))
        .collect();
    names.sort();
    let mut autoexec = format!("[autoexec]\nmount c {}\nc:\n", dir.display());
    let mut runs = Vec::new();
    for (number, name) in names.iter().enumerate() {
        let start = parity.join(format!("{name}-start.asm"));
        run("jwasm", &["-q", "-c", "-Cp", "-Zg", "-omf", &format!("-Fo{name}_s.obj"), start.to_str().unwrap()]);
        for (route, flags) in [("O", &["--opt"][..]), ("I", &[][..])] {
            let program = format!("P{number}{route}");
            let source = parity.join(format!("{name}.cgs"));
            run("llrm-c", &[&[source.to_str().unwrap()][..], flags, &["-o", &format!("{program}.obj")]].concat());
            run("jwlink", &["format", "dos", "name", &format!("{program}.EXE"), "file", &format!("{name}_s.obj"), "file", &format!("{program}.obj"), "op", "quiet"]);
            autoexec += &format!("del VALUE.BIN\n{program}\ncopy VALUE.BIN {program}.BIN\n");
            runs.push((name.clone(), route, program));
        }
    }
    std::fs::write(dir.join("dosbox.conf"), autoexec + "exit\n").unwrap();
    run("dosbox-x", &["-nolog", "-exit", "-conf", dir.join("dosbox.conf").to_str().unwrap()]);
    for (name, route, program) in runs {
        let bytes = std::fs::read(dir.join(format!("{program}.BIN"))).unwrap_or_default();
        let value = bytes.get(..4).map(|word| i64::from(i32::from_le_bytes(word.try_into().unwrap())));
        assert_eq!(value, expected[&name].as_i64(), "{name} {route}");
    }
}

/// Ten arrays, near and far, summed over one counter on a P5: loop strength
/// reduction kept four products live beside the counter and the allocator
/// found "value cannot be spilled and no register is free", where main built
/// it. A far access takes a register for its selector that the pass did not
/// count (three loop-corpus cases of `conc10`/`conc11`).
#[test]
fn test_a_loop_over_ten_arrays_builds_on_a_p5() {
    let scratch = tempfile::tempdir().unwrap();
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c"));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/tenarrays.c");
    let done = Command::new(bin).args([source.to_str().unwrap(), "--cpu", "P5", "-O2", "-o", "tenarrays.obj"]).current_dir(scratch.path()).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}
