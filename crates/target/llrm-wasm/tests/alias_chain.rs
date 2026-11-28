use std::path::Path;
use std::process::Command;

/// `WebAssemblySubtarget::useAA` is true: the pin's chain walk asks alias
/// analysis, so a store waits for its own load only. Without it nbody_fixed
/// -O1 chained `vy`'s load behind `vx`'s store through both loads, loaded
/// both before the first store and spent a local on it (4 B over the pin).
#[test]
fn a_store_does_not_wait_for_a_load_of_another_global() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/alias_chain.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O1", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let accesses: Vec<&str> = asm
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("i32.load") || line.starts_with("i32.store"))
        .collect();
    let kinds: Vec<&str> = accesses.iter().map(|line| line.split_whitespace().next().unwrap()).collect();
    assert_eq!(kinds, ["i32.load", "i32.store", "i32.load", "i32.store"], "{asm}");
}

/// Thirty adjacent halfword copies: the pin merges the first run to 64-bit
/// moves, then the rest (`h q q q q q q w h w`). Ours left the tail as
/// halfwords (`... w h h h`): `getStoreSource` takes any `LOAD`, and after
/// type legalization the loads are extending ones, which ours refused.
#[test]
fn a_run_of_halfword_copies_is_merged_to_the_end() {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir/store_merge_run.ll");
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-o", "-"])
        .arg(ir)
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    let asm = String::from_utf8_lossy(&out.stdout).into_owned();
    let kinds: Vec<&str> = asm
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("i32.load") || line.starts_with("i64.load"))
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    let want = [
        "i32.load16_u",
        "i64.load",
        "i64.load",
        "i64.load",
        "i64.load",
        "i64.load",
        "i64.load",
        "i32.load",
        "i32.load16_u",
        "i32.load",
    ];
    assert_eq!(kinds, want, "{asm}");
}

/// The loads and stores of `ir` at -O2, in the order the backend emits them.
fn access_kinds(ir: &str) -> Vec<String> {
    let ir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir").join(ir);
    let out = Command::new(env!("CARGO_BIN_EXE_llrm-wasm-llc"))
        .args(["-S", "-O2", "-o", "-"])
        .arg(ir)
        .env("LLRM_WASM_BACKEND_ONLY", "1")
        .output()
        .expect("llrm-wasm-llc");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("i32.load") || line.starts_with("i32.store"))
        .map(|line| line.split_whitespace().next().unwrap().to_owned())
        .collect()
}

/// A store through a pointer that steps through `@line` (`phi [@line, gep phi
/// 3]`) does not touch a load from `@planes`: the pin's `aliasPHI` says so, the
/// load follows the first store and needs no local. The DAG's oracle looked
/// through no phi: both loads went above the first store, one held in a local
/// (jpeg_fixed -O2: two extra `local.set`/`local.get` in 49,152 iterations).
#[test]
fn a_store_through_a_stepping_phi_does_not_wait_for_a_load_of_another_global() {
    assert_eq!(access_kinds("alias_phi.ll"), ["i32.load8_u", "i32.store8", "i32.load8_u", "i32.store8"]);
}

/// The same loop where the phi may also point where the loads do (`phi [%q,
/// gep phi 3]`, `%q` a parameter): `aliasPHI` stays MayAlias, both loads stay
/// above the first store.
#[test]
fn a_phi_with_a_source_that_may_alias_still_orders_the_loads_first() {
    assert_eq!(access_kinds("alias_phi_may.ll"), ["i32.load8_u", "i32.load8_u", "i32.store8", "i32.store8"]);
}
