//! A variable of an inlined callee was written as a variable of the importing
//! function: `inc` listed `x`, which belongs to `helper`. `-g` x86-64 object.

mod common;

use std::process::Command;

const IR: &str = include_str!("../crates/ir/llrm-mir/tests/clang/dbg_inlined.ll");

#[test]
fn test_a_callee_variable_is_not_listed_under_the_function_it_was_inlined_into() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("inc.ll");
    let layout = format!("target datalayout = \"{}\"\ntarget triple = \"x86_64-unknown-linux-gnu\"", common::datalayout("x86-m64"));
    let text: String = IR
        .lines()
        .map(|line| if line.starts_with("target datalayout") { layout.clone() } else { line.to_owned() })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&input, text).unwrap();
    let object = directory.path().join("inc.o");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-O2", "-g", "-o"])
        .arg(&object)
        .arg(&input)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let Ok(dump) = Command::new("readelf").arg("--debug-dump=info").arg(&object).output() else {
        panic!("readelf is not installed");
    };
    let info = String::from_utf8_lossy(&dump.stdout);
    let mut names = Vec::new();
    let mut variable = false;
    for line in info.lines() {
        if line.contains("Abbrev Number") {
            variable = line.contains("DW_TAG_variable") || line.contains("DW_TAG_formal_parameter");
        } else if variable && line.contains("DW_AT_name") {
            names.push(line.rsplit(": ").next().unwrap_or("").trim().to_owned());
        }
    }
    names.sort();
    assert_eq!(names, ["k", "m", "n"], "{info}");
}
