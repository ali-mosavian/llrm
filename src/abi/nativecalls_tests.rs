//! Port of the `tests/test_native_frame.py` tests that exercise this module.
//!
//! The four `nativeframe`-only tests live in `backend/nativeframe.rs`.
//! `test_private_calls_and_explicit_pascal_cleanup_balance_recursive_body`
//! keeps its cleanup, balance and interface half; its `mir.bodies` and
//! `lower.lowered` half is skipped until those are ported.
//! Skipped, needing `mir.bodies`, `lower` and `flow`:
//! `test_lowering_uses_the_same_per_site_clobbers_as_raising`,
//! `test_native_register_saves_survive_allocation`.

use std::path::Path;

use super::{cleanups, interfaces};
use crate::abi::runtime::{self, Memory};
use crate::backend::nativeframe;
use crate::frontend::blocks::{self, Block};
use crate::frontend::extent;
use crate::objectfile::module;
use crate::support::hash::IndexMap;

#[test]
fn test_private_calls_and_explicit_pascal_cleanup_balance_recursive_body() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/regressions/r_walk-borland.obj");
    let module = module::load(path).unwrap().unwrap();
    let partition = extent::partition(&module).unwrap();
    let mapped = blocks::code_map(&module).unwrap();
    let parts = blocks::partition(&module, &mapped);
    // Fresh r_bsp R_EMIT_ENTITIES returns with RETF 28, unlike the private RETs.
    let external: IndexMap<i64, i64> =
        module.calls.iter().filter(|(_, name)| *name == "R_EMIT_ENTITIES").map(|(&at, _)| (at, 28)).collect();
    let cleanup = cleanups(&module, &partition, &parts, &external);
    assert_eq!(cleanup[&0x3C4], 0);
    assert_eq!(cleanup[&0x4FE], 0);
    assert_eq!(cleanup[&0x45C], 28);
    assert!(!cleanups(&module, &partition, &parts, &IndexMap::default()).contains_key(&0x45C));
    let body = partition.bodies.iter().find(|body| body.seed == 0x334).unwrap();
    let owned: Vec<Block> = parts
        .iter()
        .filter(|block| body.ranges.iter().any(|&(lo, hi)| lo <= block.at && block.at < hi))
        .cloned()
        .collect();
    let plan = nativeframe::plan(&owned, body.seed).unwrap();
    let by_address: IndexMap<usize, i64> = cleanup.iter().map(|(&at, &size)| (at as usize, size)).collect();
    assert!(nativeframe::balanced(&owned, &plan, &by_address));
    let contracts = interfaces(&module, &partition, &parts, &runtime::for_module(&module, None).unwrap());
    assert_eq!(contracts[&0x3C4].cleanup, Some(0));
    assert!(!contracts[&0x3C4].established);
    assert_eq!(contracts[&0x3C4].writes, Memory::Any);
}
