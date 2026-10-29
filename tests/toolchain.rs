//! The tools build.rs puts beside llrm's binaries.

pub mod common;

use std::path::Path;
use std::process::Command;

/// A fresh checkout had no jwasm or jwlink; then cargo's DEBUG made their
/// makefiles build into GccUnixD and the build script failed.
#[test]
fn test_jwasm_and_jwlink_are_built_beside_llrm() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-cc")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    os_layer_objects(bin, scratch.path());
    assert!(scratch.path().join("start.obj").exists());
    let linker = Command::new(bin.join("jwlink"))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("jwlink is in target/<profile>");
    assert!(String::from_utf8_lossy(&linker.stdout).contains("JWlink"));
}

/// The real-mode OS layer assembled in `dir` as start.obj and os.obj, and Nib's
/// start-up hook as init.obj, told their descriptions' defines.
fn os_layer_objects(
    bin: &Path,
    dir: &Path,
) {
    let layer = |field: &str| {
        let done = Command::new(bin.join("llrm-nib")).args(["--os-layer", field]).output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stdout).trim().to_owned()
    };
    let directory = layer("directory");
    let defines: Vec<String> = layer("defines").split_whitespace().map(|one| format!("-D{one}")).collect();
    for (field, object) in [("start", "start.obj"), ("implementation", "os.obj"), ("language_file", "init.obj")] {
        let source = if field == "language_file" { layer(field) } else { format!("{directory}/{}", layer(field)) };
        let done = Command::new(bin.join("jwasm"))
            .args(["-q", "-c", "-Cp", "-Zg", common::assembler()])
            .args(&defines)
            .arg(format!("-Fo{object}"))
            .arg(source)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(done.success(), "{field}");
    }
}

/// The dosrun DOSBox-X only built on macOS (pthread_threadid_np, a missing
/// headless SDL_SetWindowIcon, archive order GNU ld rejects), so no e2e test
/// could run elsewhere.
#[test]
fn test_a_jwlink_exe_runs_under_the_built_dosbox() {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-cc")).parent().unwrap();
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
    run("jwasm", &["-q", common::assembler(), "-FoHELLO.OBJ", "HELLO.ASM"]);
    run("jwlink", &common::jwlink(&["file", "HELLO.OBJ", "name", "HELLO.EXE", "op", "quiet"]));
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
        .env("NIB_CC", "llrm-cc")
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
    os_layer_objects(&bin, dir);
    // The divide fault's handler, which runtime.nib otherwise supplies.
    std::fs::write(
        dir.join("fault.asm"),
        format!(
            "{}.code\npublic N$EDIV\nN$EDIV proc far\nmov ax, 4c63h\nint 21h\nN$EDIV endp\nend\n",
            common::header()
        ),
    )
    .unwrap();
    run(&bin.join("jwasm"), &["-q", "-c", "-Cp", common::assembler(), "-Fofault.obj", "fault.asm"]);
    let slice = root.join("tests/fixtures/nib/port/c7e7588fa1/slice.nib");
    run(&bin.join("llrm-nib"), &[slice.to_str().unwrap(), "-o", "slice.obj"]);
    run(
        &bin.join("jwlink"),
        &common::jwlink(&[
            "name",
            "SLICE.EXE",
            "file",
            "start.obj",
            "file",
            "init.obj",
            "file",
            "slice.obj",
            "file",
            "os.obj",
            "file",
            "fault.obj",
            "op",
            "quiet",
        ]),
    );
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
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-nib")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let run = |program: &Path, args: &[&str]| {
        assert!(Command::new(program).args(args).current_dir(dir).status().unwrap().success(), "{}", program.display());
    };
    os_layer_objects(&bin, dir);
    std::fs::write(
        dir.join("fault.asm"),
        format!(
            "{}.code\npublic N$EDIV\nN$EDIV proc far\nmov ax, 4c63h\nint 21h\nN$EDIV endp\nend\n",
            common::header()
        ),
    )
    .unwrap();
    run(&bin.join("jwasm"), &["-q", "-c", "-Cp", common::assembler(), "-Fofault.obj", "fault.asm"]);
    std::fs::write(
        dir.join("frame.nib"),
        "var marker: u16 = 5\n\nfn main() -> i16:\n    let mut cells: u16[600] = [0] * 600\n    for at in 0..600:\n        cells[at] = u16(at)\n    \
         let mut total: u16 = 0\n    for at in 0..600:\n        total += cells[599 - at]\n    return total == 48628 ? i16(marker) : 99\n",
    )
    .unwrap();
    run(&bin.join("llrm-nib"), &["frame.nib", "-o", "frame.obj"]);
    run(
        &bin.join("jwlink"),
        &common::jwlink(&[
            "name",
            "FRAME.EXE",
            "file",
            "start.obj",
            "file",
            "init.obj",
            "file",
            "frame.obj",
            "file",
            "os.obj",
            "file",
            "fault.obj",
            "op",
            "quiet",
        ]),
    );
    let conf = format!(
        "[autoexec]\nmount c {}\nc:\nFRAME\nif errorlevel 6 goto other\nif errorlevel 5 goto five\n:other\necho other > OUT.TXT\ngoto end\n:five\necho 5 > OUT.TXT\n:end\nexit\n",
        dir.display()
    );
    std::fs::write(dir.join("dosbox.conf"), conf).unwrap();
    run(&bin.join("dosbox-x"), &["-nolog", "-exit", "-conf", dir.join("dosbox.conf").to_str().unwrap()]);
    assert_eq!(std::fs::read_to_string(dir.join("OUT.TXT")).unwrap(), "5\r\n");
}

/// Nine to eleven arrays, near and far, summed over one counter on a P5 or
/// Core: loop strength reduction kept four products live beside the counter and
/// the allocator found "value cannot be spilled and no register is free", where
/// main built them. A far access takes registers the pass did not count:
/// one still failed the nine, two the eleven on a Core (loop-corpus
/// `conc9`, `conc10`, `conc11`).
#[test]
fn test_loops_over_many_arrays_build_on_a_p5() {
    let scratch = tempfile::tempdir().unwrap();
    for (name, cpu) in [("ninearrays", "P5"), ("tenarrays", "P5"), ("elevenarrays", "Core")] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/c/{name}.c"));
        let done = common::cc::llrm_cc()
            .args([source.to_str().unwrap(), &march(cpu), "-O2", "-o", &format!("{name}.obj")])
            .current_dir(scratch.path())
            .output()
            .unwrap();
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
    let done = common::cc::llrm_cc()
        .args([source.to_str().unwrap(), "-march=pentium", "-O2", "-o", "fivestreams.obj"])
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
    let done = common::cc::llrm_cc()
        .args([source.to_str().unwrap(), "-march=i386", "-O2", "-S", "-o", listing.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = std::fs::read_to_string(listing).unwrap();
    assert!(!asm.contains("shl dword ptr [bp"), "{asm}");
}

/// Far and huge are near where a target has one address space, and the compiler
/// says so, once for each place: m16 and a flat program that writes neither
/// stay silent, and `-Wno-target-width` (the runtime's build, which writes
/// `*far` for the targets that have one) silences it.
#[test]
fn test_far_and_huge_are_near_with_a_warning_on_m32() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("p.obj");
    let stderr = |source: &str, extra: &[&str]| {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .arg(root.join(source))
            .args(extra)
            .args(["-O2", "-o", object.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(done.status.success(), "{source}: {}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stderr).into_owned()
    };
    let flat = ["-m32"];
    let warned = stderr("tests/run/nib/far_near.nib", &flat);
    assert!(
        warned.contains("far_near.nib:5:6: warning: 'huge' is near") && warned.contains("warning: 'far' is near"),
        "{warned}"
    );
    assert_eq!(stderr("tests/run/nib/far_near.nib", &[]), "");
    assert_eq!(stderr("tests/run/nib/far_near.nib", &["-m32", "-Wno-target-width"]), "");
    assert_eq!(stderr("tests/run/nib/flat_arith.nib", &flat), "");
}

/// A huge array that is stored and read back made the layout take the far
/// segment assembly had built for the default one, so every near global was
/// listed twice (near and in the far segment) and far_near.nib printed 32755
/// for 36.
#[test]
fn test_nib_near_globals_are_listed_once_beside_a_huge_array() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let listing = scratch.path().join("p.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .arg(root.join("tests/run/nib/far_near.nib"))
        .args(["-O2", "-march=i486", "-S", "-o", listing.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(listing).unwrap();
    let labels: Vec<&str> = text.lines().filter(|line| line.starts_with("G$")).collect();
    let mut unique = labels.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(labels.len(), unique.len(), "{text}");
}

/// C's far and huge are near with a warning where the target has one address
/// space, and an unmarked pointer is near in every model: `-ml` (default data
/// pointers far) is not a switch.
///
/// llrm-cc: flat.h defines `__far` and `__huge` away, so clang sees plain
/// pointers and has nothing to warn about: the flat compile succeeds silently.
/// `-ml` is refused by clang ("unknown argument").
#[test]
fn test_c_far_and_huge_are_near_with_a_warning_on_m32() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("p.obj");
    let compile = |extra: &[&str]| {
        common::cc::llrm_cc()
            .arg(root.join("tests/run/c/huge_array.c"))
            .args(extra)
            .args(["-O2", "-o", object.to_str().unwrap()])
            .output()
            .unwrap()
    };
    let flat = compile(&["-m32"]);
    assert!(flat.status.success() && flat.stderr.is_empty(), "{}", String::from_utf8_lossy(&flat.stderr));
    let real = compile(&[]);
    assert!(real.status.success() && real.stderr.is_empty(), "{}", String::from_utf8_lossy(&real.stderr));
    let large = compile(&["-ml"]);
    assert!(!large.status.success() && String::from_utf8_lossy(&large.stderr).contains("unknown argument: '-ml'"));
}

/// The C header Nib generates for a program's exports said `__far` on every
/// target: flat C code including it declared a far function and the flat
/// compiler refused its call.
#[test]
fn test_the_generated_header_is_far_only_where_far_code_is() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let header = |extra: &[&str]| {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .arg(root.join("examples/interop/main.nib"))
            .args(["--declare", "h"])
            .args(extra)
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stdout).into_owned()
    };
    // An unmarked declaration is the target's default convention; a named ABI
    // says its own.
    assert!(header(&[]).contains("extern short __far weight(short value);"));
    assert!(header(&["-mabi=cdecl"]).contains("extern short __far __cdecl weight(short value);"));
    let flat = header(&["-m32"]);
    assert!(flat.contains("extern short __watcall weight(short value);") && !flat.contains("__far"), "{flat}");
}

/// A flat target's block clears and copies are `rep stos`/`rep movs` on dwords
/// through DS=ES, as m16's are through ES: the lowering took segment operands
/// and 16-bit counts (a departure row), and a constant fill or copy on m32 was
/// a loop. Neither sets a segment register here. A copy above the inline
/// threshold is the `memcpy` call, as in the pin (`llc -mtriple=i386 -O2`:
/// 64, 100 and 128 B at align 4 are `rep movsl`; 132 B and 300 B, at align 1
/// or 4, are `calll memcpy`; X86 `getMaxInlineSizeThreshold` is 128). This test
/// asserted `rep movsd` for a 300 B copy, which the call replaced. llrm-cc: the
/// 300 B clear is `call memset` (the pin's llc on the same C), the 100 B copy
/// `rep movsd`; m16 has no library to call and fills with `rep stosd` (the
/// dword fill of a multiple of 4).
#[test]
fn test_m32_block_operations_are_rep_string_instructions_without_segments() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("m.c");
    std::fs::write(&source, "char a[300], b[300];\nint c[25], d[25];\nvoid clear(void) { unsigned i; for (i = 0; i < 300; i++) a[i] = 0; }\nvoid copy(void) { int i; for (i = 0; i < 25; i++) d[i] = c[i]; }\nvoid copy_big(void) { int i; for (i = 0; i < 300; i++) b[i] = a[i]; }\nint main(void) { clear(); copy(); copy_big(); return b[5] + d[5]; }\n").unwrap();
    let listing = |target: &str| {
        let out = scratch.path().join(format!("{target}.asm"));
        let done = common::cc::llrm_cc()
            .arg(&source)
            .args([&mode(target), "-O2", "-S", "-o", out.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        std::fs::read_to_string(out).unwrap()
    };
    let flat = listing("x86-m32");
    assert!(flat.contains("call memset_") && flat.contains("rep movsd"), "{flat}");
    assert!(flat.contains("call memcpy_"), "{flat}");
    assert!(
        !flat.contains("DGROUP")
            && flat.lines().all(|line| !matches!(line.trim(), "pop es" | "push es") && !line.trim().ends_with(", es")),
        "{flat}"
    );
    let real = listing("x86-m16");
    assert!(real.contains("rep stosd") && real.lines().any(|line| line.trim() == "pop es"), "{real}");
}

/// A program the repository ships as an example or a benchmark compiles without
/// a warning for each target it runs on (`# targets:` names the ones it does
/// not): a warning there is a lesson the example teaches wrongly, and the flat
/// targets' warnings (far and huge are near, usize narrowing) would otherwise
/// go unseen in a corpus nobody reads the stderr of.
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
                let header = text
                    .lines()
                    .take_while(|line| line.starts_with('#'))
                    .find_map(|line| line.trim_start_matches('#').trim().strip_prefix("targets:"));
                let targets: Vec<&str> =
                    header.map_or(vec!["x86-m16", "x86-m32"], |list| list.split_whitespace().take(1).collect());
                for target in targets {
                    let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
                        .arg(file)
                        .args([&mode(target), "-O2", "-o", object.to_str().unwrap()])
                        .output()
                        .unwrap();
                    let stderr = String::from_utf8_lossy(&done.stderr);
                    // A program that needs a library (link:) or refuses on a
                    // target is not this test's business.
                    if done.status.success() && stderr.contains("warning") {
                        warned
                            .lock()
                            .unwrap()
                            .push(
                                format!(
                                    "{} [{target}]: {}",
                                    file.strip_prefix(root).unwrap().display(),
                                    stderr.lines().next().unwrap_or("")
                                ),
                            );
                    }
                }
            });
        }
    });
    let warned = warned.into_inner().unwrap();
    assert!(warned.is_empty(), "{}", warned.join("\n"));
}

/// The Zed extension is built apart from the workspace, so nothing compiled it:
/// a refactor moved its library path to a file that is not there, and it
/// carries no way to name the project's target to nib-lsp. Its manifest's
/// library exists, and it passes the `initialization_options` setting to the
/// server.
#[test]
fn test_the_zed_extension_names_a_library_that_exists_and_passes_the_projects_target() {
    let zed = Path::new(env!("CARGO_MANIFEST_DIR")).join("editors/zed");
    let manifest = std::fs::read_to_string(zed.join("Cargo.toml")).unwrap();
    let library = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("path = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("a library path");
    let source =
        std::fs::read_to_string(zed.join(library)).unwrap_or_else(|_| panic!("{library} is not in editors/zed"));
    assert!(
        source.contains("fn language_server_initialization_options")
            && source.contains("settings.initialization_options")
    );
}

/// The file calls' result was an `i32` on m16 and an `isize` on m32, so a
/// program naming the type was written for one target; both OS layers declare
/// the same one.
#[test]
fn test_both_targets_declare_the_same_file_call_result() {
    let results = |target: &str| -> Vec<String> {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .args([&mode(target), "--os-layer", "module"])
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        let text = String::from_utf8_lossy(&done.stdout).into_owned();
        ["pub fn read(", "pub fn write_file("]
            .iter()
            .map(|head| {
                text.lines()
                    .find(|line| line.starts_with(head))
                    .and_then(|line| line.rsplit_once("-> "))
                    .map(|(_, result)| result.trim().to_owned())
                    .expect("declared")
            })
            .collect()
    };
    assert_eq!(results("x86-m16"), ["i32", "i32"]);
    assert_eq!(results("x86-m32"), results("x86-m16"));
}

/// start.asm and os.asm each named a constant of their own (the stack, the
/// heap's arena, DOS's function numbers) beside the descriptions'; the
/// assembler is now told every fact, and a target is told only its own.
#[test]
fn test_the_assembler_is_told_the_runtime_descriptions_fields() {
    let defines = |target: &str| {
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .args([&mode(target), "--os-layer", "defines"])
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
        String::from_utf8_lossy(&done.stdout).split_whitespace().map(str::to_owned).collect::<Vec<_>>()
    };
    let flat = defines("x86-m32");
    for told in ["STACK_BYTES=131072", "HEAP_BYTES=16777216", "DOS_OPEN=61", "DOS_INT=33", "DOS_DPMI_ALLOC=1281"] {
        assert!(flat.contains(&told.to_owned()), "{told} in {flat:?}");
    }
    let real = defines("x86-m16");
    assert!(
        real.contains(&"DOS_OPEN=61".to_owned()) && !real.iter().any(|one| one.starts_with("HEAP_BYTES")),
        "{real:?}"
    );
}

/// The identity gate is an instrument: a build compared with itself must say
/// SAME of every program, and a build whose output differs must be reported
/// DIFF with a failing exit, or a change that moved a target's code would pass
/// the gate silently.
///
/// llrm-cc: the gate compiles with `-c` under `LLRM_CC_TARGET` (llrm-cc links
/// by default) and takes `IDENTITY_SAME_SOURCE=1` for the self-comparison,
/// which its same-binary guard refused.
#[test]
fn test_the_identity_gate_passes_a_build_against_itself_and_fails_a_different_one() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    let compiler = env!("CARGO_BIN_EXE_llrm-cc");
    let different = scratch.path().join("llrm-cc-os");
    std::fs::write(&different, format!("#!/bin/sh\nexec {compiler} \"$@\" -Os\n")).unwrap();
    std::fs::set_permissions(&different, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let gate = |new: &Path| {
        Command::new(root.join("tools/identity.sh"))
            .args(["c", compiler, new.to_str().unwrap()])
            .env("TMPDIR", scratch.path())
            .env("IDENTITY_SAME_SOURCE", "1")
            .output()
            .unwrap()
    };
    let same = gate(Path::new(compiler));
    let same_text = String::from_utf8_lossy(&same.stdout);
    assert!(same.status.success() && same_text.contains("SAME") && !same_text.contains("DIFF"), "{same_text}");
    let other = gate(&different);
    let other_text = String::from_utf8_lossy(&other.stdout);
    assert!(!other.status.success() && other_text.contains("DIFF "), "{other_text}");
}

/// The `-m` flag that names `target`: its description says which number.
fn mode(target: &str) -> String {
    let found = llrm_driver::all().into_iter().find(|one| one.name() == target).expect("a built-in target");
    format!("-m{}", found.layout().mode)
}

/// The `-march` flag for the m16 CPU `cpu`, as its `timings.times` names it.
fn march(cpu: &str) -> String {
    let m16 = llrm_driver::all().into_iter().find(|one| one.name() == "x86-m16").expect("m16");
    let at = m16.cpus().iter().position(|one| *one == cpu).expect("a CPU m16 prices");
    format!("-march={}", m16.marches()[at])
}

/// `#include "llrm_os.h"` finds the target's OS layer header with no `-I`: the
/// benches' bench_io.h includes it, and every test that compiled all of bench/
/// (dwarf, objwriters, this file's warnings test) stopped on `Unable to open
/// 'llrm_os.h'`.
#[test]
fn test_llrm_os_h_needs_no_include_path() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("io.c");
    std::fs::write(
        &source,
        "#include \"llrm_os.h\"\nint main(void) { return llrm_os_close(llrm_os_open(\"X\", 0)); }\n",
    )
    .unwrap();
    for mode in ["-m16", "-m32"] {
        let done = common::cc::llrm_cc()
            .args([mode, source.to_str().unwrap(), "-o", scratch.path().join("io.obj").to_str().unwrap()])
            .current_dir(scratch.path())
            .output()
            .unwrap();
        assert!(done.status.success(), "llrm-cc {mode}: {}", String::from_utf8_lossy(&done.stdout));
    }
}

/// The link script compiled the float helpers with `target/release/llrm-cc`
/// under the checkout, so a driver built in any other target directory could
/// compile but not link: "No such file or directory:
/// .../target/release/llrm-cc".
#[test]
fn test_a_program_links_with_a_driver_built_outside_the_checkouts_target_dir() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("p.c");
    std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();
    for mode in ["-m16", "-m32"] {
        let done = common::cc::llrm_cc()
            .args([mode, source.to_str().unwrap(), "-o", scratch.path().join("p.exe").to_str().unwrap()])
            .output()
            .unwrap();
        assert!(done.status.success(), "{mode}: {}", String::from_utf8_lossy(&done.stderr));
    }
}
