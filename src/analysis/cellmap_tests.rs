//! Port of tests/test_cellmap.py.
//!
//! Skipped: `test_alias_classes_are_not_rebuilt_per_question`. It pins
//! Python's lru_cache of a write's alias classes; Rust's `alias_class` is a
//! field copy and has no such cache.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::thread::LocalKey;

use super::{VERIFIED, VERIFYING};
use crate::analysis::alias::{_key_bucket, _key_place, _keys_overlap, _kill, ASKED, CellKey};
use crate::analysis::avail::DEAD_OVERLAPS;
use crate::analysis::cellmap::CellMap;
use crate::analysis::consts::MAY_OVERLAP;
use crate::analysis::regions::{PICKED, displaced_buckets, overlap_bucket, overlap_span, overlapping};
use crate::frontends::qb::{compile as qb_compile, driver as qb_driver};
use crate::model::memory::{Identity, MemoryKind, MemoryObject, OBJECT_ALIASES};
use crate::model::passes::O2;
use crate::model::mir::{MemRef, Value};
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::IndexMap;

fn written(directory: &tempfile::TempDir, name: &str, lines: &[String]) -> PathBuf {
    let path = directory.path().join(name);
    std::fs::write(&path, format!("{}\r\n", lines.join("\r\n"))).expect("writes the source");
    path
}

fn cells_program(directory: &tempfile::TempDir, scalars: usize) -> PathBuf {
    let total = (0..scalars).map(|k| format!("x{k}")).collect::<Vec<_>>().join(" + ");
    let mut lines = vec!["DEFINT A-Z".to_owned(), "DIM a(10)".to_owned()];
    lines.extend((0..scalars).map(|k| format!("x{k} = {k}")));
    lines.extend(["FOR i = 0 TO 10: a(i) = i: NEXT".to_owned(), format!("PRINT {total} + a(3)")]);
    written(directory, "CELLS.BAS", &lines)
}

fn loop_program(directory: &tempfile::TempDir, scalars: usize) -> PathBuf {
    let mut lines = vec!["DEFINT A-Z".to_owned(), "DIM a(10)".to_owned(), "FOR j = 1 TO 3".to_owned()];
    lines.extend((0..scalars).map(|k| format!("x{k} = x{k} + j: a(j) = x{k}")));
    lines.extend([
        "NEXT".to_owned(),
        format!("PRINT {}", (0..scalars).map(|k| format!("x{k}")).collect::<Vec<_>>().join(" + ")),
    ]);
    written(directory, "LOOP.BAS", &lines)
}

fn elements_program(directory: &tempfile::TempDir, elements: usize) -> PathBuf {
    let mut lines = vec!["DEFINT A-Z".to_owned(), format!("DIM a({elements})")];
    lines.extend((0..elements).map(|k| format!("a({k}) = {k}")));
    lines.push(format!("PRINT {}", (0..elements).map(|k| format!("a({k})")).collect::<Vec<_>>().join(" + ")));
    written(directory, "ELEMENTS.BAS", &lines)
}

fn compiled(basic: &Path) -> Vec<u8> {
    let program = qb_driver::parsed(basic, &qb_driver::Frontend::new("qb45", "qb45"), None)
        .unwrap_or_else(|error| panic!("{}: {error}", basic.display()));
    qb_compile::object_bytes(&program, Path::new(basic.file_name().expect("a file")), None, &O2()).expect("compiles")
}

/// What `counter` counted while compiling `basic`.
fn counted(counter: &'static LocalKey<Cell<usize>>, basic: &Path) -> usize {
    counter.with(|count| count.set(0));
    compiled(basic);
    counter.with(Cell::get)
}

#[test]
fn a_store_is_not_tested_against_every_known_cell() {
    // deedlines compiled for 45 minutes: each store asked may_overlap of
    // every constant cell, 285K questions here for 24 scalars.
    let directory = tempfile::TempDir::new().unwrap();
    let asked = counted(&MAY_OVERLAP, &cells_program(&directory, 24));
    assert!(asked < 50_000, "{asked}");
}

#[test]
fn dead_stores_do_not_test_every_overwritten_cell() {
    // dead_stores was 44% of deedlines' compile: each access tested every
    // cell overwritten below it, 17K overlap tests here for 24 scalars.
    let directory = tempfile::TempDir::new().unwrap();
    let asked = counted(&DEAD_OVERLAPS, &loop_program(&directory, 24));
    assert!(asked < 5_000, "{asked}");
}

#[test]
fn a_direct_store_asks_only_the_cells_it_meets() {
    // matmul.nib's array cells name no object, so the object index skipped
    // none of them: every store into the frame asked may_overlap of every
    // cell there, 60K questions here for 24 elements.
    let directory = tempfile::TempDir::new().unwrap();
    let asked = counted(&MAY_OVERLAP, &elements_program(&directory, 24));
    assert!(asked < 10_000, "{asked}");
}

#[test]
fn a_write_does_not_ask_alias_of_every_bucket() {
    // Picking the buckets a write reaches asked objects_may_alias of every
    // bucket for every store: 145K questions here, 27M in 5 min of deedlines.
    let directory = tempfile::TempDir::new().unwrap();
    let asked = counted(&OBJECT_ALIASES, &cells_program(&directory, 24));
    assert!(asked < 50_000, "{asked}");
}

#[test]
fn picking_buckets_does_not_grow_with_the_cells_held() {
    // Cached per pair, picking buckets still scanned every bucket held: 327
    // calls a write for 96 scalars, 19M scans in 5 min of deedlines' consts.
    let directory = tempfile::TempDir::new().unwrap();
    let basic = cells_program(&directory, 96);
    PICKED.with(|picked| picked.set((0, 0)));
    compiled(&basic);
    let (writes, scanned) = PICKED.with(Cell::get);
    assert!(writes > 0 && scanned < 100 * writes, "{scanned} / {writes}");
}

#[test]
fn skipped_buckets_hold_no_cell_the_store_reaches() {
    // The index only skips work: every cell it leaves untested, in consts
    // and dead stores alike, is one the exact test says the write cannot reach.
    let directory = tempfile::TempDir::new().unwrap();
    VERIFYING.with(|on| on.set(true));
    VERIFIED.with(|checked| checked.set(0));
    compiled(&cells_program(&directory, 8));
    compiled(&loop_program(&directory, 8));
    VERIFYING.with(|on| on.set(false));
    assert!(VERIFIED.with(Cell::get) > 0);
}

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
    let mut reference = |random: &mut dyn FnMut(u64) -> u64| {
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
