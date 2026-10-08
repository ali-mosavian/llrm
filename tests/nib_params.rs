//! A Nib parameter that arrives in a register is stored to a cell at the entry where the debug format cannot follow it in the register
//! (`-g` for OMF, any flavor). The program runs: stopped at the address CodeView 4's records name as the start of the body, the cell the
//! records name holds the argument. Needs jwasm and jwlink (the build's own), and the DOSBox-X this crate builds.
//! `LLRM_REQUIRE_NIBRUN` makes a missing tool a failure.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use common::dosbox::{self, Session};
use llrm_core::objectfile::{cvinfo, omf};

fn skipped(reason: &str) {
    dosbox::skipped("LLRM_REQUIRE_NIBRUN", reason);
}

fn bin() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-nib")).parent().unwrap().to_owned()
}

fn word(hex: &str) -> u64 {
    (0..hex.len() / 2).rev().fold(0, |value, at| value << 8 | u64::from(u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).unwrap()))
}

/// `add(a: i16, b: i32)` under `-mabi=regparm3`, built with `-g`: `a` arrives in AX and `b` in EDX. Stopped where the debug records say the
/// body begins, `a` and `b` are in the cells the records name (2 and 3), which `-g` of OMF stores them to at the entry.
#[test]
fn a_register_parameter_is_in_its_cell_where_the_debugger_stops_at_the_body() {
    if !bin().join("jwasm").exists() || !bin().join("jwlink").exists() || !dosbox::binary().exists() {
        skipped("needs jwasm and jwlink beside the compiler, and the DOSBox-X this crate builds");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::tempdir().unwrap();
    std::fs::copy(root.join("tests/fixtures/dwarf/params.nib"), scratch.path().join("params.nib")).unwrap();
    let built = Command::new("sh")
        .arg(root.join("tools/nib-build.sh"))
        .args([scratch.path().join("params.nib"), scratch.path().join("PAR.EXE")])
        .arg("-O0")
        .env("TOOLCHAIN", bin())
        .env("TMPDIR", scratch.path())
        .env("NIB_FLAGS", "-mabi=regparm3 -g")
        .env("NIB_MAP", scratch.path().join("par.map"))
        .env("NIB_OBJ", scratch.path().join("program.obj"))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}{}", String::from_utf8_lossy(&built.stdout), String::from_utf8_lossy(&built.stderr));
    // Where the body of `add` starts: its segment's place in the program and the debug records' offset into it.
    let map = std::fs::read_to_string(scratch.path().join("par.map")).unwrap();
    let segment = map.lines().find(|line| line.starts_with("PARAMS_TEXT")).and_then(|line| line.split_whitespace().nth(3)).and_then(|at| at.split_once(':')).map(|(_, offset)| usize::from_str_radix(offset, 16).unwrap()).expect("the program's segment in the map");
    let records = omf::parse(&std::fs::read(scratch.path().join("program.obj")).unwrap()).unwrap();
    let info = cvinfo::parse(&records);
    let add = info.procedures.iter().find(|one| one.name == "add").expect("add");
    let body = segment + add.offset as usize + add.debug_start as usize;

    let session = Session::start(&[('w', scratch.path())], "");
    session.command(&serde_json::json!({"cmd": "bp_on_load"}));
    session.dos("par.exe");
    let registers = |session: &Session| {
        let regs = session.command(&serde_json::json!({"cmd": "regs"}));
        let at = |name: &str| u64::from_str_radix(regs[name].as_str().unwrap_or("0x0").trim_start_matches("0x"), 16).unwrap_or(0) as usize;
        (regs["CS"].as_u64().unwrap_or(0) as usize * 16 + at("EIP"), regs)
    };
    // The program is loaded and stopped at its first instruction before the breakpoint is set: set while it runs, the
    // breakpoint comes after `add` has run, on a loaded machine, and the program has ended.
    let loading = Instant::now();
    let (load, entry) = loop {
        let answer = session.command(&serde_json::json!({"cmd": "get_load_info"}));
        if answer["available"] == true && answer["loadInfo"]["program"].as_str().is_some_and(|name| name.eq_ignore_ascii_case("par.exe")) {
            break (answer["loadInfo"]["loadLinear"].as_u64().unwrap() as usize, answer["loadInfo"]["entryLinear"].as_u64().unwrap() as usize);
        }
        assert!(loading.elapsed() < Duration::from_secs(120), "the program did not load");
        std::thread::sleep(Duration::from_millis(300));
    };
    while registers(&session).0 != entry {
        assert!(loading.elapsed() < Duration::from_secs(120), "the program did not stop at its entry: {}", registers(&session).1);
        std::thread::sleep(Duration::from_millis(300));
    }
    session.command(&serde_json::json!({"cmd": "bp_set_linear_exec", "linear": load + body}));
    session.command(&serde_json::json!({"cmd": "bp_on_load_clear"}));
    session.command(&serde_json::json!({"cmd": "continue"}));
    let running = Instant::now();
    let regs = loop {
        let (at, regs) = registers(&session);
        if at == load + body {
            break regs;
        }
        assert!(running.elapsed() < Duration::from_secs(120), "never stopped at the body: {regs}");
        std::thread::sleep(Duration::from_millis(300));
    };
    let bp = u64::from_str_radix(regs["EBP"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap() as usize & 0xFFFF;
    let frame = regs["SS"].as_u64().unwrap() as usize * 16 + bp;
    let read = |disp: usize, len: usize| word(session.command(&serde_json::json!({"cmd": "mem_read_linear", "addr": frame - disp, "len": len}))["data"].as_str().unwrap());
    // `a` is at BP-2 (a word) and `b` at BP-6 (a dword): the cells the debug records give the two parameters.
    let cells: Vec<(String, i64)> = info.procedures.iter().find(|one| one.name == "add").unwrap().locals.iter().map(|one| (one.name.clone(), one.bp_offset)).collect();
    assert_eq!(cells, [("a".to_owned(), -2), ("b".to_owned(), -6)], "{cells:?}");
    assert_eq!((read(2, 2), read(6, 4)), (2, 3), "the cells at the body's first instruction");
}
