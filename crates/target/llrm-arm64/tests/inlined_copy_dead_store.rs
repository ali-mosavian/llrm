//! A body the -Os trial inlined went through the cleanup list only, so the
//! dead first store of two inlined `next()` calls (a volatile load between
//! them) stayed: 2 stores to `state` per iteration, the pin's DSE leaves 1.

#[test]
fn the_dead_store_of_an_inlined_copy_is_removed() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, include_str!("data/inlined_copy_dead_store.ll")).unwrap();
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-Os", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let stores = std::fs::read_to_string(&output)
        .unwrap()
        .lines()
        .filter(|line| line.trim_start().starts_with("str ") && line.contains("state@PAGEOFF"))
        .count();
    assert_eq!(stores, 1, "one store of `state` per iteration");
}
