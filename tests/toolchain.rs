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

/// A flat target's block clears and copies are `rep stos`/`rep movs` on dwords through DS=ES, as code16's
/// are through ES: the lowering took segment operands and 16-bit counts (a departure row), and a constant
/// fill or copy on code32 was a loop. Neither sets a segment register here.
#[test]
fn test_code32_block_operations_are_rep_string_instructions_without_segments() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("m.c");
    std::fs::write(&source, "char a[300], b[300];\nvoid clear(void) { unsigned i; for (i = 0; i < 300; i++) a[i] = 0; }\nvoid copy(void) { int i; for (i = 0; i < 300; i++) b[i] = a[i]; }\nint main(void) { clear(); copy(); return b[5]; }\n").unwrap();
    let listing = |target: &str| {
        let out = scratch.path().join(format!("{target}.asm"));
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-c")).arg(&source).args(["--target", target, "-O2", "-S", "-o", out.to_str().unwrap()]).output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        std::fs::read_to_string(out).unwrap()
    };
    let flat = listing("x86-code32");
    assert!(flat.contains("rep stosd") && flat.contains("rep movsd"), "{flat}");
    assert!(!flat.contains("DGROUP") && flat.lines().all(|line| !matches!(line.trim(), "pop es" | "push es") && !line.trim().ends_with(", es")), "{flat}");
    let real = listing("x86-code16");
    assert!(real.contains("rep stosd") && real.lines().any(|line| line.trim() == "pop es"), "{real}");
}

/// A program the repository ships as an example or a benchmark compiles without a warning for each
/// target it runs on (`# targets:` names the ones it does not): a warning there is a lesson the
/// example teaches wrongly, and the flat targets' warnings (far and huge are near, usize narrowing)
/// would otherwise go unseen in a corpus nobody reads the stderr of.
#[test]
fn test_the_examples_and_benchmarks_compile_without_warnings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for source in ["examples", "bench"] {
        let mut pending = vec![root.join(source)];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|one| one == "nib") {
                    files.push(path);
                }
            }
        }
    }
    assert!(files.len() > 40, "{} programs found", files.len());
    let warned = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for (at, file) in files.iter().enumerate() {
            let (warned, object) = (&warned, scratch.path().join(format!("p{at}.obj")));
            scope.spawn(move || {
                let text = std::fs::read_to_string(file).unwrap();
                let header = text.lines().take_while(|line| line.starts_with('#')).find_map(|line| line.trim_start_matches('#').trim().strip_prefix("targets:"));
                let targets: Vec<&str> = header.map_or(vec!["x86-code16", "x86-code32"], |list| list.split_whitespace().take(1).collect());
                for target in targets {
                    let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib")).arg(file).args(["--target", target, "-O2", "-o", object.to_str().unwrap()]).output().unwrap();
                    let stderr = String::from_utf8_lossy(&done.stderr);
                    // A program that needs a library (link:) or refuses on a target is not this test's business.
                    if done.status.success() && stderr.contains("warning") {
                        warned.lock().unwrap().push(format!("{} [{target}]: {}", file.strip_prefix(root).unwrap().display(), stderr.lines().next().unwrap_or("")));
                    }
                }
            });
        }
    });
    let warned = warned.into_inner().unwrap();
    assert!(warned.is_empty(), "{}", warned.join("\n"));
}

/// The Zed extension is built apart from the workspace, so nothing compiled it: a refactor moved its
/// library path to a file that is not there, and it carries no way to name the project's target to nib-lsp.
/// Its manifest's library exists, and it passes the `initialization_options` setting to the server.
#[test]
fn test_the_zed_extension_names_a_library_that_exists_and_passes_the_projects_target() {
    let zed = Path::new(env!("CARGO_MANIFEST_DIR")).join("editors/zed");
    let manifest = std::fs::read_to_string(zed.join("Cargo.toml")).unwrap();
    let library = manifest.lines().find_map(|line| line.trim().strip_prefix("path = \"")).and_then(|rest| rest.strip_suffix('"')).expect("a library path");
    let source = std::fs::read_to_string(zed.join(library)).unwrap_or_else(|_| panic!("{library} is not in editors/zed"));
    assert!(source.contains("fn language_server_initialization_options") && source.contains("settings.initialization_options"));
}

/// The file calls' result was an `i32` on code16 and an `isize` on code32, so a program naming the type
/// was written for one target; both OS layers declare the same one.
#[test]
fn test_both_targets_declare_the_same_file_call_result() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let results = |target: &str| -> Vec<String> {
        let text = std::fs::read_to_string(root.join(format!("crates/target/llrm-{target}/runtime/nib/os.nib"))).unwrap();
        ["pub fn read(", "pub fn write_file("].iter().map(|head| text.lines().find(|line| line.starts_with(head)).and_then(|line| line.rsplit_once("-> ")).map(|(_, result)| result.trim().to_owned()).expect("declared")).collect()
    };
    assert_eq!(results("x86-code16"), ["i32", "i32"]);
    assert_eq!(results("x86-code32"), results("x86-code16"));
}

/// start.asm and dos.asm each named a constant of their own (the stack, the heap's arena) beside the
/// description's; the assembler is now told the description's fields, and a target that lists none is told none.
#[test]
fn test_the_assembler_is_told_the_runtime_descriptions_fields() {
    let defines = |target: &str| {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib")).args(["--target", target, "--os-layer", "defines"]).output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stdout).trim().to_owned()
    };
    assert_eq!(defines("x86-code32"), "STACK_BYTES=16384 HEAP_BYTES=16777216");
    assert_eq!(defines("x86-code16"), "");
}

/// The identity gate is an instrument: a build compared with itself must say SAME of every
/// program, and a build whose output differs must be reported DIFF with a failing exit, or a
/// change that moved a target's code would pass the gate silently.
#[test]
fn test_the_identity_gate_passes_a_build_against_itself_and_fails_a_different_one() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let compiler = env!("CARGO_BIN_EXE_llrm-c");
    let different = scratch.path().join("llrm-c-os");
    std::fs::write(&different, format!("#!/bin/sh\nexec {compiler} \"$@\" -Os\n")).unwrap();
    std::fs::set_permissions(&different, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let gate = |new: &Path| Command::new(root.join("tools/identity.sh")).args(["c", compiler, new.to_str().unwrap()]).env("TMPDIR", scratch.path()).output().unwrap();
    let same = gate(Path::new(compiler));
    let same_text = String::from_utf8_lossy(&same.stdout);
    assert!(same.status.success() && same_text.contains("SAME") && !same_text.contains("DIFF"), "{same_text}");
    let other = gate(&different);
    let other_text = String::from_utf8_lossy(&other.stdout);
    assert!(!other.status.success() && other_text.contains("DIFF "), "{other_text}");
}
