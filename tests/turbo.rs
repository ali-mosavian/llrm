//! `-gtd`: Borland's debug information in an OMF object. Borland's own tools read it: `TLINK /v`
//! builds Turbo Debugger's table from it, and `TDUMP` prints that table, which for the same program
//! compiled by Turbo C++ 3.0 (`-v -r-`, no register variables) says the same of its types, its
//! variables and their types and its lines. Needs Turbo C++ in `TCPP30_DIR` (default
//! `~/scratch/toolchains/tcpp30`) and the DOSBox-X this crate builds.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn llrm_c() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-c")
}

fn toolchain() -> Option<(PathBuf, PathBuf)> {
    let borland = std::env::var_os("TCPP30_DIR").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join("scratch/toolchains/tcpp30")))?;
    let dosbox = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("dosbox-x");
    (borland.join("bin/TCC.EXE").exists() && borland.join("bin/TLINK.EXE").exists() && borland.join("bin/TDUMP.EXE").exists() && dosbox.exists()).then_some((borland, dosbox))
}

/// Runs `commands` in DOS with Turbo C++ on C: and `work` on W:.
fn dos(borland: &Path, dosbox: &Path, work: &Path, commands: &[String]) {
    let mut conf = format!(
        "[sdl]\noutput=surface\n[dosbox]\nmemsize=32\nstartquiet=true\nstartbanner=false\nquit warning=false\n[cpu]\ncore=normal\ncputype=pentium\ncycles=max\n[dos]\nxms=true\n[mixer]\nnosound=true\n[autoexec]\nmount c {}\nmount w {}\npath c:\\bin\nw:\n",
        borland.display(),
        work.display()
    );
    for one in commands {
        conf.push_str(one);
        conf.push('\n');
    }
    conf.push_str("exit\n");
    let file = work.join("dos.conf");
    std::fs::write(&file, conf).unwrap();
    Command::new(dosbox).args(["-nolog", "-conf"]).arg(&file).env("SDL_VIDEODRIVER", "dummy").output().unwrap();
}

/// What TDUMP's Module Table says of a program: its type definitions and its variables' kinds, names and
/// types (not where they are), each sorted, and its source lines.
fn module(dump: &str) -> (Vec<String>, Vec<String>, BTreeSet<u32>) {
    let table = &dump[dump.find("Module Table").expect("a module table")..];
    let part = |from: &str, to: &str| -> Vec<String> {
        let start = table.find(from).map_or(0, |at| at + from.len());
        let end = table[start..].find(to).map_or(table.len(), |at| start + at);
        table[start..end].lines().map(str::trim).filter(|one| !one.is_empty()).map(str::to_owned).collect()
    };
    let strip = |line: &str| -> String {
        let mut line = line.to_owned();
        while let Some(open) = line.find('[') {
            let Some(close) = line[open..].find(']') else { break };
            if line[open + 1..open + close].chars().all(|c| c.is_ascii_hexdigit()) && close == 4 {
                line.replace_range(open..open + close + 2, "");
            } else {
                break;
            }
        }
        line.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let mut types: Vec<String> = part("Module Type Definitions:", "Correlation Records:").iter().map(|one| strip(one)).collect();
    types.sort();
    let correlation = part("Correlation Records:", "Line Numbers:");
    let mut locals: Vec<String> = correlation
        .iter()
        .filter(|one| one.contains("auto") || one.contains("register") || one.contains("static"))
        .map(|one| {
            let mut words = one.split_whitespace();
            // The first word is where it is; the rest is what it is.
            let (_, rest) = (words.next(), words.collect::<Vec<_>>().join(" "));
            rest
        })
        .collect();
    locals.sort();
    let mut lines = BTreeSet::new();
    for line in part("Line Numbers:", "\u{0}") {
        for entry in line.split_whitespace() {
            if let Some((number, _)) = entry.split_once(':') {
                lines.extend(number.parse::<u32>());
            }
        }
    }
    (types, locals, lines)
}

/// The tables TLINK builds from an llrm object and from Turbo C++'s of the same C program agree on the
/// module's types (a struct, one holding a pointer to itself, an array), on each variable and its type,
/// and llrm's lines are among Turbo C++'s (which also numbers each `{` and `}`).
#[test]
fn tlink_builds_turbo_debuggers_table_from_an_llrm_object_as_from_turbo_cs() {
    let Some((borland, dosbox)) = toolchain() else {
        eprintln!("skipped: needs Turbo C++ 3.0 (TCPP30_DIR) and DOSBox-X");
        return;
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    // DOS names are 8.3: each program is a short name.
    for (short, source) in [("obs", "observed.c"), ("lst", "list.c")] {
        std::fs::copy(fixtures.join(source), scratch.path().join(format!("{short}.c"))).unwrap();
        let made = Command::new(llrm_c()).args(["-m16", "-gtd", "-O0"]).arg(scratch.path().join(format!("{short}.c"))).arg("-o").arg(scratch.path().join(format!("l{short}.obj"))).output().unwrap();
        assert!(made.status.success(), "{short}: {}", String::from_utf8_lossy(&made.stderr));
    }
    let mut commands = Vec::new();
    for short in ["obs", "lst"] {
        commands.push(format!("tcc -v -r- -c -mm {short}.c"));
        for (prefix, object) in [("t", format!("{short}.obj")), ("l", format!("l{short}.obj"))] {
            commands.push(format!("tlink /v c:\\lib\\c0m {object}, {prefix}{short}.exe,, c:\\lib\\cm > {prefix}{short}.tl"));
            commands.push(format!("tdump {prefix}{short}.exe > {prefix}{short}.tx"));
        }
    }
    dos(&borland, &dosbox, scratch.path(), &commands);
    for short in ["obs", "lst"] {
        let read = |prefix: &str| std::fs::read_to_string(scratch.path().join(format!("{}{}.TX", prefix.to_uppercase(), short.to_uppercase()))).unwrap_or_else(|error| panic!("DOS made no {prefix}{short}.tx: {error}"));
        let (theirs, ours) = (module(&read("t")), module(&read("l")));
        assert!(!theirs.0.is_empty() && !theirs.1.is_empty(), "{short}: premise, Turbo C++'s own table has types and variables: {theirs:?}");
        assert_eq!(ours.0, theirs.0, "{short}: types");
        assert_eq!(ours.1, theirs.1, "{short}: variables");
        assert!(!ours.2.is_empty() && ours.2.is_subset(&theirs.2), "{short}: lines {:?} against {:?}", ours.2, theirs.2);
    }
}
