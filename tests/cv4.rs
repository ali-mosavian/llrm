//! `-g` on C for OMF is CodeView 4 as C7 writes it, and Microsoft's own tools
//! read it: LINK /CO and CVPACK take an llrm object and write a packed table
//! (the `NB08` trailer), and CodeView, driven, stops at a function by its
//! name and shows its parameters and locals with their types and values. Needs
//! VB/DOS's tools (`VBDOS_DIR`, default `~/work/other/d32x/toolchains/vbdos`:
//! LINK, CVPACK, CV), Turbo C++'s start-up and library (`TCPP30_DIR`, default
//! `~/scratch/toolchains/tcpp30`) and the DOSBox-X this crate builds.
//! `LLRM_REQUIRE_CODEVIEW` makes a missing tool a failure.

pub mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::dosbox::{self, Session};

fn skipped(reason: &str) {
    dosbox::skipped("LLRM_REQUIRE_CODEVIEW", reason);
}

fn directory(
    variable: &str,
    default: &str,
    marker: &str,
) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let found = std::env::var_os(variable).map(PathBuf::from).unwrap_or_else(|| Path::new(&home).join(default));
    found.join(marker).exists().then_some(found)
}

/// A program of structs, a pointer to itself and two functions to stop in,
/// linked by LINK /CO.
fn linked(
    microsoft: &Path,
    borland: &Path,
    scratch: &Path,
) -> Vec<u8> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codeview");
    std::fs::copy(fixtures.join("m.c"), scratch.join("m.c")).unwrap();
    // Borland's C0M and CM name their symbols with a leading underscore.
    let made = common::cc::llrm_c()
        .env("LLRM_CC_TARGET", "x86_16-bcc")
        .args(["-m16", "-g", "-O0"])
        .arg(scratch.join("m.c"))
        .arg("-o")
        .arg(scratch.join("m.obj"))
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    dosbox::run(
        &[('c', borland), ('v', microsoft), ('w', scratch)],
        "v:\\bin",
        &["link /CO /NOI c:\\lib\\c0m.obj m.obj,m.exe,nul,c:\\lib\\cm.lib; > link.txt".into()],
    );
    std::fs::read(scratch.join("M.EXE")).unwrap_or_else(|error| {
        panic!("LINK made no M.EXE: {error}: {}", std::fs::read_to_string(scratch.join("LINK.TXT")).unwrap_or_default())
    })
}

/// What CodeView's locals window says after `commands`, each typed at its
/// prompt.
fn locals(
    session: &Session,
    commands: &[&str],
) -> Vec<String> {
    for command in commands {
        session.type_line(command);
        std::thread::sleep(Duration::from_millis(2500));
    }
    session.video_text()
}

/// LINK /CO hands the object to CVPACK, which rewrites the table it can read as
/// `NB08`; a table it could not read stays `NB00` or LINK stops. CodeView then
/// breaks at `sum` and `f` by name, on the lines the source window shows, and
/// the locals window has the types the object gave and the values the program
/// has: `short`, a `struct node near *`.
#[test]
fn microsoft_link_and_cvpack_take_the_object_and_codeview_reads_its_variables() {
    let (Some(microsoft), Some(borland)) = (
        directory("VBDOS_DIR", "work/other/d32x/toolchains/vbdos", "BIN/CV.EXE"),
        directory("TCPP30_DIR", "scratch/toolchains/tcpp30", "lib/C0M.OBJ"),
    ) else {
        skipped("needs VB/DOS's LINK, CVPACK and CV (VBDOS_DIR) and Turbo C++'s C0M and CM (TCPP30_DIR)");
        return;
    };
    if !dosbox::binary().exists() {
        skipped("needs the DOSBox-X this crate builds");
        return;
    }
    if std::env::var_os("LLRM_BORLAND_INC").is_none() {
        // llrm-cc compiles for x86_16-bcc against Borland's headers; with the
        // tools above present but these absent the test
        // was red ("the target needs the environment variable
        // LLRM_BORLAND_INC") instead of skipped.
        skipped("needs Borland's C headers (LLRM_BORLAND_INC)");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let exe = linked(&microsoft, &borland, scratch.path());
    assert_eq!(&exe[exe.len() - 8..exe.len() - 4], b"NB08", "CVPACK packed the table");

    let session = Session::start(&[('c', &borland), ('v', &microsoft), ('w', scratch.path())], "v:\\bin");
    session.dos("cv m.exe");
    let started = Instant::now();
    while !session.video_text().iter().any(|row| row.contains("source1")) {
        assert!(started.elapsed() < Duration::from_secs(60), "CodeView did not come up");
        std::thread::sleep(Duration::from_secs(1));
    }
    // `sum` is called first: one step past each of its three lines of setup.
    let screen = locals(&session, &["bp sum", "g", "p", "p", "p"]).join("\n");
    assert!(screen.contains("short s = "), "sum's local s:\n{screen}");
    assert!(screen.contains("node near * n"), "sum's parameter n, a pointer to a struct:\n{screen}");
    let screen = locals(&session, &["bp f", "g", "p", "p"]).join("\n");
    for expected in ["short a = 1", "short b = 2", "short x = 3"] {
        assert!(screen.contains(expected), "no {expected:?} in CodeView's screen:\n{screen}");
    }
}

/// A small-model program the model describes and no frontend does: an enum, a
/// `const`, a variable in a register, a block with a variable of its own.
/// `main` is 28 bytes: `e = GREEN; si = 42; { k = 7 }; return 0`.
fn synthetic() -> (llrm_debug::Described, String) {
    use llrm_debug::{
        Block, Enumerator, File, Function, Info, Kind, Language, Line, Location, Range, Register, Scalar, Type,
        Variable,
    };
    use llrm_object::{Binding, Definition, Object, Role, Section, Symbol};
    let code = vec![
        0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x06, 0xC7, 0x46, 0xFE, 0x05, 0x00, 0xBE, 0x2A, 0x00, 0xC7, 0x46, 0xFC, 0x07,
        0x00, 0x8B, 0x46, 0xFC, 0x33, 0xC0, 0x8B, 0xE5, 0x5D, 0xCB,
    ];
    let length = code.len();
    // `_isatty`, which the library's start-up wants: returns 0.
    let mut code = code;
    code.extend([0x33, 0xC0, 0xCB]);
    let total = code.len();
    let text = Section {
        name: "_TEXT".into(),
        role: Role::Text,
        near: false,
        align: 1,
        image: code,
        spans: vec![[0, total]],
        relocs: Vec::new(),
    };
    let variable = |name: &str, r#type, location| Variable { name: name.into(), r#type, kind: Kind::Local, location };
    let whole = Range { section: 0, offset: 0, length };
    let enumerators =
        [("RED", 0), ("GREEN", 5), ("BLUE", 6)].map(|(name, value)| Enumerator { name: name.into(), value }).to_vec();
    let function = Function {
        name: "main".into(),
        symbol: 0,
        r#type: 3,
        ranges: vec![whole],
        body: Some((6, 24)),
        far: true,
        module: false,
        variables: vec![
            variable("e", 1, Location::Frame { disp: -2 }),
            variable("r", 0, Location::Register("si".into())),
        ],
        blocks: vec![Block {
            ranges: vec![Range { section: 0, offset: 14, length: 8 }],
            variables: vec![variable("k", 2, Location::Frame { disp: -4 })],
            blocks: Vec::new(),
            inlined: None,
        }],
        frame: Vec::new(),
    };
    let info = Info {
        language: Language::C,
        options: llrm_debug::DebugOptions { dialect: llrm_debug::Dialect::Cv4, ..Default::default() },
        target: Some(llrm_x86::mc::fixup::debug_target(16).with_registers(
            "bp",
            "",
            vec![Register { name: "si".into(), bits: 16, dwarf: None, codeview: Some(15) }],
        )),
        code: vec![whole],
        types: vec![
            Type::Scalar(Scalar::Int { bytes: 2, signed: true }),
            Type::Enum { name: "color".into(), underlying: 0, enumerators },
            Type::Qualified { target: 0, constant: true, volatile: false },
            Type::Procedure { result: Some(0), parameters: Vec::new(), convention: None },
        ],
        functions: vec![function],
        files: vec![File { name: "syn.c".into(), checksum: None }],
        lines: [(1, 0), (3, 6), (4, 11), (5, 14), (6, 19), (7, 22)]
            .map(|(line, offset)| Line { section: 0, offset, file: 0, line, column: 0 })
            .to_vec(),
        ..Info::default()
    };
    let object = llrm_debug::Described {
        debug: Some(info),
        object: Object {
            name: "syn.c".into(),
            bits: 16,
            sections: vec![text],
            symbols: vec![
                Symbol {
                    name: "_main".into(),
                    binding: Binding::Public,
                    definition: Definition::Defined { section: 0, offset: 0 },
                    size: 0,
                    group: None,
                },
                Symbol {
                    name: "_isatty".into(),
                    binding: Binding::Public,
                    definition: Definition::Defined { section: 0, offset: length },
                    size: 0,
                    group: None,
                },
            ],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        },
    };
    (object, "int main(void)\n{\n    enum color e = GREEN;\n    register int r = 42;\n    { const int k = 7;\n      r = k; }\n    return 0;\n}\n".to_owned())
}

/// CodeView, driven, on the program above: LINK /CO and CVPACK take it, and the
/// locals window at the first line of the block shows each record as CodeView 4
/// means it: `e` an enum by its name, `r` in register SI, and `k`, a
/// `const short` that is listed only inside its block. `LF_ENUM`,
/// `LF_MODIFIER`, `S_REGISTER` and `S_BLOCK16` were written from Open Watcom's
/// headers alone before this ran.
#[test]
fn codeview_reads_an_enum_a_const_a_register_variable_and_a_block_scope() {
    let (Some(microsoft), Some(borland)) = (
        directory("VBDOS_DIR", "work/other/d32x/toolchains/vbdos", "BIN/CV.EXE"),
        directory("TCPP30_DIR", "scratch/toolchains/tcpp30", "lib/C0M.OBJ"),
    ) else {
        skipped("needs VB/DOS's LINK, CVPACK and CV (VBDOS_DIR) and Turbo C++'s C0M and CM (TCPP30_DIR)");
        return;
    };
    if !dosbox::binary().exists() {
        skipped("needs the DOSBox-X this crate builds");
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let (object, source) = synthetic();
    std::fs::write(scratch.path().join("syn.c"), source).unwrap();
    std::fs::write(
        scratch.path().join("syn.obj"),
        llrm_codegen::objectfile::write::write(&object.object, object.debug.as_ref(), &llrm_x86::mc::omf::X86Omf)
            .unwrap(),
    )
    .unwrap();
    dosbox::run(
        &[('c', &borland), ('v', &microsoft), ('w', scratch.path())],
        "v:\\bin",
        &["link /CO /NOI c:\\lib\\c0m.obj syn.obj,syn.exe,nul,c:\\lib\\cm.lib; > link.txt".into()],
    );
    let exe = std::fs::read(scratch.path().join("SYN.EXE")).unwrap_or_else(|error| {
        panic!(
            "LINK made no SYN.EXE: {error}: {}",
            std::fs::read_to_string(scratch.path().join("LINK.TXT")).unwrap_or_default()
        )
    });
    assert_eq!(&exe[exe.len() - 8..exe.len() - 4], b"NB08", "CVPACK packed the table");
    let session = Session::start(&[('c', &borland), ('v', &microsoft), ('w', scratch.path())], "v:\\bin");
    session.dos("cv syn.exe");
    let started = Instant::now();
    while !session.video_text().iter().any(|row| row.contains("source1")) {
        assert!(started.elapsed() < Duration::from_secs(60), "CodeView did not come up");
        std::thread::sleep(Duration::from_secs(1));
    }
    // Stopped at `main`, then a line at a time to the first line of the block.
    let screen = locals(&session, &["bp main", "g", "p", "p", "p"]).join("\n");
    for expected in ["const short k", "color e = 5", "SI reg short r = 42"] {
        assert!(screen.contains(expected), "no {expected:?} in CodeView's screen:\n{screen}");
    }
}

/// What `llrm-nib --os-layer FIELD` says of the real-mode target's OS layer.
fn os_layer(field: &str) -> String {
    let out = Command::new(Path::new(env!("CARGO_BIN_EXE_llrm-cc")).parent().unwrap().join("llrm-nib"))
        .args(["-m16", "--os-layer", field])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A Nib program is CodeView 4 as C is: `bp add` finds the function by name,
/// and the locals window at its second line shows the parameters and a local
/// with their values. Written in the BASIC compilers' dialect it could not be
/// debugged (#892): CodeView took the module's addresses for the start-up's
/// segment and found no `add`. Built as `tools/nib-build.sh` builds it but
/// linked by MS LINK /CO, which links the whole runtime in one segment.
#[test]
fn codeview_debugs_a_nib_program_as_it_does_a_c_one() {
    let Some(screen) = nib_in_codeview("p", &["-O0"], &["bp add", "g", "p"]) else { return };
    // `add`'s parameters arrive in registers (regparm3); `-g` stores them to no
    // cell (it changes no code). `a` is overwritten by the first instruction
    // (`movsx ecx, ax` ends its register, then `lea eax, ...` takes it), so it
    // has no one place over the function and is not a variable of CodeView's.
    // `b` stays in EDX through the whole function (the DAG selector adds into
    // a fresh register, `lea eax, [edx+ecx]`), so it is a register variable.
    // The locals are in cells.
    for expected in ["long s = 2", "EDX reg long b = 3"] {
        assert!(screen.contains(expected), "no {expected:?} in CodeView's screen:\n{screen}");
    }
    assert!(!screen.contains("short a ="), "`a`, in no one place, is in CodeView's screen:\n{screen}");
}

/// A variable the allocator keeps in one register from its first value to the
/// last statement is a register variable to CodeView (`S_REGISTER`), and its
/// value is read there: `k` of `regvar.nib`, built optimised, lives in `bx`
/// across the call that clobbers `ax`.
#[test]
fn codeview_reads_a_nib_variable_the_allocator_keeps_in_a_register() {
    let Some(screen) = nib_in_codeview(
        "regvar",
        &["-O2", "-fno-inline-functions", "-fno-inline-functions-called-once"],
        &["bp work", "g", "p"],
    ) else {
        return;
    };
    assert!(screen.contains("reg short k = 12"), "no register `k = 12` in CodeView's screen:\n{screen}");
}

/// A segment per procedure of one name is one segment to Microsoft's LINK,
/// which keeps the last one's length: the procedures longer than it drew L1103
/// (#899), and with the error hidden the image lost the code of the rest
/// (`main` mapped into `big`'s bytes). A COMDAT per procedure, as MSC's /Gy
/// writes, is placed, packed and dropped by every linker: the program and the
/// runtime cut to what it names are linked by LINK, with and without /CO, and
/// RUN. Both print what the jwlink-linked program prints.
#[test]
fn ms_link_links_a_comdat_per_procedure_and_the_program_runs() {
    let Some(microsoft) = directory("VBDOS_DIR", "work/other/d32x/toolchains/vbdos", "BIN/LINK.EXE") else {
        skipped("needs VB/DOS's LINK (VBDOS_DIR)");
        return;
    };
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-cc")).parent().unwrap().to_owned();
    if !dosbox::binary().exists() || !bin.join("jwasm").exists() {
        skipped("needs the DOSBox-X this crate builds and jwasm beside the compiler");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("p.nib"),
        "fn big(a: i16) -> i16:\n    let mut v = a\n    v = v * 3 + 1\n    v = v * 3 + 2\n    return v\n\nfn mid(a: i16) -> i16:\n    print(big(a))\n    return big(a) + 1\n\nfn main() -> i16:\n    print(mid(1))\n    return 0\n",
    )
    .unwrap();
    let run = |program: PathBuf, args: Vec<String>| {
        let done = Command::new(&program).args(&args).current_dir(root).output().unwrap();
        assert!(
            done.status.success(),
            "{} {args:?}: {}{}",
            program.display(),
            String::from_utf8_lossy(&done.stdout),
            String::from_utf8_lossy(&done.stderr)
        );
    };
    let path = |name: &str| scratch.path().join(name).display().to_string();
    let layer = PathBuf::from(os_layer("directory"));
    let defines: Vec<String> = os_layer("defines").split_whitespace().map(|one| format!("-D{one}")).collect();
    for (output, source) in [
        ("start.obj", layer.join(os_layer("start"))),
        ("impl.obj", layer.join(os_layer("implementation"))),
        ("hook.obj", PathBuf::from(os_layer("language_file"))),
    ] {
        let mut args = vec!["-q".to_owned(), "-c".into(), "-Cp".into(), "-Zg".into(), "-omf".into()];
        args.extend(defines.iter().cloned());
        args.extend([format!("-Fo{}", path(output)), source.display().to_string()]);
        run(bin.join("jwasm"), args);
    }
    let nib = bin.join("llrm-nib");
    let runtime = root.join("crates/frontends/nib/src/runtime/runtime.nib").display().to_string();
    for (program, cut, flags) in [("p.obj", "rt.obj", None), ("pg.obj", "rtg.obj", Some("-g"))] {
        let mut arguments: Vec<String> = vec!["-m16".into(), "-O0".into(), "--procedure-segments".into()];
        arguments.extend(flags.map(str::to_owned));
        arguments.extend([path("p.nib"), "-o".into(), path(program)]);
        run(nib.clone(), arguments);
        let mut arguments: Vec<String> = vec!["-m16".into(), "-O0".into(), "--procedure-segments".into()];
        for used in [program, "start.obj", "hook.obj", "impl.obj"] {
            arguments.extend(["--used-by".into(), path(used)]);
        }
        arguments.extend([runtime.clone(), "-o".into(), path(cut)]);
        run(nib.clone(), arguments);
    }
    dosbox::run(
        &[('v', &microsoft), ('w', scratch.path())],
        "v:\\bin",
        &[
            "link /NOI start.obj hook.obj p.obj rt.obj impl.obj,p.exe,p.map,; > link.txt".into(),
            "link /CO /NOI start.obj hook.obj pg.obj rtg.obj impl.obj,pg.exe,pg.map,; > linkg.txt".into(),
            "p.exe > p.out".into(),
            "pg.exe > pg.out".into(),
        ],
    );
    for (exe, log, out) in [("P.EXE", "LINK.TXT", "P.OUT"), ("PG.EXE", "LINKG.TXT", "PG.OUT")] {
        let said = std::fs::read_to_string(scratch.path().join(log)).unwrap_or_default();
        assert!(scratch.path().join(exe).exists(), "LINK made no {exe}: {said}");
        let printed = std::fs::read_to_string(scratch.path().join(out)).unwrap_or_default();
        assert_eq!(
            printed.split_whitespace().collect::<Vec<_>>(),
            ["14", "15"],
            "{exe} printed {printed:?}; LINK said {said}"
        );
    }
}

/// `fixture` (a Nib program of tests/fixtures/codeview) built with `flags` as
/// `tools/nib-build.sh` builds it, linked by MS LINK /CO, and CodeView's screen
/// after `commands`; none where the tools are not there.
fn nib_in_codeview(
    fixture: &str,
    flags: &[&str],
    commands: &[&str],
) -> Option<String> {
    let (Some(microsoft), bin) = (
        directory("VBDOS_DIR", "work/other/d32x/toolchains/vbdos", "BIN/CV.EXE"),
        Path::new(env!("CARGO_BIN_EXE_llrm-cc")).parent().unwrap().to_owned(),
    ) else {
        skipped("needs VB/DOS's LINK, CVPACK and CV (VBDOS_DIR)");
        return None;
    };
    if !dosbox::binary().exists() || !bin.join("jwasm").exists() {
        skipped("needs the DOSBox-X this crate builds and jwasm beside the compiler");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    std::fs::copy(root.join(format!("tests/fixtures/codeview/{fixture}.nib")), scratch.path().join("p.nib")).unwrap();
    let run = |program: PathBuf, args: Vec<String>| {
        let done = Command::new(&program).args(&args).current_dir(root).output().unwrap();
        assert!(
            done.status.success(),
            "{} {args:?}: {}{}",
            program.display(),
            String::from_utf8_lossy(&done.stdout),
            String::from_utf8_lossy(&done.stderr)
        );
    };
    let nib = bin.join("llrm-nib");
    let mut arguments: Vec<String> = vec!["-m16".into(), "-g".into()];
    arguments.extend(flags.iter().map(|one| (*one).to_owned()));
    arguments.extend([
        scratch.path().join("p.nib").display().to_string(),
        "-o".into(),
        scratch.path().join("p.obj").display().to_string(),
    ]);
    run(nib.clone(), arguments);
    run(
        nib,
        vec![
            "-m16".into(),
            "-O0".into(),
            root.join("crates/frontends/nib/src/runtime/runtime.nib").display().to_string(),
            "-o".into(),
            scratch.path().join("rt.obj").display().to_string(),
        ],
    );
    // The OS layer's start-up, its language hook and its implementation,
    // assembled as the OS layer says.
    let layer = PathBuf::from(os_layer("directory"));
    let defines: Vec<String> = os_layer("defines").split_whitespace().map(|one| format!("-D{one}")).collect();
    for (output, source) in [
        ("start.obj", layer.join(os_layer("start"))),
        ("impl.obj", layer.join(os_layer("implementation"))),
        ("hook.obj", PathBuf::from(os_layer("language_file"))),
    ] {
        let mut args = vec!["-q".to_owned(), "-c".into(), "-Cp".into(), "-Zg".into(), "-omf".into()];
        args.extend(defines.iter().cloned());
        args.extend([format!("-Fo{}", scratch.path().join(output).display()), source.display().to_string()]);
        run(bin.join("jwasm"), args);
    }
    dosbox::run(
        &[('v', &microsoft), ('w', scratch.path())],
        "v:\\bin",
        &["link /CO /NOI start.obj hook.obj p.obj rt.obj impl.obj,p.exe,p.map,; > link.txt".into()],
    );
    assert!(
        scratch.path().join("P.EXE").exists(),
        "LINK made no P.EXE: {}",
        std::fs::read_to_string(scratch.path().join("LINK.TXT")).unwrap_or_default()
    );
    let session = Session::start(&[('v', &microsoft), ('w', scratch.path())], "v:\\bin");
    session.dos("cv p.exe");
    let started = Instant::now();
    while !session.video_text().iter().any(|row| row.contains("source1")) {
        assert!(started.elapsed() < Duration::from_secs(60), "CodeView did not come up");
        std::thread::sleep(Duration::from_secs(1));
    }
    Some(locals(&session, commands).join("\n"))
}

/// A lexical block in two pieces (optimised code puts other code between them)
/// made the whole object unwritable: "CodeView 4: a lexical block in pieces",
/// so every optimised `-g` OMF compile with a nested scope failed. A format
/// with no list of ranges writes it over its first byte to its last.
#[test]
fn a_block_in_two_pieces_is_written_over_its_hull() {
    let (mut object, _) = synthetic();
    let function = &mut object.debug.as_mut().unwrap().functions[0];
    function.blocks[0].ranges = vec![
        llrm_debug::Range { section: 0, offset: 14, length: 3 },
        llrm_debug::Range { section: 0, offset: 19, length: 3 },
    ];
    let written =
        llrm_codegen::objectfile::write::write(&object.object, object.debug.as_ref(), &llrm_x86::mc::omf::X86Omf);
    let bytes = written.unwrap_or_else(|error| panic!("{error}"));
    let shape = llrm_codegen::objectfile::cv4info::shape(&llrm_codegen::objectfile::omf::parse(&bytes).unwrap());
    assert!(shape.iter().any(|one| one.contains("k")), "{shape:#?}");
}

/// `-g` shape of `regvar.nib` built optimised with the selector `isel`.
fn regvar_shape(isel: &str) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-cc")).parent().unwrap().to_owned();
    let scratch = tempfile::tempdir().unwrap();
    let object = scratch.path().join("p.obj");
    let made = Command::new(bin.join("llrm-nib"))
        .args(["-m16", "-g", "-O2", "-fno-inline-functions", "-fno-inline-functions-called-once"])
        .arg(format!("--isel={isel}"))
        .arg(root.join("tests/fixtures/codeview/regvar.nib"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let records = llrm_codegen::objectfile::omf::parse(&std::fs::read(&object).unwrap()).unwrap();
    llrm_codegen::objectfile::cv4info::shape(&records)
}

/// The DAG selector kept no `dbg_value` (`k = a * 3` became `imul cx, ax, 3`
/// and its record had no instruction to stand before): `k` was in no
/// CodeView record, where the old selector wrote it as a register variable.
#[test]
fn a_nib_register_variable_is_in_the_object_under_the_dag_selector() {
    let (old, dag) = (regvar_shape("old"), regvar_shape("dag"));
    let has_k = |shape: &[String]| shape.iter().any(|one| one.starts_with("REGISTER work.k"));
    assert!(has_k(&old), "{old:#?}");
    assert!(has_k(&dag), "the DAG selector wrote no `k`:\n{dag:#?}\nthe old one wrote:\n{old:#?}");
}
