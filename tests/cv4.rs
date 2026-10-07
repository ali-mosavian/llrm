//! `-g` on C for OMF is CodeView 4 as C7 writes it, and Microsoft's own tools read it: LINK /CO and CVPACK take an
//! llrm object and write a packed table (the `NB08` trailer), and CodeView, driven, stops at a function by its
//! name and shows its parameters and locals with their types and values. Needs VB/DOS's tools
//! (`VBDOS_DIR`, default `~/work/other/d32x/toolchains/vbdos`: LINK, CVPACK, CV), Turbo C++'s start-up and
//! library (`TCPP30_DIR`, default `~/scratch/toolchains/tcpp30`) and the DOSBox-X this crate builds.
//! `LLRM_REQUIRE_CODEVIEW` makes a missing tool a failure.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::dosbox::{self, Session};

fn skipped(reason: &str) {
    dosbox::skipped("LLRM_REQUIRE_CODEVIEW", reason);
}

fn directory(variable: &str, default: &str, marker: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let found = std::env::var_os(variable).map(PathBuf::from).unwrap_or_else(|| Path::new(&home).join(default));
    found.join(marker).exists().then_some(found)
}

/// A program of structs, a pointer to itself and two functions to stop in, linked by LINK /CO.
fn linked(microsoft: &Path, borland: &Path, scratch: &Path) -> Vec<u8> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codeview");
    std::fs::copy(fixtures.join("m.c"), scratch.join("m.c")).unwrap();
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-c")).args(["-m16", "-g", "-O0"]).arg(scratch.join("m.c")).arg("-o").arg(scratch.join("m.obj")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    dosbox::run(
        &[('c', borland), ('v', microsoft), ('w', scratch)],
        "v:\\bin",
        &["link /CO /NOI c:\\lib\\c0m.obj m.obj,m.exe,nul,c:\\lib\\cm.lib; > link.txt".into()],
    );
    std::fs::read(scratch.join("M.EXE")).unwrap_or_else(|error| panic!("LINK made no M.EXE: {error}: {}", std::fs::read_to_string(scratch.join("LINK.TXT")).unwrap_or_default()))
}

/// What CodeView's locals window says after `commands`, each typed at its prompt.
fn locals(session: &Session, commands: &[&str]) -> Vec<String> {
    for command in commands {
        session.type_line(command);
        std::thread::sleep(Duration::from_millis(2500));
    }
    session.video_text()
}

/// LINK /CO hands the object to CVPACK, which rewrites the table it can read as `NB08`; a table it could not
/// read stays `NB00` or LINK stops. CodeView then breaks at `sum` and `f` by name, on the lines the source
/// window shows, and the locals window has the types the object gave and the values the program has: `short`,
/// a `struct node near *`.
#[test]
fn microsoft_link_and_cvpack_take_the_object_and_codeview_reads_its_variables() {
    let (Some(microsoft), Some(borland)) = (directory("VBDOS_DIR", "work/other/d32x/toolchains/vbdos", "BIN/CV.EXE"), directory("TCPP30_DIR", "scratch/toolchains/tcpp30", "lib/C0M.OBJ")) else {
        skipped("needs VB/DOS's LINK, CVPACK and CV (VBDOS_DIR) and Turbo C++'s C0M and CM (TCPP30_DIR)");
        return;
    };
    if !dosbox::binary().exists() {
        skipped("needs the DOSBox-X this crate builds");
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
