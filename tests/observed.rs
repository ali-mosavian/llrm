//! `-g`: a variable the program declares for a debugger stays in its frame cell as the source
//! writes it. A debugger reads the cell at any line, so no store to it may be dropped, merged or
//! moved: they are marked volatile where the frontend lowers them, and every pass already keeps those.
//! What the optimiser leaves is read in the last stage the pipeline dumps, whichever object format
//! (CodeView, DWARF) then describes the cells.

use std::path::{Path, PathBuf};
use std::process::Command;

fn llrm_c() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap().join("llrm-c")
}

/// The pipeline's last dumped MIR of `source` compiled with `arguments`.
fn final_mir(source: &Path, arguments: &[&str], dump: &Path) -> String {
    let made = Command::new(llrm_c()).args(arguments).arg("--dump").arg(dump).arg(source).arg("-o").arg(dump.join("x.o")).output().unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let mut stages: Vec<PathBuf> = std::fs::read_dir(dump).unwrap().map(|one| one.unwrap().path()).filter(|one| one.extension().is_some_and(|ext| ext == "ll")).collect();
    stages.sort();
    std::fs::read_to_string(stages.last().expect("a stage")).unwrap()
}

/// The (alloca, stores, volatile stores) of each alloca a `#dbg_declare` record names, in every function
/// (a value's name is its function's).
fn declared_stores(mir: &str) -> Vec<(String, usize, usize)> {
    let mut found = Vec::new();
    for function in mir.split("\ndefine ").skip(1) {
        let function = function.split("\n}").next().unwrap();
        let declared: Vec<String> = function.lines().filter_map(|line| line.trim().strip_prefix("#dbg_declare(ptr ")).map(|rest| rest.split(',').next().unwrap().to_owned()).collect();
        for name in declared {
            // The variable's own address and every one made from it by an offset: a store through either is a store to it.
            let mut through = vec![name.clone()];
            for line in function.lines().map(str::trim) {
                if let Some((result, rest)) = line.split_once(" = getelementptr ")
                    && through.iter().any(|one| rest.contains(&format!("ptr {one},")))
                {
                    through.push(result.to_owned());
                }
            }
            let stores: Vec<&str> = function.lines().map(str::trim).filter(|line| line.starts_with("store ") && through.iter().any(|one| line.contains(&format!("ptr {one},")))).collect();
            let volatile = stores.iter().filter(|line| line.starts_with("store volatile ")).count();
            found.push((name, stores.len(), volatile));
        }
    }
    found
}

/// At -O2 every store to a declared variable is volatile in the MIR the optimiser leaves, and a
/// variable the program writes has some. Before, promotion and the loop's store sink moved the
/// stores of `s` and `i` out of the loop and the cells held their first values until it ended, so a
/// debugger read `s = 0, i = 0` through all of it.
#[test]
fn every_store_to_a_declared_variable_stays_at_o2() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    for (source, level) in [("observed.c", "-O2"), ("observed.c", "-O0")] {
        let mir = final_mir(&fixtures.join(source), &["-m32", level, "-g"], scratch.path());
        let stores = declared_stores(&mir);
        assert!(stores.len() >= 4, "{source} {level}: the premise, declared variables: {stores:?}");
        for (name, all, volatile) in &stores {
            assert_eq!(all, volatile, "{source} {level}: {name} has a store that is not volatile");
        }
        assert!(stores.iter().any(|(_, all, _)| *all > 0), "{source} {level}: no declared variable is written: {stores:?}");
    }
}

/// Without -g nothing is marked: the code of a program is what it was.
#[test]
fn nothing_is_volatile_without_g() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dwarf");
    let scratch = tempfile::tempdir().unwrap();
    let mir = final_mir(&fixtures.join("observed.c"), &["-m32", "-O2"], scratch.path());
    assert!(!mir.contains("store volatile"), "a volatile store without -g");
}
