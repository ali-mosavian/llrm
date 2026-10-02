//! Port of tests/test_cellmap.py.
//!
//! Skipped: `test_alias_classes_are_not_rebuilt_per_question`. It pins
//! Python's lru_cache of a write's alias classes; Rust's `alias_class` is a
//! field copy and has no such cache.

use llrm_core::analysis::alias::{_key_bucket, _key_place, _keys_overlap, _kill, ASKED, CellKey};
use llrm_core::analysis::cellmap::CellMap;
use llrm_core::analysis::regions::{displaced_buckets, overlap_bucket, overlap_span, overlapping};
use llrm_core::model::memory::{Identity, MemoryKind, MemoryObject};
use llrm_core::model::mir::{MemRef, Value};
use llrm_core::objectfile::module::{Addr, Space};
use llrm_core::support::hash::IndexMap;

#[test]
fn an_alias_store_kills_what_the_pairwise_scan_killed() {
    // The alias walk rebuilt its cell map per store, testing all of it; the
    // index tests only the stored key's object and must drop the same cells.
    let mut state = 7_u64;
    let mut random = move |below: u64| {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % below
    };
    let objects = (0..6)
        .map(|index| MemoryObject { identity: Some(Identity::Int(index)), ..MemoryObject::new(MemoryKind::Global) })
        .collect::<Vec<_>>();
    let key = |random: &mut dyn FnMut(u64) -> u64| {
        if random(2) == 0 {
            let low = random(8) as i64;
            CellKey::Object(objects[random(6) as usize].clone(), low, low + 1 + random(3) as i64)
        } else {
            let spaces = [Space::Segment, Space::Frame, Space::Group];
            CellKey::Address(spaces[random(3) as usize], random(8) as i64, random(8) as i64, 1 + random(3) as i64)
        }
    };
    for _ in 0..300 {
        let size = 1 + random(19);
        let cells = (0..size).map(|n| (key(&mut random), n as i64)).collect::<IndexMap<_, _>>();
        let stored = if random(10) == 0 { None } else { Some(key(&mut random)) };
        let scanned = cells
            .iter()
            .filter(|(old, _)| Some(*old) == stored.as_ref() || !_keys_overlap(Some(old), stored.as_ref()))
            .map(|(old, fact)| (old.clone(), *fact))
            .collect::<IndexMap<_, _>>();
        let mut indexed = CellMap::new(cells, _key_place);
        ASKED.with(|asked| asked.borrow_mut().clear());
        _kill(&mut indexed, stored.as_ref());
        assert!(indexed.iter().eq(scanned.iter()));
        if let Some(stored) = &stored {
            ASKED.with(|asked| {
                assert!(asked.borrow().iter().all(|one| _key_bucket(one) == _key_bucket(stored)));
            });
        }
    }
}

#[test]
fn a_displaced_store_kills_what_the_full_scan_killed() {
    // Looking a direct write's frame up by displacement must drop exactly
    // the cells, in the same order, that asking every cell drops.
    let mut state = 11_u64;
    let mut random = move |below: u64| {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % below
    };
    let values = [Value::new(0, 0), Value::new(1, 0)];
    let reference = |random: &mut dyn FnMut(u64) -> u64| {
        let space = [Space::Frame, Space::Segment, Space::Far][random(3) as usize];
        let mut addr = Addr::new(space, random(32) as i64 - 8);
        addr.index = random(2) as i64;
        let mut one = MemRef::new(Some(addr), 1 + random(8) as u32);
        one.segment = (space == Space::Far && random(2) == 0).then_some(values[0]);
        one.base = (random(3) == 0).then_some(values[1]);
        one
    };
    let mut displaced = 0;
    for _ in 0..300 {
        let size = 1 + random(39);
        let cells = (0..size).map(|n| (reference(&mut random), n as i64)).collect::<IndexMap<_, _>>();
        let write = reference(&mut random);
        let overlaps = |one: &MemRef| overlapping(one, &write, None, None, None).unwrap_or(true);
        let scanned =
            cells.iter().filter(|(one, _)| !overlaps(one)).map(|(one, at)| (one.clone(), *at)).collect::<Vec<_>>();
        let mut indexed = CellMap::new(cells, |one| (overlap_bucket(one), overlap_span(one)));
        let near = displaced_buckets(&write, &indexed.parts);
        displaced += usize::from(near.is_some());
        indexed.kill(None, overlaps, near);
        assert!(indexed.iter().map(|(one, at)| (one.clone(), *at)).eq(scanned));
    }
    assert!(displaced > 100, "{displaced}");
}
