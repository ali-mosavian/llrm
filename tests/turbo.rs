//! `-gtd`: Borland's debug information in an OMF object. Borland's own tools
//! read it: `TLINK /v` builds Turbo Debugger's table from it, and `TDUMP`
//! prints that table, which for the same program compiled by Turbo C++ 3.0 (`-v
//! -r-`, no register variables) says the same of its types, its variables and
//! their types and its lines. Needs Turbo C++ in `TCPP30_DIR` (default
//! `~/scratch/toolchains/tcpp30`) and the DOSBox-X this crate builds.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

pub mod common;

use common::dosbox::{self, Session};

/// A test that cannot run says so on stderr, and fails where
/// `LLRM_REQUIRE_TURBO` is set, so a gate that has Turbo C++ and Turbo Debugger
/// cannot pass by skipping.
fn skipped(reason: &str) {
    dosbox::skipped("LLRM_REQUIRE_TURBO", reason);
}

fn llrm_c() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-c")
}

fn toolchain() -> Option<(PathBuf, PathBuf)> {
    let borland = std::env::var_os("TCPP30_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join("scratch/toolchains/tcpp30")))?;
    let dosbox = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("dosbox-x");
    (borland.join("bin/TCC.EXE").exists()
        && borland.join("bin/TLINK.EXE").exists()
        && borland.join("bin/TDUMP.EXE").exists()
        && dosbox.exists())
    .then_some((borland, dosbox))
}

/// What TDUMP's Module Table says of a program: its type definitions and its
/// variables' kinds, names and types (not where they are), each sorted, and its
/// source lines.
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
    let mut types: Vec<String> =
        part("Module Type Definitions:", "Correlation Records:").iter().map(|one| strip(one)).collect();
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

/// The tables TLINK builds from an llrm object and from Turbo C++'s of the same
/// C program agree on the module's types (a struct, one holding a pointer to
/// itself, an array), on each variable and its type, and llrm's lines are among
/// Turbo C++'s (which also numbers each `{` and `}`).
#[test]
fn tlink_builds_turbo_debuggers_table_from_an_llrm_object_as_from_turbo_cs() {
    let Some((borland, _)) = toolchain() else {
        skipped("needs Turbo C++ 3.0 (TCPP30_DIR) and DOSBox-X");
        return;
    };
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    // DOS names are 8.3: each program is a short name.
    for (short, source) in [("obs", "observed.c"), ("lst", "list.c")] {
        std::fs::copy(fixtures.join(source), scratch.path().join(format!("{short}.c"))).unwrap();
        let made = Command::new(llrm_c())
            .args(["-m16", "-gtd", "-O0"])
            .arg(scratch.path().join(format!("{short}.c")))
            .arg("-o")
            .arg(scratch.path().join(format!("l{short}.obj")))
            .output()
            .unwrap();
        assert!(made.status.success(), "{short}: {}", String::from_utf8_lossy(&made.stderr));
    }
    let mut commands = Vec::new();
    for short in ["obs", "lst"] {
        commands.push(format!("tcc -v -r- -c -mm {short}.c"));
        for (prefix, object) in [("t", format!("{short}.obj")), ("l", format!("l{short}.obj"))] {
            commands.push(format!(
                "tlink /v c:\\lib\\c0m {object}, {prefix}{short}.exe,, c:\\lib\\cm > {prefix}{short}.tl"
            ));
            commands.push(format!("tdump {prefix}{short}.exe > {prefix}{short}.tx"));
        }
    }
    dosbox::run(&[('c', &borland), ('w', scratch.path())], "c:\\bin", &commands);
    for short in ["obs", "lst"] {
        let read = |prefix: &str| {
            std::fs::read_to_string(scratch.path().join(format!(
                "{}{}.TX",
                prefix.to_uppercase(),
                short.to_uppercase()
            )))
            .unwrap_or_else(|error| panic!("DOS made no {prefix}{short}.tx: {error}"))
        };
        let (theirs, ours) = (module(&read("t")), module(&read("l")));
        assert!(
            !theirs.0.is_empty() && !theirs.1.is_empty(),
            "{short}: premise, Turbo C++'s own table has types and variables: {theirs:?}"
        );
        assert_eq!(ours.0, theirs.0, "{short}: types");
        assert_eq!(ours.1, theirs.1, "{short}: variables");
        assert!(
            !ours.2.is_empty() && ours.2.is_subset(&theirs.2),
            "{short}: lines {:?} against {:?}",
            ours.2,
            theirs.2
        );
    }
}

/// A pointer to a function points to a function type called as far as the
/// pointer reaches. TDUMP's symbol table names the type of each public symbol:
/// for the same C program Turbo C++ says `far pointer function far C` of `fp`
/// and `near pointer _CS function near C` of `np`; llrm's near pointer read
/// `function far C`.
#[test]
fn tdump_names_the_type_of_a_function_pointer_as_turbo_cs_does() {
    let Some((borland, _)) = toolchain() else {
        skipped("needs Turbo C++ 3.0 (TCPP30_DIR) and DOSBox-X");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/fnptr.c"),
        scratch.path().join("fnp.c"),
    )
    .unwrap();
    // Turbo C++'s own convention, whose symbols the table is compared by:
    // cdecl.
    let made = Command::new(llrm_c())
        .args(["-m16", "-mabi=cdecl", "-gtd", "-O0"])
        .arg(scratch.path().join("fnp.c"))
        .arg("-o")
        .arg(scratch.path().join("lfnp.obj"))
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let mut commands = vec!["tcc -v -r- -c -ml fnp.c".to_owned()];
    for (prefix, object) in [("t", "fnp.obj"), ("l", "lfnp.obj")] {
        commands.push(format!("tlink /v c:\\lib\\c0l {object}, {prefix}fnp.exe,, c:\\lib\\cl > {prefix}fnp.tl"));
        commands.push(format!("tdump {prefix}fnp.exe > {prefix}fnp.tx"));
    }
    dosbox::run(&[('c', &borland), ('w', scratch.path())], "c:\\bin", &commands);
    let types = |prefix: &str| -> Vec<String> {
        let text = std::fs::read_to_string(scratch.path().join(format!("{}FNP.TX", prefix.to_uppercase())))
            .unwrap_or_else(|error| panic!("DOS made no {prefix}fnp.tx: {error}"));
        let wanted = ["_CALLEE ", "_INNER ", "_APPLY ", "_FP ", "_NP "];
        text.lines()
            .filter(|line| wanted.iter().any(|name| line.contains(name)))
            .filter_map(|line| {
                line.split_once("]").map(|(_, rest)| rest.split_whitespace().collect::<Vec<_>>().join(" "))
            })
            .collect()
    };
    let (theirs, ours) = (types("t"), types("l"));
    assert_eq!(theirs.len(), 5, "premise: Turbo C++'s table names the five: {theirs:?}");
    assert_eq!(ours, theirs);
}

/// Turbo Debugger itself, driven. DOSBox-X's debug socket (`DOSBOX_DEBUG_PORT`,
/// a private port) injects keys and reads the text screen; it stops the
/// emulator at every INT 3, which Turbo Debugger uses for its own breakpoints,
/// so a reader thread answers those with `continue`.
struct Dosbox(Session);

impl Dosbox {
    fn start(
        borland: &Path,
        debugger: &Path,
        work: &Path,
    ) -> Dosbox {
        Dosbox(Session::start(&[('c', borland), ('d', debugger), ('w', work)], "c:\\bin;d:\\"))
    }

    fn command(
        &self,
        value: &serde_json::Value,
    ) -> serde_json::Value {
        self.0.command(value)
    }

    fn screen(&self) -> Vec<String> {
        self.command(&serde_json::json!({"cmd": "text_screen"}))["text"]
            .as_str()
            .unwrap_or("")
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn key(
        &self,
        key: serde_json::Value,
    ) {
        let mut value = serde_json::json!({"cmd": "key"});
        if let (Some(object), Some(extra)) = (value.as_object_mut(), key.as_object()) {
            object.extend(extra.clone());
        }
        self.command(&value);
        std::thread::sleep(Duration::from_millis(1500));
    }

    fn named(
        &self,
        name: &str,
    ) {
        self.key(serde_json::json!({"key": name}));
    }

    /// The (file, line) of Turbo Debugger's source window.
    fn at(&self) -> Option<(String, u32)> {
        let header = self.screen().get(1)?.clone();
        let (_, rest) = header.split_once("File: ")?;
        let mut words = rest.split_whitespace();
        Some((words.next()?.to_owned(), words.next()?.parse().ok()?))
    }

    /// Steps (F8, or F7 where `into`) until the window is at `line`.
    fn step_to(
        &self,
        line: u32,
        into: bool,
    ) {
        for _ in 0..12 {
            if self.at().is_some_and(|(_, at)| at == line) {
                return;
            }
            self.named(if into { "f7" } else { "f8" });
        }
        panic!("never reached line {line}: {:?}", self.at());
    }

    /// Debugs `exe`: to the call of `sum` at `call`, into it and to `line`;
    /// then each of `watches`, the Watches window's lines. A watch is
    /// Ctrl-F7 (scan code 0x64) and the text.
    fn debug(
        &self,
        exe: &str,
        call: u32,
        line: u32,
        watches: &[&str],
    ) -> Vec<String> {
        self.command(&serde_json::json!({"cmd": "dos_cmd", "command": format!("td {exe}")}));
        let started = Instant::now();
        while !self.screen().iter().any(|one| one.contains("Module:")) {
            assert!(started.elapsed() < Duration::from_secs(40), "Turbo Debugger did not come up");
            std::thread::sleep(Duration::from_secs(1));
        }
        self.step_to(call, false);
        self.named("f7");
        self.step_to(line, false);
        for watch in watches {
            self.key(serde_json::json!({"scancode": 0x64, "ascii": 0}));
            for character in watch.chars() {
                self.command(&serde_json::json!({"cmd": "key", "key": character.to_string()}));
                std::thread::sleep(Duration::from_millis(150));
            }
            self.named("enter");
        }
        let screen = self.screen();
        let from = screen.iter().position(|one| one.contains("Watches")).expect("a Watches window");
        screen[from + 1..]
            .iter()
            .map(|one| one.trim().to_owned())
            .take_while(|one| !one.starts_with("F1-Help"))
            .filter(|one| !one.is_empty())
            .collect()
    }
}

/// Borland's own Turbo Debugger, stopped on a line of the program built by
/// Turbo C++ and of the one llrm built with `-gtd`, shows the same values: a
/// parameter (`n`, a `struct node *` at `[_head]`), a local (`s`),
/// what the parameter points at (`*n`, a `struct node`) and its field (`n->v`);
/// and both are on the line the debugger was asked to reach, which needs the
/// line table, the scopes and the types.
#[test]
fn turbo_debugger_shows_the_same_values_for_an_llrm_program_as_for_turbo_cs() {
    let Some((borland, _)) = toolchain() else {
        skipped("needs Turbo C++ 3.0 (TCPP30_DIR) and DOSBox-X");
        return;
    };
    let debugger = std::env::var_os("TD_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join("scratch/toolchains/td")))
        .unwrap();
    if !debugger.join("Td.exe").exists() {
        skipped("needs Turbo Debugger in TD_DIR");
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    std::fs::copy(fixtures.join("list.c"), scratch.path().join("lst.c")).unwrap();
    let made = Command::new(llrm_c())
        .args(["-m16", "-gtd", "-O0"])
        .arg(scratch.path().join("lst.c"))
        .arg("-o")
        .arg(scratch.path().join("llst.obj"))
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let commands = [
        "tcc -v -r- -c -mm lst.c".to_owned(),
        "tlink /v c:\\lib\\c0m lst.obj, tlst.exe,, c:\\lib\\cm".to_owned(),
        "tlink /v c:\\lib\\c0m llst.obj, llst.exe,, c:\\lib\\cm".to_owned(),
    ];
    dosbox::run(&[('c', &borland), ('w', scratch.path())], "c:\\bin", &commands);
    let watches = ["s", "n", "*n", "n->v"];
    let mut seen = Vec::new();
    for exe in ["tlst.exe", "llst.exe"] {
        let session = Dosbox::start(&borland, &debugger, scratch.path());
        seen.push(session.debug(exe, 17, 9, &watches));
    }
    // The value of each watch, sorted: `n->v  int 3 (0x3)` is the same on both.
    let [turbo, llrm] = [&seen[0], &seen[1]].map(|lines| {
        let mut lines: Vec<String> =
            lines.iter().map(|one| one.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
        lines.sort();
        lines
    });
    assert!(
        turbo.iter().any(|one| one == "n->v int 3 (0x3)")
            && turbo.iter().any(|one| one.starts_with("n struct node * ds:") && one.ends_with("[_head]")),
        "premise: Turbo C++'s own values: {turbo:?}"
    );
    assert_eq!(llrm, turbo);
}

/// A variable the allocator keeps in one register from its first value to the
/// last statement is a register variable to Turbo Debugger too: `k` of
/// `regvar.c`, built optimised, is in TLINK's table as a `register`.
#[test]
fn tdump_names_a_variable_the_allocator_keeps_in_a_register_as_a_register() {
    let Some((borland, _)) = toolchain() else {
        skipped("needs Turbo C++ 3.0 (TCPP30_DIR) and DOSBox-X");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf/regvar.c"),
        scratch.path().join("rv.c"),
    )
    .unwrap();
    let made = Command::new(llrm_c())
        .args(["-m16", "-gtd", "-O2", "-fno-inline-functions", "-fno-inline-functions-called-once"])
        .arg(scratch.path().join("rv.c"))
        .arg("-o")
        .arg(scratch.path().join("lrv.obj"))
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    dosbox::run(
        &[('c', &borland), ('w', scratch.path())],
        "c:\\bin",
        &["tlink /v c:\\lib\\c0m lrv.obj, lrv.exe,, c:\\lib\\cm > lrv.tl".into(), "tdump lrv.exe > lrv.tx".into()],
    );
    let dump = std::fs::read_to_string(scratch.path().join("LRV.TX")).expect("TDUMP's output");
    let (_, locals, _) = module(&dump);
    assert!(
        locals.iter().any(|one| one.contains("register") && one.split_whitespace().any(|word| word == "k")),
        "no register k in {locals:?}"
    );
}
