//! Every x87 operation a program can reach has a price, found by compiling the
//! operations rather than listing the pricer's: at -Os under `LLRM_VERIFY=1` a
//! function with an instruction the encoder cannot price panics ("no byte price
//! for ... flog2", llrm#1307, found on a demo), and without it was priced as
//! nothing. The QB programs are made from qbfront's own table of intrinsics and
//! the C ones from the operators and types C has, so a new operation reaches
//! the sweep without anyone adding it to a list.

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

fn bin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join(name)
}

/// `tool` compiling `source` for `flags` at -Os with every invariant checked;
/// the complaint if it failed.
fn compiled(
    tool: &str,
    source: &str,
    extension: &str,
    flags: &[&str],
) -> Result<(), String> {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join(format!("x87.{extension}"));
    std::fs::write(&path, source).unwrap();
    // llrm-c is gone: its command line is served by llrm-cc over the pinned
    // clang.
    let done = if tool == "llrm-c" {
        common::cc::llrm_c()
            .arg(&path)
            .args(flags)
            .arg("-o")
            .arg(scratch.path().join("x87.obj"))
            .env("LLRM_VERIFY", "1")
            .output()
            .unwrap()
    } else {
        Command::new(bin(tool))
            .arg(&path)
            .args(flags)
            .arg("-o")
            .arg(scratch.path().join("x87.obj"))
            .env("LLRM_VERIFY", "1")
            .output()
            .unwrap()
    };
    if done.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&done.stderr).into_owned()) }
}

/// The one-argument numeric intrinsics of QuickBASIC that need no runtime, from
/// qbfront's table.
fn qb_functions() -> Vec<String> {
    let table =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/frontends/qb/src/intrinsics.rs"))
            .unwrap();
    let mut names = Vec::new();
    for line in table.lines() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix("intrinsic!(\"") else { continue };
        let Some((name, rest)) = rest.split_once('"') else { continue };
        let rest = rest.trim_start_matches(", ");
        if rest.starts_with("1..=1, DynamicNumeric, Pure")
            || rest.starts_with("1..=1, Single, Pure")
            || rest.starts_with("1..=1, Double, Pure")
        {
            names.push(name.to_owned());
        }
    }
    names
}

#[test]
fn test_every_numeric_qb_function_of_every_numeric_type_is_priced_at_os() {
    let names = qb_functions();
    assert!(
        ["ABS", "ATN", "COS", "EXP", "LOG", "SIN", "SQR", "TAN"].iter().all(|one| names.iter().any(|name| name == one)),
        "{names:?}"
    );
    let mut declarations = String::new();
    let (mut calls, mut bodies) = (String::new(), String::new());
    for (number, (name, ty)) in
        names.iter().flat_map(|name| ["SINGLE", "DOUBLE", "INTEGER", "LONG"].map(|ty| (name, ty))).enumerate()
    {
        declarations.push_str(&format!("DECLARE SUB S{number} (x AS {ty})\n"));
        calls.push_str(&format!("S{number} 1\n"));
        bodies.push_str(&format!("SUB S{number} (x AS {ty})\n  PRINT {name}(x)\nEND SUB\n"));
    }
    let source = format!("{declarations}{calls}END\n{bodies}");
    if let Err(why) = compiled("llrm-qb", &source, "bas", &["--dialect", "qb45", "--runtime", "qb45", "-Os"]) {
        panic!("{why}");
    }
}

/// Functions over every float type: each operator with the operands in a
/// register pair, a global, a constant, and on either side; negation; each
/// comparison; every conversion to and from every integer type; and between the
/// float types.
fn c_operations() -> String {
    let floats = ["float", "double", "long double"];
    let integers = [
        "signed char",
        "unsigned char",
        "short",
        "unsigned short",
        "int",
        "unsigned int",
        "long",
        "unsigned long",
        "long long",
        "unsigned long long",
    ];
    let mut out = Vec::new();
    let mut function = |ret: &str, params: &str, body: &str| {
        out.push(format!("{ret} f{}({params}) {{ {body} }}", out.len()));
    };
    let mut globals = Vec::new();
    for ty in floats {
        let key = ty.replace(' ', "_");
        globals.push(format!("{ty} g_{key}_a = 1.5, g_{key}_b = 2.5;"));
    }
    for ty in floats {
        let key = ty.replace(' ', "_");
        for op in ["+", "-", "*", "/"] {
            function(ty, &format!("{ty} a, {ty} b"), &format!("return a {op} b;"));
            function(ty, &format!("{ty} a"), &format!("return a {op} g_{key}_a;"));
            function(ty, &format!("{ty} a"), &format!("return g_{key}_a {op} a;"));
            function(ty, &format!("{ty} a"), &format!("return a {op} 2.5;"));
            function(ty, &format!("{ty} a"), &format!("return 2.5 {op} a;"));
            function(ty, &format!("{ty} a"), &format!("return 1.0 {op} a;"));
            function(ty, "void", &format!("return g_{key}_a {op} g_{key}_b;"));
        }
        function(ty, &format!("{ty} a"), "return -a;");
        for compare in ["<", "<=", "==", "!=", ">", ">="] {
            function("int", &format!("{ty} a, {ty} b"), &format!("return a {compare} b;"));
            function("int", &format!("{ty} a"), &format!("return a {compare} g_{key}_a;"));
            function("int", &format!("{ty} a"), &format!("return a {compare} 0.0;"));
        }
        for int in integers {
            function(int, &format!("{ty} a"), &format!("return ({int})a;"));
            function(ty, &format!("{int} a"), &format!("return ({ty})a;"));
            function(int, "void", &format!("return ({int})g_{key}_a;"));
        }
        for other in floats.iter().filter(|other| **other != ty) {
            function(other, &format!("{ty} a"), &format!("return ({other})a;"));
        }
    }
    format!("{}\n{}\n", globals.join("\n"), out.join("\n"))
}

#[test]
fn test_every_c_float_operation_is_priced_at_os_in_both_modes() {
    let source = c_operations();
    for mode in ["-m16", "-m32"] {
        if let Err(why) = compiled("llrm-c", &source, "c", &[mode, "-Os"]) {
            panic!("{mode}: {why}");
        }
    }
}
