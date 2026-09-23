import sys
import random
from pathlib import Path

from qbopt.model import mir
from qbopt.model import memory
from qbopt.analysis import alias
from qbopt.analysis import avail
from qbopt.analysis import consts
from qbopt.analysis import cellmap
from qbopt.frontend.qb import driver as qb_driver
from qbopt.frontend.qb import compile as qb_compile


def _cells_program(tmp_path: Path, scalars: int) -> Path:
    basic = tmp_path / "CELLS.BAS"
    total = " + ".join(f"x{k}" for k in range(scalars))
    lines = ["DEFINT A-Z", "DIM a(10)", *(f"x{k} = {k}" for k in range(scalars))]
    lines += ["FOR i = 0 TO 10: a(i) = i: NEXT", f"PRINT {total} + a(3)"]
    basic.write_bytes("\r\n".join(lines).encode() + b"\r\n")
    return basic


def _loop_program(tmp_path: Path, scalars: int) -> Path:
    basic = tmp_path / "LOOP.BAS"
    lines = ["DEFINT A-Z", "DIM a(10)", "FOR j = 1 TO 3", *(f"x{k} = x{k} + j: a(j) = x{k}" for k in range(scalars))]
    lines += ["NEXT", "PRINT " + " + ".join(f"x{k}" for k in range(scalars))]
    basic.write_bytes("\r\n".join(lines).encode() + b"\r\n")
    return basic


def _compiled(basic: Path) -> bytes:
    return qb_compile.object_bytes(qb_driver.parsed(basic, dialect="qb45", runtime="qb45"), basic.name)


def test_a_store_is_not_tested_against_every_known_cell(tmp_path, monkeypatch) -> None:
    """deedlines compiled for 45 minutes: each store asked may_overlap of
    every constant cell, 285K questions here for 24 scalars."""
    asked = 0
    original = consts._MemoryQueries.may_overlap

    def counted(self, where, ref):
        nonlocal asked
        asked += 1
        return original(self, where, ref)

    monkeypatch.setattr(consts._MemoryQueries, "may_overlap", counted)
    _compiled(_cells_program(tmp_path, 24))
    assert asked < 50_000, asked


def test_dead_stores_do_not_test_every_overwritten_cell(tmp_path, monkeypatch) -> None:
    """dead_stores was 44% of deedlines' compile: each access tested every
    cell overwritten below it, 17K overlap tests here for 24 scalars."""
    inside, asked = False, 0
    overlapping, dead_stores = mir.overlapping, avail.dead_stores

    def counted(*args, **named):
        nonlocal asked
        asked += inside
        return overlapping(*args, **named)

    def scoped(*args, **named):
        nonlocal inside
        inside = True
        try:
            return dead_stores(*args, **named)
        finally:
            inside = False

    monkeypatch.setattr(mir, "overlapping", counted)
    monkeypatch.setattr(avail, "dead_stores", scoped)
    _compiled(_loop_program(tmp_path, 24))
    assert asked < 5_000, asked


def test_a_write_does_not_ask_alias_of_every_bucket(tmp_path, monkeypatch) -> None:
    """Picking the buckets a write reaches asked objects_may_alias of every
    bucket for every store: 145K questions here, 27M in 5 min of deedlines."""
    asked = 0
    original = memory.objects_may_alias

    def counted(one, other):
        nonlocal asked
        asked += 1
        return original(one, other)

    monkeypatch.setattr(memory, "objects_may_alias", counted)
    _compiled(_cells_program(tmp_path, 24))
    assert asked < 50_000, asked


def test_picking_buckets_does_not_grow_with_the_cells_held(tmp_path, monkeypatch) -> None:
    """Cached per pair, picking buckets still scanned every bucket held: 327
    calls a write for 96 scalars, 19M scans in 5 min of deedlines' consts."""
    writes = calls = 0
    original = mir.overlap_buckets

    def count(frame, event, arg):
        nonlocal calls
        calls += 1

    def profiled(ref, cells):
        nonlocal writes
        writes += 1
        sys.setprofile(count)
        try:
            return original(ref, cells)
        finally:
            sys.setprofile(None)

    monkeypatch.setattr(mir, "overlap_buckets", profiled)
    _compiled(_cells_program(tmp_path, 96))
    assert calls / writes < 100, calls / writes


def test_skipped_buckets_hold_no_cell_the_store_reaches(tmp_path, monkeypatch) -> None:
    """The index only skips work: every cell it leaves untested, in consts
    and dead stores alike, is one the exact test says the write cannot reach."""
    original = cellmap.CellMap.kill
    checked = 0

    def verified(self, reached, overlaps):
        nonlocal checked
        if reached is not None:
            skipped = [key for bucket, keys in self.buckets.items() if bucket not in reached for key in keys]
            checked += len(skipped)
            assert not any(overlaps(key) for key in skipped)
        original(self, reached, overlaps)

    monkeypatch.setattr(cellmap.CellMap, "kill", verified)
    _compiled(_cells_program(tmp_path, 8))
    _compiled(_loop_program(tmp_path, 8))
    assert checked


def test_an_alias_store_kills_what_the_pairwise_scan_killed() -> None:
    """The alias walk rebuilt its cell map per store, testing all of it; the
    index tests only the stored key's object and must drop the same cells."""
    rng = random.Random(7)
    objects = [memory.Object(memory.Kind.GLOBAL, index) for index in range(6)]

    def key():
        if rng.random() < 0.5:
            low = rng.randrange(8)
            return (rng.choice(objects), low, low + rng.randrange(1, 4))
        return ("space", rng.randrange(3), rng.randrange(8), rng.randrange(1, 4))

    for _ in range(300):
        cells = {key(): rng.random() for _ in range(rng.randrange(1, 20))}
        stored = None if rng.random() < 0.1 else key()
        scanned = {old: fact for old, fact in cells.items() if old == stored or not alias._keys_overlap(old, stored)}
        asked = []
        indexed = cellmap.CellMap(alias._key_bucket, cells)
        original = alias._keys_overlap
        try:
            alias._keys_overlap = lambda one, other: asked.append(one) or original(one, other)
            alias._kill(indexed, stored)
        finally:
            alias._keys_overlap = original
        assert dict(indexed) == scanned
        if stored is not None:
            assert all(alias._key_bucket(one) == alias._key_bucket(stored) for one in asked)
