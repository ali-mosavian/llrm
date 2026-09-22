import random
from pathlib import Path

from qbopt.model import memory
from qbopt.analysis import alias
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
    basic = _cells_program(tmp_path, 24)
    qb_compile.object_bytes(qb_driver.parsed(basic, dialect="qb45", runtime="qb45"), basic.name)
    assert asked < 50_000, asked


def test_skipped_buckets_hold_no_cell_the_store_reaches(tmp_path, monkeypatch) -> None:
    """The index only skips work: every cell it leaves untested is one
    mir.overlapping says the store cannot reach."""
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
    basic = _cells_program(tmp_path, 8)
    qb_compile.object_bytes(qb_driver.parsed(basic, dialect="qb45", runtime="qb45"), basic.name)
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
