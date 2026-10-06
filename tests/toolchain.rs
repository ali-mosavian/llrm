//! The tools build.rs puts beside llrm's binaries.

use std::path::Path;
use std::process::Command;

/// A fresh checkout had no jwasm or jwlink; then cargo's DEBUG made their
/// makefiles build into GccUnixD and the build script failed.
#[test]
fn test_jwasm_and_jwlink_are_built_beside_llrm() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let start = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/target/llrm-x86-code16/runtime/nib/start.asm");
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
    let runtime = root.join("crates/target/llrm-x86-code16/runtime/nib");
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
    let runtime = root.join("crates/target/llrm-x86-code16/runtime/nib");
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

/// llrm-c runs each parity fixture to bench/parity/expected.json's value.
#[test]
fn test_c_parity_fixtures_compute_their_expected_values() {
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
        let program = format!("P{number}");
        let source = parity.join(format!("{name}.cgs"));
        run("llrm-c", &[source.to_str().unwrap(), "-o", &format!("{program}.obj")]);
        run("jwlink", &["format", "dos", "name", &format!("{program}.EXE"), "file", &format!("{name}_s.obj"), "file", &format!("{program}.obj"), "op", "quiet"]);
        autoexec += &format!("del VALUE.BIN\n{program}\ncopy VALUE.BIN {program}.BIN\n");
        runs.push((name.clone(), program));
    }
    std::fs::write(dir.join("dosbox.conf"), autoexec + "exit\n").unwrap();
    run("dosbox-x", &["-nolog", "-exit", "-conf", dir.join("dosbox.conf").to_str().unwrap()]);
    for (name, program) in runs {
        let bytes = std::fs::read(dir.join(format!("{program}.BIN"))).unwrap_or_default();
        let value = bytes.get(..4).map(|word| i64::from(i32::from_le_bytes(word.try_into().unwrap())));
        assert_eq!(value, expected[&name].as_i64(), "{name}");
    }
}

/// `fixture`.cgs through llrm-c and jwlink with a start-up object that names
/// `entry`; the linker's complaint if it refuses.
fn linked_fixture(fixture: &str, entry: &str) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    std::fs::write(dir.join("START.ASM"), format!(".model medium\n.stack 256\nextrn {entry}:far\n.code\nstart: call far ptr {entry}\nmov ax, 4C00h\nint 21h\nend start\n")).unwrap();
    let run = |program: &str, args: &[&str]| {
        let done = Command::new(bin.join(program)).args(args).current_dir(dir).output().unwrap();
        if done.status.success() { Ok(()) } else { Err(format!("{program}: {}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr))) }
    };
    run("jwasm", &["-q", "-omf", "-FoSTART.OBJ", "START.ASM"])?;
    let source = root.join(format!("tests/fixtures/c/{fixture}.cgs"));
    run("llrm-c", &[source.to_str().unwrap(), "-O2", "--cpu", "486", "-o", "P.OBJ"])?;
    run("jwlink", &["format", "dos", "name", "P.EXE", "file", "START.OBJ", "file", "P.OBJ", "op", "quiet"])
}

/// A far segment was word aligned: after `odd` (3 bytes) the next far
/// segment held a 64K array at offset 4 of its frame, and jwlink refused it:
/// "E2021: size of segment exceeds 64k by 8 bytes" (#102).
#[test]
fn test_a_64k_far_array_after_an_odd_sized_far_segment_links() {
    let done = linked_fixture("farsegments", "_get");
    assert!(done.is_ok(), "{}", done.unwrap_err());
}

/// A `__huge` global past 64K stopped llrm-c: "a14_DATA data before any
/// label", the second segment of its 80000 bytes (#101).
#[test]
fn test_a_huge_global_past_64k_compiles_and_links_across_two_segments() {
    let done = linked_fixture("hugeglobal", "_first");
    assert!(done.is_ok(), "{}", done.unwrap_err());
}

/// Nine to eleven arrays, near and far, summed over one counter on a P5 or Core: loop
/// strength reduction kept four products live beside the counter and the
/// allocator found "value cannot be spilled and no register is free", where
/// main built them. A far access takes registers the pass did not count:
/// one still failed the nine, two the eleven on a Core (loop-corpus
/// `conc9`, `conc10`, `conc11`).
#[test]
fn test_loops_over_many_arrays_build_on_a_p5() {
    let scratch = tempfile::tempdir().unwrap();
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c"));
    for (name, cpu) in [("ninearrays", "P5"), ("tenarrays", "P5"), ("elevenarrays", "Core")] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/c/{name}.c"));
        let done = Command::new(bin).args([source.to_str().unwrap(), "--cpu", cpu, "-O2", "-o", &format!("{name}.obj")]).current_dir(scratch.path()).output().unwrap();
        assert!(done.status.success(), "{name}: {}", String::from_utf8_lossy(&done.stderr));
    }
}

/// Five pointer streams on a P5: a reload confined to BX lost its register to
/// a split after it was placed, and with eviction only tried at `Assign` and
/// blocked by a younger cascade, failed "cannot be spilled and no register is
/// free" (loop-corpus `rnd98_0183`).
#[test]
fn test_an_unspillable_range_evicts_a_spillable_holder_at_any_stage() {
    let scratch = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/fivestreams.c");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .args([source.to_str().unwrap(), "--cpu", "P5", "-O2", "-o", "fivestreams.obj"])
        .current_dir(scratch.path())
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

/// Seven arrays of different element sizes summed over one symbolic count on
/// a 386: the pass assumed a dword counter takes a scaled address, the
/// selector scales it only where its range keeps it in a word, so the loop
/// computed `i*2`, `i*4` and `i*8` into frame cells each trip (`shl dword ptr
/// [bp-22], 1`), 40 instructions where main's walked pointers took 24
/// (loop-corpus `rnd98_0305`).
#[test]
fn test_a_loop_over_many_arrays_keeps_no_product_in_a_frame_cell() {
    let scratch = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c/tenstreams.c");
    let listing = scratch.path().join("tenstreams.asm");
    let done = Command::new(Path::new(env!("CARGO_BIN_EXE_llrm-c"))).args([source.to_str().unwrap(), "--cpu", "386", "-O2", "-S", "-o", listing.to_str().unwrap()]).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(listing).unwrap();
    assert!(!asm.contains("shl dword ptr [bp"), "{asm}");
}

/// A `__huge` array of 80000 bytes, run on DOS: each routine reads or writes
/// where a 16-bit offset that wraps at 64K would give another element, and a
/// huge pointer's loop end, difference and step must carry into the selector
/// by the DOS stride (#101).
#[test]
fn test_a_huge_array_past_64k_reads_and_writes_the_right_elements_on_dos() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let routines = [("_hfill", 59_998), ("_hsumidx", 599_990_000), ("_hsumptr", 599_990_000), ("_hdiff", 19_989), ("_hmid", 51_001)];
    let mut start = String::from(".model medium\n.386\n");
    for (name, _) in routines {
        start += &format!("extrn {name}:far\n");
    }
    start += &format!(".data\nvalues dd {} dup (?)\nfilename db 'VALUES.BIN', 0\nstack_space db 1024 dup (?)\nstack_top label byte\n.code\nstart:\n    mov ax, @data\n    mov ds, ax\n    cli\n    mov ss, ax\n    mov sp, offset stack_top\n    sti\n    fninit\n", routines.len());
    for (at, (name, _)) in routines.iter().enumerate() {
        start += &format!("    call far ptr {name}\n    mov word ptr values+{}, ax\n    mov word ptr values+{}, dx\n", at * 4, at * 4 + 2);
    }
    start += &format!("    mov ah, 3ch\n    xor cx, cx\n    lea dx, filename\n    int 21h\n    mov bx, ax\n    mov ah, 40h\n    mov cx, {}\n    lea dx, values\n    int 21h\n    mov ax, 4c00h\n    int 21h\nend start\n", routines.len() * 4);
    std::fs::write(dir.join("START.ASM"), start).unwrap();
    let run = |program: &str, args: &[&str]| {
        let done = Command::new(bin.join(program)).args(args).current_dir(dir).output().unwrap();
        assert!(done.status.success(), "{program}: {}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr));
    };
    run("jwasm", &["-q", "-c", "-Cp", "-Zg", "-omf", "-FoSTART.OBJ", "START.ASM"]);
    let source = root.join("tests/fixtures/c/hugearray.cgs");
    run("llrm-c", &[source.to_str().unwrap(), "-O2", "--cpu", "486", "-o", "P.OBJ"]);
    run("jwlink", &["format", "dos", "name", "P.EXE", "file", "START.OBJ", "file", "P.OBJ", "op", "quiet"]);
    std::fs::write(dir.join("dosbox.conf"), format!("[autoexec]\nmount c {}\nc:\nP\nexit\n", dir.display())).unwrap();
    run("dosbox-x", &["-nolog", "-exit", "-conf", dir.join("dosbox.conf").to_str().unwrap()]);
    let bytes = std::fs::read(dir.join("VALUES.BIN")).unwrap_or_default();
    let got: Vec<i64> = bytes.chunks_exact(4).map(|word| i64::from(i32::from_le_bytes(word.try_into().unwrap()))).collect();
    let want: Vec<i64> = routines.iter().map(|(_, value)| *value).collect();
    assert_eq!(got, want, "{:?}", routines.map(|(name, _)| name));
}

/// Far and huge are near where a target has one address space, and the compiler says so, once for each
/// place: code16 and a flat program that writes neither stay silent, and `-Wno-target-width` (the runtime's
/// build, which writes `*far` for the targets that have one) silences it.
#[test]
fn test_far_and_huge_are_near_with_a_warning_on_code32() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("p.obj");
    let stderr = |source: &str, extra: &[&str]| {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib")).arg(root.join(source)).args(extra).args(["-O2", "-o", object.to_str().unwrap()]).output().unwrap();
        assert!(done.status.success(), "{source}: {}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stderr).into_owned()
    };
    let flat = ["--target", "x86-code32"];
    let warned = stderr("tests/run/nib/far_near.nib", &flat);
    assert!(warned.contains("far_near.nib:5:6: warning: 'huge' is near") && warned.contains("warning: 'far' is near"), "{warned}");
    assert_eq!(stderr("tests/run/nib/far_near.nib", &[]), "");
    assert_eq!(stderr("tests/run/nib/far_near.nib", &["--target", "x86-code32", "-Wno-target-width"]), "");
    assert_eq!(stderr("tests/run/nib/flat_arith.nib", &flat), "");
}

/// C's far and huge are near with a warning where the target has one address space, and an unmarked
/// pointer is near in every model: `-ml` (default data pointers far) is not a switch.
#[test]
fn test_c_far_and_huge_are_near_with_a_warning_on_code32() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("p.obj");
    let compile = |extra: &[&str]| Command::new(env!("CARGO_BIN_EXE_llrm-c")).arg(root.join("tests/run/c/huge_array.c")).args(extra).args(["-O2", "-o", object.to_str().unwrap()]).output().unwrap();
    let flat = compile(&["--target", "x86-code32"]);
    assert!(flat.status.success(), "{}", String::from_utf8_lossy(&flat.stderr));
    assert!(String::from_utf8_lossy(&flat.stderr).contains("warning: __far and __huge pointers are near on this target"), "{}", String::from_utf8_lossy(&flat.stderr));
    let real = compile(&[]);
    assert!(real.status.success() && real.stderr.is_empty(), "{}", String::from_utf8_lossy(&real.stderr));
    let large = compile(&["-ml"]);
    assert!(!large.status.success() && String::from_utf8_lossy(&large.stderr).contains("unrecognized arguments: -ml"));
}

/// The C header Nib generates for a program's exports said `__far` on every target: flat C code
/// including it declared a far function and the flat compiler refused its call.
#[test]
fn test_the_generated_header_is_far_only_where_far_code_is() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let header = |extra: &[&str]| {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib")).arg(root.join("examples/interop/main.nib")).args(["--declare", "h"]).args(extra).output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stdout).into_owned()
    };
    assert!(header(&[]).contains("extern short __far __cdecl weight(short value);"));
    let flat = header(&["--target", "x86-code32"]);
    assert!(flat.contains("extern short __cdecl weight(short value);") && !flat.contains("__far"), "{flat}");
}
