//! `LowerBUILD_VECTOR`: one constant in several lanes is the base, the other
//! lanes are inserted.

fn instructions(function: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/build_vector_constant_base.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let asm = std::fs::read_to_string(&output).unwrap();
    let body = asm.split(&format!("{function}:")).nth(1).expect("premise: the function is there");
    body.lines()
        .skip(1)
        .take_while(|line| !(line.ends_with(':') && line.starts_with(|c: char| c.is_alphabetic())))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('.') && !line.starts_with("//"))
        .map(str::to_owned)
        .collect()
}

/// QuickrBasic a64 -O2 (table.c `tabl_moved`): a reduction's start vector
/// `<x, 0, 0, 0>` was built with a lane insert of `wzr` each, 5 instructions
/// where the pin has `movi` and one insert (3), 76 B more in the function.
#[test]
fn a_vector_of_zeros_and_one_value_starts_from_movi() {
    let f = instructions("f");
    assert!(f[0].starts_with("movi"), "{f:?}");
    assert!(f.iter().all(|line| !line.contains("wzr")), "{f:?}");
    assert_eq!(f.len(), 4, "movi, ins, str, ret: {f:?}");
}

/// The same for another constant, `<7, x, 7, y>`.
#[test]
fn a_vector_of_one_constant_and_two_values_starts_from_the_constant() {
    let g = instructions("g");
    assert!(g[0].starts_with("movi"), "{g:?}");
    assert_eq!(g.len(), 5, "movi, ins, ins, str, ret: {g:?}");
}
